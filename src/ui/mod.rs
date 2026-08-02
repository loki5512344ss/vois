//! Ratatui full-screen interface: `app` keeps state, `render` draws screens
//! and drives the main loop, `widgets` provides shared helpers, `screens`
//! builds the widgets for each screen.

pub mod app;
pub mod render;
pub mod screens;
pub mod widgets;

pub use render::run;

/// Static information about the audio session, shown in the TUI.
pub struct SessionInfo {
    pub input: String,
    pub output: String,
    pub sample_rate: u32,
    pub in_ch: usize,
    pub out_ch: usize,
    pub format_in: String,
    pub format_out: String,
}
