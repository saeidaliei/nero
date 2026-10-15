# GitHub CI

Nero's GitHub Actions workflow lives in `.github/workflows/ci.yml`.

It runs on pushes to `main`, pull requests, and manual dispatches. The workflow deliberately stays small:

- **Rust matrix:** Ubuntu, macOS, and Windows; `fmt`, `check`, `clippy`, tests, and release builds.
- **GUI:** Node 24, TypeScript typecheck, Vite build, and Tauri backend check.
- **Smoke test:** builds the CLI in debug mode and runs `tests/smoke.sh`, including workspace selection from outside a workspace, named workspace overrides, local backup, encrypted recovery, Git, and provider-neutral storage behavior.

The workspace declares Rust 1.90 as its minimum supported version, so CI pins that toolchain rather than silently testing only the newest stable compiler.

The workflow uses current major GitHub Actions for checkout/setup-node; the upstream repositories currently publish checkout 7 and setup-node 7 as their current major lines. `upload-artifact` is not needed by CI yet, keeping the workflow focused on verification instead of artifact management.

## Releases

Release automation lives in `.github/workflows/release.yml`. A semantic-version tag such as `vX.Y.Z` starts a version check, runs formatting, Clippy, and tests, then builds the CLI and desktop GUI for Linux x86_64, macOS Intel, macOS Apple Silicon, and Windows x86_64. Individual installers are attached directly to the GitHub Release rather than hiding all platform bundles inside a generic archive:

- CLI/TUI: `.tar.gz` on Linux/macOS and `.zip` on Windows.
- Linux GUI: `.deb` and `.rpm`; an `.AppImage` is attempted separately with `NO_STRIP=true`. The AppImage step is non-blocking because Tauri's upstream `linuxdeploy` can fail independently; a failure should not discard the successful DEB/RPM packages.
- macOS GUI: `.dmg` for Intel and Apple Silicon.
- Windows GUI: `.msi` and/or NSIS setup `.exe`.
- Arch Linux CLI/TUI: a best-effort `.pkg.tar.zst` build with `makepkg` inside an Arch Linux container. A packaging hiccup does not block the other installers. It installs `/usr/bin/nero`; it is not a package of the desktop GUI.
- `SHA256SUMS.txt`: checksums for all attached release assets.

The macOS CI build uses Tauri's ad-hoc signing identity rather than a Developer ID certificate. Until signing and notarization are configured, macOS may show a Gatekeeper warning on first launch.



## Documentation site

Nero's canonical `docs/` directory is published as the project documentation site at:

https://saeidaliei.github.io/nero/

The Hugo source lives in `docs-site/` and is built by `.github/workflows/docs.yml`.
The workflow generates the Hugo content tree from `docs/` so the documentation is not maintained twice.
