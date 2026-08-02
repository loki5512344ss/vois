pub mod tui;

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
