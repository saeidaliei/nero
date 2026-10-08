# Nero roadmap

## 1.0 hardening

The 1.0 foundation is treated as stable. This pass locks down filesystem boundaries, atomic writes, recovery, deterministic toolchain selection, and renderer consistency before any new product area is considered.

## 0.1 — foundation

- [x] Rust workspace
- [x] filesystem-backed notes
- [x] note creation
- [x] listing
- [x] opening/editing via `$EDITOR`
- [x] simple search
- [x] `[[links]]`
- [x] backlinks
- [x] daily notes
- [x] doctor

## 0.2 — document engine

- [x] Comrak Markdown parser/renderer
- [x] frontmatter extraction
- [x] robust enough wiki-link extraction and resolution
- [x] SQLite FTS5 index
- [x] incremental index refresh
- [x] file watcher
- [x] ranked search results
- [x] HTML rendering command
- [x] task and math metadata

## 0.3 — TUI

- [x] Ratatui application shell
- [x] note list
- [x] reader pane
- [x] keyboard navigation
- [x] search mode
- [x] command palette
- [x] backlinks panel
- [x] recent notes
- [x] context panel
- [x] terminal math rendering (lightweight Unicode renderer)
- [x] live file-change reload
- [x] polished typography/spacing

## 0.4 — core refinement

- [x] formal `Document` model containing source and derived metadata
- [x] explicit `WikiLink` targets and display labels
- [x] deterministic path/title/stem link resolution
- [x] backlink resolution through current note identity rather than cached aliases
- [x] TUI renderer consumes parsed document source

## 0.5 — GUI

- [x] Tauri shell
- [x] beautiful Markdown renderer
- [x] KaTeX
- [x] command palette
- [x] native file watching / live reload
- [x] backlinks/context sidebar
- [x] note creation and daily note commands

## 0.6 — editing

- [x] split source/preview mode
- [x] embedded Markdown source editor
- [x] explicit save through the Rust core
- [x] dirty state and external-change protection
- [x] line numbers and indentation helpers

## 0.7 — editor quality

- [x] syntax-aware Markdown highlighting
- [x] scroll-synchronized preview
- [x] smart list/task/blockquote continuation
- [x] selection preservation across mode changes
- [x] native image import into `assets/`
- [x] workspace-safe image rendering
- [ ] drag/drop attachments
- [ ] paste images from the clipboard
- [x] heading-aware preview navigation

## 0.8 — resilience foundation

- [x] audit core invariants before adding cloud features
- [x] portable local ZIP backups
- [x] backup manifest with SHA-256 hashes
- [x] backup verification
- [x] safe restore into an empty destination
- [x] symlink and path-traversal protections
- [x] document encryption/storage architecture
- [x] security model and recovery notes

## 0.9 — encrypted backup + versioning

- [x] age-compatible X25519 backup encryption
- [x] private identity stored outside the workspace
- [x] public recipient export
- [x] Git versioning helpers
- [x] GitHub/GitLab/self-hosted remote workflow through the Git executable
- [x] recovery test command
- [ ] passphrase-protected identity/backup mode
- [ ] backup retention policy

## 1.0 — remote storage

- [x] rclone transport adapter
- [x] named storage profiles without embedded credentials
- [x] Mega support through rclone
- [x] generic S3/WebDAV/Drive support through rclone
- [x] encrypted-backup policy per storage profile
- [x] remote backup listing, upload, download, and restore
- [ ] scheduled backup hooks
- [ ] backup status in GUI/TUI

## Later, only if needed
- [ ] export to HTML/PDF
- [ ] optional bidirectional synchronization
- [ ] embedded editor engine

## Non-goals

Nero is not intended to become a general project-management suite, collaborative workspace, or block-based database application.
