//! **Klanganalys** — en inspelad klang blir dragstänger (FL Harmor: `Analyze timbre`).
//!
//! Ingen ny motor: analysen fyller samma 64 nivåer som bildvägen och rattarna gör, och skickas
//! som `SetHarmonics`. Det gör hela steget provbart utan fönster: in med PCM, ut med nivåer.

use crate::audio::synth::HARMONIC_COUNT;

/// Fönstrets storlek. 8 192 sampel vid 44,1 kHz är 186 ms — nog att skilja partial 1 från 2 vid
/// 55 Hz (0,9 Hz per bin) utan att analysen känns långsam.
const WINDOW: usize = 8192;
/// Hur många fönster som prövas när det mest tonala stället letas upp. Ett anslag är brett och
/// brusigt; svansen är ofta den enda del där övertonerna syns.
const CHUNKS: usize = 8;

fn pow2_floor(v: usize) -> usize {
    let mut n = 1;
    while n * 2 <= v {
        n *= 2;
    }
    n
}

/// Radix-2 FFT på plats. Samma räkning som spektrumanalysatorn använder — ingen ny väg för
/// samma matematik.
fn fft(re: &mut [f32], im: &mut [f32]) {
    let n = re.len();
    if n <= 1 {
        return;
    }
    // Bitar omvänd ordning.
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let mut len = 2;
    while len <= n {
        let ang = -std::f32::consts::TAU / len as f32;
        let (wr, wi) = (ang.cos(), ang.sin());
        let mut i = 0;
        while i < n {
            let (mut cr, mut ci) = (1.0f32, 0.0f32);
            for k in 0..len / 2 {
                let (ur, ui) = (re[i + k], im[i + k]);
                let (vr, vi) = (
                    re[i + k + len / 2] * cr - im[i + k + len / 2] * ci,
                    re[i + k + len / 2] * ci + im[i + k + len / 2] * cr,
                );
                re[i + k] = ur + vr;
                im[i + k] = ui + vi;
                re[i + k + len / 2] = ur - vr;
                im[i + k + len / 2] = ui - vi;
                let ncr = cr * wr - ci * wi;
                ci = cr * wi + ci * wr;
                cr = ncr;
            }
            i += len;
        }
        len <<= 1;
    }
}

/// Var i samplingen tonen är som tydligast: fönstret med högst RMS. Ett ensamt fönster i början
/// träffar anslaget, som är brett och brusigt, och då blir analysen av en bastrumma vit brus i
/// stället för en klang.
fn loudest_window(pcm: &[f32], n: usize) -> usize {
    if pcm.len() <= n {
        return 0;
    }
    let span = pcm.len() - n;
    let step = (span / CHUNKS).max(1);
    let mut best = (0.0f32, 0usize);
    let mut start = 0usize;
    while start <= span {
        let sum: f32 = pcm[start..start + n].iter().map(|s| s * s).sum();
        let rms = (sum / n as f32).sqrt();
        if rms > best.0 {
            best = (rms, start);
        }
        start += step;
    }
    best.1
}

/// **En klang blir 64 dragstänger.** `fundamental_hz` är den ton analysen mäts mot — appen tar
/// kanalens grundton. Nivåerna normaliseras till peak 1,0; tystnad ger nollor, aldrig division
/// med noll.
pub fn envelope_from_pcm(
    pcm: &[f32],
    sample_rate: f32,
    fundamental_hz: f32,
) -> [f32; HARMONIC_COUNT] {
    let mut out = [0.0f32; HARMONIC_COUNT];
    if pcm.len() < 256 || sample_rate <= 0.0 || fundamental_hz <= 0.0 {
        return out;
    }
    let n = pow2_floor(pcm.len().min(WINDOW));
    if n < 256 {
        return out;
    }
    let start = loudest_window(pcm, n);
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    for (i, sample) in pcm[start..start + n].iter().enumerate() {
        let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos();
        re[i] = sample * w;
    }
    fft(&mut re, &mut im);

    let bins = n / 2;
    let hz_per_bin = sample_rate / n as f32;
    // En fullskalig sinus i sin egen bin ger |X| ≈ N/4 med Hann-fönster.
    let full_scale = n as f32 * 0.25;
    let mut peak = 0.0f32;
    for k in 1..=HARMONIC_COUNT {
        let target = k as f32 * fundamental_hz;
        let center = (target / hz_per_bin).round() as usize;
        if center >= bins {
            break;
        }
        // ±1 bin: grundtonen i en sampling är sällan exakt den not man anger.
        let lo = center.saturating_sub(1).max(1);
        let hi = (center + 1).min(bins - 1);
        let mut mag = 0.0f32;
        for b in lo..=hi {
            let m = (re[b] * re[b] + im[b] * im[b]).sqrt();
            if m > mag {
                mag = m;
            }
        }
        out[k - 1] = mag / full_scale;
        peak = peak.max(out[k - 1]);
    }
    if peak > 0.0 {
        for v in out.iter_mut() {
            *v = (*v / peak).clamp(0.0, 1.0);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(hz: f32, rate: f32, len: usize, partials: &[(f32, f32)]) -> Vec<f32> {
        (0..len)
            .map(|i| {
                let t = i as f32 / rate;
                partials
                    .iter()
                    .map(|(mult, amp)| amp * (std::f32::consts::TAU * hz * mult * t).sin())
                    .sum::<f32>()
                    * 0.5
            })
            .collect()
    }

    /// En ren sinus ska ge **en** dragstång, inte en smet. Det är skillnaden mellan en analys och
    /// ett brus.
    #[test]
    fn a_pure_tone_puts_all_its_energy_in_one_partial() {
        let pcm = tone(440.0, 44100.0, 16384, &[(1.0, 1.0)]);
        let env = envelope_from_pcm(&pcm, 44100.0, 440.0);
        assert!(env[0] > 0.95, "grundtonen: {}", env[0]);
        let rest: f32 = env[1..].iter().sum();
        assert!(rest < 0.05, "övertonerna borde vara tysta: {rest}");
    }

    /// En sågtand har 1/k över sina övertoner. Analysen ska se det — annars mäter den fel sak.
    #[test]
    fn a_sawtooth_falls_off_like_one_over_k() {
        let partials: Vec<(f32, f32)> = (1..=8).map(|k| (k as f32, 1.0 / k as f32)).collect();
        let pcm = tone(220.0, 44100.0, 16384, &partials);
        let env = envelope_from_pcm(&pcm, 44100.0, 220.0);
        assert!((env[0] - 1.0).abs() < 0.05, "grundtonen normaliseras till 1: {}", env[0]);
        assert!((env[1] - 0.5).abs() < 0.08, "andra övertonen: {}", env[1]);
        assert!((env[2] - 0.333).abs() < 0.08, "tredje övertonen: {}", env[2]);
        assert!(env[3] > env[6], "övertonerna ska avta");
    }

    /// Tystnad, för kort ljud och orimlig grundton ger nollor — ingen NaN, ingen panik.
    #[test]
    fn silence_and_nonsense_give_zeros() {
        assert!(envelope_from_pcm(&[0.0; 4096], 44100.0, 440.0).iter().all(|v| *v == 0.0));
        assert!(envelope_from_pcm(&[0.1; 64], 44100.0, 440.0).iter().all(|v| *v == 0.0));
        assert!(envelope_from_pcm(&[0.1; 4096], 0.0, 440.0).iter().all(|v| *v == 0.0));
        assert!(envelope_from_pcm(&[0.1; 4096], 44100.0, 0.0).iter().all(|v| *v == 0.0));
    }

    /// Analysen ska leta upp det tonala stället, inte nöja sig med första fönstret: en ton som
    /// börjar med brus ska ändå ge en ren partial.
    #[test]
    fn the_loudest_window_is_not_always_the_first() {
        let mut pcm = vec![0.02f32; 4096]; // svagt brus först
        pcm.extend(tone(330.0, 44100.0, 16384, &[(1.0, 1.0)]));
        let env = envelope_from_pcm(&pcm, 44100.0, 330.0);
        assert!(env[0] > 0.9, "grundtonen hittades inte: {}", env[0]);
    }
}
