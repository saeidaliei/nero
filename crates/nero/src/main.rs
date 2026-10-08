use std::{env, io::{self, Write}, path::PathBuf, process::{exit, Command as ProcessCommand}};

use nero_core::{NoteSummary, Result, SearchResult, Workspace};

fn main() {
    if let Err(error) = run() {
        eprintln!("nero: {error}");
        exit(1);
    }
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "help".into());

    match command.as_str() {
        "help" | "--help" | "-h" => print_help(),
        "version" | "--version" => println!("nero {}", env!("CARGO_PKG_VERSION")),
        "init" => {
            let root = args.next().map(PathBuf::from).unwrap_or(env::current_dir()?);
            let workspace = Workspace::init(root)?;
            println!("initialized Nero workspace at {}", workspace.root().display());
        }
        "new" => {
            let title = join_args(args)?;
            let workspace = Workspace::discover(env::current_dir()?)?;
            let note = workspace.create_note(&title)?;
            println!("created {}", note.path.display());
        }
        "list" | "ls" => {
            let workspace = Workspace::discover(env::current_dir()?)?;
            print_notes(workspace.list_notes()?);
        }
        "open" => {
            let query = join_args(args)?;
            let workspace = Workspace::discover(env::current_dir()?)?;
            let note = workspace.read_note(&query)?;
            println!("{}\n\n{}", note.summary.title, note.document.source.trim_end());
        }
        "edit" => {
            let query = join_args(args)?;
            let workspace = Workspace::discover(env::current_dir()?)?;
            let status = workspace.edit(&query)?;
            if !status.success() { exit(status.code().unwrap_or(1)); }
        }
        "find" | "search" => {
            let query = join_args(args)?;
            let workspace = Workspace::discover(env::current_dir()?)?;
            print_search(workspace.search(&query)?);
        }
        "backlinks" => {
            let query = join_args(args)?;
            let workspace = Workspace::discover(env::current_dir()?)?;
            print_notes(workspace.backlinks(&query)?);
        }
        "today" => {
            let workspace = Workspace::discover(env::current_dir()?)?;
            println!("{}", workspace.today()?.path.display());
        }
        "render" => {
            let query = join_args(args)?;
            let workspace = Workspace::discover(env::current_dir()?)?;
            print!("{}", workspace.render_note_html(&query)?);
        }
        "reindex" | "index" => {
            let workspace = Workspace::discover(env::current_dir()?)?;
            let stats = workspace.reindex()?;
            println!("index refreshed: {} updated, {} removed", stats.indexed, stats.removed);
        }
        "watch" => {
            let workspace = Workspace::discover(env::current_dir()?)?;
            let (_watcher, events) = workspace.watch()?;
            println!("watching {} (Ctrl-C to stop)", workspace.root().display());
            for paths in events {
                for path in paths { println!("changed {}", path.display()); }
                let _ = workspace.reindex();
            }
        }
        "backup" => {
            let subcommand = args.next().unwrap_or_else(|| "help".into());
            let values = args.collect::<Vec<_>>();
            match subcommand.as_str() {
                "create" => {
                    let workspace = Workspace::discover(env::current_dir()?)?;
                    let (encrypt, identity, positional) = parse_backup_flags(values)?;
                    let default_extension = if encrypt { "age" } else { "zip" };
                    let destination = positional.first().map(PathBuf::from).unwrap_or_else(|| {
                        workspace.root().parent().unwrap_or(workspace.root()).join(format!("nero-backup-{}.{}", backup_timestamp(), default_extension))
                    });
                    let (path, manifest) = if encrypt {
                        let identity = identity.unwrap_or(Workspace::identity_path()?);
                        workspace.create_encrypted_backup_with_identity(destination, identity)?
                    } else {
                        workspace.create_backup(destination)?
                    };
                    println!("created {} ({} files, {} bytes){}", path.display(), manifest.files.len(), manifest.stats().bytes, if encrypt { " [encrypted]" } else { "" });
                }
                "inspect" => {
                    let archive = values.first().ok_or_else(|| nero_core::NeroError::Message("backup archive path is required".into()))?;
                    let manifest = Workspace::inspect_backup(archive)?;
                    println!("Nero backup v{} — {} files, {} bytes", manifest.version, manifest.files.len(), manifest.stats().bytes);
                    for entry in manifest.files {
                        println!("{:>10}  {}  {}", entry.size, entry.sha256, entry.path);
                    }
                }
                "verify" => {
                    let (identity, positional) = parse_identity_flag(values)?;
                    let archive = positional.first().ok_or_else(|| nero_core::NeroError::Message("backup archive path is required".into()))?;
                    let is_encrypted = PathBuf::from(archive).extension().and_then(|s| s.to_str()) == Some("age");
                    let stats = if is_encrypted {
                        Workspace::verify_encrypted_backup(archive, identity.unwrap_or(Workspace::identity_path()?))?
                    } else {
                        Workspace::verify_backup(archive)?
                    };
                    println!("backup verified: {} files, {} bytes{}", stats.files, stats.bytes, if is_encrypted { " [decrypted + verified]" } else { "" });
                }
                "restore" => {
                    let (identity, positional) = parse_identity_flag(values)?;
                    let archive = positional.first().ok_or_else(|| nero_core::NeroError::Message("backup archive path is required".into()))?;
                    let destination = positional.get(1).ok_or_else(|| nero_core::NeroError::Message("restore destination is required".into()))?;
                    let is_encrypted = PathBuf::from(archive).extension().and_then(|s| s.to_str()) == Some("age");
                    let stats = if is_encrypted {
                        Workspace::restore_encrypted_backup(archive, destination, identity.unwrap_or(Workspace::identity_path()?))?
                    } else {
                        Workspace::restore_backup(archive, destination)?
                    };
                    println!("backup restored: {} files, {} bytes{}", stats.files, stats.bytes, if is_encrypted { " [decrypted]" } else { "" });
                }
                "push" => {
                    let (force_encrypt, identity, positional) = parse_backup_push_flags(values)?;
                    let storage_name = positional.first().ok_or_else(|| nero_core::NeroError::Message("storage remote name is required".into()))?;
                    let workspace = Workspace::discover(env::current_dir()?)?;
                    let remote = workspace.storage_get(storage_name)?;
                    let encrypt = remote.encrypt_backups || force_encrypt;
                    let extension = if encrypt { "age" } else { "zip" };
                    let temporary = env::temp_dir().join(format!("nero-upload-{}-{}.{}", std::process::id(), backup_timestamp(), extension));

                    let result = if encrypt {
                        let identity = identity.unwrap_or(Workspace::identity_path()?);
                        workspace.create_encrypted_backup_with_identity(&temporary, identity)
                    } else {
                        workspace.create_backup(&temporary)
                    };
                    let result = match result {
                        Ok((path, manifest)) => {
                            let filename = path
                                .file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("backup").to_owned();
                            let upload = workspace.storage_upload_file(storage_name, &path);
                            let _ = std::fs::remove_file(&path);
                            upload.map(|destination| (destination, filename, manifest))
                        }
                        Err(error) => {
                            let _ = std::fs::remove_file(&temporary);
                            Err(error)
                        }
                    }?;
                    println!("uploaded {} -> {} ({} files, {} bytes){}",
                        result.1, result.0, result.2.files.len(), result.2.stats().bytes,
                        if encrypt { " [encrypted]" } else { "" });
                }
                "list" => {
                    let storage_name = values.first().ok_or_else(|| nero_core::NeroError::Message("storage remote name is required".into()))?;
                    let workspace = Workspace::discover(env::current_dir()?)?;
                    let backups = workspace.storage_list_backups(storage_name)?;
                    if backups.is_empty() {
                        println!("No remote backups.");
                    } else {
                        for backup in backups {
                            println!("{}{}", backup.name, if backup.encrypted { "  [encrypted]" } else { "" });
                        }
                    }
                }
                "pull" => {
                    let (identity, positional) = parse_identity_flag(values)?;
                    let storage_name = positional.first().ok_or_else(|| nero_core::NeroError::Message("storage remote name is required".into()))?;
                    let backup_name = positional.get(1).ok_or_else(|| nero_core::NeroError::Message("remote backup filename is required".into()))?;
                    let destination = positional.get(2).ok_or_else(|| nero_core::NeroError::Message("restore destination is required".into()))?;
                    let workspace = Workspace::discover(env::current_dir()?)?;
                    let temporary = env::temp_dir().join(format!("nero-download-{}-{}", std::process::id(), backup_name));
                    let downloaded = workspace.storage_download_file(storage_name, backup_name, &temporary)?;
                    let encrypted = downloaded.extension().and_then(|s| s.to_str()) == Some("age");
                    let result = if encrypted {
                        workspace.restore_encrypted_backup(
                            &downloaded,
                            destination,
                            identity.unwrap_or(Workspace::identity_path()?),
                        )
                    } else {
                        workspace.restore_backup(&downloaded, destination)
                    };
                    let _ = std::fs::remove_file(&downloaded);
                    let stats = result?;
                    println!("remote backup restored: {} files, {} bytes{}", stats.files, stats.bytes, if encrypted { " [decrypted]" } else { "" });
                }
                "recovery-test" | "test" => {
                    let (identity, positional) = parse_identity_flag(values)?;
                    let archive = positional.first().ok_or_else(|| nero_core::NeroError::Message("backup archive path is required".into()))?;
                    let stats = Workspace::recovery_test_backup(archive, identity)?;
                    println!("backup recovery test passed: {} files, {} bytes", stats.files, stats.bytes);
                }
                _ => {
                    println!("backup commands: create [--encrypt] [--identity path] [path], inspect <zip>, verify <zip|age> [--identity path], restore <zip|age> <empty-dir> [--identity path], recovery-test <zip|age> [--identity path], push <storage> [--encrypt] [--identity path], list <storage>, pull <storage> <backup-name> <empty-dir> [--identity path]");
                }
            }
        }
        "key" => {
            match args.next().as_deref() {
                Some("generate") => {
                    let force = args.any(|arg| arg == "--force");
                    let key = Workspace::generate_backup_key(force)?;
                    println!("backup identity: {}", key.identity_path.display());
                    println!("recipient: {}", key.recipient);
                    println!("keep the identity file private; the recipient is safe to share");
                }
                Some("show") => {
                    match Workspace::backup_key_info()? {
                        Some(key) => {
                            println!("identity: {}", key.identity_path.display());
                            println!("recipient: {}", key.recipient);
                        }
                        None => println!("no Nero backup identity exists; run `nero key generate`"),
                    }
                }
                Some("path") => println!("{}", Workspace::identity_path()?.display()),
                _ => println!("key commands: generate [--force], show, path"),
            }
        }
        "storage" | "remote" => {
            let subcommand = args.next().unwrap_or_else(|| "help".into());
            let workspace = Workspace::discover(env::current_dir()?)?;
            match subcommand.as_str() {
                "add" => {
                    let values = args.collect::<Vec<_>>();
                    let (encrypt, positional) = parse_storage_flags(values)?;
                    let name = positional.first().ok_or_else(|| nero_core::NeroError::Message("storage name is required".into()))?;
                    let target = positional.get(1).ok_or_else(|| nero_core::NeroError::Message("rclone target is required".into()))?;
                    let remote = workspace.storage_add(name, target, encrypt)?;
                    println!("added storage `{}` -> {}{}", remote.name, remote.target, if remote.encrypt_backups { " [encrypted backups]" } else { "" });
                }
                "list" => {
                    let remotes = workspace.storage_remotes()?;
                    if remotes.is_empty() {
                        println!("No storage remotes configured.");
                    } else {
                        for remote in remotes {
                            println!("{:16} {}{}", remote.name, remote.target, if remote.encrypt_backups { "  [encrypted]" } else { "" });
                        }
                    }
                }
                "test" => {
                    let name = args.next().ok_or_else(|| nero_core::NeroError::Message("storage name is required".into()))?;
                    let output = workspace.storage_test(&name)?;
                    println!("storage `{name}` is reachable");
                    if !output.trim().is_empty() { print!("{output}"); }
                }
                "remove" => {
                    let name = args.next().ok_or_else(|| nero_core::NeroError::Message("storage name is required".into()))?;
                    let remote = workspace.storage_remove(&name)?;
                    println!("removed storage `{}` ({})", remote.name, remote.target);
                }
                _ => println!("storage commands: add <name> <rclone-target> [--encrypt], list, test <name>, remove <name>"),
            }
        }
        "git" => {
            let subcommand = args.next().unwrap_or_else(|| "help".into());
            let workspace = Workspace::discover(env::current_dir()?)?;
            match subcommand.as_str() {
                "init" => print!("{}", workspace.git_init()?),
                "status" => {
                    let status = workspace.git_status()?;
                    println!("branch: {}", if status.branch.is_empty() { "(unborn / detached)" } else { &status.branch });
                    print!("{}", status.porcelain);
                }
                "snapshot" => {
                    let message = join_args(args)?;
                    print!("{}", workspace.git_snapshot(&message)?);
                }
                "remote" => {
                    match args.next().as_deref() {
                        Some("add") => {
                            let name = args.next().ok_or_else(|| nero_core::NeroError::Message("remote name is required".into()))?;
                            let url = args.next().ok_or_else(|| nero_core::NeroError::Message("remote URL is required".into()))?;
                            print!("{}", workspace.git_add_remote(&name, &url)?);
                        }
                        _ => print!("{}", workspace.git_remotes()?),
                    }
                }
                "push" => {
                    let remote = args.next().unwrap_or_else(|| "origin".into());
                    let branch = args.next();
                    print!("{}", workspace.git_push(&remote, branch.as_deref())?);
                }
                "pull" => {
                    let remote = args.next().unwrap_or_else(|| "origin".into());
                    let branch = args.next();
                    print!("{}", workspace.git_pull(&remote, branch.as_deref())?);
                }
                _ => println!("git commands: init, status, snapshot <message>, remote [add <name> <url>], push [remote] [branch], pull [remote] [branch]"),
            }
        }
        "doctor" => {
            let workspace = Workspace::discover(env::current_dir()?)?;
            for line in workspace.doctor()? { println!("{line}"); }
        }
        "gui" => {
            let workspace = Workspace::discover(env::current_dir()?)?;
            let status = ProcessCommand::new("nero-gui")
                .env("NERO_WORKSPACE", workspace.root())
                .status()
                .map_err(|_| nero_core::NeroError::Message(
                    "`nero-gui` is not installed or not on PATH; see gui/README.md".into(),
                ))?;
            if !status.success() { exit(status.code().unwrap_or(1)); }
        }
        "tui" => {
            let workspace = Workspace::discover(env::current_dir()?)?;
            nero_tui::run(workspace).map_err(nero_core::NeroError::Io)?;
        }
        other => return Err(nero_core::NeroError::Message(format!("unknown command `{other}` — run `nero help`"))),
    }

    io::stdout().flush().ok();
    Ok(())
}

fn parse_backup_flags(values: Vec<String>) -> Result<(bool, Option<PathBuf>, Vec<String>)> {
    let mut encrypt = false;
    let mut identity = None;
    let mut positional = Vec::new();
    let mut iter = values.into_iter();
    while let Some(value) = iter.next() {
        match value.as_str() {
            "--encrypt" => encrypt = true,
            "--identity" => {
                let path = iter.next().ok_or_else(|| nero_core::NeroError::Message("--identity requires a path".into()))?;
                identity = Some(PathBuf::from(path));
            }
            _ if value.starts_with("--identity=") => identity = Some(PathBuf::from(value[11..].to_owned())),
            _ => positional.push(value),
        }
    }
    Ok((encrypt, identity, positional))
}

fn parse_identity_flag(values: Vec<String>) -> Result<(Option<PathBuf>, Vec<String>)> {
    let mut identity = None;
    let mut positional = Vec::new();
    let mut iter = values.into_iter();
    while let Some(value) = iter.next() {
        match value.as_str() {
            "--identity" => {
                let path = iter.next().ok_or_else(|| nero_core::NeroError::Message("--identity requires a path".into()))?;
                identity = Some(PathBuf::from(path));
            }
            _ if value.starts_with("--identity=") => identity = Some(PathBuf::from(value[11..].to_owned())),
            _ => positional.push(value),
        }
    }
    Ok((identity, positional))
}

fn parse_backup_push_flags(values: Vec<String>) -> Result<(bool, Option<PathBuf>, Vec<String>)> {
    let mut encrypt = false;
    let mut identity = None;
    let mut positional = Vec::new();
    let mut iter = values.into_iter();
    while let Some(value) = iter.next() {
        match value.as_str() {
            "--encrypt" => encrypt = true,
            "--identity" => {
                let path = iter.next().ok_or_else(|| nero_core::NeroError::Message("--identity requires a path".into()))?;
                identity = Some(PathBuf::from(path));
            }
            _ if value.starts_with("--identity=") => identity = Some(PathBuf::from(value[11..].to_owned())),
            _ => positional.push(value),
        }
    }
    Ok((encrypt, identity, positional))
}

fn parse_storage_flags(values: Vec<String>) -> Result<(bool, Vec<String>)> {
    let mut encrypt = false;
    let mut positional = Vec::new();
    for value in values {
        match value.as_str() {
            "--encrypt" => encrypt = true,
            _ => positional.push(value),
        }
    }
    Ok((encrypt, positional))
}

fn backup_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

fn print_help() {
    println!(r#"Nero — a small place for your thoughts.

USAGE
  nero <command> [arguments]

COMMANDS
  init [path]             Create a workspace
  new <title>             Create a Markdown note
  list                    List notes
  open <note>             Print a note
  edit <note>             Open a note in $EDITOR
  find <query>            Search notes
  backlinks <note>        Find notes linking to a note
  today                   Create/open today's daily note
  render <note>            Render a note to HTML
  reindex                 Rebuild/update the search index
  watch                   Watch Markdown files and refresh the index
  doctor                  Check workspace health
  backup create [path]    Create a portable local backup
  backup create --encrypt [path]
                          Create an age-encrypted backup
  backup inspect <file>   Inspect an unencrypted Nero backup manifest
  backup verify <file>    Verify a ZIP or decrypt+verify an age backup
  backup restore <file> <dir>
                          Restore a ZIP or age backup into an empty directory
  backup recovery-test <file>
                          Decrypt/restore to a temp dir and verify recovery
  backup push <storage>   Create and upload a backup through rclone
  backup list <storage>  List remote backup snapshots
  backup pull <storage> <file> <dir>
                          Download and restore a remote backup
  storage add <name> <target> [--encrypt]
                          Register an rclone target; optionally require encrypted backups
  storage list            List configured storage targets
  storage test <name>     Check an rclone target
  storage remove <name>  Remove a storage target from Nero
  key generate [--force] Generate Nero's age backup identity
  key show                Show identity location and public recipient
  key path                Print the identity path
  git init                Initialize Git versioning
  git status              Show Git status
  git snapshot <message>  Stage workspace and create a commit
  git remote [add ...]    Show or add a remote
  git push/pull [remote]  Push/pull using Git
  gui                     Run the desktop interface (requires `nero-gui`)
  tui                     Run the terminal interface
  help                    Show this help
  version                 Show the version

NOTES
  Notes are ordinary Markdown files.
  Internal links use [[Note Name]].
  Math supports $...$, $$...$$, \(...\), and \[...\] via Comrak.
"#);
}

fn print_notes(notes: Vec<NoteSummary>) {
    if notes.is_empty() { println!("No notes."); return; }
    for note in notes { println!("{:30} {}", note.title, note.path.display()); }
}

fn print_search(results: Vec<SearchResult>) {
    if results.is_empty() { println!("No matches."); return; }
    for result in results {
        println!("\n{}  ({})", result.note.title, result.note.path.display());
        println!("  {}", result.preview);
    }
}

fn join_args<I>(args: I) -> Result<String>
where I: IntoIterator<Item = String> {
    let joined = args.into_iter().collect::<Vec<_>>().join(" ");
    if joined.trim().is_empty() { Err(nero_core::NeroError::Message("an argument is required".into())) }
    else { Ok(joined) }
}
