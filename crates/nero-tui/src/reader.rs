use std::{io, time::Duration};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use nero_core::Document;
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::markdown;

/// Open any Markdown source in a read-only terminal reader, without requiring a workspace.
pub fn run(title: &str, path: &str, source: &str) -> io::Result<()> {
    let document = Document::parse(source);
    let rendered = markdown::render(&document);
    let title = title.to_owned();
    let path = path.to_owned();
    let metadata = document
        .frontmatter
        .iter()
        .filter(|(key, _)| !key.eq_ignore_ascii_case("title"))
        .map(|(key, value)| format!("{key}: {value}"))
        .collect::<Vec<_>>()
        .join("  ·  ");

    ratatui::run(|terminal| {
        let mut scroll = 0_u16;
        loop {
            terminal.draw(|frame| draw(frame, &title, &path, &metadata, &rendered, scroll))?;
            if !event::poll(Duration::from_millis(75))? {
                continue;
            }
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break Ok(()),
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break Ok(()),
                KeyCode::Down | KeyCode::Char('j') => scroll = scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => scroll = scroll.saturating_sub(1),
                KeyCode::PageDown | KeyCode::Char('d') => scroll = scroll.saturating_add(10),
                KeyCode::PageUp | KeyCode::Char('u') => scroll = scroll.saturating_sub(10),
                KeyCode::Home | KeyCode::Char('g') => scroll = 0,
                KeyCode::End | KeyCode::Char('G') => {
                    scroll = rendered
                        .lines
                        .len()
                        .saturating_sub(1)
                        .min(u16::MAX as usize) as u16;
                }
                _ => {}
            }
        }
    })
}

fn draw(
    frame: &mut Frame,
    title: &str,
    path: &str,
    metadata: &str,
    rendered: &Text<'static>,
    scroll: u16,
) {
    let area = frame.area();
    let sections = Layout::vertical([
        Constraint::Length(if metadata.is_empty() { 1 } else { 2 }),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    let title_line = Line::from(vec![
        Span::styled(
            format!(" {title} "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" · {path}"), Style::default().fg(Color::DarkGray)),
    ]);
    let mut header = vec![title_line];
    if !metadata.is_empty() {
        header.push(Line::from(Span::styled(
            format!(" {metadata}"),
            Style::default().fg(Color::DarkGray),
        )));
    }
    frame.render_widget(Paragraph::new(header), sections[0]);

    frame.render_widget(
        Paragraph::new(rendered.clone())
            .block(Block::default().borders(Borders::ALL).title(" Markdown "))
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0)),
        sections[1],
    );
    frame.render_widget(
        Paragraph::new(" j/k or ↑/↓ scroll · PgUp/PgDn page · g/G top/bottom · q/Esc close ")
            .style(Style::default().fg(Color::DarkGray)),
        sections[2],
    );
}
