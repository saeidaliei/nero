mod markdown;

use std::{collections::VecDeque, env, io, path::PathBuf, sync::mpsc::Receiver, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use nero_core::{NoteSummary, Result, Workspace, WorkspaceWatcher};
use ratatui::{
    layout::{Constraint, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    DefaultTerminal, Frame,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SideMode {
    None,
    Backlinks,
    Recent,
    Context,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputMode {
    Normal,
    Search,
    Command,
}

#[derive(Debug, Clone)]
struct SideItem {
    label: String,
    detail: String,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy)]
struct CommandSpec {
    name: &'static str,
    description: &'static str,
    usage: &'static str,
}

const COMMANDS: &[CommandSpec] = &[
    CommandSpec { name: "new", description: "create a Markdown note", usage: "new <title>" },
    CommandSpec { name: "open", description: "open a note", usage: "open <note>" },
    CommandSpec { name: "search", description: "search the workspace", usage: "search <query>" },
    CommandSpec { name: "today", description: "open today's daily note", usage: "today" },
    CommandSpec { name: "reindex", description: "refresh the search index", usage: "reindex" },
    CommandSpec { name: "backlinks", description: "show links into the current note", usage: "backlinks" },
    CommandSpec { name: "recent", description: "show recently opened notes", usage: "recent" },
    CommandSpec { name: "context", description: "show links, tasks and metadata", usage: "context" },
    CommandSpec { name: "close", description: "close the side panel", usage: "close" },
    CommandSpec { name: "quit", description: "quit Nero", usage: "quit" },
];

struct App {
    workspace: Workspace,
    _watcher: WorkspaceWatcher,
    watcher_rx: Receiver<Vec<PathBuf>>,
    notes: Vec<NoteSummary>,
    selected: usize,
    selected_path: Option<PathBuf>,
    title: String,
    path: PathBuf,
    document: Option<nero_core::Document>,
    rendered: Text<'static>,
    status: String,
    input_mode: InputMode,
    input: String,
    command_selected: usize,
    side_mode: SideMode,
    side_selected: usize,
    side_items: Vec<SideItem>,
    recent: VecDeque<NoteSummary>,
    scroll: u16,
    dirty_notice: bool,
}

impl App {
    fn new(workspace: Workspace) -> Result<Self> {
        let (_watcher, watcher_rx) = workspace.watch()?;
        let notes = workspace.list_notes()?;
        let mut app = Self {
            workspace,
            _watcher,
            watcher_rx,
            notes,
            selected: 0,
            selected_path: None,
            title: String::from("Nero"),
            path: PathBuf::new(),
            rendered: Text::raw("No notes yet.") ,
            status: String::from("Ready"),
            input_mode: InputMode::Normal,
            input: String::new(),
            command_selected: 0,
            side_mode: SideMode::None,
            side_selected: 0,
            side_items: Vec::new(),
            recent: VecDeque::new(),
            scroll: 0,
            dirty_notice: false,
        };
        app.select_index(0)?;
        Ok(app)
    }

    fn select_index(&mut self, index: usize) -> Result<()> {
        if self.notes.is_empty() {
            self.selected = 0;
            self.selected_path = None;
            self.title = "No notes".into();
            self.path.clear();
            self.document = None;
            self.rendered = Text::raw("Create your first note with :new <title>.");
            self.scroll = 0;
            return Ok(());
        }

        self.selected = index.min(self.notes.len() - 1);
        let path = self.notes[self.selected].path.clone();
        self.open_path(path)
    }

    fn open_path(&mut self, path: PathBuf) -> Result<()> {
        let note = self.workspace.read_note(&path.to_string_lossy())?;
        self.title = note.summary.title.clone();
        self.path = note.summary.path.clone();
        self.document = Some(note.document.clone());
        self.rendered = markdown::render(&note.document);
        self.dirty_notice = false;
        self.selected_path = Some(note.summary.path.clone());
        self.scroll = 0;
        self.push_recent(note.summary);
        self.refresh_side_panel()?;
        Ok(())
    }

    fn push_recent(&mut self, note: NoteSummary) {
        self.recent.retain(|item| item.path != note.path);
        self.recent.push_front(note);
        while self.recent.len() > 12 {
            self.recent.pop_back();
        }
    }

    fn refresh_notes(&mut self) -> Result<()> {
        let old_path = self.selected_path.clone();
        self.notes = self.workspace.list_notes()?;
        self.selected = old_path
            .as_ref()
            .and_then(|path| self.notes.iter().position(|note| &note.path == path))
            .unwrap_or_else(|| self.selected.min(self.notes.len().saturating_sub(1)));

        if let Some(path) = old_path {
            if self.notes.iter().any(|note| note.path == path) {
                self.open_path(path)?;
            } else {
                self.select_index(self.selected)?;
            }
        } else {
            self.select_index(self.selected)?;
        }
        self.refresh_side_panel()?;
        Ok(())
    }

    fn poll_watcher(&mut self) -> Result<()> {
        let mut changed = false;
        for paths in self.watcher_rx.try_iter() {
            if !paths.is_empty() {
                changed = true;
            }
        }
        if !changed {
            return Ok(());
        }

        let current_path = self.selected_path.clone();
        self.workspace.reindex()?;
        self.refresh_notes()?;
        if let Some(path) = current_path {
            if self.notes.iter().any(|note| note.path == path) {
                self.status = format!("Reloaded {}", path.display());
            } else {
                self.status = "Note set changed".into();
            }
        } else {
            self.status = "Workspace changed".into();
        }
        self.dirty_notice = true;
        Ok(())
    }

    fn move_selection(&mut self, delta: isize) -> Result<()> {
        if self.notes.is_empty() {
            return Ok(());
        }
        let len = self.notes.len() as isize;
        let next = (self.selected as isize + delta).clamp(0, len - 1) as usize;
        if next != self.selected {
            self.select_index(next)?;
        }
        Ok(())
    }

    fn move_side_selection(&mut self, delta: isize) {
        if self.side_items.is_empty() {
            self.side_selected = 0;
            return;
        }
        let len = self.side_items.len() as isize;
        self.side_selected = (self.side_selected as isize + delta).clamp(0, len - 1) as usize;
    }

    fn activate_side_item(&mut self) -> Result<()> {
        let Some(item) = self.side_items.get(self.side_selected).cloned() else {
            return Ok(());
        };
        if let Some(path) = item.path {
            if let Some(index) = self.notes.iter().position(|note| note.path == path) {
                self.side_mode = SideMode::None;
                self.select_index(index)?;
                self.status = format!("Opened {}", item.label);
            }
        }
        Ok(())
    }

    fn refresh_side_panel(&mut self) -> Result<()> {
        self.side_items.clear();
        self.side_selected = 0;
        match self.side_mode {
            SideMode::None => {}
            SideMode::Backlinks => {
                self.side_items = self
                    .workspace
                    .backlinks(&self.path.to_string_lossy())?
                    .into_iter()
                    .map(|note| SideItem {
                        label: note.title,
                        detail: note.path.display().to_string(),
                        path: Some(note.path),
                    })
                    .collect();
            }
            SideMode::Recent => {
                self.side_items = self
                    .recent
                    .iter()
                    .filter(|note| note.path != self.path)
                    .cloned()
                    .map(|note| SideItem {
                        label: note.title,
                        detail: note.path.display().to_string(),
                        path: Some(note.path),
                    })
                    .collect();
            }
            SideMode::Context => {
                if let Ok(note) = self.workspace.read_note(&self.path.to_string_lossy()) {
                    for link in &note.document.wiki_links {
                        let resolved = self.workspace.resolve_link(&note.summary.path, &link.target)?;
                        let label = link.display_label().to_owned();
                        self.side_items.push(SideItem {
                            label: format!("→ {label}"),
                            detail: resolved
                                .as_ref()
                                .map(|note| note.path.display().to_string())
                                .unwrap_or_else(|| "broken link".into()),
                            path: resolved.map(|note| note.path),
                        });
                    }
                    self.side_items.push(SideItem {
                        label: format!("{} open tasks", note.document.tasks.open),
                        detail: format!("{} completed", note.document.tasks.done),
                        path: None,
                    });
                    self.side_items.push(SideItem {
                        label: format!("{} math block(s)", note.document.math_count),
                        detail: "LaTeX / dollar math".into(),
                        path: None,
                    });
                    for (key, value) in note.document.frontmatter {
                        self.side_items.push(SideItem {
                            label: key,
                            detail: value,
                            path: None,
                        });
                    }
                }
            }
        }
        Ok(())
    }

    fn set_side_mode(&mut self, mode: SideMode) -> Result<()> {
        self.side_mode = if self.side_mode == mode { SideMode::None } else { mode };
        self.refresh_side_panel()
    }

    fn search(&mut self) -> Result<()> {
        let query = self.input.trim().to_owned();
        self.input_mode = InputMode::Normal;
        if query.is_empty() {
            self.refresh_notes()?;
            self.status = format!("{} note(s)", self.notes.len());
            self.input.clear();
            return Ok(());
        }
        self.notes = self.workspace.search(&query)?.into_iter().map(|result| result.note).collect();
        self.selected = 0;
        self.selected_path = None;
        self.open_selected_after_search()?;
        self.status = format!("{} result(s) for {query}", self.notes.len());
        self.input.clear();
        Ok(())
    }

    fn open_selected_after_search(&mut self) -> Result<()> {
        if let Some(note) = self.notes.first() {
            self.open_path(note.path.clone())?;
        } else {
            self.title = "No matches".into();
            self.path.clear();
            self.body.clear();
            self.rendered = Text::raw("No notes match the query.");
            self.scroll = 0;
        }
        Ok(())
    }

    fn filtered_commands(&self) -> Vec<(usize, CommandSpec)> {
        let needle = self.input.trim_start_matches(':').trim().to_lowercase();
        let command_word = needle.split_whitespace().next().unwrap_or("");
        let has_arguments = needle.split_whitespace().count() > 1;
        COMMANDS
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, command)| {
                if needle.is_empty() {
                    true
                } else if has_arguments {
                    command.name == command_word
                } else {
                    command.name.contains(&needle) || command.description.contains(&needle)
                }
            })
            .collect()
    }

    fn execute_command(&mut self) -> Result<bool> {
        let raw = self.input.trim().trim_start_matches(':').trim().to_owned();
        if raw.is_empty() {
            return Ok(false);
        }
        let mut parts = raw.splitn(2, char::is_whitespace);
        let command = parts.next().unwrap_or_default();
        let argument = parts.next().unwrap_or_default().trim();

        match command {
            "new" => {
                if argument.is_empty() {
                    self.status = "Usage: :new <title>".into();
                } else {
                    let note = self.workspace.create_note(argument)?;
                    self.refresh_notes()?;
                    if let Some(index) = self.notes.iter().position(|item| item.path == note.path) {
                        self.select_index(index)?;
                    }
                    self.status = format!("Created {}", note.path.display());
                    self.input.clear();
                    self.input_mode = InputMode::Normal;
                }
            }
            "open" => {
                if argument.is_empty() {
                    self.status = "Usage: :open <note>".into();
                } else if let Some(index) = self.notes.iter().position(|item| item.path.to_string_lossy().eq_ignore_ascii_case(argument) || item.title.eq_ignore_ascii_case(argument)) {
                    self.select_index(index)?;
                    self.input.clear();
                    self.input_mode = InputMode::Normal;
                } else if let Some(note) = self.workspace.read_note(argument).ok() {
                    if let Some(index) = self.notes.iter().position(|item| item.path == note.summary.path) {
                        self.select_index(index)?;
                    } else {
                        self.open_path(note.summary.path)?;
                    }
                    self.input.clear();
                    self.input_mode = InputMode::Normal;
                } else {
                    self.status = format!("Note not found: {argument}");
                }
            }
            "search" => {
                if argument.is_empty() {
                    self.status = "Usage: :search <query>".into();
                } else {
                    self.input = argument.to_owned();
                    self.search()?;
                }
            }
            "today" => {
                let note = self.workspace.today()?;
                self.refresh_notes()?;
                if let Some(index) = self.notes.iter().position(|item| item.path == note.path) {
                    self.select_index(index)?;
                }
                self.status = "Opened today".into();
                self.input.clear();
                self.input_mode = InputMode::Normal;
            }
            "reindex" => {
                let stats = self.workspace.reindex()?;
                self.status = format!("Index: {} updated, {} removed", stats.indexed, stats.removed);
                self.input.clear();
                self.input_mode = InputMode::Normal;
            }
            "backlinks" => {
                self.set_side_mode(SideMode::Backlinks)?;
                self.input.clear();
                self.input_mode = InputMode::Normal;
            }
            "recent" => {
                self.set_side_mode(SideMode::Recent)?;
                self.input.clear();
                self.input_mode = InputMode::Normal;
            }
            "context" => {
                self.set_side_mode(SideMode::Context)?;
                self.input.clear();
                self.input_mode = InputMode::Normal;
            }
            "close" => {
                self.side_mode = SideMode::None;
                self.input.clear();
                self.input_mode = InputMode::Normal;
            }
            "quit" | "q" => return Ok(true),
            _ => self.status = format!("Unknown command: {command}"),
        }
        Ok(false)
    }

    fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        loop {
            terminal.draw(|frame| draw(frame, self))?;
            self.poll_watcher().map_err(to_io)?;
            if event::poll(Duration::from_millis(75))? {
                if handle_event(self)? {
                    break Ok(());
                }
            }
        }
    }
}

pub fn run(workspace: Workspace) -> io::Result<()> {
    let mut app = App::new(workspace).map_err(to_io)?;
    ratatui::run(|terminal| app.run(terminal))
}

fn draw(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let outer = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    draw_header(frame, app, outer[0]);
    draw_main(frame, app, outer[1]);
    draw_footer(frame, app, outer[2]);

    match app.input_mode {
        InputMode::Normal => {}
        InputMode::Search => draw_search_overlay(frame, app),
        InputMode::Command => draw_command_overlay(frame, app),
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let mode = match app.side_mode {
        SideMode::None => "",
        SideMode::Backlinks => "  · backlinks",
        SideMode::Recent => "  · recent",
        SideMode::Context => "  · context",
    };
    let dirty = if app.dirty_notice { "  •  changed on disk" } else { "" };
    let line = Line::from(vec![
        Span::styled("NERO", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  "),
        Span::styled(app.title.as_str(), Style::default().add_modifier(Modifier::BOLD)),
        Span::styled(mode, Style::default().fg(Color::DarkGray)),
        Span::styled(dirty, Style::default().fg(Color::Yellow)),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    let constraints = match app.side_mode {
        SideMode::None => vec![Constraint::Length(30), Constraint::Min(0)],
        _ => vec![Constraint::Length(28), Constraint::Min(0), Constraint::Length(30)],
    };
    let parts = Layout::horizontal(constraints).split(area);

    draw_note_list(frame, app, parts[0]);
    draw_reader(frame, app, parts[1]);
    if app.side_mode != SideMode::None {
        draw_side(frame, app, parts[2]);
    }
}

fn draw_note_list(frame: &mut Frame, app: &App, area: Rect) {
    let items = app.notes.iter().enumerate().map(|(index, note)| {
        let marker = if index == app.selected { "›" } else { " " };
        let style = if index == app.selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(Line::from(vec![Span::styled(format!("{marker} "), style), Span::styled(note.title.clone(), style)]))
    });
    let mut state = ListState::default();
    if !app.notes.is_empty() {
        state.select(Some(app.selected));
    }
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::RIGHT).title("Notes"))
            .highlight_style(Style::default().bg(Color::DarkGray)),
        area,
        &mut state,
    );
}

fn draw_reader(frame: &mut Frame, app: &App, area: Rect) {
    let title = if app.path.as_os_str().is_empty() {
        app.title.clone()
    } else {
        format!("{}  ·  {}", app.title, app.path.display())
    };
    let paragraph = Paragraph::new(app.rendered.clone())
        .block(Block::default().borders(Borders::LEFT).title(title))
        .wrap(Wrap { trim: false })
        .scroll((app.scroll, 0));
    frame.render_widget(paragraph, area);
}

fn draw_side(frame: &mut Frame, app: &App, area: Rect) {
    let title = match app.side_mode {
        SideMode::Backlinks => "Backlinks",
        SideMode::Recent => "Recent",
        SideMode::Context => "Context",
        SideMode::None => "",
    };
    let items = app.side_items.iter().enumerate().map(|(index, item)| {
        let style = if index == app.side_selected {
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        ListItem::new(Text::from(vec![
            Line::from(Span::styled(item.label.clone(), style)),
            Line::from(Span::styled(format!("  {}", item.detail), Style::default().fg(Color::DarkGray))),
        ]))
    });
    let mut state = ListState::default();
    if !app.side_items.is_empty() {
        state.select(Some(app.side_selected));
    }
    frame.render_stateful_widget(
        List::new(items)
            .block(Block::default().borders(Borders::LEFT).title(title))
            .highlight_style(Style::default().bg(Color::DarkGray)),
        area,
        &mut state,
    );
}

fn draw_footer(frame: &mut Frame, app: &App, area: Rect) {
    let text = match app.input_mode {
        InputMode::Normal => format!(
            "{}  ·  j/k navigate  / search  : commands  b links  c context  r recent  tab side  q quit",
            app.status
        ),
        InputMode::Search => format!("/{}  ·  Enter search  Esc cancel", app.input),
        InputMode::Command => format!(":{}  ·  Enter run  ↑↓ choose  Esc cancel", app.input),
    };
    frame.render_widget(Paragraph::new(text).style(Style::default().fg(Color::DarkGray)), area);
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(area);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(vertical[1])[1]
}

fn draw_search_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(70, 20, frame.area());
    frame.render_widget(Clear, area);
    frame.render_widget(
        Paragraph::new(format!("/{}", app.input))
            .block(Block::default().borders(Borders::ALL).title("Search")),
        area,
    );
    frame.set_cursor_position((area.x + 1 + app.input.len() as u16 + 1, area.y + 1));
}

fn draw_command_overlay(frame: &mut Frame, app: &App) {
    let area = centered_rect(72, 70, frame.area());
    frame.render_widget(Clear, area);
    let commands = app.filtered_commands();
    let height = area.height.saturating_sub(3) as usize;
    let start = app.command_selected.saturating_sub(height / 2).min(commands.len().saturating_sub(height));
    let visible = commands.iter().skip(start).take(height).enumerate().map(|(offset, (_, command))| {
        let absolute = start + offset;
        let selected = absolute == app.command_selected.min(commands.len().saturating_sub(1));
        let style = if selected { Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD) } else { Style::default() };
        ListItem::new(Text::from(vec![
            Line::from(vec![Span::styled(command.name, style), Span::raw("  "), Span::styled(command.description, Style::default().fg(Color::DarkGray))]),
            Line::from(Span::styled(format!("   {}", command.usage), Style::default().fg(Color::DarkGray))),
        ]))
    });
    frame.render_widget(
        List::new(visible).block(Block::default().borders(Borders::ALL).title(format!("Command  :{}", app.input))),
        area,
    );
}

fn handle_event(app: &mut App) -> io::Result<bool> {
    let Event::Key(key) = event::read()? else { return Ok(false); };
    if key.kind != KeyEventKind::Press {
        return Ok(false);
    }

    match app.input_mode {
        InputMode::Search => return handle_search_input(app, key),
        InputMode::Command => return handle_command_input(app, key),
        InputMode::Normal => {}
    }

    match key.code {
        KeyCode::Char('q') => return Ok(true),
        KeyCode::Down | KeyCode::Char('j') => {
            if app.side_mode == SideMode::None { app.move_selection(1).map_err(to_io)?; } else { app.move_side_selection(1); }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.side_mode == SideMode::None { app.move_selection(-1).map_err(to_io)?; } else { app.move_side_selection(-1); }
        }
        KeyCode::PageDown | KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll = app.scroll.saturating_add(10);
        }
        KeyCode::PageUp | KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.scroll = app.scroll.saturating_sub(10);
        }
        KeyCode::PageDown => app.scroll = app.scroll.saturating_add(10),
        KeyCode::PageUp => app.scroll = app.scroll.saturating_sub(10),
        KeyCode::Home => app.scroll = 0,
        KeyCode::End => app.scroll = u16::MAX,
        KeyCode::Char('/') => {
            app.input_mode = InputMode::Search;
            app.input.clear();
        }
        KeyCode::Char(':') => {
            app.input_mode = InputMode::Command;
            app.input.clear();
            app.command_selected = 0;
        }
        KeyCode::Char('b') => app.set_side_mode(SideMode::Backlinks).map_err(to_io)?,
        KeyCode::Char('c') => app.set_side_mode(SideMode::Context).map_err(to_io)?,
        KeyCode::Char('r') => app.set_side_mode(SideMode::Recent).map_err(to_io)?,
        KeyCode::Tab => {
            let next = match app.side_mode {
                SideMode::None => SideMode::Context,
                SideMode::Context => SideMode::Backlinks,
                SideMode::Backlinks => SideMode::Recent,
                SideMode::Recent => SideMode::None,
            };
            app.set_side_mode(next).map_err(to_io)?;
        }
        KeyCode::Esc => {
            app.side_mode = SideMode::None;
        }
        KeyCode::Enter => {
            if app.side_mode != SideMode::None {
                app.activate_side_item().map_err(to_io)?;
            } else if !app.notes.is_empty() {
                app.select_index(app.selected).map_err(to_io)?;
            }
        }
        KeyCode::Char('g') => app.scroll = 0,
        KeyCode::Char('G') => app.scroll = u16::MAX,
        _ => {}
    }
    Ok(false)
}

fn handle_search_input(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<bool> {
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.input.clear();
        }
        KeyCode::Enter => app.search().map_err(to_io)?,
        KeyCode::Backspace => { app.input.pop(); }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => app.input.push(ch),
        _ => {}
    }
    Ok(false)
}

fn handle_command_input(app: &mut App, key: crossterm::event::KeyEvent) -> io::Result<bool> {
    let matches = app.filtered_commands();
    match key.code {
        KeyCode::Esc => {
            app.input_mode = InputMode::Normal;
            app.input.clear();
        }
        KeyCode::Enter => {
            let raw = app.input.trim_start_matches(':').trim().to_owned();
            if raw.is_empty() && !matches.is_empty() {
                let command = matches[app.command_selected.min(matches.len() - 1)].1;
                if matches!(command.name, "new" | "open" | "search") {
                    app.input = format!("{} ", command.name);
                    return Ok(false);
                }
                app.input = command.name.to_owned();
            } else if matches!(raw.as_str(), "new" | "open" | "search") {
                app.input.push(' ');
                return Ok(false);
            }
            if app.execute_command().map_err(to_io)? { return Ok(true); }
        }
        KeyCode::Down => {
            if !matches.is_empty() { app.command_selected = (app.command_selected + 1).min(matches.len() - 1); }
        }
        KeyCode::Up => {
            app.command_selected = app.command_selected.saturating_sub(1);
        }
        KeyCode::Backspace => {
            app.input.pop();
            app.command_selected = 0;
        }
        KeyCode::Char(ch) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.input.push(ch);
            app.command_selected = 0;
        }
        _ => {}
    }
    Ok(false)
}

fn to_io(error: nero_core::NeroError) -> io::Error {
    io::Error::other(error.to_string())
}

pub fn current_workspace() -> Result<Workspace> {
    Workspace::discover(env::current_dir()?)
}
