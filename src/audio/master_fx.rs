//! Real-time master bus FX chain and per-track equalizer DSP.
//!
//! Everything here is allocation-free while processing and designed to run on
//! the audio thread. The parameters are plain `Copy` structs so they can be
//! shipped through the lock-free command ring buffer without cloning buffers.

use std::f32::consts::PI;

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
    pub multiband: MultibandParams,
    pub multiband_enabled: bool,
    pub gate: GateParams,
    pub gate_enabled: bool,
    pub filter: MasterFilterParams,
    pub limiter: LimiterParams,
    pub limiter_enabled: bool,
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
            multiband: MultibandParams::default(),
            multiband_enabled: false,
            gate: GateParams::default(),
            gate_enabled: false,
            filter: MasterFilterParams::default(),
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
    multiband: MultibandCompressor,
    gate: NoiseGate,
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
            multiband: MultibandCompressor::new(sample_rate),
            gate: NoiseGate::new(sample_rate),
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

        if p.sat_enabled {
            let (nl, nr) = self.sat.process(l, r, &p.sat);
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
}
