# Nero documentation

This directory contains the documentation for Nero itself. The root `README.md` is intentionally short; start here when you need details.

## Start here

- [Usage](USAGE.md) — installation, workspace setup, everyday CLI/TUI/GUI usage, and commands.
- [Markdown format](MARKDOWN.md) — the supported Markdown, math, wiki-link, frontmatter, task, and asset conventions.
- [Architecture](DESIGN.md) — the core model and how the CLI, TUI, GUI, index, backups, Git, and storage relate.
- [GUI](GUI.md) — desktop development and frontend details.
- [Development](DEVELOPMENT.md) — building, testing, formatting, local tooling, and contributing.

## Data safety and portability

- [Backups](BACKUP.md) — local ZIP snapshots, manifests, verification, and restore.
- [Recovery](RECOVERY.md) — disaster recovery and encrypted-backup recovery procedures.
- [Security](SECURITY.md) — workspace boundaries, encryption, secrets, and threat model notes.
- [Storage](STORAGE.md) — remote backup targets through `rclone`.

## Project maintenance

- [CI](CI.md) — GitHub Actions validation and release workflows.
- [Roadmap](ROADMAP.md) — deliberately small future work.
- [Changelog](CHANGELOG.md) — release history.

## Design rule

The most important sentence in the project is:

> **Markdown files are the source of truth. Everything else is an interface or a cache.**

When adding a feature, ask whether it preserves that property before adding another database, service, or proprietary format.
