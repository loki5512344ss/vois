//! Freeverb-style reverb.

use super::super::Effect;

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
