//! Splash, help, header, preset and parameter widgets of the main screen.

use crate::dsp::control::Mode;
use crate::dsp::preset::presets;
use crate::ui::app::Snap;
use crate::ui::widgets::{block, centered_rect, p_row, HELP_TEXT, LOGO};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Splash screen shown for the first ~1.5s.
pub(crate) fn splash(f: &mut Frame, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();
    for (i, l) in LOGO.iter().enumerate() {
        let color = match i {
            0 | 1 => Color::Cyan,
            2 | 3 => Color::Magenta,
            4 | 5 => Color::Blue,
            _ => Color::Gray,
        };
        lines.push(Line::styled(
            l.to_string(),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(
        "  press any key, click, or use the arrow keys ...",
    ));
    lines.push(Line::styled(
        "  audio is already running",
        Style::default().fg(Color::DarkGray),
    ));
    let para = Paragraph::new(lines).alignment(Alignment::Center);
    f.render_widget(para, centered_rect(70, 55, area));
}

/// Static key reference screen.
pub(crate) fn help(f: &mut Frame, area: Rect) {
    let para = Paragraph::new(HELP_TEXT)
        .style(Style::default().fg(Color::White))
        .block(block(" vois · help "));
    f.render_widget(para, area);
}

/// Top status line: mode, record / tone / tune / mute indicators.
pub(crate) fn header_line(s: &Snap) -> Line<'static> {
    let mode_txt = match s.mode {
        Mode::Live => "LIVE",
        Mode::Passthrough => "A/B PASSTHROUGH",
    };
    let rec = if s.record { "● REC" } else { "" };
    let tone = if s.tone { "● TONE" } else { "" };
    let tune = if s.tune { "● TUNE" } else { "" };
    let mute = if s.mute { "MUTED" } else { "" };
    let b = Modifier::BOLD;
    Line::from(vec![
        Span::styled("  VOIS ", Style::default().fg(Color::Cyan).add_modifier(b)),
        Span::styled(
            "real-time voice changer",
            Style::default()
                .fg(Color::Gray)
                .add_modifier(Modifier::ITALIC),
        ),
        Span::raw("   ·   "),
        Span::styled(mode_txt, Style::default().fg(Color::Yellow).add_modifier(b)),
        Span::raw("  "),
        Span::styled(rec, Style::default().fg(Color::Red).add_modifier(b)),
        Span::raw("  "),
        Span::styled(tone, Style::default().fg(Color::Blue)),
        Span::raw("  "),
        Span::styled(tune, Style::default().fg(Color::Magenta)),
        Span::raw("  "),
        Span::styled(mute, Style::default().fg(Color::Red).add_modifier(b)),
    ])
}

/// Preset list column, current one highlighted.
pub(crate) fn presets_paragraph(preset_idx: usize) -> Paragraph<'static> {
    let mut lines = Vec::new();
    for (i, p) in presets().iter().enumerate() {
        let selected = i == preset_idx;
        let label = format!(
            "{}{}  {}",
            if selected { "▶" } else { " " },
            if selected { "" } else { " " },
            p.name
        );
        let style = if selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Gray)
        };
        lines.push(Line::styled(label, style));
    }
    Paragraph::new(lines).block(block(" presets · ↑↓ to switch "))
}

/// Parameters column built from the live values.
pub(crate) fn params_paragraph(s: &Snap) -> Paragraph<'static> {
    let gate = if s.gate <= -90.0 {
        "off".to_string()
    } else {
        format!("{:.0} dB", s.gate)
    };
    let gain = 20.0 * s.gain.max(1e-6).log10();
    let lines = vec![
        p_row("preset", &s.name, Color::Yellow, "voice character"),
        p_row(
            "pitch",
            &format!("{:+.1} st", s.pitch),
            Color::Cyan,
            "higher=girl, lower=deep",
        ),
        p_row(
            "formant",
            &format!("{:.2}×", s.formant),
            Color::Magenta,
            "brightness/timbre",
        ),
        p_row("gate", &gate, Color::Blue, "noise below this dB is cut"),
        p_row(
            "gain",
            &format!("{gain:+.1} dB"),
            Color::Green,
            "output loudness",
        ),
        p_row(
            "mute",
            if s.mute { "ON" } else { "off" },
            if s.mute { Color::Red } else { Color::DarkGray },
            "silence output",
        ),
        p_row(
            "tune",
            if s.tune { "ON" } else { "off" },
            if s.tune {
                Color::Magenta
            } else {
                Color::DarkGray
            },
            "auto-tune: snap pitch",
        ),
    ];
    Paragraph::new(lines).block(block(" parameters · info "))
}
