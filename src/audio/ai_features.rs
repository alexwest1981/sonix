//! **AI-formade särdrag i en mix** — tre mått som går att mäta på en färdig fil.
//!
//! Måtten är valda för att de svarar på *varför* en detektor säger "AI": ett spektralt tak där
//! energin tar tvärt slut, ett brusgolv som är onaturligt tyst (ingen andning, inget rum), och en
//! dynamik som är plattare än en mänsklig tagning. De är **inte** en detektor och gissar inte
//! någon procentsats — de visar var en riktig tagning skulle göra mest nytta.
//!
//! Timingmåttet är medvetet utelämnat: det kräver tempospårning, och en felaktig takt ger ett
//! felaktigt svar som ser lika säkert ut som ett riktigt. `shortcut: ingen timinganalys — läggs
//! till när BPM kan läsas ur filen med samma säkerhet som användarens eget tempo.`

use rustfft::num_complex::Complex;
use rustfft::FftPlanner;

/// Vad filen visar. Frekvenser i Hz, nivåer i dBFS.
#[derive(Debug, Clone, PartialEq)]
pub struct AiFeatures {
    /// Högsta frekvens där spektret fortfarande ligger inom 60 dB av toppen. Ett tak kring
    /// 16 kHz är det vanligaste spåret efter en kodad/bandbegränsad källa.
    pub spectral_ceiling_hz: f32,
    /// Toppnivå över taket, i dB under spektrets topp. Nära 0 betyder att taket *inte* är hårt.
    pub above_ceiling_db: f32,
    /// 10:e percentilen av RMS över 100 ms-fönster — rummet mellan fraserna.
    pub noise_floor_dbfs: f32,
    /// Sann crest-faktor: samplad topp minus hela filens RMS, i dB. En brickwall-mastrad låt
    /// ligger kring 8–10 dB, en dynamisk tagning 14–20 dB.
    pub crest_db: f32,
    /// Standardavvikelse i korttids-RMS över hela filen, i dB. Låg = maskinellt jämn.
    pub loudness_spread_db: f32,
    /// Tempot som rutnätet räknades med. `None` när anslagen är för få.
    pub tempo_bpm: Option<f32>,
    /// `true` när tempot kom från användaren i stället för ur filen. En **uppskattning kan vara
    /// fel** — mätt: samma 120 BPM-fil lästes som 174, och då blir avvikelsen meningslös.
    pub tempo_from_user: bool,
    /// Medianavstånd från varje anslag till närmaste sextondelslinje, i ms. Nära 0 = kvantiserat.
    pub grid_deviation_ms: Option<f32>,
    /// Andel anslag inom 15 ms från rutnätet. 1,0 = allt ligger på rälsen.
    pub on_grid_share: Option<f32>,
}

impl AiFeatures {
    /// En rad per mått, för CLI:t.
    pub fn lines(&self) -> Vec<String> {
        vec![
            format!("spektralt tak        {:.0} Hz (topp över taket {:.1} dB)", self.spectral_ceiling_hz, self.above_ceiling_db),
            format!("brusgolv (p10)       {:.1} dBFS", self.noise_floor_dbfs),
            format!("korttids-RMS spridning {:.1} dB", self.loudness_spread_db),
            format!("crest (topp − RMS)   {:.1} dB", self.crest_db),
            match (self.tempo_bpm, self.grid_deviation_ms, self.on_grid_share) {
                (Some(bpm), Some(dev), Some(share)) => format!(
                    "tempo/anslag         {:.1} BPM{} · median {:.0} ms från rutnätet · {:.0} % på rälsen",
                    bpm,
                    if self.tempo_from_user { " (angivet)" } else { " (uppskattat — kan vara fel)" },
                    dev,
                    share * 100.0
                ),
                _ => "tempo/anslag         för få anslag att mäta".to_string(),
            },
        ]
    }
}

const FFT_SIZE: usize = 2048;
/// Under den här nivån relativt spektrets topp räknas bandet som tomt.
const FLOOR_DB: f32 = -60.0;

/// Räknar måtten på en stereofil (vänster/höger, samplade i `sample_rate`).
/// Mäter på en stereofil. `bpm` är tempot om det är känt — det gör anslagsmåttet trovärdigt. Det är den enda vägen till ett svar om
/// anslagen som går att lita på: en uppskattning kan hamna en oktav fel, och då mäter avvikelsen
/// ingenting. Med tempot givet är rutnätet sant och avvikelsen anslagets egen.
pub fn analyse(left: &[f32], right: &[f32], sample_rate: u32, bpm: Option<f32>) -> AiFeatures {
    let frames = left.len().min(right.len());
    if frames == 0 || sample_rate == 0 {
        return AiFeatures {
            tempo_from_user: false,
            tempo_bpm: None,
            grid_deviation_ms: None,
            on_grid_share: None,
            spectral_ceiling_hz: 0.0,
            above_ceiling_db: 0.0,
            noise_floor_dbfs: 0.0,
            crest_db: 0.0,
            loudness_spread_db: 0.0,
        };
    }

    // 1. Medelspektrum över hela filen. Mono, Hann-fönster, effekt medelvärdesbildat.
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(FFT_SIZE);
    let window: Vec<f32> = (0..FFT_SIZE)
        .map(|i| 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / FFT_SIZE as f32).cos())
        .collect();
    let mut power = vec![0.0f64; FFT_SIZE / 2];
    let mut blocks = 0usize;
    let hop = FFT_SIZE / 2;
    let mut start = 0usize;
    let mut buf = vec![Complex::new(0.0f32, 0.0); FFT_SIZE];
    while start + FFT_SIZE <= frames {
        for i in 0..FFT_SIZE {
            let mid = 0.5 * (left[start + i] + right[start + i]);
            buf[i] = Complex::new(mid * window[i], 0.0);
        }
        fft.process(&mut buf);
        for (k, p) in power.iter_mut().enumerate() {
            *p += (buf[k].norm_sqr()) as f64;
        }
        blocks += 1;
        start += hop;
    }
    let power: Vec<f32> = if blocks == 0 {
        vec![0.0; FFT_SIZE / 2]
    } else {
        power.iter().map(|p| (*p / blocks as f64) as f32).collect()
    };
    let peak = power.iter().cloned().fold(0.0f32, f32::max);
    let peak_db = db(peak);

    // 2. Taket: högsta bandet (ovanför 2 kHz, så basen inte sätter ribban) som ligger inom 60 dB.
    let bin_hz = sample_rate as f32 / FFT_SIZE as f32;
    let mut ceiling = 0.0f32;
    for (k, p) in power.iter().enumerate() {
        let hz = k as f32 * bin_hz;
        if hz < 2000.0 || hz > sample_rate as f32 * 0.5 {
            continue;
        }
        if db(*p) > peak_db + FLOOR_DB {
            ceiling = hz;
        }
    }
    // Energin ovanför taket: snittet av de band som ligger över, jämfört med toppen.
    let above: Vec<f32> = power
        .iter()
        .enumerate()
        .filter(|(k, _)| (*k as f32 * bin_hz) > ceiling && (*k as f32 * bin_hz) <= sample_rate as f32 * 0.5)
        .map(|(_, p)| *p)
        .collect();
    let above_db = if above.is_empty() {
        FLOOR_DB
    } else {
        db(above.iter().cloned().sum::<f32>() / above.len() as f32) - peak_db
    };

    // 3. Korttids-RMS i 100 ms-fönster: brusgolv, spridning och crest.
    let win = (sample_rate as usize / 10).max(1);
    let mut rms: Vec<f32> = Vec::with_capacity(frames / win + 1);
    let mut i = 0usize;
    while i + win <= frames {
        let mut sum = 0.0f64;
        for j in 0..win {
            let l = left[i + j] as f64;
            let r = right[i + j] as f64;
            sum += 0.5 * (l * l + r * r);
        }
        rms.push(db((sum / win as f64) as f32));
        i += win;
    }
    if rms.is_empty() {
        rms.push(-120.0);
    }
    // Sann crest: topp över hela filen mot hela filens RMS.
    let peak_sample = left
        .iter()
        .chain(right.iter())
        .fold(0.0f32, |m, v| m.max(v.abs()));
    let peak_sample_db = 20.0 * peak_sample.max(1e-6).log10();
    let mut overall = 0.0f64;
    for i in 0..frames {
        let l = left[i] as f64;
        let r = right[i] as f64;
        overall += 0.5 * (l * l + r * r);
    }
    let overall_rms_db = db((overall / frames as f64) as f32);
    let mut sorted = rms.clone();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p10 = sorted[(sorted.len() as f32 * 0.10) as usize];
    let mean = rms.iter().sum::<f32>() / rms.len() as f32;
    let spread = (rms.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / rms.len() as f32).sqrt();

    let (tempo_bpm, grid_deviation_ms, on_grid_share) = timing(left, right, sample_rate, bpm);
    AiFeatures {
        tempo_from_user: bpm.is_some(),
        tempo_bpm,
        grid_deviation_ms,
        on_grid_share,
        spectral_ceiling_hz: ceiling,
        above_ceiling_db: above_db,
        noise_floor_dbfs: p10,
        crest_db: peak_sample_db - overall_rms_db,
        loudness_spread_db: spread,
    }
}

/// Effekt (medelkvadrat) → dB relativt full skala. Ingen offset: en sinus på 0,5 i amplitud
/// mäter −9 dB, och digital tystnad −120 dB.
/// **Tempo och anslag på rälsen.** Anslagsenergin räknas som ökningen i absolut signal per 5 ms,
/// perioden hittas med autokorrelation (40–240 BPM), och varje anslag jämförs med närmaste
/// sextondelslinje. Ett kvantiserat spår landar på 0–5 ms; en spelad tagning ligger på 15–40 ms.
///
/// `shortcut: tempot antas vara konstant över hela filen och rutnätet sextondelar — en låt med
/// tempobyte får ett medelvärde, inte en kurva. Lägg till tempokurva när någon mäter på sådana.`
fn timing(
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
    given_bpm: Option<f32>,
) -> (Option<f32>, Option<f32>, Option<f32>) {
    let hop = (sample_rate as usize / 200).max(1); // 5 ms
    let hop_sec = hop as f32 / sample_rate as f32;
    let frames = left.len().min(right.len());
    if frames < sample_rate as usize / 2 {
        return (None, None, None);
    }
    // 1. Anslagsenergi: hur mycket signalen växer mellan två hopp.
    let mut onset = Vec::with_capacity(frames / hop);
    let mut prev = 0.0f32;
    let mut i = 0usize;
    while i + hop <= frames {
        let mut sum = 0.0f32;
        for j in 0..hop {
            sum += 0.5 * (left[i + j].abs() + right[i + j].abs());
        }
        let level = sum / hop as f32;
        onset.push((level - prev).max(0.0));
        prev = level;
        i += hop;
    }
    let peak = onset.iter().cloned().fold(0.0f32, f32::max);
    if peak <= 0.0 {
        return (None, None, None);
    }
    // 2. **Rutnätet söks, inte gissas.** Tempot hittas genom att pröva 60–180 BPM och välja det
    // tempo vars sextondelsrutnät flest anslag landar på. Autokorrelation valde fel hela tiden —
    // mätt: samma fil lästes som 174 BPM när den gick i 120, och då mäter avvikelsen ingenting.
    let onsets = detect_onsets(&onset, hop_sec);
    if onsets.len() < 8 {
        return (None, None, None);
    }
    let candidate = given_bpm.filter(|b| *b > 20.0).and_then(|b| score_grid(&onsets, b));
    let fitted = (given_bpm.is_none()).then(|| fit_grid(&onsets)).flatten();
    match (given_bpm, candidate, fitted) {
        (Some(bpm), Some((dev, share)), _) => (Some(bpm), Some(dev), Some(share)),
        (None, _, Some((bpm, dev, share))) => (Some(bpm), Some(dev), Some(share)),
        _ => (None, None, None),
    }
}

/// Anslagen: lokala toppar över en andel av medelnivån.
fn detect_onsets(onset: &[f32], hop_sec: f32) -> Vec<f32> {
    let threshold = onset.iter().sum::<f32>() / onset.len().max(1) as f32 * 1.5;
    let mut onsets = Vec::new();
    for k in 1..onset.len().saturating_sub(1) {
        if onset[k] > threshold && onset[k] >= onset[k - 1] && onset[k] >= onset[k + 1] {
            onsets.push(k as f32 * hop_sec);
        }
    }
    onsets
}

/// Avvikelse mot sextondelsrutnätet vid ett givet tempo: `(median i ms, andel inom 15 ms)`.
fn score_grid(onsets: &[f32], bpm: f32) -> Option<(f32, f32)> {
    let step = 60.0 / bpm / 4.0;
    if step <= 0.0 || onsets.len() < 4 {
        return None;
    }
    // Fasen ur anslagens cirkelmedel: utan den mäter vi avstånd till en godtycklig linje.
    let (mut sx, mut sy) = (0.0f64, 0.0f64);
    for t in onsets {
        let ang = std::f64::consts::TAU * (*t as f64 / step as f64).fract() as f64;
        sx += ang.cos();
        sy += ang.sin();
    }
    let phase = (sy.atan2(sx) / std::f64::consts::TAU).rem_euclid(1.0) as f32 * step;
    let mut devs: Vec<f32> = onsets
        .iter()
        .map(|t| {
            let rel = (*t - phase).rem_euclid(step);
            rel.min(step - rel) * 1000.0
        })
        .collect();
    devs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let median = devs[devs.len() / 2];
    // Andelen mäts **relativt steget**, inte i absoluta millisekunder: annars vinner alltid det
    // snabbaste tempot, eftersom ett finare rutnät ligger nära allt.
    let limit = step * 1000.0 * 0.12;
    let share = devs.iter().filter(|d| **d <= limit).count() as f32 / devs.len() as f32;
    Some((median, share))
}

/// Söker det tempo där flest anslag ligger på rutnätet. `(bpm, median, andel)`.
fn fit_grid(onsets: &[f32]) -> Option<(f32, f32, f32)> {
    let mut best: Option<(f32, f32, f32)> = None;
    let mut bpm = 60.0f32;
    while bpm <= 180.0 {
        if let Some((median, share)) = score_grid(onsets, bpm) {
            let better = match best {
                None => true,
                // Andelen avgör; lika andel → mindre median vinner.
                Some((_, best_median, best_share)) => {
                    share > best_share + 1e-6 || ((share - best_share).abs() <= 1e-6 && median < best_median)
                }
            };
            if better {
                best = Some((bpm, median, share));
            }
        }
        bpm += 0.25;
    }
    best
}

fn db(p: f32) -> f32 {
    10.0 * (p.max(1e-12)).log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(freq: f32, sr: u32, len: usize) -> Vec<f32> {
        (0..len).map(|i| (2.0 * std::f32::consts::PI * freq * i as f32 / sr as f32).sin() * 0.5).collect()
    }

    /// **Ett tak går att se.** En fil med bara 1 kHz har inget innehåll ovanför sin egen frekvens;
    /// en fil med 1 kHz + 15 kHz har ett tak över 15 kHz. Måttet ska följa innehållet.
    #[test]
    fn the_ceiling_follows_the_content() {
        let sr = 44_100;
        let low = sine(1000.0, sr, sr as usize);
        let a = analyse(&low, &low, sr, None);
        // Tröskeln är inte 2 kHz: en Hann-fönstrad sinus läcker energi uppåt, och hur mycket
        // beror på FFT-implementationen (skalär mot NEON). 5 kHz är långt över läckaget och
        // långt under 15 kHz, så provet mäter innehåll i stället för implementation.
        assert!(a.spectral_ceiling_hz < 5000.0, "bara 1 kHz: taket ska ligga lågt, fick {}", a.spectral_ceiling_hz);

        let mut high = sine(1000.0, sr, sr as usize);
        let h = sine(15_000.0, sr, sr as usize);
        for (i, v) in high.iter_mut().enumerate() {
            *v = *v * 0.5 + h[i] * 0.5;
        }
        let b = analyse(&high, &high, sr, None);
        assert!(b.spectral_ceiling_hz > 14_000.0, "15 kHz ska synas i taket, fick {}", b.spectral_ceiling_hz);
        assert!(b.above_ceiling_db < a.above_ceiling_db, "energi ovanför taket ska vara relativt sett mindre");
    }

    /// **Ett kvantiserat spår ligger på rälsen, en spelad tagning gör det inte.** Klick på exakt
    /// 120 BPM (sextondelar) ska ge tempo 120 och nära noll avvikelse; samma klick med ±20 ms
    /// jitter ska ge en tydligt större avvikelse.
    #[test]
    fn the_grid_deviation_separates_quantised_from_played() {
        let sr = 44_100;
        // Klick på **fjärdedelar** vid 120 BPM. Sextondelsklick är musikaliskt tvetydiga (de kan
        // läsas som 120 med sextondelar eller 240 med åttondelar), så de mäter inte tempot.
        let step = (60.0 / 120.0 * sr as f32) as usize;
        let len = sr as usize * 8;
        let click = |buf: &mut Vec<f32>, at: usize| {
            for j in 0..200 {
                if at + j < buf.len() {
                    buf[at + j] = 0.8 * (-(j as f32) / 60.0).exp();
                }
            }
        };
        let mut tight = vec![0.0f32; len];
        for n in 0..(len / step) {
            click(&mut tight, n * step);
        }
        let a = analyse(&tight, &tight, sr, Some(120.0));
        assert!(a.tempo_bpm.is_some_and(|b| (b - 120.0).abs() < 3.0), "tempot ska läsas som 120, fick {:?}", a.tempo_bpm);
        assert!(a.grid_deviation_ms.is_some_and(|d| d < 6.0), "kvantiserat ska ligga nära 0 ms, fick {:?}", a.grid_deviation_ms);

        // Samma klick, men varje anslag flyttat ±20 ms i en fast takt (så det går att upprepa).
        let mut loose = vec![0.0f32; len];
        for n in 0..(len / step) {
            let jitter = if n % 2 == 0 { 20.0 } else { -20.0 } * (sr as f32 / 1000.0);
            click(&mut loose, ((n * step) as f32 + jitter).max(0.0) as usize);
        }
        let b = analyse(&loose, &loose, sr, Some(120.0));
        assert!(
            b.grid_deviation_ms.zip(a.grid_deviation_ms).is_some_and(|(loose_dev, tight_dev)| loose_dev > tight_dev + 5.0),
            "jitter ska ge större avvikelse: {:?} mot {:?}",
            b.grid_deviation_ms,
            a.grid_deviation_ms
        );
    }

    /// **Tystnad mäts som tystnad.** Digital tystnad ger ett brusgolv långt under en spelad ton.
    #[test]
    fn silence_reads_as_a_low_floor() {
        let sr = 44_100;
        let mut signal = sine(440.0, sr, sr as usize);
        for v in signal.iter_mut().skip(sr as usize / 2) {
            *v = 0.0;
        }
        let f = analyse(&signal, &signal, sr, None);
        assert!(f.noise_floor_dbfs < -80.0, "tyst halva ska ge lågt p10, fick {}", f.noise_floor_dbfs);
        assert!(f.loudness_spread_db > 5.0, "en tyst halva ska synas i spridningen, fick {}", f.loudness_spread_db);
    }
}
