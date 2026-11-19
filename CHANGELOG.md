# Changelog

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
