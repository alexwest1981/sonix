//! Modulation och tonhöjd: ringmodulator och en tvåtaps pitchshifter.

use std::f32::consts::PI;

use crate::audio::plugin_host::PluginCategory;

use super::{Dsp, EffectDef, ParamDef};

// ---------------------------------------------------------------------------
// Ringmodulator
// ---------------------------------------------------------------------------

pub static RING_MOD: EffectDef = EffectDef {
    id: "ringmod",
    name: "Ringmodulator",
    category: PluginCategory::Effect,
    description: "Multiplicerar signalen med en egen oscillator — summa och differens, ingen bärvåg kvar",
    params: &[
        ParamDef::knob("Frekvens", 0.1, 5_000.0, 440.0),
        ParamDef::knob("Mix", 0.0, 1.0, 1.0),
        ParamDef::knob("Stereofas grader", 0.0, 180.0, 0.0),
    ],
    make: |sr| Box::new(RingMod::new(sr)),
};

struct RingMod {
    sr: f32,
    freq: f32,
    mix: f32,
    offset: f32,
    phase: [f32; 2],
}

impl RingMod {
    fn new(sr: f32) -> Self {
        Self { sr, freq: 440.0, mix: 1.0, offset: 0.0, phase: [0.0; 2] }
    }
}

impl Dsp for RingMod {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let step = 2.0 * PI * self.freq / self.sr;
        let offset = self.offset.to_radians();
        for i in 0..left.len() {
            let car_l = self.phase[0].sin();
            let car_r = (self.phase[1] + offset).sin();
            self.phase[0] += step;
            self.phase[1] += step;
            if self.phase[0] > 2.0 * PI {
                self.phase[0] -= 2.0 * PI;
            }
            if self.phase[1] > 2.0 * PI {
                self.phase[1] -= 2.0 * PI;
            }
            let dry_l = left[i];
            let dry_r = right[i];
            left[i] = dry_l + (dry_l * car_l - dry_l) * self.mix;
            right[i] = dry_r + (dry_r * car_r - dry_r) * self.mix;
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.freq = value as f32,
            1 => self.mix = value as f32,
            2 => self.offset = value as f32,
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.phase = [0.0; 2];
    }
}

// ---------------------------------------------------------------------------
// Pitchshifter
// ---------------------------------------------------------------------------

pub static PITCH: EffectDef = EffectDef {
    id: "pitch",
    name: "Pitch",
    category: PluginCategory::PitchCorrection,
    description: "Realtidspitch ±12 semitoner med två korsande läspunkter",
    params: &[
        ParamDef::knob("Semitoner", -12.0, 12.0, 0.0),
        ParamDef::knob("Fönster ms", 20.0, 200.0, 50.0),
        ParamDef::knob("Mix", 0.0, 1.0, 1.0),
    ],
    make: |sr| Box::new(Pitch::new(sr)),
};

struct Pitch {
    sr: f32,
    semis: f32,
    window_ms: f32,
    mix: f32,
    buf: [Vec<f32>; 2],
    write: usize,
    phase: f32,
}

impl Pitch {
    fn new(sr: f32) -> Self {
        let cap = (0.2 * sr) as usize + 4;
        Self {
            sr,
            semis: 0.0,
            window_ms: 50.0,
            mix: 1.0,
            buf: [vec![0.0; cap], vec![0.0; cap]],
            write: 0,
            phase: 0.0,
        }
    }

    fn window(&self) -> f32 {
        (self.window_ms * 0.001 * self.sr).clamp(64.0, (self.buf[0].len() - 4) as f32)
    }

    fn read(buf: &[f32], write: usize, delay: f32) -> f32 {
        let cap = buf.len() as f32;
        let mut idx = write as f32 - delay;
        while idx < 0.0 {
            idx += cap;
        }
        let i0 = idx.floor() as usize % buf.len();
        let i1 = (i0 + 1) % buf.len();
        let frac = idx - idx.floor();
        buf[i0] * (1.0 - frac) + buf[i1] * frac
    }
}

impl Dsp for Pitch {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let rate = 2.0_f32.powf(self.semis / 12.0);
        // `shortcut: rate nära 1 lämnas orörd — två statiska läspunkter hade blivit en kamfilter-
        // komb i stället för rent ljud. Uppgradera till fas-vocoder om små intervall måste vara
        // artefaktfria.`
        if (rate - 1.0).abs() < 1.0e-4 {
            return;
        }
        let win = self.window();
        let step = (1.0 - rate) / win;
        for i in 0..left.len() {
            for ch in 0..2 {
                let buf = &mut self.buf[ch];
                let x = if ch == 0 { left[i] } else { right[i] };
                buf[self.write] = x;
            }
            let p = self.phase;
            let d1 = p * win;
            let d2 = ((p + 0.5) % 1.0) * win;
            let g1 = (PI * p).sin();
            let g2 = (PI * (p + 0.5)).sin();
            let out_l = Pitch::read(&self.buf[0], self.write, d1) * g1
                + Pitch::read(&self.buf[0], self.write, d2) * g2;
            let out_r = Pitch::read(&self.buf[1], self.write, d1) * g1
                + Pitch::read(&self.buf[1], self.write, d2) * g2;
            left[i] += (out_l - left[i]) * self.mix;
            right[i] += (out_r - right[i]) * self.mix;
            self.phase += step;
            if self.phase >= 1.0 {
                self.phase -= 1.0;
            } else if self.phase < 0.0 {
                self.phase += 1.0;
            }
            self.write = (self.write + 1) % self.buf[0].len();
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.semis = value as f32,
            1 => self.window_ms = value as f32,
            2 => self.mix = value as f32,
            _ => {}
        }
    }

    fn reset(&mut self) {
        for b in &mut self.buf {
            b.iter_mut().for_each(|v| *v = 0.0);
        }
        self.write = 0;
        self.phase = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    /// Ren multiplikation: likström in blir bärvågen ut, med sinusens RMS.
    #[test]
    fn the_ring_modulator_multiplies_by_its_carrier() {
        let sr = 48_000.0;
        let mut rm = RingMod::new(sr);
        rm.set(0, 100.0);
        let mut l = vec![1.0f32; 24_000];
        let mut r = vec![1.0f32; 24_000];
        rm.process(&mut l, &mut r);
        let level = rms(&l[2_000..]);
        assert!((level - 0.707).abs() < 0.05, "nivå {level:.3}");
        let peak = l.iter().fold(0.0f32, |a, v| a.max(v.abs()));
        assert!((peak - 1.0).abs() < 0.05, "topp {peak:.3}");
    }

    /// En oktav upp ska ge dubbla frekvensen — mätt som nollgenomgångar.
    #[test]
    fn twelve_semitones_up_doubles_the_frequency() {
        let sr = 48_000.0;
        let n = 48_000;
        let input: Vec<f32> = (0..n).map(|i| 0.5 * (2.0 * PI * 440.0 * i as f32 / sr).sin()).collect();
        let mut p = Pitch::new(sr);
        p.set(0, 12.0);
        let (mut l, mut r) = (input.clone(), input.clone());
        p.process(&mut l, &mut r);
        let half = n / 2;
        let crossings = l[half..].windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        let freq = crossings as f32 * sr / half as f32;
        assert!((freq - 880.0).abs() < 44.0, "frekvens {freq:.0} Hz");
    }

    #[test]
    fn zero_semitones_is_passthrough() {
        let sr = 48_000.0;
        let input: Vec<f32> = (0..2_000).map(|i| (i as f32 * 0.01).sin()).collect();
        let mut p = Pitch::new(sr);
        let (mut l, mut r) = (input.clone(), input.clone());
        p.process(&mut l, &mut r);
        assert_eq!(l, input);
    }
}
