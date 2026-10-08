# Nero remote storage

Nero treats cloud storage as a transport layer, not as part of the note format. The live workspace remains ordinary Markdown files. Remote storage receives backup artifacts created by Nero.

## Requirements

Install `rclone` and configure at least one remote with:

```bash
rclone config
```

Nero uses the rclone configuration already available to the user. Nero does not copy provider credentials into the workspace.

## Configure a storage profile

```bash
nero storage add mega mega:nero-backups --encrypt
nero storage list
nero storage test mega
```

The profile contains only the rclone target and whether Nero should force encrypted `.age` backups. It does not contain passwords, OAuth tokens, or other provider secrets.

## Upload a backup

```bash
nero backup push mega
```

For an encrypted profile, Nero creates a temporary age-encrypted backup, uploads it, and removes the temporary local artifact. For a plain profile, the temporary artifact is an ordinary ZIP. You can also force encryption for a plain profile:

```bash
nero backup push mega --encrypt
```

Nero uploads one file with rclone `copyto --checksum --immutable`; it never uses `rclone sync` for backup upload, so an interrupted or misconfigured remote cannot cause Nero to delete unrelated remote data or silently replace an existing snapshot.

## List remote backups

```bash
nero backup list mega
```

## Restore

Restore into a new or empty directory:

```bash
nero backup pull mega nero-backup-1234567890.age ~/recovered-notes
```

Encrypted backups require the Nero age identity:

```bash
nero backup pull mega nero-backup-1234567890.age ~/recovered-notes --identity /path/to/identity.txt
```

## Providers

The provider is selected entirely in `rclone config`. Current rclone documentation lists Mega, Amazon S3, WebDAV, Google Drive, Proton Drive, and many other storage systems as supported backends.

## Design rule

Remote storage is **backup**, not bidirectional synchronization. Nero will not silently merge or delete note files based on remote state. A future synchronization feature, if we ever need one, will be a separate explicit operation.
