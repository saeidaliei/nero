use std::{env, io::{self, Write}, path::PathBuf, process::{exit, Command as ProcessCommand}};

mod config;

use nero_core::{is_workspace_root, nero_home_dir, NoteSummary, Result, SearchResult, Workspace};

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
                Some("help") | Some("--help") | Some("-h") => println!("home [show|path] — show Nero's application home and data directories; override with NERO_HOME"),
                Some(other) => return Err(nero_core::NeroError::Message(format!("unknown home command `{other}`; use show or path"))),
            }
        }
        "init" => {
            let root = args.next().map(PathBuf::from)
                .or_else(|| workspace_override.clone().map(PathBuf::from))
                .unwrap_or(env::current_dir()?);
            let workspace = Workspace::init(root)?;
            println!("initialized Nero workspace at {}", workspace.root().display());
        }
        "workspace" | "workspaces" => handle_workspace_command(args, workspace_override.as_deref())?,
        "new" => {
            let title = join_args(args)?;
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let note = workspace.create_note(&title)?;
            println!("created {}", note.path.display());
        }
        "list" | "ls" => {
            let workspace = selected_workspace(workspace_override.as_deref())?;
            print_notes(workspace.list_notes()?);
        }
        "open" => {
            let query = join_args(args)?;
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let note = workspace.read_note(&query)?;
            println!("{}\n\n{}", note.summary.title, note.document.source.trim_end());
        }
        "edit" => {
            let query = join_args(args)?;
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let status = workspace.edit(&query)?;
            if !status.success() { exit(status.code().unwrap_or(1)); }
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
                    println!("  [{status}] {} ({}, due {})", item.summary.title, item.summary.path.display(), item.due_date);
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
            println!("index refreshed: {} updated, {} removed", stats.indexed, stats.removed);
        }
        "watch" => {
            let workspace = selected_workspace(workspace_override.as_deref())?;
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
                "home" => {
                    let action = values.first().map(String::as_str).unwrap_or("help");
                    let positional = values.iter().skip(1).cloned().collect::<Vec<_>>();
                    match action {
                        "create" => {
                            if positional.len() > 1 {
                                return Err(nero_core::NeroError::Message("usage: nero backup home create [archive.zip.age]".into()));
                            }
                            let destination = if let Some(path) = positional.first() {
                                PathBuf::from(path)
                            } else {
                                nero_home_dir()?.join("backups").join(format!("nero-home-backup-{}.zip.age", backup_timestamp()))
                            };
                            let passphrase = prompt_home_backup_passphrase(true)?;
                            let (path, manifest) = Workspace::create_home_backup(nero_home_dir()?, destination, passphrase)?;
                            println!("created encrypted Nero home backup: {} ({} files, {} bytes before compression)", path.display(), manifest.files.len(), manifest.stats().bytes);
                            println!("keep the passphrase somewhere safe; it cannot be recovered by Nero");
                        }
                        "verify" => {
                            let archive = positional.first().ok_or_else(|| nero_core::NeroError::Message("usage: nero backup home verify <archive.zip.age>".into()))?;
                            let passphrase = prompt_home_backup_passphrase(false)?;
                            let stats = Workspace::verify_home_backup(archive, passphrase)?;
                            println!("home backup verified: {} files, {} bytes before compression", stats.files, stats.bytes);
                        }
                        "restore" => {
                            let archive = positional.first().ok_or_else(|| nero_core::NeroError::Message("usage: nero backup home restore <archive.zip.age> [destination]".into()))?;
                            let destination = positional.get(1).map(PathBuf::from).unwrap_or(nero_home_dir()?);
                            let passphrase = prompt_home_backup_passphrase(false)?;
                            let stats = Workspace::restore_home_backup(archive, destination.clone(), passphrase)?;
                            println!("Nero home restored to {} ({} files, {} bytes before path rebasing)", destination.display(), stats.files, stats.bytes);
                            println!("run `nero home` and `nero workspace list` to check the restored profile");
                        }
                        "recovery-test" | "test" => {
                            let archive = positional.first().ok_or_else(|| nero_core::NeroError::Message("usage: nero backup home recovery-test <archive.zip.age>".into()))?;
                            let passphrase = prompt_home_backup_passphrase(false)?;
                            let stats = Workspace::recovery_test_home_backup(archive, passphrase)?;
                            println!("Nero home backup recovery test passed: {} files, {} bytes", stats.files, stats.bytes);
                        }
                        _ => println!("home backup commands: create [archive.zip.age], verify <archive.zip.age>, restore <archive.zip.age> [destination], recovery-test <archive.zip.age>"),
                    }
                }
                "create" => {
                    let workspace = selected_workspace(workspace_override.as_deref())?;
                    let (encrypt, identity, positional) = parse_backup_flags(values)?;
                    let default_extension = if encrypt { "age" } else { "zip" };
                    let destination = if let Some(path) = positional.first() {
                        PathBuf::from(path)
                    } else {
                        nero_home_dir()?.join("backups").join(format!("nero-backup-{}.{}", backup_timestamp(), default_extension))
                    };
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
                    let is_encrypted = Workspace::backup_is_encrypted(archive)?;
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
                    let is_encrypted = Workspace::backup_is_encrypted(archive)?;
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
                    let workspace = selected_workspace(workspace_override.as_deref())?;
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
                    let workspace = selected_workspace(workspace_override.as_deref())?;
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
                    let workspace = selected_workspace(workspace_override.as_deref())?;
                    let temporary = env::temp_dir().join(format!("nero-download-{}-{}", std::process::id(), backup_name));
                    let downloaded = workspace.storage_download_file(storage_name, backup_name, &temporary)?;
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
                    if force {
                        eprintln!("WARNING: replacing the Nero backup identity makes existing encrypted backups unrecoverable with the new identity.");
                    }
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
            let workspace = selected_workspace(workspace_override.as_deref())?;
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
            let workspace = selected_workspace(workspace_override.as_deref())?;
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
            let workspace = selected_workspace(workspace_override.as_deref())?;
            for line in workspace.doctor()? { println!("{line}"); }
        }
        "gui" => {
            let workspace = selected_workspace(workspace_override.as_deref())?;
            let status = ProcessCommand::new("nero-gui")
                .env("NERO_WORKSPACE", workspace.root())
                .status()
                .map_err(|_| nero_core::NeroError::Message(
                    "`nero-gui` is not installed or not on PATH; see gui/README.md".into(),
                ))?;
            if !status.success() { exit(status.code().unwrap_or(1)); }
        }
        "tui" => {
            let workspace = selected_workspace(workspace_override.as_deref())?;
            nero_tui::run(workspace).map_err(nero_core::NeroError::Io)?;
        }
        other => return Err(nero_core::NeroError::Message(format!("unknown command `{other}` — run `nero help`"))),
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
                nero_core::NeroError::Message(format!("{current} requires a path or registered workspace name"))
            })?;
            if selected.is_some() {
                return Err(nero_core::NeroError::Message("specify --workspace only once".into()));
            }
            selected = Some(value.clone());
            index += 2;
        } else if let Some(value) = current.strip_prefix("--workspace=") {
            if value.trim().is_empty() {
                return Err(nero_core::NeroError::Message("--workspace requires a path or registered workspace name".into()));
            }
            if selected.replace(value.to_owned()).is_some() {
                return Err(nero_core::NeroError::Message("specify --workspace only once".into()));
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
            workspace.root().display(), workspace.root().display()
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
            None | Some("show") => println!("workspace home: {}", config::workspace_home()?.display()),
            Some("set") => {
                let path = args.next().ok_or_else(|| nero_core::NeroError::Message("usage: nero workspace home set <parent-directory>".into()))?;
                let path = config::set_workspace_home(PathBuf::from(path))?;
                println!("workspace home: {}", path.display());
            }
            Some("reset") => println!("workspace home: {}", config::reset_workspace_home()?.display()),
            Some(other) => return Err(nero_core::NeroError::Message(format!("unknown workspace home command `{other}`; use show, set, or reset"))),
        },
        "create" => {
            let name = args.next().ok_or_else(|| nero_core::NeroError::Message("usage: nero workspace create <name>".into()))?;
            let path = config::create_workspace(&name)?;
            println!("created workspace `{name}` at {}", path.display());
            let view = config::view()?;
            if view.default_workspace_name.as_deref() == Some(name.as_str()) {
                println!("selected as the default workspace");
            }
        }
        "set" | "default" => {
            let path = args.next().ok_or_else(|| {
                nero_core::NeroError::Message("usage: nero workspace set <initialized-workspace-path>".into())
            })?;
            let path = config::set_default_path(PathBuf::from(path))?;
            println!("default workspace: {}", path.display());
            println!("commands now work from any directory; use `-w` to override for one command");
        }
        "add" => {
            let name = args.next().ok_or_else(|| nero_core::NeroError::Message("usage: nero workspace add <name> <path>".into()))?;
            let path = args.next().ok_or_else(|| nero_core::NeroError::Message("usage: nero workspace add <name> <path>".into()))?;
            let path = config::add_workspace(&name, PathBuf::from(path))?;
            println!("registered workspace `{name}` -> {}", path.display());
        }
        "use" | "switch" => {
            let name = args.next().ok_or_else(|| nero_core::NeroError::Message("usage: nero workspace use <name>".into()))?;
            let path = config::use_workspace(&name)?;
            println!("default workspace: {name} -> {}", path.display());
        }
        "list" | "ls" => {
            let view = config::view()?;
            println!("workspace home: {}", view.workspace_home_path.display());
            if view.workspaces.is_empty() && view.default_workspace_path.is_none() {
                println!("No workspaces configured yet. Use `nero workspace create personal` or `nero workspace add <name> <path>`.");
                return Ok(());
            }
            let default_named = view.default_workspace_name.as_deref();
            for (name, path) in &view.workspaces {
                println!("{} {:16} {}", if default_named == Some(name.as_str()) { "*" } else { " " }, name, path.display());
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
            let name = args.next().ok_or_else(|| nero_core::NeroError::Message("usage: nero workspace remove <name>".into()))?;
            let path = config::remove_workspace(&name)?;
            println!("removed workspace `{name}` ({})", path.display());
        }
        "clear" | "unset" => {
            config::clear_default()?;
            println!("default workspace cleared; Nero will use NERO_WORKSPACE or discover from the current directory");
        }
        "help" | "--help" | "-h" => {
            println!("workspace commands:\n  home [set <path>|reset] show/set the parent directory for workspaces\n  create <name>    create a workspace under the workspace home\n  set <path>       set the default to an initialized workspace path\n  add <name> <path> register an existing workspace\n  use <name>       make a named workspace the default\n  list             list registered workspaces\n  show             print the workspace selected for this command\n  remove <name>    remove a registered workspace\n  clear            clear the saved default")
        }
        other => return Err(nero_core::NeroError::Message(format!("unknown workspace command `{other}`; run `nero workspace help`"))),
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

fn prompt_home_backup_passphrase(confirm: bool) -> Result<String> {
    let passphrase = rpassword::prompt_password("Home backup passphrase: ")
        .map_err(|error| nero_core::NeroError::Message(format!("could not read passphrase: {error}")))?;
    if passphrase.is_empty() {
        return Err(nero_core::NeroError::Message("passphrase cannot be empty".into()));
    }
    if confirm {
        if passphrase.chars().count() < 12 {
            return Err(nero_core::NeroError::Message("use a home-backup passphrase of at least 12 characters".into()));
        }
        let confirmation = rpassword::prompt_password("Confirm passphrase: ")
            .map_err(|error| nero_core::NeroError::Message(format!("could not read passphrase confirmation: {error}")))?;
        if passphrase != confirmation {
            return Err(nero_core::NeroError::Message("passphrases do not match; backup was not created".into()));
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
    println!(r#"Nero — Markdown notes, search, links, and math across CLI, terminal, and desktop.

USAGE
  nero [--workspace PATH|NAME] <command> [arguments]
  nero -w PATH|NAME <command> [arguments]

APPLICATION HOME
  home                    Show Nero's application home and data directories
  home path               Print the application home path (default ~/.nero)

WORKSPACES
  workspace home          Show the parent directory for workspaces (default ~/.nero/workspaces)
  workspace home set <path>
                          Choose a different parent directory
  workspace home reset   Return to $NERO_HOME/workspaces
  workspace create <name>
                          Create and register a workspace under that directory
  workspace set <path>    Set the default to an initialized workspace
  workspace add <name> <path>
                          Register an existing workspace
  workspace use <name>    Make a named workspace the default
  workspace list          List registered workspaces
  workspace show          Show the workspace currently selected
  workspace remove <name> Remove a registered workspace
  workspace clear         Clear the saved default workspace
  --workspace / -w        Override the workspace for one command

COMMANDS
  init [path]             Create a workspace
  new <title>             Create a Markdown note
  list                    List notes
  open <note>             Print a note
  edit <note>             Open a note in $EDITOR
  find <query>            Search notes
  backlinks <note>        Find notes linking to a note
  today                   Open today's daily note and list due/overdue notes
  render <note>           Render a note to HTML
  reindex                 Rebuild/update the search index
  watch                   Watch Markdown files and refresh the index
  doctor                  Check workspace health
  backup create [path]    Create a portable local backup
  backup create --encrypt [path]
                          Create an age-encrypted workspace backup
  backup home create [path.zip.age]
                          Create a passphrase-encrypted full-home backup
  backup home verify <path.zip.age>
                          Verify a full-home backup
  backup home restore <path.zip.age> [destination]
                          Restore the whole Nero home on this/new machine
  backup home recovery-test <path.zip.age>
                          Test a full-home backup without replacing anything
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

WORKSPACE SELECTION
  --workspace PATH|NAME (or -w) overrides the selected workspace for one command.
  Otherwise Nero uses NERO_WORKSPACE, directory discovery, then the saved default.
  New named workspaces are created under $NERO_HOME/workspaces (default ~/.nero/workspaces).
  Config, backup keys, named workspaces, and local backups live beneath $NERO_HOME.
  Set NERO_HOME to an absolute path to relocate Nero's entire application home.
  `backup home create` encrypts the complete app home with a separate passphrase.

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

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn extracts_workspace_option_before_or_after_command() {
        let mut args = vec!["-w", "research", "find", "eigenvalues"]
            .into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(extract_workspace_option(&mut args).unwrap().as_deref(), Some("research"));
        assert_eq!(args, vec!["find".to_owned(), "eigenvalues".to_owned()]);

        let mut args = vec!["find", "eigenvalues", "--workspace=/home/user/notes"]
            .into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert_eq!(extract_workspace_option(&mut args).unwrap().as_deref(), Some("/home/user/notes"));
        assert_eq!(args, vec!["find".to_owned(), "eigenvalues".to_owned()]);
    }

    #[test]
    fn rejects_duplicate_workspace_selectors() {
        let mut args = vec!["-w", "personal", "--workspace", "research"]
            .into_iter().map(str::to_owned).collect::<Vec<_>>();
        assert!(extract_workspace_option(&mut args).is_err());
    }
}
