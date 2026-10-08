# Nero security model

Nero is local-first. The live workspace is intentionally ordinary files on disk.

## What Nero protects today

- The `.nero/` index is disposable and is never part of a backup archive.
- Backup archives contain a manifest and SHA-256 hashes for every backed-up file.
- Backup restore refuses non-empty destinations and rejects absolute or `..` paths from untrusted archives.
- Workspace asset reads are constrained to the workspace root.

## 0.9 encryption model

Nero should not encrypt the live Markdown workspace by default. Doing so would break the core interoperability promise: normal files, external editors, Git history, shell tools, and direct filesystem access.

The 0.9 encryption layer is for **backup data** only. The intended format is the age format, implemented natively through the Rust `age` crate. It is interoperable with the `rage` CLI and the reference age implementation. 0.9 uses an X25519 identity; passphrase-protected backup/identity mode is planned. The current Rust crate is 0.12.1.

The private decryption identity lives outside the workspace by default. `nero key generate` creates an age X25519 identity and a separate public recipient file; the identity file is written with restrictive permissions on Unix. A backup must remain useless to a storage provider that only has the encrypted archive.

Nero does not currently encrypt the live working tree. A later "vault" mode may provide encrypted working storage, but it should remain a separate product surface and must not distort the plain-files workspace model.

In 1.0, remote backup transport is delegated to rclone. Nero stores only a target path and an encrypted-backup policy in `.nero/storage.json`. Provider credentials remain in rclone's configuration. Backup uploads use `copyto`, not `sync`, to avoid destructive remote behavior.

## Remote storage

Storage adapters remain separate from encryption:

```text
workspace
   │
   ▼
backup snapshot
   │
   ▼
optional age encryption
   │
   ├── Git remote (GitHub, GitLab, self-hosted)
   └── rclone remote (Mega, S3, Drive, WebDAV, etc.)
```

GitHub can host private repositories accessible only to authorized users, but repository/object limits make it better suited to text/history than as a universal large-asset store. GitHub currently enforces a 100 MB single-object limit and recommends Git LFS for larger tracked files. For broader storage, rclone already supports Mega and a large set of other backends, while its `crypt` remote provides client-side encryption before data reaches the remote.

## 1.0 hardening

Nero treats the workspace as a trust boundary. Note resolution rejects absolute and parent-traversal paths, indexing skips symlinks, and file writes use temporary-file replacement paths. The GUI renderer uses Comrak's safe-by-default HTML/link handling before inserting rendered Markdown into the desktop WebView.

Encrypted backup recovery decrypts into OS-managed temporary files that are removed when the operation ends. The live workspace remains plain Markdown; encryption applies to backup artifacts rather than the source files.
