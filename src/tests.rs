use super::*;
use std::cell::{Cell, RefCell};

#[test]
fn parse_repo_cases() {
    let ok = |s: &str| parse_repo(s).map(|(a, b)| format!("{}/{}", a, b));
    assert_eq!(ok("https://github.com/owner/repo"), Some("owner/repo".into()));
    assert_eq!(ok("http://www.github.com/owner/repo/releases/tag/v1"), Some("owner/repo".into()));
    assert_eq!(ok("github.com/o/r.git"), Some("o/r".into()));
    assert_eq!(ok("  owner/repo  "), Some("owner/repo".into()));
    assert_eq!(ok("owner/repo.git"), Some("owner/repo".into()));
    assert_eq!(ok("owner"), None);
    assert_eq!(ok("owner/repo/extra"), None);
    assert_eq!(ok("https://example.com/a/b"), None);
    assert_eq!(ok(""), None);
    assert_eq!(ok("/repo"), None);
}

#[test]
fn url_quote_roundtrip() {
    assert_eq!(url_quote("v1.0/β"), "v1.0%2F%CE%B2");
    assert_eq!(url_unquote("v1.0%2F%CE%B2"), "v1.0/β");
    assert_eq!(url_unquote("100%"), "100%");
    assert_eq!(url_unquote("a%2"), "a%2");
}

const FEED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<feed xmlns="http://www.w3.org/2005/Atom" xml:lang="en-US">
  <id>tag:github.com,2008:https://github.com/o/r/releases</id>
  <title>Release notes from r</title>
  <entry>
    <id>tag:github.com,2008:Repository/123/v1.2.3%2Fx</id>
    <updated>2026-11-17T10:20:30Z</updated>
    <link rel="alternate" type="text/html" href="https://github.com/o/r/releases/tag/v1.2.3%2Fx"/>
    <title>Release &amp; more</title>
    <author><name>someone</name></author>
  </entry>
  <entry><id>tag:github.com,2008:Repository/123/old</id><title>old</title></entry>
</feed>"#;

#[test]
fn feed_parse() {
    let r = parse_feed(FEED, "o", "r").unwrap().unwrap();
    assert_eq!(r.tag, "v1.2.3/x");
    assert_eq!(r.name, "Release & more");
    assert_eq!(r.published_at, "2026-11-17T10:20:30Z");
    assert_eq!(r.url, "https://github.com/o/r/releases/tag/v1.2.3%2Fx");
    assert_eq!(r.id, Id::Str("tag:github.com,2008:Repository/123/v1.2.3%2Fx".into()));
    assert_eq!(r.source, "feed");
    assert!(!r.prerelease);
}

#[test]
fn feed_edge_cases() {
    assert_eq!(parse_feed("<html>nope</html>", "o", "r"), Err(GitHubError::Other("Release feed was not valid XML.".into())));
    assert_eq!(parse_feed("<feed></feed>", "o", "r"), Ok(None));
    // no link: tag comes from the id, url falls back to the releases page
    let r = parse_feed("<feed><entry><id>tag:github.com,2008:Repository/9/v2</id><title>t</title></entry></feed>", "o", "r").unwrap().unwrap();
    assert_eq!(r.tag, "v2");
    assert_eq!(r.url, "https://github.com/o/r/releases");
    // a <feedback> or <entryx> element must not be mistaken for feed/entry
    assert_eq!(parse_feed("<feedback/>", "o", "r").is_err(), true);
    // single-quoted attributes and a non-alternate link first
    let x = "<feed><entry><id>i</id><link rel='self' href='https://github.com/self'/><link href='https://github.com/o/r/releases/tag/t'/></entry></feed>";
    let r = parse_feed(x, "o", "r").unwrap().unwrap();
    assert_eq!(r.url, "https://github.com/o/r/releases/tag/t");
    assert_eq!(r.tag, "t");
}

struct MockNet {
    feed: Result<String, GitHubError>,
    api: RefCell<Vec<(String, Result<Value, GitHubError>)>>,
    calls: RefCell<Vec<String>>,
    rate: Cell<Option<i64>>,
    urls: RefCell<Vec<(String, Result<Value, GitHubError>)>>,
}

impl MockNet {
    fn new(feed: Result<String, GitHubError>) -> Self {
        MockNet { feed, api: RefCell::new(vec![]), calls: RefCell::new(vec![]), rate: Cell::new(None), urls: RefCell::new(vec![]) }
    }
    fn on(self, prefix: &str, r: Result<Value, GitHubError>) -> Self {
        self.api.borrow_mut().push((prefix.to_string(), r));
        self
    }
    /// Answer for a full URL (other hosts), matched by prefix.
    fn url(self, prefix: &str, r: Result<Value, GitHubError>) -> Self {
        self.urls.borrow_mut().push((prefix.to_string(), r));
        self
    }
}

impl Net for MockNet {
    fn api_get(&self, path: &str, _t: &str) -> Result<Value, GitHubError> {
        self.calls.borrow_mut().push(path.to_string());
        for (p, r) in self.api.borrow().iter() {
            if path.starts_with(p.as_str()) {
                return r.clone();
            }
        }
        Err(GitHubError::not_found())
    }
    fn feed_get(&self, _o: &str, _r: &str) -> Result<String, GitHubError> {
        self.calls.borrow_mut().push("feed".into());
        self.feed.clone()
    }
    fn http_json(&self, url: &str) -> Result<Value, GitHubError> {
        self.calls.borrow_mut().push(url.to_string());
        for (p, r) in self.urls.borrow().iter() {
            if url.to_lowercase().starts_with(&p.to_lowercase()) { // real servers match names case-insensitively
                return r.clone();
            }
        }
        Err(GitHubError::not_found())
    }
    fn rate_remaining(&self) -> Option<i64> {
        self.rate.get()
    }
}

#[test]
fn get_latest_prefers_feed() {
    let net = MockNet::new(Ok(FEED.into()));
    let r = get_latest(&net, "o", "r", "", true).unwrap().unwrap();
    assert_eq!(r.source, "feed");
    assert_eq!(net.calls.borrow().as_slice(), ["feed"]);
}

#[test]
fn get_latest_falls_back_to_api_then_tags() {
    // feed 404 -> releases list (draft skipped)
    let rel = json!([{"id": 1, "draft": true, "tag_name": "d"}, {"id": 7, "tag_name": "v9", "name": "Nine", "published_at": "2026-01-02T00:00:00Z", "html_url": "https://github.com/o/r/releases/tag/v9", "prerelease": true}]);
    let net = MockNet::new(Err(GitHubError::not_found())).on("/repos/o/r/releases?", Ok(rel));
    let r = get_latest(&net, "o", "r", "", true).unwrap().unwrap();
    assert_eq!((r.id.clone(), r.tag.as_str(), r.prerelease, r.source.as_str()), (Id::Int(7), "v9", true, "release"));
    // nothing but tags
    let net = MockNet::new(Ok("<feed></feed>".into())).on("/repos/o/r/tags", Ok(json!([{"name": "v0/1", "commit": {"sha": "abc"}}])));
    let r = get_latest(&net, "o", "r", "", true).unwrap().unwrap();
    assert_eq!((r.source.as_str(), r.id.clone(), r.url.as_str()), ("tag", Id::Str("abc".into()), "https://github.com/o/r/releases/tag/v0%2F1"));
    // nothing at all
    let net = MockNet::new(Ok("<feed></feed>".into()));
    assert_eq!(get_latest(&net, "o", "r", "", true), Ok(None));
}

#[test]
fn stable_only_uses_api_latest_then_falls_through() {
    let net = MockNet::new(Ok(FEED.into())).on("/repos/o/r/releases/latest", Ok(json!({"id": 5, "tag_name": "v5", "html_url": "https://github.com/o/r/releases/tag/v5"})));
    let r = get_latest(&net, "o", "r", "", false).unwrap().unwrap();
    assert_eq!(r.tag, "v5");
    assert!(!net.calls.borrow().contains(&"feed".to_string()));
    // rate limit is passed on, not swallowed
    let net = MockNet::new(Ok(FEED.into())).on("/repos/o/r/releases/latest", Err(GitHubError::RateLimit("limit".into())));
    assert_eq!(get_latest(&net, "o", "r", "", false), Err(GitHubError::RateLimit("limit".into())));
}

fn proj(owner: &str, repo: &str) -> Project {
    normalize_project(&json!({"owner": owner, "repo": repo})).unwrap()
}

fn rel(id: i64, tag: &str, date: &str) -> Release {
    Release { id: Id::Int(id), tag: tag.into(), name: String::new(), published_at: date.into(), url: String::new(), prerelease: false, source: "release".into() }
}

#[test]
fn update_detection_and_apply_result() {
    let mut p = proj("o", "r");
    p.fmt = Some(2);
    p.latest = Some(rel(1, "v1", "2026-01-01"));
    p.seen_id = Some(Id::Int(1));
    p.seen_tag = Some("v1".into());
    assert!(!has_update(&p));
    assert_eq!(status_text(&p), "Up to date");
    apply_result(&mut p, Some(rel(2, "v2", "2026-02-01")), None, "2026-11-19T10:00:00");
    assert!(has_update(&p));
    assert_eq!(status_text(&p), "Update available: v1 -> v2");
    // same tag, different id scheme (API id vs feed id): acknowledgement is carried over, no false update
    p.seen_id = Some(Id::Int(2));
    p.seen_tag = Some("v2".into());
    apply_result(&mut p, Some(Release { id: Id::Str("feed:2".into()), ..rel(0, "v2", "x") }), None, "n");
    assert!(!has_update(&p));
    // an error keeps the old data
    apply_result(&mut p, None, Some("Network error".into()), "n2");
    assert_eq!(status_text(&p), "Error: Network error");
    assert!(p.latest.is_some());
    // old-format entry that found nothing before: baseline instead of a false update
    let mut q = proj("o", "q");
    apply_result(&mut q, Some(rel(9, "v9", "")), None, "n");
    assert!(!has_update(&q));
    assert_eq!(q.fmt, Some(2));
    // no releases
    let z = proj("o", "z");
    assert_eq!(status_text(&z), "No releases published");
}

#[test]
fn normalize_rejects_and_cleans_input() {
    assert!(normalize_project(&json!({"owner": "a b", "repo": "r"})).is_none());
    assert!(normalize_project(&json!({"owner": "a", "repo": "../x"})).is_none());
    assert!(normalize_project(&json!(42)).is_none());
    let p = normalize_project(&json!({
        "owner": "o", "repo": "r", "html_url": "javascript:alert(1)", "custom_name": "  My   name \n here ",
        "latest": {"id": true, "tag": "v1", "url": "file:///etc/passwd", "source": "weird"}, "seen_id": 3.5, "fmt": 7
    })).unwrap();
    assert_eq!(p.html_url, "https://github.com/o/r");
    assert_eq!(p.custom_name.as_deref(), Some("My name here"));
    assert_eq!(display_name(&p), "My name here");
    let l = p.latest.unwrap();
    assert_eq!((l.id, l.url.as_str(), l.source.as_str()), (Id::Str("v1".into()), "https://github.com/o/r/releases", "release"));
    assert_eq!((p.seen_id, p.fmt), (None, None));
    // plain strings
    assert_eq!(normalize_project(&json!("https://github.com/a/b")).unwrap().repo, "b");
    // long custom name is cut
    let long = "x".repeat(200);
    assert_eq!(clean_name(&long).chars().count(), MAX_CUSTOM_NAME);
}

#[test]
fn import_merge_export() {
    let raw = json!({"projects": [{"owner": "A", "repo": "one"}, "a/ONE", {"owner": "bad name", "repo": "x"}, "b/two", 5]});
    let (list, skipped) = parse_import(&raw).unwrap();
    assert_eq!((list.len(), skipped), (2, 3));
    assert!(parse_import(&json!({"projects": "x"})).is_none());
    assert!(parse_import(&json!("x")).is_none());
    assert_eq!(parse_import(&json!(["x/y", "z/w"])).unwrap().0.len(), 2);
    // the cap
    let many: Vec<Value> = (0..MAX_PROJECTS + 100).map(|i| json!(format!("o/r{}", i))).collect();
    let (l, s) = parse_import(&Value::Array(many)).unwrap();
    assert_eq!((l.len(), s), (MAX_PROJECTS, 100));
    // merge never overwrites an existing repo (so custom names of new files are not applied to existing entries)
    let mut mine = vec![proj("a", "one")];
    let mut theirs = proj("A", "One");
    theirs.custom_name = Some("Renamed".into());
    assert_eq!(merge_projects(&mut mine, vec![theirs, proj("c", "three")]), 1);
    assert_eq!(mine.len(), 2);
    assert_eq!(mine[0].custom_name, None);
    // export has no token
    let e = build_export(&mine, "2026-11-19T10:00:00").to_string();
    assert!(!e.contains("token") && e.contains("\"format\":2"));
}

#[test]
fn json_file_is_compatible_with_the_python_version() {
    let mut p = proj("o", "r");
    p.latest = Some(Release { url: "https://github.com/o/r/releases/tag/v1".into(), ..rel(5, "v1", "2026-01-01T00:00:00Z") });
    p.seen_id = Some(Id::Int(5));
    let d = Data { settings: Settings { theme: "dark".into(), ..Settings::default() }, projects: vec![p.clone()] };
    let text = data_to_json(&d);
    // key order and shape as written by the Python version
    let keys: Vec<&str> = ["\"owner\"", "\"repo\"", "\"title\"", "\"description\"", "\"html_url\"", "\"include_prereleases\"", "\"fmt\"", "\"latest\"", "\"seen_id\"", "\"seen_tag\"", "\"last_checked\"", "\"error\""].to_vec();
    let pos: Vec<usize> = keys.iter().map(|k| text.find(k).unwrap()).collect();
    assert!(pos.windows(2).all(|w| w[0] < w[1]), "{}", text);
    assert!(!text.contains("custom_name"));
    assert!(text.contains("\"fmt\": null") && text.contains("\"id\": 5") && text.contains("\"sort_asc\": true"));
    let back = parse_data(&text).unwrap();
    assert_eq!(back.projects, d.projects);
    assert_eq!(back.settings.theme, "dark");
    // a file as written by the Python app (BOM, ids as ints and strings, fmt 2)
    let py = "\u{feff}{\"settings\": {\"token\": \"t\", \"check_on_launch\": true, \"theme\": \"light\", \"sort_col\": \"published\", \"sort_asc\": false}, \"projects\": [{\"owner\": \"x\", \"repo\": \"y\", \"title\": \"Y\", \"description\": \"d\", \"html_url\": \"https://github.com/x/y\", \"include_prereleases\": true, \"fmt\": 2, \"latest\": {\"id\": \"tag:github.com,2008:Repository/1/v1\", \"tag\": \"v1\", \"name\": \"n\", \"published_at\": \"2026-11-17T00:00:00Z\", \"url\": \"https://github.com/x/y/releases/tag/v1\", \"prerelease\": false, \"source\": \"feed\"}, \"seen_id\": \"tag:github.com,2008:Repository/1/v1\", \"seen_tag\": \"v1\", \"last_checked\": \"2026-11-18T14:45:00\", \"error\": null}]}";
    let d = parse_data(py).unwrap();
    assert_eq!((d.settings.token.as_str(), d.settings.check_on_launch, d.settings.sort_col.as_str(), d.settings.sort_asc), ("t", true, "published", false));
    assert_eq!(d.projects.len(), 1);
    assert!(!has_update(&d.projects[0]));
    // broken files are rejected so the caller can keep a .bak
    assert!(parse_data("{not json").is_none());
    assert!(parse_data("[1,2]").is_none());
    assert!(parse_data("{\"projects\": 5}").is_none());
}

#[test]
fn sorting_rules() {
    let mut a = proj("o", "alpha");
    let mut b = proj("o", "Bravo");
    let mut c = proj("o", "charlie");
    let d = proj("o", "delta"); // no releases
    a.latest = Some(rel(1, "v1", "2026-03-01"));
    a.seen_id = Some(Id::Int(1));
    b.latest = Some(rel(2, "v2", "2026-05-01")); // update (seen_id None)
    c.latest = Some(rel(3, "v3", "2026-04-01"));
    c.seen_id = Some(Id::Int(3));
    c.error = Some("boom".into());
    let v = vec![a, b, c, d];
    let names = |o: Vec<usize>| o.into_iter().map(|i| v[i].repo.clone()).collect::<Vec<_>>();
    assert_eq!(names(sort_order(&v, "name", true)), ["alpha", "Bravo", "charlie", "delta"]);
    assert_eq!(names(sort_order(&v, "name", false)), ["delta", "charlie", "Bravo", "alpha"]);
    assert_eq!(names(sort_order(&v, "published", false)), ["Bravo", "charlie", "alpha", "delta"]); // undated last
    assert_eq!(names(sort_order(&v, "published", true)), ["alpha", "charlie", "Bravo", "delta"]);
    assert_eq!(names(sort_order(&v, "status", true)), ["Bravo", "charlie", "delta", "alpha"]); // update, error, none, up to date
    assert_eq!(names(sort_order(&v, "status", false)), ["alpha", "delta", "charlie", "Bravo"]);
    assert_eq!(names(sort_order(&v, "", true))[0], "Bravo"); // default: updates first
}

#[test]
fn labels() {
    let mut p = proj("o", "r");
    assert_eq!((release_label(&p).as_str(), published_label(&p).as_str(), checked_label(&p).as_str()), ("-", "-", "never"));
    p.last_checked = "2026-11-18T14:45:09".into();
    assert_eq!(checked_label(&p), "2026-11-18 14:45");
    let mut r = rel(1, "v1", "2026-11-17T01:02:03Z");
    r.prerelease = true;
    p.latest = Some(r.clone());
    assert_eq!((release_label(&p).as_str(), published_label(&p).as_str()), ("v1 (pre-release)", "2026-11-17"));
    r.source = "tag".into();
    p.latest = Some(r);
    assert_eq!(release_label(&p), "v1 (tag)");
}

#[test]
fn column_layout_fills_the_width() {
    for avail in [700, 1000, 1066, 1200, 1400, 1900] {
        let w = layout_columns(&COL_BASE, &COL_MIN, avail);
        assert_eq!(w.len(), COL_BASE.len());
        assert!(w.iter().zip(COL_MIN).all(|(&a, m)| a >= m), "{:?}", w);
        if avail >= COL_MIN.iter().sum::<i32>() {
            assert_eq!(w.iter().sum::<i32>(), avail, "avail {} -> {:?}", avail, w);
        }
    }
    let w = layout_columns(&COL_BASE, &COL_MIN, COL_BASE.iter().sum()); // preferred total
    assert_eq!(w, COL_BASE.to_vec());
}

#[test]
fn rename_rules() {
    let mut p = proj("o", "repo");
    assert_eq!(apply_rename(&mut p, "  My  App "), Some("Renamed to My App.".into()));
    assert_eq!(apply_rename(&mut p, "My App"), None);
    assert_eq!(apply_rename(&mut p, "repo"), Some("Name reset to repo.".into()));
    assert_eq!(p.custom_name, None);
    assert_eq!(apply_rename(&mut p, ""), None);
}


// ----- other hosts: GitLab, Gitea / Forgejo, self-hosted -----
fn segs(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

#[test]
fn parse_project_ref_cases() {
    let pr = |t: &str| parse_project_ref(t).map(|r| (r.host, r.kind, r.segs));
    let t = |h: &str, k: Option<&str>, v: &[&str]| Some((h.to_string(), k.map(String::from), segs(v)));
    assert_eq!(pr("owner/repo"), t("", None, &["owner", "repo"]));
    assert_eq!(pr("https://github.com/o/r/releases"), t("", None, &["o", "r"]));
    assert_eq!(pr("https://codeberg.org/forgejo/forgejo/releases/tag/v1"), t("codeberg.org", Some("gitea"), &["forgejo", "forgejo"]));
    assert_eq!(pr("codeberg.org/o/r.git"), t("codeberg.org", Some("gitea"), &["o", "r"]));
    assert_eq!(pr("https://gitlab.com/gitlab-org/cli/-/releases"), t("gitlab.com", Some("gitlab"), &["gitlab-org", "cli"]));
    assert_eq!(pr("https://gitlab.com/a/b/c/-/tags"), t("gitlab.com", Some("gitlab"), &["a", "b", "c"]));
    assert_eq!(pr("https://git.example.org/o/r"), t("git.example.org", None, &["o", "r"]));
    assert_eq!(pr("http://10.0.0.5:3000/o/r"), t("http://10.0.0.5:3000", None, &["o", "r"]));
    assert_eq!(pr("HTTPS://Codeberg.ORG/O/R?tab=x#y"), t("codeberg.org", Some("gitea"), &["O", "R"]));
    assert_eq!(pr("https://user:pw@codeberg.org/o/r"), None);
    assert_eq!(pr("https://codeberg.org/onlyowner"), None);
    assert_eq!(pr("https://github.com/onlyowner"), None);
    assert_eq!(pr("https://codeberg.org/o/../x"), None);
    assert_eq!(pr("hello"), None);
    assert_eq!(pr(""), None);
    assert_eq!(pr("bad host.org/o/r"), None);
}

#[test]
fn keys_labels_and_pages() {
    let gh = proj("O", "R");
    assert_eq!((project_key(&gh).as_str(), repo_label(&gh).as_str()), ("o/r", "O/R"));
    let cb = normalize_project(&json!({"owner": "O", "repo": "R", "host": "Codeberg.org", "kind": "gitea"})).unwrap();
    assert_eq!((project_key(&cb).as_str(), repo_label(&cb).as_str()), ("codeberg.org/o/r", "codeberg.org/O/R"));
    assert_eq!(release_page(&cb), "https://codeberg.org/O/R/releases");
    let gl = normalize_project(&json!({"owner": "g/s", "repo": "p", "host": "gitlab.com", "kind": "gitlab"})).unwrap();
    assert_eq!(release_page(&gl), "https://gitlab.com/g/s/p/-/releases");
    assert_eq!(repo_page(&gl), "https://gitlab.com/g/s/p");
    let lan = normalize_project(&json!({"owner": "o", "repo": "r", "host": "http://10.0.0.5:3000", "kind": "gitea"})).unwrap();
    assert_eq!((project_key(&lan).as_str(), project_base(&lan).as_str()), ("10.0.0.5:3000/o/r", "http://10.0.0.5:3000"));
}

fn gitea_net() -> MockNet {
    MockNet::new(Err(GitHubError::not_found()))
        .url("http://h:1/api/v1/repos/own/proj/releases", Ok(json!([
            {"id": 90, "tag_name": "v3.0.0-rc1", "name": "RC", "draft": false, "prerelease": true, "published_at": "2026-11-20T10:00:00Z", "html_url": "http://h:1/own/proj/releases/tag/v3.0.0-rc1"},
            {"id": 80, "tag_name": "v2.9.0", "name": "Stable", "draft": false, "prerelease": false, "published_at": "2026-11-10T10:00:00Z", "html_url": "http://evil.example/x"},
            {"id": 99, "tag_name": "v9-draft", "draft": true, "published_at": "2026-11-21T10:00:00Z"}
        ])))
        .url("http://h:1/api/v1/repos/own/proj", Ok(json!({"full_name": "Own/Proj", "name": "Proj", "description": "d", "html_url": "http://h:1/Own/Proj"})))
        .url("http://h:1/api/v1/repos/own/tagsonly/releases", Ok(json!([])))
        .url("http://h:1/api/v1/repos/own/tagsonly", Ok(json!({"full_name": "own/tagsonly", "name": "tagsonly"})))
        .url("http://h:1/api/v4/projects/grp%2Fsub%2Fapp/releases", Ok(json!([
            {"tag_name": "v9.9.9", "released_at": "2099-01-01T00:00:00Z", "upcoming_release": true, "_links": {"self": "http://evil.example/x"}},
            {"tag_name": "v1.4.0", "name": "One four", "released_at": "2026-11-15T08:00:00Z", "upcoming_release": false, "_links": {"self": "http://h:1/grp/sub/app/-/releases/v1.4.0"}}
        ])))
        .url("http://h:1/api/v4/projects/grp%2Fsub%2Fapp", Ok(json!({"path_with_namespace": "grp/sub/app", "name": "App", "description": "gl", "web_url": "http://h:1/grp/sub/app"})))
        .url("http://h:1/api/v4/projects/grp%2Fpriv", Err(GitHubError::Other("The server asks for a sign-in (HTTP 401).".into())))
}

#[test]
fn resolve_gitea_forgejo_detected_on_unknown_host() {
    let net = gitea_net();
    let (info, l) = resolve_project(&net, "http://h:1", None, &segs(&["own", "proj"]), true).unwrap();
    assert_eq!((info.owner.as_str(), info.repo.as_str(), info.kind.as_deref(), info.host.as_deref()), ("Own", "Proj", Some("gitea"), Some("http://h:1")));
    assert_eq!((l.tag.as_str(), l.prerelease, l.id.clone()), ("v3.0.0-rc1", true, Id::Int(90)));
    let (_, stable) = resolve_project(&net, "http://h:1", None, &segs(&["own", "proj"]), false).unwrap();
    assert_eq!(stable.tag, "v2.9.0");
    assert_eq!(stable.url, "http://h:1/Own/Proj/releases/tag/v2.9.0"); // foreign link replaced by the host's own page
}

#[test]
fn resolve_gitlab_with_subgroup() {
    let net = gitea_net();
    let (info, l) = resolve_project(&net, "http://h:1", None, &segs(&["grp", "sub", "app"]), true).unwrap();
    assert_eq!((info.owner.as_str(), info.repo.as_str(), info.kind.as_deref()), ("grp/sub", "app", Some("gitlab")));
    assert_eq!((l.tag.as_str(), l.id.clone(), l.url.as_str()), ("v1.4.0", Id::Str("v1.4.0".into()), "http://h:1/grp/sub/app/-/releases/v1.4.0"));
}

#[test]
fn resolve_rejects_tags_only_missing_private_unreachable() {
    let net = gitea_net();
    assert!(matches!(resolve_project(&net, "http://h:1", Some("gitea"), &segs(&["own", "tagsonly"]), true), Err(GitHubError::NoReleases(m)) if m.contains("no official releases")));
    assert!(matches!(resolve_project(&net, "http://h:1", None, &segs(&["nope", "missing"]), true), Err(GitHubError::NotFound(m)) if m.contains("Could not find")));
    assert!(matches!(resolve_project(&net, "http://h:1", Some("gitlab"), &segs(&["grp", "priv"]), true), Err(GitHubError::Other(m)) if m.contains("sign-in")));
    let down = MockNet::new(Err(GitHubError::not_found())).url("http://h:1/", Err(GitHubError::Network("down".into())));
    assert!(matches!(resolve_project(&down, "http://h:1", None, &segs(&["a", "b"]), true), Err(GitHubError::Network(_))));
    assert_eq!(down.calls.borrow().len(), 1, "an unreachable server is probed once, not once per kind");
}

#[test]
fn check_path_uses_no_token_and_handles_lost_releases() {
    let net = gitea_net();
    let l = fetch_latest_for(&net, "http://h:1", Some("gitea"), "Own", "Proj", "SECRET", true).unwrap().unwrap();
    assert_eq!(l.tag, "v3.0.0-rc1");
    assert!(net.calls.borrow().iter().all(|c| !c.contains("SECRET")));
    assert_eq!(fetch_latest_for(&net, "http://h:1", Some("gitea"), "own", "tagsonly", "", true), Ok(None));
}

#[test]
fn other_host_projects_storage_and_compat() {
    let gh = proj("o", "r");
    let gj = serde_json::to_string(&gh).unwrap();
    assert!(!gj.contains("\"host\"") && !gj.contains("\"kind\""), "GitHub projects must serialize exactly as before");
    assert!(gj.starts_with("{\"owner\":\"o\",\"repo\":\"r\",\"title\":"), "{}", gj);
    let n = normalize_project(&json!({"owner": "grp/sub", "repo": "app", "host": "GitLab.com", "kind": "gitlab", "html_url": "https://evil.example/x",
        "latest": {"id": "v1", "tag": "v1", "url": "https://evil.example/y", "published_at": "2026-01-01"}, "fmt": 2})).unwrap();
    assert_eq!((n.host.as_deref(), n.html_url.as_str()), (Some("gitlab.com"), "https://gitlab.com/grp/sub/app"));
    assert_eq!(n.latest.as_ref().unwrap().url, "https://gitlab.com/grp/sub/app/-/releases");
    let nj = serde_json::to_string(&n).unwrap();
    assert!(nj.starts_with("{\"owner\":\"grp/sub\",\"repo\":\"app\",\"host\":\"gitlab.com\",\"kind\":\"gitlab\",\"title\":"), "{}", nj);
    assert!(normalize_project(&json!({"owner": "o", "repo": "r", "host": "github.com", "kind": "gitea"})).unwrap().host.is_none());
    for bad in [json!({"owner": "a/b", "repo": "r"}), json!({"owner": "o", "repo": "r", "host": "x.org"}), json!({"owner": "o", "repo": "r", "host": "x.org", "kind": "svn"}),
        json!({"owner": "o", "repo": "r", "host": "bad host", "kind": "gitea"}), json!({"owner": "o/../x", "repo": "r", "host": "x.org", "kind": "gitea"})] {
        assert!(normalize_project(&bad).is_none(), "{}", bad);
    }
    assert_eq!(normalize_project(&json!("https://codeberg.org/o/r")).unwrap().host.as_deref(), Some("codeberg.org"));
    assert!(normalize_project(&json!("https://git.example.org/o/r")).is_none());
    let raw = json!({"projects": [n.clone(), gh.clone(), "owner/x", "https://git.example.org/o/r", "garbage"]});
    let (imp, skipped) = parse_import(&raw).unwrap();
    assert_eq!((imp.len(), skipped), (3, 2));
    let (again, _) = parse_import(&build_export(&imp, "2026-11-22T00:00:00")).unwrap();
    assert_eq!(again.iter().map(project_key).collect::<Vec<_>>(), imp.iter().map(project_key).collect::<Vec<_>>());
}

#[cfg(test)]
mod filter_tests {
    use super::*;

    fn proj(owner: &str, repo: &str, custom: Option<&str>, release: Option<&str>, seen: Option<&str>, err: Option<&str>) -> Project {
        let mut p = super::proj(owner, repo);
        p.title = repo.to_string();
        p.custom_name = custom.map(|c| c.to_string());
        p.latest = release.map(|r| Release { id: Id::Str(r.to_string()), tag: r.to_string(), name: r.to_string(), published_at: String::new(), url: String::new(), prerelease: false, source: "release".into() });
        p.seen_id = seen.map(|s| Id::Str(s.to_string()));
        p.error = err.map(|e| e.to_string());
        p
    }

    fn sample() -> Vec<Project> {
        vec![
            proj("a", "emu-roms", None, Some("v1"), Some("v1"), None),
            proj("b", "tool", None, Some("v2"), Some("v1"), None),
            proj("c", "broken", None, None, None, Some("boom")),
            proj("d", "empty", None, None, None, None),
            proj("Zed", "Game-Launcher", Some("My Launcher"), Some("v3"), Some("v3"), None),
        ]
    }

    fn names(ps: &[Project], q: &str, k: &str) -> Vec<String> {
        let m = filter_matches(ps, q, k);
        ps.iter().zip(m).filter(|(_, ok)| *ok).map(|(p, _)| p.repo.clone()).collect()
    }

    #[test]
    fn status_kinds() {
        let ps = sample();
        let k: Vec<&str> = ps.iter().map(status_kind).collect();
        assert_eq!(k, ["ok", "update", "error", "none", "ok"]);
    }

    #[test]
    fn text_search_is_case_insensitive_and_needs_every_word() {
        let ps = sample();
        assert_eq!(names(&ps, "", "all").len(), 5);
        assert_eq!(names(&ps, "   ", "all").len(), 5);
        assert_eq!(names(&ps, "EMU", "all"), ["emu-roms"]);
        assert_eq!(names(&ps, "emu roms", "all"), ["emu-roms"]);
        assert_eq!(names(&ps, "roms emu", "all"), ["emu-roms"]);
        assert_eq!(names(&ps, "zed/game", "all"), ["Game-Launcher"]); // owner/repo address
        assert_eq!(names(&ps, "my launcher", "all"), ["Game-Launcher"]); // custom display name
        assert!(names(&ps, "nomatch", "all").is_empty());
        assert!(names(&ps, "emu nomatch", "all").is_empty());
    }

    #[test]
    fn status_filter_and_text_combine() {
        let ps = sample();
        assert_eq!(names(&ps, "", "update"), ["tool"]);
        assert_eq!(names(&ps, "", "error"), ["broken"]);
        assert_eq!(names(&ps, "", "none"), ["empty"]);
        assert_eq!(names(&ps, "", "ok"), ["emu-roms", "Game-Launcher"]);
        assert_eq!(names(&ps, "game", "ok"), ["Game-Launcher"]);
        assert!(names(&ps, "game", "error").is_empty());
        assert_eq!(status_filter_text("error"), "Errors");
        assert_eq!(status_filter_text("???"), "All projects");
    }

    #[test]
    fn limit_is_large_enough_for_hundreds_of_projects() {
        assert!(MAX_PROJECTS >= 2000);
        let items: Vec<serde_json::Value> = (0..1500).map(|i| serde_json::json!({"owner": "o", "repo": format!("r{}", i)})).collect();
        let text = serde_json::json!({"projects": items}).to_string();
        let d = parse_data(&text).unwrap();
        assert_eq!(d.projects.len(), 1500);
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;

    fn mk(owner: &str, repo: &str, host: Option<&str>) -> Project {
        let mut p = super::proj(owner, repo);
        p.title = repo.to_string();
        p.host = host.map(|h| h.to_string());
        p.kind = host.map(|_| "gitea".to_string());
        p
    }

    fn sample() -> Vec<Project> {
        vec![
            mk("o", "zeta", None),
            mk("g", "alpha", Some("gitlab.com")),
            mk("f", "forgejo", Some("codeberg.org")),
            mk("t", "tool", Some("git.example.com")),
            mk("a", "aaa", None),
            mk("l", "local", Some("http://127.0.0.1:3000")),
            mk("h", "up", Some("GitLab.com")),
        ]
    }

    fn order(ps: &[Project], col: &str, asc: bool) -> Vec<String> {
        sort_order(ps, col, asc).into_iter().map(|i| ps[i].repo.clone()).collect()
    }

    #[test]
    fn source_labels() {
        let l: Vec<String> = sample().iter().map(source_label).collect();
        assert_eq!(l, ["GitHub", "GitLab", "Codeberg", "git.example.com", "GitHub", "127.0.0.1:3000", "GitLab"]);
        assert_eq!(repo_path(&sample()[1]), "g/alpha"); // no host in the Repository column
    }

    #[test]
    fn source_sort_groups_then_names() {
        let ps = sample();
        assert!(SORTABLE.contains(&"source"));
        assert_eq!(order(&ps, "source", true), ["aaa", "zeta", "alpha", "up", "forgejo", "local", "tool"]);
        // reversed: the groups flip, the names inside a group stay A-Z
        assert_eq!(order(&ps, "source", false), ["tool", "local", "forgejo", "alpha", "up", "aaa", "zeta"]);
        // the Repository column sorts by what it shows
        let by_repo: Vec<String> = sort_order(&ps, "repo", true).into_iter().map(|i| repo_path(&ps[i])).collect();
        let mut want: Vec<String> = ps.iter().map(repo_path).collect();
        want.sort_by_key(|s| s.to_lowercase());
        assert_eq!(by_repo, want);
    }

    #[test]
    fn search_matches_the_source() {
        let ps = sample();
        let names = |q: &str| -> Vec<String> {
            let m = filter_matches(&ps, q, "all");
            ps.iter().zip(m).filter(|(_, ok)| *ok).map(|(p, _)| p.repo.clone()).collect()
        };
        assert_eq!(names("gitlab"), ["alpha", "up"]);
        assert_eq!(names("codeberg"), ["forgejo"]);
        assert_eq!(names("github"), ["zeta", "aaa"]);
    }

    #[test]
    fn unknown_saved_sort_column_falls_back() {
        let d = parse_data(r#"{"settings":{"sort_col":"bogus"},"projects":[]}"#).unwrap();
        assert_eq!(d.settings.sort_col, "");
    }
}
