//! Filter och ton: branta filter, tilt, stege och fasrotation.

use std::f32::consts::PI;

use super::biquad::{Biquad, Butterworth, Kind};
use crate::audio::filter_topologies::{FilterDsp, FilterMode, FilterTopology};
use crate::audio::plugin_host::PluginCategory;

use super::{Dsp, EffectDef, ParamDef};

// ---------------------------------------------------------------------------
// Filter 24/48/72 dB per oktav
// ---------------------------------------------------------------------------

pub static FILTER72: EffectDef = EffectDef {
    id: "filter72",
    name: "Filter 72",
    category: PluginCategory::Equalizer,
    description: "Butterworth låg-/högpass med 24, 48 eller 72 dB per oktav",
    params: &[
        ParamDef::step("Typ (0=låg, 1=steg)", 0.0, 1.0, 0.0),
        ParamDef::step("Brantic (0=24, 1=48, 2=72 dB)", 0.0, 2.0, 0.0),
        ParamDef::knob("Frekvens", 20.0, 20_000.0, 1_000.0),
        ParamDef::knob("Utgång dB", -24.0, 24.0, 0.0),
    ],
    make: |sr| Box::new(Filter72::new(sr)),
};

struct Filter72 {
    sr: f32,
    lp: [Butterworth; 2],
    hp: [Butterworth; 2],
    highpass: bool,
    order: usize,
    freq: f32,
    gain: f32,
}

impl Filter72 {
    fn new(sr: f32) -> Self {
        let mut f = Self {
            sr,
            lp: Default::default(),
            hp: Default::default(),
            highpass: false,
            order: 4,
            freq: 1_000.0,
            gain: 1.0,
        };
        f.rebuild();
        f
    }

    fn rebuild(&mut self) {
        for ch in 0..2 {
            self.lp[ch].set(Kind::LowPass, self.sr, self.freq, self.order);
            self.hp[ch].set(Kind::HighPass, self.sr, self.freq, self.order);
        }
    }
}

impl Dsp for Filter72 {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for ch in 0..2 {
            let buf = if ch == 0 { &mut *left } else { &mut *right };
            let mut state = if self.highpass { self.hp[ch] } else { self.lp[ch] };
            for x in buf.iter_mut() {
                *x = state.process(*x) * self.gain;
            }
            if self.highpass {
                self.hp[ch] = state;
            } else {
                self.lp[ch] = state;
            }
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.highpass = value >= 0.5,
            1 => self.order = [4, 8, 12][(value as usize).min(2)],
            2 => self.freq = value as f32,
            3 => self.gain = 10.0_f32.powf(value as f32 / 20.0),
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
        }
    }
}

// ---------------------------------------------------------------------------
// Tilt
// ---------------------------------------------------------------------------

pub static TILT: EffectDef = EffectDef {
    id: "tilt",
    name: "Tilt",
    category: PluginCategory::Equalizer,
    description: "Lutar hela spektrat kring en vridpunkt: bas ned, diskant upp (eller tvärtom)",
    params: &[
        ParamDef::knob("Vridpunkt", 100.0, 8_000.0, 700.0),
        ParamDef::knob("Lutning dB", -12.0, 12.0, 0.0),
    ],
    make: |sr| Box::new(Tilt::new(sr)),
};

struct Tilt {
    sr: f32,
    pivot: f32,
    db: f32,
    stages: [[Biquad; 2]; 2],
}

impl Tilt {
    fn new(sr: f32) -> Self {
        let mut t = Self { sr, pivot: 700.0, db: 0.0, stages: [[Biquad::default(); 2]; 2] };
        t.rebuild();
        t
    }

    fn rebuild(&mut self) {
        for ch in 0..2 {
            self.stages[ch][0].set(Kind::LowShelf, self.sr, self.pivot, -self.db, 0.7071);
            self.stages[ch][1].set(Kind::HighShelf, self.sr, self.pivot, self.db, 0.7071);
        }
    }
}

impl Dsp for Tilt {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for ch in 0..2 {
            let buf = if ch == 0 { &mut *left } else { &mut *right };
            let [low, high] = &mut self.stages[ch];
            for x in buf.iter_mut() {
                *x = high.process(low.process(*x));
            }
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.pivot = value as f32,
            1 => self.db = value as f32,
            _ => {}
        }
        self.rebuild();
    }

    fn reset(&mut self) {
        for ch in 0..2 {
            self.stages[ch][0].reset();
            self.stages[ch][1].reset();
        }
    }
}

// ---------------------------------------------------------------------------
// Moog-stege
// ---------------------------------------------------------------------------

pub static MOOG: EffectDef = EffectDef {
    id: "moog",
    name: "Stege 24",
    category: PluginCategory::Equalizer,
    description: "Fyrpolig ladder-lågpass med mättnad i återkopplingen — karaktär, inte bara branthet",
    params: &[
        ParamDef::knob("Cutoff", 20.0, 20_000.0, 1_200.0),
        ParamDef::knob("Resonans", 0.0, 1.0, 0.35),
        ParamDef::knob("Drive dB", 0.0, 30.0, 0.0),
        ParamDef::knob("Mix", 0.0, 1.0, 1.0),
    ],
    make: |sr| Box::new(Moog::new(sr)),
};

struct Moog {
    sr: f32,
    cutoff: f32,
    res: f32,
    drive: f32,
    mix: f32,
    state: [[f32; 4]; 2],
}

impl Moog {
    fn new(sr: f32) -> Self {
        Self { sr, cutoff: 1_200.0, res: 0.35, drive: 1.0, mix: 1.0, state: [[0.0; 4]; 2] }
    }
}

impl Dsp for Moog {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        // En pol per sampel: g ställs om per block, vilket är hela poängen med billiga stegar.
        let g = 1.0 - (-2.0 * PI * self.cutoff.clamp(20.0, self.sr * 0.45) / self.sr).exp();
        let fb = self.res * 3.9;
        for ch in 0..2 {
            let buf = if ch == 0 { &mut *left } else { &mut *right };
            let s = &mut self.state[ch];
            for x in buf.iter_mut() {
                let dry = *x;
                let u = ((dry * self.drive) - fb * (s[3] - 0.5 * dry)).tanh();
                s[0] += g * (u - s[0]);
                s[1] += g * (s[0] - s[1]);
                s[2] += g * (s[1] - s[2]);
                s[3] += g * (s[2] - s[3]);
                *x = dry + (s[3] - dry) * self.mix;
            }
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.cutoff = value as f32,
            1 => self.res = value as f32,
            2 => self.drive = 10.0_f32.powf(value as f32 / 20.0),
            3 => self.mix = value as f32,
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.state = [[0.0; 4]; 2];
    }
}

// ---------------------------------------------------------------------------
// Cytomic-topologier (Sallen-Key, SVF 2x, ladder med poldiffusion)
// ---------------------------------------------------------------------------

pub static CYTOMIC: EffectDef = EffectDef {
    id: "cytomic",
    name: "Cytomic-filter",
    category: PluginCategory::Equalizer,
    description: "Sallen-Key 12 dB, SVF med 2x oversampling och ladder med poldiffusion — drive i resonansåterkopplingen",
    params: &[
        ParamDef::step("Topologi (0=Sallen-Key, 1=SVF, 2=ladder)", 0.0, 2.0, 0.0),
        ParamDef::step("Typ (0=låg, 1=band, 2=hög, 3=notch; ladder har bara låg)", 0.0, 3.0, 0.0),
        ParamDef::knob("Cutoff", 20.0, 20_000.0, 1_000.0),
        ParamDef::knob("Resonans", 0.0, 1.0, 0.3),
        ParamDef::knob("Drive", 0.0, 1.0, 0.0),
    ],
    make: |sr| Box::new(Cytomic::new(sr)),
};

struct Cytomic {
    sr: f32,
    topo: FilterTopology,
    mode: FilterMode,
    cutoff: f32,
    res: f32,
    drive: f32,
    dsp: Box<dyn FilterDsp>,
}

impl Cytomic {
    fn new(sr: f32) -> Self {
        let topo = FilterTopology::SallenKeyMs;
        let mut c = Self {
            sr,
            topo,
            mode: FilterMode::LowPass,
            cutoff: 1_000.0,
            res: 0.3,
            drive: 0.0,
            dsp: topo.make(sr),
        };
        c.apply();
        c
    }

    fn apply(&mut self) {
        self.dsp.set_mode(self.mode);
        self.dsp.set(self.cutoff, self.res, self.drive);
    }
}

impl Dsp for Cytomic {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        self.dsp.process_stereo(left, right);
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => {
                let topo = FilterTopology::ALL[(value as usize).min(2)];
                if topo != self.topo {
                    self.topo = topo;
                    // Ny topologi = nytt tillstånd, som att byta krets i en burk.
                    self.dsp = topo.make(self.sr);
                }
            }
            1 => self.mode = [FilterMode::LowPass, FilterMode::BandPass, FilterMode::HighPass, FilterMode::Notch]
                [(value as usize).min(3)],
            2 => self.cutoff = value as f32,
            3 => self.res = value as f32,
            4 => self.drive = value as f32,
            _ => {}
        }
        self.apply();
    }

    fn reset(&mut self) {
        self.dsp.reset();
    }
}

// ---------------------------------------------------------------------------
// Fasrotation (allpass)
// ---------------------------------------------------------------------------

pub static PHASE_ROTATE: EffectDef = EffectDef {
    id: "phaserotate",
    name: "Fasrotation",
    category: PluginCategory::Equalizer,
    description: "Allpass som vrider fasen kring en frekvens — för att få två spår att summera",
    params: &[
        ParamDef::knob("Frekvens", 20.0, 4_000.0, 200.0),
        ParamDef::knob("Mängd", 0.0, 1.0, 1.0),
    ],
    make: |sr| Box::new(PhaseRotate::new(sr)),
};

struct PhaseRotate {
    sr: f32,
    freq: f32,
    amount: f32,
    ap: [Biquad; 2],
}

impl PhaseRotate {
    fn new(sr: f32) -> Self {
        let mut p = Self { sr, freq: 200.0, amount: 1.0, ap: [Biquad::default(); 2] };
        p.rebuild();
        p
    }

    fn rebuild(&mut self) {
        for ch in 0..2 {
            self.ap[ch].set(Kind::AllPass, self.sr, self.freq, 0.0, 0.7071);
        }
    }
}

impl Dsp for PhaseRotate {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for ch in 0..2 {
            let buf = if ch == 0 { &mut *left } else { &mut *right };
            let ap = &mut self.ap[ch];
            for x in buf.iter_mut() {
                let dry = *x;
                *x = dry + (ap.process(dry) - dry) * self.amount;
            }
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => {
                self.freq = value as f32;
                self.rebuild();
            }
            1 => self.amount = value as f32,
            _ => {}
        }
    }

    fn reset(&mut self) {
        for ap in &mut self.ap {
            ap.reset();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn tone(sr: f32, freq: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (2.0 * PI * freq * i as f32 / sr).sin()).collect()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    fn run(dsp: &mut dyn Dsp, input: &[f32]) -> Vec<f32> {
        let mut l = input.to_vec();
        let mut r = input.to_vec();
        dsp.process(&mut l, &mut r);
        l
    }

    fn db(a: f32, b: f32) -> f32 {
        20.0 * (a / b).log10()
    }

    /// 72 dB/okt dämpar en oktav över gränsen klart mer än 24 dB/okt gör.
    #[test]
    fn the_steep_filter_is_steeper_than_the_shallow_one() {
        let sr = 48_000.0;
        let input = tone(sr, 4_000.0, 0.5, 44_100);
        let half = input.len() / 2;

        let mut shallow = Filter72::new(sr);
        shallow.set(1, 0.0);
        shallow.set(2, 1_000.0);
        let mut steep = Filter72::new(sr);
        steep.set(1, 2.0);
        steep.set(2, 1_000.0);

        let a = run(&mut shallow, &input);
        let b = run(&mut steep, &input);
        let shallow_db = db(rms(&a[half..]), rms(&input[half..]));
        let steep_db = db(rms(&b[half..]), rms(&input[half..]));
        // Två oktaver över gränsen: 24 dB/okt ger ~-48 dB, 72 dB/okt ~-144 dB.
        assert!(shallow_db > -60.0 && shallow_db < -35.0, "24 dB: {shallow_db:.1}");
        assert!(steep_db < shallow_db - 30.0, "72 dB: {steep_db:.1} mot {shallow_db:.1}");
    }

    #[test]
    fn zero_db_tilt_changes_nothing() {
        let sr = 48_000.0;
        let input = tone(sr, 1_000.0, 0.5, 9_600);
        let mut t = Tilt::new(sr);
        t.set(1, 0.0);
        let out = run(&mut t, &input);
        let half = input.len() / 2;
        assert!((db(rms(&out[half..]), rms(&input[half..]))).abs() < 0.1);
    }

    /// Lutning kring 1 kHz: basen ned, diskanten upp när ratten är positiv.
    #[test]
    fn tilt_moves_the_two_ends_in_opposite_directions() {
        let sr = 48_000.0;
        let mut t = Tilt::new(sr);
        t.set(0, 700.0);
        t.set(1, 6.0);
        let low = run(&mut t, &tone(sr, 100.0, 0.5, 24_000));
        let mut t = Tilt::new(sr);
        t.set(0, 700.0);
        t.set(1, 6.0);
        let high = run(&mut t, &tone(sr, 8_000.0, 0.5, 24_000));
        let half = 12_000;
        let low_db = db(rms(&low[half..]), 0.5 / 2.0_f32.sqrt());
        let high_db = db(rms(&high[half..]), 0.5 / 2.0_f32.sqrt());
        assert!(low_db < -2.0, "bas: {low_db:.1} dB");
        assert!(high_db > 2.0, "diskant: {high_db:.1} dB");
    }

    #[test]
    fn the_ladder_stays_finite_at_full_resonance() {
        let sr = 48_000.0;
        let mut m = Moog::new(sr);
        m.set(1, 1.0);
        m.set(2, 24.0);
        let out = run(&mut m, &tone(sr, 220.0, 0.9, 48_000));
        let peak = out.iter().fold(0.0f32, |a, v| a.max(v.abs()));
        assert!(out.iter().all(|v| v.is_finite()) && peak < 8.0, "topp {peak}");
    }

    /// En allpass ändrar fasen men inte nivån: vid 200 Hz ska fasen vara nära 90°.
    #[test]
    fn the_allpass_rotates_phase_without_touching_level() {
        let sr = 48_000.0;
        let input = tone(sr, 200.0, 0.5, 44_100);
        let mut p = PhaseRotate::new(sr);
        p.set(1, 1.0);
        let out = run(&mut p, &input);
        let half = input.len() / 2;
        assert!(db(rms(&out[half..]), rms(&input[half..])).abs() < 0.5);

        let w = 2.0 * PI * 200.0 / sr;
        let (mut s, mut c) = (0.0f32, 0.0f32);
        for (i, y) in out.iter().enumerate().skip(half) {
            s += y * (w * i as f32).sin();
            c += y * (w * i as f32).cos();
        }
        let phase = c.atan2(s).to_degrees().abs();
        // Andra ordningens allpass vrider 180° vid gränsfrekvensen.
        assert!((150.0..=180.0).contains(&phase), "fas {phase:.0}°");
    }
}
