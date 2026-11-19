# Releasing (Rust edition)

The version lives in **one place**: `version = "X.Y.Z"` in `Cargo.toml`. The window title, About box and `build.bat` all read it. The Git tag must be `v` + that version (for example `v2.0.1`).

## Release steps

1. Change `version` in `Cargo.toml` (and the same numbers in `app.rc` and `app.manifest` if you want the exe's file properties to match).
2. Add a `## [X.Y.Z]` section at the top of `CHANGELOG.md`.
3. Run `cargo build` once so `Cargo.lock` picks up the new version, and commit `Cargo.lock` too (the build uses `--locked`).
4. Commit and push everything to GitHub.
5. Build the exe, with **one** of:
   - **Local:** double-click `build.bat`. Output: `dist\GitHubReleaseTracker.exe`, `.sha256`, and `release-notes-vX.Y.Z.md`.
   - **GitHub Actions:** *Actions > Build Windows exe > Run workflow*, then download the artifact `GitHubReleaseTracker-windows` from the finished run.
6. Run the exe once. Check that the title bar shows the new version.
7. On GitHub: *Releases > Draft a new release*. Tag `vX.Y.Z` (create it on publish), title `vX.Y.Z`, paste the notes file, attach `GitHubReleaseTracker.exe`, publish.

The workflow never creates or edits a Release, so it cannot overwrite your exe.

## Troubleshooting the build

| Message | Fix |
|---|---|
| `cargo not found` | Install Rust from https://rustup.rs, then open a new window |
| ``linker `link.exe` not found`` | Install *Build Tools for Visual Studio*, workload *Desktop development with C++* |
| `RC.EXE` / `could not embed the resources` | Same workload; tick a Windows 10/11 SDK |
| `the lock file needs to be updated` | Run `cargo build` once (without `--locked`) and commit `Cargo.lock` |
| Windows SmartScreen warns about the exe | The exe is not code-signed. Compare its SHA-256 with the one in the release notes |
