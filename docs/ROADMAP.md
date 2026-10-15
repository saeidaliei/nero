# Nero roadmap

Nero's guiding principle is that **Markdown files remain the source of truth**. The CLI, TUI, desktop GUI, search index, and documentation should make those files easier to use without making notes depend on a proprietary format or a running service.

This roadmap is ordered by priority rather than by promises about release dates. Checkboxes should only be marked complete after the behavior is implemented and verified on the supported platforms.

## Current focus: release readiness

The next milestone is a reproducible release with green tests and working desktop builds—not another large feature area.

### P0 — correctness and CI

- [ ] Run the complete Rust check suite on the exact release candidate:
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets -- -D warnings`
  - `cargo test --workspace --all-targets`
  - `./tests/smoke.sh`
- [ ] Run the GUI tests and production frontend checks from `gui/`:
  - `npm ci`
  - `npm test`
  - `npx tsc --noEmit`
  - `npm run build`
- [ ] Run `cargo check --manifest-path gui/src-tauri/Cargo.toml` and `cargo fmt --manifest-path gui/src-tauri/Cargo.toml -- --check`.
- [ ] Ensure the latest frontmatter parser regression test passes and checks all ten fixture fields, including `title`.
- [ ] Keep backup regression tests green, including workspace-boundary checks when destination parents do not exist and portable full-home restore.
- [ ] Verify backup destination validation on Linux, Windows, and macOS, including canonical paths, Windows extended/short path aliases, symlinks, and `..` traversal.
- [ ] Make the smoke test pass end to end, including create/delete, indexing and links, backup restore, and fake-`rclone` upload **and** download.
- [ ] Ensure CI runs the checks above on the intended operating systems and fails on errors rather than silently skipping a check.

### P0 — release workflow and artifacts

- [ ] Verify release tag, workspace/package versions, Tauri configuration, and generated artifact names agree.
- [ ] Verify the release publish job checks out the repository, has `contents: write`, and passes `--repo "$GITHUB_REPOSITORY"` to `gh release create`.
- [ ] Test a release candidate from tag through artifact upload and GitHub Release publication.
- [ ] Inspect the actual Linux, Windows, and macOS artifacts; verify expected files are present and names are understandable.
- [ ] Test launching the packaged GUI on each supported operating system, not just compiling it.
- [ ] Document any platform-specific dependencies and installation/uninstallation steps.

### P0 — docs and search are part of the release

- [ ] Run the documentation synchronization script and build the Hugo site successfully.
- [ ] Test deployed search with words that occur only in a page's body, multi-word queries, empty queries, and no-result queries.
- [ ] Check internal documentation links and ensure examples still match the actual CLI behavior.
- [ ] Verify the documentation navbar, narrow-window layout, typography, and white page background in a real browser.
- [ ] Ensure the help text, README, GUI guide, and roadmap describe the same commands and behavior.

## P1 — usability and data quality

### Frontmatter contract

- [ ] Document the exact supported frontmatter syntax and behavior in one canonical guide.
- [ ] Clearly distinguish fields Nero actively interprets from optional user-defined metadata.
- [ ] Add validation and tests for malformed frontmatter, duplicate keys, empty values, invalid dates, and unexpected field types.
- [ ] Decide and document whether tags support comma-separated strings, YAML lists, or both.
- [ ] Decide how inline comments, quoted strings, colons inside values, multiline values, and nested YAML should behave. If these are unsupported, show a useful warning rather than silently implying full YAML support.
- [ ] Test metadata consistency across CLI output, TUI context, GUI inspector/chips, search results, and exported/rendered HTML.
- [ ] Keep creation defaults consistent: new notes should include a title and creation date; daily notes should include their date and `type: daily`.
- [ ] Keep `due: YYYY-MM-DD` behavior documented and tested, including the due date itself, overdue dates, malformed dates, and timezone-independent date-only handling.

### Note management and search

- [ ] Verify persistent integer IDs (`nero list`, `nero list -l`, and `nero edit/delete <ID>`) on Linux, Windows, and macOS, including workspace backup/restore, Git checkout, deletion, and recreating a note at the same path.
- [ ] Verify `nero config show` presents workspace, key, Git, and rclone settings without leaking private key material or credentials.
- [ ] Confirm `nero delete` / `nero rm` rejects paths outside the active workspace and handles nested note paths, missing files, and confirmation policy consistently.
- [ ] Confirm deleted notes disappear from search results and backlinks after index refresh.
- [ ] Test search ranking for title matches, body matches, multiple query terms, Unicode/case folding, and punctuation.
- [ ] Verify wiki-link resolution and backlinks for note title, filename stem, relative path, aliases, and fragments.
- [ ] Ensure broken-link diagnostics identify the source note and target clearly and avoid false positives for external URLs or code examples.
- [ ] Keep file watching resilient to rapid saves, atomic editor writes, renames, and deleted files.

### GUI and TUI quality

- [ ] Manually test the desktop GUI at narrow and wide window sizes, including search-bar layout and hidden/visible context panels.
- [ ] Test keyboard-only navigation, focus order, visible focus, command/search shortcuts, and dialogs.
- [ ] Verify that the note title is not duplicated when the first Markdown H1 matches it, while later headings still render normally.
- [ ] Test unsaved edits, external file changes, conflicts, and save errors without losing note content.
- [ ] Check local image paths and imported images in reading, editor, and split-preview modes.
- [ ] Check math, wiki links, fenced code, raw HTML handling, long lines, large notes, and Unicode in CLI, TUI, and GUI.
- [ ] Add a short manual GUI smoke-test checklist that can be followed before each release.

### `nero doctor` and diagnostics

- [ ] Improve `nero doctor` messages for malformed optional metadata, invalid `due` values, missing assets, broken links, stale indexes, and inaccessible workspace paths.
- [ ] Keep diagnostics actionable: state what was detected, which file is affected, and what command or edit can fix it.
- [ ] Distinguish errors from warnings and informational notices consistently across CLI output.

## P1 — backup and recovery confidence

- [ ] Test plain ZIP and encrypted backups from creation through verification and restore on all supported operating systems.
- [ ] Keep full-home backup/restore tests covering configuration, identity keys, workspace registrations, and named workspaces.
- [ ] Test restore into empty destinations, refusal to overwrite existing data, corrupt archives, missing keys, and incorrect passphrases.
- [ ] Confirm backup archives can be moved to another machine and restored without machine-specific absolute paths leaking into portable configuration.
- [ ] Document which material is encrypted, where keys live, how recovery works, and which data is intentionally excluded.
- [ ] Consider configurable backup retention after the safety and restore contract is stable.
- [ ] Consider a clear backup status/history surface in the GUI or TUI.
- [ ] Consider optional scheduled-backup hooks only after failure reporting and retention semantics are designed.

## P1 — installation and developer experience

- [ ] Keep `docs/GUI.md` explicit about the difference between `npm run tauri:dev`, a production Tauri bundle, and the `nero gui` launcher (`nero-gui` on `PATH`).
- [ ] Test a clean checkout setup using only documented commands and pinned Rust/Node toolchains.
- [ ] Keep dependency installation reproducible with lockfiles and `npm ci` in CI.
- [ ] Verify command help, README quick start, error messages, and examples after CLI changes.
- [ ] Make release notes explain relevant behavior changes, migration steps, and platform limitations.

## P2 — optional features, only when they solve a real need

These are candidates, not commitments. Preserve the filesystem-first model and avoid adding a feature solely because other note apps have it.

- [ ] Drag-and-drop attachments into the editor.
- [ ] Paste images from the clipboard into a note and store them safely under `assets/`.
- [ ] Export selected notes to standalone HTML and PDF while documenting how links and math are rendered.
- [ ] Add a passphrase-protected identity or backup mode if it improves recovery without making key management confusing.
- [ ] Add optional bidirectional synchronization only after conflict resolution, deletion propagation, offline edits, and recovery behavior are clearly specified.
- [ ] Consider a more capable embedded editor only if the current editor's maintainability or accessibility becomes a real limitation.

## Existing foundations

The project has already implemented the core of these areas; retain regression coverage as they evolve.

- [x] Rust workspace with filesystem-backed Markdown notes.
- [x] CLI commands for note creation, listing, opening, editing, search, deletion, daily notes, rendering, indexing, and workspace health checks.
- [x] Frontmatter extraction and default title/date metadata for newly created notes.
- [x] Wiki links, aliases/link resolution, and backlinks.
- [x] SQLite FTS5 search index with incremental refresh and file watching.
- [x] Comrak rendering, math support, task parsing, and HTML rendering command.
- [x] TUI with note list, reader, search, command palette, backlinks, context, and live reload.
- [x] Tauri desktop GUI with Markdown reading/editing, split preview, KaTeX, note creation, file watching, metadata display, and local image import.
- [x] Workspace selection, named workspaces, application-home configuration, and one-command `-w` / `--workspace` override.
- [x] Portable ZIP backups, manifests and hashes, backup verification, safe restore, and recovery tests.
- [x] Age-compatible encrypted backups and full-home backup/restore support.
- [x] Git versioning helpers and rclone-based remote backup transport, including encrypted-backup policy per storage profile.
- [x] Documentation site and CI/release workflows.

## Non-goals

Nero is not intended to become a general project-management suite, a collaborative multi-user service, a block-based database, or a proprietary notes format. Notes should remain readable and editable with ordinary text tools even if Nero is no longer installed.

## Definition of release-ready

A release candidate is ready when:

1. The Rust formatting, Clippy, test, and smoke-test commands pass on the intended CI matrix.
2. GUI unit tests, type-checking, frontend build, and Tauri builds pass; the resulting app is launched and smoke-tested on supported operating systems.
3. Backup, restore, encrypted recovery, and workspace path-boundary tests pass cross-platform.
4. Documentation sync/build and real search queries pass; links and installation instructions are checked.
5. The release workflow publishes the intended artifacts from a tag, and the artifacts can be downloaded and run on a clean machine.
