# Nero

**Simple but powerful personal notes.** Nero is a local-first Markdown app with a command-line interface (CLI), a keyboard-driven terminal interface (TUI), and a desktop GUI. Your notes stay as ordinary Markdown files on your machine; no account or cloud service is required.

> **Want the desktop app?** Start with [Install the GUI](#desktop-gui). If no installer is attached to the latest release for your operating system, use [Build the GUI from source](#build-the-gui-from-source).

## Table of contents

- [Features](#features)
- [Install Nero](#install-nero)
  - [Desktop GUI](#desktop-gui)
  - [CLI and terminal UI](#cli-and-terminal-ui)
  - [Build the GUI from source](#build-the-gui-from-source)
- [Quick start](#quick-start)
- [Everyday commands](#everyday-commands)
- [Markdown and metadata](#markdown-and-metadata)
- [Where your data lives](#where-your-data-lives)
- [Backups and synchronization](#backups-and-synchronization)
- [Documentation](#documentation)
- [Development and tests](#development-and-tests)
- [Status and license](#status-and-license)

## Features

- **Plain Markdown files** with frontmatter, tasks, images, code blocks, and math.
- **Linked notes** using `[[Wiki Links]]`, backlinks, aliases, and heading fragments.
- **Full-text search** with a rebuildable local SQLite index.
- **Three interfaces:** CLI, terminal UI, and desktop GUI share the same Rust core.
- **Daily notes and due dates** without a separate task database.
- **Portable workflows** with Git versioning, local backups, encryption, and optional `rclone` remotes.

## Install Nero

Choose the interface you want. The CLI/TUI and desktop GUI are packaged separately; installing one does not automatically install the other.

### Desktop GUI

1. Open [Nero Releases](https://github.com/saeidaliei/nero/releases/latest).
2. Download the **`nero-gui-...`** archive for your operating system, if one is attached to the release. Use the Linux, Windows, or macOS build that matches your machine.
3. Extract the archive and install or open the platform bundle inside it:
   - **Linux:** install an included `.deb` with `sudo apt install ./name.deb`, install an `.rpm` with `sudo dnf install ./name.rpm`, or make an AppImage executable (`chmod +x name.AppImage`) and run it. The archive may not contain every format.
   - **Windows:** run the included `.msi` or `.exe` installer.
   - **macOS:** open the included `.dmg` and move Nero to Applications, or open the provided app bundle.

**If the release only contains “Source code” downloads and no `nero-gui-...` archive, a prebuilt desktop bundle is not available there yet.** In that case, follow [Build the GUI from source](#build-the-gui-from-source).

Once installed, launch Nero from your desktop environment's application menu. The `nero gui` command is a CLI convenience that starts a `nero-gui` executable found on your `PATH`; it works only when that executable is available there.

### CLI and terminal UI

**Requirements:** Git and Rust via [rustup](https://rustup.rs/). The repository pins its Rust toolchain in `rust-toolchain.toml`.

Install Nero's CLI and TUI from the repository:

```bash
git clone https://github.com/saeidaliei/nero.git
cd nero
cargo install --path crates/nero --force
```

Make sure Cargo's binary directory (`~/.cargo/bin`) is on your `PATH`. To build without installing, use:

```bash
cargo build --release -p nero
./target/release/nero --help
```

This builds the CLI and TUI, **not the desktop GUI**.

### Build the GUI from source

**Requirements:** Rust via rustup, Node.js 24, npm, and the native Tauri/WebView build dependencies for your operating system. See [GUI setup and platform requirements](docs/GUI.md).

Run these commands from the repository root to build the CLI and create a workspace for the GUI to open:

```bash
cargo build -p nero
./target/debug/nero init "$HOME/notes"
```

Then build and launch the development GUI. The project pins Node.js 24 in `gui/.nvmrc`; if you use nvm, run `nvm use` from `gui/` before installing dependencies. Otherwise, make sure Node.js 24 is active.

```bash
cd gui
npm ci
NERO_WORKSPACE="$HOME/notes" npm run tauri:dev
```

Tauri compiles the desktop backend, starts the frontend, and opens a desktop window. Keep the terminal open while using the development app; press `Ctrl+C` to stop it.

To build a production bundle instead of running in development mode:

```bash
npm run tauri:build
```

Look for the generated platform bundle under `gui/src-tauri/target/release/bundle/`. See [GUI documentation](docs/GUI.md) for details and platform-specific requirements.

## Quick start

After installing the CLI/TUI, create a workspace and a first note:

```bash
nero workspace create personal
nero workspace use personal
nero new "Welcome to Nero"
nero list
nero list --long  # include note frontmatter
nero config show  # inspect paths, workspace, keys, and remotes
nero tui
```

Open today's note and see notes due today or overdue:

```bash
nero today
```

Nero remembers your default workspace, so you can use it from any directory. For example:

```bash
cd ~/Downloads
nero new "An idea"
nero find "Fourier"
nero backlinks "Welcome to Nero"
```

Create another workspace and select it for one command with `-w`:

```bash
nero workspace create research
nero -w research new "Paper notes"
nero -w personal list
nero workspace list
```

## Everyday commands

| Command | What it does |
| --- | --- |
| `nero new "Title"` | Create a Markdown note with default frontmatter |
| `nero list` | Show a table with stable note IDs and timestamps |
| `nero list --long` | Include frontmatter metadata such as tags, type, due date, and priority |
| `nero open 3` | Print note ID 3 (titles and paths also work; `'#3'` is an explicit ID) |
| `nero edit 3` | Edit note ID 3 in `$EDITOR` or `$VISUAL` |
| `nero delete 3` / `nero rm 3` | Permanently delete note ID 3 |
| `nero config show` | Show resolved paths, workspace settings, backup-key status, Git/rclone remotes |
| `nero find "query"` | Search note content |
| `nero backlinks "Title"` | Find notes linking to a note |
| `nero today` | Open today's note and list due/overdue notes |
| `nero tui` | Open the terminal interface |
| `nero gui` | Launch `nero-gui`, if it is installed and on `PATH` |
| `nero reindex` | Rebuild/update the local search index |
| `nero doctor` | Check workspace health |
| `nero --help` | Show all commands and options |

## Markdown and metadata

Nero stores notes as regular Markdown files. New notes include a `title` and `date`; daily notes also include `type: daily`. Add `due` when a note should appear in `nero today` on or after that date.

```markdown
---
title: Fourier Transform
date: 2026-10-14
due: 2026-10-20
tags: math, signal-processing
status: in-progress
type: research
---

# Fourier Transform

For a function $f(t)$:

$$
F(\omega) = \int_{-\infty}^{\infty} f(t)e^{-i\omega t}\,dt
$$

- [ ] Review the derivation

See also [[Signal Processing]].
```

Frontmatter parsing is intentionally lightweight: Nero supports simple `key: value` lines, not the complete YAML specification. `title` and `due` have built-in behavior; other keys are optional metadata conventions. See [Markdown support](docs/MARKDOWN.md), [Due dates](docs/DUE_DATES.md), and the [examples directory](examples/).

## Where your data lives

- **Workspace:** Markdown notes, assets, a rebuildable `.nero` search index, and a persistent `.nero/note-ids.sqlite` ID registry.
- **Application home:** `~/.nero` by default, containing configuration, workspace registrations, backup keys, and local backups.
- **Relocation:** set `NERO_HOME` to an absolute path before running Nero to move the application home.

Your Markdown files are the source of truth; the SQLite index is a cache and can be rebuilt. See [Workspace configuration](docs/WORKSPACES.md) and [Application home](docs/APP_HOME.md).

## Backups and synchronization

Nero works offline. Git and `rclone` are optional additions for version history and off-machine backups:

- [Git versioning](docs/GIT.md) — snapshots, remotes, pushing, and cloning a workspace.
- [Backups](docs/BACKUP.md) — local ZIP and encrypted workspace backups.
- [Recovery](docs/RECOVERY.md) — restore notes and your full Nero home on another machine.
- [Remote storage](docs/STORAGE.md) — use `rclone` targets for remote backups.
- [Security](docs/SECURITY.md) — encryption, keys, and safety boundaries.

A private Git repository controls who can access its contents but does **not** client-side encrypt your notes. For disaster recovery, keep a separate encrypted backup and protect its passphrase or identity key.

## Documentation

- [Usage guide](docs/USAGE.md) — everyday CLI/TUI usage and commands.
- [Desktop GUI](docs/GUI.md) — GUI setup, development, and platform notes.
- [Markdown and math](docs/MARKDOWN.md) — frontmatter, wiki links, tasks, images, and equations.
- [Workspaces](docs/WORKSPACES.md) · [Application home](docs/APP_HOME.md) · [Due dates](docs/DUE_DATES.md)
- [Git](docs/GIT.md) · [Backups](docs/BACKUP.md) · [Recovery](docs/RECOVERY.md) · [Storage](docs/STORAGE.md) · [Security](docs/SECURITY.md)
- [Architecture](docs/DESIGN.md) · [Development](docs/DEVELOPMENT.md) · [CI and releases](docs/CI.md) · [Roadmap](docs/ROADMAP.md) · [Changelog](docs/CHANGELOG.md)

The documentation website is published at [saeidaliei.github.io/nero](https://saeidaliei.github.io/nero/).

## Development and tests

The repository pins Rust in `rust-toolchain.toml` and Node.js 24 in `gui/.nvmrc`.

Run the Rust checks from the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
./tests/smoke.sh
```

Run GUI checks from `gui/`:

```bash
npm ci
npm test
npx tsc --noEmit
npm run build
cargo check --manifest-path src-tauri/Cargo.toml
```

For development details and CI behavior, see [Development](docs/DEVELOPMENT.md) and [CI](docs/CI.md).

## Status and license

Nero is under active development. Some installation options depend on platform bundles being published with a release; if in doubt, use the source-build instructions above and check the [roadmap](docs/ROADMAP.md).

Licensed under the [MIT License](LICENSE).
