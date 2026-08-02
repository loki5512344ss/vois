//! Stats screen widget: peak meters, dsp load, rates, uptime, devices.

use crate::ui::widgets::{meter_ratio, meter_style};
use crate::ui::SessionInfo;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Gauge, Paragraph};
use ratatui::Frame;

/// Snapshot of the DSP stats column, pre-computed by the render layer.
#[derive(Default)]
pub(crate) struct StatsSnap {
    pub(crate) in_db: f32,
    pub(crate) out_db: f32,
    pub(crate) peak_in: f32,
    pub(crate) peak_out: f32,
    pub(crate) cpu: f32,
    pub(crate) latency: f32,
    pub(crate) blocks: u64,
    pub(crate) samples: u64,
    pub(crate) uptime: u64,
}

/// Stats column: meters on top, text lines below.
pub(crate) fn stats_column(f: &mut Frame, area: Rect, info: &SessionInfo, s: &StatsSnap) {
    let secs = (s.uptime as f32).max(0.1);
    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("  peak in   ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:+.0} dB", s.peak_in), meter_style(s.peak_in)),
            Span::styled("   peak out ", Style::default().fg(Color::DarkGray)),
            Span::styled(format!("{:+.0} dB", s.peak_out), meter_style(s.peak_out)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  dsp load  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.1} %", s.cpu),
                if s.cpu > 50.0 {
                    Style::default().fg(Color::Red)
                } else {
                    Style::default().fg(Color::Green)
                },
            ),
            Span::raw("  "),
            Span::styled("latency", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(" {:.0} ms", s.latency)),
        ]),
        Line::from(vec![
            Span::styled("  rate      ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{} Hz  {} ch → {} ch  ({} → {})",
                info.sample_rate, info.in_ch, info.out_ch, info.format_in, info.format_out
            )),
        ]),
        Line::from(vec![
            Span::styled("  uptime    ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{:02}:{:02}:{:02}",
                s.uptime / 3600,
                (s.uptime / 60) % 60,
                s.uptime % 60
            )),
        ]),
        Line::from(vec![
            Span::styled("  blocks    ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{}  ({:.0} blk/s)",
                s.blocks,
                s.samples as f32 / secs
            )),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            format!("  in:  {}", info.input),
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(Span::styled(
            format!("  out: {}", info.output),
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let vert = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    let in_gauge = Gauge::default()
        .ratio(meter_ratio(s.in_db))
        .gauge_style(meter_style(s.in_db));
    let out_gauge = Gauge::default()
        .ratio(meter_ratio(s.out_db))
        .gauge_style(meter_style(s.out_db));

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  in  "),
            Span::styled(format!("{:+.0} dB", s.in_db), meter_style(s.in_db)),
        ])),
        vert[0],
    );
    f.render_widget(in_gauge, vert[1]);
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::raw("  out "),
            Span::styled(format!("{:+.0} dB", s.out_db), meter_style(s.out_db)),
        ])),
        vert[2],
    );
    f.render_widget(out_gauge, vert[3]);
    f.render_widget(Paragraph::new(lines), vert[4]);
}
