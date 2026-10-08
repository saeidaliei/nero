# Nero

> Plain Markdown. Local files. Fast search. Beautiful reading.

Nero is a local-first knowledge tool built around a deliberately boring idea:
**Markdown files are the source of truth.** Everything else is an interface or a cache.

The long-term plan is one core with three interfaces:

- `nero` — CLI
- `nero-tui` — terminal UI
- GUI — a future native app backed by the same core

## Design principles

1. **Files first.** A note is a normal `.md` file you can edit with Vim, Emacs, VS Code, or anything else.
2. **Small surface area.** Nero should solve notes, linking, search, and reading exceptionally well before adding anything else.
3. **Unix-friendly.** Commands should compose with other tools, and machine-readable output should be easy to add.
4. **No lock-in.** Deleting Nero must never make your notes unusable.
5. **Beautiful by restraint.** Reading is the primary experience; chrome is secondary.

## Current MVP

The first slice implements:

- workspace initialization
- Markdown note creation
- note listing
- note opening
- `$EDITOR` integration
- full-text-like filename/content search without a database dependency yet
- `[[wiki-style links]]`
- backlinks
- daily notes
- a `doctor` command for workspace sanity checks
- a TUI entry point reserved for the next iteration

This first implementation intentionally uses **only the Rust standard library**. It gives us a clean foundation before introducing SQLite/FTS5, Markdown rendering, Ratatui, or a GUI.

## Example

```bash
nero init ~/notes
cd ~/notes
nero new "Fourier Transform"
nero edit "Fourier Transform"
nero find "frequency"
nero today
nero backlinks "Fourier Transform"
nero tui
```

A note can contain ordinary Markdown and math:

```markdown
# Fourier Transform

For a function $f(t)$:

$$
F(\omega) = \int_{-\infty}^{\infty} f(t)e^{-i\omega t}\,dt
$$

See also [[Signal Processing]].
```

## Architecture

```text
Markdown files
      │
      ▼
┌───────────────────┐
│    nero-core      │
│ notes / links /   │
│ search / storage  │
└─────────┬─────────┘
          │
     ┌────┼────┐
     ▼    ▼    ▼
    CLI   TUI  GUI*

* planned
```

The eventual search index will be disposable. The authoritative data will remain the Markdown files.

## Development

A Rust toolchain with Rust 1.85+ is expected.

```bash
cargo test --workspace
cargo run -p nero -- --help
```
