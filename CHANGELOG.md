# Changelog

## 1.0.0

- Added a provider-neutral rclone storage adapter.
- Added named workspace storage profiles stored without provider credentials.
- Added `nero storage add`, `storage list`, `storage test`, and `storage remove`.
- Added `nero backup push`, `backup list`, and `backup pull`.
- Storage profiles can require age-encrypted backup artifacts.
- Remote transfers use rclone `copyto` and never use destructive `sync` operations.
- Added `STORAGE.md` with Mega, S3-compatible, WebDAV, and other rclone workflows.

## 0.9.0

- Added age-compatible X25519 encrypted backup snapshots.
- Added `nero key generate`, `key show`, and `key path`.
- Added streamed encrypted backup creation so ZIP plaintext is not loaded into memory.
- Added encrypted backup verification, restore, and `recovery-test`.
- Added Git helpers for init, status, snapshots, remotes, push, and pull.
- Added an explicit recovery guide and expanded the security model.
- Kept the private backup identity outside the workspace by default.

## 0.8.0

- Audited and hardened the core before adding remote storage features.
- Added portable local ZIP backups with a JSON manifest.
- Added SHA-256 integrity hashes for every backed-up file.
- Added backup inspection and verification.
- Added safe restore into a new or empty destination, with archive path traversal protection.
- Excluded `.nero/` and `.git/` from backups.
- Rejects symlinks in backup sources instead of following them outside the workspace.
- Streams file hashing instead of loading large assets into memory.
- Fixed Unicode-unsafe search preview slicing.
- Fixed image reference cleanup when Markdown image URLs contain fragments or query strings.
- Removed the placeholder GitHub repository URL from package metadata.
- Documented the planned age encryption and Git/rclone storage architecture.
## 0.7.0

- Added syntax-aware Markdown highlighting behind the real source textarea.
- Added synchronized source/preview scrolling in split mode.
- Added smart Enter behavior for unordered lists, ordered lists, tasks, and blockquotes.
- Added selection preservation across editor mode changes and saves.
- Added native image import through the Tauri dialog plugin.
- Added safe workspace image hydration and local image rendering in previews.

## 0.6.0

- Added embedded Markdown source editing to the desktop GUI.
- Added read, edit, and split source/preview modes.
- Added explicit save through the Rust document core.
- Added dirty-state protection and external-change handling.
- Added line numbers, Tab indentation, and simple bold/italic shortcuts.
- Added live preview for common Markdown while editing.
- Added `:edit`, `:save`, and `:split` commands to the command palette.

## 0.5.0

- Added the first desktop GUI under `gui/`.
- Added a Tauri 2 shell backed directly by `nero-core`.
- Added native commands for note listing, reading, searching, backlinks, link resolution, note creation, daily notes, and reindexing.
- Added live Markdown change events from the Rust watcher to the frontend.
- Added a restrained three-column reading layout with notes, reader, and context/backlinks panes.
- Added a command/search palette.
- Added KaTeX rendering for Comrak's annotated math nodes.
- Kept the GUI dependency-light: vanilla TypeScript/CSS, no frontend framework.

## 0.4.0

- Formalized the core `Document` model around the original Markdown source.
- Added structured `WikiLink` metadata with optional display labels.
- Made internal-link resolution relative to the source note, then workspace path, title, and filename stem.
- Backlinks now resolve against current workspace identity instead of relying on cached string aliases.
- TUI Markdown rendering now consumes the core document model.

## 0.3.0

- Reworked `nero-tui` into a full reading/navigation interface.
- Added a keyboard-first command palette.
- Added backlinks, recent-note, and context side panels.
- Added live Markdown-file reload and index refresh.
- Added scrolling and page navigation for long notes.
- Added terminal Markdown presentation with task/list/heading/link styling.
- Added lightweight Unicode rendering for common LaTeX constructs in display math.
- Wired `nero tui` directly to the TUI crate.


## 0.2.0

Nero's document engine milestone.

- Added Comrak-backed Markdown and math rendering
- Added lightweight frontmatter extraction
- Added wiki-link and task metadata extraction
- Added SQLite FTS5 indexing
- Added incremental index refreshes
- Added indexed backlink lookup
- Added cross-platform Markdown file watching
- Added `render`, `reindex`, and `watch` commands
- Added the first Ratatui TUI reader shell
- Moved workspace metadata from legacy `.note` to `.nero` (legacy discovery remains supported)
