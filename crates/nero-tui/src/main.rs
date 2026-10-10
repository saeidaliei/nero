fn main() -> std::io::Result<()> {
    let workspace =
        nero_tui::current_workspace().map_err(|error| std::io::Error::other(error.to_string()))?;
    nero_tui::run(workspace)
}
