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
}

impl MockNet {
    fn new(feed: Result<String, GitHubError>) -> Self {
        MockNet { feed, api: RefCell::new(vec![]), calls: RefCell::new(vec![]), rate: Cell::new(None) }
    }
    fn on(self, prefix: &str, r: Result<Value, GitHubError>) -> Self {
        self.api.borrow_mut().push((prefix.to_string(), r));
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
    let many: Vec<Value> = (0..600).map(|i| json!(format!("o/r{}", i))).collect();
    let (l, s) = parse_import(&Value::Array(many)).unwrap();
    assert_eq!((l.len(), s), (500, 100));
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
        assert_eq!(w.len(), 6);
        assert!(w.iter().zip(COL_MIN).all(|(&a, m)| a >= m), "{:?}", w);
        if avail >= COL_MIN.iter().sum::<i32>() {
            assert_eq!(w.iter().sum::<i32>(), avail, "avail {} -> {:?}", avail, w);
        }
    }
    let w = layout_columns(&COL_BASE, &COL_MIN, 1185); // preferred total
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
