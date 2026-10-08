# Development

## Toolchains

The repository pins Rust 1.90 in `rust-toolchain.toml` and Node 24 in `gui/.nvmrc`.

The workspace uses Rust 2024 edition.

## Build and test

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --release
```

For the GUI:

```bash
cd gui
npm ci
npx tsc --noEmit
npm run build
```

## Source layout

```text
crates/
├── nero-core/     document model, filesystem, index, backup, storage, Git
├── nero/          CLI
└── nero-tui/      Ratatui interface

gui/
├── src/           TypeScript desktop frontend
└── src-tauri/     Tauri commands and application state

tests/             end-to-end smoke test
docs/              project documentation
```

## Where logic belongs

Put behavior in `nero-core` when it is about notes, files, Markdown, links, search, backup, storage, or Git.

The CLI, TUI, and GUI should primarily translate user input into core API calls and present the result. Avoid implementing a second document parser in a frontend.

## Comments and documentation

Comments should explain **why** a piece of code exists when the reason is not obvious from the code itself. Good examples are:

- workspace containment checks
- atomic replacement semantics
- backup security boundaries
- age streaming and secret handling
- SQLite schema reset/versioning
- Tauri event/command boundaries

Do not add comments that merely restate `for`, `if`, or straightforward assignments.

## Tests

Security-sensitive behavior should have regression tests next to the implementation: path traversal, symlinks, unsafe Git arguments, backup verification, and encrypted recovery.

The `tests/smoke.sh` script is intentionally coarse-grained. It exercises the major CLI/recovery/storage paths.

## Formatting

Use `cargo fmt` for Rust. The CI workflow treats formatting and Clippy warnings as failures.

## Pull requests

Prefer small changes that preserve the filesystem-first model. New features should document their effect on Markdown portability, backup/recovery, and all three frontends before implementation.
