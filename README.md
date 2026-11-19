# GitHub Release Tracker (Rust edition)

A small, portable Windows app that tracks the latest release of the GitHub projects you care about and tells you when a new one appears. No account, no login, and no token needed.

> **Rewrite notice:** this project is a complete rewrite, in Rust with a native Win32 interface, of my earlier Python / tkinter app **[github-release-tracker](https://github.com/santosasarno-code/github-release-tracker)**. It has the same features and uses the same `projects.json`, so you can switch between the two without losing your list. The Python version stays available.

It ships as one small `.exe` with nothing to install.

🇮🇩 [Baca dalam Bahasa Indonesia](README.id.md)

## Features

- Add a project by pasting a GitHub URL or `owner/repo`
- See the project name, repository, current release, publish date and status at a glance
- **Check for updates** refreshes every project; a newer release is highlighted as `Update available: v1.0 -> v1.1`
- **Mark as seen** clears the highlight once you have dealt with an update
- Double-click a project row (or use **Open release page**) to open its release page in your browser; double-clicking the **Project** name renames it instead
- **Rename** a project: click the pencil that appears when you hover it, or select it and press **F2**, or right-click > Rename, or double-click the name. *Reset name* brings the GitHub name back
- Click the **Project**, **Repository**, **Published** or **Status** header to sort (click again to reverse)
- **Import / Export** the list as `.json` (merge or replace on import; the token is never exported)
- **Light / dark** mode: click the moon / sun icon at the bottom right (starts with your Windows theme)
- Columns resize in proportion when you resize the window; drag a column border to change the proportions
- Per-monitor DPI aware; thin separator lines between rows and columns
- Portable: one `.exe`, no installer, no registry entries. Data is `projects.json` next to the exe (falls back to `%APPDATA%\GitHubReleaseTracker` if that folder is read-only)

The app only reads release information. It never downloads or installs anything.

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
- The optional GitHub token (Settings) is stored in plain text in `projects.json` and never exported. Normal use does not need it: releases are read from GitHub's public feed.

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
