//! Core logic of GitHub Release Tracker: no Windows code in here, so it can be unit-tested anywhere.
//!
//! A port of the Python v1.1.1 logic: repo parsing, release lookup (feed first, API as fallback),
//! update detection, safe import/export, sorting and status text. `projects.json` stays compatible
//! with the Python version.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::fmt;

pub const APP_NAME: &str = "GitHub Release Tracker";
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const REPO_URL: &str = "https://github.com/santosasarno-code/github-release-tracker-rs";
pub const NO_DESCRIPTION: &str = "No description, website, or topics provided.";
pub const MAX_PROJECTS: usize = 2000;
pub const MAX_IMPORT_BYTES: u64 = 5 * 1024 * 1024;
pub const MAX_CUSTOM_NAME: usize = 80;
pub const SORTABLE: [&str; 5] = ["name", "repo", "source", "published", "status"];

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, PartialEq)]
pub enum GitHubError {
    NotFound(String),
    RateLimit(String),
    /// The server could not be reached at all (DNS, refused, timeout, certificate).
    Network(String),
    /// The project exists but has no official release (only tags, or nothing).
    NoReleases(String),
    Other(String),
}

impl GitHubError {
    pub fn not_found() -> Self {
        GitHubError::NotFound("Not found (wrong name, or a private repository).".into())
    }
}

impl fmt::Display for GitHubError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitHubError::NotFound(m)
            | GitHubError::RateLimit(m)
            | GitHubError::Network(m)
            | GitHubError::NoReleases(m)
            | GitHubError::Other(m) => f.write_str(m),
        }
    }
}

/// What the program needs from the network. The Windows build implements this with WinHTTP.
pub trait Net {
    /// GET https://api.github.com{path}; parsed JSON.
    fn api_get(&self, path: &str, token: &str) -> Result<Value, GitHubError>;
    /// GET the public releases.atom feed of owner/repo; the body as text.
    fn feed_get(&self, owner: &str, repo: &str) -> Result<String, GitHubError>;
    /// GET a public JSON document from any host (GitLab, Gitea / Forgejo, ...); no credentials are sent.
    fn http_json(&self, url: &str) -> Result<Value, GitHubError>;
    /// API calls left this hour, as reported by the last API response.
    fn rate_remaining(&self) -> Option<i64>;
}

// ---------------------------------------------------------------------------
// Data model (field order = key order written to projects.json, same as Python)
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Id {
    Int(i64),
    Str(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Release {
    pub id: Id,
    pub tag: String,
    pub name: String,
    pub published_at: String,
    pub url: String,
    pub prerelease: bool,
    pub source: String, // "release" | "tag" | "feed"
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub owner: String,
    pub repo: String,
    /// Only for projects that are not on github.com: "codeberg.org", or "http://host:3000" when typed so.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    /// "gitlab" or "gitea" (Forgejo / Codeberg speak the Gitea API). Present together with `host`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub title: String,
    pub description: String,
    pub html_url: String,
    pub include_prereleases: bool,
    pub fmt: Option<i64>,
    pub latest: Option<Release>,
    pub seen_id: Option<Id>,
    pub seen_tag: Option<String>,
    pub last_checked: String,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Settings {
    pub token: String,
    pub check_on_launch: bool,
    pub theme: String, // "light" | "dark" | ""
    pub sort_col: String,
    pub sort_asc: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { token: String::new(), check_on_launch: false, theme: String::new(), sort_col: String::new(), sort_asc: true }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Data {
    pub settings: Settings,
    pub projects: Vec<Project>,
}

impl Default for Data {
    fn default() -> Self {
        Data { settings: Settings::default(), projects: Vec::new() }
    }
}

// ---------------------------------------------------------------------------
// Small text helpers
// ---------------------------------------------------------------------------
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-'
}

/// `^[A-Za-z0-9_.-]{1,100}$`
pub fn valid_name(s: &str) -> bool {
    let n = s.chars().count();
    (1..=100).contains(&n) && s.chars().all(is_name_char) && !s.chars().all(|c| c == '.') // no "." / ".." (path tricks)
}

fn take_name(s: &str) -> (&str, &str) {
    let end = s.char_indices().find(|(_, c)| !is_name_char(*c)).map(|(i, _)| i).unwrap_or(s.len());
    s.split_at(end)
}

/// Accept `https://github.com/owner/repo[/...]` or `owner/repo`.
pub fn parse_repo(text: &str) -> Option<(String, String)> {
    let text = text.trim();
    let mut rest = text;
    if let Some(r) = rest.strip_prefix("https://") {
        rest = r;
    } else if let Some(r) = rest.strip_prefix("http://") {
        rest = r;
    }
    if let Some(r) = rest.strip_prefix("www.") {
        rest = r;
    }
    let found = if let Some(r) = rest.strip_prefix("github.com/") {
        let (owner, r) = take_name(r);
        let r = r.strip_prefix('/');
        match r {
            Some(r) if !owner.is_empty() => {
                let (repo, _) = take_name(r);
                if repo.is_empty() { None } else { Some((owner, repo)) }
            }
            _ => None,
        }
    } else {
        None
    };
    let (owner, repo) = match found {
        Some(x) => x,
        None => {
            let (owner, r) = take_name(text);
            let r = r.strip_prefix('/')?;
            let (repo, tail) = take_name(r);
            if owner.is_empty() || repo.is_empty() || !tail.is_empty() {
                return None;
            }
            (owner, repo)
        }
    };
    let repo = repo.strip_suffix(".git").unwrap_or(repo);
    if repo.is_empty() {
        return None;
    }
    Some((owner.to_string(), repo.to_string()))
}

fn trim_chars(s: &str, limit: usize) -> String {
    s.trim().chars().take(limit).collect()
}

fn str_field(v: Option<&Value>, limit: usize) -> String {
    match v {
        Some(Value::String(s)) => trim_chars(s, limit),
        _ => String::new(),
    }
}

/// Only ever open links that live on the project's own host, whatever a JSON file says.
pub fn safe_url(url: Option<&str>, base: &str, fallback: &str) -> String {
    match url {
        Some(u) if u.starts_with(&format!("{}/", base)) && u.chars().count() < 500 && !u.chars().any(|c| c.is_whitespace()) => u.to_string(),
        _ => fallback.to_string(),
    }
}

pub fn safe_github_url(url: Option<&str>, fallback: &str) -> String {
    safe_url(url, "https://github.com", fallback)
}

fn id_value(v: Option<&Value>) -> Option<Id> {
    match v {
        Some(Value::Number(n)) => n.as_i64().map(Id::Int),
        Some(Value::String(s)) => Some(Id::Str(s.chars().take(300).collect())),
        _ => None,
    }
}

/// One line, single spaces, at most MAX_CUSTOM_NAME characters.
pub fn clean_name(s: &str) -> String {
    let joined = s.split_whitespace().collect::<Vec<_>>().join(" ");
    joined.chars().take(MAX_CUSTOM_NAME).collect::<String>().trim().to_string()
}

pub fn url_quote(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'-' || b == b'~' {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

pub fn url_unquote(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let h = std::str::from_utf8(&b[i + 1..i + 3]).ok().and_then(|h| u8::from_str_radix(h, 16).ok());
            if let Some(v) = h {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---------------------------------------------------------------------------
// Project helpers
// ---------------------------------------------------------------------------
fn host_of(p: &Project) -> Option<&str> {
    p.host.as_deref().filter(|h| !h.is_empty())
}

pub fn project_key(p: &Project) -> String {
    match host_of(p) {
        Some(h) => format!("{}/{}/{}", host_label(h), p.owner, p.repo).to_lowercase(),
        None => format!("{}/{}", p.owner, p.repo).to_lowercase(),
    }
}

pub fn project_base(p: &Project) -> String {
    match host_of(p) {
        Some(h) => host_base(h),
        None => "https://github.com".to_string(),
    }
}

pub fn repo_page(p: &Project) -> String {
    format!("{}/{}/{}", project_base(p), p.owner, p.repo)
}

/// The page listing the project's releases (used when no exact release link is known).
pub fn release_page(p: &Project) -> String {
    let tail = if p.kind.as_deref() == Some("gitlab") { "/-/releases" } else { "/releases" };
    format!("{}{}", repo_page(p), tail)
}

/// What the Repository column shows: owner/repo, with the host in front for other hosts.
pub fn repo_label(p: &Project) -> String {
    match host_of(p) {
        Some(h) => format!("{}/{}/{}", host_label(h), p.owner, p.repo),
        None => format!("{}/{}", p.owner, p.repo),
    }
}

/// What the Repository column shows: owner/repo (the host is in the Source column).
pub fn repo_path(p: &Project) -> String {
    format!("{}/{}", p.owner, p.repo)
}

/// Where the project lives: GitHub, GitLab, Codeberg, or the server's own address.
pub fn source_label(p: &Project) -> String {
    match host_of(p) {
        None => "GitHub".to_string(),
        Some(h) => {
            let shown = host_label(h);
            match shown.to_lowercase().as_str() {
                "github.com" => "GitHub".to_string(),
                "gitlab.com" => "GitLab".to_string(),
                "codeberg.org" => "Codeberg".to_string(),
                _ => shown.to_string(),
            }
        }
    }
}

/// Order of the Source column: GitHub, GitLab, Codeberg, then other servers A-Z.
fn source_key(p: &Project) -> (usize, String) {
    let name = source_label(p);
    let rank = ["GitHub", "GitLab", "Codeberg"].iter().position(|n| *n == name).unwrap_or(3);
    (rank, name.to_lowercase())
}

pub fn display_name(p: &Project) -> String {
    if let Some(c) = &p.custom_name {
        if !c.is_empty() {
            return c.clone();
        }
    }
    if p.title.is_empty() { p.repo.clone() } else { p.title.clone() }
}

pub fn has_update(p: &Project) -> bool {
    match &p.latest {
        Some(l) => Some(&l.id) != p.seen_id.as_ref(),
        None => false,
    }
}

pub fn status_text(p: &Project) -> String {
    if let Some(e) = &p.error {
        return format!("Error: {}", e);
    }
    let latest = match &p.latest {
        Some(l) => l,
        None => return "No releases published".into(),
    };
    if has_update(p) {
        let new = if !latest.tag.is_empty() { &latest.tag } else if !latest.name.is_empty() { &latest.name } else { "new release" };
        match &p.seen_tag {
            Some(old) if !old.is_empty() => format!("Update available: {} -> {}", old, new),
            _ => format!("Update available: {}", new),
        }
    } else {
        "Up to date".into()
    }
}

/// 0 update, 1 error, 2 no releases, 3 up to date.
pub fn status_rank(p: &Project) -> i32 {
    if p.error.is_some() {
        1
    } else if p.latest.is_none() {
        2
    } else if has_update(p) {
        0
    } else {
        3
    }
}

/// "error", "update", "none" (no releases) or "ok": the same split that colours the rows.
pub fn status_kind(p: &Project) -> &'static str {
    if p.error.is_some() {
        "error"
    } else if has_update(p) {
        "update"
    } else if p.latest.is_none() {
        "none"
    } else {
        "ok"
    }
}

/// Choices of the status filter: (value, menu text). "all" shows everything.
pub const STATUS_FILTERS: [(&str, &str); 5] = [
    ("all", "All projects"),
    ("update", "Updates available"),
    ("error", "Errors"),
    ("ok", "Up to date"),
    ("none", "No releases"),
];

/// Menu text of a status filter value ("All projects" for anything unknown).
pub fn status_filter_text(kind: &str) -> &'static str {
    STATUS_FILTERS.iter().find(|f| f.0 == kind).map_or(STATUS_FILTERS[0].1, |f| f.1)
}

/// Which projects (by index, in their current order) match the search text and the status
/// filter. Every word of the search text must appear (any case) in the project name or the
/// repository address, so "emu rom" finds "Emu Roms" and "owner/emu-roms". Only decides what is
/// shown; nothing is changed.
pub fn filter_matches(projects: &[Project], query: &str, kind: &str) -> Vec<bool> {
    let terms: Vec<String> = query.split_whitespace().map(|t| t.to_lowercase()).collect();
    projects
        .iter()
        .map(|p| {
            if kind != "all" && status_kind(p) != kind {
                return false;
            }
            if terms.is_empty() {
                return true;
            }
            let hay = format!("{} {} {}", display_name(p), repo_label(p), source_label(p)).to_lowercase();
            terms.iter().all(|t| hay.contains(t.as_str()))
        })
        .collect()
}

/// Text of the "Current release" cell.
pub fn release_label(p: &Project) -> String {
    let latest = match &p.latest {
        Some(l) => l,
        None => return "-".into(),
    };
    let mut s = if !latest.tag.is_empty() { latest.tag.clone() } else { latest.name.clone() };
    if latest.source == "tag" {
        s.push_str(" (tag)");
    } else if latest.prerelease {
        s.push_str(" (pre-release)");
    }
    if s.is_empty() { "-".into() } else { s }
}

pub fn published_label(p: &Project) -> String {
    let d: String = p.latest.as_ref().map(|l| l.published_at.chars().take(10).collect()).unwrap_or_default();
    if d.is_empty() { "-".into() } else { d }
}

pub fn checked_label(p: &Project) -> String {
    let s: String = p.last_checked.chars().take(16).collect::<String>().replace('T', " ");
    if s.is_empty() { "never".into() } else { s }
}

pub fn apply_result(p: &mut Project, latest: Option<Release>, err: Option<String>, now: &str) {
    p.last_checked = now.to_string();
    if let Some(e) = err {
        p.error = Some(e);
        return;
    }
    p.error = None;
    if let Some(l) = &latest {
        if p.seen_tag.as_deref().map_or(false, |t| !t.is_empty() && t == l.tag) {
            // Same release as already seen; only the ID scheme differs (API id vs feed id).
            p.seen_id = Some(l.id.clone());
        } else if p.fmt != Some(2) && p.seen_id.is_none() {
            p.seen_id = Some(l.id.clone());
            p.seen_tag = Some(l.tag.clone());
        }
    }
    p.latest = latest;
    p.fmt = Some(2);
}

// ---------------------------------------------------------------------------
// Normalizing untrusted input
// ---------------------------------------------------------------------------
fn normalize_release(v: Option<&Value>, base: &str, fallback: &str) -> Option<Release> {
    let r = v?.as_object()?;
    let tag = str_field(r.get("tag"), 200);
    let id = match id_value(r.get("id")) {
        Some(i) => i,
        None if !tag.is_empty() => Id::Str(tag.clone()),
        None => return None,
    };
    let source = match r.get("source").and_then(|s| s.as_str()) {
        Some(s @ ("release" | "tag" | "feed")) => s.to_string(),
        _ => "release".to_string(),
    };
    Some(Release {
        id,
        tag,
        name: str_field(r.get("name"), 300),
        published_at: str_field(r.get("published_at"), 40),
        url: safe_url(r.get("url").and_then(|u| u.as_str()), base, fallback),
        prerelease: r.get("prerelease").map_or(false, truthy),
        source,
    })
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().map_or(true, |f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// A clean project from untrusted input (an object, or an `owner/repo` string / GitHub URL).
pub fn normalize_project(raw: &Value) -> Option<Project> {
    let obj;
    let owned;
    let o = match raw {
        Value::String(s) => {
            let r = parse_project_ref(s)?;
            if !r.host.is_empty() && r.kind.is_none() {
                return None; // an unknown server cannot be identified offline
            }
            let (owner, repo) = split_path(&r.segs);
            owned = if r.host.is_empty() {
                json!({ "owner": owner, "repo": repo })
            } else {
                json!({ "owner": owner, "repo": repo, "host": r.host, "kind": r.kind })
            };
            obj = owned.as_object()?;
            obj
        }
        Value::Object(o) => o,
        _ => return None,
    };
    let owner = o.get("owner")?.as_str()?;
    let repo = o.get("repo")?.as_str()?;
    let (host, kind): (Option<String>, Option<String>) = match o.get("host") {
        None | Some(Value::Null) => (None, None),
        Some(Value::String(h)) if h.is_empty() || h == "github.com" => (None, None),
        Some(Value::String(h)) => {
            let h = h.to_lowercase();
            match o.get("kind").and_then(|k| k.as_str()) {
                Some(k @ ("gitlab" | "gitea")) if valid_host(&h) => (Some(h), Some(k.to_string())),
                _ => return None,
            }
        }
        _ => return None,
    };
    let owner_ok = if host.is_some() {
        let n = owner.split('/').count();
        (1..MAX_PATH_SEGMENTS).contains(&n) && owner.split('/').all(valid_name)
    } else {
        valid_name(owner)
    };
    if !owner_ok || !valid_name(repo) {
        return None;
    }
    let title = str_field(o.get("title"), 200);
    let custom = match o.get("custom_name") {
        Some(Value::String(s)) => clean_name(s),
        _ => String::new(),
    };
    let error = str_field(o.get("error"), 300);
    let seen_tag = str_field(o.get("seen_tag"), 200);
    let mut p = Project {
        owner: owner.to_string(),
        repo: repo.to_string(),
        host,
        kind,
        title: if title.is_empty() { repo.to_string() } else { title },
        description: str_field(o.get("description"), 500),
        html_url: String::new(),
        include_prereleases: o.get("include_prereleases").map_or(true, truthy),
        fmt: if o.get("fmt").and_then(|f| f.as_i64()) == Some(2) { Some(2) } else { None },
        latest: None,
        seen_id: id_value(o.get("seen_id")),
        seen_tag: if seen_tag.is_empty() { None } else { Some(seen_tag) },
        last_checked: str_field(o.get("last_checked"), 40),
        error: if error.is_empty() { None } else { Some(error) },
        custom_name: if custom.is_empty() { None } else { Some(custom) },
    };
    let base = project_base(&p);
    p.html_url = safe_url(o.get("html_url").and_then(|u| u.as_str()), &base, &repo_page(&p));
    p.latest = normalize_release(o.get("latest"), &base, &release_page(&p));
    Some(p)
}

/// (projects, skipped) or None if there is no project list. Accepts {"projects": [...]}, a bare list,
/// and lists of plain `owner/repo` strings or GitHub URLs.
pub fn parse_import(raw: &Value) -> Option<(Vec<Project>, usize)> {
    let items = match raw {
        Value::Object(o) => o.get("projects")?.as_array()?,
        Value::Array(a) => a,
        _ => return None,
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut skipped = items.len().saturating_sub(MAX_PROJECTS);
    for item in items.iter().take(MAX_PROJECTS) {
        match normalize_project(item) {
            Some(p) if seen.insert(project_key(&p)) => out.push(p),
            _ => skipped += 1,
        }
    }
    Some((out, skipped))
}

/// Add incoming projects that are not already listed. Returns how many were added.
pub fn merge_projects(existing: &mut Vec<Project>, incoming: Vec<Project>) -> usize {
    let mut keys: std::collections::HashSet<String> = existing.iter().map(project_key).collect();
    let mut added = 0;
    for p in incoming {
        if keys.insert(project_key(&p)) {
            existing.push(p);
            added += 1;
        }
    }
    added
}

/// Export payload. The access token is deliberately never included.
pub fn build_export(projects: &[Project], now: &str) -> Value {
    json!({ "app": APP_NAME, "format": 2, "exported_at": now, "projects": projects })
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------
pub fn parse_data(text: &str) -> Option<Data> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let v: Value = serde_json::from_str(text).ok()?;
    let obj = v.as_object()?;
    if let Some(p) = obj.get("projects") {
        if !p.is_array() {
            return None;
        }
    }
    let mut data = Data::default();
    if let Some(s) = obj.get("settings").and_then(|s| s.as_object()) {
        data.settings.token = str_field(s.get("token"), 200);
        data.settings.check_on_launch = s.get("check_on_launch").map_or(false, truthy);
        data.settings.theme = match s.get("theme").and_then(|t| t.as_str()) {
            Some(t @ ("light" | "dark")) => t.to_string(),
            _ => String::new(),
        };
        data.settings.sort_col = match s.get("sort_col").and_then(|t| t.as_str()) {
            Some(c) if SORTABLE.contains(&c) => c.to_string(),
            _ => String::new(),
        };
        data.settings.sort_asc = s.get("sort_asc") != Some(&Value::Bool(false));
    }
    data.projects = parse_import(&v).map(|(p, _)| p).unwrap_or_default();
    Some(data)
}

pub fn data_to_json(data: &Data) -> String {
    serde_json::to_string_pretty(data).unwrap_or_else(|_| "{}".into())
}

// ---------------------------------------------------------------------------
// Sorting (same rules as the Python version)
// ---------------------------------------------------------------------------
fn title_key(p: &Project) -> String {
    display_name(p).to_lowercase()
}

fn published(p: &Project) -> &str {
    p.latest.as_ref().map_or("", |l| l.published_at.as_str())
}

/// Returns the indexes of `projects` in display order. col "" = updates first, then A-Z.
pub fn sort_order(projects: &[Project], col: &str, asc: bool) -> Vec<usize> {
    let mut idx: Vec<usize> = (0..projects.len()).collect();
    let dir = |o: Ordering| if asc { o } else { o.reverse() };
    match col {
        "status" => {
            let sign = if asc { 1 } else { -1 };
            idx.sort_by(|&a, &b| {
                let (pa, pb) = (&projects[a], &projects[b]);
                (sign * status_rank(pa), title_key(pa), project_key(pa)).cmp(&(sign * status_rank(pb), title_key(pb), project_key(pb)))
            });
        }
        "name" => idx.sort_by(|&a, &b| {
            let (pa, pb) = (&projects[a], &projects[b]);
            dir((title_key(pa), project_key(pa)).cmp(&(title_key(pb), project_key(pb))))
        }),
        // what the column shows (owner/repo); the full key only breaks ties
        "repo" => idx.sort_by(|&a, &b| {
            let (pa, pb) = (&projects[a], &projects[b]);
            dir((repo_path(pa).to_lowercase(), project_key(pa)).cmp(&(repo_path(pb).to_lowercase(), project_key(pb))))
        }),
        // GitHub, GitLab, Codeberg, then other servers A-Z. Same source: A-Z by name in either
        // direction (the second sort is stable, so the name order is kept).
        "source" => {
            idx.sort_by(|&a, &b| {
                let (pa, pb) = (&projects[a], &projects[b]);
                (title_key(pa), project_key(pa)).cmp(&(title_key(pb), project_key(pb)))
            });
            idx.sort_by(|&a, &b| dir(source_key(&projects[a]).cmp(&source_key(&projects[b]))));
        }
        "published" => {
            let (mut dated, mut undated): (Vec<usize>, Vec<usize>) = idx.into_iter().partition(|&i| !published(&projects[i]).is_empty());
            dated.sort_by(|&a, &b| {
                let (pa, pb) = (&projects[a], &projects[b]);
                dir((published(pa), title_key(pa)).cmp(&(published(pb), title_key(pb))))
            });
            undated.sort_by_key(|&i| title_key(&projects[i]));
            dated.extend(undated);
            idx = dated;
        }
        _ => idx.sort_by(|&a, &b| {
            let (pa, pb) = (&projects[a], &projects[b]);
            (!has_update(pa), title_key(pa)).cmp(&(!has_update(pb), title_key(pb)))
        }),
    }
    idx
}

// ---------------------------------------------------------------------------
// Release lookup
// ---------------------------------------------------------------------------
fn release_from_api(r: &Value) -> Release {
    let s = |k: &str| r.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let tag = s("tag_name");
    let published = if !s("published_at").is_empty() { s("published_at") } else { s("created_at") };
    Release {
        id: id_value(r.get("id")).unwrap_or_else(|| Id::Str(tag.clone())),
        tag,
        name: s("name"),
        published_at: published,
        url: s("html_url"),
        prerelease: r.get("prerelease").map_or(false, truthy),
        source: "release".into(),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RepoInfo {
    pub owner: String,
    pub repo: String,
    pub title: String,
    pub description: String,
    pub html_url: String,
    pub host: Option<String>,
    pub kind: Option<String>,
}

pub fn fetch_repo_info(net: &dyn Net, owner: &str, repo: &str, token: &str) -> Result<RepoInfo, GitHubError> {
    let d = net.api_get(&format!("/repos/{}/{}", owner, repo), token)?;
    let s = |k: &str| d.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let full = if s("full_name").is_empty() { format!("{}/{}", owner, repo) } else { s("full_name") };
    let (ro, rr) = full.split_once('/').map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((owner.to_string(), repo.to_string()));
    Ok(RepoInfo {
        title: if s("name").is_empty() { rr.clone() } else { s("name") },
        description: s("description"),
        html_url: if s("html_url").is_empty() { format!("https://github.com/{}", full) } else { s("html_url") },
        owner: ro,
        repo: rr,
        host: None,
        kind: None,
    })
}

fn newest_release(net: &dyn Net, owner: &str, repo: &str, token: &str) -> Result<Option<Release>, GitHubError> {
    let data = match net.api_get(&format!("/repos/{}/{}/releases?per_page=10", owner, repo), token) {
        Ok(d) => d,
        Err(GitHubError::NotFound(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    for r in data.as_array().map(|a| a.as_slice()).unwrap_or(&[]) {
        if !r.get("draft").map_or(false, truthy) {
            return Ok(Some(release_from_api(r)));
        }
    }
    Ok(None)
}

fn newest_tag(net: &dyn Net, owner: &str, repo: &str, token: &str) -> Result<Option<Release>, GitHubError> {
    let data = match net.api_get(&format!("/repos/{}/{}/tags?per_page=1", owner, repo), token) {
        Ok(d) => d,
        Err(GitHubError::NotFound(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    let t = match data.as_array().and_then(|a| a.first()) {
        Some(t) => t,
        None => return Ok(None),
    };
    let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let sha = t.get("commit").and_then(|c| c.get("sha")).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).unwrap_or(&name).to_string();
    Ok(Some(Release {
        id: Id::Str(sha),
        url: format!("https://github.com/{}/{}/releases/tag/{}", owner, repo, url_quote(&name)),
        tag: name,
        name: String::new(),
        published_at: String::new(),
        prerelease: false,
        source: "tag".into(),
    }))
}

/// 1. newest stable release (or newest incl. pre-releases), 2. newest pre-release when no stable one
/// exists, 3. newest git tag when there is no GitHub Release at all.
pub fn fetch_latest_release(net: &dyn Net, owner: &str, repo: &str, token: &str, include_pre: bool) -> Result<Option<Release>, GitHubError> {
    if !include_pre {
        match net.api_get(&format!("/repos/{}/{}/releases/latest", owner, repo), token) {
            Ok(v) => return Ok(Some(release_from_api(&v))),
            Err(GitHubError::NotFound(_)) => {}
            Err(e) => return Err(e),
        }
    }
    if let Some(r) = newest_release(net, owner, repo, token)? {
        return Ok(Some(r));
    }
    newest_tag(net, owner, repo, token)
}

// ----- tiny Atom reader (the feed is machine-written and regular; no XML crate needed) -----
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        if let Some(end) = rest.find(';').filter(|&e| e <= 10) {
            let ent = &rest[1..end];
            let rep = match ent {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                _ if ent.starts_with("#x") || ent.starts_with("#X") => u32::from_str_radix(&ent[2..], 16).ok().and_then(char::from_u32),
                _ if ent.starts_with('#') => ent[1..].parse::<u32>().ok().and_then(char::from_u32),
                _ => None,
            };
            if let Some(c) = rep {
                out.push(c);
                rest = &rest[end + 1..];
                continue;
            }
        }
        out.push('&');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

/// Position of `<name` followed by a real tag-name boundary.
fn find_open(hay: &str, name: &str, from: usize) -> Option<usize> {
    let pat = format!("<{}", name);
    let mut pos = from;
    while let Some(i) = hay[pos..].find(&pat) {
        let at = pos + i;
        match hay[at + pat.len()..].chars().next() {
            Some(c) if c == '>' || c == '/' || c.is_whitespace() => return Some(at),
            _ => pos = at + pat.len(),
        }
    }
    None
}

fn element_text(hay: &str, name: &str) -> Option<String> {
    let at = find_open(hay, name, 0)?;
    let gt = hay[at..].find('>')? + at;
    if hay[..gt].ends_with('/') {
        return Some(String::new());
    }
    let close = format!("</{}>", name);
    let end = hay[gt + 1..].find(&close)? + gt + 1;
    Some(decode_entities(hay[gt + 1..end].trim()))
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let mut pos = 0;
    while let Some(i) = tag[pos..].find(name) {
        let at = pos + i;
        let before_ok = at == 0 || tag[..at].ends_with(char::is_whitespace);
        let after = &tag[at + name.len()..];
        let after_trim = after.trim_start();
        if before_ok && after_trim.starts_with('=') {
            let v = after_trim[1..].trim_start();
            let q = v.chars().next()?;
            if q == '"' || q == '\'' {
                let end = v[1..].find(q)?;
                return Some(decode_entities(&v[1..1 + end]));
            }
        }
        pos = at + name.len();
    }
    None
}

/// Newest release from the parsed feed text. Ok(None) = feed is empty.
pub fn parse_feed(body: &str, owner: &str, repo: &str) -> Result<Option<Release>, GitHubError> {
    if find_open(body, "feed", 0).is_none() {
        return Err(GitHubError::Other("Release feed was not valid XML.".into()));
    }
    let start = match find_open(body, "entry", 0) {
        Some(s) => s,
        None => return Ok(None),
    };
    let end = body[start..].find("</entry>").map_or(body.len(), |e| start + e);
    let entry = &body[start..end];
    let entry_id = element_text(entry, "id").unwrap_or_default();
    let title = element_text(entry, "title").unwrap_or_default();
    let updated = element_text(entry, "updated").unwrap_or_default();
    let mut href = String::new();
    let mut pos = 0;
    while let Some(at) = find_open(entry, "link", pos) {
        let gt = entry[at..].find('>').map_or(entry.len(), |g| at + g);
        let tag = &entry[at + 5..gt];
        if attr(tag, "rel").unwrap_or_else(|| "alternate".into()) == "alternate" {
            href = attr(tag, "href").unwrap_or_default();
            break;
        }
        pos = gt;
    }
    let marker = "/releases/tag/";
    let tag = if let Some(i) = href.find(marker) {
        url_unquote(&href[i + marker.len()..])
    } else if let Some(i) = entry_id.find("Repository/") {
        let r = &entry_id[i + "Repository/".len()..];
        let digits = r.chars().take_while(|c| c.is_ascii_digit()).count();
        if digits > 0 && r[digits..].starts_with('/') && r.len() > digits + 1 { r[digits + 1..].to_string() } else { String::new() }
    } else {
        String::new()
    };
    if entry_id.is_empty() && tag.is_empty() {
        return Ok(None);
    }
    Ok(Some(Release {
        id: Id::Str(if entry_id.is_empty() { tag.clone() } else { entry_id }),
        tag,
        name: title,
        published_at: updated,
        url: if href.is_empty() { format!("https://github.com/{}/{}/releases", owner, repo) } else { href },
        prerelease: false, // not reported by the feed
        source: "feed".into(),
    }))
}

pub fn fetch_latest_from_feed(net: &dyn Net, owner: &str, repo: &str) -> Result<Option<Release>, GitHubError> {
    let body = net.feed_get(owner, repo)?;
    parse_feed(&body, owner, repo)
}

/// Feed first (free, no quota); the API as fallback. Stable-only needs the API (the feed does not
/// mark pre-releases).
pub fn get_latest(net: &dyn Net, owner: &str, repo: &str, token: &str, include_pre: bool) -> Result<Option<Release>, GitHubError> {
    if include_pre {
        if let Ok(Some(r)) = fetch_latest_from_feed(net, owner, repo) {
            return Ok(Some(r));
        }
    }
    fetch_latest_release(net, owner, repo, token, include_pre)
}

// ---------------------------------------------------------------------------
// Other hosts: GitLab, Gitea / Forgejo (Codeberg) and self-hosted servers.
// Public repositories only, no token, official releases only (not bare tags).
// A GitHub project has no "host" / "kind" keys, so GitHub-only files stay as they were.
// ---------------------------------------------------------------------------
/// host -> kind ("gitea" also covers Forgejo, which speaks the same API)
pub const KNOWN_HOSTS: [(&str, &str); 4] = [("gitlab.com", "gitlab"), ("codeberg.org", "gitea"), ("gitea.com", "gitea"), ("code.forgejo.org", "gitea")];
pub const MAX_PATH_SEGMENTS: usize = 20;

fn known_kind(netloc: &str) -> Option<&'static str> {
    KNOWN_HOSTS.iter().find(|(h, _)| *h == netloc).map(|(_, k)| *k)
}

/// `^(http://)?[a-z0-9]([a-z0-9.-]{0,251}[a-z0-9])?(:\d{1,5})?$`
pub fn valid_host(h: &str) -> bool {
    let rest = h.strip_prefix("http://").unwrap_or(h);
    let (name, port) = match rest.split_once(':') {
        Some((n, p)) => (n, Some(p)),
        None => (rest, None),
    };
    if let Some(p) = port {
        if p.is_empty() || p.len() > 5 || !p.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
    }
    let b = name.as_bytes();
    let alnum = |c: u8| c.is_ascii_lowercase() || c.is_ascii_digit();
    !b.is_empty() && b.len() <= 253 && alnum(b[0]) && alnum(b[b.len() - 1]) && b.iter().all(|&c| alnum(c) || c == b'.' || c == b'-')
}

/// Base URL of a stored host: "codeberg.org" -> https://codeberg.org; "http://x:3000" stays http.
pub fn host_base(host: &str) -> String {
    if host.starts_with("http://") { host.to_string() } else { format!("https://{}", host) }
}

pub fn host_label(host: &str) -> &str {
    host.strip_prefix("http://").unwrap_or(host)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectRef {
    /// "" = github.com; otherwise "example.org" (https) or "http://example.org:3000" (only when typed so).
    pub host: String,
    /// "gitlab" / "gitea" for known hosts, None for an unknown server (found out when adding).
    pub kind: Option<String>,
    /// [owner, repo] for GitHub / Gitea; the full path for GitLab (groups and subgroups).
    pub segs: Vec<String>,
}

fn strip_prefix_ci<'a>(s: &'a str, p: &str) -> Option<&'a str> {
    if s.len() >= p.len() && s.is_char_boundary(p.len()) && s[..p.len()].eq_ignore_ascii_case(p) { Some(&s[p.len()..]) } else { None }
}

pub fn split_path(segs: &[String]) -> (String, String) {
    match segs.split_last() {
        Some((last, head)) => (head.join("/"), last.clone()),
        None => (String::new(), String::new()),
    }
}

/// Understand a pasted project reference (GitHub, or any other host).
pub fn parse_project_ref(text: &str) -> Option<ProjectRef> {
    let text = text.trim();
    if let Some((o, r)) = parse_repo(text) {
        return Some(ProjectRef { host: String::new(), kind: None, segs: vec![o, r] });
    }
    let (https, rest) = if let Some(r) = strip_prefix_ci(text, "https://") {
        (true, Some(r))
    } else if let Some(r) = strip_prefix_ci(text, "http://") {
        (false, Some(r))
    } else {
        (true, None)
    };
    let (netloc, path): (String, String) = match rest {
        Some(r) => {
            let r = r.split_whitespace().next().unwrap_or(""); // a URL ends at the first whitespace
            let nend = r.find(['/', '?', '#']).unwrap_or(r.len());
            let path = r[nend..].split(['?', '#']).next().unwrap_or("");
            (r[..nend].to_ascii_lowercase(), path.to_string())
        }
        None => {
            // "codeberg.org/owner/repo" without the scheme
            if text.chars().any(|c| c.is_whitespace() || c == '?' || c == '#') {
                return None;
            }
            let end = text.find('/')?;
            let nl = text[..end].to_ascii_lowercase();
            let name = nl.split(':').next().unwrap_or("");
            if !name.contains('.') || name.split('.').any(|l| l.is_empty() || !l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')) {
                return None;
            }
            (nl, text[end..].to_string())
        }
    };
    if netloc.contains('@') || netloc == "github.com" || netloc == "www.github.com" {
        return None;
    }
    let host = if https { netloc.clone() } else { format!("http://{}", netloc) };
    if !valid_host(&host) {
        return None;
    }
    let kind = known_kind(&netloc);
    let mut path = path.split("/-/").next().unwrap_or("").to_string();
    if path.ends_with("/-") {
        path.truncate(path.len() - 2);
    }
    let mut segs: Vec<String> = path.split('/').filter(|x| !x.is_empty()).map(String::from).collect();
    if let Some(l) = segs.last_mut() {
        if let Some(x) = l.strip_suffix(".git") {
            *l = x.to_string();
        }
    }
    if segs.len() < 2 || segs.len() > MAX_PATH_SEGMENTS || !segs.iter().all(|x| valid_name(x)) {
        return None;
    }
    if kind == Some("gitea") {
        segs.truncate(2);
    }
    Some(ProjectRef { host, kind: kind.map(String::from), segs })
}

fn api_url(base: &str, kind: &str, path: &[String], tail: &str) -> String {
    if kind == "gitea" {
        format!("{}/api/v1/repos/{}{}", base, path.iter().map(|x| url_quote(x)).collect::<Vec<_>>().join("/"), tail)
    } else {
        format!("{}/api/v4/projects/{}{}", base, url_quote(&path.join("/")), tail)
    }
}

fn host_info(net: &dyn Net, base: &str, host: &str, kind: &str, path: &[String]) -> Result<RepoInfo, GitHubError> {
    let d = net.http_json(&api_url(base, kind, path, ""))?;
    if !d.is_object() {
        return Err(GitHubError::Other("Unexpected answer from the server.".into()));
    }
    let key = if kind == "gitea" { "full_name" } else { "path_with_namespace" };
    let full = d.get(key).and_then(|v| v.as_str()).filter(|s| !s.is_empty()).ok_or_else(|| GitHubError::NotFound("Not found on this server.".into()))?;
    let parts: Vec<String> = full.split('/').map(String::from).collect();
    if !(2..=MAX_PATH_SEGMENTS).contains(&parts.len()) || !parts.iter().all(|x| valid_name(x)) {
        return Err(GitHubError::Other("Unexpected project name from the server.".into()));
    }
    let (owner, repo) = split_path(&parts);
    let page = format!("{}/{}", base, full);
    let url_key = if kind == "gitea" { "html_url" } else { "web_url" };
    let title = str_field(d.get("name"), 200);
    Ok(RepoInfo {
        title: if title.is_empty() { repo.clone() } else { title },
        description: str_field(d.get("description"), 500),
        html_url: safe_url(d.get(url_key).and_then(|v| v.as_str()), base, &page),
        owner,
        repo,
        host: Some(host.to_string()),
        kind: Some(kind.to_string()),
    })
}

/// One release object from the server -> this app's release (None if unusable).
fn host_release(base: &str, kind: &str, owner: &str, repo: &str, r: &Value) -> Option<Release> {
    if !r.is_object() || r.get("draft").map_or(false, truthy) {
        return None;
    }
    let tag = str_field(r.get("tag_name"), 200);
    if tag.is_empty() {
        return None;
    }
    let full = format!("{}/{}", owner, repo);
    let pick = |a: &str, b: &str| {
        let x = str_field(r.get(a), 40);
        if x.is_empty() { str_field(r.get(b), 40) } else { x }
    };
    let (id, when, url, fallback, pre) = if kind == "gitea" {
        let id = match r.get("id") {
            Some(Value::Number(n)) if n.is_i64() => Id::Int(n.as_i64().unwrap_or(0)),
            _ => Id::Str(tag.clone()),
        };
        (id, pick("published_at", "created_at"), r.get("html_url"), format!("{}/{}/releases/tag/{}", base, full, url_quote(&tag)), r.get("prerelease").map_or(false, truthy))
    } else {
        if r.get("upcoming_release").map_or(false, truthy) {
            return None; // dated in the future: not released yet
        }
        let url = r.get("_links").and_then(|l| l.get("self"));
        (Id::Str(tag.clone()), pick("released_at", "created_at"), url, format!("{}/{}/-/releases/{}", base, full, url_quote(&tag)), false)
    };
    Some(Release {
        id,
        name: str_field(r.get("name"), 300),
        published_at: when,
        url: safe_url(url.and_then(|v| v.as_str()), base, &fallback),
        prerelease: pre,
        source: "release".into(),
        tag,
    })
}

/// Newest official release of a project on another host, or None if it has none.
pub fn fetch_latest_host(net: &dyn Net, host: &str, kind: &str, owner: &str, repo: &str, include_pre: bool) -> Result<Option<Release>, GitHubError> {
    let base = host_base(host);
    let path: Vec<String> = format!("{}/{}", owner, repo).split('/').map(String::from).collect();
    let tail = if kind == "gitea" { "/releases?limit=10" } else { "/releases?per_page=10" };
    let data = net.http_json(&api_url(&base, kind, &path, tail))?;
    let arr = data.as_array().ok_or_else(|| GitHubError::Other("Unexpected answer from the server.".into()))?;
    let rels: Vec<Release> = arr.iter().filter_map(|r| host_release(&base, kind, owner, repo, r)).collect();
    if !include_pre {
        if let Some(r) = rels.iter().find(|r| !r.prerelease) {
            return Ok(Some(r.clone()));
        }
    }
    Ok(rels.into_iter().next())
}

/// Add-time lookup on another host: find out what the server is, check that the project exists and has
/// an official release.
pub fn resolve_project(net: &dyn Net, host: &str, kind: Option<&str>, segs: &[String], include_pre: bool) -> Result<(RepoInfo, Release), GitHubError> {
    let base = host_base(host);
    let kinds: Vec<&str> = match kind {
        Some(k) => vec![k],
        None => vec!["gitea", "gitlab"],
    };
    let mut last: Option<GitHubError> = None;
    for k in kinds {
        let path: &[String] = if k == "gitea" { &segs[..segs.len().min(2)] } else { segs };
        let attempt = host_info(net, &base, host, k, path).and_then(|info| {
            let latest = fetch_latest_host(net, host, k, &info.owner, &info.repo, include_pre)?;
            Ok((info, latest))
        });
        match attempt {
            Err(e @ GitHubError::Network(_)) => return Err(e), // unreachable: another kind will not help
            Err(e) => {
                if last.is_none() {
                    last = Some(e);
                }
            }
            Ok((info, None)) => {
                return Err(GitHubError::NoReleases(format!("{} has no official releases (only tags, or nothing yet), so it cannot be tracked.", info.title)));
            }
            Ok((info, Some(l))) => return Ok((info, l)),
        }
    }
    match last {
        Some(e) if kind.is_some() || !matches!(e, GitHubError::NotFound(_)) => Err(e),
        _ => Err(GitHubError::NotFound(format!("Could not find this project on {}. It must be a public repository on a GitLab, Gitea or Forgejo server.", host_label(host)))),
    }
}

/// Latest release for a GitHub project (host "") or one on another host.
pub fn fetch_latest_for(net: &dyn Net, host: &str, kind: Option<&str>, owner: &str, repo: &str, token: &str, include_pre: bool) -> Result<Option<Release>, GitHubError> {
    if host.is_empty() {
        return get_latest(net, owner, repo, token, include_pre);
    }
    fetch_latest_host(net, host, kind.unwrap_or("gitea"), owner, repo, include_pre)
}

// ---------------------------------------------------------------------------
// Column layout (pixels at 96 DPI; the GUI scales them)
// ---------------------------------------------------------------------------
pub const COL_BASE: [i32; 7] = [230, 250, 125, 150, 125, 260, 140];
pub const COL_MIN: [i32; 7] = [120, 120, 95, 90, 115, 120, 110];

/// Column widths for `avail` pixels, proportional to the preferred widths. Wider than the preferred
/// total: all grow by the same ratio; narrower: each shrinks toward its minimum at the same rate.
/// The rounding remainder goes to the Status column (index 5).
pub fn layout_columns(base: &[i32], low: &[i32], avail: i32) -> Vec<i32> {
    let total_pref: i32 = base.iter().sum();
    let total_low: i32 = low.iter().sum();
    let target: Vec<f64> = if avail >= total_pref {
        base.iter().map(|&p| p as f64 * avail as f64 / total_pref as f64).collect()
    } else {
        let t = ((avail - total_low) as f64 / (total_pref - total_low).max(1) as f64).max(0.0);
        base.iter().zip(low).map(|(&p, &lo)| lo as f64 + (p - lo) as f64 * t).collect()
    };
    let mut sizes: Vec<i32> = target.iter().zip(low).map(|(&x, &lo)| (x as i32).max(lo)).collect();
    if avail >= total_low && sizes.len() > 5 {
        let sum: i32 = sizes.iter().sum();
        sizes[5] += (avail - sum).max(0);
    }
    sizes
}

/// Apply a name typed in the rename box. Blank, or the GitHub name itself, resets to the original.
/// Returns a status message if the displayed name changed.
pub fn apply_rename(p: &mut Project, typed: &str) -> Option<String> {
    let text = clean_name(typed);
    let original = if p.title.is_empty() { p.repo.clone() } else { p.title.clone() };
    let before = display_name(p);
    p.custom_name = if !text.is_empty() && text != original { Some(text) } else { None };
    let after = display_name(p);
    if after == before {
        None
    } else if p.custom_name.is_some() {
        Some(format!("Renamed to {}.", after))
    } else {
        Some(format!("Name reset to {}.", after))
    }
}

#[cfg(test)]
mod tests;
