//! **Omsampling med fönstrat sinc**, i stället för linjär interpolation.
//!
//! Linjär interpolation är ett 2-punkters lågpass: den dämpar diskanten olika mycket beroende på
//! var i intervallet sampeln hamnar, och den läcker in vikningsdistorsion när man går nedåt i
//! samplingsfrekvens. Det hörs som en grumlig diskant, och för stämsepareringen spelar det roll
//! eftersom modellerna arbetar i 48 kHz medan filen ofta är 44,1 kHz — **två** omsamplingar per
//! separation, in och ut.
//!
//! Implementationen är en rak fönstrad sinc (Blackman, 32 taggar ≈ 16 per sida). Ingen
//! polyfas-tabell: det här körs en gång per fil, inte i realtid, och 32 taggar per sampel är
//! billigare än att underhålla en tabell som måste stämmas av mot varje bråkdel.

/// Antal taggar i sinc-kärnan. 32 ger ~90 dB stoppbandsdämpning vid 2× omsampling.
const TAPS: i32 = 32;

/// Svansen: hur många sampel på varje sida kärnan når.
const HALF: i32 = TAPS / 2;

/// **Omsampla stereo** från `from` till `to`. Returnerar `(vänster, höger)`.
pub fn resample_sinc(left: &[f32], right: &[f32], from: u32, to: u32) -> (Vec<f32>, Vec<f32>) {
    let n = left.len().min(right.len());
    if n == 0 || from == 0 || to == 0 || from == to {
        return (left[..n].to_vec(), right[..n].to_vec());
    }
    let ratio = to as f64 / from as f64;
    // Vid nedåt omsampling måste kärnan skalas med förhållandet, annars viks spektrum in.
    let scale = if ratio < 1.0 { ratio } else { 1.0 };
    let cutoff = 0.5 * scale; // i cykler per insignalsampel
    let out_len = ((n as f64) * ratio).round() as usize;
    let mut out_l = Vec::with_capacity(out_len);
    let mut out_r = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src = i as f64 / ratio;
        let base = src.floor() as i64;
        let (mut acc_l, mut acc_r, mut weight) = (0.0f64, 0.0f64, 0.0f64);
        for k in -HALF..=HALF {
            let idx = base + k as i64;
            if idx < 0 || idx >= n as i64 {
                continue;
            }
            let t = src - idx as f64; // avstånd i insignalsampel
            // Sinc med avskärning vid `cutoff`.
            let arg = 2.0 * std::f64::consts::PI * cutoff * t;
            let sinc = if arg.abs() < 1e-9 { 1.0 } else { arg.sin() / arg };
            // Blackman-fönster över kärnans bredd.
            let x = (t / (HALF as f64 + 0.5)).abs();
            if x >= 1.0 {
                continue;
            }
            let w = 0.42 + 0.5 * (std::f64::consts::PI * x).cos() + 0.08 * (2.0 * std::f64::consts::PI * x).cos();
            let tap = sinc * w;
            acc_l += left[idx as usize] as f64 * tap;
            acc_r += right[idx as usize] as f64 * tap;
            weight += tap;
        }
        // Normalisera mot kärnans summa: annars får en ton nära DC olika nivå än en nära Nyquist.
        let norm = if weight.abs() < 1e-9 { 1.0 } else { weight };
        out_l.push((acc_l / norm) as f32);
        out_r.push((acc_r / norm) as f32);
    }
    (out_l, out_r)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::neural_separator::resample_linear;

    /// Ett varv upp och ned ska lämna tonen nästan orörd. Linjär interpolation gör inte det: den
    /// tappar diskant och lägger till fel, och felet syns som skillnaden mot originalet.
    #[test]
    fn a_round_trip_through_sinc_keeps_the_signal() {
        let from = 44_100u32;
        let mid = 48_000u32;
        let n = from as usize / 2;
        // 6 kHz — högt nog att linjär interpolation ska smeta ut den märkbart.
        let tone: Vec<f32> = (0..n)
            .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 6000.0 * i as f32 / from as f32).sin())
            .collect();

        let (sinc_mid, _) = resample_sinc(&tone, &tone, from, mid);
        let (sinc_back, _) = resample_sinc(&sinc_mid, &sinc_mid, mid, from);
        let (lin_mid, _) = resample_linear(&tone, &tone, from, mid);
        let (lin_back, _) = resample_linear(&lin_mid, &lin_mid, mid, from);

        // Mät felet i mitten av filen, där kärnan har sampel på båda sidor.
        let skip = 64;
        let len = (sinc_back.len().min(lin_back.len())).min(tone.len()) - skip;
        let err = |out: &[f32]| -> f64 {
            let mut sum = 0.0f64;
            for i in skip..skip + len {
                let d = (out[i] - tone[i]) as f64;
                sum += d * d;
            }
            (sum / len as f64).sqrt()
        };
        let (e_sinc, e_lin) = (err(&sinc_back), err(&lin_back));
        println!(
            "rundtur 44,1→48→44,1 kHz, 6 kHz-ton: sinc {:.1} dB, linjär {:.1} dB (lägre är bättre)",
            20.0 * e_sinc.log10(),
            20.0 * e_lin.log10()
        );
        assert!(
            e_sinc < e_lin * 0.5,
            "sinc ska ha minst halva felet mot linjär: sinc {e_sinc:.6} linjär {e_lin:.6}"
        );
    }

    /// Samma samplingsfrekvens ska ge identisk data, inte en kopia genom en kärna.
    #[test]
    fn same_rate_is_a_passthrough() {
        let x: Vec<f32> = (0..100).map(|i| i as f32 * 0.01).collect();
        let (l, r) = resample_sinc(&x, &x, 44_100, 44_100);
        assert_eq!(l, x);
        assert_eq!(r, x);
    }
}
