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

        self.read_pos += self.rate;
        if self.read_pos >= n as f32 {
            self.read_pos -= n as f32;
        }
        let i0 = self.read_pos as usize % n;
        let i1 = (i0 + 1) % n;
        let frac = self.read_pos - self.read_pos.floor();
        let ol = self.buffer_l[i0] * (1.0 - frac) + self.buffer_l[i1] * frac;
        let or = self.buffer_r[i0] * (1.0 - frac) + self.buffer_r[i1] * frac;

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
}
