//! Dynamik: spektral kompressor (STFT) och Inflator (udda ordningens vågformning).

use std::collections::VecDeque;
use std::f32::consts::PI;
use std::sync::Arc;

use rustfft::num_complex::Complex32;
use rustfft::{Fft, FftPlanner};

use crate::audio::plugin_host::PluginCategory;

use super::{Dsp, EffectDef, ParamDef};

// ---------------------------------------------------------------------------
// Spektral kompressor
// ---------------------------------------------------------------------------

pub static SPECTRAL_COMP: EffectDef = EffectDef {
    id: "spectralcomp",
    name: "Spektral kompressor",
    category: PluginCategory::Compressor,
    description: "Komprimering i frekvensdomänen: frekvensberoende tröskel, nedåt- och uppåtkoppling",
    params: &[
        ParamDef::knob("Tröskel dB", -60.0, 0.0, -20.0),
        ParamDef::knob("Lutning dB/okt", -12.0, 12.0, 0.0),
        ParamDef::knob("Nedåt ratio", 1.0, 20.0, 4.0),
        ParamDef::knob("Uppåt ratio", 1.0, 10.0, 1.0),
        ParamDef::knob("Knee dB", 0.0, 24.0, 6.0),
        ParamDef::knob("Attack ms", 1.0, 200.0, 10.0),
        ParamDef::knob("Release ms", 10.0, 2_000.0, 150.0),
        ParamDef::knob("Mix", 0.0, 1.0, 1.0),
        ParamDef::knob("Utgång dB", -24.0, 24.0, 0.0),
    ],
    make: |sr| Box::new(SpectralComp::new(sr)),
};

const FFT_N: usize = 1024;
const HOP: usize = FFT_N / 4;
const BINS: usize = FFT_N / 2 + 1;

struct SpectralComp {
    sr: f32,
    threshold: f32,
    slope: f32,
    down: f32,
    up: f32,
    knee: f32,
    attack: f32,
    release: f32,
    mix: f32,
    out_gain: f32,

    fft: Arc<dyn Fft<f32>>,
    ifft: Arc<dyn Fft<f32>>,
    window: [f32; FFT_N],
    /// Ringbuffer med de senaste N inspelade samplen per kanal.
    hist: [[f32; FFT_N]; 2],
    /// `hist`-index för den **äldsta** sampeln.
    oldest: usize,
    since_hop: usize,
    /// Överlappande utdata, N sampel per kanal.
    ola: [[f32; FFT_N]; 2],
    /// Färdiga utsampel som ännu inte lämnat effekten.
    queue: [VecDeque<f32>; 2],
    /// Utjämnad linjär förstärkning per bin (länkad mellan kanalerna).
    gain: [f32; BINS],
    /// Tröskeln per bin i dB, omräknad när tröskel eller lutning ändras.
    threshold_db: [f32; BINS],
}

impl SpectralComp {
    fn new(sr: f32) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_N);
        let ifft = planner.plan_fft_inverse(FFT_N);
        let mut window = [0.0f32; FFT_N];
        for (i, w) in window.iter_mut().enumerate() {
            *w = 0.5 - 0.5 * (2.0 * PI * i as f32 / FFT_N as f32).cos();
        }
        let mut s = Self {
            sr,
            threshold: -20.0,
            slope: 0.0,
            down: 4.0,
            up: 1.0,
            knee: 6.0,
            attack: 10.0,
            release: 150.0,
            mix: 1.0,
            out_gain: 1.0,
            fft,
            ifft,
            window,
            hist: [[0.0; FFT_N]; 2],
            oldest: 0,
            since_hop: 0,
            ola: [[0.0; FFT_N]; 2],
            queue: [VecDeque::with_capacity(FFT_N), VecDeque::with_capacity(FFT_N)],
            gain: [1.0; BINS],
            threshold_db: [0.0; BINS],
        };
        s.rebuild_thresholds();
        s
    }

    fn rebuild_thresholds(&mut self) {
        for (k, t) in self.threshold_db.iter_mut().enumerate() {
            let f = (k as f32 * self.sr / FFT_N as f32).max(20.0);
            *t = self.threshold + self.slope * (f / 1_000.0).log2();
        }
    }

    /// Kör en bildruta: analys, binvis förstärkning, syntes och överlapp-addering.
    fn frame(&mut self) {
        let atk = (-(HOP as f32) / (self.attack * 0.001 * self.sr)).exp();
        let rel = (-(HOP as f32) / (self.release * 0.001 * self.sr)).exp();

        let mut spectra = [[Complex32::new(0.0, 0.0); BINS]; 2];
        for ch in 0..2 {
            let mut buf = [Complex32::new(0.0, 0.0); FFT_N];
            for i in 0..FFT_N {
                let x = self.hist[ch][(self.oldest + i) % FFT_N];
                buf[i] = Complex32::new(self.window[i] * x, 0.0);
            }
            self.fft.process(&mut buf);
            spectra[ch].copy_from_slice(&buf[..BINS]);
        }

        // Fönstrets summa är N/2, så en sinus med amplitud A får toppbin A·N/4 — det är
        // normaliseringen som gör dB till dBFS och inte till "FFT-enheter".
        let level_norm = FFT_N as f32 / 4.0;
        for k in 0..BINS {
            let level = spectra[0][k].norm().max(spectra[1][k].norm()) / level_norm;
            let level_db = 20.0 * (level + 1.0e-9).log10();
            let t = self.threshold_db[k];
            let over = level_db - t;
            let mut gr_db = 0.0f32;
            if self.knee > 0.0 && over.abs() < self.knee * 0.5 {
                let x = over + self.knee * 0.5;
                gr_db = (1.0 / self.down - 1.0) * (x * x) / (2.0 * self.knee);
            } else if over >= 0.0 {
                gr_db = over * (1.0 / self.down - 1.0);
            } else if self.up > 1.0 {
                gr_db = -over * (1.0 - 1.0 / self.up);
            }
            let target = 10.0_f32.powf(gr_db / 20.0);
            let g = &mut self.gain[k];
            let coeff = if target < *g { atk } else { rel };
            *g = target + (*g - target) * coeff;
        }

        // Syntes: spektrat skalas, och den negativa halvan får samma förstärkning så
        // resultatet förblir reellt.
        let scale = 1.0 / (FFT_N as f32 * 1.5);
        for ch in 0..2 {
            let mut buf = [Complex32::new(0.0, 0.0); FFT_N];
            for k in 0..BINS {
                buf[k] = spectra[ch][k] * self.gain[k];
            }
            // Negativa halvan är spegelvänd och konjugerad, så signalen förblir reell.
            for k in 1..FFT_N / 2 {
                buf[FFT_N - k] = buf[k].conj();
            }
            self.ifft.process(&mut buf);
            for i in 0..FFT_N {
                self.ola[ch][i] += buf[i].re * scale * self.window[i];
            }
        }

        for ch in 0..2 {
            for i in 0..HOP {
                self.queue[ch].push_back(self.ola[ch][i] * self.out_gain);
            }
            self.ola[ch].copy_within(HOP.., 0);
            for v in self.ola[ch][FFT_N - HOP..].iter_mut() {
                *v = 0.0;
            }
        }
    }
}

impl Dsp for SpectralComp {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for i in 0..left.len() {
            let write = self.oldest;
            self.hist[0][write] = left[i];
            self.hist[1][write] = right[i];
            self.oldest = (write + 1) % FFT_N;
            self.since_hop += 1;
            if self.since_hop == HOP {
                self.since_hop = 0;
                self.frame();
            }
            let (dry_l, dry_r) = (left[i], right[i]);
            let wet_l = self.queue[0].pop_front().unwrap_or(dry_l);
            let wet_r = self.queue[1].pop_front().unwrap_or(dry_r);
            left[i] = dry_l + (wet_l - dry_l) * self.mix;
            right[i] = dry_r + (wet_r - dry_r) * self.mix;
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => {
                self.threshold = value as f32;
                self.rebuild_thresholds();
            }
            1 => {
                self.slope = value as f32;
                self.rebuild_thresholds();
            }
            2 => self.down = (value as f32).max(1.0),
            3 => self.up = (value as f32).max(1.0),
            4 => self.knee = value as f32,
            5 => self.attack = (value as f32).max(0.1),
            6 => self.release = (value as f32).max(1.0),
            7 => self.mix = value as f32,
            8 => self.out_gain = 10.0_f32.powf(value as f32 / 20.0),
            _ => {}
        }
    }

    fn reset(&mut self) {
        self.hist = [[0.0; FFT_N]; 2];
        self.ola = [[0.0; FFT_N]; 2];
        self.queue[0].clear();
        self.queue[1].clear();
        self.oldest = 0;
        self.since_hop = 0;
        self.gain = [1.0; BINS];
    }

    fn latency(&self) -> u32 {
        (FFT_N - HOP) as u32
    }
}

// ---------------------------------------------------------------------------
// Inflator
// ---------------------------------------------------------------------------

pub static INFLATOR: EffectDef = EffectDef {
    id: "inflator",
    name: "Inflator",
    category: PluginCategory::Mastering,
    description: "Udda ordningens vågformning: mer tryck utan att toppen går över",
    params: &[
        ParamDef::knob("Drive dB", 0.0, 24.0, 6.0),
        ParamDef::knob("Kurva", 0.0, 1.0, 0.5),
        ParamDef::knob("Utgång dB", -24.0, 12.0, 0.0),
        ParamDef::knob("Mix", 0.0, 1.0, 1.0),
    ],
    make: |sr| Box::new(Inflator::new(sr)),
};

struct Inflator {
    drive: f32,
    curve: f32,
    out: f32,
    mix: f32,
}

impl Inflator {
    fn new(_sr: f32) -> Self {
        Self { drive: 2.0, curve: 0.5, out: 1.0, mix: 1.0 }
    }

    #[inline]
    fn shape(&self, x: f32) -> f32 {
        let d = x * self.drive;
        // Udda funktion: bara udda övertoner, och kurvan gör att RMS närmar sig toppen.
        (d * (1.0 + 2.0 * self.curve * d * d)).tanh()
    }
}

impl Dsp for Inflator {
    fn process(&mut self, left: &mut [f32], right: &mut [f32]) {
        for x in left.iter_mut() {
            let dry = *x;
            *x = dry + (self.shape(dry) * self.out - dry) * self.mix;
        }
        for x in right.iter_mut() {
            let dry = *x;
            *x = dry + (self.shape(dry) * self.out - dry) * self.mix;
        }
    }

    fn set(&mut self, index: usize, value: f64) {
        match index {
            0 => self.drive = 10.0_f32.powf(value as f32 / 20.0),
            1 => self.curve = value as f32,
            2 => self.out = 10.0_f32.powf(value as f32 / 20.0),
            3 => self.mix = value as f32,
            _ => {}
        }
    }

    fn reset(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    fn tone(sr: f32, freq: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (2.0 * PI * freq * i as f32 / sr).sin()).collect()
    }

    fn run(dsp: &mut dyn Dsp, input: &[f32]) -> Vec<f32> {
        let mut l = input.to_vec();
        let mut r = input.to_vec();
        dsp.process(&mut l, &mut r);
        l
    }

    /// Ratio 1 är ingen kompression: då ska STFT:n rekonstruera signalen oförändrad.
    #[test]
    fn ratio_one_leaves_the_signal_alone() {
        let sr = 48_000.0;
        let input = tone(sr, 1_000.0, 0.3, 48_000);
        let mut c = SpectralComp::new(sr);
        c.set(2, 1.0);
        c.set(3, 1.0);
        let out = run(&mut c, &input);
        let ratio = rms(&out[24_000..]) / rms(&input[24_000..]);
        assert!((ratio - 1.0).abs() < 0.05, "förhållande {ratio:.3}");
        assert_eq!(c.latency(), 768);
    }

    /// En ton 19,5 dB över tröskeln med ratio 4 ska ned ~14,6 dB.
    #[test]
    fn a_tone_above_the_threshold_is_compressed_by_the_ratio() {
        let sr = 48_000.0;
        let input = tone(sr, 1_000.0, 0.3, 48_000);
        let mut c = SpectralComp::new(sr);
        c.set(0, -30.0);
        c.set(4, 0.0);
        c.set(5, 1.0);
        c.set(6, 50.0);
        let out = run(&mut c, &input);
        let level = rms(&out[24_000..]);
        let expected = 0.3 * 10.0_f32.powf(-14.6 / 20.0);
        assert!((level - expected).abs() < expected * 0.35, "nivå {level:.3}, väntat {expected:.3}");
    }

    /// Mätning, inte ett krav: hur många gånger realtid klarar en instans i releasebygget?
    /// `cargo test --release --bin sonix spectral_comp_speed -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn spectral_comp_speed() {
        let sr = 48_000.0;
        let seconds = 10;
        let input = tone(sr, 1_000.0, 0.3, sr as usize * seconds);
        let mut c = SpectralComp::new(sr);
        let start = std::time::Instant::now();
        let out = run(&mut c, &input);
        let elapsed = start.elapsed().as_secs_f32();
        println!(
            "spectralcomp: {:.1}x realtid ({} s ljud på {:.2} s), ut-RMS {:.3}",
            seconds as f32 / elapsed,
            seconds,
            elapsed,
            rms(&out)
        );
    }

    /// Inflatorn ska minska toppfaktorn: mer tryck, toppen kvar under 1.
    #[test]
    fn the_inflator_lowers_the_crest_factor() {
        let sr = 48_000.0;
        let input = tone(sr, 220.0, 0.5, 24_000);
        let crest_in = input.iter().fold(0.0f32, |a, v| a.max(v.abs())) / rms(&input);
        let mut inf = Inflator::new(sr);
        inf.set(0, 6.0);
        inf.set(1, 0.5);
        let out = run(&mut inf, &input);
        let crest_out = out.iter().fold(0.0f32, |a, v| a.max(v.abs())) / rms(&out);
        assert!(crest_out < crest_in * 0.95, "toppfaktor {crest_out:.3} mot {crest_in:.3}");
        assert!(out.iter().fold(0.0f32, |a, v| a.max(v.abs())) <= 1.0);
    }
}
