use anyhow::Result;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
use anyhow::{bail, Context};
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{thread, time::Duration};
pub struct VirtualMic;
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn wireplumber_conf() -> std::path::PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::env::var("HOME")
                .map(|h| std::path::PathBuf::from(h).join(".config"))
                .unwrap_or_else(|_| std::path::PathBuf::from("."))
        });
    base.join("wireplumber")
        .join("wireplumber.conf.d")
        .join("51-vois.conf")
}
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
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
        if Self::ensure_wireplumber_fix() {
            Self::restart_wireplumber();
        }
        Ok(Some("vois.rs".to_string()))
    }
    pub fn ensure_wireplumber_fix() -> bool {
        let conf = wireplumber_conf();
        let conf_text = "wireplumber.settings = {\n node.stream.restore-target = false\n node.stream.restore-props = false\n}\n";
        if let Ok(existing) = std::fs::read_to_string(&conf) {
            if existing.contains("restore-target = false") {
                return false;
            }
        }
        if let Some(dir) = conf.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(&conf, conf_text).is_ok()
    }
    pub fn restart_wireplumber() {
        let _ = Command::new("systemctl")
            .args(["--user", "restart", "wireplumber"])
            .output();
        for _ in 0..40 {
            if Command::new("pactl")
                .args(["info"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
            {
                return;
            }
            thread::sleep(Duration::from_millis(100));
        }
    }
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
    pub fn wait_default_sink(name: &str) {
        for _ in 0..40 {
            if Self::get_default_sink().as_deref() == Some(name) {
                return;
            }
            thread::sleep(Duration::from_millis(50));
        }
    }
    pub fn keep_others_off(stop: Arc<AtomicBool>, prev_sink: String) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let target_sink_id = sink_id_named("vois.rs");
                let prev_id = sink_id_named(&prev_sink);
                if let (Some(lv), Some(pv)) = (target_sink_id, prev_id) {
                    if let (Ok(names), Ok(inputs)) = (client_names(), sink_inputs()) {
                        for (id, sink, client) in &inputs {
                            let ours = names.get(client).map(|n| n == "vois").unwrap_or(false);
                            if ours && sink != &lv {
                                let _ = Command::new("pactl")
                                    .args(["move-sink-input", id, "vois.rs"])
                                    .output();
                            } else if !ours && sink == &lv {
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
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
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
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
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
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
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
#[cfg(any(target_os = "linux", target_os = "freebsd"))]
fn has_entry(list: &str, name: &str) -> bool {
    list.lines()
        .any(|l| l.split_whitespace().nth(1).is_some_and(|n| n == name))
}
#[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
impl VirtualMic {
    pub fn setup() -> Result<Option<String>> {
        Ok(None)
    }
    pub fn get_default_sink() -> Option<String> {
        None
    }
    pub fn set_default_sink(_name: &str) {}
    pub fn wait_default_sink(_name: &str) {}
    pub fn keep_others_off(stop: Arc<AtomicBool>, _prev_sink: String) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(500));
            }
        })
    }
    pub fn restore_on_signal(_prev: String) -> thread::JoinHandle<()> {
        thread::spawn(|| {})
    }
}
