// Virtual microphone setup for PipeWire/PulseAudio.
//
// Creates a "vois.rs" null sink and remaps its monitor source to "vois.rs".
// The app does NOT hijack the default sink permanently: it briefly points the
// default sink at `vois.rs` while its own output stream is created (so the
// processed voice lands in the virtual mic) and restores it right after.
// Other apps' streams are not moved, so their audio never leaks into the mic.
use anyhow::{bail, Context, Result};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct VirtualMic;

impl VirtualMic {
    pub fn setup() -> Result<Option<String>> {
        let pactl = "pactl";

        let sinks = Command::new(pactl)
            .args(["list", "short", "sinks"])
            .output()
            .context("failed to run `pactl list short sinks`")?;
        if !sinks.status.success() {
            bail!("`pactl list short sinks` failed");
        }
        let sinks_out = String::from_utf8_lossy(&sinks.stdout);

        if !has_entry(&sinks_out, "vois.rs") {
            let out = Command::new(pactl)
                .args([
                    "load-module",
                    "module-null-sink",
                    "sink_name=vois.rs",
                    "sink_properties=device.description=vois virtual mic",
                ])
                .output()
                .context("failed to create the vois.rs null sink")?;
            if !out.status.success() {
                bail!(
                    "`pactl load-module module-null-sink sink_name=vois.rs` failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                );
            }
        }

        let sources = Command::new(pactl)
            .args(["list", "short", "sources"])
            .output()
            .context("failed to run `pactl list short sources`")?;
        if !sources.status.success() {
            bail!("`pactl list short sources` failed");
        }
        let sources_out = String::from_utf8_lossy(&sources.stdout);

        if !has_entry(&sources_out, "vois.rs") {
            let out = Command::new(pactl)
                .args([
                    "load-module",
                    "module-remap-source",
                    "source_name=vois.rs",
                    "master=vois.rs.monitor",
                ])
                .output()
                .context("failed to create the vois.rs remap source")?;
            if !out.status.success() {
                bail!(
                    "`pactl load-module module-remap-source source_name=vois.rs` failed: {}",
                    String::from_utf8_lossy(&out.stderr).trim()
                );
            }
        }

        Ok(Some("vois.rs".to_string()))
    }

    /// Current default sink name, if any.
    pub fn get_default_sink() -> Option<String> {
        let out = Command::new("pactl")
            .arg("get-default-sink")
            .output()
            .ok()?;
        let name = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if name.is_empty() {
            None
        } else {
            Some(name)
        }
    }

    pub fn set_default_sink(name: &str) {
        let _ = Command::new("pactl")
            .args(["set-default-sink", name])
            .output();
    }

    /// Keep every sink-input that is NOT this app's (client name "vois") away
    /// from the `vois.rs` sink, moving it to `prev_sink`. Runs until `stop`.
    ///
    /// WirePlumber routes ALSA streams to the default sink, so while the app
    /// runs with `vois.rs` as default, other apps' audio would leak into the
    /// virtual mic. Moving their streams explicitly sticks, so this thread
    /// parks them on the real output.
    pub fn keep_others_off(stop: Arc<AtomicBool>, prev_sink: String) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let target_sink_id = sink_id_named("vois.rs");
                let prev_id = sink_id_named(&prev_sink);
                if let (Some(lv), Some(pv)) = (target_sink_id, prev_id) {
                    if let (Ok(names), Ok(inputs)) = (client_names(), sink_inputs()) {
                        for (id, sink, client) in &inputs {
                            let ours = names.get(client).map(|n| n == "vois").unwrap_or(false);
                            if sink == &lv && !ours {
                                let _ = Command::new("pactl")
                                    .args(["move-sink-input", id, &pv])
                                    .output();
                            }
                        }
                    }
                }
                thread::sleep(Duration::from_millis(500));
            }
        })
    }

    /// Restore the default sink if the process is killed (SIGINT/SIGTERM),
    /// so the user's audio isn't left pointing at the silent null sink.
    pub fn restore_on_signal(prev: String) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let mut signals = match signal_hook::iterator::Signals::new([
                signal_hook::consts::signal::SIGINT,
                signal_hook::consts::signal::SIGTERM,
            ]) {
                Ok(s) => s,
                Err(_) => return,
            };
            if let Some(_sig) = signals.forever().next() {
                Self::set_default_sink(&prev);
                std::process::exit(130);
            }
        })
    }
}

fn sink_id_named(name: &str) -> Option<String> {
    let out = Command::new("pactl")
        .args(["list", "short", "sinks"])
        .output()
        .ok()?;
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 2 && fields[1] == name {
            return Some(fields[0].to_string());
        }
    }
    None
}

fn client_names() -> Result<std::collections::HashMap<String, String>> {
    let out = Command::new("pactl")
        .args(["list", "short", "clients"])
        .output()
        .context("failed to run `pactl list short clients`")?;
    let mut map = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 3 {
            map.insert(fields[0].to_string(), fields[2].to_string());
        }
    }
    Ok(map)
}

fn sink_inputs() -> Result<Vec<(String, String, String)>> {
    let out = Command::new("pactl")
        .args(["list", "short", "sink-inputs"])
        .output()
        .context("failed to run `pactl list short sink-inputs`")?;
    let mut inputs = Vec::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 3 {
            inputs.push((
                fields[0].to_string(),
                fields[1].to_string(),
                fields[2].to_string(),
            ));
        }
    }
    Ok(inputs)
}

fn has_entry(list: &str, name: &str) -> bool {
    list.lines()
        .any(|l| l.split_whitespace().nth(1).is_some_and(|n| n == name))
}
