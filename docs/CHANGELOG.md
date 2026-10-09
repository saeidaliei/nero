# Changelog

## 1.0.0

Build/test fixes applied after the first user Cargo run:

- Fixed TUI `App` initialization to initialize its parsed `document` state.
- Clear the current document and selected path when a search returns no matches; removed a stale reference to the deleted `body` field.
- Removed an unused Ratatui `Position` import.
- Stream ZIP archives so encrypted backup output only needs `Write`, not `Seek`.
- Index the string target of each structured `WikiLink` rather than binding the struct itself to SQLite.
- Convert a note's relative path to the string query expected by `read_note` during `nero doctor`, with a regression test for nested notes.
- Updated the TUI Markdown renderer test to parse its source into a `Document` before rendering.
- Corrected remote backup pull to call the static `Workspace::restore_*` functions using associated-function syntax.

- Documentation is consolidated under `docs/`, with dedicated usage, Markdown-format, development, GUI, security, recovery, backup, storage, CI, roadmap, and changelog guides.
- Source files include focused comments around filesystem boundaries, atomic writes, encryption, backup safety, indexing, Git argument validation, and Tauri/core boundaries.

This is the first stable Nero release. The final hardening pass tightened workspace boundaries, save/recovery safety, search/index reliability, and GUI rendering consistency.

- Refused absolute/path-traversal note lookups and symlinked notes; indexing no longer follows workspace symlinks.
- Note saves, new notes, daily notes, backup snapshots, encrypted backups, and backup identity files now use safer temporary-file replacement paths.
- Local daily notes now use the machine's local calendar date instead of UTC.
- Search snippets handle Unicode case-folding without slicing invalid UTF-8 boundaries.
- SQLite indexing now has a schema version, WAL mode, normal synchronous mode, and a busy timeout.
- GUI split preview now uses the same Comrak renderer as normal reading mode.
- Markdown rendering explicitly remains safe for raw HTML and dangerous links before content reaches the WebView.
- Encrypted backup detection now inspects the age header instead of trusting the filename extension; encrypted recovery uses OS-managed temporary files.
- Git branch arguments are rejected when they look like command-line options or contain whitespace.
- Storage configuration writes now use the same atomic file replacement path as notes and keys.
- Workspace filename/path lookup now respects native filesystem case semantics while keeping title matching friendly.
- Heading fragments in `[[Note#Heading]]` links now navigate to the matching heading in the desktop reader.
- The Rust toolchain is pinned to 1.90.0 and Node development to 24 for consistent local/CI environments.

- Added Comrak-powered Markdown rendering with math support.
- Added `[[wiki links]]`, backlinks, SQLite FTS5 search, file watching, and the Ratatui TUI.
- Added the Tauri desktop GUI with Markdown editing, split preview, KaTeX, image import, and context navigation.
- Added portable ZIP backups with manifests, SHA-256 verification, safe restore, and recovery tests.
- Added age-compatible X25519 encrypted backups and external key storage.
- Added Git helpers for versioning, remotes, snapshots, push, and pull.
- Added rclone-backed remote backup storage with named storage profiles.
- Added GitHub Actions CI and tag-driven cross-platform release automation.

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
