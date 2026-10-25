use super::dynamics::{XoverChan, XoverCoeffs};
use super::*;

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
    let len = ((recipe.seconds.max(0.05) * sr) as usize).clamp(64, CONV_MAX_LEN);

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

    for (n, slot) in ir.iter_mut().enumerate().skip(predelay) {
        let t = (n - predelay) as f32 / sr;
        let env = (-t / tau).exp();
        // Mörkare mot slutet: damping styr hur mycket brytfrekvensen faller.
        let progress = (n as f32 / len as f32).clamp(0.0, 1.0);
        let k = (1.0 - recipe.damping.clamp(0.0, 1.0) * progress).clamp(0.05, 1.0) * 0.6;
        lp += k * (noise() - lp);
        *slot = lp * env;
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
// Convolution reverb (FL Studio: Fruity Convolver)
// ---------------------------------------------------------------------------

/// Blocklängd för den partitionerade faltningen. 256 sampel = **5,3 ms** vid 48 kHz, och
/// det är samtidigt våt-signalens latens (se [`Convolver`]). Blocket är en avvägning: mindre
/// ger kortare latens men fler FFT:er per sekund.
pub(crate) const CONV_BLOCK: usize = 256;
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

// ---------------------------------------------------------------------------
// Love Philter (FL: Fruity Love Philter)
// ---------------------------------------------------------------------------

/// **Fruity Love Philter**: tre filter i **serie** kring en gemensam brytfrekvens —
/// lågpass under, bandpass i mitten, högpass över — med en envelopföljare som flyttar
/// hela banken. `spread` är avståndet mellan banden i oktaver, `env_octaves` hur långt
/// ett anslag lyfter banken. Det är banken och svepningen som är effekten; en enda
/// SVF hade varit `analog_filter`. Serie och inte parallellt: tre parallella band på
/// samma brytfrekvens summerar tillbaka till ingången och blir då en genomsläppning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LovePhilterParams {
    pub cutoff: f32,      // Hz, bankens mitt
    pub spread: f32,      // oktaver mellan banden (0 = alla på samma)
    pub resonance: f32,   // Q
    pub attack_ms: f32,
    pub release_ms: f32,
    pub env_octaves: f32, // 0 = ingen svepning
    pub mix: f32,
    pub enabled: bool,
}

impl Default for LovePhilterParams {
    fn default() -> Self {
        Self {
            cutoff: 1000.0,
            spread: 1.5,
            resonance: 0.9,
            attack_ms: 10.0,
            release_ms: 120.0,
            env_octaves: 0.0,
            mix: 1.0,
            enabled: false,
        }
    }
}

pub struct LovePhilter {
    sample_rate: f32,
    /// `[kanal][band]`: 0 = lågpass, 1 = bandpass, 2 = högpass. Eget tillstånd per kanal,
    /// annars hade vänster och höger filtrerat varandras sampel.
    bands: [[StateVariableFilter; 3]; 2],
    envelope: f32,
    attack_coeff: f32,
    release_coeff: f32,
}

fn envelope_coeff(ms: f32, sample_rate: f32) -> f32 {
    let n = (ms * 0.001 * sample_rate).max(1.0);
    1.0 - (-1.0 / n).exp()
}

impl LovePhilter {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            bands: [[StateVariableFilter::new(sample_rate); 3]; 2],
            envelope: 0.0,
            attack_coeff: 1.0,
            release_coeff: 1.0,
        }
    }

    pub fn set_params(&mut self, p: &LovePhilterParams) {
        self.attack_coeff = envelope_coeff(p.attack_ms, self.sample_rate);
        self.release_coeff = envelope_coeff(p.release_ms, self.sample_rate);
    }

    /// Envelopens öppning 0–1. Läses av provet, inte av gränssnittet.
    #[cfg(test)]
    pub fn envelope(&self) -> f32 {
        self.envelope
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &LovePhilterParams) -> (f32, f32) {
        if !p.enabled {
            return (l, r);
        }
        let peak = l.abs().max(r.abs()).min(4.0);
        let coeff = if peak > self.envelope { self.attack_coeff } else { self.release_coeff };
        self.envelope += (peak - self.envelope) * coeff;
        // Roten gör att ett svagt anslag ändå öppnar: amplitud 0,25 ger halva vägen.
        let open = self.envelope.sqrt().min(1.0) * p.env_octaves.max(0.0);
        let spread = p.spread.clamp(0.0, 4.0);
        let q = p.resonance.clamp(0.2, 8.0);
        let mix = p.mix.clamp(0.0, 1.0);
        let mut wet = [0.0_f32; 2];
        for (ch, x) in [l, r].into_iter().enumerate() {
            let mut y = x;
            for band in 0..3 {
                let oct = match band {
                    0 => -spread,
                    1 => 0.0,
                    _ => spread,
                };
                let cutoff = (p.cutoff * 2.0_f32.powf(oct + open))
                    .clamp(20.0, self.sample_rate * 0.45);
                let fp = FilterParams { cutoff, resonance: q };
                y = match band {
                    0 => self.bands[ch][band].process_lowpass(y, &fp),
                    1 => self.bands[ch][band].process_bandpass(y, &fp),
                    _ => self.bands[ch][band].process_highpass(y, &fp),
                };
            }
            wet[ch] = y;
        }
        (l * (1.0 - mix) + wet[0] * mix, r * (1.0 - mix) + wet[1] * mix)
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

pub struct MultibandDelay {
    sample_rate: f32,
    coeffs: XoverCoeffs,
    /// Delningsfrekvenserna som `coeffs` räknades för.
    xover: (f32, f32),
    xover_l: XoverChan,
    xover_r: XoverChan,
    /// En ringbuffert per band och kanal: 0–2 vänster, 3–5 höger.
    buf: [Vec<f32>; 6],
    write: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MultibandDelayParams {
    pub times_ms: [f32; 3],
    pub xover_low: f32,
    pub xover_high: f32,
    pub mix: f32,
}

impl Default for MultibandDelayParams {
    fn default() -> Self {
        Self {
            times_ms: [125.0, 250.0, 375.0],
            xover_low: 250.0,
            xover_high: 4000.0,
            mix: 0.35,
        }
    }
}

impl MultibandDelay {
    const MAX_MS: f32 = 1000.0;

    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let cap = (Self::MAX_MS * 0.001 * sr).ceil() as usize + 4;
        let (lo, hi) = (250.0, 4000.0);
        Self {
            sample_rate: sr,
            coeffs: XoverCoeffs::new(sr, lo, hi),
            xover: (lo, hi),
            xover_l: XoverChan::default(),
            xover_r: XoverChan::default(),
            buf: std::array::from_fn(|_| vec![0.0; cap]),
            write: 0,
        }
    }

    pub fn process(&mut self, l: f32, r: f32, p: &MultibandDelayParams) -> (f32, f32) {
        let (lo, hi) = (p.xover_low.clamp(40.0, 1000.0), p.xover_high.clamp(1000.0, 16000.0));
        if (lo, hi) != self.xover {
            self.coeffs = XoverCoeffs::new(self.sample_rate, lo, hi);
            self.xover = (lo, hi);
        }
        let (low_l, mid_l, high_l) = self.xover_l.split(l, &self.coeffs);
        let (low_r, mid_r, high_r) = self.xover_r.split(r, &self.coeffs);
        for (band, (bl, br)) in [(low_l, low_r), (mid_l, mid_r), (high_l, high_r)]
            .into_iter()
            .enumerate()
        {
            self.buf[band][self.write] = bl;
            self.buf[band + 3][self.write] = br;
        }

        let len = self.buf[0].len();
        let (mut wet_l, mut wet_r) = (0.0, 0.0);
        for (band, &ms) in p.times_ms.iter().enumerate() {
            let samples = ((ms.clamp(1.0, Self::MAX_MS) * 0.001 * self.sample_rate) as usize)
                .clamp(1, len - 1);
            let idx = (self.write + len - samples) % len;
            wet_l += self.buf[band][idx];
            wet_r += self.buf[band + 3][idx];
        }
        self.write = (self.write + 1) % len;

        let mix = p.mix.clamp(0.0, 1.0);
        (l + wet_l * mix, r + wet_r * mix)
    }
}

/// **Frekvensskifte** — inte tonhöjd. En 1 kHz-ton som skiftas +200 Hz blir 1 200 Hz och
/// ingenting annat; en tonhöjdstransponering hade gett 1 200 Hz *och* ändrat formanterna.
/// Det är skillnaden som gör effekten användbar för att flytta ett helt spektrum utan att
/// röra intervallerna inom det.
///
/// Bygger på en Hilbert-transform (FIR med 63 tapp, Hamming-fönster): den ger signalen i
/// kvadrat, 90° fasförskjuten, och då räcker en komplex multiplikation för att välja **en**
/// sidofrekvens i stället för två.
pub struct FrequencyShifter {
    sample_rate: f32,
    buf_l: Vec<f32>,
    buf_r: Vec<f32>,
    write: usize,
    h: Vec<f32>,
    phase_l: f32,
    phase_r: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FreqShiftParams {
    /// Skiftet i hertz. Negativt flyttar spektrumet nedåt.
    pub hz: f32,
    pub mix: f32,
    /// Vänster kanal upp och höger ned (eller tvärtom): samma spektrum på båda sidor blir
    /// bredare än en vanlig panorering, eftersom kanalerna inte längre är samma signal.
    pub spread: bool,
}

impl Default for FreqShiftParams {
    fn default() -> Self {
        Self {
            hz: 0.0,
            mix: 1.0,
            spread: false,
        }
    }
}

/// **Bitcrush och decimering** på en gång (FL: Fruity Squeeze): BITS kvantiserar amplituden,
/// NEDSAMPLING håller kvar varje sampel ett antal steg. Kvantiseringen ger harmonisk
/// distorsion, nedblandningen ger vikta spegelbilder — två olika ljud, därför två rattar.
pub struct BitCrusher {
    hold: f32,
    last_l: f32,
    last_r: f32,
    lp_l: f32,
    lp_r: f32,
    sample_rate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BitCrushParams {
    /// 1–16 bitar. 16 är i praktiken ofärgat.
    pub bits: f32,
    /// 1–32: hur många sampel varje värde hålls.
    pub downsample: f32,
    /// Ett enpoligt lågpass efter krossen, 200 Hz–20 kHz. Tar bort det vassaste av
    /// vikningen utan att röra kvantiseringen.
    pub tone: f32,
    pub mix: f32,
}

impl Default for BitCrushParams {
    fn default() -> Self {
        Self {
            bits: 16.0,
            downsample: 1.0,
            tone: 1.0,
            mix: 1.0,
        }
    }
}

impl BitCrusher {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            hold: 0.0,
            last_l: 0.0,
            last_r: 0.0,
            lp_l: 0.0,
            lp_r: 0.0,
            sample_rate: sample_rate.max(1.0),
        }
    }

    pub fn process(&mut self, l: f32, r: f32, p: &BitCrushParams) -> (f32, f32) {
        let step = p.downsample.clamp(1.0, 32.0).round().max(1.0);
        self.hold -= 1.0;
        if self.hold <= 0.0 {
            self.hold = step;
            let levels = 2.0f32.powf(p.bits.clamp(1.0, 16.0)).max(2.0);
            let quantize = |x: f32| (x.clamp(-1.0, 1.0) * levels).round() / levels;
            self.last_l = quantize(l);
            self.last_r = quantize(r);
        }
        let mix = p.mix.clamp(0.0, 1.0);
        let (mut out_l, mut out_r) = (
            l * (1.0 - mix) + self.last_l * mix,
            r * (1.0 - mix) + self.last_r * mix,
        );
        // TON = 1,0 är öppet (20 kHz), 0,0 är 200 Hz. Koefficienten är en enpolig RC.
        // Öppet läge är **exakt** öppet: annars hade en enpoligt filtrerad 20 kHz-kurva
        // färgat ljudet även när man inte bett om det.
        let a = if p.tone >= 0.999 {
            1.0
        } else {
            let cutoff = 200.0 * (10.0f32).powf(p.tone.clamp(0.0, 1.0) * 2.0);
            (1.0 - (-2.0 * std::f32::consts::PI * cutoff / self.sample_rate).exp()).clamp(0.0, 1.0)
        };
        self.lp_l += a * (out_l - self.lp_l);
        self.lp_r += a * (out_r - self.lp_r);
        out_l = self.lp_l;
        out_r = self.lp_r;
        (out_l, out_r)
    }
}

/// Fasackumulatorn hålls inom en period, annars tappar `sin_cos` precision efter några minuter.
fn advance(phase: f32, hz: f32, sample_rate: f32) -> f32 {
    let mut next = phase + 2.0 * std::f32::consts::PI * hz / sample_rate;
    if next > std::f32::consts::TAU {
        next -= std::f32::consts::TAU;
    } else if next < 0.0 {
        next += std::f32::consts::TAU;
    }
    next
}

impl FrequencyShifter {
    const TAPS: usize = 63;

    pub fn new(sample_rate: f32) -> Self {
        let n = Self::TAPS;
        let center = (n - 1) as f32 / 2.0;
        let h: Vec<f32> = (0..n)
            .map(|i| {
                let d = i as f32 - center;
                if d.abs() < 1e-6 {
                    return 0.0;
                }
                let s = (std::f32::consts::PI * d / 2.0).sin();
                let window =
                    0.54 - 0.46 * (2.0 * std::f32::consts::PI * i as f32 / (n - 1) as f32).cos();
                2.0 * s * s / (std::f32::consts::PI * d) * window
            })
            .collect();
        Self {
            sample_rate: sample_rate.max(1.0),
            buf_l: vec![0.0; n],
            buf_r: vec![0.0; n],
            write: 0,
            h,
            phase_l: 0.0,
            phase_r: 0.0,
        }
    }

    pub fn process(&mut self, l: f32, r: f32, p: &FreqShiftParams) -> (f32, f32) {
        let n = self.h.len();
        let center = n / 2;
        self.buf_l[self.write] = l;
        self.buf_r[self.write] = r;
        let (mut ql, mut qr) = (0.0f32, 0.0f32);
        for (i, &coeff) in self.h.iter().enumerate() {
            let idx = (self.write + n - i) % n;
            ql += coeff * self.buf_l[idx];
            qr += coeff * self.buf_r[idx];
        }
        // Torrsignalen läses vid filtrets mittpunkt: annars hade skiftet fått en fördröjning
        // mot det torra och blandningen blivit ett kamfilter.
        let dry_idx = (self.write + n - center) % n;
        let (dl, dr) = (self.buf_l[dry_idx], self.buf_r[dry_idx]);
        self.write = (self.write + 1) % n;

        let hz_r = if p.spread { -p.hz } else { p.hz };
        let (sn_l, cs_l) = self.phase_l.sin_cos();
        let (sn_r, cs_r) = self.phase_r.sin_cos();
        self.phase_l = advance(self.phase_l, p.hz, self.sample_rate);
        self.phase_r = advance(self.phase_r, hz_r, self.sample_rate);

        let wet_l = dl * cs_l - ql * sn_l;
        let wet_r = dr * cs_r - qr * sn_r;
        let mix = p.mix.clamp(0.0, 1.0);
        (
            dl * (1.0 - mix) + wet_l * mix,
            dr * (1.0 - mix) + wet_r * mix,
        )
    }
}

/// **Distructor** (FL): tre distorsioner med egen drive, kopplade i serie eller parallellt,
/// med ett tonläge efter.
///
/// Serie staplar kurvorna: varje steg får det föregående stegets redan klippta signal.
/// Parallellt ger varje steg **ingångssignalen** i stället, vilket låter mer än man kan tro —
/// vid hårt drive kan vikningssteget då arbeta på en renare signal och ge **fler** övertoner
/// än serien, inte färre. Skillnaden mellan lägena är alltså karaktär, inte mängd.
pub struct Distructor {
    sample_rate: f32,
    lp_l: f32,
    lp_r: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DistructorParams {
    /// Drive per steg, 0–1 → 1× till 40×.
    pub drive: [f32; 3],
    /// 0 = serie, 1 = parallellt.
    pub parallel: f32,
    /// Tonlägets knä i Hz (enpoligt lågpass efter distorsionen).
    pub tone_hz: f32,
    pub mix: f32,
}

impl Default for DistructorParams {
    fn default() -> Self {
        Self {
            drive: [0.35, 0.35, 0.35],
            parallel: 0.0,
            tone_hz: 6_000.0,
            mix: 0.5,
        }
    }
}

/// Kurvorna de tre stegen använder: mjuk mättnad, hård klippning och vikning.
fn distructor_curve(stage: usize, x: f32) -> f32 {
    match stage {
        0 => x.tanh(),
        1 => x.clamp(-1.0, 1.0),
        _ => {
            // Vikning: över −1..1 vänds kurvan tillbaka, vilket ger en vassare övertonston
            // än både tanh och klippning.
            let folded = (x + 1.0).rem_euclid(4.0);
            let y = if folded <= 2.0 { folded } else { 4.0 - folded };
            y - 1.0
        }
    }
}

impl Distructor {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate: sample_rate.max(1.0),
            lp_l: 0.0,
            lp_r: 0.0,
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &DistructorParams) -> (f32, f32) {
        // **Nivån kompenseras per steg.** Utan det hade tre steg i serie med 40× var klättrat
        // till full klippning redan vid lite drive, och ratten hade inte gjort någon skillnad:
        // allt över 0,1 lät likadant. Nu normaliseras varje steg mot sin egen kurva vid
        // fullskala, så det är **kurvans** form som ändras med drivet, inte bara nivån.
        // **Drivet är stegets förstärkning in i kurvan.** Lågt drive ska ligga under
        // klipptröskeln — annars viker sig kurvan redan vid 0,1 och ratten gör ingenting.
        let gain = |i: usize| 0.2 + p.drive[i].clamp(0.0, 1.0) * 7.8;
        let stage_out = |stage: usize, x: f32| distructor_curve(stage, x * gain(stage));
        let shape = |x: f32| -> f32 {
            let parallel = p.parallel.clamp(0.0, 1.0);
            let mut series = x;
            for stage in 0..3 {
                series = stage_out(stage, series);
            }
            if parallel <= 0.0 {
                return series;
            }
            // Parallellt: varje steg på **samma** ingång, summerade med samma vikt.
            let mixed: f32 = (0..3).map(|stage| stage_out(stage, x)).sum::<f32>() / 3.0;
            series * (1.0 - parallel) + mixed * parallel
        };

        let (dl, dr) = (shape(l), shape(r));
        let a = (1.0
            - (-2.0 * std::f32::consts::PI * p.tone_hz.clamp(200.0, 20_000.0) / self.sample_rate)
                .exp())
        .clamp(0.0, 1.0);
        self.lp_l += a * (dl - self.lp_l);
        self.lp_r += a * (dr - self.lp_r);
        let mix = p.mix.clamp(0.0, 1.0);
        (
            l * (1.0 - mix) + self.lp_l * mix,
            r * (1.0 - mix) + self.lp_r * mix,
        )
    }
}

/// **Hardcore** (FL): gitarreffektkedja i serie — kompressor, distorsion, wah och delay, i den
/// ordningen. Varje steg är en av de kretsar som redan finns i masterkedjan: kedjan är
/// sammansättningen, inte fyra nya effekter. Ordningen är den som gör ljudet: distorsion före
/// wah ger en vokalliknande kurva, wah före distorsion ger ett hårdare, plattare ljud.
pub struct Hardcore {
    comp: Compressor,
    dist: Distructor,
    wah_l: [BiquadState; 2],
    wah_r: [BiquadState; 2],
    wah_coeffs: BiquadCoeffs,
    wah_hz: f32,
    delay: DelayBank,
    sample_rate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HardcoreParams {
    /// Kompressionens mängd, 0 = av.
    pub comp: f32,
    /// Distorsionens drive, 0 = rent.
    pub drive: f32,
    /// Wah: bandpassets centrum i Hz.
    pub wah_hz: f32,
    /// Wah: hur mycket bandpasset blandas in, 0 = av.
    pub wah_depth: f32,
    /// Delay: tid i ms, 0 = av.
    pub delay_ms: f32,
    pub delay_mix: f32,
    pub mix: f32,
}

impl Default for HardcoreParams {
    fn default() -> Self {
        Self {
            comp: 0.3,
            drive: 0.35,
            wah_hz: 1_200.0,
            wah_depth: 0.0,
            delay_ms: 220.0,
            delay_mix: 0.0,
            mix: 1.0,
        }
    }
}

/// RBJ-bandpass (0 dB topp, Q styr bredden).
fn bandpass_coeffs(sr: f32, freq: f32, q: f32) -> BiquadCoeffs {
    let w0 = 2.0 * std::f32::consts::PI * freq.clamp(20.0, sr * 0.45) / sr;
    let (sin_w0, cos_w0) = w0.sin_cos();
    let alpha = sin_w0 / (2.0 * q.max(0.1));
    let a0 = 1.0 + alpha;
    BiquadCoeffs {
        b0: alpha / a0,
        b1: 0.0,
        b2: -alpha / a0,
        a1: -2.0 * cos_w0 / a0,
        a2: (1.0 - alpha) / a0,
    }
}

impl Hardcore {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        Self {
            comp: Compressor::new(sr),
            dist: Distructor::new(sr),
            wah_l: [BiquadState::default(); 2],
            wah_r: [BiquadState::default(); 2],
            wah_coeffs: bandpass_coeffs(sr, 1_200.0, 1.4),
            wah_hz: 1_200.0,
            delay: DelayBank::new(sr),
            sample_rate: sr,
        }
    }

    pub fn process(&mut self, l: f32, r: f32, p: &HardcoreParams) -> (f32, f32) {
        let (mut l, mut r) = (l, r);

        // 1. Kompressor: pedalkedjans första steg, så distorsionen får jämnare insignal.
        if p.comp > 0.001 {
            let amount = p.comp.clamp(0.0, 1.0);
            let params = CompressorParams {
                threshold_db: -6.0 - 18.0 * amount,
                ratio: 1.0 + 7.0 * amount,
                ..CompressorParams::default()
            };
            let (nl, nr) = self.comp.process(l, r, &params);
            l = nl;
            r = nr;
        }

        // 2. Distorsion: samma tre kurvor som Distructorn, men alla tre på samma drive.
        if p.drive > 0.001 {
            let (nl, nr) = self.dist.process(
                l,
                r,
                &DistructorParams {
                    drive: [p.drive.clamp(0.0, 1.0); 3],
                    parallel: 0.0,
                    tone_hz: 9_000.0,
                    mix: 1.0,
                },
            );
            l = nl;
            r = nr;
        }

        // 3. Wah: bandpass ovanpå signalen. Två biquads i rad ger en brantare kurva, alltså
        //    en mer uttalad "röst" i stället för en mjuk tonkontroll.
        if p.wah_depth > 0.001 {
            if (p.wah_hz - self.wah_hz).abs() > 1.0 {
                self.wah_coeffs = bandpass_coeffs(self.sample_rate, p.wah_hz, 1.4);
                self.wah_hz = p.wah_hz;
            }
            let mut wl = l;
            let mut wr = r;
            for i in 0..2 {
                wl = self.wah_l[i].process(wl, &self.wah_coeffs);
                wr = self.wah_r[i].process(wr, &self.wah_coeffs);
            }
            let depth = p.wah_depth.clamp(0.0, 1.0);
            l = l * (1.0 - depth) + wl * depth * 2.0;
            r = r * (1.0 - depth) + wr * depth * 2.0;
        }

        // 4. Delay: sist, så ekot bär hela kedjans färg.
        if p.delay_ms > 1.0 && p.delay_mix > 0.001 {
            let (nl, nr) = self.delay.process(
                l,
                r,
                &DelayBankParams {
                    times_ms: [p.delay_ms, p.delay_ms * 2.0, p.delay_ms * 3.0],
                    levels: [1.0, 0.4, 0.2],
                    feedback: 0.3,
                    spread: 0.4,
                    mix: p.delay_mix.clamp(0.0, 1.0),
                },
            );
            l = nl;
            r = nr;
        }

        let mix = p.mix.clamp(0.0, 1.0);
        (l * mix, r * mix)
    }
}

/// **Effector** (FL): en XY-styrd performance-effekt. Två rattar styr allt — X är "hur mycket",
/// Y är "hur hårt" — och läget avgör vilken krets de pekar på. Det är inte åtta effekter i en
/// låda: det är fyra kretsar som redan finns, med sina viktigaste parametrar dragna ut till en
/// yta man kan spela på.
pub struct Effector {
    filter: MasterFilter,
    delay: DelayBank,
    crusher: BitCrusher,
    shift: FrequencyShifter,
    gate_phase: f32,
    sample_rate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectorParams {
    /// X: hur mycket. Betydelsen beror på läget.
    pub x: f32,
    /// Y: hur hårt. Sekundär parametern.
    pub y: f32,
    /// 0 = svep, 1 = upprepning, 2 = port, 3 = kross.
    pub mode: u8,
    pub mix: f32,
}

impl Default for EffectorParams {
    fn default() -> Self {
        Self {
            x: 0.5,
            y: 0.3,
            mode: 0,
            mix: 1.0,
        }
    }
}

impl Effector {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        Self {
            filter: MasterFilter::new(sr),
            delay: DelayBank::new(sr),
            crusher: BitCrusher::new(sr),
            shift: FrequencyShifter::new(sr),
            gate_phase: 0.0,
            sample_rate: sr,
        }
    }

    pub fn process(&mut self, l: f32, r: f32, p: &EffectorParams) -> (f32, f32) {
        let x = p.x.clamp(0.0, 1.0);
        let y = p.y.clamp(0.0, 1.0);
        let (mut out_l, mut out_r) = match p.mode {
            1 => {
                // Upprepning: X är tiden, Y är återkopplingen.
                let time = 60.0 + x * 540.0;
                self.delay.process(
                    l,
                    r,
                    &DelayBankParams {
                        times_ms: [time, time * 2.0, time * 3.0],
                        levels: [1.0, 0.5, 0.25],
                        feedback: 0.15 + y * 0.75,
                        spread: 0.3,
                        mix: 0.6,
                    },
                )
            }
            2 => {
                // Port: X är frekvensen, Y är djupet. Fyrkantig grind — en port, inte en
                // tremolo, så den klipper i stället för att svepa.
                let rate = 2.0 + x * 14.0;
                self.gate_phase += rate / self.sample_rate;
                if self.gate_phase >= 1.0 {
                    self.gate_phase -= 1.0;
                }
                let open = if self.gate_phase < 0.5 { 1.0 } else { 1.0 - y };
                (l * open, r * open)
            }
            3 => {
                // Kross: X är bitdjupet, Y är frekvensskiftet.
                let (cl, cr) = self.crusher.process(
                    l,
                    r,
                    &BitCrushParams {
                        bits: 8.0 - x * 7.0,
                        downsample: 1.0 + x * 8.0,
                        tone: 1.0,
                        mix: 1.0,
                    },
                );
                self.shift.process(
                    cl,
                    cr,
                    &FreqShiftParams {
                        hz: y * 800.0,
                        mix: 1.0,
                        spread: false,
                    },
                )
            }
            _ => {
                // Svep: X är delningen, Y är resonansen.
                let cutoff = 200.0 * (10.0f32).powf(x * 1.8);
                self.filter.process(
                    l,
                    r,
                    &MasterFilterParams {
                        cutoff,
                        resonance: 0.1 + y * 0.8,
                        drive: 1.0 + y * 3.0,
                        enabled: true,
                    },
                )
            }
        };
        let mix = p.mix.clamp(0.0, 1.0);
        if mix < 1.0 {
            out_l = l * (1.0 - mix) + out_l * mix;
            out_r = r * (1.0 - mix) + out_r * mix;
        }
        (out_l, out_r)
    }
}

/// **Fruity Phase Inverter** och **Fruity Stereo Shaper** i en krets: polaritet per kanal,
/// bredd och balans. De är samma räkning på samma två tal, så de delar en struct — FL har dem
/// som två plugins, men en plugin som bara vänder på fasen är en inställning, inte en effekt.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StereoParams {
    /// 0 = mono, 1 = som inspelat, 2 = dubbelt så brett.
    pub width: f32,
    /// −1 = allt till vänster, 0 = jämnt, 1 = allt till höger.
    pub balance: f32,
    pub invert_l: bool,
    pub invert_r: bool,
    /// **Fruity Center:** −1 tar bort mitten (bara sidan kvar), +1 tar bort sidorna
    /// (bara mitten kvar). 0 är neutralt.
    pub center: f32,
    pub mix: f32,
}

impl Default for StereoParams {
    fn default() -> Self {
        Self {
            width: 1.0,
            balance: 0.0,
            invert_l: false,
            invert_r: false,
            center: 0.0,
            mix: 1.0,
        }
    }
}

impl StereoParams {
    /// **Bredden verkar på sidan, inte på kanalerna**: mitt och sida skalas var för sig, så
    /// mitten står still när bredden ändras. Balansen är en ren viktning efteråt.
    pub fn apply(&self, l: f32, r: f32) -> (f32, f32) {
        let c = self.center.clamp(-1.0, 1.0);
        let mid = (l + r) * 0.5 * (1.0 + c).max(0.0);
        let side = (l - r) * 0.5 * self.width.clamp(0.0, 2.0) * (1.0 - c.max(0.0));
        let (mut l, mut r) = (mid + side, mid - side);
        if self.invert_l {
            l = -l;
        }
        if self.invert_r {
            r = -r;
        }
        // Balans: ena sidan dämpas, den andra står still — som en panorering, inte som en
        // total volymändring.
        let b = self.balance.clamp(-1.0, 1.0);
        if b > 0.0 {
            l *= 1.0 - b;
        } else if b < 0.0 {
            r *= 1.0 + b;
        }
        (l, r)
    }
}

/// Två hyllor: en nedre och en övre. Det är vad **Low Lifter** (bara den nedre) och en
/// **tonkontur** i största allmänhet är — samma biquad-form, olika frekvens och riktning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShelvesParams {
    pub low_hz: f32,
    pub low_db: f32,
    pub high_hz: f32,
    pub high_db: f32,
    pub mix: f32,
}

impl Default for ShelvesParams {
    fn default() -> Self {
        Self {
            low_hz: 90.0,
            low_db: 0.0,
            high_hz: 6_000.0,
            high_db: 0.0,
            mix: 1.0,
        }
    }
}

/// RBJ-hylla. `gain_db` > 0 lyfter, < 0 sänker; `slope` 1,0 är en mjuk hylla.
fn shelf_coeffs(sr: f32, freq: f32, gain_db: f32, high: bool) -> BiquadCoeffs {
    let a = (10.0f32).powf(gain_db / 40.0);
    let w0 = 2.0 * std::f32::consts::PI * freq.clamp(20.0, sr * 0.45) / sr;
    let (sin_w0, cos_w0) = w0.sin_cos();
    let alpha = sin_w0 / 2.0 * (2.0f32).sqrt();
    let two_sqrt_a_alpha = 2.0 * a.sqrt() * alpha;
    if high {
        // Övre hylla: nämnaren har MINUS framför (A−1)cos — det är enda skillnaden mot den
        // nedre, och att låna den nedres nämnare ger en hylla som bara lyfter halvvägs.
        let a0 = (a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        BiquadCoeffs {
            b0: (a * ((a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha)) / a0,
            b1: (-2.0 * a * ((a - 1.0) + (a + 1.0) * cos_w0)) / a0,
            b2: (a * ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha)) / a0,
            a1: (2.0 * ((a - 1.0) - (a + 1.0) * cos_w0)) / a0,
            a2: ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha) / a0,
        }
    } else {
        let a0 = (a + 1.0) + (a - 1.0) * cos_w0 + two_sqrt_a_alpha;
        BiquadCoeffs {
            b0: (a * ((a + 1.0) - (a - 1.0) * cos_w0 + two_sqrt_a_alpha)) / a0,
            b1: (2.0 * a * ((a - 1.0) - (a + 1.0) * cos_w0)) / a0,
            b2: (a * ((a + 1.0) - (a - 1.0) * cos_w0 - two_sqrt_a_alpha)) / a0,
            a1: (-2.0 * ((a - 1.0) + (a + 1.0) * cos_w0)) / a0,
            a2: ((a + 1.0) + (a - 1.0) * cos_w0 - two_sqrt_a_alpha) / a0,
        }
    }
}

/// De två hyllorna med sina tillstånd, så att de kan ligga i masterkedjan.
pub struct Shelves {
    low_l: BiquadState,
    low_r: BiquadState,
    high_l: BiquadState,
    high_r: BiquadState,
    low_c: BiquadCoeffs,
    high_c: BiquadCoeffs,
    cached: (f32, f32, f32, f32),
    sample_rate: f32,
}

impl Shelves {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let p = ShelvesParams::default();
        Self {
            low_l: BiquadState::default(),
            low_r: BiquadState::default(),
            high_l: BiquadState::default(),
            high_r: BiquadState::default(),
            low_c: shelf_coeffs(sr, p.low_hz, p.low_db, false),
            high_c: shelf_coeffs(sr, p.high_hz, p.high_db, true),
            cached: (p.low_hz, p.low_db, p.high_hz, p.high_db),
            sample_rate: sr,
        }
    }

    pub fn process(&mut self, l: f32, r: f32, p: &ShelvesParams) -> (f32, f32) {
        // Koefficienterna räknas bara om när en ratt flyttats — en hylla som räknas om varje
        // sampel kostar mer än den gör nytta.
        if (p.low_hz, p.low_db, p.high_hz, p.high_db) != self.cached {
            self.low_c = shelf_coeffs(self.sample_rate, p.low_hz, p.low_db, false);
            self.high_c = shelf_coeffs(self.sample_rate, p.high_hz, p.high_db, true);
            self.cached = (p.low_hz, p.low_db, p.high_hz, p.high_db);
        }
        let mut out_l = l;
        let mut out_r = r;
        if p.low_db.abs() > 0.01 {
            out_l = self.low_l.process(out_l, &self.low_c);
            out_r = self.low_r.process(out_r, &self.low_c);
        }
        if p.high_db.abs() > 0.01 {
            out_l = self.high_l.process(out_l, &self.high_c);
            out_r = self.high_r.process(out_r, &self.high_c);
        }
        let mix = p.mix.clamp(0.0, 1.0);
        (l * (1.0 - mix) + out_l * mix, r * (1.0 - mix) + out_r * mix)
    }
}
