# Nero documentation

This directory contains the detailed documentation for Nero. The root `README.md` is the project overview and quick start; start here for deeper usage and implementation guides.

## Start here

- [Usage](USAGE.md) — installation, everyday CLI/TUI/GUI usage, and commands.
- [Application home](APP_HOME.md) — `~/.nero`, configuration, keys, backup storage, and relocation with `NERO_HOME`.
- [Workspace configuration](WORKSPACES.md) — workspace folders, named workspaces, `-w`, and selection behavior.
- [Markdown format](MARKDOWN.md) — the supported Markdown, math, wiki-link, frontmatter, task, and asset conventions.
- [Git versioning](GIT.md) — private GitHub repository setup, history, and cloning.
- [Architecture](DESIGN.md) — the core model and how the CLI, TUI, GUI, index, backups, Git, and storage relate.
- [GUI](GUI.md) — desktop development and frontend details.
- [Development](DEVELOPMENT.md) — building, testing, formatting, local tooling, and contributing.

## Everyday use

- [Due dates](DUE_DATES.md) — date-only frontmatter surfaced by `nero today`.

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
