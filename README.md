# Nero

> A small place for your thoughts.
>
> Plain Markdown. Local files. Fast search. Beautiful reading.

Nero is a local-first knowledge tool built around one deliberately boring rule:

> **Markdown files are the source of truth. Everything else is an interface or a cache.**

It provides one core with three interfaces:

- `nero` — CLI
- `nero-tui` — terminal UI
- `nero-gui` — desktop GUI

## Quick start

```bash
nero init ~/notes
cd ~/notes
nero new "Fourier Transform"
nero edit "Fourier Transform"
nero find "frequency"
nero today
```

Start the TUI with:

```bash
cargo run -p nero -- tui
```

Start the desktop GUI from `gui/` with:

```bash
pnpm install
NERO_WORKSPACE=~/notes pnpm tauri dev
```

## Project structure

```text
crates/              Rust core + CLI + TUI
gui/                  Tauri desktop frontend
examples/             example Markdown notes
tests/                end-to-end smoke test
docs/                 full project documentation
.github/workflows/    CI and release automation
```

## Documentation

The full documentation lives in [`docs/`](docs/README.md):

- [Usage](docs/USAGE.md)
- [Markdown format](docs/MARKDOWN.md)
- [Architecture](docs/DESIGN.md)
- [Development](docs/DEVELOPMENT.md)
- [GUI](docs/GUI.md)
- [Backups](docs/BACKUP.md)
- [Recovery](docs/RECOVERY.md)
- [Security](docs/SECURITY.md)
- [Remote storage](docs/STORAGE.md)
- [CI and releases](docs/CI.md)
- [Roadmap](docs/ROADMAP.md)
- [Changelog](docs/CHANGELOG.md)

## License

MIT. See [`LICENSE`](LICENSE).
