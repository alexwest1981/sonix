//! Real-time master bus FX chain and per-track equalizer DSP.
//!
//! Everything here is allocation-free while processing and designed to run on
//! the audio thread. The parameters are plain `Copy` structs so they can be
//! shipped through the lock-free command ring buffer without cloning buffers.

use std::f32::consts::PI;
use std::sync::Arc;

use super::vocal_harmonizer::fft_radix2;

// ---------------------------------------------------------------------------
// Biquad building blocks
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Default)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl BiquadCoeffs {
    fn bypass() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0 }
    }
}

#[derive(Clone, Copy, Default)]
struct BiquadState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadState {
    #[inline(always)]
    fn process(&mut self, x: f32, c: &BiquadCoeffs) -> f32 {
        let y = c.b0 * x + c.b1 * self.x1 + c.b2 * self.x2 - c.a1 * self.y1 - c.a2 * self.y2;
        self.x2 = self.x1;
        self.x1 = x;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }
}

fn peaking(sr: f32, freq: f32, gain_db: f32, q: f32) -> BiquadCoeffs {
    if gain_db.abs() < 0.001 {
        return BiquadCoeffs::bypass();
    }
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq.clamp(20.0, sr * 0.45) / sr;
    let cos_w0 = w0.cos();
    let alpha = w0.sin() / (2.0 * q.max(0.05));
    let a0 = 1.0 + alpha / a;
    BiquadCoeffs {
        b0: (1.0 + alpha * a) / a0,
        b1: (-2.0 * cos_w0) / a0,
        b2: (1.0 - alpha * a) / a0,
        a1: (-2.0 * cos_w0) / a0,
        a2: (1.0 - alpha / a) / a0,
    }
}

fn low_shelf(sr: f32, freq: f32, gain_db: f32) -> BiquadCoeffs {
    if gain_db.abs() < 0.001 {
        return BiquadCoeffs::bypass();
    }
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq.clamp(20.0, sr * 0.45) / sr;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let sqrt_a = a.sqrt();
    let alpha = sin_w0 / 2.0 * (2.0_f32).sqrt();
    let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
    BiquadCoeffs {
        b0: a * ((a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha) / a0,
        b1: 2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0) / a0,
        b2: a * ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha) / a0,
        a1: -2.0 * ((a - 1.0) + (a + 1.0) * cos_w0) / a0,
        a2: ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha) / a0,
    }
}

fn high_shelf(sr: f32, freq: f32, gain_db: f32) -> BiquadCoeffs {
    if gain_db.abs() < 0.001 {
        return BiquadCoeffs::bypass();
    }
    let a = 10.0_f32.powf(gain_db / 40.0);
    let w0 = 2.0 * PI * freq.clamp(20.0, sr * 0.45) / sr;
    let cos_w0 = w0.cos();
    let sin_w0 = w0.sin();
    let sqrt_a = a.sqrt();
    let alpha = sin_w0 / 2.0 * (2.0_f32).sqrt();
    let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha;
    BiquadCoeffs {
        b0: a * ((a + 1.0) + (a - 1.0) * cos_w0 + 2.0 * sqrt_a * alpha) / a0,
        b1: -2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0) / a0,
        b2: a * ((a + 1.0) + (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha) / a0,
        a1: 2.0 * ((a - 1.0) - (a + 1.0) * cos_w0) / a0,
        a2: ((a + 1.0) - (a - 1.0) * cos_w0 - 2.0 * sqrt_a * alpha) / a0,
    }
}

// ---------------------------------------------------------------------------
// Per-track 3-band equalizer
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrackEqSettings {
    pub enabled: bool,
    pub low_freq: f32,
    pub low_gain_db: f32,
    pub mid_freq: f32,
    pub mid_gain_db: f32,
    pub mid_q: f32,
    pub high_freq: f32,
    pub high_gain_db: f32,
}

impl Default for TrackEqSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            low_freq: 100.0,
            low_gain_db: 0.0,
            mid_freq: 1500.0,
            mid_gain_db: 0.0,
            mid_q: 1.0,
            high_freq: 8000.0,
            high_gain_db: 0.0,
        }
    }
}

impl TrackEqSettings {
    pub fn is_flat(&self) -> bool {
        !self.enabled
            || (self.low_gain_db.abs() < 0.001
                && self.mid_gain_db.abs() < 0.001
                && self.high_gain_db.abs() < 0.001)
    }
}

#[derive(Clone)]
pub struct StereoEq {
    sample_rate: f32,
    coeffs: [BiquadCoeffs; 3],
    left: [BiquadState; 3],
    right: [BiquadState; 3],
    settings: TrackEqSettings,
}

impl StereoEq {
    pub fn new(sample_rate: f32) -> Self {
        let settings = TrackEqSettings::default();
        let mut eq = Self {
            sample_rate,
            coeffs: [BiquadCoeffs::bypass(); 3],
            left: [BiquadState::default(); 3],
            right: [BiquadState::default(); 3],
            settings,
        };
        eq.set_settings(settings);
        eq
    }

    pub fn set_settings(&mut self, s: TrackEqSettings) {
        self.settings = s;
        self.coeffs[0] = low_shelf(self.sample_rate, s.low_freq, s.low_gain_db);
        self.coeffs[1] = peaking(self.sample_rate, s.mid_freq, s.mid_gain_db, s.mid_q);
        self.coeffs[2] = high_shelf(self.sample_rate, s.high_freq, s.high_gain_db);
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if self.settings.is_flat() {
            return (l, r);
        }
        let mut l = l;
        let mut r = r;
        for i in 0..3 {
            l = self.left[i].process(l, &self.coeffs[i]);
            r = self.right[i].process(r, &self.coeffs[i]);
        }
        (l, r)
    }
}

// ---------------------------------------------------------------------------
// Compressor
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CompressorParams {
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    pub makeup_db: f32,
}

impl Default for CompressorParams {
    fn default() -> Self {
        Self { threshold_db: -18.0, ratio: 4.0, attack_ms: 15.0, release_ms: 120.0, makeup_db: 0.0 }
    }
}

pub struct Compressor {
    sample_rate: f32,
    env: f32,
    gain_reduction_db: f32,
}

impl Compressor {
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate, env: 0.0, gain_reduction_db: 0.0 }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &CompressorParams) -> (f32, f32) {
        if p.ratio <= 1.0 {
            self.gain_reduction_db *= 0.85;
            return (l, r);
        }
        let level = l.abs().max(r.abs());
        let attack = (-1.0 / (p.attack_ms.max(0.1) * 0.001 * self.sample_rate)).exp();
        let release = (-1.0 / (p.release_ms.max(1.0) * 0.001 * self.sample_rate)).exp();
        let coeff = if level > self.env { attack } else { release };
        self.env = level + coeff * (self.env - level);

        let env_db = 20.0 * self.env.max(1e-6).log10();
        let over = env_db - p.threshold_db;
        let gr = if over > 0.0 { -over * (1.0 - 1.0 / p.ratio.max(1.0)) } else { 0.0 };
        self.gain_reduction_db = gr;
        let gain = 10.0_f32.powf((gr + p.makeup_db) / 20.0);
        (l * gain, r * gain)
    }
}

// ---------------------------------------------------------------------------
// Stereo doubler / chorus
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DoublerParams {
    pub strength: f32, // 0..1 wet amount
    pub modulation: f32, // 0..1 LFO depth
    pub width: f32, // 0..1 stereo spread
}

impl Default for DoublerParams {
    fn default() -> Self {
        Self { strength: 0.7, modulation: 0.35, width: 0.8 }
    }
}

pub struct Doubler {
    sample_rate: f32,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    phase: f32,
}

impl Doubler {
    pub fn new(sample_rate: f32) -> Self {
        let len = ((sample_rate * 0.1) as usize).max(1);
        Self {
            sample_rate,
            buffer_l: vec![0.0; len],
            buffer_r: vec![0.0; len],
            write_pos: 0,
            phase: 0.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &DoublerParams) -> (f32, f32) {
        if p.strength <= 0.001 {
            return (l, r);
        }
        let len = self.buffer_l.len();
        self.buffer_l[self.write_pos] = l;
        self.buffer_r[self.write_pos] = r;

        self.phase += 0.35 / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let lfo = (self.phase * 2.0 * PI).sin();

        let base_ms = 22.0;
        let depth_ms = 8.0 * p.modulation;
        let dl = ((base_ms + depth_ms * lfo) * 0.001 * self.sample_rate) as usize;
        let dr = ((base_ms - depth_ms * lfo) * 0.001 * self.sample_rate) as usize;

        let read_l = (self.write_pos + len - dl.min(len - 1)) % len;
        let read_r = (self.write_pos + len - dr.min(len - 1)) % len;
        let wet_l = self.buffer_l[read_l];
        let wet_r = self.buffer_r[read_r];

        self.write_pos = (self.write_pos + 1) % len;

        // Mid/side width widening of the wet signal.
        let mid = (wet_l + wet_r) * 0.5;
        let side = (wet_l - wet_r) * 0.5 * (0.5 + p.width * 1.5);
        let (wet_l, wet_r) = (mid + side, mid - side);

        let mix = p.strength.clamp(0.0, 1.0);
        (l * (1.0 - mix) + wet_l * mix, r * (1.0 - mix) + wet_r * mix)
    }
}

// ---------------------------------------------------------------------------
// Noise gate
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GateParams {
    pub threshold: f32, // linear amplitude
    pub attack_ms: f32,
    pub release_ms: f32,
}

impl Default for GateParams {
    fn default() -> Self {
        Self { threshold: 0.02, attack_ms: 5.0, release_ms: 120.0 }
    }
}

pub struct NoiseGate {
    sample_rate: f32,
    env: f32,
    gain: f32,
}

impl NoiseGate {
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate, env: 0.0, gain: 1.0 }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &GateParams) -> (f32, f32) {
        let level = l.abs().max(r.abs());
        let coeff = if level > self.env { 0.4 } else { 0.9995 };
        self.env = level + coeff * (self.env - level);

        let target = if self.env >= p.threshold { 1.0 } else { 0.0 };
        let coeff = if target > self.gain {
            (-1.0 / (p.attack_ms.max(0.1) * 0.001 * self.sample_rate)).exp()
        } else {
            (-1.0 / (p.release_ms.max(1.0) * 0.001 * self.sample_rate)).exp()
        };
        self.gain = target + coeff * (self.gain - target);
        (l * self.gain, r * self.gain)
    }
}

// ---------------------------------------------------------------------------
// De-esser (high-band compressor)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeEsserParams {
    pub amount: f32, // 0..1
    pub freq: f32,
}

impl Default for DeEsserParams {
    fn default() -> Self {
        Self { amount: 0.45, freq: 6500.0 }
    }
}

pub struct DeEsser {
    sample_rate: f32,
    lp_l: f32,
    lp_r: f32,
    env: f32,
}

impl DeEsser {
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate, lp_l: 0.0, lp_r: 0.0, env: 0.0 }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &DeEsserParams) -> (f32, f32) {
        if p.amount <= 0.001 {
            return (l, r);
        }
        let coeff = 1.0 - (-2.0 * PI * p.freq.clamp(1000.0, 16000.0) / self.sample_rate).exp();
        self.lp_l += coeff * (l - self.lp_l);
        self.lp_r += coeff * (r - self.lp_r);
        let hi_l = l - self.lp_l;
        let hi_r = r - self.lp_r;

        let level = hi_l.abs().max(hi_r.abs());
        self.env = level + 0.995 * (self.env - level);
        let over = (self.env - 0.08).max(0.0);
        let reduction = (over * 6.0 * p.amount).min(0.9);
        let gain = 1.0 - reduction;
        (self.lp_l + hi_l * gain, self.lp_r + hi_r * gain)
    }
}

// ---------------------------------------------------------------------------
// Transient shaper (FL Studio: Transient Processor)
// ---------------------------------------------------------------------------

/// **Anslagskontroll.** Två höljesföljare på samma signal: en snabb som följer anslaget och en
/// långsam som följer kroppen. Skillnaden dem emellan, delad med den snabba nivån, är hur mycket
/// av ljudet som *är* ett anslag — och den andelen styr två separata vinster.
///
/// Vid 0 dB på båda är förstärkningen exakt 1,0 (10⁰ · 10⁰), alltså bit-identisk genomgång.
/// `transient_shaper_is_transparent_at_zero` håller den regeln.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransientParams {
    /// Anslagets vinst i dB (−12 .. +12). 0 = orörd.
    pub attack_db: f32,
    /// Kroppens (svansens) vinst i dB (−12 .. +12). 0 = orörd.
    pub sustain_db: f32,
    /// Hur stor skillnad som räknas som anslag (0 = bara knivskarpa, 1 = även mjuka). 0,5 = normal.
    pub sensitivity: f32,
}

impl Default for TransientParams {
    fn default() -> Self {
        Self { attack_db: 0.0, sustain_db: 0.0, sensitivity: 0.5 }
    }
}

pub struct TransientShaper {
    fast_rel: f32,
    slow_atk: f32,
    slow_rel: f32,
    fast: f32,
    slow: f32,
}

impl TransientShaper {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        Self {
            fast_rel: (-1.0 / (0.030 * sr)).exp(),
            slow_atk: 1.0 / (0.150 * sr),
            slow_rel: (-1.0 / (0.400 * sr)).exp(),
            fast: 0.0,
            slow: 0.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &TransientParams) -> (f32, f32) {
        let det = l.abs().max(r.abs());
        // Snabb: omedelbart anslag, 30 ms släpp — topphållaren.
        self.fast = if det > self.fast { det } else { det + self.fast_rel * (self.fast - det) };
        // Långsam: 150 ms upp, 400 ms ned — kroppens nivå, utan anslagets spets.
        self.slow = if det > self.slow {
            self.slow + self.slow_atk * (det - self.slow)
        } else {
            det + self.slow_rel * (self.slow - det)
        };
        let peak = self.fast.max(1e-9);
        let transient = ((self.fast - self.slow) / peak).clamp(0.0, 1.0);
        let thr = 1.0 - 0.8 * p.sensitivity.clamp(0.0, 1.0);
        let t = if transient <= thr {
            0.0
        } else {
            ((transient - thr) / (1.0 - thr)).clamp(0.0, 1.0)
        };
        let gain = 10.0_f32.powf(p.attack_db * t / 20.0)
            * 10.0_f32.powf(p.sustain_db * (1.0 - t) / 20.0);
        (l * gain, r * gain)
    }
}

// ---------------------------------------------------------------------------
// Saturator (FL Studio: Fruity Soft Clipper / Fast Dist / WaveShaper)
// ---------------------------------------------------------------------------

/// **Saturering**: mjuk klippning (`tanh`) med tonkontroll efter distorsionen.
///
/// Normaliseringen `1/tanh(drive)` gör att full skala in blir full skala ut **oavsett drive** —
/// reglaget ändrar hur mycket kurvan böjer, inte hur stark signalen är. Utgången kan därför
/// aldrig gå över 1,0: `saturator_never_exceeds_full_scale` mäter det.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SaturatorParams {
    /// Förstärkning in i kurvan (1,0 = ren, 20,0 = hårt mättad).
    pub drive: f32,
    /// 0 = mörk (500 Hz), 1 = öppen (20 kHz). Lågpass **efter** kurvan.
    pub tone: f32,
    /// Torr/våt (0 = ren genomgång).
    pub mix: f32,
}

impl Default for SaturatorParams {
    fn default() -> Self {
        Self { drive: 1.0, tone: 1.0, mix: 0.0 }
    }
}

pub struct Saturator {
    sample_rate: f32,
    lp_l: f32,
    lp_r: f32,
}

impl Saturator {
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate: sample_rate.max(1.0), lp_l: 0.0, lp_r: 0.0 }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &SaturatorParams) -> (f32, f32) {
        let mix = p.mix.clamp(0.0, 1.0);
        if mix <= 0.001 {
            return (l, r);
        }
        let drive = p.drive.clamp(1.0, 20.0);
        let norm = 1.0 / drive.tanh();
        let shaped_l = (drive * l).tanh() * norm;
        let shaped_r = (drive * r).tanh() * norm;

        let fc = 500.0 * 40.0_f32.powf(p.tone.clamp(0.0, 1.0));
        let coeff = 1.0 - (-2.0 * PI * fc / self.sample_rate).exp();
        self.lp_l += coeff * (shaped_l - self.lp_l);
        self.lp_r += coeff * (shaped_r - self.lp_r);

        (
            l + mix * (self.lp_l - l),
            r + mix * (self.lp_r - r),
        )
    }
}

// ---------------------------------------------------------------------------
// Modulation: chorus, flanger, phaser (FL Studio: Fruity/Hyper/Vintage Chorus,
// Fruity Flanger, Fruity Phaser — tio varianter av samma tre idéer)
// ---------------------------------------------------------------------------

/// Gemensamma reglage för de tre modulerande effekterna.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ModFxParams {
    /// LFO-hastighet i Hz (0,02–8).
    pub rate: f32,
    /// 0–1: hur långt delaytiden (chorus/flanger) eller brytfrekvensen (phaser) sveper.
    pub depth: f32,
    /// Torr/våt. 0 = ren genomgång. Phaserns kam ligger djupast vid 0,5.
    pub mix: f32,
    /// −0,9–0,9 återkoppling: flangerns vassa kam, phaserns resonans.
    pub feedback: f32,
}

impl Default for ModFxParams {
    fn default() -> Self {
        Self { rate: 0.4, depth: 0.4, mix: 0.0, feedback: 0.0 }
    }
}

/// Modulerad fördröjningslinje. Chorus och flanger är samma krets med olika skalor:
/// chorus har lång bas och litet svep, flangern kort bas, stort svep och återkoppling.
pub struct ModDelay {
    sample_rate: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    write: usize,
    phase: f32,
    base_ms: f32,
    swing_ms: f32,
    delay_ms: f32,
}

impl ModDelay {
    pub fn new(sample_rate: f32, base_ms: f32, swing_ms: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let cap = ((base_ms + swing_ms) * 0.001 * sr).ceil() as usize + 4;
        Self {
            sample_rate: sr,
            buf_l: vec![0.0; cap],
            buf_r: vec![0.0; cap],
            write: 0,
            phase: 0.0,
            base_ms,
            swing_ms,
            delay_ms: base_ms,
        }
    }

    /// Aktuell fördröjning i millisekunder — går att mäta direkt i stället för att gissa ur ljudet.
    #[cfg(test)]
    pub fn delay_ms(&self) -> f32 {
        self.delay_ms
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &ModFxParams) -> (f32, f32) {
        let mix = p.mix.clamp(0.0, 1.0);
        if mix <= 0.001 {
            return (l, r);
        }
        self.phase += p.rate.clamp(0.02, 8.0) / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let depth = p.depth.clamp(0.0, 1.0);
        // Kvarts period mellan kanalerna: det är bredden i choruset.
        let lfo_l = 0.5 + 0.5 * (2.0 * PI * self.phase).sin();
        let lfo_r = 0.5 + 0.5 * (2.0 * PI * (self.phase + 0.25)).sin();
        self.delay_ms = self.base_ms + self.swing_ms * depth * lfo_l;
        let delay_r = self.base_ms + self.swing_ms * depth * lfo_r;

        let n = self.buf_l.len() as f32;
        let max_d = n - 2.0;
        let tap = |buf: &[f32], write: usize, d: f32| -> f32 {
            let d = d.clamp(1.0, max_d);
            let mut read = write as f32 - d;
            if read < 0.0 {
                read += n;
            }
            let i0 = read.floor() as usize % buf.len();
            let frac = read - read.floor();
            let i1 = (i0 + 1) % buf.len();
            buf[i0] + frac * (buf[i1] - buf[i0])
        };

        let wet_l = tap(&self.buf_l, self.write, self.delay_ms * 0.001 * self.sample_rate);
        let wet_r = tap(&self.buf_r, self.write, delay_r * 0.001 * self.sample_rate);

        // Återkopplingen skrivs in i bufferten (inte ut i signalen): kammen blir vass utan att nivån skenar.
        let fb = p.feedback.clamp(-0.9, 0.9);
        self.buf_l[self.write] = l + wet_l * fb;
        self.buf_r[self.write] = r + wet_r * fb;
        self.write = (self.write + 1) % self.buf_l.len();

        (l + mix * (wet_l - l), r + mix * (wet_r - r))
    }
}

/// Fyra allpass-steg med svepande brytfrekvens — phaser. Vid `mix = 1,0` är utgången
/// bara fasvriden (allpass), så kammen hörs först när torr och våt blandas: 0,5 ger djupast kam.
pub struct Phaser {
    sample_rate: f32,
    phase: f32,
    x1_l: [f32; 4],
    x1_r: [f32; 4],
    y1_l: [f32; 4],
    y1_r: [f32; 4],
    fb_l: f32,
    fb_r: f32,
    fc_hz: f32,
}

impl Phaser {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(1.0),
            phase: 0.0,
            x1_l: [0.0; 4],
            x1_r: [0.0; 4],
            y1_l: [0.0; 4],
            y1_r: [0.0; 4],
            fb_l: 0.0,
            fb_r: 0.0,
            fc_hz: 200.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &ModFxParams) -> (f32, f32) {
        let mix = p.mix.clamp(0.0, 1.0);
        if mix <= 0.001 {
            return (l, r);
        }
        self.phase += p.rate.clamp(0.02, 8.0) / self.sample_rate;
        if self.phase >= 1.0 {
            self.phase -= 1.0;
        }
        let lfo = 0.5 + 0.5 * (2.0 * PI * self.phase).sin();
        self.fc_hz = 200.0 * 10.0_f32.powf(p.depth.clamp(0.0, 1.0) * lfo);
        let t = (PI * self.fc_hz / self.sample_rate).tan();
        // a = (tan−1)/(tan+1) hör till y = a·x + x1 − a·y1; den omvända kvoten ger inget filter alls.
        let c = (t - 1.0) / (t + 1.0);
        let fb = p.feedback.clamp(-0.9, 0.9);

        let mut xl = l + self.fb_l * fb;
        let mut xr = r + self.fb_r * fb;
        for s in 0..4 {
            let yl = c * xl + self.x1_l[s] - c * self.y1_l[s];
            self.x1_l[s] = xl;
            self.y1_l[s] = yl;
            xl = yl;
            let yr = c * xr + self.x1_r[s] - c * self.y1_r[s];
            self.x1_r[s] = xr;
            self.y1_r[s] = yr;
            xr = yr;
        }
        self.fb_l = xl;
        self.fb_r = xr;

        (l + mix * (xl - l), r + mix * (xr - r))
    }
}

// ---------------------------------------------------------------------------
// Impulsbank (FL Studio skeppar 260 impulser i 13 kategorier — vi genererar våra egna
// ur brus och tre tal, så banken är vår egen och ingen annans ljudfiler följer med)
// ---------------------------------------------------------------------------

/// Recept för en syntetisk impulsrespons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct IrRecipe {
    pub name: &'static str,
    /// Längd i sekunder (kapas till [`CONV_MAX_LEN`]).
    pub seconds: f32,
    /// Hur snabbt svansen faller: tau = seconds / decay.
    pub decay: f32,
    /// 0 = ljust, 1 = mörkt. Ett enpoligt lågpass vars brytfrekvens sjunker över tiden.
    pub damping: f32,
    /// 0 = torrt, 1 = breda tidiga reflektioner.
    pub width: f32,
    /// Baklänges svans (swell) i stället för avklingande.
    pub reverse: bool,
}

/// Banken som följer med: sex rum, alla genererade. FL:s 260 är riktiga inspelningar;
/// de här är modeller, och det står i statusraden när de laddas.
pub const IR_BANK: [IrRecipe; 6] = [
    IrRecipe { name: "Small Room", seconds: 0.6, decay: 3.2, damping: 0.35, width: 0.30, reverse: false },
    IrRecipe { name: "Studio Plate", seconds: 1.6, decay: 2.4, damping: 0.15, width: 0.80, reverse: false },
    IrRecipe { name: "Hall", seconds: 2.8, decay: 1.8, damping: 0.45, width: 0.70, reverse: false },
    IrRecipe { name: "Cathedral", seconds: 4.5, decay: 1.4, damping: 0.55, width: 0.90, reverse: false },
    IrRecipe { name: "Reverse Swell", seconds: 2.0, decay: 1.2, damping: 0.30, width: 0.60, reverse: true },
    IrRecipe { name: "Spring", seconds: 1.2, decay: 2.0, damping: 0.25, width: 0.20, reverse: false },
];

/// Genererar en monoimpulsrespons ur receptet.
///
/// Bruset kommer från en egen LCG med fast frö, alltså **deterministiskt**: samma recept ger
/// samma respons, vilket är det som gör svansen mätbar i ett prov i stället för hörbar bara ibland.
pub fn synthesize_ir(recipe: &IrRecipe, sample_rate: f32) -> Vec<f32> {
    let sr = sample_rate.max(1.0);
    let len = ((recipe.seconds.max(0.05) * sr) as usize).min(CONV_MAX_LEN).max(64);

    let mut state: u32 = 0x1234_5678;
    let mut noise = || {
        // xorshift32: stabilt, inga beroenden, samma följd varje gång.
        state ^= state << 13;
        state ^= state >> 17;
        state ^= state << 5;
        (state as f32 / u32::MAX as f32) * 2.0 - 1.0
    };

    let tau = (recipe.seconds.max(0.05) / recipe.decay.max(0.1)).max(0.005);
    // Ingen predelay: responsen är en insert, och varje millisekund fördröjning där
    // är latens i hela kedjan. Rumskänslan kommer från reflektionerna i stället.
    let predelay = 0usize;
    let mut ir = vec![0.0f32; len];
    let mut lp = 0.0f32;

    for n in predelay..len {
        let t = (n - predelay) as f32 / sr;
        let env = (-t / tau).exp();
        // Mörkare mot slutet: damping styr hur mycket brytfrekvensen faller.
        let progress = (n as f32 / len as f32).clamp(0.0, 1.0);
        let k = (1.0 - recipe.damping.clamp(0.0, 1.0) * progress).clamp(0.05, 1.0) * 0.6;
        lp += k * (noise() - lp);
        ir[n] = lp * env;
    }

    // Tidiga reflektioner: få, tydliga studsar som ger riktning åt rummet.
    let w = recipe.width.clamp(0.0, 1.0);
    if w > 0.01 {
        for i in 1..=6 {
            let delay = (0.006 + 0.0075 * i as f32 * (1.0 + w)) * sr;
            let idx = predelay + delay as usize;
            if idx < len {
                let sign = if i % 2 == 0 { -1.0 } else { 1.0 };
                ir[idx] += sign * w * 0.55 / i as f32;
            }
        }
    }

    if recipe.reverse {
        ir.reverse();
    }

    // Normalisera toppen till 1,0: då är `gain_db` i ConvolverParams den enda nivåkontrollen.
    let peak = ir.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if peak > 1e-9 {
        let g = 1.0 / peak;
        ir.iter_mut().for_each(|v| *v *= g);
    }
    ir
}
// ---------------------------------------------------------------------------
// Multiband compressor (FL Studio: Maximus / Fruity Multiband Compressor)
// ---------------------------------------------------------------------------

/// RBJ-lågpass (2:a ordningen). Två i rad = Linkwitz-Riley 4:e ordningen.
fn lowpass_coeffs(sr: f32, freq: f32, q: f32) -> BiquadCoeffs {
    let w0 = 2.0 * PI * freq.clamp(20.0, sr * 0.45) / sr;
    let cos_w0 = w0.cos();
    let alpha = w0.sin() / (2.0 * q.max(0.05));
    let a0 = 1.0 + alpha;
    BiquadCoeffs {
        b0: (1.0 - cos_w0) * 0.5 / a0,
        b1: (1.0 - cos_w0) / a0,
        b2: (1.0 - cos_w0) * 0.5 / a0,
        a1: -2.0 * cos_w0 / a0,
        a2: (1.0 - alpha) / a0,
    }
}

/// RBJ-högpass (2:a ordningen) — syster till [`lowpass_coeffs`].
fn highpass_coeffs(sr: f32, freq: f32, q: f32) -> BiquadCoeffs {
    let w0 = 2.0 * PI * freq.clamp(20.0, sr * 0.45) / sr;
    let cos_w0 = w0.cos();
    let alpha = w0.sin() / (2.0 * q.max(0.05));
    let a0 = 1.0 + alpha;
    BiquadCoeffs {
        b0: (1.0 + cos_w0) * 0.5 / a0,
        b1: -(1.0 + cos_w0) / a0,
        b2: (1.0 + cos_w0) * 0.5 / a0,
        a1: -2.0 * cos_w0 / a0,
        a2: (1.0 - alpha) / a0,
    }
}

/// Tre band med två delningar (Linkwitz-Riley 4:e ordningen, Q = 0,7071 i båda stegen).
///
/// `low = LP(f1)`, `mid = HP(f1)·LP(f2)`, `high = HP(f1)·HP(f2)` — summan är
/// amplitudplatt över delningarna, vilket `multiband_crossover_is_flat` mäter med tre toner.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MultibandParams {
    /// Nedre delningen i Hz.
    pub xover_low: f32,
    /// Övre delningen i Hz.
    pub xover_high: f32,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
    /// Hur hårt varje band komprimeras (0 = orört band, 1 = full kompression).
    pub amounts: [f32; 3],
}

impl Default for MultibandParams {
    fn default() -> Self {
        Self {
            xover_low: 250.0,
            xover_high: 4000.0,
            threshold_db: -18.0,
            ratio: 4.0,
            attack_ms: 5.0,
            release_ms: 120.0,
            amounts: [0.0; 3],
        }
    }
}

struct XoverCoeffs {
    f1_lp: BiquadCoeffs,
    f1_hp: BiquadCoeffs,
    f2_lp: BiquadCoeffs,
    f2_hp: BiquadCoeffs,
}

impl XoverCoeffs {
    fn new(sr: f32, f1: f32, f2: f32) -> Self {
        let q = std::f32::consts::FRAC_1_SQRT_2;
        Self {
            f1_lp: lowpass_coeffs(sr, f1, q),
            f1_hp: highpass_coeffs(sr, f1, q),
            f2_lp: lowpass_coeffs(sr, f2, q),
            f2_hp: highpass_coeffs(sr, f2, q),
        }
    }
}

/// En kanals delningsfilter: fyra biquads i två steg per delning.
#[derive(Default)]
struct XoverChan {
    f1_lp: [BiquadState; 2],
    f1_hp: [BiquadState; 2],
    f2_lp: [BiquadState; 2],
    f2_hp: [BiquadState; 2],
}

impl XoverChan {
    #[inline(always)]
    fn split(&mut self, x: f32, c: &XoverCoeffs) -> (f32, f32, f32) {
        let mut low = self.f1_lp[0].process(x, &c.f1_lp);
        low = self.f1_lp[1].process(low, &c.f1_lp);
        let mut above = self.f1_hp[0].process(x, &c.f1_hp);
        above = self.f1_hp[1].process(above, &c.f1_hp);

        let mut mid = self.f2_lp[0].process(above, &c.f2_lp);
        mid = self.f2_lp[1].process(mid, &c.f2_lp);
        let mut high = self.f2_hp[0].process(above, &c.f2_hp);
        high = self.f2_hp[1].process(high, &c.f2_hp);

        (low, mid, high)
    }
}

pub struct MultibandCompressor {
    sample_rate: f32,
    coeffs: XoverCoeffs,
    left: XoverChan,
    right: XoverChan,
    bands: [Compressor; 3],
    params: MultibandParams,
}

impl MultibandCompressor {
    pub fn new(sample_rate: f32) -> Self {
        let params = MultibandParams::default();
        Self {
            sample_rate,
            coeffs: XoverCoeffs::new(sample_rate, params.xover_low, params.xover_high),
            left: XoverChan::default(),
            right: XoverChan::default(),
            bands: [
                Compressor::new(sample_rate),
                Compressor::new(sample_rate),
                Compressor::new(sample_rate),
            ],
            params,
        }
    }

    pub fn set_params(&mut self, p: MultibandParams) {
        if (p.xover_low - self.params.xover_low).abs() > 0.5
            || (p.xover_high - self.params.xover_high).abs() > 0.5
        {
            self.coeffs = XoverCoeffs::new(self.sample_rate, p.xover_low, p.xover_high);
        }
        self.params = p;
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let (ll, lm, lh) = self.left.split(l, &self.coeffs);
        let (rl, rm, rh) = self.right.split(r, &self.coeffs);
        let dry = [(ll, rl), (lm, rm), (lh, rh)];

        let p = self.params;
        let band_params = CompressorParams {
            threshold_db: p.threshold_db,
            ratio: p.ratio.max(1.0),
            attack_ms: p.attack_ms,
            release_ms: p.release_ms,
            makeup_db: 0.0,
        };

        let mut out = (0.0f32, 0.0f32);
        for (i, (dl, dr)) in dry.iter().enumerate() {
            let (wl, wr) = self.bands[i].process(*dl, *dr, &band_params);
            let a = p.amounts[i].clamp(0.0, 1.0);
            out.0 += dl + a * (wl - dl);
            out.1 += dr + a * (wr - dr);
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Step gate (FL Studio: Gross Beat — volymkurvan)
// ---------------------------------------------------------------------------

/// **Steggrind**: 16 steg om 1/16-del, varje steg en egen nivå. Det är Gross Beats
/// volymkurva utan tidsmanipulationen — samma sak som en trance-gate, men styrd av ett
/// mönster i stället för en LFO, alltså rytmiskt i stället för periodiskt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GatePatternParams {
    pub gains: [f32; 16],
    pub bpm: f32,
    /// 0 = hårda steg (klick i kanten), 1 = mjuk övergång över halva steget.
    pub smooth: f32,
    pub enabled: bool,
}

impl Default for GatePatternParams {
    fn default() -> Self {
        Self { gains: [1.0; 16], bpm: 120.0, smooth: 0.0, enabled: false }
    }
}

pub struct StepGate {
    sample_rate: f32,
    pos: f32,
    gain: f32,
}

impl StepGate {
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate, pos: 0.0, gain: 1.0 }
    }

    /// Nollställer till steg 0 — anropas när grinden slås på, så mönstret alltid börjar
    /// från början i stället för mitt i ett steg.
    pub fn reset(&mut self) {
        self.pos = 0.0;
        self.gain = 1.0;
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &GatePatternParams) -> (f32, f32) {
        let bpm = p.bpm.clamp(20.0, 300.0);
        let step_len = self.sample_rate * 60.0 / (bpm * 4.0);
        let index = ((self.pos / step_len) as usize) % p.gains.len();
        let target = p.gains[index].clamp(0.0, 1.0);
        if p.smooth <= 0.001 {
            self.gain = target;
        } else {
            // Ett steg långt glid om `smooth = 1`, omedelbart om 0.
            let coeff = (1.0 / (1.0 + p.smooth * step_len * 0.5)).clamp(0.0, 1.0);
            self.gain += (target - self.gain) * coeff;
        }
        self.pos += 1.0;
        if self.pos >= step_len * p.gains.len() as f32 {
            self.pos = 0.0;
        }
        (l * self.gain, r * self.gain)
    }
}

// ---------------------------------------------------------------------------
// Convolution reverb (FL Studio: Fruity Convolver)
// ---------------------------------------------------------------------------

/// Blocklängd för den partitionerade faltningen. 256 sampel = **5,3 ms** vid 48 kHz, och
/// det är samtidigt våt-signalens latens (se [`Convolver`]). Blocket är en avvägning: mindre
/// ger kortare latens men fler FFT:er per sekund.
const CONV_BLOCK: usize = 256;
const CONV_N: usize = 2 * CONV_BLOCK;
/// Tak för impulsresponsen: 2 s vid 48 kHz (376 partitioner). `shortcut:` längre rums-
/// respons klipps, för kostnaden är linjär i antalet partitioner — höj taket när någon
/// behöver längre rum, inte förrän dess.
const CONV_MAX_LEN: usize = 96_000;

/// Torr/våt och nivå för convolvern. Själva impulsresponsen ligger **inte** här: den är
/// hundratusentals sampel och kan inte följa med ett `Copy`-kommando. Den skickas för sig
/// (`SetConvolverIr`) och ägs av kedjan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ConvolverParams {
    pub mix: f32,
    pub gain_db: f32,
}

impl Default for ConvolverParams {
    fn default() -> Self {
        Self { mix: 0.35, gain_db: 0.0 }
    }
}

/// **Faltning med en impulsrespons**, uniformt partitionerad (overlap-save).
///
/// Responsen delas i block om [`CONV_BLOCK`]; varje block FFT:as en gång vid laddning och
/// summeras sedan som `Y = Σ X(n−p)·H(p)` per block. Det ger **linjär** kostnad i
/// responsens längd och ingen allokering i `process`.
///
/// **Latensen är ett block** (5,3 ms vid 48 kHz): den första utsignalen kan inte beräknas
/// förrän ett helt block inkommit. `mix = 0` är bit-exakt genomgång, och provet
/// `convolver_delays_by_exactly_one_block` mäter latensen i stället för att dölja den.
pub struct Convolver {
    parts: usize,
    ir_re: Vec<f32>,
    ir_im: Vec<f32>,
    hist_l_re: Vec<f32>,
    hist_l_im: Vec<f32>,
    hist_r_re: Vec<f32>,
    hist_r_im: Vec<f32>,
    hist_pos: usize,
    ring_l: Vec<f32>,
    ring_r: Vec<f32>,
    write: usize,
    fill: usize,
    fft_re: Vec<f32>,
    fft_im: Vec<f32>,
    acc_re: Vec<f32>,
    acc_im: Vec<f32>,
    out_l: Vec<f32>,
    out_r: Vec<f32>,
    out_pos: usize,
}

impl Convolver {
    pub fn new(_sample_rate: f32) -> Self {
        Self {
            parts: 0,
            ir_re: Vec::new(),
            ir_im: Vec::new(),
            hist_l_re: Vec::new(),
            hist_l_im: Vec::new(),
            hist_r_re: Vec::new(),
            hist_r_im: Vec::new(),
            hist_pos: 0,
            ring_l: vec![0.0; CONV_N],
            ring_r: vec![0.0; CONV_N],
            write: 0,
            fill: 0,
            fft_re: vec![0.0; CONV_N],
            fft_im: vec![0.0; CONV_N],
            acc_re: vec![0.0; CONV_N],
            acc_im: vec![0.0; CONV_N],
            out_l: vec![0.0; CONV_BLOCK],
            out_r: vec![0.0; CONV_BLOCK],
            out_pos: CONV_BLOCK,
        }
    }

    /// Laddar en (mono) impulsrespons. `None` = ingen faltning alls (bit-exakt genomgång).
    pub fn set_ir(&mut self, ir: Option<Arc<Vec<f32>>>) {
        let Some(ir) = ir.filter(|i| !i.is_empty()) else {
            self.parts = 0;
            self.ir_re.clear();
            self.ir_im.clear();
            return;
        };
        let len = ir.len().min(CONV_MAX_LEN);
        let parts = len.div_ceil(CONV_BLOCK).max(1);
        self.parts = parts;
        self.ir_re = vec![0.0; parts * CONV_N];
        self.ir_im = vec![0.0; parts * CONV_N];
        let mut block = vec![0.0f32; CONV_N];
        let mut imag = vec![0.0f32; CONV_N];
        for p in 0..parts {
            block.iter_mut().for_each(|v| *v = 0.0);
            imag.iter_mut().for_each(|v| *v = 0.0);
            let start = p * CONV_BLOCK;
            let end = (start + CONV_BLOCK).min(len);
            block[..end - start].copy_from_slice(&ir[start..end]);
            fft_radix2(&mut block, &mut imag, false);
            self.ir_re[p * CONV_N..(p + 1) * CONV_N].copy_from_slice(&block);
            self.ir_im[p * CONV_N..(p + 1) * CONV_N].copy_from_slice(&imag);
        }
        self.hist_l_re = vec![0.0; parts * CONV_N];
        self.hist_l_im = vec![0.0; parts * CONV_N];
        self.hist_r_re = vec![0.0; parts * CONV_N];
        self.hist_r_im = vec![0.0; parts * CONV_N];
        self.hist_pos = 0;
        self.ring_l.iter_mut().for_each(|v| *v = 0.0);
        self.ring_r.iter_mut().for_each(|v| *v = 0.0);
        self.write = 0;
        self.fill = 0;
        self.out_pos = CONV_BLOCK;
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &ConvolverParams) -> (f32, f32) {
        if self.parts == 0 || p.mix <= 0.001 {
            return (l, r);
        }
        let (wet_l, wet_r) = if self.out_pos < CONV_BLOCK {
            let s = (self.out_l[self.out_pos], self.out_r[self.out_pos]);
            self.out_pos += 1;
            s
        } else {
            (0.0, 0.0)
        };

        self.ring_l[self.write] = l;
        self.ring_r[self.write] = r;
        self.write = (self.write + 1) % CONV_N;
        self.fill += 1;
        if self.fill == CONV_BLOCK {
            self.fill = 0;
            self.convolve_block();
        }

        let gain = 10.0_f32.powf(p.gain_db / 20.0);
        let mix = p.mix.clamp(0.0, 1.0);
        (
            l + mix * (wet_l * gain - l),
            r + mix * (wet_r * gain - r),
        )
    }

    fn convolve_block(&mut self) {
        for ch in 0..2 {
            // Rulla ut de senaste N sampeln ur ringen och FFT:a dem.
            for i in 0..CONV_N {
                let v = if ch == 0 {
                    self.ring_l[(self.write + i) % CONV_N]
                } else {
                    self.ring_r[(self.write + i) % CONV_N]
                };
                self.fft_re[i] = v;
                self.fft_im[i] = 0.0;
            }
            fft_radix2(&mut self.fft_re, &mut self.fft_im, false);
            let base = self.hist_pos * CONV_N;
            if ch == 0 {
                self.hist_l_re[base..base + CONV_N].copy_from_slice(&self.fft_re);
                self.hist_l_im[base..base + CONV_N].copy_from_slice(&self.fft_im);
            } else {
                self.hist_r_re[base..base + CONV_N].copy_from_slice(&self.fft_re);
                self.hist_r_im[base..base + CONV_N].copy_from_slice(&self.fft_im);
            }

            for i in 0..CONV_N {
                self.acc_re[i] = 0.0;
                self.acc_im[i] = 0.0;
            }
            for p in 0..self.parts {
                let idx = (self.hist_pos + self.parts - p) % self.parts;
                let hb = idx * CONV_N;
                let ib = p * CONV_N;
                for i in 0..CONV_N {
                    let (xr, xi) = if ch == 0 {
                        (self.hist_l_re[hb + i], self.hist_l_im[hb + i])
                    } else {
                        (self.hist_r_re[hb + i], self.hist_r_im[hb + i])
                    };
                    let (hr, hi) = (self.ir_re[ib + i], self.ir_im[ib + i]);
                    self.acc_re[i] += xr * hr - xi * hi;
                    self.acc_im[i] += xr * hi + xi * hr;
                }
            }
            fft_radix2(&mut self.acc_re, &mut self.acc_im, true);
            // Overlap-save: den **andra** halvan är den giltiga.
            if ch == 0 {
                self.out_l.copy_from_slice(&self.acc_re[CONV_BLOCK..CONV_N]);
            } else {
                self.out_r.copy_from_slice(&self.acc_re[CONV_BLOCK..CONV_N]);
            }
        }
        self.hist_pos = (self.hist_pos + 1) % self.parts;
        self.out_pos = 0;
    }
}

// ---------------------------------------------------------------------------
// Master filter (with drive) + limiter
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MasterFilterParams {
    pub cutoff: f32,
    pub resonance: f32,
    pub drive: f32, // 1.0 = clean
    pub enabled: bool,
}

impl Default for MasterFilterParams {
    fn default() -> Self {
        Self { cutoff: 18000.0, resonance: 0.25, drive: 1.0, enabled: false }
    }
}

#[derive(Clone, Copy, Default)]
struct SvfState {
    ic1eq: f32,
    ic2eq: f32,
}

pub struct MasterFilter {
    sample_rate: f32,
    l: SvfState,
    r: SvfState,
}

impl MasterFilter {
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate, l: SvfState::default(), r: SvfState::default() }
    }

    #[inline(always)]
    fn lowpass(state: &mut SvfState, input: f32, sr: f32, p: &MasterFilterParams) -> f32 {
        let cutoff = p.cutoff.clamp(20.0, sr * 0.48);
        let q = p.resonance.clamp(0.1, 10.0);
        let g = (PI * cutoff / sr).tan();
        let k = 1.0 / q;
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = input - state.ic2eq;
        let v1 = a1 * state.ic1eq + a2 * v3;
        let v2 = state.ic2eq + a2 * state.ic1eq + a3 * v3;
        state.ic1eq = 2.0 * v1 - state.ic1eq;
        state.ic2eq = 2.0 * v2 - state.ic2eq;
        v2
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &MasterFilterParams) -> (f32, f32) {
        if !p.enabled {
            return (l, r);
        }
        let (mut l, mut r) = (l, r);
        if p.drive > 1.05 {
            l = (l * p.drive).tanh() / (p.drive * 0.7 + 0.3);
            r = (r * p.drive).tanh() / (p.drive * 0.7 + 0.3);
        }
        let sr = self.sample_rate;
        let ol = Self::lowpass(&mut self.l, l, sr, p);
        let or = Self::lowpass(&mut self.r, r, sr, p);
        (ol, or)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LimiterParams {
    pub ceiling: f32,
    pub release_ms: f32,
    pub boost: f32, // linear input gain
}

impl Default for LimiterParams {
    fn default() -> Self {
        Self { ceiling: 0.95, release_ms: 80.0, boost: 1.0 }
    }
}

pub struct Limiter {
    sample_rate: f32,
    gain: f32,
}

impl Limiter {
    pub fn new(sample_rate: f32) -> Self {
        Self { sample_rate, gain: 1.0 }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &LimiterParams) -> (f32, f32) {
        let ceiling = p.ceiling.clamp(0.1, 1.0);
        let l = l * p.boost;
        let r = r * p.boost;
        let peak = l.abs().max(r.abs());
        let desired = if peak > ceiling { ceiling / peak } else { 1.0 };
        if desired < self.gain {
            self.gain = desired;
        } else {
            let coeff = (-1.0 / (p.release_ms.max(1.0) * 0.001 * self.sample_rate)).exp();
            self.gain = desired + coeff * (self.gain - desired);
        }
        let out_l = (l * self.gain).clamp(-1.0, 1.0);
        let out_r = (r * self.gain).clamp(-1.0, 1.0);
        (out_l, out_r)
    }
}

// ---------------------------------------------------------------------------
// Full master FX chain
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MasterEqBand {
    pub freq: f32,
    pub gain_db: f32,
    pub q: f32,
    pub active: bool,
}

impl MasterEqBand {
    pub const fn flat() -> Self {
        Self { freq: 1000.0, gain_db: 0.0, q: 1.0, active: false }
    }
}

// ---------------------------------------------------------------------------
// Granulizer
// ---------------------------------------------------------------------------

/// Granulizer: korn läses ur en buffert av det senaste ljudet. Positionen är hur långt
/// bakåt i historiken kornet börjar, spridningen hur mycket den slumpas, och tätheten hur
/// många korn per sekund som startas. Kornen ligger på varandra, så ljudet får en svävande,
/// stammande karaktär — det är hela poängen, inte en artefakt.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GranularParams {
    pub size_ms: f32,     // 10–200
    pub density: f32,     // korn per sekund, 2–60
    pub position_ms: f32, // hur långt bakåt kornet börjar, 20–1000
    pub spray_ms: f32,    // slumpmässig spridning kring positionen, 0–200
    /// Kornens uppspelningshastighet: 1,0 = som inspelat, 2,0 = en oktav upp.
    pub speed: f32,
    pub mix: f32,         // 0 = torrt, 1 = bara korn
}

impl Default for GranularParams {
    fn default() -> Self {
        Self { size_ms: 60.0, density: 12.0, position_ms: 120.0, spray_ms: 40.0, speed: 1.0, mix: 0.6 }
    }
}

const GRAIN_SLOTS: usize = 4;

pub struct Granular {
    sample_rate: f32,
    l: Vec<f32>,
    r: Vec<f32>,
    write: usize,
    to_next: f32,
    seed: u32,
    /// (start, längd, läst) per kornplats. `läst` är 0 när platsen är ledig.
    grains: [(usize, usize, usize); GRAIN_SLOTS],
    /// Läspositionen i sampel per korn, med bråkdel — den driver iväg när kornet spelas
    /// i annan hastighet än 1,0 och kan därför inte vara ett heltal.
    reads: [f32; GRAIN_SLOTS],
}

impl Granular {
    pub fn new(sample_rate: f32) -> Self {
        // Två sekunder räcker: längsta positionen (1 s) plus längsta kornet (200 ms).
        let len = (sample_rate * 2.0) as usize;
        Self {
            sample_rate,
            l: vec![0.0; len],
            r: vec![0.0; len],
            write: 0,
            to_next: 0.0,
            seed: 0x2545_F491,
            grains: [(0, 0, 0); GRAIN_SLOTS],
            reads: [0.0; GRAIN_SLOTS],
        }
    }

    /// Deterministisk slumpsiffra: samma inställning ger samma kornläge, så ett prov kan
    /// mäta något och en export låter som förhandslyssningen.
    fn rand(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / 16_777_216.0
    }

    pub fn process(&mut self, l: f32, r: f32, p: &GranularParams) -> (f32, f32) {
        let len = self.l.len();
        self.l[self.write] = l;
        self.r[self.write] = r;

        let size = (p.size_ms.clamp(10.0, 200.0) * 0.001 * self.sample_rate) as usize;
        let position = (p.position_ms.clamp(20.0, 1000.0) * 0.001 * self.sample_rate) as usize;
        let spray = (p.spray_ms.clamp(0.0, 200.0) * 0.001 * self.sample_rate) as usize;

        self.to_next -= 1.0;
        if self.to_next <= 0.0 {
            let density = p.density.clamp(2.0, 60.0);
            self.to_next += self.sample_rate / density;
            // Äldsta platsen återanvänds när alla är upptagna.
            let slot = self
                .grains
                .iter()
                .position(|g| g.2 == 0)
                .unwrap_or_else(|| {
                    let (mut oldest, mut at) = (usize::MAX, 0);
                    for (i, g) in self.grains.iter().enumerate() {
                        if g.2 > oldest || oldest == usize::MAX {
                            oldest = g.2;
                            at = i;
                        }
                    }
                    at
                });
            let jitter = if spray > 0 { (self.rand() * spray as f32) as usize } else { 0 };
            let behind = (position + jitter).min(len - 1);
            let start = (self.write + len - behind) % len;
            self.grains[slot] = (start, size.max(1), 1);
            self.reads[slot] = start as f32;
        }

        let speed = p.speed.clamp(0.25, 4.0);
        let (mut gl, mut gr) = (0.0f32, 0.0f32);
        for (slot, g) in self.grains.iter_mut().enumerate() {
            if g.2 == 0 {
                continue;
            }
            // Bråkdelad läsposition med linjär interpolering: utan den hackar varje korn när
            // hastigheten inte är 1,0 (heltalssteg hörs som kvantiseringsbrus på ton).
            let read = self.reads[slot];
            let i0 = (read as usize) % len;
            let i1 = (i0 + 1) % len;
            let frac = read - read.floor();
            let l = self.l[i0] * (1.0 - frac) + self.l[i1] * frac;
            let r = self.r[i0] * (1.0 - frac) + self.r[i1] * frac;
            // Hann-fönster: kornet tonar in och ut, annars knäpper varje korn.
            let t = g.2 as f32 / g.1 as f32;
            let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * t).cos();
            gl += l * w;
            gr += r * w;
            self.reads[slot] = (read + speed) % len as f32;
            g.2 += 1;
            if g.2 > g.1 {
                g.2 = 0;
            }
        }

        self.write = (self.write + 1) % len;
        let mix = p.mix.clamp(0.0, 1.0);
        (l * (1.0 - mix) + gl * mix, r * (1.0 - mix) + gr * mix)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]

pub struct MasterFxParams {
    pub eq_bands: [MasterEqBand; 4],
    pub eq_enabled: bool,
    pub comp: CompressorParams,
    pub comp_enabled: bool,
    pub doubler: DoublerParams,
    pub doubler_enabled: bool,
    pub deesser: DeEsserParams,
    pub deesser_enabled: bool,
    pub transient: TransientParams,
    pub transient_enabled: bool,
    pub sat: SaturatorParams,
    pub sat_enabled: bool,
    pub chorus: ModFxParams,
    pub chorus_enabled: bool,
    pub flanger: ModFxParams,
    pub flanger_enabled: bool,
    pub phaser: ModFxParams,
    pub phaser_enabled: bool,
    pub multiband: MultibandParams,
    pub multiband_enabled: bool,
    pub convolver: ConvolverParams,
    pub convolver_enabled: bool,
    pub gate: GateParams,
    pub gate_enabled: bool,
    pub filter: MasterFilterParams,
    pub limiter: LimiterParams,
    pub limiter_enabled: bool,
    pub granular: GranularParams,
    pub granular_enabled: bool,
}

impl Default for MasterFxParams {
    fn default() -> Self {
        Self {
            eq_bands: [MasterEqBand::flat(); 4],
            eq_enabled: true,
            comp: CompressorParams::default(),
            comp_enabled: false,
            doubler: DoublerParams::default(),
            doubler_enabled: false,
            deesser: DeEsserParams::default(),
            deesser_enabled: false,
            transient: TransientParams::default(),
            transient_enabled: false,
            sat: SaturatorParams::default(),
            sat_enabled: false,
            chorus: ModFxParams::default(),
            chorus_enabled: false,
            flanger: ModFxParams::default(),
            flanger_enabled: false,
            phaser: ModFxParams::default(),
            phaser_enabled: false,
            multiband: MultibandParams::default(),
            multiband_enabled: false,
            convolver: ConvolverParams::default(),
            convolver_enabled: false,
            gate: GateParams::default(),
            gate_enabled: false,
            filter: MasterFilterParams::default(),
            granular: GranularParams::default(),
            granular_enabled: false,
            limiter: LimiterParams::default(),
            limiter_enabled: false,
        }
    }
}

pub struct MasterFxChain {
    sample_rate: f32,
    eq_coeffs: [BiquadCoeffs; 4],
    eq_l: [BiquadState; 4],
    eq_r: [BiquadState; 4],
    eq_active: bool,
    comp: Compressor,
    doubler: Doubler,
    deesser: DeEsser,
    transient: TransientShaper,
    sat: Saturator,
    chorus: ModDelay,
    flanger: ModDelay,
    phaser: Phaser,
    multiband: MultibandCompressor,
    convolver: Convolver,
    gate: NoiseGate,
    granular: Granular,
    filter: MasterFilter,
    limiter: Limiter,
    params: MasterFxParams,
}

impl MasterFxChain {
    pub fn new(sample_rate: f32) -> Self {
        let mut chain = Self {
            sample_rate,
            eq_coeffs: [BiquadCoeffs::bypass(); 4],
            eq_l: [BiquadState::default(); 4],
            eq_r: [BiquadState::default(); 4],
            eq_active: false,
            comp: Compressor::new(sample_rate),
            doubler: Doubler::new(sample_rate),
            deesser: DeEsser::new(sample_rate),
            transient: TransientShaper::new(sample_rate),
            sat: Saturator::new(sample_rate),
            chorus: ModDelay::new(sample_rate, 12.0, 8.0),
            flanger: ModDelay::new(sample_rate, 2.5, 2.0),
            phaser: Phaser::new(sample_rate),
            multiband: MultibandCompressor::new(sample_rate),
            convolver: Convolver::new(sample_rate),
            gate: NoiseGate::new(sample_rate),
            granular: Granular::new(sample_rate),
            filter: MasterFilter::new(sample_rate),
            limiter: Limiter::new(sample_rate),
            params: MasterFxParams::default(),
        };
        chain.set_params(chain.params);
        chain
    }

    pub fn set_params(&mut self, params: MasterFxParams) {
        self.params = params;
        let mut any = false;
        for (i, band) in params.eq_bands.iter().enumerate() {
            if band.active && band.gain_db.abs() > 0.001 {
                self.eq_coeffs[i] = peaking(self.sample_rate, band.freq, band.gain_db, band.q);
                any = true;
            } else {
                self.eq_coeffs[i] = BiquadCoeffs::bypass();
            }
        }
        self.eq_active = params.eq_enabled && any;
        self.multiband.set_params(params.multiband);
    }

    /// Laddar (eller rensar) convolverns impulsrespons. Responsen är för stor för att
    /// följa med `MasterFxParams` (som är `Copy`), så den kommer som ett eget kommando.
    pub fn set_convolver_ir(&mut self, ir: Option<Arc<Vec<f32>>>) {
        self.convolver.set_ir(ir);
    }

    /// Current compressor gain reduction in dB (<= 0.0). Read by the UI meter.
    pub fn gain_reduction_db(&self) -> f32 {
        if self.params.comp_enabled { self.comp.gain_reduction_db } else { 0.0 }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let p = self.params;
        let (mut l, mut r) = (l, r);

        if p.gate_enabled {
            let (nl, nr) = self.gate.process(l, r, &p.gate);
            l = nl;
            r = nr;
        }

        if p.granular_enabled {
            let (nl, nr) = self.granular.process(l, r, &p.granular);
            l = nl;
            r = nr;
        }

        // Anslaget formas före kompressorn: annars hade kompressorn ätit upp den vinst
        // transientreglaget gjorde, och reglaget hade verkat svagare ju hårdare man komprimerar.
        if p.transient_enabled {
            let (nl, nr) = self.transient.process(l, r, &p.transient);
            l = nl;
            r = nr;
        }

        if self.eq_active {
            for i in 0..4 {
                l = self.eq_l[i].process(l, &self.eq_coeffs[i]);
                r = self.eq_r[i].process(r, &self.eq_coeffs[i]);
            }
        }

        if p.comp_enabled {
            let (nl, nr) = self.comp.process(l, r, &p.comp);
            l = nl;
            r = nr;
        }

        if p.deesser_enabled {
            let (nl, nr) = self.deesser.process(l, r, &p.deesser);
            l = nl;
            r = nr;
        }

        if p.filter.enabled {
            let (nl, nr) = self.filter.process(l, r, &p.filter);
            l = nl;
            r = nr;
        }

        // Sist före limitern: en mättnad som ligger efter allt annat färgar hela kedjan,
        // och limitern tar hand om den extra toppen den kan ge.
        if p.multiband_enabled {
            let (nl, nr) = self.multiband.process(l, r);
            l = nl;
            r = nr;
        }

        if p.convolver_enabled {
            let (nl, nr) = self.convolver.process(l, r, &p.convolver);
            l = nl;
            r = nr;
        }

        if p.sat_enabled {
            let (nl, nr) = self.sat.process(l, r, &p.sat);
            l = nl;
            r = nr;
        }

        // Modulation efter mättnaden: då sveper kammen i den färgade signalen,
        // och limitern sist tar hand om toppen återkopplingen kan ge.
        if p.chorus_enabled {
            let (nl, nr) = self.chorus.process(l, r, &p.chorus);
            l = nl;
            r = nr;
        }

        if p.flanger_enabled {
            let (nl, nr) = self.flanger.process(l, r, &p.flanger);
            l = nl;
            r = nr;
        }

        if p.phaser_enabled {
            let (nl, nr) = self.phaser.process(l, r, &p.phaser);
            l = nl;
            r = nr;
        }

        if p.doubler_enabled {
            let (nl, nr) = self.doubler.process(l, r, &p.doubler);
            l = nl;
            r = nr;
        }

        if p.limiter_enabled {
            let (nl, nr) = self.limiter.process(l, r, &p.limiter);
            l = nl;
            r = nr;
        }

        (l, r)
    }
}

// ---------------------------------------------------------------------------
// DJ performance FX: beat-repeat / reverse stutter and tape-stop varispeed
// ---------------------------------------------------------------------------

/// Beat-repeat / stutter / reverse effect for the Remix FX view.
///
/// When a mode is engaged the engine captures one subdivision of the master
/// bus (based on the project tempo) and loops it. Mode 4 plays the captured
/// slice backwards.
#[derive(Debug, Clone)]
pub struct RemixFx {
    pub mode: u8, // 0 = off, 1 = 1/4, 2 = 1/8, 3 = 1/16, 4 = reverse
    bpm: f32,
    sample_rate: f32,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    read_pos: usize,
    capture_len: usize,
    capturing: bool,
    active: bool,
}

impl RemixFx {
    pub fn new(sample_rate: f32) -> Self {
        let cap = 96_000; // ~2 s at 48 kHz, enough for 1 beat at 40 BPM
        Self {
            mode: 0,
            bpm: 120.0,
            sample_rate,
            buffer_l: vec![0.0; cap],
            buffer_r: vec![0.0; cap],
            write_pos: 0,
            read_pos: 0,
            capture_len: 0,
            capturing: false,
            active: false,
        }
    }

    pub fn set(&mut self, mode: u8, bpm: f32) {
        if bpm > 1.0 {
            self.bpm = bpm;
        }
        if mode != self.mode {
            self.mode = mode;
            self.active = mode != 0;
            self.capturing = self.active;
            self.write_pos = 0;
            self.read_pos = 0;
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        if !self.active {
            return (l, r);
        }
        let beat = 60.0 / self.bpm.max(20.0) * self.sample_rate;
        let seg = match self.mode {
            2 => beat * 0.5,
            3 => beat * 0.25,
            _ => beat,
        } as usize;
        self.capture_len = seg.clamp(64, self.buffer_l.len());

        if self.capturing {
            self.buffer_l[self.write_pos] = l;
            self.buffer_r[self.write_pos] = r;
            self.write_pos += 1;
            if self.write_pos >= self.capture_len {
                self.write_pos = 0;
                self.capturing = false;
                self.read_pos = 0;
            }
            return (l, r);
        }

        let (ol, or) = if self.mode == 4 {
            self.read_pos = if self.read_pos == 0 {
                self.capture_len - 1
            } else {
                self.read_pos - 1
            };
            (self.buffer_l[self.read_pos], self.buffer_r[self.read_pos])
        } else {
            let o = (self.buffer_l[self.read_pos], self.buffer_r[self.read_pos]);
            self.read_pos += 1;
            if self.read_pos >= self.capture_len {
                self.read_pos = 0;
            }
            o
        };
        (ol, or)
    }
}

impl Default for RemixFx {
    fn default() -> Self {
        Self::new(48_000.0)
    }
}

/// Tape-stop varispeed: ramps the master output playback rate to zero when
/// engaged and back to normal when released, producing the classic pitch-down
/// and stop.
#[derive(Debug, Clone)]
pub struct TapeStop {
    active: bool,
    rate: f32,
    sample_rate: f32,
    buffer_l: Vec<f32>,
    buffer_r: Vec<f32>,
    write_pos: usize,
    read_pos: f32,
}

impl TapeStop {
    pub fn new(sample_rate: f32) -> Self {
        let cap = 1 << 15;
        Self {
            active: false,
            rate: 1.0,
            sample_rate,
            buffer_l: vec![0.0; cap],
            buffer_r: vec![0.0; cap],
            write_pos: 0,
            read_pos: 0.0,
        }
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    #[inline]
    pub fn process(&mut self, l: f32, r: f32) -> (f32, f32) {
        let n = self.buffer_l.len();
        self.buffer_l[self.write_pos] = l;
        self.buffer_r[self.write_pos] = r;

        let target = if self.active { 0.0 } else { 1.0 };
        let step = 1.0 / (self.sample_rate * 0.8).max(1.0);
        if self.rate < target {
            self.rate = (self.rate + step).min(target);
        } else if self.rate > target {
            self.rate = (self.rate - step).max(target);
        }

        // Read at the current position *before* advancing it. `read_pos` starts
        // equal to `write_pos`, so at rate 1.0 the just-written sample passes
        // through with no added latency; as the rate falls, the read pointer
        // lags behind the write pointer and the pitch drops.
        let i0 = self.read_pos as usize % n;
        let i1 = (i0 + 1) % n;
        let frac = self.read_pos - self.read_pos.floor();
        let ol = self.buffer_l[i0] * (1.0 - frac) + self.buffer_l[i1] * frac;
        let or = self.buffer_r[i0] * (1.0 - frac) + self.buffer_r[i1] * frac;

        self.read_pos += self.rate;
        if self.read_pos >= n as f32 {
            self.read_pos -= n as f32;
        }

        self.write_pos = (self.write_pos + 1) % n;

        // Fade with the rate so a fully stopped tape is silent.
        let fade = self.rate.clamp(0.0, 1.0);
        (ol * fade, or * fade)
    }
}

impl Default for TapeStop {
    fn default() -> Self {
        Self::new(48_000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remix_fx_repeats_captured_slice() {
        let mut fx = RemixFx::new(48_000.0);
        // 120 BPM -> one beat = 24000 samples; capture then loop.
        fx.set(1, 120.0);
        let mut captured = Vec::new();
        for i in 0..24_000 {
            let x = (i as f32 / 24_000.0).sin();
            let (l, _) = fx.process(x, x);
            captured.push(l);
        }
        // Now looping: the output must repeat the captured slice.
        let mut a = 0.0;
        for _ in 0..1000 {
            let (l, _) = fx.process(0.0, 0.0);
            a = l;
        }
        assert!(captured.iter().any(|&v| (v - a).abs() < 1e-6));
    }

    #[test]
    fn tape_stop_is_transparent_at_normal_rate() {
        let mut ts = TapeStop::new(48_000.0);
        // Inactive -> rate 1.0: output must equal the input with no added
        // latency (regression for a full ring-buffer read-ahead).
        for i in 0..2000 {
            let x = ((i as f32) * 0.01).sin();
            let (l, r) = ts.process(x, -x);
            assert!(
                (l - x).abs() < 1e-6 && (r + x).abs() < 1e-6,
                "tape-stop must be a passthrough when idle (frame {i}: {l} vs {x})"
            );
        }
    }

    #[test]
    fn tape_stop_fades_to_silence() {
        let mut ts = TapeStop::new(48_000.0);
        ts.set_active(true);
        let mut last = 1.0;
        for _ in 0..48_000 {
            let (l, _) = ts.process(1.0, 1.0);
            last = l;
        }
        assert!(last.abs() < 1e-3, "expected silence, got {last}");
    }

    #[test]
    fn compressor_reduces_loud_signal() {
        let mut c = Compressor::new(48_000.0);
        let p = CompressorParams {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            makeup_db: 0.0,
        };
        let mut out = 0.0;
        for _ in 0..48_000 {
            let (l, _) = c.process(0.5, 0.5, &p);
            out = l;
        }
        assert!(out < 0.5 * 0.6, "compressor should pull 0.5 down, got {out}");
        assert!(out > 0.0);
    }

    #[test]
    fn compressor_bypassed_below_threshold() {
        let mut c = Compressor::new(48_000.0);
        let p = CompressorParams {
            threshold_db: -6.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            makeup_db: 0.0,
        };
        let mut out = 0.0;
        for _ in 0..48_000 {
            let (l, _) = c.process(0.01, 0.01, &p);
            out = l;
        }
        assert!((out - 0.01).abs() < 1e-6, "quiet signal should pass, got {out}");
    }

    #[test]
    fn limiter_respects_ceiling() {
        let mut lim = Limiter::new(48_000.0);
        let p = LimiterParams { ceiling: 0.5, release_ms: 80.0, boost: 2.0 };
        let mut peak = 0.0f32;
        for _ in 0..48_000 {
            let (l, r) = lim.process(1.0, -1.0, &p);
            peak = peak.max(l.abs()).max(r.abs());
        }
        assert!(peak <= 0.5 + 1e-4, "limiter exceeded ceiling: {peak}");
        assert!(peak > 0.4, "limiter should still pass level, got {peak}");
    }

    #[test]
    fn gate_closes_below_threshold_and_opens_above() {
        let mut gate = NoiseGate::new(48_000.0);
        let p = GateParams { threshold: 0.1, attack_ms: 5.0, release_ms: 120.0 };
        let mut quiet = 0.0;
        for _ in 0..96_000 {
            let (l, _) = gate.process(0.001, 0.001, &p);
            quiet = l;
        }
        assert!(quiet.abs() < 1e-4, "gate should mute sub-threshold signal, got {quiet}");

        let mut loud = 0.0;
        for _ in 0..48_000 {
            let (l, _) = gate.process(0.5, 0.5, &p);
            loud = l;
        }
        assert!((loud - 0.5).abs() < 0.02, "gate should open on loud signal, got {loud}");
    }

    #[test]
    fn master_chain_passes_signal_when_effects_off() {
        let mut chain = MasterFxChain::new(48_000.0);
        let params = MasterFxParams::default();
        chain.set_params(params);
        let mut out = 0.0;
        for _ in 0..1000 {
            let (l, _) = chain.process(0.3, 0.3);
            out = l;
        }
        assert!(out.is_finite() && out.abs() > 0.1, "dry chain should pass audio, got {out}");
        assert_eq!(chain.gain_reduction_db(), 0.0);
    }

    #[test]
    fn master_chain_reports_compressor_gain_reduction() {
        let mut chain = MasterFxChain::new(48_000.0);
        let mut params = MasterFxParams::default();
        params.comp_enabled = true;
        params.comp = CompressorParams {
            threshold_db: -20.0,
            ratio: 4.0,
            attack_ms: 1.0,
            release_ms: 50.0,
            makeup_db: 0.0,
        };
        chain.set_params(params);
        for _ in 0..48_000 {
            chain.process(0.5, 0.5);
        }
        assert!(chain.gain_reduction_db() < -1.0, "expected gain reduction, got {}", chain.gain_reduction_db());
    }

    /// Amplituden hos en enskild frekvens, mätt med Goertzel-formen (en bin av en DFT).
    fn tone_mag(x: &[f32], freq: f32, sr: f32) -> f32 {
        let w = 2.0 * std::f32::consts::PI * freq / sr;
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (i, s) in x.iter().enumerate() {
            let ph = w * i as f32;
            re += s * ph.cos();
            im += s * ph.sin();
        }
        2.0 * (re * re + im * im).sqrt() / x.len() as f32
    }

    #[test]
    fn transient_shaper_is_transparent_at_zero() {
        // 0 dB på båda reglagen måste ge exakt insignalen tillbaka — förstärkningen är
        // 10⁰ · 10⁰ = 1,0, alltså ingen avrundning alls.
        let mut ts = TransientShaper::new(48_000.0);
        let p = TransientParams::default();
        for i in 0..5_000 {
            let t = i as f32 / 48_000.0;
            let x = (t * 220.0 * 2.0 * std::f32::consts::PI).sin() * 0.7;
            let (l, r) = ts.process(x, -x, &p);
            assert_eq!(l, x, "frame {i}");
            assert_eq!(r, -x, "frame {i}");
        }
    }

    #[test]
    fn attack_boost_lifts_the_attack_of_a_hit() {
        // Ett anslag (1,0 följt av tystnad): med +12 dB ska första samplen upp, resten vara tyst.
        let mut ts = TransientShaper::new(48_000.0);
        let p = TransientParams { attack_db: 12.0, sustain_db: 0.0, sensitivity: 0.5 };
        let (l, _) = ts.process(1.0, 1.0, &p);
        assert!(l > 3.0, "anslaget skulle lyfts med ~4 gånger, fick {l}");
        let (l2, _) = ts.process(0.0, 0.0, &p);
        assert_eq!(l2, 0.0, "tystnad ska förbli tyst");
    }

    #[test]
    fn sustain_cut_lowers_a_steady_tone() {
        // En jämn ton har inget anslag: −12 dB kropp ska ge ~0,25 av insignalen, inte 1,0.
        let mut ts = TransientShaper::new(48_000.0);
        let p = TransientParams { attack_db: 0.0, sustain_db: -12.0, sensitivity: 0.5 };
        let mut out = Vec::new();
        for i in 0..48_000 {
            let t = i as f32 / 48_000.0;
            let x = (t * 220.0 * 2.0 * std::f32::consts::PI).sin() * 0.5;
            let (l, _) = ts.process(x, x, &p);
            out.push(l);
        }
        let tail = &out[24_000..];
        let rms = (tail.iter().map(|v| v * v).sum::<f32>() / tail.len() as f32).sqrt();
        let input_rms = 0.5 / 2.0_f32.sqrt();
        let ratio = rms / input_rms;
        assert!(
            (ratio - 0.251).abs() < 0.03,
            "jämn ton med −12 dB kropp blev {ratio:.3} av insignalen (väntat ~0,251)"
        );
    }

    #[test]
    fn saturator_is_transparent_at_zero_mix() {
        let mut sat = Saturator::new(48_000.0);
        let p = SaturatorParams { drive: 12.0, tone: 0.3, mix: 0.0 };
        for i in 0..1_000 {
            let x = (i as f32 * 0.01).sin();
            let (l, r) = sat.process(x, -x, &p);
            assert_eq!(l, x);
            assert_eq!(r, -x);
        }
    }

    #[test]
    fn saturation_harmonics_grow_with_drive() {
        let sr = 48_000.0;
        let measure = |drive: f32| {
            let mut sat = Saturator::new(sr);
            let p = SaturatorParams { drive, tone: 1.0, mix: 1.0 };
            let mut out = Vec::new();
            for i in 0..9_600 {
                let x = (i as f32 / sr * 1_000.0 * 2.0 * std::f32::consts::PI).sin() * 0.4;
                let (l, _) = sat.process(x, x, &p);
                out.push(l);
            }
            let f = tone_mag(&out[1_000..], 1_000.0, sr);
            let h3 = tone_mag(&out[1_000..], 3_000.0, sr);
            h3 / f
        };
        let (d2, d4, d8) = (measure(2.0), measure(4.0), measure(8.0));
        assert!(d2 < d4 && d4 < d8, "övertonerna ska växa med drive: {d2:.4} {d4:.4} {d8:.4}");
        assert!(d8 > 0.01, "drive 8 ska ge hörbar tredjeton, fick {d8:.4}");
    }

    #[test]
    fn saturator_never_exceeds_full_scale() {
        // Normaliseringen 1/tanh(drive) håller utgången inom ±1 även för insignal 10.
        let mut sat = Saturator::new(48_000.0);
        let p = SaturatorParams { drive: 20.0, tone: 1.0, mix: 1.0 };
        for i in 0..1_000 {
            let x = if i % 2 == 0 { 10.0 } else { -10.0 };
            let (l, r) = sat.process(x, -x, &p);
            assert!(l.abs() <= 1.0 + 1e-6, "utgång {l} över full skala");
            assert!(r.abs() <= 1.0 + 1e-6, "utgång {r} över full skala");
        }
    }

    #[test]
    fn multiband_crossover_is_flat() {
        // Delningsfiltret ska släppa igenom allt när inget band komprimeras: tre toner in,
        // samma tre toner ut. Det är summan low+mid+high som mäts, alltså filtret självt.
        let sr = 48_000.0;
        let mut mb = MultibandCompressor::new(sr);
        mb.set_params(MultibandParams {
            amounts: [0.0; 3],
            threshold_db: -50.0,
            ratio: 10.0,
            ..MultibandParams::default()
        });
        let bands = [(100.0f32, 0.3f32), (1_000.0, 0.2), (10_000.0, 0.1)];
        let mut out = Vec::new();
        for i in 0..48_000 {
            let t = i as f32 / sr;
            let x: f32 = bands
                .iter()
                .map(|(f, a)| (t * f * 2.0 * std::f32::consts::PI).sin() * a)
                .sum();
            let (l, _) = mb.process(x, x);
            out.push(l);
        }
        for (f, a) in bands {
            let got = tone_mag(&out[24_000..], f, sr);
            let ratio = got / a;
            assert!(
                (0.88..1.13).contains(&ratio),
                "{f} Hz blev {ratio:.3} av insignalen — delningen är inte platt"
            );
        }
    }

    #[test]
    fn a_loud_band_is_tamed_and_the_quiet_one_is_not() {
        // 100 Hz på 0,5 och 5 kHz på 0,02: bara basbandet ska sänkas.
        let sr = 48_000.0;
        let mut mb = MultibandCompressor::new(sr);
        mb.set_params(MultibandParams {
            amounts: [1.0, 0.0, 1.0],
            threshold_db: -24.0,
            ratio: 4.0,
            ..MultibandParams::default()
        });
        let mut out = Vec::new();
        for i in 0..48_000 {
            let t = i as f32 / sr;
            let x = (t * 100.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
                + (t * 5_000.0 * 2.0 * std::f32::consts::PI).sin() * 0.02;
            let (l, _) = mb.process(x, x);
            out.push(l);
        }
        let low = tone_mag(&out[24_000..], 100.0, sr) / 0.5;
        let high = tone_mag(&out[24_000..], 5_000.0, sr) / 0.02;
        assert!(low < 0.72, "basbandet skulle sänkts minst 3 dB, blev {low:.3}");
        assert!(
            (0.9..1.1).contains(&high),
            "diskanten skulle vara orörd, blev {high:.3}"
        );
    }

    #[test]
    fn convolver_without_ir_is_a_passthrough() {
        let mut c = Convolver::new(48_000.0);
        let p = ConvolverParams { mix: 1.0, gain_db: 0.0 };
        for i in 0..1_000 {
            let x = (i as f32 * 0.01).sin();
            let (l, r) = c.process(x, -x, &p);
            assert_eq!(l, x);
            assert_eq!(r, -x);
        }
    }

    #[test]
    fn convolver_delays_by_exactly_one_block() {
        // IR = [1,0,0…] ska ge insignalen tillbaka, fördröjd med **ett block** (CONV_BLOCK).
        // Latensen mäts här i stället för att döljas: den är priset för att svaret inte kan
        // räknas förrän ett helt block inkommit.
        let mut c = Convolver::new(48_000.0);
        c.set_ir(Some(Arc::new(vec![1.0])));
        let p = ConvolverParams { mix: 1.0, gain_db: 0.0 };
        let mut out = Vec::new();
        for i in 0..1_024 {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let (l, _) = c.process(x, x, &p);
            out.push(l);
        }
        assert!(
            out[..CONV_BLOCK].iter().all(|v| *v == 0.0),
            "innan första blocket är färdigt finns inget svar"
        );
        assert!((out[CONV_BLOCK] - 1.0).abs() < 1e-5, "impulsen ska komma på plats {}", CONV_BLOCK);
    }

    #[test]
    fn convolver_applies_the_ir_shape() {
        // En två-taps-respons [0,5, −0,5] ska ge två sampel med rätt tecken — alltså är det
        // filens respons som faltas in, inte en fördröjning.
        let mut c = Convolver::new(48_000.0);
        c.set_ir(Some(Arc::new(vec![0.5, -0.5])));
        let p = ConvolverParams { mix: 1.0, gain_db: 0.0 };
        let mut out = Vec::new();
        for i in 0..1_024 {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let (l, _) = c.process(x, x, &p);
            out.push(l);
        }
        assert!((out[CONV_BLOCK] - 0.5).abs() < 1e-5, "första tappen: {}", out[CONV_BLOCK]);
        assert!((out[CONV_BLOCK + 1] + 0.5).abs() < 1e-5, "andra tappen: {}", out[CONV_BLOCK + 1]);
        assert!(out[CONV_BLOCK + 2].abs() < 1e-5, "responsen är två sampel lång");
    }

    #[test]
    fn an_all_ones_pattern_is_transparent() {
        let mut g = StepGate::new(48_000.0);
        let p = GatePatternParams { enabled: true, ..GatePatternParams::default() };
        for i in 0..5_000 {
            let x = (i as f32 * 0.01).sin();
            let (l, r) = g.process(x, -x, &p);
            assert_eq!(l, x);
            assert_eq!(r, -x);
        }
    }

    #[test]
    fn a_gate_pattern_mutes_the_off_steps() {
        // 120 BPM → ett 1/16-steg är 6 000 sampel. Steg 0 öppet, resten stängt.
        let mut g = StepGate::new(48_000.0);
        let mut gains = [0.0f32; 16];
        gains[0] = 1.0;
        let p = GatePatternParams { gains, bpm: 120.0, smooth: 0.0, enabled: true };
        let mut open = Vec::new();
        let mut closed = Vec::new();
        for i in 0..12_000 {
            let x = (i as f32 * 0.05).sin();
            let (l, _) = g.process(x, x, &p);
            if i < 6_000 { open.push(l) } else { closed.push(l) }
        }
        let rms = |v: &[f32]| (v.iter().map(|s| s * s).sum::<f32>() / v.len() as f32).sqrt();
        assert!(rms(&open) > 0.5, "steg 0 ska släppa igenom, fick {}", rms(&open));
        assert_eq!(rms(&closed), 0.0, "steg 1 ska vara tyst");
    }

    #[test]
    fn mod_fx_is_transparent_at_zero_mix() {
        for kind in 0..3 {
            let mut chorus = ModDelay::new(48_000.0, 12.0, 8.0);
            let mut flanger = ModDelay::new(48_000.0, 2.5, 2.0);
            let mut phaser = Phaser::new(48_000.0);
            let p = ModFxParams { rate: 3.0, depth: 1.0, mix: 0.0, feedback: 0.7 };
            for i in 0..256 {
                let x = ((i as f32) * 0.01).sin();
                let out = match kind {
                    0 => chorus.process(x, -x, &p),
                    1 => flanger.process(x, -x, &p),
                    _ => phaser.process(x, -x, &p),
                };
                assert_eq!(out.0.to_bits(), x.to_bits(), "kind {kind} ändrade signalen vid mix 0");
                assert_eq!(out.1.to_bits(), (-x).to_bits(), "kind {kind} ändrade signalen vid mix 0");
            }
        }
    }

    #[test]
    fn chorus_sweeps_the_delay_and_flanger_sweeps_less() {
        let p = ModFxParams { rate: 2.0, depth: 1.0, mix: 0.5, feedback: 0.0 };
        let mut chorus = ModDelay::new(48_000.0, 12.0, 8.0);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for _ in 0..24_000 {
            chorus.process(0.0, 0.0, &p);
            lo = lo.min(chorus.delay_ms());
            hi = hi.max(chorus.delay_ms());
        }
        // 12 ms bas ± 8 ms svep: hela svepet ska rymmas, annars rör sig inte LFO:n.
        assert!(lo < 12.5, "nedre läget {lo} ms nådde inte basen");
        assert!(hi > 19.0, "övre läget {hi} ms nådde inte toppen");

        let mut flanger = ModDelay::new(48_000.0, 2.5, 2.0);
        let (mut flo, mut fhi) = (f32::MAX, f32::MIN);
        for _ in 0..24_000 {
            flanger.process(0.0, 0.0, &p);
            flo = flo.min(flanger.delay_ms());
            fhi = fhi.max(flanger.delay_ms());
        }
        assert!((fhi - flo) > 1.8, "flangern svepte bara {} ms", fhi - flo);
        assert!(fhi < 5.0, "flangern är för långsam: {} ms", fhi);
    }

    #[test]
    fn phaser_moves_a_notch_only_when_the_lfo_runs() {
        let sr = 48_000.0;
        let blocks = |depth: f32| -> f32 {
            let mut ph = Phaser::new(sr);
            let p = ModFxParams { rate: 2.0, depth, mix: 0.5, feedback: 0.0 };
            let mut levels = Vec::new();
            let block = 2_400; // 50 ms
            let mut acc = 0.0;
            let mut n = 0;
            for i in 0..(sr as usize) {
                let x = (2.0 * PI * 1_000.0 * i as f32 / sr).sin() * 0.5;
                let (out, _) = ph.process(x, x, &p);
                acc += out * out;
                n += 1;
                if n == block {
                    levels.push((acc / block as f32).sqrt());
                    acc = 0.0;
                    n = 0;
                }
            }
            let mean = levels.iter().sum::<f32>() / levels.len() as f32;
            let max = levels.iter().cloned().fold(f32::MIN, f32::max);
            let min = levels.iter().cloned().fold(f32::MAX, f32::min);
            (max - min) / mean.max(1e-6)
        };
        let still = blocks(0.0);
        let swept = blocks(1.0);
        // Stilla LFO = fast kam = jämn nivå. Svepande = kammen vandrar över tonen.
        assert!(still < 0.01, "nivån rörde sig {still} utan svep");
        assert!(swept > 10.0 * still, "svepet gav {swept}, stilla gav {still}");
        assert!(swept > 0.05, "svepet hörs knappt: {swept}");
    }


    #[test]
    fn bank_ir_is_bounded_finite_and_normalised() {
        for recipe in IR_BANK.iter() {
            let ir = synthesize_ir(recipe, 48_000.0);
            assert!(ir.len() > 1_000, "{}: {} sampel", recipe.name, ir.len());
            assert!(ir.iter().all(|v| v.is_finite()), "{}: icke-finita värden", recipe.name);
            let peak = ir.iter().fold(0.0f32, |m, v| m.max(v.abs()));
            assert!((peak - 1.0).abs() < 1e-3, "{}: topp {peak}", recipe.name);
        }
    }

    #[test]
    fn bank_ir_decays_and_the_hall_is_longer_than_the_room() {
        let energy = |ir: &[f32], from: f32, to: f32| -> f32 {
            let a = (ir.len() as f32 * from) as usize;
            let b = ((ir.len() as f32 * to) as usize).min(ir.len());
            ir[a..b].iter().map(|v| v * v).sum::<f32>() / (b - a).max(1) as f32
        };
        for recipe in IR_BANK.iter().filter(|r| !r.reverse) {
            let ir = synthesize_ir(recipe, 48_000.0);
            let head = energy(&ir, 0.10, 0.20);
            let tail = energy(&ir, 0.85, 0.95);
            assert!(head > 4.0 * tail, "{}: svansen dog inte ({head} mot {tail})", recipe.name);
        }
        let room = synthesize_ir(&IR_BANK[0], 48_000.0);
        let hall = synthesize_ir(&IR_BANK[3], 48_000.0);
        // Convolvern kapar vid CONV_MAX_LEN (2 s), så katedralens recept blir 96 000 sampel.
        assert!(hall.len() > 3 * room.len(), "katedralen är inte längre än rummet");
    }

    #[test]
    fn a_click_through_the_bank_leaves_a_tail() {
        let mut conv = Convolver::new(48_000.0);
        conv.set_ir(Some(std::sync::Arc::new(synthesize_ir(&IR_BANK[0], 48_000.0))));
        let p = ConvolverParams { mix: 1.0, gain_db: 0.0 };
        let mut out = Vec::new();
        for i in 0..(48_000 / 2) {
            let x = if i == 0 { 1.0 } else { 0.0 };
            out.push(conv.process(x, x, &p));
        }
        // Direktljudet ligger en blockfördröjning in; svansen ska leva långt efter det.
        let peak = out[CONV_BLOCK..CONV_BLOCK + 512]
            .iter()
            .fold(0.0f32, |m, (l, _)| m.max(l.abs()));
        let early: f32 = out[CONV_BLOCK..CONV_BLOCK + 256].iter().map(|(l, _)| l * l).sum();
        let late: f32 = out[6_000..6_256].iter().map(|(l, _)| l * l).sum();
        assert!(peak > 0.5, "inget direktljud alls (topp {peak})");
        assert!(late > 0.0, "svansen är tom 125 ms efter klicket");
        assert!(late < early, "svansen är starkare än direktljudet ({late} mot {early})");
    }

    /// **Granulizern spelar vidare efter att ljudet tystnat.** Ett 200 ms-burst matas in,
    /// sedan tystnad: med granulizern på hörs korn ur det som nyss spelades, med den av
    /// är utgången tyst. Det är skillnaden mellan en granulizer och en efterklang.
    #[test]
    fn the_granulizer_keeps_sounding_after_the_input_stops() {
        let tail_energy = |enabled: bool| {
            let mut chain = MasterFxChain::new(48_000.0);
            let mut params = MasterFxParams::default();
            params.granular_enabled = enabled;
            params.granular = GranularParams {
                size_ms: 80.0,
                density: 20.0,
                position_ms: 100.0,
                spray_ms: 30.0,
                speed: 1.0,
                mix: 1.0,
            };
            chain.set_params(params);
            for i in 0..9_600 {
                let x = (2.0 * std::f32::consts::PI * 1_000.0 * i as f32 / 48_000.0).sin() * 0.5;
                chain.process(x, x);
            }
            let mut sum = 0.0f64;
            for _ in 0..9_600 {
                let (l, _) = chain.process(0.0, 0.0);
                sum += (l as f64).abs();
            }
            sum / 9_600.0
        };
        let off = tail_energy(false);
        let on = tail_energy(true);
        println!("medelutgång efter tystnaden: av {off:.5}, på {on:.5}");
        assert!(off < 1e-6, "utan granulizer ska det vara tyst: {off:.6}");
        assert!(on > 0.01, "med granulizer ska kornen höras: {on:.5}");
    }

    /// **Kornens hastighet transponerar.** Ett enda korn spelas in med 500 Hz och spelas upp
    /// med hastighet 2,0 — då ska kornet innehålla 1 kHz. Kornet mäts för sig (låg täthet,
    /// långt korn), eftersom överlappande korn med olika fas summa inte ger en ren ton.
    #[test]
    fn the_granular_speed_transposes_the_grains() {
        let render = |speed: f32| {
            let mut chain = MasterFxChain::new(48_000.0);
            let mut params = MasterFxParams::default();
            params.granular_enabled = true;
            params.granular = GranularParams {
                size_ms: 200.0,
                density: 2.0,
                position_ms: 100.0,
                spray_ms: 0.0,
                speed,
                mix: 1.0,
            };
            chain.set_params(params);
            let mut out = Vec::new();
            for i in 0..96_000 {
                let x = (2.0 * std::f32::consts::PI * 500.0 * i as f32 / 48_000.0).sin() * 0.5;
                let (l, _) = chain.process(x, x);
                out.push(l);
            }
            out
        };
        // Grundton i ett fönster, med autokorrelation. Första toppen, inte högsta — en
        // periodisk signal toppar också på två och tre perioder.
        let f0 = |samples: &[f32], sr: f32| -> f32 {
            let (min_lag, max_lag) = ((sr / 3_000.0) as usize, (sr / 200.0) as usize);
            let mut scores = Vec::new();
            for lag in min_lag..max_lag {
                let mut dot = 0.0f32;
                let mut energy = 0.0f32;
                for i in 0..samples.len() - lag {
                    dot += samples[i] * samples[i + lag];
                    energy += samples[i] * samples[i];
                }
                scores.push((lag, dot / (energy + 1e-9)));
            }
            let best = scores.iter().map(|(_, v)| *v).fold(f32::NEG_INFINITY, f32::max);
            let lag = scores
                .iter()
                .find(|(_, v)| *v >= best * 0.9)
                .map(|(l, _)| *l)
                .unwrap_or(min_lag);
            sr / lag as f32
        };
        // Korn nummer två börjar vid 24000 sampel; mät mitten av det.
        let plain = render(1.0);
        let double = render(2.0);
        let a = f0(&plain[28_000..32_000], 48_000.0);
        let b = f0(&double[28_000..32_000], 48_000.0);
        println!("kornets tonhöjd: 1,0× {a:.0} Hz, 2,0× {b:.0} Hz");
        // Absolutvärdena ligger några procent högt (autokorrelationen har heltalsfördröjning),
        // men **kvoten** är det som mäts: två gånger hastigheten ska ge två gånger frekvensen.
        assert!(a > 400.0, "kornet ska höras: {a:.0} Hz");
        let ratio = b / a;
        assert!(
            (ratio - 2.0).abs() < 0.1,
            "dubbel hastighet ska ge dubbel frekvens: {ratio:.3} ({a:.0} Hz mot {b:.0} Hz)"
        );
    }

}
