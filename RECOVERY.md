# Nero recovery guide

The goal is that a complete Nero workspace can be recovered with no Nero-specific server or database.

## 1. Plain backup

```bash
nero backup verify backup.zip
nero backup restore backup.zip ~/recovered-notes
```

## 2. Encrypted backup

First make the private age identity available on the recovery machine.

```bash
nero backup verify backup.age --identity /path/to/identity.txt
nero backup restore backup.age ~/recovered-notes --identity /path/to/identity.txt
```

Or, if the identity is in Nero's default configuration location:

```bash
nero backup verify backup.age
nero backup restore backup.age ~/recovered-notes
```

## 3. Test recovery regularly

```bash
nero backup recovery-test backup.age
```

This performs the entire decrypt → restore → manifest verification path in a temporary directory without changing the real workspace.

## 4. Git recovery

A Git repository can be cloned normally:

```bash
git clone git@github.com:you/notes.git ~/notes
cd ~/notes
nero reindex
```

Nero does not require `.nero/index.sqlite` to be present in Git because the index is disposable.

## 5. Keep the identity separate

The age private identity is the key to encrypted backups. Store an additional offline copy or another protected backup of the identity. Do not commit it to the Git repository or put it beside encrypted backups in unprotected storage.
