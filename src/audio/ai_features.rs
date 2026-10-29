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
}

impl AiFeatures {
    /// En rad per mått, för CLI:t.
    pub fn lines(&self) -> Vec<String> {
        vec![
            format!("spektralt tak        {:.0} Hz (topp över taket {:.1} dB)", self.spectral_ceiling_hz, self.above_ceiling_db),
            format!("brusgolv (p10)       {:.1} dBFS", self.noise_floor_dbfs),
            format!("korttids-RMS spridning {:.1} dB", self.loudness_spread_db),
            format!("crest (topp − RMS)   {:.1} dB", self.crest_db),
        ]
    }
}

const FFT_SIZE: usize = 2048;
/// Under den här nivån relativt spektrets topp räknas bandet som tomt.
const FLOOR_DB: f32 = -60.0;

/// Räknar måtten på en stereofil (vänster/höger, samplade i `sample_rate`).
pub fn analyse(left: &[f32], right: &[f32], sample_rate: u32) -> AiFeatures {
    let frames = left.len().min(right.len());
    if frames == 0 || sample_rate == 0 {
        return AiFeatures {
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

    AiFeatures {
        spectral_ceiling_hz: ceiling,
        above_ceiling_db: above_db,
        noise_floor_dbfs: p10,
        crest_db: peak_sample_db - overall_rms_db,
        loudness_spread_db: spread,
    }
}

/// Effekt (medelkvadrat) → dB relativt full skala. Ingen offset: en sinus på 0,5 i amplitud
/// mäter −9 dB, och digital tystnad −120 dB.
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
        let a = analyse(&low, &low, sr);
        assert!(a.spectral_ceiling_hz < 2000.0, "bara 1 kHz: taket ska ligga lågt, fick {}", a.spectral_ceiling_hz);

        let mut high = sine(1000.0, sr, sr as usize);
        let h = sine(15_000.0, sr, sr as usize);
        for (i, v) in high.iter_mut().enumerate() {
            *v = *v * 0.5 + h[i] * 0.5;
        }
        let b = analyse(&high, &high, sr);
        assert!(b.spectral_ceiling_hz > 14_000.0, "15 kHz ska synas i taket, fick {}", b.spectral_ceiling_hz);
        assert!(b.above_ceiling_db < a.above_ceiling_db, "energi ovanför taket ska vara relativt sett mindre");
    }

    /// **Tystnad mäts som tystnad.** Digital tystnad ger ett brusgolv långt under en spelad ton.
    #[test]
    fn silence_reads_as_a_low_floor() {
        let sr = 44_100;
        let mut signal = sine(440.0, sr, sr as usize);
        for v in signal.iter_mut().skip(sr as usize / 2) {
            *v = 0.0;
        }
        let f = analyse(&signal, &signal, sr);
        assert!(f.noise_floor_dbfs < -80.0, "tyst halva ska ge lågt p10, fick {}", f.noise_floor_dbfs);
        assert!(f.loudness_spread_db > 5.0, "en tyst halva ska synas i spridningen, fick {}", f.loudness_spread_db);
    }
}
