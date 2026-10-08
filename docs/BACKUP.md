# Nero backup design

Nero keeps the live workspace as ordinary files and treats backup, encryption, and versioning as separate layers.

## Local snapshot

```text
workspace
   │
   ▼
ZIP snapshot
├── manifest.json
├── notes/...
├── assets/...
└── other workspace files
```

`.nero/` and `.git/` are excluded because they are cache/version-control internals.

Each manifest entry records:

- relative path
- byte size
- SHA-256 digest

Commands:

```bash
nero backup create
nero backup inspect backup.zip
nero backup verify backup.zip
nero backup restore backup.zip /tmp/nero-restore
nero backup recovery-test backup.zip
```

## Encrypted snapshot

0.9 adds an age-compatible encryption layer around the same ZIP format:

```text
workspace → ZIP → age encryption → .age
```

The private age X25519 identity is stored outside the workspace by default. The public recipient can be shared with people/devices that should be able to encrypt backups.

Create a key once:

```bash
nero key generate
nero key show
nero key path
```

Then:

```bash
nero backup create --encrypt
nero backup verify nero-backup-123.age
nero backup recovery-test nero-backup-123.age
nero backup restore nero-backup-123.age /tmp/nero-restored
```

A non-default identity can be supplied explicitly:

```bash
nero backup verify backup.age --identity /path/to/identity.txt
nero backup restore backup.age /tmp/restore --identity /path/to/identity.txt
```

The encrypted backup is streamed through the age writer rather than first loading the entire workspace archive into memory. The age format is interoperable with `rage` and the reference age implementation. Nero currently uses the Rust `age` crate 0.12.1.

### Recovery rule

A backup is not considered healthy just because an archive exists. `recovery-test` decrypts when necessary, restores into a fresh temporary directory, and verifies every file against the backup manifest before deleting the temporary restore.

Keep at least one copy of the private identity outside the machine being backed up. Losing the identity without another copy means an age-encrypted backup cannot be decrypted.

## Git versioning

Git is for human-readable history and diffs, not as Nero's encrypted backup container:

```bash
nero git init
nero git status
nero git snapshot "Write Fourier notes"
nero git remote add origin git@github.com:you/notes.git
nero git push
nero git pull
```

The actual Git executable is used, so normal Git configuration, credential helpers, SSH keys, signed commits, GitHub, GitLab, and self-hosted Git servers remain available without Nero embedding a separate credential system.

`.nero/` is ignored by the workspace `.gitignore`, so the disposable SQLite index is not committed.

## Remote storage

The 1.0 storage layer is provider-neutral:

```text
workspace
   │
   ▼
backup snapshot
   │
   ▼
optional age encryption
   │
   ▼
rclone / another transport
   │
   ├── Mega
   ├── S3-compatible storage
   ├── Google Drive
   ├── WebDAV
   └── other configured providers
```

GitHub/GitLab are best treated as version-control remotes. Object storage is better suited to encrypted disaster-recovery snapshots and large assets.
