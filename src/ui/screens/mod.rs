//! Widget builders for each screen, one file per screen.

pub(super) mod main;
pub(super) mod settings;
pub(super) mod stats;

pub(super) use main::{header_line, help, params_paragraph, presets_paragraph, splash};
pub(super) use settings::settings_paragraph;
pub(super) use stats::{stats_column, StatsSnap};
