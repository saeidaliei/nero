# Nero security model

Nero is local-first. The live workspace is intentionally ordinary files on disk.

## What Nero protects today

- The `.nero/` index is disposable and is never part of a backup archive.
- Backup archives contain a manifest and SHA-256 hashes for every backed-up file.
- Backup restore refuses non-empty destinations and rejects absolute or `..` paths from untrusted archives.
- Workspace asset reads are constrained to the workspace root.

## Backup encryption model

Nero uses the age format for backup encryption, implemented through the Rust `age` crate and interoperable with the `rage` CLI and reference age implementation.

- **Workspace backups** use a generated X25519 identity. The private decryption identity lives at `$NERO_HOME/keys/identity.txt` (default `~/.nero/keys/identity.txt`), outside the workspace. `nero key generate` creates the identity and its public recipient; the identity file is written with restrictive permissions on Unix. Keep the private identity safe because these workspace `.age` archives do not contain it.
- **Full-home backups** (`nero backup home create`) use a separate passphrase-based age encryption. The archive can therefore include the private identity and the rest of the Nero profile without encrypting a key with itself. The passphrase is prompted interactively, is never saved by Nero, and cannot be recovered if lost.

Nero does not encrypt the live Markdown workspace. The source files remain interoperable with external editors, Git history, shell tools, and direct filesystem access. Encryption applies to backup artifacts rather than the live source tree.

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
