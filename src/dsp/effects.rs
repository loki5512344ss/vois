//! Effects chain: small processing units applied to the mono stream after
//! pitch/formant shifting. Each effect mutates the block in place.

use std::f32::consts::PI;

pub trait Effect: Send {
    fn process(&mut self, mono: &mut [f32]);
}

// ---------------------------------------------------------------------------
// Distortion (waveshaper)
// ---------------------------------------------------------------------------

pub struct Distortion {
    drive: f32,
    output: f32,
}

impl Distortion {
    pub fn new(drive: f32, output: f32) -> Self {
        Self { drive, output }
    }
}

impl Effect for Distortion {
    fn process(&mut self, mono: &mut [f32]) {
        for s in mono.iter_mut() {
            *s = (*s * self.drive).tanh() * self.output;
        }
    }
}

// ---------------------------------------------------------------------------
// Bitcrusher (bit depth reduction + sample rate reduction)
// ---------------------------------------------------------------------------

pub struct Bitcrush {
    bits: u8,
    sample_rate_div: u32,
    /// Max boost applied to quiet input (auto-level normalization).
    max_gain: f32,
    /// Target peak level after normalization.
    target: f32,
    gain_smooth: f32,
    hold: f32,
    count: u32,
}

impl Bitcrush {
    /// `max_gain`/`target` normalize each block before quantization so the
    /// effect works on quiet mics without clipping loud ones.
    pub fn new(bits: u8, sample_rate_div: u32, max_gain: f32, target: f32) -> Self {
        Self {
            bits: bits.max(2),
            sample_rate_div: sample_rate_div.max(1),
            max_gain: max_gain.max(1.0),
            target: target.clamp(0.1, 1.0),
            gain_smooth: 1.0,
            hold: 0.0,
            count: 0,
        }
    }
}

impl Effect for Bitcrush {
    fn process(&mut self, mono: &mut [f32]) {
        if mono.is_empty() {
            return;
        }
        let levels = (1i32 << (self.bits - 1)) as f32;

        // Auto-level: bring the block peak up to `target` (with smoothing).
        let peak = mono.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        let gain = if peak > 1e-6 {
            (self.target / peak).clamp(1.0, self.max_gain)
        } else {
            self.max_gain
        };
        self.gain_smooth += (gain - self.gain_smooth) * 0.3;
        let g = self.gain_smooth;

        for s in mono.iter_mut() {
            self.count = (self.count + 1) % self.sample_rate_div;
            if self.count == 0 {
                self.hold = *s;
            }
            let q = (self.hold * g * levels).round();
            *s = (q / levels).clamp(-1.0, 1.0);
        }
    }
}

// ---------------------------------------------------------------------------
// Freeverb-style reverb
// ---------------------------------------------------------------------------

struct Comb {
    buf: Vec<f32>,
    idx: usize,
    filter: f32,
    feedback: f32,
    damp1: f32,
    damp2: f32,
}

impl Comb {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len],
            idx: 0,
            filter: 0.0,
            feedback: 0.84,
            damp1: 0.5,
            damp2: 0.5,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let output = self.buf[self.idx];
        self.filter = output * self.damp2 + self.filter * self.damp1;
        self.buf[self.idx] = input + self.filter * self.feedback;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        output
    }
}

struct Allpass {
    buf: Vec<f32>,
    idx: usize,
    feedback: f32,
}

impl Allpass {
    fn new(len: usize) -> Self {
        Self {
            buf: vec![0.0; len],
            idx: 0,
            feedback: 0.5,
        }
    }

    #[inline]
    fn process(&mut self, input: f32) -> f32 {
        let bufout = self.buf[self.idx];
        self.buf[self.idx] = input + bufout * self.feedback;
        self.idx += 1;
        if self.idx >= self.buf.len() {
            self.idx = 0;
        }
        bufout - input
    }
}

pub struct Reverb {
    combs: Vec<Comb>,
    allpasses: Vec<Allpass>,
    wet: f32,
    room: f32,
    damp: f32,
}

impl Reverb {
    pub fn new(sample_rate: u32, room: f32, damp: f32, wet: f32) -> Self {
        let scale = sample_rate as f32 / 44100.0;
        let comb_lens = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
        let allpass_lens = [556, 441, 341, 225];

        let combs: Vec<Comb> = comb_lens
            .iter()
            .map(|&l| Comb::new((l as f32 * scale) as usize))
            .collect();
        let allpasses: Vec<Allpass> = allpass_lens
            .iter()
            .map(|&l| Allpass::new((l as f32 * scale) as usize))
            .collect();

        let mut reverb = Self {
            combs,
            allpasses,
            wet: wet.clamp(0.0, 1.0),
            room: room.clamp(0.0, 1.0),
            damp: damp.clamp(0.0, 1.0),
        };
        reverb.apply_params();
        reverb
    }

    fn apply_params(&mut self) {
        // Map room size 0..1 to feedback 0..~0.98.
        let feedback = self.room * 0.88 + 0.10;
        for comb in &mut self.combs {
            comb.feedback = feedback;
            comb.damp1 = self.damp;
            comb.damp2 = 1.0 - self.damp;
        }
    }
}

impl Effect for Reverb {
    fn process(&mut self, mono: &mut [f32]) {
        let wet = self.wet;
        let dry = 1.0 - wet;
        for s in mono.iter_mut() {
            let input = *s;
            let mut acc = 0.0;
            for comb in &mut self.combs {
                acc += comb.process(input);
            }
            acc /= self.combs.len() as f32;
            for ap in &mut self.allpasses {
                acc = ap.process(acc);
            }
            *s = input * dry + acc * wet * 1.5;
        }
    }
}

// ---------------------------------------------------------------------------
// Chorus
// ---------------------------------------------------------------------------

pub struct Chorus {
    delay: Vec<f32>,
    idx: usize,
    cap: usize,
    depth: f32,
    rate: f32,
    wet: f32,
    phase: f32,
    fs: f32,
}

impl Chorus {
    pub fn new(sample_rate: u32, depth_ms: f32, rate_hz: f32, wet: f32) -> Self {
        let depth = depth_ms / 1000.0 * sample_rate as f32;
        let cap = (sample_rate as f32 * 0.1) as usize + (depth * 4.0) as usize;
        Self {
            delay: vec![0.0; cap],
            idx: 0,
            cap,
            depth: depth.max(1.0),
            rate: rate_hz,
            wet: wet.clamp(0.0, 1.0),
            phase: 0.0,
            fs: sample_rate as f32,
        }
    }

    #[inline]
    fn read(&self, offset: f32) -> f32 {
        let mut pos = self.idx as f32 - offset;
        if pos < 0.0 {
            pos += self.cap as f32;
        }
        let i = pos.floor() as usize;
        let j = (i + 1) % self.cap;
        let t = pos - i as f32;
        self.delay[i] * (1.0 - t) + self.delay[j] * t
    }
}

impl Effect for Chorus {
    fn process(&mut self, mono: &mut [f32]) {
        let depth = self.depth;
        let rate = self.rate;
        let wet = self.wet;
        let dry = 1.0 - wet;
        let cap = self.cap as f32;

        for s in mono.iter_mut() {
            self.delay[self.idx] = *s;
            let lfo = (2.0 * PI * self.phase).sin();

            // Two slightly dephased voices.
            let d1 = depth * (0.35 + 0.35 * lfo);
            let d2 = depth * (0.35 - 0.35 * lfo);
            let v1 = self.read(d1);
            let v2 = self.read(d2);

            *s = *s * dry + (v1 + v2) * 0.5 * wet;

            self.idx += 1;
            if self.idx as f32 >= cap {
                self.idx = 0;
            }
            self.phase += rate / self.fs;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ring modulator
// ---------------------------------------------------------------------------

pub struct RingMod {
    freq: f32,
    depth: f32,
    phase: f32,
    fs: f32,
}

impl RingMod {
    pub fn new(sample_rate: u32, freq_hz: f32, depth: f32) -> Self {
        Self {
            freq: freq_hz,
            depth: depth.clamp(0.0, 1.0),
            phase: 0.0,
            fs: sample_rate as f32,
        }
    }
}

impl Effect for RingMod {
    fn process(&mut self, mono: &mut [f32]) {
        let depth = self.depth;
        for s in mono.iter_mut() {
            let m = 1.0 - depth + depth * (2.0 * PI * self.phase).sin();
            *s *= m;
            self.phase += self.freq / self.fs;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Biquad filters (RBJ cookbook) for bandpass / narrow EQ
// ---------------------------------------------------------------------------

pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    /// Constant-skirt bandpass.
    pub fn bandpass(sample_rate: u32, f0: f32, q: f32) -> Self {
        let w0 = 2.0 * PI * f0 / sample_rate as f32;
        let alpha = w0.sin() / (2.0 * q);
        let b0 = alpha;
        let b1 = 0.0;
        let b2 = -alpha;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha;
        Self::normalized(b0, b1, b2, a1, a2, a0)
    }

    fn normalized(b0: f32, b1: f32, b2: f32, a1: f32, a2: f32, a0: f32) -> Self {
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.z1;
        self.z1 = self.b1 * x - self.a1 * y + self.z2;
        self.z2 = self.b2 * x - self.a2 * y;
        y
    }
}

pub struct Bandpass {
    filter: Biquad,
    mix: f32,
}

impl Bandpass {
    pub fn new(sample_rate: u32, low_hz: f32, high_hz: f32, mix: f32) -> Self {
        let f0 = (low_hz * high_hz).sqrt();
        let q = f0 / (high_hz - low_hz).max(1.0);
        Self {
            filter: Biquad::bandpass(sample_rate, f0, q),
            mix: mix.clamp(0.0, 1.0),
        }
    }
}

impl Effect for Bandpass {
    fn process(&mut self, mono: &mut [f32]) {
        let mix = self.mix;
        for s in mono.iter_mut() {
            let y = self.filter.tick(*s);
            *s = *s * (1.0 - mix) + y * mix;
        }
    }
}

// ---------------------------------------------------------------------------
// White noise generator
// ---------------------------------------------------------------------------

pub struct Noise {
    gain: f32,
    state: u32,
}

impl Noise {
    pub fn new(gain: f32) -> Self {
        Self {
            gain,
            state: 0x9E3779B9,
        }
    }

    #[inline]
    fn next(&mut self) -> f32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.state = x;
        x as f32 / u32::MAX as f32 * 2.0 - 1.0
    }
}

impl Effect for Noise {
    fn process(&mut self, mono: &mut [f32]) {
        for s in mono.iter_mut() {
            *s += self.next() * self.gain;
        }
    }
}

// ---------------------------------------------------------------------------
// Compressor
// ---------------------------------------------------------------------------

pub struct Compressor {
    threshold_db: f32,
    ratio: f32,
    makeup_db: f32,
    env: f32,
    gain: f32,
    fs: f32,
}

impl Compressor {
    pub fn new(sample_rate: u32, threshold_db: f32, ratio: f32, makeup_db: f32) -> Self {
        Self {
            threshold_db,
            ratio: ratio.max(1.0),
            makeup_db,
            env: 0.0,
            gain: 1.0,
            fs: sample_rate as f32,
        }
    }
}

impl Effect for Compressor {
    fn process(&mut self, mono: &mut [f32]) {
        let threshold = 10f32.powf(self.threshold_db / 20.0);
        let makeup = 10f32.powf(self.makeup_db / 20.0);
        let release = 1.0 / (self.fs * 0.1).max(1.0); // ~100 ms
        let attack = 1.0 / (self.fs * 0.002).max(1.0); // ~2 ms
        let inv_ratio = 1.0 - 1.0 / self.ratio;

        for s in mono.iter_mut() {
            let peak = s.abs();
            self.env *= 1.0 - release;
            if peak > self.env {
                self.env = self.env + (peak - self.env) * attack;
            }

            let over_db = 20.0 * (self.env / threshold.max(1e-9)).log10();
            let target_gain = if over_db > 0.0 {
                let g_db = -over_db * inv_ratio;
                10f32.powf(g_db / 20.0)
            } else {
                1.0
            };
            self.gain += (target_gain - self.gain) * attack;
            *s *= self.gain * makeup;
        }
    }
}

// ---------------------------------------------------------------------------
// Channel vocoder (classic robot voice)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum CarrierKind {
    Noise,
    Saw,
}

pub struct ChannelVocoder {
    bands: Vec<VocoderBand>,
    carrier: CarrierKind,
    bands_count: usize,
    wet: f32,
    phase: f32,
    noise_state: u32,
    fs: f32,
}

struct VocoderBand {
    analysis: Biquad,
    synthesis: Biquad,
    env: f32,
}

impl ChannelVocoder {
    pub fn new(sample_rate: u32, bands_count: usize, carrier: CarrierKind, wet: f32) -> Self {
        let fs = sample_rate as f32;
        let low: f32 = 150.0;
        let high: f32 = 4500.0;
        let n = bands_count.max(2);
        let mut bands = Vec::with_capacity(n);
        for i in 0..n {
            let f = low * (high / low).powf(i as f32 / (n - 1) as f32);
            let f0: f32 = f;
            let q = 8.0;
            let analysis = Biquad::bandpass(sample_rate, f0, q);
            let synthesis = Biquad::bandpass(sample_rate, f0, q);
            bands.push(VocoderBand {
                analysis,
                synthesis,
                env: 0.0,
            });
        }
        Self {
            bands,
            carrier,
            bands_count: n,
            wet: wet.clamp(0.0, 1.0),
            phase: 0.0,
            noise_state: 0x2545F491,
            fs,
        }
    }

    #[inline]
    fn carrier_sample(&mut self) -> f32 {
        match self.carrier {
            CarrierKind::Noise => {
                let mut x = self.noise_state;
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                self.noise_state = x;
                x as f32 / u32::MAX as f32 * 2.0 - 1.0
            }
            CarrierKind::Saw => {
                let s = self.phase * 2.0 - 1.0;
                self.phase += 1.0 / self.fs * 120.0;
                if self.phase >= 1.0 {
                    self.phase -= 1.0;
                }
                s
            }
        }
    }
}

impl Effect for ChannelVocoder {
    fn process(&mut self, mono: &mut [f32]) {
        let wet = self.wet;
        let dry = 1.0 - wet;
        let attack = 0.06;
        let release = 0.0015;
        let inv_n = 1.0 / self.bands_count as f32;

        for s in mono.iter_mut() {
            let input = *s;
            let carrier = self.carrier_sample();

            let mut acc = 0.0;
            for band in &mut self.bands {
                let a = band.analysis.tick(input);
                let a_env = a.abs();
                if a_env > band.env {
                    band.env += (a_env - band.env) * attack;
                } else {
                    band.env += (a_env - band.env) * release;
                }
                acc += band.synthesis.tick(carrier) * band.env;
            }

            *s = input * dry + acc * wet * inv_n * 4.0;
        }
    }
}
