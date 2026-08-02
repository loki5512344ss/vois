//! Shared constants and stateless render helpers for the TUI screens.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders};

pub(super) const LOGO: [&str; 7] = [
    "   ██╗   ██╗ ██████╗ ██╗███████╗   ",
    "   ██║   ██║██╔═══██╗██║██╔════╝   ",
    "   ██║   ██║██║   ██║██║███████╗   ",
    "   ╚██╗ ██╔╝██║   ██║██║╚════██║   ",
    "    ╚████╔╝ ╚██████╔╝██║███████║   ",
    "     ╚═══╝   ╚═════╝ ╚═╝╚══════╝   ",
    "        real-time voice changer     ",
];

pub(super) const SETTING_INFO: [&str; 9] = [
    "voice character, previewed live",
    "pitch: higher = girl/kid, lower = deep",
    "formant: brightness / timbre of the voice",
    "noise gate: audio below this dB is cut",
    "output loudness, in dB",
    "silence the output",
    "LIVE = processed, PASSTHROUGH = raw input (A/B)",
    "write processed audio to a WAV file",
    "auto-tune: snap pitch to the nearest semitone",
];

pub(super) const HELP_TEXT: &str = "\
  KEY / MOUSE                 ACTION
  ───────────────────────────────────────────────
  ↑ ↓  (or mouse)             select preset / settings row
  ← →                         change value
  Enter                       toggle / apply
  S  /  Tab                   toggle SETTINGS screen
  Esc                         back to main screen
  H                           toggle this help screen
  Q  /  Ctrl+C                quit
  0 - 9                       jump to preset
  M                           mute      R  record
  B                           A/B passthrough mode
";

const METER_FLOOR_DB: f32 = -60.0;

/// RMS / peak value converted to decibels, floored at the meter range.
pub(super) fn db(rms: f32) -> f32 {
    if rms <= 0.0 {
        METER_FLOOR_DB
    } else {
        (20.0 * rms.log10()).clamp(METER_FLOOR_DB, 6.0)
    }
}

/// Meter ratio in 0..=1 for the level gauges.
pub(super) fn meter_ratio(db: f32) -> f64 {
    ((db - METER_FLOOR_DB) / -METER_FLOOR_DB).clamp(0.0, 1.0) as f64
}

/// Meter color: red near the top, yellow mid, green low.
pub(super) fn meter_style(level_db: f32) -> Style {
    if level_db >= -6.0 {
        Style::default().fg(Color::Red)
    } else if level_db >= -20.0 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    }
}

/// Layout rects for the main screen: (header, body, footer).
pub(super) fn main_layout(area: Rect) -> (Rect, Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    (chunks[0], chunks[1], chunks[2])
}

/// Layout rects for the settings screen: (header, list, footer).
pub(super) fn settings_layout(area: Rect) -> (Rect, Rect, Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    (chunks[0], chunks[1], chunks[2])
}

/// A settings table row: label / value / hint.
pub(super) fn p_row(label: &str, value: &str, color: Color, info: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("  {label:<8}"),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            value.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("   ({info})"), Style::default().fg(Color::DarkGray)),
    ])
}

/// Bordered panel with a cyan title.
pub(super) fn block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title.to_string(),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::DarkGray))
}

/// A rect of `percent_x`/`percent_y` centered inside `area`.
pub(super) fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let height = ((area.height as u32 * percent_y as u32 / 100) as u16).max(12);
    let top = area.y + (area.height.saturating_sub(height) / 2);
    let rect = Rect::new(area.x, top, area.width, height);
    let width = ((rect.width as u32 * percent_x as u32 / 100) as u16).max(30);
    let x = rect.x + (rect.width.saturating_sub(width) / 2);
    Rect::new(x, rect.y, width, rect.height)
}
