//! Lågändet: mono under en frekvens, hyllet och fas — bass managern.

use super::biquad::{Biquad, Kind};
use crate::audio::plugin_host::PluginCategory;

use super::{Dsp, EffectDef, ParamDef};

pub static BASS_MANAGER: EffectDef = EffectDef {
    id: "bassmanager",
    name: "Bass Manager",
    category: PluginCategory::Equalizer,
    description: "Mono under vald frekvens, hyllning av basen och fas på lågbandet",
    params: &[
        ParamDef::knob("Mono under Hz", 20.0, 400.0, 120.0),
        ParamDef::knob("Mono mängd", 0.0, 1.0, 1.0),
        ParamDef::knob("Bas dB", -12.0, 12.0, 0.0),
        ParamDef::step("Fas (0/1)", 0.0, 1.0, 0.0),
    ],
    make: |sr| Box::new(BassManager::new(sr)),
};

struct BassManager {
    sr: f32,
    freq: f32,
    mono: f32,
    db: f32,
    invert: bool,
    lp: [Butterworth2; 2],
    hp: [Butterworth2; 2],
    shelf: [Biquad; 2],
}

/// Två biquad i serie = 24 dB/okt. Lågbandet dras ifrån signalen, så summan är exakt torr
/// när mono och hylla är av — en delning som inte färgar något den inte ska.
#[derive(Clone, Copy, Default)]
struct Butterworth2 {
    a: Biquad,
    b: Biquad,
}

impl Butterworth2 {
    fn set(&mut self, sr: f32, freq: f32, kind: Kind) {
        self.a.set(kind, sr, freq, 0.0, 0.7071);
        self.b.set(kind, sr, freq, 0.0, 0.7071);
    }
    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        self.b.process(self.a.process(x))
    }
    fn reset(&mut self) {
        self.a.reset();
        self.b.reset();
    }
}

impl BassManager {
    fn new(sr: f32) -> Self {
        let mut b = Self {
            sr,
            freq: 120.0,
            mono: 1.0,
            db: 0.0,
            invert: false,
            lp: Default::default(),
            hp: Default::default(),
            shelf: [Biquad::default(); 2],
        };
        b.rebuild();
        b
    }

    fn rebuild(&mut self) {
        for ch in 0..2 {
            self.lp[ch].set(self.sr, self.freq, Kind::LowPass);
            self.hp[ch].set(self.sr, self.freq, Kind::HighPass);
            self.shelf[ch].set(Kind::LowShelf, self.sr, self.freq, self.db, 0.7071);
        }
    }
}

impl Dsp for BassManager {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        let sign = if self.invert { -1.0 } else { 1.0 };
        for i in 0..left.len() {
            let (l, r) = (left[i], right[i]);
            let low_l = self.lp[0].process(l);
            let low_r = self.lp[1].process(r);
            // Länkwitz-Riley: låg- och högbandet delas i var sin kaskad, inte som `x - låg`.
            // Ett subtraktivt högband hade burit kvar basenergi ur lågbandets fasvridning.
            let high_l = self.hp[0].process(l);
            let high_r = self.hp[1].process(r);
            // Monot: båda kanalerna får medelvärdet, i den mängd ratten säger.
            let mono_low = 0.5 * (low_l + low_r);
            let low_l = low_l + (mono_low - low_l) * self.mono;
            let low_r = low_r + (mono_low - low_r) * self.mono;
            let low_l = self.shelf[0].process(low_l) * sign;
            let low_r = self.shelf[1].process(low_r) * sign;
            left[i] = low_l + high_l;
            right[i] = low_r + high_r;
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.freq = value as f32,
            1 => self.mono = value as f32,
            2 => self.db = value as f32,
            3 => self.invert = value >= 0.5,
            _ => {}
        }
        if index <= 2 {
            self.rebuild();
        }
    }

    fn reset(&mut self) {
        for ch in 0..2 {
            self.lp[ch].reset();
            self.hp[ch].reset();
            self.shelf[ch].reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    /// Motsatt fas i basen försvinner när lågbandet blir mono; diskanten rörs inte.
    #[test]
    fn mono_below_the_frequency_removes_an_out_of_phase_bass() {
        let sr = 48_000.0;
        let n = 48_000;
        let make = |f: f32| {
            let l: Vec<f32> = (0..n).map(|i| 0.5 * (2.0 * PI * f * i as f32 / sr).sin()).collect();
            let r: Vec<f32> = l.iter().map(|v| -v).collect();
            (l, r)
        };

        let mut bm = BassManager::new(sr);
        let (mut l, mut r) = make(50.0);
        bm.process(&mut l, &mut r);
        let bass = rms(&l[n / 2..]);
        assert!(bass < 0.02, "bas kvar: {bass:.4}");

        let mut bm = BassManager::new(sr);
        let (mut l, mut r) = make(1_000.0);
        bm.process(&mut l, &mut r);
        let high = rms(&l[n / 2..]);
        assert!((high - 0.5 / 2.0_f32.sqrt()).abs() < 0.02, "diskant: {high:.4}");
    }

    /// Med mono och hylla av ska delningen vara nivåneutral — LR-summan är allpass, inte
    /// sample-identisk, men den får inte ändra styrkan.
    #[test]
    fn the_default_split_keeps_the_level_when_mono_is_off() {
        let sr = 48_000.0;
        let input: Vec<f32> = (0..9_600).map(|i| 0.3 * (2.0 * PI * 440.0 * i as f32 / sr).sin()).collect();
        let mut bm = BassManager::new(sr);
        bm.set(1, 0.0);
        let (mut l, mut r) = (input.clone(), input.clone());
        bm.process(&mut l, &mut r);
        let ratio = rms(&l[4_800..]) / rms(&input[4_800..]);
        assert!((ratio - 1.0).abs() < 0.01, "nivåförhållande {ratio:.4}");
    }
}
