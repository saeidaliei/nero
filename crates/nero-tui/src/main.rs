use std::{env, io::{self, Write}};
use nero_core::Result;

fn main() {
    if let Err(error) = run() {
        eprintln!("nero-tui: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let workspace = match nero_core::Workspace::discover(&cwd) {
        Ok(workspace) => workspace,
        Err(_) => {
            println!("Nero TUI");
            println!();
            println!("No Nero workspace found in {}", cwd.display());
            println!("Run `nero init` first.");
            return Ok(());
        }
    };

    println!("Nero — TUI shell");
    println!("================");
    println!();
    println!("Workspace: {}", workspace.root().display());
    println!();
    println!("The Ratatui interface is the next milestone.");
    println!("The core is already usable; try `nero list`, `nero find`, and `nero today`.");
    io::stdout().flush().ok();
    Ok(())
}
