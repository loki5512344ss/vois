//! WAV recording of the processed mono stream via a dedicated writer thread.

use hound::{SampleFormat, WavSpec, WavWriter};
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;
use std::thread::JoinHandle;

pub fn spawn_writer(path: String, sample_rate: u32) -> (Sender<Vec<f32>>, JoinHandle<()>) {
    let (tx, rx) = mpsc::channel::<Vec<f32>>();
    let handle = thread::spawn(move || {
        let spec = WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = match WavWriter::create(&path, spec) {
            Ok(w) => w,
            Err(e) => {
                eprintln!("record: cannot create {path}: {e}");
                return;
            }
        };
        while let Ok(block) = rx.recv() {
            for &s in &block {
                let v = (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                if writer.write_sample(v).is_err() {
                    break;
                }
            }
        }
        if let Err(e) = writer.finalize() {
            eprintln!("record: {e}");
        } else {
            println!("recorded to {path}");
        }
    });
    (tx, handle)
}
