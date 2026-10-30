//! **Roar** — trebandsattnare med egen kurvuppsättning.
//!
//! Strukturen är modellerad efter den Ableton-effekt som binärkartan dokumenterar
//! (`docs/ABLETON-BINARY-MAP.md`: RTTI-klassen `ORoarProcessor`, tolv kurvnamn, åtta
//! filtertyper, tre serialled med egen kurva + filter + gain, samt en återkopplingsslinga
//! med grind). Det vi ärver är **formen**: delning i tre band, en kurva per band, ett
//! filter efter kurvan och en global återkoppling över alltihop.
//!
//! **Kurvorna är våra egna.** Kurvorna i Abletons binär går inte att läsa ut, och målet
//! här är inte att låta som originalet — det är att ge sex mätbart olika
//! övertonprofiler. Det som står i [`shape`] är vad som gäller, och proven i botten av
//! filen mäter dem i stället för att beskriva dem.

use super::dynamics::{highpass_coeffs, lowpass_coeffs};
use super::*;

/// Kurvorna i [`RoarBand::mode`]-ordning.
pub const ROAR_MODES: [&str; 6] = ["Tanh", "Hard Clip", "Fold", "Sine Fold", "Bit Crush", "Wrap"];

/// **En kurva.** `drive` förstärker in i kurvan, `mix` blandar torrt/vått inom bandet och
/// `tilt` är ett enpoligt lutningsfilter *efter* kurvan (0 = mörk, 0,5 = neutral, 1 = ljus).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoarBand {
    pub mode: u8,
    pub drive: f32,
    pub mix: f32,
    pub tilt: f32,
}

impl Default for RoarBand {
    fn default() -> Self {
        Self { mode: 0, drive: 2.0, mix: 0.0, tilt: 0.5 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoarParams {
    /// Nedre delningen i Hz (låg/mid).
    pub xover_low: f32,
    /// Övre delningen i Hz (mid/hög).
    pub xover_high: f32,
    pub bands: [RoarBand; 3],
    /// Hur mycket av utgången som matas tillbaka in i första ledet (0–0,95).
    pub feedback: f32,
    /// Grindens tröskel i dBFS: under den matas ingen återkoppling in.
    pub fb_gate_db: f32,
    /// Antal led i serie, 1–3.
    pub stages: u8,
}

impl Default for RoarParams {
    fn default() -> Self {
        Self {
            xover_low: 200.0,
            xover_high: 2_000.0,
            bands: [RoarBand::default(); 3],
            feedback: 0.0,
            fb_gate_db: -60.0,
            stages: 1,
        }
    }
}

/// Drive-kalibrering per kurva.
///
/// Rattens läge betyder inte samma överstyrning i alla kurvor, för kurvorna har olika
/// knän. Tabellen är satt så att THD:t stiger i den ordning kurvorna är listade —
/// `tanh` mildast, `hard clip` hårdast — vilket `thd_ordering_tanh_fold_clip` mäter.
/// Utan kalibreringen är **vikningen** hårdast av de tre: dess derivata vänder, medan
/// klippningens bara planar ut. Det är en medveten avvägning, inte en naturlag.
const ROAR_KNEE: [f32; 6] = [0.5, 2.5, 0.63, 1.0, 1.0, 1.0];

/// Våra sex kurvor. Alla är udda och begränsade till ungefär ±1, så ett band kan aldrig
/// lämna fullskala bara för att kurvan är vald.
#[inline(always)]
fn shape(mode: u8, x: f32) -> f32 {
    match mode % 6 {
        0 => x.tanh(),
        1 => x.clamp(-1.0, 1.0),
        2 => {
            // Triangelvikning: kurvan vänder vid ±1 och går tillbaka, med ett veck per
            // fullskala. Vecket är en derivatodiskontinuitet, alltså rikligt med övertoner.
            let f = (x + 1.0).rem_euclid(4.0);
            (if f <= 2.0 { f } else { 4.0 - f }) - 1.0
        }
        3 => (x * std::f32::consts::FRAC_PI_2).sin(),
        4 => {
            // Åtta nivåer: kvantiseringssteget är 1/4 fullskala, hörbart som brus + övertoner.
            const LEVELS: f32 = 8.0;
            (x.clamp(-1.0, 1.0) * LEVELS).round() / LEVELS
        }
        _ => (x + 1.0).rem_euclid(2.0) - 1.0,
    }
}

/// LR2-delningsfilter: ett andra ordningens Butterworth-steg per sida (Q = 0,5), alltså två
/// kaskadkopplade första ordningen.
///
/// Tecknen är det som gör delningen platt: `låg + hög − mid`, inte `låg + mid + hög`. Två
/// LR2-par summerar var för sig till ett allpass, men den nästlade trevägsformen gör det
/// bara med **mid** inverterat — annars möts banden i en nolla vid delningen i stället för
/// att mötas i en jämn övergång. `bands_sum_flat` mäter det över hela registret.
struct RoarXover {
    low_lp: BiquadCoeffs,
    low_hp: BiquadCoeffs,
    high_lp: BiquadCoeffs,
    high_hp: BiquadCoeffs,
}

impl RoarXover {
    fn new(sr: f32, f1: f32, f2: f32) -> Self {
        let q = 0.5;
        Self {
            low_lp: lowpass_coeffs(sr, f1, q),
            low_hp: highpass_coeffs(sr, f1, q),
            high_lp: lowpass_coeffs(sr, f2, q),
            high_hp: highpass_coeffs(sr, f2, q),
        }
    }
}

/// Ett led: delningsfilter, kurvsteg och lutningsfilter per band och kanal.
#[derive(Clone, Copy, Default)]
struct RoarStage {
    /// Per kanal: [lågpass vid låg delning, högpass vid låg, lågpass vid hög, högpass vid hög].
    split: [[BiquadState; 4]; 2],
    /// Lutningsfiltrets tillstånd per kanal och band.
    tilt: [[f32; 3]; 2],
}

impl RoarStage {
    #[inline(always)]
    fn channel(&mut self, ch: usize, x: f32, p: &RoarParams, c: &RoarXover, a: f32) -> f32 {
        let low = self.split[ch][0].process(x, &c.low_lp);
        let above = self.split[ch][1].process(x, &c.low_hp);
        let mid = -self.split[ch][2].process(above, &c.high_lp);
        let high = self.split[ch][3].process(above, &c.high_hp);

        let mut sum = 0.0;
        for (band, dry) in [low, mid, high].into_iter().enumerate() {
            let bp = p.bands[band];
            let drive = bp.drive.clamp(1.0, 32.0) * ROAR_KNEE[(bp.mode % 6) as usize];
            let wet = shape(bp.mode, dry * drive);
            // Lutningsfiltret ligger bara på den våta vägen: `mix = 0` är därför exakt
            // bandgenomgång, vilket är det som gör att delningen går att mäta ren.
            let lp = &mut self.tilt[ch][band];
            *lp += a * (wet - *lp);
            let g = db_to_gain((bp.tilt.clamp(0.0, 1.0) - 0.5) * 24.0);
            let tilted = *lp / g + (wet - *lp) * g;
            let mix = bp.mix.clamp(0.0, 1.0);
            sum += dry + mix * (tilted - dry);
        }
        sum
    }

    #[inline(always)]
    fn process(&mut self, l: f32, r: f32, p: &RoarParams, c: &RoarXover, a: f32) -> (f32, f32) {
        (self.channel(0, l, p, c, a), self.channel(1, r, p, c, a))
    }
}

#[inline(always)]
fn db_to_gain(db: f32) -> f32 {
    10.0_f32.powf(db / 20.0)
}

pub struct Roar {
    sample_rate: f32,
    coeffs: RoarXover,
    xover: (f32, f32),
    stages: [RoarStage; 3],
    /// Utgången, ett sampel fördröjd: återkopplingen får inte vara en algebraisk slinga.
    fb_l: f32,
    fb_r: f32,
    /// Grindens öppning 0–1, glättad så att den inte klipper ljudet när den slår till.
    gate: f32,
    gate_coeff: f32,
    tilt_coeff: f32,
}

impl Roar {
    pub fn new(sample_rate: f32) -> Self {
        let sr = sample_rate.max(1.0);
        let (lo, hi) = (200.0, 2_000.0);
        Self {
            sample_rate: sr,
            coeffs: RoarXover::new(sr, lo, hi),
            xover: (lo, hi),
            stages: [RoarStage::default(); 3],
            fb_l: 0.0,
            fb_r: 0.0,
            gate: 0.0,
            // 2 ms: snabb nog att stänga innan svansen hinner snurra, långsam nog att
            // inte klicka.
            gate_coeff: 1.0 - (-1.0 / (0.002 * sr)).exp(),
            // 1 kHz lutningsaxel, enpolig.
            tilt_coeff: 1.0 - (-2.0 * PI * 1_000.0 / sr).exp(),
        }
    }

    #[inline(always)]
    pub fn process(&mut self, l: f32, r: f32, p: &RoarParams) -> (f32, f32) {
        let (lo, hi) = (
            p.xover_low.clamp(40.0, 2_000.0),
            p.xover_high.clamp(500.0, 12_000.0),
        );
        if (lo, hi) != self.xover {
            self.coeffs = RoarXover::new(self.sample_rate, lo, hi);
            self.xover = (lo, hi);
        }

        // Grinden tittar på **ingången**: tyst in ger ingen återkoppling, så en slinga med
        // hög mängd kan inte fortsätta snurra på sin egen svans.
        let open = l.abs().max(r.abs()) > db_to_gain(p.fb_gate_db.clamp(-90.0, 0.0));
        self.gate += ((if open { 1.0 } else { 0.0 }) - self.gate) * self.gate_coeff;
        let fb = p.feedback.clamp(0.0, 0.95) * self.gate;
        let (mut out_l, mut out_r) = (l + self.fb_l * fb, r + self.fb_r * fb);

        let stages = p.stages.clamp(1, 3) as usize;
        for i in 0..stages {
            let (nl, nr) = self.stages[i].process(out_l, out_r, p, &self.coeffs, self.tilt_coeff);
            out_l = nl;
            out_r = nr;
        }

        // Taket ligger före masterns limiter: hårt ställd kan Roar aldrig lämna fullskala,
        // och återkopplingen kan därför inte växa sig förbi 1,0 heller.
        out_l = out_l.clamp(-1.0, 1.0);
        out_r = out_r.clamp(-1.0, 1.0);
        self.fb_l = out_l;
        self.fb_r = out_r;
        (out_l, out_r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 48_000.0;

    /// Amplitud för en bin-exakt frekvens (`freq * len / SR` måste vara ett heltal).
    fn amplitude(sig: &[f32], freq: f32) -> f32 {
        let (mut re, mut im) = (0.0f32, 0.0f32);
        for (i, &x) in sig.iter().enumerate() {
            let ph = std::f32::consts::TAU * freq * i as f32 / SR;
            re += x * ph.cos();
            im += x * ph.sin();
        }
        2.0 * (re * re + im * im).sqrt() / sig.len() as f32
    }

    fn sine(freq: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (std::f32::consts::TAU * freq * i as f32 / SR).sin())
            .collect()
    }

    fn run(roar: &mut Roar, input: &[f32], p: &RoarParams) -> Vec<f32> {
        input.iter().map(|&x| roar.process(x, x, p).0).collect()
    }

    fn identity() -> RoarParams {
        RoarParams {
            bands: [RoarBand { mode: 0, drive: 1.0, mix: 0.0, tilt: 0.5 }; 3],
            feedback: 0.0,
            ..RoarParams::default()
        }
    }

    /// (a) Delningen och omsummeringen är amplitudplatt över registret.
    #[test]
    fn bands_sum_flat() {
        let mut worst = 0.0f32;
        let mut worst_hz = 0.0;
        for hz in [20.0, 30.0, 50.0, 500.0, 5_000.0, 12_000.0, 16_000.0] {
            let n = 48_000; // hela sekunder, så varje testfrekvens hamnar på en exakt bin
            let input = sine(hz, 0.5, n + 4_800);
            let mut roar = Roar::new(SR);
            let out = run(&mut roar, &input, &identity());
            // Hoppa över inspelningen och det allpassa fasdrejet från LR2.
            let tail = &out[4_800..];
            let level_in = amplitude(&input[4_800..], hz);
            let level_out = amplitude(tail, hz);
            let dev = 20.0 * (level_out / level_in).log10();
            println!("band-sum {hz:>6.0} Hz: {dev:+.4} dB");
            if dev.abs() > worst {
                worst = dev.abs();
                worst_hz = hz;
            }
        }
        println!("band-sum worst: {worst:.4} dB at {worst_hz:.0} Hz");
        assert!(worst < 0.5, "delningen är inte platt: {worst:.3} dB vid {worst_hz:.0} Hz");
    }

    /// (b) THD per kurva: 200 Hz sinus, fast drive, övertonerna vid 400/600/800 Hz.
    ///
    /// Delningarna flyttas undan (60 Hz / 6 kHz) så att både grundtonen och de tre första
    /// övertonerna ligger i **samma** band — annars mäts delningens egen färgning också.
    fn thd(mode: u8, drive: f32) -> f32 {
        let n = 43_200; // 0,9 s: 200 Hz hamnar på bin 180, övertonerna på 360/540/720
        let skip = 4_800;
        let input = sine(200.0, 0.5, n + skip);
        let p = RoarParams {
            bands: [RoarBand { mode, drive, mix: 1.0, tilt: 0.5 }; 3],
            xover_low: 60.0,
            xover_high: 6_000.0,
            ..RoarParams::default()
        };
        let mut roar = Roar::new(SR);
        let out = run(&mut roar, &input, &p);
        let tail = &out[skip..];
        let fund = amplitude(tail, 200.0);
        let harmonics: f32 = [400.0, 600.0, 800.0]
            .iter()
            .map(|&f| amplitude(tail, f).powi(2))
            .sum::<f32>()
            .sqrt();
        let r = harmonics / fund;
        println!(
            "{}: fundament {fund:.4}, THD {:.3} % ({:.1} dB)",
            ROAR_MODES[mode as usize],
            r * 100.0,
            20.0 * r.log10()
        );
        r
    }

    #[test]
    fn thd_ordering_tanh_fold_clip() {
        let drive = 4.0;
        let tanh = thd(0, drive);
        let clip = thd(1, drive);
        let fold = thd(2, drive);
        for mode in 3..6 {
            thd(mode, drive);
        }
        assert!(
            tanh < fold && fold < clip,
            "ordningen ska vara tanh < fold < hard clip: {tanh:.4} < {fold:.4} < {clip:.4}"
        );
    }

    /// (c) Hög återkoppling med tyst ingång: svansen dör, den snurrar inte vidare.
    #[test]
    fn feedback_gate_stops_the_tail() {
        let loud = sine(200.0, 0.9, 24_000);
        let silent = vec![0.0f32; 48_000];
        let p = RoarParams {
            bands: [RoarBand { mode: 2, drive: 6.0, mix: 1.0, tilt: 0.5 }; 3],
            feedback: 0.95,
            fb_gate_db: -60.0,
            stages: 2,
            ..RoarParams::default()
        };
        let mut roar = Roar::new(SR);
        let _ = run(&mut roar, &loud, &p);
        let out = run(&mut roar, &silent, &p);
        // Sista 100 ms: efter grindens 2 ms och återkopplingens eget fall.
        let tail = &out[out.len() - 4_800..];
        let peak = tail.iter().fold(0.0f32, |m, v| m.max(v.abs()));
        let db = 20.0 * peak.max(1e-12).log10();
        println!("feedback tail peak in silence: {db:.1} dBFS");
        assert!(db < -60.0, "återkopplingen fortsätter: {db:.1} dBFS");
    }

    /// (d) Taket: extrema inställningar kan aldrig lämna fullskala.
    #[test]
    fn ceiling_holds_at_extremes() {
        let mut roar = Roar::new(SR);
        let p = RoarParams {
            bands: [
                RoarBand { mode: 2, drive: 32.0, mix: 1.0, tilt: 1.0 },
                RoarBand { mode: 5, drive: 32.0, mix: 1.0, tilt: 0.0 },
                RoarBand { mode: 1, drive: 32.0, mix: 1.0, tilt: 1.0 },
            ],
            feedback: 0.95,
            fb_gate_db: -90.0,
            stages: 3,
            ..RoarParams::default()
        };
        let mut peak = 0.0f32;
        let mut seed = 0x1234_5678u32;
        for i in 0..48_000 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let noise = (seed >> 8) as f32 / 8_388_608.0 - 1.0;
            let t = i as f32 / SR;
            let x = (noise * 7.0 + (std::f32::consts::TAU * 55.0 * t).sin() * 8.0).clamp(-8.0, 8.0);
            let (l, r) = roar.process(x, -x, &p);
            peak = peak.max(l.abs()).max(r.abs());
        }
        println!("ceiling: peak {peak:.6}");
        assert!(peak <= 1.0, "utgången gick över fullskala: {peak:.6}");
    }
}
