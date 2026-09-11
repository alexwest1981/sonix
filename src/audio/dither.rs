//! Dither vid kvantisering till fast punkt (Fas 6.5).
//!
//! Varför: när mastern kvantiseras till 16 bitar hamnar kvantiseringsfelet på
//! ett *korrelerat* sätt — en svag ton får sina övertoner och ett svagt parti
//! får en hörbar "trappa". Dither gör felet okorrelerat med materialet, så det
//! låter som ett jämnt brusgolv i stället för som distorsion.
//!
//! Två oberoende rektangulärt fördelade slumptal summrade ger en **triangulär**
//! fördelning över ±1 kvantsteg (TPDF), vilket är det som krävs för att fel
//! *och* medelvärde ska bli oberoende av insignalen. Detta är samma metod som
//! `sox`, `ffmpeg` och masterningskedjor använder.
//!
//! Valfritt **noise shaping**: felet från föregående sample dras av innan
//! kvantiseringen, vilket flyttar bruset uppåt i frekvens — där örat hör det
//! sämre och där ett lågpassfilter i samplingsomvandlingen tar det.

use crate::rng::Rng;

/// Standardfrö: fast, så att en export blir reproducerbar. Samma projekt + samma
/// inställningar ger samma fil, vilket är värt mer än att varje export brusar
/// olika.
pub const DEFAULT_SEED: u64 = 0x5EED_1D17;

/// Kvantstegsstorlek för 16 bitar, i normaliserad skala.
const LSB_16: f32 = 1.0 / 32768.0;

/// Dither-generator för två kanaler (interleaved stereo), med eget
/// shaping-tillstånd per kanal — annars skulle felet från vänster kanal
/// påverka höger.
#[derive(Clone, Debug)]
pub struct TpdfDither {
    rng: Rng,
    shaping: bool,
    prev_err: [f32; 2],
}

impl TpdfDither {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            shaping: false,
            prev_err: [0.0; 2],
        }
    }

    /// Slår på eller av noise shaping (första ordningen).
    pub fn with_noise_shaping(mut self, on: bool) -> Self {
        self.shaping = on;
        self
    }

    /// Två rektangulära slumptal summrade → triangulär fördelning över ±1 LSB.
    fn next_noise(&mut self) -> f32 {
        (self.rng.next_f32() - self.rng.next_f32()) * LSB_16
    }

    /// Kvantiserar en normaliserad sample till 16 bitars fast punkt, med dither.
    ///
    /// `channel` 0 eller 1 (interleaved stereo) håller isär shaping-tillståndet.
    /// Returen är den kvantiserade nivån i normaliserad skala, så att felet går
    /// att mäta utan att räkna om från heltal.
    pub fn quantize_to_16(&mut self, sample: f32, channel: usize) -> f32 {
        let ch = channel.min(1);
        let x = sample.clamp(-1.0, 1.0);
        let noise = self.next_noise();
        // Felet från förra samplet dras av *före* kvantiseringen: då blir
        // utfelet d + (1 − z⁻¹)·e, dvs högpassformat i stället för vitt.
        let w = if self.shaping {
            x + noise - self.prev_err[ch]
        } else {
            x + noise
        };
        // Avrunda till närmaste kvantnivå (inte trunkera — trunkering är ensidig
        // och lägger till en extra felkomponent).
        let q = (w / LSB_16).round().clamp(-32768.0, 32767.0) * LSB_16;
        if self.shaping {
            self.prev_err[ch] = q - w;
        }
        q
    }

    /// Som [`Self::quantize_to_16`], men färdig som `i16` för WAV-filen.
    pub fn quantize_i16(&mut self, sample: f32, channel: usize) -> i16 {
        (self.quantize_to_16(sample, channel) / LSB_16).round() as i16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44_100.0;

    /// Kvantisering utan dither, som referens: trunkering mot ±1, det som
    /// koden gjorde före Fas 6.5.
    fn truncate_to_16(s: f32) -> f32 {
        ((s.clamp(-1.0, 1.0) * 32767.0) as i32 as f32) / 32768.0
    }

    /// Goertzel: energin vid en frekvens, utan att bygga en hel FFT.
    fn goertzel(buf: &[f32], freq: f32, sr: f32) -> f32 {
        let k = 2.0 * (2.0 * std::f32::consts::PI * freq / sr).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in buf {
            let s0 = x + k * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        ((s1 * s1 + s2 * s2 - k * s1 * s2).max(0.0)).sqrt() / buf.len() as f32
    }

    fn sine(freq: f32, amp: f32, n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| amp * (2.0 * std::f32::consts::PI * freq * i as f32 / SR).sin())
            .collect()
    }

    #[test]
    fn a_loud_signal_stays_within_one_step() {
        let mut d = TpdfDither::new(1);
        for &s in sine(1000.0, 0.5, 2000).iter() {
            let q = d.quantize_to_16(s, 0);
            assert!(
                (q - s).abs() <= 1.5 * LSB_16,
                "felet {} större än ett kvantsteg",
                q - s
            );
        }
    }

    #[test]
    fn dither_is_deterministic_for_a_seed() {
        let input = sine(440.0, 0.3, 500);
        let run = |seed: u64| {
            let mut d = TpdfDither::new(seed);
            input.iter().map(|&s| d.quantize_i16(s, 0)).collect::<Vec<i16>>()
        };
        assert_eq!(run(7), run(7), "samma frö ska ge samma fil");
        assert_ne!(run(7), run(8), "olika frö ska ge olika brus");
    }

    #[test]
    fn dither_is_unbiased_between_the_steps() {
        // En konstant som ligger exakt mitt emellan två kvantnivåer: utan dither
        // hamnar den alltid på samma nivå (systematiskt fel), med dither ska
        // medelvärdet hamna rätt.
        let x = 0.25 + 0.5 * LSB_16;
        let n = 20_000;

        // Summeringen sker i f64: 20 000 f32-termer kring 0.25 ger ett uppsamlat
        // avrundningsfel i samma storlek som en kvantnivå, vilket skulle se ut som
        // en bias i dithern (det var precis vad den här mätningen först visade).
        let mut d = TpdfDither::new(DEFAULT_SEED);
        let sum: f64 = (0..n).map(|_| d.quantize_to_16(x, 0) as f64).sum();
        let mean_dithered = sum / n as f64;
        let err = (mean_dithered - x as f64).abs();
        assert!(err < 0.05 * LSB_16 as f64, "medelfelet {err} ska vara litet");

        let sum_t: f64 = (0..n).map(|_| truncate_to_16(x) as f64).sum();
        let mean_truncated = sum_t / n as f64;
        let trunc_err = (mean_truncated - x as f64).abs();
        // Exakt en halv nivå: trunkering är ensidig, så felet är systematiskt
        // och inte slumpmässigt — det är det som hörs som distorsion.
        assert!(
            trunc_err >= 0.5 * LSB_16 as f64,
            "utan dither ska felet vara en halv nivå, var {trunc_err}"
        );
    }

    #[test]
    fn dither_decorrelates_the_error_from_the_signal() {
        // En svag ton (≈ −80 dBFS) är det värsta fallet: där är kvantiserings-
        // felet nästan helt korrelerat med signalen och hörs som distorsion.
        let amp = 10.0 * LSB_16;
        let input = sine(1000.0, amp, 20_000);
        let corr = |qs: &[f32]| {
            // f64: e·s är ~1e-5 stort, och 20 000 sådana termer tappar precision i f32.
            let mut num = 0.0f64;
            let mut de = 0.0f64;
            let mut ds = 0.0f64;
            for (q, s) in qs.iter().zip(input.iter()) {
                let e = (*q - *s) as f64;
                let s = *s as f64;
                num += e * s;
                de += e * e;
                ds += s * s;
            }
            (num / (de.sqrt() * ds.sqrt() + 1e-30)).abs()
        };

        let mut d = TpdfDither::new(DEFAULT_SEED);
        let with: Vec<f32> = input.iter().map(|&s| d.quantize_to_16(s, 0)).collect();
        let without: Vec<f32> = input.iter().map(|&s| truncate_to_16(s)).collect();

        let c_with = corr(&with);
        let c_without = corr(&without);
        assert!(c_with < 0.15, "dithrad korrelation {c_with} ska vara låg");
        assert!(
            c_without > 0.5,
            "utan dither ska felet hänga ihop med signalen, var {c_without}"
        );
        assert!(c_with * 3.0 < c_without, "dither ska förbättra tydligt");
    }

    #[test]
    fn silence_only_gets_a_whisper() {
        // Dither får aldrig lägga till något hörbart i tystnad: högst ett steg.
        let mut d = TpdfDither::new(DEFAULT_SEED);
        let mut peak = 0.0f32;
        let mut sum_sq = 0.0f64;
        let n = 10_000;
        for _ in 0..n {
            let q = d.quantize_to_16(0.0, 0);
            peak = peak.max(q.abs());
            sum_sq += (q * q) as f64;
        }
        assert!(peak <= LSB_16 * 1.0001, "topp {peak} ska vara inom ett steg");
        let rms = (sum_sq / n as f64).sqrt();
        assert!(rms < LSB_16 as f64, "rms {rms} ska ligga under ett steg");
    }

    #[test]
    fn noise_shaping_moves_the_error_upwards_in_frequency() {
        // Första ordningens shaping ger utfelet d + (1 − z⁻¹)·e, dvs tydligt
        // högpassformat. Mät det på två sätt: lag-1-autokorrelationen och
        // energin i ett lågt respektive högt band.
        let input = sine(1000.0, 0.01, 20_000);

        let mut plain = TpdfDither::new(DEFAULT_SEED);
        let mut shaped = TpdfDither::new(DEFAULT_SEED).with_noise_shaping(true);
        let e_plain: Vec<f32> = input.iter().map(|&s| plain.quantize_to_16(s, 0) - s).collect();
        let e_shaped: Vec<f32> = input.iter().map(|&s| shaped.quantize_to_16(s, 0) - s).collect();

        let autocorr1 = |e: &[f32]| {
            let mean = e.iter().map(|v| *v as f64).sum::<f64>() / e.len() as f64;
            let mut num = 0.0;
            let mut den = 0.0;
            for i in 0..e.len() - 1 {
                num += (e[i] as f64 - mean) * (e[i + 1] as f64 - mean);
            }
            for v in e {
                den += (*v as f64 - mean) * (*v as f64 - mean);
            }
            num / den
        };
        let ac_plain = autocorr1(&e_plain);
        let ac_shaped = autocorr1(&e_shaped);
        assert!(
            ac_plain.abs() < 0.1,
            "vanligt dither ska vara vitt, var {ac_plain}"
        );
        assert!(
            ac_shaped < -0.2,
            "shaping ska ge högpassformat (negativ lag-1), var {ac_shaped}"
        );

        let low = |e: &[f32]| (goertzel(e, 500.0, SR) + goertzel(e, 1500.0, SR)) / 2.0;
        let high = |e: &[f32]| (goertzel(e, 18_000.0, SR) + goertzel(e, 21_000.0, SR)) / 2.0;
        assert!(
            low(&e_shaped) < low(&e_plain),
            "shaping ska sänka bruset i låga frekvenser"
        );
        assert!(
            high(&e_shaped) > high(&e_plain),
            "shaping ska lyfta bruset i höga frekvenser"
        );
    }

    #[test]
    fn channels_have_separate_shaping_state() {
        // Samma sample på båda kanalerna ska ge samma utsignal — då har de egna,
        // oberoende tillstånd och bruset är okorrelerat mellan kanalerna.
        let mut d = TpdfDither::new(DEFAULT_SEED).with_noise_shaping(true);
        let mut differ = 0;
        for _ in 0..1000 {
            let l = d.quantize_to_16(0.3, 0);
            let r = d.quantize_to_16(0.3, 1);
            if (l - r).abs() > 1e-9 {
                differ += 1;
            }
        }
        assert!(differ > 400, "kanalerna ska brusa olika, olika i {differ} fall");
    }

    #[test]
    fn out_of_range_input_is_clamped_not_wrapped() {
        let mut d = TpdfDither::new(DEFAULT_SEED);
        for &s in &[2.0f32, -2.0, 1.5, -1.5] {
            let q = d.quantize_to_16(s, 0);
            assert!((-1.0..=1.0).contains(&q), "{s} gav {q}");
        }
        let i = d.quantize_i16(3.0, 0);
        assert!(i <= i16::MAX && i >= 0);
    }
}
