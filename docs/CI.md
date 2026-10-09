# GitHub CI

Nero's GitHub Actions workflow lives in `.github/workflows/ci.yml`.

It runs on pushes to `main`, pull requests, and manual dispatches. The workflow deliberately stays small:

- **Rust matrix:** Ubuntu, macOS, and Windows; `fmt`, `check`, `clippy`, tests, and release builds.
- **GUI:** Node 24, TypeScript typecheck, Vite build, and Tauri backend check.
- **Smoke test:** builds the CLI in debug mode and runs `tests/smoke.sh`, including workspace selection from outside a workspace, named workspace overrides, local backup, encrypted recovery, Git, and provider-neutral storage behavior.

The workspace declares Rust 1.90 as its minimum supported version, so CI pins that toolchain rather than silently testing only the newest stable compiler.

The workflow uses current major GitHub Actions for checkout/setup-node; the upstream repositories currently publish checkout 7 and setup-node 7 as their current major lines. `upload-artifact` is not needed by CI yet, keeping the workflow focused on verification instead of artifact management.

## Releases

Release automation lives in `.github/workflows/release.yml`. A tag such as `v1.0.0` starts a version check, builds the CLI and Tauri desktop bundles for Linux x86_64, macOS Intel, macOS Apple Silicon, and Windows x86_64, creates SHA-256 checksums, and publishes a GitHub Release.

The macOS CI build uses Tauri's ad-hoc signing identity only when no Apple certificate is configured. Proper Developer ID signing and notarization can be added later without changing the release artifact flow.



## Documentation site

Nero's canonical `docs/` directory is published as the project documentation site at:

https://saeidaliei.github.io/nero/

The Hugo source lives in `docs-site/` and is built by `.github/workflows/docs.yml`.
The workflow generates the Hugo content tree from `docs/` so the documentation is not maintained twice.
