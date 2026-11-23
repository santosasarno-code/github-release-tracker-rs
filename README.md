# GitHub Release Tracker (Rust edition)

A small, portable Windows app that tracks the latest release of the projects you care about (GitHub, GitLab, Codeberg, Gitea / Forgejo and self-hosted servers) and tells you when a new one appears. No account, no login, and no token needed.

> **Rewrite notice:** this project is a complete rewrite, in Rust with a native Win32 interface, of my earlier Python / tkinter app **[github-release-tracker](https://github.com/santosasarno-code/github-release-tracker)**. It has the same features and uses the same `projects.json`, so you can switch between the two without losing your list. The Python version stays available.

It ships as one small `.exe` with nothing to install.

🇮🇩 [Baca dalam Bahasa Indonesia](README.id.md)

## Features

- Add a project by pasting a GitHub URL or `owner/repo`
- **Other hosts, same way.** Paste the URL of a project on **GitLab** (`gitlab.com` or your own server, including groups and subgroups), **Codeberg**, **Gitea**, **Forgejo** or any self-hosted server of those. Public repositories only, no token. Only **official releases** are tracked: a project that has just tags (or nothing yet) is refused when you add it
- See the project name, repository, current release, publish date and status at a glance
- **Check for updates** refreshes every project; a newer release is highlighted as `Update available: v1.0 -> v1.1`
- **Mark as seen** clears the highlight once you have dealt with an update
- Double-click a project row (or use **Open release page**) to open its release page in your browser; double-clicking the **Project** name renames it instead
- **Rename** a project: click the pencil that appears when you hover it, or select it and press **F2**, or right-click > Rename, or double-click the name. *Reset name* brings the GitHub name back
- Click the **Project**, **Repository**, **Source**, **Published** or **Status** header to sort (click again to reverse)
- The **Source** column tells where each project lives (GitHub, GitLab, Codeberg, or your own server's address); click its header to sort by it. The search box matches it too.
- **Filter** the list: type in the search box (Ctrl+F jumps there, Esc clears it) to show only projects whose name or repository contains every word you typed, and/or pick *Show:* All projects / Updates available / Errors / Up to date / No releases. It only changes what is listed; nothing is removed or saved differently. The box shows "n of m".
- **Import / Export** the list as `.json` (merge or replace on import; the token is never exported)
- **Light / dark** mode: click the moon / sun icon at the bottom right (starts with your Windows theme)
- Columns resize in proportion when you resize the window; drag a column border to change the proportions
- Per-monitor DPI aware; thin separator lines between rows and columns
- Portable: one `.exe`, no installer, no registry entries. Data is `projects.json` next to the exe (falls back to `%APPDATA%\GitHubReleaseTracker` if that folder is read-only)

The app only reads release information. It never downloads or installs anything.

## Keyboard and mouse

### Adding and filtering
| Action | How |
|---|---|
| Add the project in the top box | **Enter**, or the *Add project* button |
| Jump to the Filter box | **Ctrl+F** |
| Clear the filter (text and status) | **Esc** in the Filter box, or the *Clear* button |

### Selecting projects
| Action | How |
|---|---|
| Select one | Click a row |
| Add or remove one row from the selection | **Ctrl+click** |
| Select a range | **Shift+click**, or **Shift+Up / Down** |
| Jump to the first / last project | **Home** / **End** |
| Move the selection | **Up / Down** |
| Select every listed project | **Ctrl+A** |

Only the rows you can see are selected, so with a filter on, *Remove* and *Mark as seen* act on those rows only.

### Working with projects
| Action | How |
|---|---|
| Rename the selected project | **F2**, or double-click the **Project** name, or the pencil that appears when you hover a row, or right-click > Rename |
| Save / cancel a rename | **Enter** / **Esc** |
| Open the release page | Double-click any column except **Project**, or *Open release page* |
| Remove the selected projects | **Delete**, or *Remove* (always asks first) |
| Row menu | **Right-click** a row: Open release page, Open repository, Mark as seen, Rename, Reset name, Include pre-releases, Remove. Right-clicking a row that is not selected selects it first |

### The list and window
| Action | How |
|---|---|
| Sort | Click the **Project**, **Repository**, **Source**, **Published** or **Status** header; click again to reverse |
| Resize a column | Drag the border in the header |
| Scroll | Mouse wheel |
| Copy the status text | **Right-click** the status bar > Copy |
| Text boxes | Right-click for Cut / Copy / Paste / Select all |
| Switch dark / light | The moon / sun button at the bottom right |

## What you need

| I want to... | You need |
|---|---|
| **Just use the app** | Windows 10 or 11 and an internet connection. Nothing else. |
| **Build the `.exe` myself** (`build.bat`) | [Rust](https://rustup.rs) (default "MSVC" option) and **Build Tools for Visual Studio** with the workload *Desktop development with C++* (it includes the Windows SDK). Internet for the first build (downloads a few crates). |

## Build it

1. Install the two things above, then open a **new** window.
2. Double-click `build.bat`.
3. The result is in `dist\`:
   - `GitHubReleaseTracker.exe`
   - `GitHubReleaseTracker.exe.sha256`
   - `release-notes-vX.Y.Z.md` (paste into the GitHub release)

See [RELEASING.md](RELEASING.md) for the release steps, including the GitHub Actions option.

By hand: `cargo test --lib` runs the logic tests, `cargo build --release` builds `target\release\github-release-tracker.exe`.

## Settings and data

- `projects.json` is compatible with the Python version in both directions (custom names are ignored by Python versions older than 1.1.0).
- The optional GitHub token (Settings) is stored in plain text in `projects.json` and never exported. Normal use does not need it: releases are read from GitHub's public feed. It is sent to `github.com` only, never to another host.
- Projects on other hosts are read through that host's public API (GitLab: `/api/v4/projects/<path>/releases`, Gitea / Forgejo / Codeberg: `/api/v1/repos/<owner>/<repo>/releases`). Draft releases and GitLab "upcoming" releases are ignored. For a server it does not know, the app tries the Gitea / Forgejo API first and then GitLab's, once, when you add the project. No token is ever sent to these hosts (the optional GitHub token is used for `github.com` only).
- Links stored in your data are only opened if they point to the project's own host.
- Other hosts: public repositories only (no token), official releases only (not bare tags), and there is no pre-release marker on GitLab. A self-hosted server with a certificate Windows does not trust is refused (the app never turns certificate checks off). A `http://` address is used only if you type it that way.
- Older versions (before this one) do not understand projects from other hosts: they skip them when reading `projects.json` and drop them when saving. Use the new version everywhere you share the file.

## Project layout

| Path | What it is |
|---|---|
| `src/lib.rs` | All logic without Windows code: parsing, release lookup, update detection, import/export, sorting. Unit-tested (`src/tests.rs`) |
| `src/ui.rs`, `src/dialogs.rs` | The Win32 windows |
| `src/http.rs` | HTTPS with WinHTTP |
| `app.rc`, `app.manifest`, `icon.ico`, `build.rs` | Icon, version info, DPI and modern-controls manifest |
| `build.bat`, `tools/release-notes.ps1` | Local build and release notes |
| `.github/workflows/build.yml` | Manual cloud build |

## License

MIT. See [LICENSE](LICENSE).
