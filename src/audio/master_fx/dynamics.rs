use super::*;

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
    pub(crate) gain_reduction_db: f32,
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
    /// **Egen tröskel per band**, som avvikelse i dB från `threshold_db`. Låg band behöver
    /// ofta en lägre tröskel än high för samma hörda kompression — det är hela poängen med
    /// Maximus, och en gemensam tröskel kan inte uttrycka det.
    pub offsets_db: [f32; 3],
    /// **Master-bandet** (FL:s MASTER-knapp i Maximus): en kompressor över summan av de tre
    /// banden, efter dem. Den limmar ihop det bandkompressionen delat upp — och den är ett eget
    /// steg, inte en fjärde ratt på banden: 1,0 komprimerar även när alla band står på 0.
    pub master_amount: f32,
    /// Master-bandets tröskel som avvikelse i dB från `threshold_db` (som bandens).
    pub master_offset_db: f32,
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
            offsets_db: [0.0; 3],
            master_amount: 0.0,
            master_offset_db: 0.0,
        }
    }
}

pub(crate) struct XoverCoeffs {
    f1_lp: BiquadCoeffs,
    f1_hp: BiquadCoeffs,
    f2_lp: BiquadCoeffs,
    f2_hp: BiquadCoeffs,
}

impl XoverCoeffs {
    pub(crate) fn new(sr: f32, f1: f32, f2: f32) -> Self {
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
pub(crate) struct XoverChan {
    f1_lp: [BiquadState; 2],
    f1_hp: [BiquadState; 2],
    f2_lp: [BiquadState; 2],
    f2_hp: [BiquadState; 2],
}

impl XoverChan {
    #[inline(always)]
    pub(crate) fn split(&mut self, x: f32, c: &XoverCoeffs) -> (f32, f32, f32) {
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
    /// Master-bandet: en kompressor över summan, efter de tre banden.
    master: Compressor,
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
            master: Compressor::new(sample_rate),
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

        let mut out = (0.0f32, 0.0f32);
        for (i, (dl, dr)) in dry.iter().enumerate() {
            let band_params = CompressorParams {
                threshold_db: p.threshold_db + p.offsets_db[i],
                ratio: p.ratio.max(1.0),
                attack_ms: p.attack_ms,
                release_ms: p.release_ms,
                makeup_db: 0.0,
            };
            let (wl, wr) = self.bands[i].process(*dl, *dr, &band_params);
            let a = p.amounts[i].clamp(0.0, 1.0);
            out.0 += dl + a * (wl - dl);
            out.1 += dr + a * (wr - dr);
        }

        // Master-bandet sist: det hör summan, inte delarna.
        let m = p.master_amount.clamp(0.0, 1.0);
        if m > 0.0 {
            let master_params = CompressorParams {
                threshold_db: p.threshold_db + p.master_offset_db,
                ratio: p.ratio.max(1.0),
                attack_ms: p.attack_ms,
                release_ms: p.release_ms,
                makeup_db: 0.0,
            };
            let (ml, mr) = self.master.process(out.0, out.1, &master_params);
            out.0 += m * (ml - out.0);
            out.1 += m * (mr - out.1);
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
