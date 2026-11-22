//! HTTPS through WinHTTP: no extra libraries, uses the Windows certificate store and proxy settings.

use crate::winutil::*;
use github_release_tracker::{GitHubError, Net};
use serde_json::Value;
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicI64, Ordering};
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Networking::WinHttp::*;

const MAX_BODY: usize = 8 * 1024 * 1024;

pub struct WinHttpNet {
    session: *mut c_void,
    rate: AtomicI64, // -1 = unknown
}

// A WinHTTP session handle may be used from several threads.
unsafe impl Send for WinHttpNet {}
unsafe impl Sync for WinHttpNet {}

struct Handle(*mut c_void);
impl Drop for Handle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WinHttpCloseHandle(self.0) };
        }
    }
}

struct Response {
    status: u32,
    remaining: Option<String>,
    reset: Option<String>,
    body: Vec<u8>,
}

fn net_error(code: u32) -> GitHubError {
    let why = match code {
        12002 => "the request timed out",
        12007 | 12029 => "cannot reach the server (no internet connection, or it is blocked)",
        12175 | 12157 => "secure connection failed",
        12038 => "certificate error (the server's certificate is not trusted)",
        _ => "request failed",
    };
    GitHubError::Network(format!("Network error: {} (code {}).", why, code))
}

impl WinHttpNet {
    pub fn new() -> Self {
        unsafe {
            let agent = w("GitHubReleaseTracker");
            let mut s = WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, null(), null(), 0);
            if s.is_null() {
                s = WinHttpOpen(agent.as_ptr(), WINHTTP_ACCESS_TYPE_DEFAULT_PROXY, null(), null(), 0);
            }
            if !s.is_null() {
                WinHttpSetTimeouts(s, 15_000, 15_000, 15_000, 15_000);
            }
            WinHttpNet { session: s, rate: AtomicI64::new(-1) }
        }
    }

    fn query_text(req: *mut c_void, name: &str) -> Option<String> {
        unsafe {
            let n = w(name);
            let mut len: u32 = 0;
            WinHttpQueryHeaders(req, WINHTTP_QUERY_CUSTOM, n.as_ptr(), null_mut(), &mut len, null_mut());
            if len == 0 || len > 4096 {
                return None;
            }
            let mut buf = vec![0u16; (len as usize) / 2 + 2];
            let mut len2 = (buf.len() * 2) as u32;
            if WinHttpQueryHeaders(req, WINHTTP_QUERY_CUSTOM, n.as_ptr(), buf.as_mut_ptr() as *mut c_void, &mut len2, null_mut()) == 0 {
                return None;
            }
            Some(from_wide_buf(&buf))
        }
    }

    fn get(&self, host: &str, port: u16, secure: bool, path: &str, headers: &str) -> Result<Response, GitHubError> {
        if self.session.is_null() {
            return Err(GitHubError::Network("Network error: could not start WinHTTP.".into()));
        }
        unsafe {
            let conn = Handle(WinHttpConnect(self.session, w(host).as_ptr(), port, 0));
            if conn.0.is_null() {
                return Err(net_error(GetLastError()));
            }
            let req = Handle(WinHttpOpenRequest(conn.0, w("GET").as_ptr(), w(path).as_ptr(), null(), null(), null_mut(), if secure { WINHTTP_FLAG_SECURE } else { 0 }));
            if req.0.is_null() {
                return Err(net_error(GetLastError()));
            }
            let h = w(headers);
            if WinHttpSendRequest(req.0, h.as_ptr(), u32::MAX, null(), 0, 0, 0) == 0 || WinHttpReceiveResponse(req.0, null_mut()) == 0 {
                return Err(net_error(GetLastError()));
            }
            let mut status: u32 = 0;
            let mut size = 4u32;
            if WinHttpQueryHeaders(req.0, WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER, null(), &mut status as *mut u32 as *mut c_void, &mut size, null_mut()) == 0 {
                return Err(net_error(GetLastError()));
            }
            let remaining = Self::query_text(req.0, "X-RateLimit-Remaining");
            let reset = Self::query_text(req.0, "X-RateLimit-Reset");
            let mut body = Vec::new();
            let mut chunk = vec![0u8; 16 * 1024];
            loop {
                let mut read = 0u32;
                if WinHttpReadData(req.0, chunk.as_mut_ptr() as *mut c_void, chunk.len() as u32, &mut read) == 0 {
                    return Err(net_error(GetLastError()));
                }
                if read == 0 {
                    break;
                }
                body.extend_from_slice(&chunk[..read as usize]);
                if body.len() > MAX_BODY {
                    return Err(GitHubError::Other("The server's answer was too large.".into()));
                }
            }
            Ok(Response { status, remaining, reset, body })
        }
    }
}

impl Drop for WinHttpNet {
    fn drop(&mut self) {
        if !self.session.is_null() {
            unsafe { WinHttpCloseHandle(self.session) };
        }
    }
}

impl Net for WinHttpNet {
    fn api_get(&self, path: &str, token: &str) -> Result<Value, GitHubError> {
        let mut headers = String::from("Accept: application/vnd.github+json\r\nX-GitHub-Api-Version: 2022-11-28\r\nUser-Agent: GitHubReleaseTracker\r\n");
        if !token.is_empty() {
            headers.push_str(&format!("Authorization: Bearer {}\r\n", token));
        }
        let r = self.get("api.github.com", INTERNET_DEFAULT_HTTPS_PORT as u16, true, path, &headers)?;
        if let Some(n) = r.remaining.as_deref().and_then(|s| s.trim().parse::<i64>().ok()) {
            self.rate.store(n, Ordering::Relaxed);
        }
        match r.status {
            200..=299 => serde_json::from_slice(&r.body).map_err(|_| GitHubError::Other("Unexpected response from GitHub.".into())),
            403 | 429 if r.remaining.as_deref().map(str::trim) == Some("0") => {
                self.rate.store(0, Ordering::Relaxed);
                let when = r.reset.as_deref().and_then(|s| s.trim().parse::<i64>().ok()).map(local_hhmm).filter(|s| !s.is_empty());
                let when = when.map(|t| format!(" Resets at {}.", t)).unwrap_or_default();
                Err(GitHubError::RateLimit(format!("GitHub API rate limit reached.{} Add a token in Settings for a higher limit.", when)))
            }
            404 => Err(GitHubError::not_found()),
            401 => Err(GitHubError::Other("GitHub rejected the token (401). Check it in Settings.".into())),
            s => Err(GitHubError::Other(format!("GitHub returned HTTP {}.", s))),
        }
    }

    fn feed_get(&self, owner: &str, repo: &str) -> Result<String, GitHubError> {
        let path = format!("/{}/{}/releases.atom", owner, repo);
        let r = self.get("github.com", INTERNET_DEFAULT_HTTPS_PORT as u16, true, &path, "User-Agent: GitHubReleaseTracker\r\nAccept: application/atom+xml, application/xml;q=0.9, */*;q=0.8\r\n")?;
        match r.status {
            200..=299 => Ok(String::from_utf8_lossy(&r.body).into_owned()),
            404 => Err(GitHubError::not_found()),
            429 => Err(GitHubError::Other("GitHub is rate-limiting the release feed (429). Try again later.".into())),
            s => Err(GitHubError::Other(format!("Release feed returned HTTP {}.", s))),
        }
    }

    fn http_json(&self, url: &str) -> Result<Value, GitHubError> {
        let (secure, host, port, path) = split_url(url).ok_or_else(|| GitHubError::Other("Not a valid web address.".into()))?;
        let r = self.get(&host, port, secure, &path, "Accept: application/json\r\nUser-Agent: GitHubReleaseTracker\r\n")?;
        match r.status {
            200..=299 => serde_json::from_slice(&r.body)
                .map_err(|_| GitHubError::Other("Unexpected answer (not JSON). This does not look like a GitLab or Gitea/Forgejo server.".into())),
            404 => Err(GitHubError::not_found()),
            401 | 403 => Err(GitHubError::Other(format!(
                "The server asks for a sign-in (HTTP {}). Only public repositories work, and tokens are not supported for this host.",
                r.status
            ))),
            429 => Err(GitHubError::Other("The server is rate-limiting requests (429). Try again later.".into())),
            s => Err(GitHubError::Other(format!("The server returned HTTP {}.", s))),
        }
    }

    fn rate_remaining(&self) -> Option<i64> {
        let v = self.rate.load(Ordering::Relaxed);
        if v < 0 { None } else { Some(v) }
    }
}

/// "https://host:3000/path?q" -> (secure, host, port, "/path?q")
fn split_url(url: &str) -> Option<(bool, String, u16, String)> {
    let (secure, rest) = if let Some(r) = url.strip_prefix("https://") {
        (true, r)
    } else if let Some(r) = url.strip_prefix("http://") {
        (false, r)
    } else {
        return None;
    };
    let end = rest.find('/').unwrap_or(rest.len());
    let hostport = &rest[..end];
    let path = if end < rest.len() { &rest[end..] } else { "/" };
    let (host, port) = match hostport.rsplit_once(':') {
        Some((h, p)) => (h, p.parse::<u16>().ok()?),
        None => (hostport, if secure { 443 } else { 80 }),
    };
    if host.is_empty() {
        return None;
    }
    Some((secure, host.to_string(), port, path.to_string()))
}
