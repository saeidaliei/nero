use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process::{Command as ProcessCommand, exit},
};

mod config;

use nero_core::{NoteSummary, Result, SearchResult, Workspace, is_workspace_root, nero_home_dir};

fn main() {
    if let Err(error) = run() {
        eprintln!("nero: {error}");
        exit(1);
    }
}

fn run() -> Result<()> {
    // Extract the global selector before parsing subcommands, so `--workspace`
    // works both before and after the command (for example `nero -w work tui`).
    let mut raw_args = env::args().skip(1).collect::<Vec<_>>();
    let workspace_override = extract_workspace_option(&mut raw_args)?;
    let mut args = raw_args.into_iter();
    let command = args.next().unwrap_or_else(|| "help".into());

    match command.as_str() {
        "help" | "--help" | "-h" => print_help(),
        "version" | "--version" => println!("nero {}", env!("CARGO_PKG_VERSION")),
        "home" | "app-home" => {
            let subcommand = args.next();
            let home = nero_home_dir()?;
            match subcommand.as_deref() {
                None | Some("show") => {
                    println!("Nero home:  {}", home.display());
                    println!("Config:     {}", config::config_path()?.display());
                    println!("Workspaces: {}", config::workspace_home()?.display());
                    println!("Keys:       {}", Workspace::config_dir()?.display());
                    println!("Backups:    {}", home.join("backups").display());
                    println!("Override with the NERO_HOME environment variable.");
                }
                Some("path") => println!("{}", home.display()),
                Some("help") | Some("--help") | Some("-h") => println!(
                    "home [show|path] — show Nero's application home and data directories; override with NERO_HOME"
                ),
                Some(other) => {
                    return Err(nero_core::NeroError::Message(format!(
                        "unknown home command `{other}`; use show or path"
                    )));
                }
            }
        }
        "config" => match args.next().as_deref() {
            None | Some("show") => print_config_show(workspace_override.as_deref())?,
            Some("path") => println!("{}", config::config_path()?.display()),
            Some("help") | Some("--help") | Some("-h") => {
                println!("config [show|path] — show resolved configuration and storage settings")
            }
            Some(other) => {
                return Err(nero_core::NeroError::Message(format!(
                    "unknown config command `{other}`; use `nero config show` or `nero config path`"
                )));
            }
        },
        "init" => {
            let root = args
                .next()
                .map(PathBuf::from)
                .or_else(|| workspace_override.clone().map(PathBuf::from))
                .unwrap_or(env::current_dir()?);
            let workspace = Workspace::init(root)?;
            println!(
                "initialized Nero workspace at {}",
                workspace.root().display()
            );
        }
        "workspace" | "workspaces" => {
            handle_workspace_command(args, workspace_override.as_deref())?
        }
        "new" => {
            let title = join_args(args)?;
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let note = workspace.create_note(&title)?;
            println!("created {}", note.path.display());
        }
        "delete" | "rm" => {
            let query = join_args(args)?;
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let note = workspace.delete_note(&query)?;
            println!("deleted {}", note.path.display());
        }
        "list" | "ls" => {
            let values = args.collect::<Vec<_>>();
            let long = values
                .iter()
                .any(|value| matches!(value.as_str(), "-l" | "--long"));
            if let Some(unexpected) = values
                .iter()
                .find(|value| !matches!(value.as_str(), "-l" | "--long"))
            {
                return Err(nero_core::NeroError::Message(format!(
                    "unknown list option `{unexpected}`; use `nero list [-l|--long]`"
                )));
            }
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let label = workspace_label(&workspace);
            print_note_listing(
                workspace.list_note_details()?,
                &label,
                workspace.root(),
                long,
            );
        }
        "open" => {
            let query = join_args(args)?;
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let note = workspace.read_note(&query)?;
            println!(
                "{}\n\n{}",
                note.summary.title,
                note.document.source.trim_end()
            );
        }
        "edit" => {
            let query = join_args(args)?;
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let status = workspace.edit(&query)?;
            if !status.success() {
                exit(status.code().unwrap_or(1));
            }
        }
        "find" | "search" => {
            let query = join_args(args)?;
            let workspace = selected_workspace(workspace_override.as_deref())?;
            print_search(workspace.search(&query)?);
        }
        "backlinks" => {
            let query = join_args(args)?;
            let workspace = selected_workspace(workspace_override.as_deref())?;
            print_notes(workspace.backlinks(&query)?);
        }
        "today" => {
            let workspace = selected_workspace(workspace_override.as_deref())?;
            // Keep the daily-note path as the first line for scripts, then show actionable due notes.
            println!("{}", workspace.today()?.path.display());
            let due_notes = workspace.due_notes()?;
            if !due_notes.is_empty() {
                println!("\nDue notes:");
                for item in due_notes {
                    let status = if item.overdue { "overdue" } else { "due today" };
                    println!(
                        "  [{status}] {} ({}, due {})",
                        item.summary.title,
                        item.summary.path.display(),
                        item.due_date
                    );
                }
            }
        }
        "render" => {
            let query = join_args(args)?;
            let workspace = selected_workspace(workspace_override.as_deref())?;
            print!("{}", workspace.render_note_html(&query)?);
        }
        "reindex" | "index" => {
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let stats = workspace.reindex()?;
            println!(
                "index refreshed: {} updated, {} removed",
                stats.indexed, stats.removed
            );
        }
        "watch" => {
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let (_watcher, events) = workspace.watch()?;
            println!("watching {} (Ctrl-C to stop)", workspace.root().display());
            for paths in events {
                for path in paths {
                    println!("changed {}", path.display());
                }
                let _ = workspace.reindex();
            }
        }
        "backup" => {
            let subcommand = args.next().unwrap_or_else(|| "help".into());
            let values = args.collect::<Vec<_>>();
            match subcommand.as_str() {
                "home" => {
                    let action = values.first().map(String::as_str).unwrap_or("help");
                    let positional = values.iter().skip(1).cloned().collect::<Vec<_>>();
                    match action {
                        "create" => {
                            if positional.len() > 1 {
                                return Err(nero_core::NeroError::Message(
                                    "usage: nero backup home create [archive.zip.age]".into(),
                                ));
                            }
                            let destination = if let Some(path) = positional.first() {
                                PathBuf::from(path)
                            } else {
                                nero_home_dir()?.join("backups").join(format!(
                                    "nero-home-backup-{}.zip.age",
                                    backup_timestamp()
                                ))
                            };
                            let passphrase = prompt_home_backup_passphrase(true)?;
                            let (path, manifest) = Workspace::create_home_backup(
                                nero_home_dir()?,
                                destination,
                                passphrase,
                            )?;
                            println!(
                                "created encrypted Nero home backup: {} ({} files, {} bytes before compression)",
                                path.display(),
                                manifest.files.len(),
                                manifest.stats().bytes
                            );
                            println!(
                                "keep the passphrase somewhere safe; it cannot be recovered by Nero"
                            );
                        }
                        "verify" => {
                            let archive = positional.first().ok_or_else(|| {
                                nero_core::NeroError::Message(
                                    "usage: nero backup home verify <archive.zip.age>".into(),
                                )
                            })?;
                            let passphrase = prompt_home_backup_passphrase(false)?;
                            let stats = Workspace::verify_home_backup(archive, passphrase)?;
                            println!(
                                "home backup verified: {} files, {} bytes before compression",
                                stats.files, stats.bytes
                            );
                        }
                        "restore" => {
                            let archive = positional.first().ok_or_else(|| nero_core::NeroError::Message("usage: nero backup home restore <archive.zip.age> [destination]".into()))?;
                            let destination = positional
                                .get(1)
                                .map(PathBuf::from)
                                .unwrap_or(nero_home_dir()?);
                            let passphrase = prompt_home_backup_passphrase(false)?;
                            let stats = Workspace::restore_home_backup(
                                archive,
                                destination.clone(),
                                passphrase,
                            )?;
                            println!(
                                "Nero home restored to {} ({} files, {} bytes before path rebasing)",
                                destination.display(),
                                stats.files,
                                stats.bytes
                            );
                            println!(
                                "run `nero home` and `nero workspace list` to check the restored profile"
                            );
                        }
                        "recovery-test" | "test" => {
                            let archive = positional.first().ok_or_else(|| {
                                nero_core::NeroError::Message(
                                    "usage: nero backup home recovery-test <archive.zip.age>"
                                        .into(),
                                )
                            })?;
                            let passphrase = prompt_home_backup_passphrase(false)?;
                            let stats = Workspace::recovery_test_home_backup(archive, passphrase)?;
                            println!(
                                "Nero home backup recovery test passed: {} files, {} bytes",
                                stats.files, stats.bytes
                            );
                        }
                        _ => println!(
                            "home backup commands: create [archive.zip.age], verify <archive.zip.age>, restore <archive.zip.age> [destination], recovery-test <archive.zip.age>"
                        ),
                    }
                }
                "create" => {
                    let workspace = selected_workspace(workspace_override.as_deref())?;
                    let (encrypt, identity, positional) = parse_backup_flags(values)?;
                    let default_extension = if encrypt { "age" } else { "zip" };
                    let destination = if let Some(path) = positional.first() {
                        PathBuf::from(path)
                    } else {
                        nero_home_dir()?.join("backups").join(format!(
                            "nero-backup-{}.{}",
                            backup_timestamp(),
                            default_extension
                        ))
                    };
                    let (path, manifest) = if encrypt {
                        let identity = identity.unwrap_or(Workspace::identity_path()?);
                        workspace.create_encrypted_backup_with_identity(destination, identity)?
                    } else {
                        workspace.create_backup(destination)?
                    };
                    println!(
                        "created {} ({} files, {} bytes){}",
                        path.display(),
                        manifest.files.len(),
                        manifest.stats().bytes,
                        if encrypt { " [encrypted]" } else { "" }
                    );
                }
                "inspect" => {
                    let archive = values.first().ok_or_else(|| {
                        nero_core::NeroError::Message("backup archive path is required".into())
                    })?;
                    let manifest = Workspace::inspect_backup(archive)?;
                    println!(
                        "Nero backup v{} — {} files, {} bytes",
                        manifest.version,
                        manifest.files.len(),
                        manifest.stats().bytes
                    );
                    for entry in manifest.files {
                        println!("{:>10}  {}  {}", entry.size, entry.sha256, entry.path);
                    }
                }
                "verify" => {
                    let (identity, positional) = parse_identity_flag(values)?;
                    let archive = positional.first().ok_or_else(|| {
                        nero_core::NeroError::Message("backup archive path is required".into())
                    })?;
                    let is_encrypted = Workspace::backup_is_encrypted(archive)?;
                    let stats = if is_encrypted {
                        Workspace::verify_encrypted_backup(
                            archive,
                            identity.unwrap_or(Workspace::identity_path()?),
                        )?
                    } else {
                        Workspace::verify_backup(archive)?
                    };
                    println!(
                        "backup verified: {} files, {} bytes{}",
                        stats.files,
                        stats.bytes,
                        if is_encrypted {
                            " [decrypted + verified]"
                        } else {
                            ""
                        }
                    );
                }
                "restore" => {
                    let (identity, positional) = parse_identity_flag(values)?;
                    let archive = positional.first().ok_or_else(|| {
                        nero_core::NeroError::Message("backup archive path is required".into())
                    })?;
                    let destination = positional.get(1).ok_or_else(|| {
                        nero_core::NeroError::Message("restore destination is required".into())
                    })?;
                    let is_encrypted = Workspace::backup_is_encrypted(archive)?;
                    let stats = if is_encrypted {
                        Workspace::restore_encrypted_backup(
                            archive,
                            destination,
                            identity.unwrap_or(Workspace::identity_path()?),
                        )?
                    } else {
                        Workspace::restore_backup(archive, destination)?
                    };
                    println!(
                        "backup restored: {} files, {} bytes{}",
                        stats.files,
                        stats.bytes,
                        if is_encrypted { " [decrypted]" } else { "" }
                    );
                }
                "push" => {
                    let (force_encrypt, identity, positional) = parse_backup_push_flags(values)?;
                    let storage_name = positional.first().ok_or_else(|| {
                        nero_core::NeroError::Message("storage remote name is required".into())
                    })?;
                    let workspace = selected_workspace(workspace_override.as_deref())?;
                    let remote = workspace.storage_get(storage_name)?;
                    let encrypt = remote.encrypt_backups || force_encrypt;
                    let extension = if encrypt { "age" } else { "zip" };
                    let temporary = env::temp_dir().join(format!(
                        "nero-upload-{}-{}.{}",
                        std::process::id(),
                        backup_timestamp(),
                        extension
                    ));

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
                                .unwrap_or("backup")
                                .to_owned();
                            let upload = workspace.storage_upload_file(storage_name, &path);
                            let _ = std::fs::remove_file(&path);
                            upload.map(|destination| (destination, filename, manifest))
                        }
                        Err(error) => {
                            let _ = std::fs::remove_file(&temporary);
                            Err(error)
                        }
                    }?;
                    println!(
                        "uploaded {} -> {} ({} files, {} bytes){}",
                        result.1,
                        result.0,
                        result.2.files.len(),
                        result.2.stats().bytes,
                        if encrypt { " [encrypted]" } else { "" }
                    );
                }
                "list" => {
                    let storage_name = values.first().ok_or_else(|| {
                        nero_core::NeroError::Message("storage remote name is required".into())
                    })?;
                    let workspace = selected_workspace(workspace_override.as_deref())?;
                    let backups = workspace.storage_list_backups(storage_name)?;
                    if backups.is_empty() {
                        println!("No remote backups.");
                    } else {
                        for backup in backups {
                            println!(
                                "{}{}",
                                backup.name,
                                if backup.encrypted {
                                    "  [encrypted]"
                                } else {
                                    ""
                                }
                            );
                        }
                    }
                }
                "pull" => {
                    let (identity, positional) = parse_identity_flag(values)?;
                    let storage_name = positional.first().ok_or_else(|| {
                        nero_core::NeroError::Message("storage remote name is required".into())
                    })?;
                    let backup_name = positional.get(1).ok_or_else(|| {
                        nero_core::NeroError::Message("remote backup filename is required".into())
                    })?;
                    let destination = positional.get(2).ok_or_else(|| {
                        nero_core::NeroError::Message("restore destination is required".into())
                    })?;
                    let workspace = selected_workspace(workspace_override.as_deref())?;
                    let temporary = env::temp_dir().join(format!(
                        "nero-download-{}-{}",
                        std::process::id(),
                        backup_name
                    ));
                    let downloaded =
                        workspace.storage_download_file(storage_name, backup_name, &temporary)?;
                    let encrypted = Workspace::backup_is_encrypted(&downloaded)?;
                    let result = if encrypted {
                        Workspace::restore_encrypted_backup(
                            &downloaded,
                            destination,
                            identity.unwrap_or(Workspace::identity_path()?),
                        )
                    } else {
                        Workspace::restore_backup(&downloaded, destination)
                    };
                    let _ = std::fs::remove_file(&downloaded);
                    let stats = result?;
                    println!(
                        "remote backup restored: {} files, {} bytes{}",
                        stats.files,
                        stats.bytes,
                        if encrypted { " [decrypted]" } else { "" }
                    );
                }
                "recovery-test" | "test" => {
                    let (identity, positional) = parse_identity_flag(values)?;
                    let archive = positional.first().ok_or_else(|| {
                        nero_core::NeroError::Message("backup archive path is required".into())
                    })?;
                    let stats = Workspace::recovery_test_backup(archive, identity)?;
                    println!(
                        "backup recovery test passed: {} files, {} bytes",
                        stats.files, stats.bytes
                    );
                }
                _ => {
                    println!(
                        "backup commands: create [--encrypt] [--identity path] [path], inspect <zip>, verify <zip|age> [--identity path], restore <zip|age> <empty-dir> [--identity path], recovery-test <zip|age> [--identity path], push <storage> [--encrypt] [--identity path], list <storage>, pull <storage> <backup-name> <empty-dir> [--identity path]"
                    );
                }
            }
        }
        "key" => match args.next().as_deref() {
            Some("generate") => {
                let force = args.any(|arg| arg == "--force");
                if force {
                    eprintln!(
                        "WARNING: replacing the Nero backup identity makes existing encrypted backups unrecoverable with the new identity."
                    );
                }
                let key = Workspace::generate_backup_key(force)?;
                println!("backup identity: {}", key.identity_path.display());
                println!("recipient: {}", key.recipient);
                println!("keep the identity file private; the recipient is safe to share");
            }
            Some("show") => match Workspace::backup_key_info()? {
                Some(key) => {
                    println!("identity: {}", key.identity_path.display());
                    println!("recipient: {}", key.recipient);
                }
                None => println!("no Nero backup identity exists; run `nero key generate`"),
            },
            Some("path") => println!("{}", Workspace::identity_path()?.display()),
            _ => println!("key commands: generate [--force], show, path"),
        },
        "storage" | "remote" => {
            let subcommand = args.next().unwrap_or_else(|| "help".into());
            let workspace = selected_workspace(workspace_override.as_deref())?;
            match subcommand.as_str() {
                "add" => {
                    let values = args.collect::<Vec<_>>();
                    let (encrypt, positional) = parse_storage_flags(values)?;
                    let name = positional.first().ok_or_else(|| {
                        nero_core::NeroError::Message("storage name is required".into())
                    })?;
                    let target = positional.get(1).ok_or_else(|| {
                        nero_core::NeroError::Message("rclone target is required".into())
                    })?;
                    let remote = workspace.storage_add(name, target, encrypt)?;
                    println!(
                        "added storage `{}` -> {}{}",
                        remote.name,
                        remote.target,
                        if remote.encrypt_backups {
                            " [encrypted backups]"
                        } else {
                            ""
                        }
                    );
                }
                "list" => {
                    let remotes = workspace.storage_remotes()?;
                    if remotes.is_empty() {
                        println!("No storage remotes configured.");
                    } else {
                        for remote in remotes {
                            println!(
                                "{:16} {}{}",
                                remote.name,
                                remote.target,
                                if remote.encrypt_backups {
                                    "  [encrypted]"
                                } else {
                                    ""
                                }
                            );
                        }
                    }
                }
                "test" => {
                    let name = args.next().ok_or_else(|| {
                        nero_core::NeroError::Message("storage name is required".into())
                    })?;
                    let output = workspace.storage_test(&name)?;
                    println!("storage `{name}` is reachable");
                    if !output.trim().is_empty() {
                        print!("{output}");
                    }
                }
                "remove" => {
                    let name = args.next().ok_or_else(|| {
                        nero_core::NeroError::Message("storage name is required".into())
                    })?;
                    let remote = workspace.storage_remove(&name)?;
                    println!("removed storage `{}` ({})", remote.name, remote.target);
                }
                _ => println!(
                    "storage commands: add <name> <rclone-target> [--encrypt], list, test <name>, remove <name>"
                ),
            }
        }
        "git" => {
            let subcommand = args.next().unwrap_or_else(|| "help".into());
            let workspace = selected_workspace(workspace_override.as_deref())?;
            match subcommand.as_str() {
                "init" => print!("{}", workspace.git_init()?),
                "status" => {
                    let status = workspace.git_status()?;
                    println!(
                        "branch: {}",
                        if status.branch.is_empty() {
                            "(unborn / detached)"
                        } else {
                            &status.branch
                        }
                    );
                    print!("{}", status.porcelain);
                }
                "snapshot" => {
                    let message = join_args(args)?;
                    print!("{}", workspace.git_snapshot(&message)?);
                }
                "remote" => match args.next().as_deref() {
                    Some("add") => {
                        let name = args.next().ok_or_else(|| {
                            nero_core::NeroError::Message("remote name is required".into())
                        })?;
                        let url = args.next().ok_or_else(|| {
                            nero_core::NeroError::Message("remote URL is required".into())
                        })?;
                        print!("{}", workspace.git_add_remote(&name, &url)?);
                    }
                    _ => print!("{}", workspace.git_remotes()?),
                },
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
                _ => println!(
                    "git commands: init, status, snapshot <message>, remote [add <name> <url>], push [remote] [branch], pull [remote] [branch]"
                ),
            }
        }
        "doctor" => {
            let workspace = selected_workspace(workspace_override.as_deref())?;
            for line in workspace.doctor()? {
                println!("{line}");
            }
        }
        "gui" => {
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let status = ProcessCommand::new("nero-gui")
                .env("NERO_WORKSPACE", workspace.root())
                .status()
                .map_err(|_| {
                    nero_core::NeroError::Message(
                        "`nero-gui` is not installed or not on PATH; see gui/README.md".into(),
                    )
                })?;
            if !status.success() {
                exit(status.code().unwrap_or(1));
            }
        }
        "tui" => {
            let workspace = selected_workspace(workspace_override.as_deref())?;
            nero_tui::run(workspace).map_err(nero_core::NeroError::Io)?;
        }
        other => {
            return Err(nero_core::NeroError::Message(format!(
                "unknown command `{other}` — run `nero help`"
            )));
        }
    }

    io::stdout().flush().ok();
    Ok(())
}

/// Remove the global workspace selector without forcing every subcommand to
/// reimplement option parsing. The selector may appear before or after a command.
fn extract_workspace_option(args: &mut Vec<String>) -> Result<Option<String>> {
    let mut selected = None;
    let mut kept = Vec::with_capacity(args.len());
    let mut index = 0;
    while index < args.len() {
        let current = &args[index];
        if current == "--workspace" || current == "-w" {
            let value = args.get(index + 1).ok_or_else(|| {
                nero_core::NeroError::Message(format!(
                    "{current} requires a path or registered workspace name"
                ))
            })?;
            if selected.is_some() {
                return Err(nero_core::NeroError::Message(
                    "specify --workspace only once".into(),
                ));
            }
            selected = Some(value.clone());
            index += 2;
        } else if let Some(value) = current.strip_prefix("--workspace=") {
            if value.trim().is_empty() {
                return Err(nero_core::NeroError::Message(
                    "--workspace requires a path or registered workspace name".into(),
                ));
            }
            if selected.replace(value.to_owned()).is_some() {
                return Err(nero_core::NeroError::Message(
                    "specify --workspace only once".into(),
                ));
            }
            index += 1;
        } else {
            kept.push(current.clone());
            index += 1;
        }
    }
    *args = kept;
    Ok(selected)
}

/// Workspace selection precedence: explicit CLI selector, environment override,
/// a workspace discovered from the current directory/its ancestors, then the saved default.
/// This lets a local workspace take precedence while the saved default handles arbitrary folders.
fn selected_workspace(explicit: Option<&str>) -> Result<Workspace> {
    if let Some(target) = explicit {
        return open_selected_workspace(target);
    }
    if let Some(target) = env::var_os("NERO_WORKSPACE") {
        let target = target.to_string_lossy();
        return open_selected_workspace(&target);
    }

    let cwd = env::current_dir()?;
    for candidate in cwd.ancestors() {
        if is_workspace_root(candidate) {
            return open_workspace(candidate.to_path_buf());
        }
    }
    if let Some(path) = config::default_workspace()? {
        return open_workspace(path);
    }
    Workspace::discover(cwd)
}

fn open_selected_workspace(target: &str) -> Result<Workspace> {
    open_workspace(config::resolve_target(target)?)
}

fn open_workspace(path: PathBuf) -> Result<Workspace> {
    let workspace = Workspace::open(path)?;
    if !is_workspace_root(workspace.root()) {
        return Err(nero_core::NeroError::Message(format!(
            "{} is not an initialized Nero workspace; run `nero init {}` first",
            workspace.root().display(),
            workspace.root().display()
        )));
    }
    Ok(workspace)
}

fn handle_workspace_command(
    mut args: impl Iterator<Item = String>,
    override_workspace: Option<&str>,
) -> Result<()> {
    let subcommand = args.next().unwrap_or_else(|| "show".to_owned());
    match subcommand.as_str() {
        "home" => match args.next().as_deref() {
            None | Some("show") => {
                println!("workspace home: {}", config::workspace_home()?.display())
            }
            Some("set") => {
                let path = args.next().ok_or_else(|| {
                    nero_core::NeroError::Message(
                        "usage: nero workspace home set <parent-directory>".into(),
                    )
                })?;
                let path = config::set_workspace_home(PathBuf::from(path))?;
                println!("workspace home: {}", path.display());
            }
            Some("reset") => println!(
                "workspace home: {}",
                config::reset_workspace_home()?.display()
            ),
            Some(other) => {
                return Err(nero_core::NeroError::Message(format!(
                    "unknown workspace home command `{other}`; use show, set, or reset"
                )));
            }
        },
        "create" => {
            let name = args.next().ok_or_else(|| {
                nero_core::NeroError::Message("usage: nero workspace create <name>".into())
            })?;
            let path = config::create_workspace(&name)?;
            println!("created workspace `{name}` at {}", path.display());
            let view = config::view()?;
            if view.default_workspace_name.as_deref() == Some(name.as_str()) {
                println!("selected as the default workspace");
            }
        }
        "set" | "default" => {
            let path = args.next().ok_or_else(|| {
                nero_core::NeroError::Message(
                    "usage: nero workspace set <initialized-workspace-path>".into(),
                )
            })?;
            let path = config::set_default_path(PathBuf::from(path))?;
            println!("default workspace: {}", path.display());
            println!("commands now work from any directory; use `-w` to override for one command");
        }
        "add" => {
            let name = args.next().ok_or_else(|| {
                nero_core::NeroError::Message("usage: nero workspace add <name> <path>".into())
            })?;
            let path = args.next().ok_or_else(|| {
                nero_core::NeroError::Message("usage: nero workspace add <name> <path>".into())
            })?;
            let path = config::add_workspace(&name, PathBuf::from(path))?;
            println!("registered workspace `{name}` -> {}", path.display());
        }
        "use" | "switch" => {
            let name = args.next().ok_or_else(|| {
                nero_core::NeroError::Message("usage: nero workspace use <name>".into())
            })?;
            let path = config::use_workspace(&name)?;
            println!("default workspace: {name} -> {}", path.display());
        }
        "list" | "ls" => {
            let view = config::view()?;
            println!("workspace home: {}", view.workspace_home_path.display());
            if view.workspaces.is_empty() && view.default_workspace_path.is_none() {
                println!(
                    "No workspaces configured yet. Use `nero workspace create personal` or `nero workspace add <name> <path>`."
                );
                return Ok(());
            }
            let default_named = view.default_workspace_name.as_deref();
            for (name, path) in &view.workspaces {
                println!(
                    "{} {:16} {}",
                    if default_named == Some(name.as_str()) {
                        "*"
                    } else {
                        " "
                    },
                    name,
                    path.display()
                );
            }
            if let Some(path) = view.default_workspace_path {
                println!("* {:16} {}", "default", path.display());
            }
        }
        "show" | "current" => {
            let workspace = selected_workspace(override_workspace)?;
            println!("workspace: {}", workspace.root().display());
            println!("config:    {}", config::config_path()?.display());
        }
        "remove" | "rm" => {
            let name = args.next().ok_or_else(|| {
                nero_core::NeroError::Message("usage: nero workspace remove <name>".into())
            })?;
            let path = config::remove_workspace(&name)?;
            println!("removed workspace `{name}` ({})", path.display());
        }
        "clear" | "unset" => {
            config::clear_default()?;
            println!(
                "default workspace cleared; Nero will use NERO_WORKSPACE or discover from the current directory"
            );
        }
        "help" | "--help" | "-h" => {
            println!(
                "workspace commands:\n  home [set <path>|reset] show/set the parent directory for workspaces\n  create <name>    create a workspace under the workspace home\n  set <path>       set the default to an initialized workspace path\n  add <name> <path> register an existing workspace\n  use <name>       make a named workspace the default\n  list             list registered workspaces\n  show             print the workspace selected for this command\n  remove <name>    remove a registered workspace\n  clear            clear the saved default"
            )
        }
        other => {
            return Err(nero_core::NeroError::Message(format!(
                "unknown workspace command `{other}`; run `nero workspace help`"
            )));
        }
    }
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
                let path = iter.next().ok_or_else(|| {
                    nero_core::NeroError::Message("--identity requires a path".into())
                })?;
                identity = Some(PathBuf::from(path));
            }
            _ if value.starts_with("--identity=") => {
                identity = Some(PathBuf::from(value[11..].to_owned()))
            }
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
                let path = iter.next().ok_or_else(|| {
                    nero_core::NeroError::Message("--identity requires a path".into())
                })?;
                identity = Some(PathBuf::from(path));
            }
            _ if value.starts_with("--identity=") => {
                identity = Some(PathBuf::from(value[11..].to_owned()))
            }
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
                let path = iter.next().ok_or_else(|| {
                    nero_core::NeroError::Message("--identity requires a path".into())
                })?;
                identity = Some(PathBuf::from(path));
            }
            _ if value.starts_with("--identity=") => {
                identity = Some(PathBuf::from(value[11..].to_owned()))
            }
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

fn prompt_home_backup_passphrase(confirm: bool) -> Result<String> {
    let passphrase = rpassword::prompt_password("Home backup passphrase: ").map_err(|error| {
        nero_core::NeroError::Message(format!("could not read passphrase: {error}"))
    })?;
    if passphrase.is_empty() {
        return Err(nero_core::NeroError::Message(
            "passphrase cannot be empty".into(),
        ));
    }
    if confirm {
        if passphrase.chars().count() < 12 {
            return Err(nero_core::NeroError::Message(
                "use a home-backup passphrase of at least 12 characters".into(),
            ));
        }
        let confirmation = rpassword::prompt_password("Confirm passphrase: ").map_err(|error| {
            nero_core::NeroError::Message(format!(
                "could not read passphrase confirmation: {error}"
            ))
        })?;
        if passphrase != confirmation {
            return Err(nero_core::NeroError::Message(
                "passphrases do not match; backup was not created".into(),
            ));
        }
    }
    Ok(passphrase)
}

fn backup_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".into())
}

fn print_help() {
    println!(
        r#"Nero — Simple but powerful.
Markdown notes for the CLI, terminal, and desktop.

USAGE
  nero [-w PATH|NAME] <command> [arguments]
  nero [--workspace PATH|NAME] <command> [arguments]

NOTES
  init [path]               Create a workspace
  new <title>               Create a Markdown note with title/date frontmatter
  list [-l|--long]          List notes with IDs and timestamps; -l includes frontmatter
  open <note|ID>            Print a note (an integer ID or #ID also works)
  edit <note|ID>            Open a note in $EDITOR
  delete <note|ID>          Permanently delete a note by title, path, or ID (alias: rm)
  find <query>              Search note content
  backlinks <note|ID>       Find notes linking to a note
  today                     Open today's daily note and list due/overdue notes
  render <note|ID>          Render a note to HTML
  reindex                   Refresh the search index
  watch                     Watch Markdown files and refresh the index
  doctor                    Check workspace health

WORKSPACES
  workspace home [show|set <path>|reset]
                              Show or change the workspaces directory
  workspace create <name>     Create and register a workspace
  workspace set <path>        Set the default to an initialized workspace
  workspace add <name> <path>
                              Register an existing workspace
  workspace use <name>        Make a named workspace the default
  workspace list            List registered workspaces
  workspace show            Show the selected workspace
  workspace remove <name>   Unregister a workspace (alias: rm)
  workspace clear           Clear the saved default

APPLICATION HOME & CONFIGURATION
  home                      Show Nero's application home and data paths
  home path                 Print only the application home path
  config show               Show resolved config, workspace, keys, remotes, and cloud setup
  config path               Print the user configuration file path

BACKUPS
  backup create [path]      Create a portable local backup
  backup create --encrypt [path]
                              Create an age-encrypted workspace backup
  backup verify <file>      Verify a ZIP or decrypt and verify an age backup
  backup inspect <file>     Inspect an unencrypted backup manifest
  backup restore <file> <dir>
                              Restore a backup into an empty directory
  backup recovery-test <file>
                              Test recovery in a temporary directory
  backup push <storage>     Create and upload a backup through rclone
  backup list <storage>     List remote backup snapshots
  backup pull <storage> <file> <dir>
                              Download and restore a remote backup
  backup home create [path.zip.age]
                              Create an encrypted full-home backup
  backup home verify <file> Verify a full-home backup
  backup home restore <file> [destination]
                              Restore a full home on this/new machine
  backup home recovery-test <file>
                              Test full-home recovery without replacing files

STORAGE & KEYS
  storage add <name> <target> [--encrypt]
                              Register an rclone target
  storage list              List configured storage targets
  storage test <name>       Check a storage target
  storage remove <name>     Remove a storage target from Nero
  key generate [--force]    Generate Nero's age backup identity
  key show                  Show identity location and public recipient
  key path                  Print the identity path

GIT & INTERFACES
  git init                  Initialize Git versioning
  git status                Show Git status
  git snapshot <message>    Stage workspace changes and create a commit
  git remote [add ...]      Show or add a Git remote
  git push [remote]         Push commits using Git
  git pull [remote]         Pull commits using Git
  gui                       Run the desktop interface (requires nero-gui)
  tui                       Run the terminal interface
  help                      Show this help
  version                   Show the Nero version

WORKSPACE SELECTION
  -w PATH|NAME, --workspace PATH|NAME overrides the selected workspace for one command.
  Otherwise Nero uses NERO_WORKSPACE, directory discovery, then the saved default.
  Named workspaces live under $NERO_HOME/workspaces (default ~/.nero/workspaces).
  Set NERO_HOME to an absolute path to relocate Nero's application home and keys.

MARKDOWN
  Notes are ordinary Markdown files; new notes include title/date frontmatter.
  `nero list` assigns persistent integer IDs; use `nero edit 3`, `nero delete 3`, or `nero open '#3'`.
  `nero list -l` shows additional frontmatter fields. IDs are workspace-local and don't change on reindex.
  Internal links use [[Note Name]].
  Math supports $...$, $$...$$, \(...\), and \[...\] via Comrak.
"#
    );
}

fn print_notes(notes: Vec<NoteSummary>) {
    if notes.is_empty() {
        println!("No notes.");
        return;
    }
    for note in notes {
        println!("{:30} {}", note.title, note.path.display());
    }
}

fn print_note_listing(
    notes: Vec<nero_core::NoteListEntry>,
    workspace_name: &str,
    workspace_path: &std::path::Path,
    long: bool,
) {
    println!(
        "Workspace: {workspace_name}  ({})",
        workspace_path.display()
    );
    if notes.is_empty() {
        println!("No notes yet. Create one with `nero new <title>`.");
        return;
    }

    let headers = ["ID", "TITLE", "CREATED", "LAST EDITED", "PATH"];
    let widths = [4, 34, 10, 16, 30];
    let rows = notes
        .iter()
        .map(|note| {
            vec![
                note.id.to_string(),
                note.summary.title.clone(),
                note.created.clone().unwrap_or_else(|| "—".into()),
                note.modified.clone().unwrap_or_else(|| "—".into()),
                note.summary.path.to_string_lossy().into_owned(),
            ]
        })
        .collect::<Vec<_>>();
    print_table(&headers, &rows, &widths);

    if long {
        for note in &notes {
            let fields = note
                .metadata
                .iter()
                // The title is already displayed in the table, so avoid repeating it.
                .filter(|(key, _)| key.as_str() != "title")
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>();
            if fields.is_empty() {
                println!("  {}  Metadata: (none)", note.id);
            } else {
                println!("  {}  Metadata: {}", note.id, fields.join(" · "));
            }
        }
        println!("  Metadata omits `title` because it is shown in the TITLE column.");
    }
}

fn print_table(headers: &[&str], rows: &[Vec<String>], widths: &[usize]) {
    fn border(widths: &[usize], left: char, middle: char, right: char) -> String {
        let mut line = String::new();
        line.push(left);
        for (index, width) in widths.iter().enumerate() {
            if index > 0 {
                line.push(middle);
            }
            line.push_str(&"─".repeat(width + 2));
        }
        line.push(right);
        line
    }
    fn row(cells: &[String], widths: &[usize]) -> String {
        let mut line = String::from("│");
        for (index, width) in widths.iter().enumerate() {
            let value = cells.get(index).map(String::as_str).unwrap_or("");
            line.push(' ');
            line.push_str(&fit_cell(value, *width));
            line.push(' ');
            line.push('│');
        }
        line
    }

    println!("{}", border(widths, '┌', '┬', '┐'));
    let header_cells = headers
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    println!("{}", row(&header_cells, widths));
    println!("{}", border(widths, '├', '┼', '┤'));
    for values in rows {
        println!("{}", row(values, widths));
    }
    println!("{}", border(widths, '└', '┴', '┘'));
}

fn fit_cell(value: &str, width: usize) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    let fitted = if chars.len() > width {
        if width == 0 {
            String::new()
        } else if width == 1 {
            "…".to_owned()
        } else {
            format!("{}…", chars[..width - 1].iter().collect::<String>())
        }
    } else {
        value.to_owned()
    };
    let padding = width.saturating_sub(fitted.chars().count());
    format!("{fitted}{}", " ".repeat(padding))
}

fn workspace_label(workspace: &Workspace) -> String {
    let root = canonical_or_original(workspace.root());
    if let Ok(view) = config::view() {
        if let Some((name, _)) = view
            .workspaces
            .iter()
            .find(|(_, path)| canonical_or_original(path) == root)
        {
            return name.clone();
        }
        if view
            .default_workspace_path
            .as_ref()
            .is_some_and(|path| canonical_or_original(path) == root)
        {
            return "default".to_owned();
        }
    }
    root.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_owned()
}

fn canonical_or_original(path: &std::path::Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn print_config_show(workspace_override: Option<&str>) -> Result<()> {
    let home = nero_home_dir()?;
    let config_path = config::config_path()?;

    println!("Nero configuration");
    println!("──────────────────");
    println!("Application home : {}", home.display());
    println!(
        "User config      : {} ({})",
        config_path.display(),
        if config_path.exists() {
            "exists"
        } else {
            "defaults; file not created yet"
        }
    );
    println!("Backup key dir   : {}", Workspace::config_dir()?.display());
    println!("Workspace home   : {}", config::workspace_home()?.display());
    println!("NERO_HOME        : {}", env_status("NERO_HOME"));
    println!("NERO_CONFIG_HOME : {}", env_status("NERO_CONFIG_HOME"));
    println!("NERO_CONFIG_DIR  : {}", env_status("NERO_CONFIG_DIR"));
    println!("NERO_WORKSPACE   : {}", env_status("NERO_WORKSPACE"));

    if let Ok(view) = config::view() {
        let default_workspace = view
            .default_workspace_name
            .as_ref()
            .map(|name| format!("name `{name}`"))
            .or_else(|| {
                view.default_workspace_path
                    .as_ref()
                    .map(|path| path.display().to_string())
            })
            .unwrap_or_else(|| "not set (directory discovery is used)".to_owned());
        println!("Default workspace: {default_workspace}");
        println!("Registered workspaces:");
        if view.workspaces.is_empty() {
            println!("  (none)");
        } else {
            for (name, path) in view.workspaces {
                println!("  {name:18} {}", path.display());
            }
        }
    }

    println!("\nBackup identity");
    match Workspace::backup_key_info()? {
        Some(key) => {
            println!(
                "  Private identity: {} (present; contents are never printed)",
                key.identity_path.display()
            );
            println!("  Public recipient: {}", key.recipient);
        }
        None => {
            println!(
                "  Not generated yet. Run `nero key generate` to enable age-encrypted workspace backups."
            );
            println!("  Expected path: {}", Workspace::identity_path()?.display());
        }
    }

    println!("\nActive workspace remotes");
    match selected_workspace(workspace_override) {
        Ok(workspace) => {
            println!(
                "  Workspace: {} ({})",
                workspace_label(&workspace),
                workspace.root().display()
            );
            println!(
                "  Workspace settings: {}",
                workspace.storage_config_path().display()
            );
            let remotes = workspace.storage_remotes()?;
            if remotes.is_empty() {
                println!("  Nero storage profiles: (none)");
            } else {
                println!("  Nero storage profiles (rclone targets):");
                for remote in remotes {
                    let encryption = if remote.encrypt_backups {
                        " [encrypted backups]"
                    } else {
                        ""
                    };
                    println!(
                        "    {:16} {}{}",
                        remote.name,
                        redact_remote_url(&remote.target),
                        encryption
                    );
                }
            }

            if workspace.root().join(".git").exists() {
                println!("  Git remotes:");
                match workspace.git_remotes() {
                    Ok(output) if output.trim().is_empty() => {
                        println!("    (repository has no remotes)");
                    }
                    Ok(output) => {
                        for line in output.lines() {
                            let mut parts = line.split_whitespace();
                            let name = parts.next().unwrap_or("remote");
                            let url = parts.next().unwrap_or("");
                            let kind = parts.next().unwrap_or("");
                            println!("    {name:16} {} {kind}", redact_remote_url(url));
                        }
                    }
                    Err(error) => println!("    unable to read Git remotes: {error}"),
                }
            } else {
                println!("  Git remotes: (Git is not initialized in this workspace)");
            }
        }
        Err(error) => println!("  No active workspace could be resolved: {error}"),
    }

    println!("\nCloud / rclone");
    match ProcessCommand::new("rclone")
        .args(["config", "file"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(path) = stdout.lines().map(str::trim).rfind(|line| !line.is_empty()) {
                println!("  rclone config: {path}");
            } else {
                println!("  rclone is installed; config path was not reported.");
            }
            match ProcessCommand::new("rclone").arg("listremotes").output() {
                Ok(remotes) if remotes.status.success() => {
                    let names = String::from_utf8_lossy(&remotes.stdout);
                    let names = names
                        .lines()
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                        .collect::<Vec<_>>();
                    if names.is_empty() {
                        println!("  Configured rclone remotes: (none)");
                    } else {
                        println!("  Configured rclone remotes: {}", names.join(", "));
                    }
                }
                _ => println!("  Could not enumerate rclone remote names."),
            }
        }
        _ => {
            println!("  rclone is not installed or has no readable config.");
            println!("  Nero stores target names, not provider credentials.");
        }
    }
    println!(
        "\nUse `nero config path`, `nero home`, `nero workspace show`, and `nero storage list`."
    );
    println!("These commands show focused configuration views.");
    Ok(())
}

fn env_status(name: &str) -> String {
    match env::var_os(name) {
        Some(value) => format!("set ({})", value.to_string_lossy()),
        None => "not set".to_owned(),
    }
}

fn redact_remote_url(value: &str) -> String {
    // Avoid echoing HTTP basic-auth credentials or token query strings in a config dump.
    let Some((scheme, rest)) = value.split_once("://") else {
        return value.to_owned();
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let suffix = &rest[authority_end..];
    let safe_authority = if let Some((_, host)) = authority.rsplit_once('@') {
        format!("[redacted]@{host}")
    } else {
        authority.to_owned()
    };
    let safe_suffix = match suffix.find(['?', '#']) {
        Some(start) if suffix[start..].starts_with('?') => {
            format!("{}?[redacted]", &suffix[..start])
        }
        Some(start) => format!("{}#[redacted]", &suffix[..start]),
        None => suffix.to_owned(),
    };
    format!("{scheme}://{safe_authority}{safe_suffix}")
}

fn print_search(results: Vec<SearchResult>) {
    if results.is_empty() {
        println!("No matches.");
        return;
    }
    for result in results {
        println!("\n{}  ({})", result.note.title, result.note.path.display());
        println!("  {}", result.preview);
    }
}

fn join_args<I>(args: I) -> Result<String>
where
    I: IntoIterator<Item = String>,
{
    let joined = args.into_iter().collect::<Vec<_>>().join(" ");
    if joined.trim().is_empty() {
        Err(nero_core::NeroError::Message(
            "an argument is required".into(),
        ))
    } else {
        Ok(joined)
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn extracts_workspace_option_before_or_after_command() {
        let mut args = vec!["-w", "research", "find", "eigenvalues"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            extract_workspace_option(&mut args).unwrap().as_deref(),
            Some("research")
        );
        assert_eq!(args, vec!["find".to_owned(), "eigenvalues".to_owned()]);

        let mut args = vec!["find", "eigenvalues", "--workspace=/home/user/notes"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert_eq!(
            extract_workspace_option(&mut args).unwrap().as_deref(),
            Some("/home/user/notes")
        );
        assert_eq!(args, vec!["find".to_owned(), "eigenvalues".to_owned()]);
    }

    #[test]
    fn rejects_duplicate_workspace_selectors() {
        let mut args = vec!["-w", "personal", "--workspace", "research"]
            .into_iter()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        assert!(extract_workspace_option(&mut args).is_err());
    }

    #[test]
    fn config_dump_redacts_embedded_url_credentials_and_query() {
        let value = redact_remote_url("https://alice:supersecret@example.com/repo?token=abc");
        assert_eq!(value, "https://[redacted]@example.com/repo?[redacted]");
        assert!(!value.contains("supersecret"));
        assert!(!value.contains("abc"));
    }

    #[test]
    fn table_cells_are_clipped_and_padded() {
        assert_eq!(fit_cell("abc", 5), "abc  ");
        assert_eq!(fit_cell("abcdef", 4), "abc…");
    }
}
