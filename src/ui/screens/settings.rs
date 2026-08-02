//! Settings screen widget builder.

use crate::ui::widgets::{block, SETTING_INFO};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

/// Settings rows from rendered values and the current cursor row.
pub(crate) fn settings_paragraph(values: &[String], cursor: usize) -> Paragraph<'static> {
    let labels = [
        "Preset",
        "Pitch",
        "Formant",
        "Gate (min)",
        "Gain",
        "Mute",
        "Mode",
        "Record",
        "Pitch corr",
    ];
    let mut lines = Vec::new();
    for (i, label) in labels.iter().enumerate() {
        let selected = i == cursor;
        let style = if selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        let info_style = Style::default().fg(Color::DarkGray);
        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "{}{} {:<12}",
                    if selected { "▶" } else { " " },
                    if selected { "" } else { " " },
                    label
                ),
                style,
            ),
            Span::styled(format!("{:<16}", values[i]), style),
            Span::styled(SETTING_INFO[i], info_style),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "  use ↑ ↓ to select, ← → (or Enter) to change, Esc to go back",
        Style::default().fg(Color::DarkGray),
    ));
    Paragraph::new(lines).block(block(" settings "))
}
