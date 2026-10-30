//! Cytomic-inspirerade filtertopologier: Sallen-Key, tillståndsvariabel med 2x oversampling och
//! ladder med poldiffusion.
//!
//! Alla tre är stereo (eget tillstånd per kanal), stateful och räknar f32 per sampel, som resten
//! av `builtin_fx`. Konventionerna är de befintliga modulernas: cutoff i Hz (20 … 20 kHz),
//! resonans 0 … 1 normaliserad och drive 0 … 1.
//!
//! Skillnaden mot våra tidigare filter (`filters.rs`, `filter.rs`) är **var mättnaden sitter**:
//! här ligger drivern i resonansåterkopplingen i stället för efter utgången. Det är därför dessa
//! självsvänger och därför hård drive ändrar klangen vid hög resonans — testmodulen mäter THD
//! med drive 0 och 1 på samma sinus.
//!
//! Resonansen mappas till dämpningen `k = 1/Q`: `2,0` (Q = 0,5) vid ratten i botten och ett
//! **litet negativt** värde vid full resonans. Negativ dämpning är själva självsvängningen;
//! tillstånden kläms vid ±1 (skenorna) så den landar på en nivå i stället för att växa.
//!
//! **Stabilitet vid 44,1 kHz:** cutoff kläms till `20 Hz … 0,45·sr` (så `g = tan(π·fc/sr) ≤ 6,3`),
//! `k` kläms till `-0,35 … 2,0`, tillstånden kläms vid ±1, alla icke-linjära återkopplingar går
//! genom en `tanh` som är begränsad till ±1/gain, och fastpunktsiterationerna har
//! kontraktionsfaktor under 1 för alla rattlägen. Ingen kombination av rattar kan därför få
//! tillståndet att växa obegränsat.
//!
//! Sallen-Key och SVF delar kärna: en tvåpolig Sallen-Key-lågpass och en tvåpolig
//! tillståndsvariabel har *samma* överföringsfunktion — det som skiljer kretsarna åt är var
//! mättnaden sitter och att SVF:en räknas på 2x rate. Att låtsas något annat hade varit två kopior
//! av samma ekvation med olika kommentar.

use std::f32::consts::PI;

/// Högsta cutoff som andel av samplerns frekvens.
const MAX_SR_RATIO: f32 = 0.45;

/// Poldiffusion: stegen i laddern sitter inte exakt på samma cutoff. Några procents spridning
/// räcker för att knät ska bli mjukare och resonansen mindre vass (Abletons `LadderT_PRD`).
const POLE_SPREAD: [f32; 4] = [0.93, 0.98, 1.02, 1.07];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterMode {
    LowPass,
    BandPass,
    HighPass,
    /// Bandstopp = högpass + lågpass ur samma kärna (`x - k·band`).
    Notch,
}

impl FilterMode {
    /// Etikett för UI:t. Anropas ännu inte — `CYTOMIC`-parametern är en sifferstege.
    #[allow(dead_code)]
    pub fn name(self) -> &'static str {
        match self {
            Self::LowPass => "lågpass",
            Self::BandPass => "bandpass",
            Self::HighPass => "högpass",
            Self::Notch => "notch",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FilterTopology {
    /// 12 dB/okt, tvåpolig resonans, drive i återkopplingen.
    SallenKeyMs,
    /// Tvåpolig SVF med LP/BP/HP/notch, resonansvägen räknas på 2x rate.
    StateVariableOs,
    /// Fyrpolig transistorladder med poldiffusion och drive i loopen. Bara lågpass.
    LadderPd,
}

impl FilterTopology {
    pub const ALL: [Self; 3] = [Self::SallenKeyMs, Self::StateVariableOs, Self::LadderPd];

    /// Etikett för UI:t. Anropas ännu inte — `CYTOMIC`-parametern är en sifferstege.
    #[allow(dead_code)]
    pub fn name(self) -> &'static str {
        match self {
            Self::SallenKeyMs => "Sallen-Key MS2",
            Self::StateVariableOs => "SVF OSR 2x",
            Self::LadderPd => "Ladder PD",
        }
    }

    pub fn make(self, sample_rate: f32) -> Box<dyn FilterDsp> {
        match self {
            Self::SallenKeyMs => Box::new(SallenKeyMs::new(sample_rate)),
            Self::StateVariableOs => Box::new(StateVariableOs::new(sample_rate)),
            Self::LadderPd => Box::new(LadderPd::new(sample_rate)),
        }
    }
}

/// Det topologierna har gemensamt. `process` tar en kanal i taget så tillståndet är per kanal;
/// `process_stereo` är den väg effektkedjan använder.
pub trait FilterDsp: Send {
    fn set_mode(&mut self, mode: FilterMode);
    /// Alla tre parametrarna på en gång, som `Moog` räknar om sina koefficienter per block.
    fn set(&mut self, cutoff_hz: f32, resonance: f32, drive: f32);
    fn process(&mut self, channel: usize, x: f32) -> f32;
    fn reset(&mut self);

    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        for ch in 0..2 {
            let buf = if ch == 0 { &mut *left } else { &mut *right };
            for x in buf.iter_mut() {
                *x = self.process(ch, *x);
            }
        }
    }
}

/// `g = tan(π·fc/sr)`, förskjuten så knät hamnar där ratten står även nära Nyquist.
fn g_at(cutoff_hz: f32, sample_rate: f32) -> f32 {
    (PI * cutoff_hz.clamp(20.0, sample_rate * MAX_SR_RATIO) / sample_rate).tan()
}

/// Dämpningen `k = 1/Q`. Negativ dämpning vid full resonans är självsvängningen.
fn k_at(resonance: f32) -> f32 {
    (2.0 - 2.35 * resonance.clamp(0.0, 1.0)).clamp(-0.35, 2.0)
}

/// Mättnad i återkopplingen: `tanh(gain·v)/gain`. Delningen gör att lutningen vid noll är 1
/// oavsett drive, så fastpunktsiterationen konvergerar lika bra på full drive — och att drivern
/// klipper *formen* i loopen i stället för att bara skruva upp nivån.
#[inline]
fn sat(v: f32, gain: f32) -> f32 {
    (gain * v).tanh() / gain
}

/// -120 dB brus i ingången. Självsvängningen behöver något att växa ur, precis som termiskt brus
/// i en riktig krets, och det håller tillståndet borta från denormala tal.
#[derive(Clone, Copy)]
struct Dither(u32);

impl Default for Dither {
    fn default() -> Self {
        Self(0x2545_F491)
    }
}

impl Dither {
    #[inline]
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        ((self.0 >> 9) as f32 * (2.0 / 8_388_607.0) - 1.0) * 1.0e-6
    }
}

/// Kärnans skenor. Utan en gräns på själva energin har en självsvängande loop ingen nivå att
/// landa på: mättnaden i dämpvägen tar bort dämpningen i stället för att sätta en amplitud.
/// Klämningen är **hård** och inte en mjuk kurva — en mjuk kurva dämpar tillståndet varje sampel
/// även under skenan, vilket läcker likspänning och sänker passbandet.
const RAIL: f32 = 1.0;

#[inline]
fn rail(v: f32) -> f32 {
    v.clamp(-RAIL, RAIL)
}

/// Två bilinjära integratorer: `g` förskjuten, `k` som dämpning. `sat` läggs på bandpasset i
/// dämpvägen — det är där en Sallen-Key har sin återkoppling och där drivern sitter.
#[derive(Clone, Copy, Default)]
struct SvfCore {
    ic1: f32,
    ic2: f32,
}

impl SvfCore {
    /// `(band, lågpass, högpass)`. Fastpunkten på `v1` konvergerar eftersom felet multipliceras
    /// med `g·|k|/(1+g²) ≤ 1,17` och startgissningen redan är den linjära lösningen.
    #[inline]
    fn step(&mut self, x: f32, g: f32, k: f32, sat_gain: f32) -> (f32, f32, f32) {
        let den = 1.0 + g * g;
        let num = self.ic1 + g * (x - self.ic2);
        let mut v1 = num / den;
        for _ in 0..3 {
            v1 = (num - g * k * sat(v1, sat_gain)) / den;
        }
        let v1 = rail(v1);
        let v2 = rail(self.ic2 + g * v1);
        self.ic1 = 2.0 * v1 - self.ic1;
        self.ic2 = 2.0 * v2 - self.ic2;
        let hp = x - k * sat(v1, sat_gain) - v2;
        (v1, v2, hp)
    }
}

#[inline]
fn tap(mode: FilterMode, bp: f32, lp: f32, hp: f32) -> f32 {
    match mode {
        FilterMode::LowPass => lp,
        FilterMode::BandPass => bp,
        FilterMode::HighPass => hp,
        FilterMode::Notch => hp + lp,
    }
}

// ---------------------------------------------------------------------------
// Sallen-Key, 12 dB/okt multimode med drive i återkopplingen
// ---------------------------------------------------------------------------

pub struct SallenKeyMs {
    sr: f32,
    cutoff: f32,
    res: f32,
    drive: f32,
    mode: FilterMode,
    g: f32,
    k: f32,
    sat_gain: f32,
    chan: [SkCh; 2],
}

#[derive(Clone, Copy, Default)]
struct SkCh {
    core: SvfCore,
    dither: Dither,
}

impl SallenKeyMs {
    pub fn new(sample_rate: f32) -> Self {
        let mut f = Self {
            sr: sample_rate,
            cutoff: 1_000.0,
            res: 0.3,
            drive: 0.0,
            mode: FilterMode::LowPass,
            g: 0.0,
            k: 0.0,
            sat_gain: 1.0,
            chan: [SkCh::default(); 2],
        };
        f.update();
        f
    }

    fn update(&mut self) {
        self.g = g_at(self.cutoff, self.sr);
        self.k = k_at(self.res);
        self.sat_gain = 1.0 + 15.0 * self.drive.clamp(0.0, 1.0);
    }
}

impl FilterDsp for SallenKeyMs {
    fn set_mode(&mut self, mode: FilterMode) {
        self.mode = mode;
    }

    fn set(&mut self, cutoff_hz: f32, resonance: f32, drive: f32) {
        self.cutoff = cutoff_hz;
        self.res = resonance;
        self.drive = drive;
        self.update();
    }

    fn process(&mut self, channel: usize, x: f32) -> f32 {
        let c = &mut self.chan[channel & 1];
        let x = x + c.dither.next();
        let (bp, lp, hp) = c.core.step(x, self.g, self.k, self.sat_gain);
        tap(self.mode, bp, lp, hp)
    }

    fn reset(&mut self) {
        self.chan = [SkCh::default(); 2];
    }
}

// ---------------------------------------------------------------------------
// Tillståndsvariabel med resonansvägen på 2x rate
// ---------------------------------------------------------------------------

pub struct StateVariableOs {
    sr: f32,
    cutoff: f32,
    res: f32,
    mode: FilterMode,
    /// `g` räknat för dubbla samplingsfrekvensen.
    g2: f32,
    k: f32,
    chan: [OsCh; 2],
}

#[derive(Clone, Copy, Default)]
struct OsCh {
    core: SvfCore,
    prev: f32,
    dither: Dither,
}

impl StateVariableOs {
    pub fn new(sample_rate: f32) -> Self {
        let mut f = Self {
            sr: sample_rate,
            cutoff: 1_000.0,
            res: 0.3,
            mode: FilterMode::LowPass,
            g2: 0.0,
            k: 0.0,
            chan: [OsCh::default(); 2],
        };
        f.update();
        f
    }

    fn update(&mut self) {
        // Hela kärnan räknas på 2x rate, alltså ligger resonansvägen inuti översamplingen.
        self.g2 = g_at(self.cutoff, self.sr * 2.0);
        self.k = k_at(self.res);
    }
}

impl FilterDsp for StateVariableOs {
    fn set_mode(&mut self, mode: FilterMode) {
        self.mode = mode;
    }

    /// `drive` ignoreras: den här topologin har ingen drivenhet i loopen, bara mättnad som håller
    /// självsvängningen begränsad (fast gain 1).
    fn set(&mut self, cutoff_hz: f32, resonance: f32, _drive: f32) {
        self.cutoff = cutoff_hz;
        self.res = resonance;
        self.update();
    }

    fn process(&mut self, channel: usize, x: f32) -> f32 {
        let c = &mut self.chan[channel & 1];
        let x = x + c.dither.next();
        // Ingången interpoleras linjärt mellan samplen, så delsteg 1 får medelvärdet.
        c.core.step(0.5 * (c.prev + x), self.g2, self.k, 1.0);
        let (bp, lp, hp) = c.core.step(x, self.g2, self.k, 1.0);
        c.prev = x;
        tap(self.mode, bp, lp, hp)
    }

    fn reset(&mut self) {
        self.chan = [OsCh::default(); 2];
    }
}

// ---------------------------------------------------------------------------
// Fyrpolig ladder med poldiffusion
// ---------------------------------------------------------------------------

pub struct LadderPd {
    sr: f32,
    cutoff: f32,
    res: f32,
    drive: f32,
    g: [f32; 4],
    /// Produkten av stegens `G = g/(1+g)`: kedjans gain utan tillstånd, behövs i ZDF-lösningen.
    chain_gain: f32,
    fb: f32,
    sat_gain: f32,
    chan: [LadderCh; 2],
}

#[derive(Clone, Copy, Default)]
struct LadderCh {
    s: [f32; 4],
    dither: Dither,
}

impl LadderPd {
    pub fn new(sample_rate: f32) -> Self {
        let mut f = Self {
            sr: sample_rate,
            cutoff: 1_200.0,
            res: 0.3,
            drive: 0.0,
            g: [0.0; 4],
            chain_gain: 0.0,
            fb: 0.0,
            sat_gain: 1.0,
            chan: [LadderCh::default(); 2],
        };
        f.update();
        f
    }

    fn update(&mut self) {
        let g = g_at(self.cutoff, self.sr);
        for (i, gi) in self.g.iter_mut().enumerate() {
            *gi = g * POLE_SPREAD[i];
        }
        self.chain_gain = self.g.iter().map(|g| g / (1.0 + g)).product();
        self.fb = 4.6 * self.res.clamp(0.0, 1.0);
        self.sat_gain = 1.0 + 15.0 * self.drive.clamp(0.0, 1.0);
    }
}

impl FilterDsp for LadderPd {
    /// Laddern har bara lågpass; moden ignoreras.
    fn set_mode(&mut self, _mode: FilterMode) {}

    fn set(&mut self, cutoff_hz: f32, resonance: f32, drive: f32) {
        self.cutoff = cutoff_hz;
        self.res = resonance;
        self.drive = drive;
        self.update();
    }

    fn process(&mut self, channel: usize, x: f32) -> f32 {
        let c = &mut self.chan[channel & 1];
        let x = x + c.dither.next();

        // Kedjans stilleståndsterm: y4 = chain_gain·u1 + b, där b kommer ur stegens tillstånd.
        let mut b = 0.0;
        for (i, &gi) in self.g.iter().enumerate() {
            let (gg, pp) = (gi / (1.0 + gi), 1.0 / (1.0 + gi));
            b = gg * b + pp * c.s[i];
        }

        // ZDF: u1 = x - fb·sat(y4 - x). Återkopplingen drar ifrån insignalen själv så likspänning
        // inte läcker ut som en nivåberoende förstärkning. Dämpad fastpunkt (0,5) konvergerar även
        // när fb·chain_gain > 1, vilket är precis vad som händer vid hög resonans.
        let mut u1 = x - self.fb * b;
        for _ in 0..4 {
            let y4 = self.chain_gain * u1 + b;
            let u = x - self.fb * sat(y4 - x, self.sat_gain);
            u1 = 0.5 * (u1 + u);
        }

        let mut y = u1;
        for (i, &gi) in self.g.iter().enumerate() {
            let (gg, pp) = (gi / (1.0 + gi), 1.0 / (1.0 + gi));
            let out = gg * y + pp * c.s[i];
            c.s[i] = 2.0 * gg * y + (1.0 - 2.0 * gg) * c.s[i];
            y = out;
        }
        y
    }

    fn reset(&mut self) {
        self.chan = [LadderCh::default(); 2];
    }
}

// ---------------------------------------------------------------------------
// Mätning
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44_100.0;
    /// Låg nivå: mätningen ska se den linjära kurvan, inte skenorna.
    const AMP: f32 = 0.05;

    fn tone(amp: f32, freq: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| amp * (2.0 * PI * freq * i as f32 / SR).sin()).collect()
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    /// Amplituden av den delton som ligger på `freq`, mätt med ett Hann-fönstrat DFT-utslag så
    /// läckage från grundtonen inte räknas som överton i THD-mätningen.
    fn amplitude(x: &[f32], freq: f32) -> f32 {
        let w = 2.0 * PI * freq / SR;
        let n = x.len() as f32;
        let (mut s, mut c, mut wsum) = (0.0f32, 0.0f32, 0.0f32);
        for (i, v) in x.iter().enumerate() {
            let hann = 0.5 - 0.5 * (2.0 * PI * i as f32 / n).cos();
            let ph = w * i as f32;
            s += hann * v * ph.sin();
            c += hann * v * ph.cos();
            wsum += hann;
        }
        2.0 * (s * s + c * c).sqrt() / wsum
    }

    /// En sekund sinus genom filtret; utsignalen mäts i sista fjärdedelen (efter svängningen).
    #[allow(clippy::too_many_arguments)]
    fn run(
        amp: f32,
        topo: FilterTopology,
        mode: FilterMode,
        cutoff: f32,
        res: f32,
        drive: f32,
        freq: f32,
    ) -> Vec<f32> {
        let n = SR as usize;
        let mut l = tone(amp, freq, n);
        let mut r = l.clone();
        let mut f = topo.make(SR);
        f.set_mode(mode);
        f.set(cutoff, res, drive);
        f.process_stereo(&mut l, &mut r);
        l[n * 3 / 4..].to_vec()
    }

    fn gain_db(topo: FilterTopology, mode: FilterMode, cutoff: f32, res: f32, drive: f32, freq: f32) -> f32 {
        20.0 * (amplitude(&run(AMP, topo, mode, cutoff, res, drive, freq), freq) / AMP).log10()
    }

    /// (a) Magnitudrespons per topologi och mod. Skriver ut dB på en frekvensgrid, -3 dB-punkten
    /// och resonanstoppen, och kräver att lågpasset släpper igenom basen och dämpar > 12 dB/okt.
    #[test]
    fn the_magnitude_response_has_the_slope_each_topology_promises() {
        let cutoff = 500.0;
        let res = 0.3;
        let grid: Vec<f32> = (0..=72).map(|i| 20.0 * 2.0_f32.powf(i as f32 / 8.0)).collect();

        println!("\n=== magnitudrespons: cutoff {cutoff} Hz, resonans {res} ===");
        for topo in FilterTopology::ALL {
            for mode in [FilterMode::LowPass, FilterMode::BandPass, FilterMode::HighPass, FilterMode::Notch] {
                if topo == FilterTopology::LadderPd && mode != FilterMode::LowPass {
                    continue;
                }
                let resp: Vec<(f32, f32)> =
                    grid.iter().map(|&f| (f, gain_db(topo, mode, cutoff, res, 0.0, f))).collect();
                let peak = resp.iter().fold(f32::MIN, |a, &(_, d)| a.max(d));
                let peak_hz = resp.iter().find(|&&(_, d)| d >= peak - 0.05).map(|&(f, _)| f).unwrap_or(0.0);

                // -3 dB-punkten: första korsningen under toppen, interpolerad i log-frekvens.
                let mut minus3 = f32::NAN;
                for pair in resp.windows(2) {
                    let ((f1, d1), (f2, d2)) = (pair[0], pair[1]);
                    if d1 >= peak - 3.0 && d2 < peak - 3.0 {
                        minus3 = f1 * (f2 / f1).powf((d1 - (peak - 3.0)) / (d1 - d2));
                        break;
                    }
                }

                let slope = gain_db(topo, mode, cutoff, res, 0.0, cutoff * 8.0)
                    - gain_db(topo, mode, cutoff, res, 0.0, cutoff * 4.0);
                let near: Vec<String> =
                    [100.0, 200.0, 400.0, 800.0, 1_600.0, 3_200.0, 6_400.0].iter()
                        .map(|&f| format!("{f:.0}Hz {:.1}", gain_db(topo, mode, cutoff, res, 0.0, f)))
                        .collect();
                // En högpasskant har ingen -3 dB-korsning under toppen; skriv streck i stället för NaN.
                let minus3_text =
                    if minus3.is_finite() { format!("{minus3:.0} Hz") } else { "-".to_string() };
                println!(
                    "{:<15} {:<9} topp {peak:+6.1} dB @ {peak_hz:>6.0} Hz  -3 dB {:>7}  \
                     lutning {slope:>6.1} dB/okt   [{}]",
                    topo.name(),
                    mode.name(),
                    minus3_text,
                    near.join(", ")
                );

                let low = gain_db(topo, mode, cutoff, res, 0.0, 50.0);
                match mode {
                    // Lågpasset måste släppa igenom basen och stupa enligt ordningen.
                    FilterMode::LowPass => {
                        assert!(low > -3.0, "{}: 50 Hz gav {low:.1} dB", topo.name());
                        if topo == FilterTopology::LadderPd {
                            // Utan återkoppling är laddern fyra ettpoliga steg: -12 dB vid cutoff.
                            assert!(minus3 < cutoff, "ladder: -3 dB först vid {minus3:.0} Hz");
                            assert!(slope < -20.0, "ladder: lutning {slope:.1} dB/okt");
                        } else {
                            assert!(
                                (minus3 / cutoff - 1.0).abs() < 0.25,
                                "{}: -3 dB-punkt {minus3:.0} Hz mot cutoff {cutoff} Hz",
                                topo.name()
                            );
                            assert!(slope < -11.0, "{}: lutning {slope:.1} dB/okt", topo.name());
                        }
                    }
                    // Bandpass har en topp vid cutoff, högpasset stupar nedåt i basen och notchen
                    // gräver ett hål där.
                    FilterMode::BandPass => {
                        let (below, above) =
                            (gain_db(topo, mode, cutoff, res, 0.0, 100.0), gain_db(topo, mode, cutoff, res, 0.0, 6_400.0));
                        assert!(peak > below + 6.0 && peak > above + 6.0, "{}: topp {peak:.1} dB", topo.name());
                    }
                    FilterMode::HighPass => {
                        assert!(low < -20.0, "{}: högpass 50 Hz gav {low:.1} dB", topo.name());
                        let high = gain_db(topo, mode, cutoff, res, 0.0, 8_000.0);
                        assert!(high > -1.0, "{}: högpass 8 kHz gav {high:.1} dB", topo.name());
                    }
                    FilterMode::Notch => {
                        let hole = gain_db(topo, mode, cutoff, res, 0.0, cutoff);
                        let band = gain_db(topo, mode, cutoff, res, 0.0, 100.0);
                        assert!(hole < -12.0, "{}: notch vid cutoff {hole:.1} dB", topo.name());
                        assert!(band > -1.0, "{}: notch utanför bandet {band:.1} dB", topo.name());
                    }
                }
            }
        }
    }

    /// Resonansratten ska lyfta toppen, inte bara flytta den.
    #[test]
    fn the_resonance_knob_lifts_the_peak_at_cutoff() {
        let cutoff = 500.0;
        println!("\n=== resonanstopp vid {cutoff} Hz ===");
        for topo in FilterTopology::ALL {
            let flat = gain_db(topo, FilterMode::LowPass, cutoff, 0.0, 0.0, cutoff);
            let hot = gain_db(topo, FilterMode::LowPass, cutoff, 0.9, 0.0, cutoff);
            println!("{:<15} resonans 0,0: {flat:+6.1} dB   resonans 0,9: {hot:+6.1} dB", topo.name());
            assert!(hot > flat + 6.0, "{}: {flat:.1} -> {hot:.1} dB", topo.name());
        }
    }

    /// (b) Full resonans utan insignal ska självsvänga, dämpat men hörbart och ändligt.
    /// Insignalen är exakt noll — det enda som finns kvar är filterts eget -120 dB-brus.
    #[test]
    fn full_resonance_self_oscillates_with_no_input() {
        let n = (SR * 2.0) as usize;
        println!("\n=== självsvängning: 600 Hz, resonans 1,0, ingen insignal ===");
        for topo in FilterTopology::ALL {
            for mode in [FilterMode::LowPass, FilterMode::BandPass, FilterMode::HighPass] {
                let mut l = vec![0.0f32; n];
                let mut r = vec![0.0f32; n];
                let mut f = topo.make(SR);
                f.set_mode(mode);
                f.set(600.0, 1.0, 0.0);
                f.process_stereo(&mut l, &mut r);

                let tail = &l[n / 2..];
                let quarter = &l[n * 3 / 4..];
                let level = rms(tail);
                let peak = tail.iter().fold(0.0f32, |a, v| a.max(v.abs()));
                println!(
                    "{:<15} {:<9} rms {level:.4} ({:+.1} dBFS)  topp {peak:.3}  sista kvarten {:.4}",
                    topo.name(),
                    mode.name(),
                    20.0 * level.log10(),
                    rms(quarter)
                );
                assert!(tail.iter().all(|v| v.is_finite()), "{}: NaN", topo.name());
                assert!(peak < 1.5, "{}: topp {peak:.2} över skenorna", topo.name());
                assert!(level > 0.01, "{}: svänger inte, rms {level:.5}", topo.name());
                // Kvar efter 2 s = självsvängning, inte en transient som dött ut.
                assert!(rms(quarter) > 0.5 * level, "{}: dör ut", topo.name());
            }
        }
    }

    /// (c) Drivern sitter i loopen: samma 200 Hz-sinus ska få klart mer övertoner med drive 1.
    #[test]
    fn drive_inside_the_loop_adds_harmonics() {
        let f0 = 200.0;
        println!("\n=== THD, 200 Hz-sinus (0,25), cutoff 600 Hz, resonans 0,5 ===");
        for topo in [FilterTopology::SallenKeyMs, FilterTopology::LadderPd] {
            // Spelnivå: drivern ska mätas där den används, inte på mätnivån för kurvorna.
            let thd = |drive: f32| {
                let out = run(0.25, topo, FilterMode::LowPass, 600.0, 0.5, drive, f0);
                let fund = amplitude(&out, f0);
                let harm = (2..=8)
                    .map(|k| amplitude(&out, f0 * k as f32).powi(2))
                    .sum::<f32>()
                    .sqrt();
                harm / fund
            };
            let (clean, driven) = (thd(0.0), thd(1.0));
            println!(
                "{:<15} drive 0,0: {:.2} %   drive 1,0: {:.2} %   ({:.1} gånger)",
                topo.name(),
                clean * 100.0,
                driven * 100.0,
                driven / clean
            );
            assert!(driven > 0.02, "{}: THD med drive bara {:.2} %", topo.name(), driven * 100.0);
            assert!(driven > 3.0 * clean, "{}: {clean:.4} -> {driven:.4}", topo.name());
        }
    }

    /// Integrationsprov: den registrerade effekten räknar alla topologier och moden i stereo utan
    /// att gå utanför det ändliga, och stereokanalerna har egna tillstånd.
    #[test]
    fn the_registered_effect_runs_every_topology_in_stereo() {
        for topo_idx in 0..3 {
            for mode_idx in 0..4 {
                let mut p = crate::audio::builtin_fx::load("builtin:cytomic", SR).expect("cytomic");
                p.set_parameter(0, topo_idx as f64);
                p.set_parameter(1, mode_idx as f64);
                p.set_parameter(2, 800.0);
                p.set_parameter(3, 0.5);
                p.set_parameter(4, 1.0);
                let mut l = tone(0.25, 330.0, 4_096);
                let mut r = vec![0.0f32; 4_096];
                p.process_stereo(&mut l, &mut r);
                assert!(l.iter().chain(r.iter()).all(|v| v.is_finite()), "topologi {topo_idx} mod {mode_idx}");
                assert!(rms(&l[2_048..]) > 1.0e-4, "topologi {topo_idx} mod {mode_idx} tystnade");
                assert!(rms(&r) < 0.5 * rms(&l), "kanalerna delar tillstånd");
                p.reset();
            }
        }
    }
}
