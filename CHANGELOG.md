# Changelog

## [2.1.0] - 2026-11-22
- Track projects on other hosts: **GitLab** (gitlab.com or your own server, with groups and subgroups), **Codeberg**, **Gitea**, **Forgejo** and self-hosted servers of those. Paste the project URL like a GitHub one; `owner/repo` still means GitHub.
- Public repositories only, no token, and only official releases: a project with just tags (or no release yet) is refused when you add it. The optional GitHub token is sent to `github.com` only.
- Projects from other hosts show the host in the Repository column. Their release links only open on the project's own host. Older versions (Rust 2.0.0, Python 1.2.x and earlier) skip these projects when reading `projects.json` and lose them when saving, so update every copy you share the file with. GitHub-only files are unchanged.
- HTTPS and plain `http://` (only when typed so) through WinHTTP; a server certificate Windows does not trust is refused with a clear message.
- Security: names such as `..` are no longer accepted in an owner or repository name.
- The About text and the token label say that other hosts are supported and that the token is for github.com only.

## [2.0.0] - 2026-11-19
- Complete rewrite of [github-release-tracker](https://github.com/santosasarno-code/github-release-tracker) (Python / tkinter) in Rust with a native Win32 interface (no Python, no tkinter). Same features as the Python v1.1.1, same `projects.json` format.
- A single small `.exe` that needs no runtime and no installer (the C runtime is linked in).
- Windows 11 look: rounded buttons and rounded input boxes (repo, token), Segoe icons, left-aligned bold headers with sort arrows, thin row and column lines (no column lines in the empty area below the list).
- Instant, readable row selection in both light and dark mode.
- Rename: hover a project to get a pencil button, or press F2, right-click > Rename, or double-click the **Project** name. Double-clicking any other column opens the release page.
- Light / dark mode (starts with your Windows theme): icon-only button at the bottom right, the status bar tells which mode is active. Dark title bar in dark mode.
- Mark as seen uses a checkbox icon.
- Smooth window resizing: columns scale in proportion, controls move in one batch, no leftover drawing artifacts.
- HTTPS through Windows' own WinHTTP: uses the Windows certificate store and your proxy settings.
- Per-monitor DPI awareness: crisp text and icons on high-DPI screens.
- Differences from the Python version: Windows 10 or 11 only; the vertical scrollbar appears only when the list is longer than the window; the right-click menus and file dialogs use the native Windows look.
