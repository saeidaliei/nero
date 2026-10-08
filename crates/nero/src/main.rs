use std::{env, io::{self, Write}, path::PathBuf, process::exit};

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
            let (note, body) = workspace.read_note(&query)?;
            println!("# {}\n\n{}", note.title, body.trim_end());
        }
        "edit" => {
            let query = join_args(args)?;
            let workspace = Workspace::discover(env::current_dir()?)?;
            let status = workspace.edit(&query)?;
            if !status.success() {
                exit(status.code().unwrap_or(1));
            }
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
            let note = workspace.today()?;
            println!("{}", note.path.display());
        }
        "doctor" => {
            let workspace = Workspace::discover(env::current_dir()?)?;
            for line in workspace.doctor()? {
                println!("{line}");
            }
        }
        "tui" => {
            println!("Nero TUI is scaffolded as `nero-tui`; interactive mode is the next milestone.");
            println!("Run `cargo run -p nero-tui` from the source tree to enter the TUI development stub.");
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

fn print_help() {
    println!(
        r#"Nero — a small place for your thoughts.

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
  doctor                  Check workspace health
  tui                     Launch the terminal interface (coming next)
  help                    Show this help
  version                 Show the version

NOTES
  Notes are ordinary Markdown files.
  Internal links use [[Note Name]].
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
        Err(nero_core::NeroError::Message("an argument is required".into()))
    } else {
        Ok(joined)
    }
}
