use nero_core::Document;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
};

fn header_style() -> Style {
    Style::default()
        .fg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}
fn link_style() -> Style {
    Style::default()
        .fg(Color::Blue)
        .add_modifier(Modifier::UNDERLINED)
}
fn muted_style() -> Style {
    Style::default().fg(Color::DarkGray)
}
fn code_style() -> Style {
    Style::default().fg(Color::Green)
}
fn math_style() -> Style {
    Style::default().fg(Color::Magenta)
}

pub fn render(document: &Document) -> Text<'static> {
    let body = document.source.as_str();
    let mut lines = Vec::new();
    let mut in_frontmatter = false;
    let mut in_code = false;
    let mut in_math = false;

    for (line_number, raw) in body.lines().enumerate() {
        let line = raw.trim_end();

        if line_number == 0 && line.trim() == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if line.trim() == "---" {
                in_frontmatter = false;
            }
            continue;
        }

        if line.trim_start().starts_with("```") || line.trim_start().starts_with("~~~") {
            if in_code {
                lines.push(Line::from(Span::styled("  └─", muted_style())));
            } else {
                let language = line
                    .trim()
                    .trim_start_matches('`')
                    .trim_start_matches('~')
                    .trim();
                let label = if language.is_empty() {
                    "code"
                } else {
                    language
                };
                lines.push(Line::from(Span::styled(
                    format!("  ┌─ {label}"),
                    muted_style(),
                )));
            }
            in_code = !in_code;
            continue;
        }

        if in_code {
            lines.push(Line::from(Span::styled(
                format!("  │ {line}"),
                code_style(),
            )));
            continue;
        }

        let trimmed = line.trim_start();
        if trimmed == "$$" {
            in_math = !in_math;
            lines.push(Line::from(Span::styled(
                if in_math { "  ⟦" } else { "  ⟧" },
                math_style(),
            )));
            continue;
        }
        if in_math {
            lines.push(Line::from(Span::styled(
                format!("  │ {}", math_to_unicode(line)),
                math_style(),
            )));
            continue;
        }
        if trimmed.starts_with("$$") && trimmed.ends_with("$$") && trimmed.len() > 4 {
            let expr = trimmed
                .trim_start_matches("$$")
                .trim_end_matches("$$")
                .trim();
            lines.push(Line::from(Span::styled(
                format!("  ⟦ {} ⟧", math_to_unicode(expr)),
                math_style(),
            )));
            continue;
        }
        if let Some(expr) = inline_display_math(trimmed) {
            lines.push(Line::from(Span::styled(
                format!("  ⟦ {} ⟧", math_to_unicode(expr)),
                math_style(),
            )));
            continue;
        }

        if trimmed.is_empty() {
            lines.push(Line::from(""));
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix("# ") {
            lines.push(Line::from(vec![Span::styled(
                heading.to_string(),
                header_style(),
            )]));
            lines.push(Line::from(Span::styled(
                "────────────────────────",
                muted_style(),
            )));
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix("## ") {
            lines.push(Line::from(Span::styled(
                heading.to_string(),
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix("### ") {
            lines.push(Line::from(Span::styled(
                heading.to_string(),
                Style::default().fg(Color::Cyan),
            )));
            continue;
        }
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            lines.push(Line::from(Span::styled(
                "────────────────────────────────────────",
                muted_style(),
            )));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("> ") {
            lines.push(Line::from(vec![
                Span::styled("│ ", muted_style()),
                styled_inline(rest),
            ]));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
            lines.push(Line::from(vec![
                Span::styled("☐ ", Style::default().fg(Color::Yellow)),
                styled_inline(rest),
            ]));
            continue;
        }
        if let Some(rest) = trimmed
            .strip_prefix("- [x] ")
            .or_else(|| trimmed.strip_prefix("- [X] "))
        {
            lines.push(Line::from(vec![
                Span::styled("☑ ", Style::default().fg(Color::Green)),
                styled_inline(rest),
            ]));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            lines.push(Line::from(vec![
                Span::styled("• ", muted_style()),
                styled_inline(rest),
            ]));
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("* ") {
            lines.push(Line::from(vec![
                Span::styled("• ", muted_style()),
                styled_inline(rest),
            ]));
            continue;
        }
        if numbered_item(trimmed).is_some() {
            lines.push(Line::from(styled_inline(trimmed)));
            continue;
        }

        lines.push(Line::from(styled_inline(line)));
    }

    if lines.is_empty() {
        lines.push(Line::from(Span::styled("(empty note)", muted_style())));
    }
    Text::from(lines)
}

fn numbered_item(line: &str) -> Option<&str> {
    let (number, rest) = line.split_once(". ")?;
    if number.chars().all(|c| c.is_ascii_digit()) {
        Some(rest)
    } else {
        None
    }
}

fn inline_display_math(line: &str) -> Option<&str> {
    if line.starts_with("\\[") && line.ends_with("\\]") && line.len() > 4 {
        return Some(
            line.trim_start_matches("\\[")
                .trim_end_matches("\\]")
                .trim(),
        );
    }
    if line.starts_with("\\(") && line.ends_with("\\)") && line.len() > 4 {
        return Some(
            line.trim_start_matches("\\(")
                .trim_end_matches("\\)")
                .trim(),
        );
    }
    None
}

fn styled_inline(input: &str) -> Span<'static> {
    let mut visible = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(index) = input[cursor..].find("[[") {
        let start = cursor + index;
        visible.push_str(&input[cursor..start]);
        let link_start = start + 2;
        let Some(end_rel) = input[link_start..].find("]]") else {
            visible.push_str(&input[start..]);
            cursor = input.len();
            break;
        };
        let end = link_start + end_rel;
        let raw = &input[link_start..end];
        let label = raw.split_once('|').map_or(raw, |(_, label)| label).trim();
        visible.push('⟨');
        visible.push_str(label);
        visible.push('⟩');
        cursor = end + 2;
    }
    if cursor < input.len() {
        visible.push_str(&input[cursor..]);
    }

    if visible.is_empty() {
        visible = input.to_owned();
    }
    if visible.contains('⟨') || visible.starts_with("http://") || visible.starts_with("https://")
    {
        Span::styled(visible, link_style())
    } else {
        Span::raw(visible)
    }
}

fn math_to_unicode(input: &str) -> String {
    let replacements = [
        (r"\\alpha", "α"),
        (r"\\beta", "β"),
        (r"\\gamma", "γ"),
        (r"\\delta", "δ"),
        (r"\\epsilon", "ε"),
        (r"\\theta", "θ"),
        (r"\\lambda", "λ"),
        (r"\\mu", "μ"),
        (r"\\pi", "π"),
        (r"\\sigma", "σ"),
        (r"\\phi", "φ"),
        (r"\\omega", "ω"),
        (r"\\infty", "∞"),
        (r"\\sum", "∑"),
        (r"\\prod", "∏"),
        (r"\\int", "∫"),
        (r"\\nabla", "∇"),
        (r"\\cdot", "·"),
        (r"\\times", "×"),
        (r"\\to", "→"),
        (r"\\rightarrow", "→"),
        (r"\\leftarrow", "←"),
        (r"\\leq", "≤"),
        (r"\\geq", "≥"),
        (r"\\neq", "≠"),
        (r"\\approx", "≈"),
        (r"\\pm", "±"),
        (r"\\sqrt", "√"),
    ];
    let mut output = input.to_owned();
    for (from, to) in replacements {
        output = output.replace(from, to);
    }
    output = replace_frac(&output);
    output.replace("^2", "²").replace("^3", "³")
}

fn replace_frac(input: &str) -> String {
    let mut out = input.to_owned();
    while let Some(start) = out.find("\\frac{") {
        let numerator_start = start + 6;
        let Some(numerator_end_rel) = out[numerator_start..].find('}') else {
            break;
        };
        let numerator_end = numerator_start + numerator_end_rel;
        let denominator_start = numerator_end + 1;
        if out.as_bytes().get(denominator_start) != Some(&b'{') {
            break;
        }
        let denominator_content_start = denominator_start + 1;
        let Some(denominator_end_rel) = out[denominator_content_start..].find('}') else {
            break;
        };
        let denominator_end = denominator_content_start + denominator_end_rel;
        let numerator = &out[numerator_start..numerator_end];
        let denominator = &out[denominator_content_start..denominator_end];
        let replacement = format!("({})/({})", numerator, denominator);
        out.replace_range(start..denominator_end + 1, &replacement);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_common_markdown_shapes() {
        let document = Document::parse(
            "---\ntitle: Test\n---\n\n# Hello\n\n- [ ] Task\n\nSee [[World]].\n\n$$\\frac{x^2}{y}$$",
        );
        let rendered = render(&document);
        let plain = rendered
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.contains("Hello"));
        assert!(plain.contains("☐ Task"));
        assert!(plain.contains("⟨World⟩"));
        assert!(plain.contains("(x²)/(y)"));
    }
}
