# Nero

Nero is a local-first Markdown knowledge tool with a CLI, keyboard-driven terminal UI, and desktop GUI. It focuses on quick capture, search, wiki links, readable documents, and mathematical notation—without requiring a cloud account or proprietary note format.

## Features

- **Plain Markdown** — notes are normal files with frontmatter, tasks, code blocks, images, and LaTeX-style mathematics.
- **Linked notes** — `[[Wiki Links]]`, labels, backlinks, and heading fragments.
- **Fast search** — ranked full-text search backed by a rebuildable SQLite FTS5 index.
- **Three interfaces** — CLI, Ratatui TUI, and Tauri desktop GUI use the same Rust core.
- **Your editor** — CLI and TUI can open notes with `$EDITOR`; the GUI includes an embedded Markdown editor.
- **Due dates** — `due: YYYY-MM-DD` frontmatter appears in `nero today` when due or overdue.
- **Portable data** — notes live on disk as Markdown; SQLite is only a cache.
- **Versioning and backups** — Git helpers, workspace snapshots, passphrase-encrypted full-home backups, and optional remote storage through rclone.

## Installation

Requirements: Git and Rust 1.90, pinned in `rust-toolchain.toml`.

```bash
git clone https://github.com/saeidaliei/nero.git
cd nero
cargo install --path crates/nero --force
```

Or build without installing:

```bash
cargo build --release -p nero
./target/release/nero --help
```

This installs the CLI and TUI. Build the desktop application separately; see [GUI setup](docs/GUI.md). Prebuilt desktop bundles, when published, are available from GitHub Releases.

## Quick start

Create a named workspace (under `~/.nero/workspaces/` by default):

```bash
nero workspace create personal
nero workspace use personal
nero new "Welcome to Nero"
nero edit "Welcome to Nero"
nero tui
```

Nero remembers the default workspace, so you can use it from any directory:

```bash
cd ~/Downloads
nero new "An idea"
nero find "Fourier"
nero today
nero gui
```

Create another workspace and select it for one command with `-w`:

```bash
nero workspace create research
nero -w research new "Paper notes"
nero -w personal list
nero workspace list
```

Nero's application home is `~/.nero`. Set `NERO_HOME` to an absolute path before running Nero if you want to move the entire configuration/workspace home. See [workspace configuration](docs/WORKSPACES.md).

## Quick start: private GitHub repository

For a longer versioning/clone guide, see [Git versioning](docs/GIT.md).

Git is for note history, diffs, and versioning. It sends the workspace's Markdown files to your repository; a private GitHub repository is access-controlled, **not client-side encrypted**.

1. [Create a new GitHub repository](https://github.com/new), select **Private**, and name it something like `nero-personal`. Leave it empty—don't add a README, `.gitignore`, or license because the local workspace already exists.
2. Make sure SSH access to GitHub is configured (`ssh -T git@github.com`).
3. Initialize/version the workspace and add the repository as its remote:

   ```bash
   nero -w personal git init
   nero -w personal git snapshot "Initial notes"
   nero -w personal git remote add origin git@github.com:YOUR_USERNAME/nero-personal.git
   git -C "$HOME/.nero/workspaces/personal" push -u origin HEAD
   ```

4. For later updates, commit a snapshot and push it:

   ```bash
   nero -w personal git snapshot "Update research notes"
   nero -w personal git push origin main
   ```

   If your branch isn't named `main`, replace it with the branch shown by `nero -w personal git status`.

GitHub is a good choice for version history and access across machines. For disaster recovery, also keep a separate encrypted backup.

## Quick start: encrypted backup to MEGA

For an encrypted snapshot of an individual workspace, Nero uses rclone's configured remote. Install [rclone](https://rclone.org/install/) and run `rclone config`; create a remote named `mega` using the Mega backend and verify it with `rclone lsd mega:`. See the [rclone Mega guide](https://rclone.org/mega/) if needed.

Then, with `personal` selected:

```bash
nero key generate
nero -w personal storage add mega mega:nero-backups/personal --encrypt
nero -w personal backup push mega
nero -w personal backup list mega
```

The key is created once. Keep its private identity safe—encrypted workspace snapshots need that identity to restore. The configured storage profile contains only the target and encryption setting, not your Mega credentials. Mega and rclone are optional; Nero works without either.

For moving Nero itself to a new machine, use a **full-home backup** instead:

```bash
nero backup home create
```

Creation prompts for a passphrase twice and prints the exact archive path. The `.zip.age` archive is passphrase-encrypted independently of the age identity stored inside it, so it can safely contain your notes, configuration, key, and workspaces. It omits prior archives and disposable SQLite indexes. Keep the passphrase separately—Nero cannot recover it.

After configuring the `mega` rclone remote, upload that exact archive. Replace the example filename below with the path Nero printed:

```bash
ARCHIVE="$HOME/.nero/backups/nero-home-backup-1234567890.zip.age"
nero backup home verify "$ARCHIVE"
rclone copyto "$ARCHIVE" "mega:nero-home-backups/$(basename "$ARCHIVE")"
```

`rclone copyto` copies one file without deleting unrelated remote files ([command reference](https://rclone.org/commands/rclone_copyto/)). You can also copy the archive to an external drive.

On a fresh machine, install Nero and configure the same `mega` rclone remote with `rclone config` (provider credentials are not included in the Nero archive). Then download the archive from Mega (replace the remote filename with the actual one):

```bash
rclone copyto "mega:nero-home-backups/nero-home-backup-1234567890.zip.age" "$HOME/nero-home-backup.zip.age"
nero backup home verify "$HOME/nero-home-backup.zip.age"
nero backup home restore "$HOME/nero-home-backup.zip.age"
nero workspace list
nero today
```

Enter the same passphrase. Restore targets `~/.nero` by default and requires it to be new or empty; it rebases paths for workspaces inside the restored Nero home. Full-home backup refuses to run if a registered workspace is outside `NERO_HOME` or advanced config/key overrides relocate config/keys from their standard locations, so it cannot produce a misleading incomplete snapshot. For a custom destination, pass it as a second argument and set `NERO_HOME` to that location on subsequent runs. See [backup and recovery](docs/BACKUP.md) for scope and limitations.

## Common commands

| Command | Purpose |
|---|---|
| `nero new "Title"` | Create a Markdown note |
| `nero edit "Title"` | Edit a note with `$EDITOR` |
| `nero list` | List notes |
| `nero find "query"` | Search notes |
| `nero backlinks "Title"` | Find incoming links |
| `nero today` | Open today's note and show due/overdue notes |
| `nero tui` / `nero gui` | Open the terminal or desktop interface |
| `nero workspace list` | List named workspaces |
| `nero home` | Show app-home paths |
| `nero backup create --encrypt` | Back up the selected workspace |
| `nero backup home create` | Create a passphrase-encrypted backup of the whole Nero home |
| `nero doctor` | Check workspace health and links |

Run `nero --help` for the full command list.

## Markdown and mathematics

```markdown
---
title: Fourier Transform
due: 2026-10-15
---

For a function $f(t)$:

$$
F(\omega) = \int_{-\infty}^{\infty} f(t)e^{-i\omega t}\,dt
$$

- [ ] Review the proof

See also [[Signal Processing]].
```

See [Markdown support](docs/MARKDOWN.md) for wiki-link, asset, task, and equation details.

## Documentation

- [Usage](docs/USAGE.md) · [Workspaces](docs/WORKSPACES.md) · [Application home](docs/APP_HOME.md)
- [Markdown and math](docs/MARKDOWN.md) · [Due dates](docs/DUE_DATES.md) · [Desktop GUI](docs/GUI.md)
- [Backups](docs/BACKUP.md) · [Recovery](docs/RECOVERY.md) · [Security](docs/SECURITY.md) · [Remote storage](docs/STORAGE.md)
- [Architecture](docs/DESIGN.md) · [Development](docs/DEVELOPMENT.md) · [CI](docs/CI.md) · [Changelog](docs/CHANGELOG.md)

The documentation site is built from these Markdown files and deployed to [saeidaliei.github.io/nero](https://saeidaliei.github.io/nero/) by GitHub Actions.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo build --workspace --release
```

The Tauri GUI has its own Cargo manifest and build process; see [GUI](docs/GUI.md) and [Development](docs/DEVELOPMENT.md).

## Project status

Version **1.0.0**. No account, cloud service, or background daemon is required. Git, rclone, and remote storage are optional. The source of truth is ordinary Markdown; indexes and local caches can be rebuilt.
