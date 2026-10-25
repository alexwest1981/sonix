//! **Edison**: redigering av ett urval ur en stereobuffert.
//!
//! Allt här är rena funktioner — inga filer, ingen UI, inget tillstånd. Redigeringen sker
//! på en **kopia** av klippets ljud: originalet ligger kvar både i minnet och på disken,
//! och resultatet blir ett nytt spår. Det är därför en misslyckad redigering inte kan
//! förstöra något som redan låter bra.

/// Urval i sampel, alltid `[start, slut)` och klippt mot buffertens längd. Tåligt mot
/// omvända och för stora intervall: en dragning som börjar till höger ska betyda samma sak.
pub fn selection(len: usize, a: usize, b: usize) -> (usize, usize) {
    let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
    (lo.min(len), hi.min(len))
}

/// Vänder urvalet. Att vända två gånger ger tillbaka originalet — provet mäter det.
pub fn reverse(l: &mut [f32], r: &mut [f32], a: usize, b: usize) {
    let (a, b) = selection(l.len().min(r.len()), a, b);
    l[a..b].reverse();
    r[a..b].reverse();
}

/// Skalar urvalet så att dess topp blir `peak` (0–1). Tyst ljud lämnas orört — att
/// förstärka brus till full skala är inte normalisering.
pub fn normalize(l: &mut [f32], r: &mut [f32], a: usize, b: usize, peak: f32) {
    let (a, b) = selection(l.len().min(r.len()), a, b);
    let top = l[a..b]
        .iter()
        .chain(r[a..b].iter())
        .fold(0.0_f32, |m, v| m.max(v.abs()));
    if top < 1e-6 {
        return;
    }
    let gain = peak.clamp(0.0, 1.0) / top;
    for v in l[a..b].iter_mut().chain(r[a..b].iter_mut()) {
        *v *= gain;
    }
}

/// Linjär toning över urvalet. `fade_in` går 0 → 1, annars 1 → 0.
pub fn fade(l: &mut [f32], r: &mut [f32], a: usize, b: usize, fade_in: bool) {
    let (a, b) = selection(l.len().min(r.len()), a, b);
    let n = b - a;
    if n < 2 {
        return;
    }
    for i in 0..n {
        let t = i as f32 / (n - 1) as f32;
        let gain = if fade_in { t } else { 1.0 - t };
        l[a + i] *= gain;
        r[a + i] *= gain;
    }
}

pub fn silence(l: &mut [f32], r: &mut [f32], a: usize, b: usize) {
    let (a, b) = selection(l.len().min(r.len()), a, b);
    l[a..b].fill(0.0);
    r[a..b].fill(0.0);
}

/// Tar bort urvalet och returnerar resten. Längden ändras, så resultatet blir ett nytt
/// klipp — aldrig en tyst förskjutning av det gamla.
pub fn cut(l: &[f32], r: &[f32], a: usize, b: usize) -> (Vec<f32>, Vec<f32>) {
    let (a, b) = selection(l.len().min(r.len()), a, b);
    let mut out_l = Vec::with_capacity(l.len() - (b - a));
    let mut out_r = Vec::with_capacity(r.len() - (b - a));
    out_l.extend_from_slice(&l[..a]);
    out_l.extend_from_slice(&l[b..]);
    out_r.extend_from_slice(&r[..a]);
    out_r.extend_from_slice(&r[b..]);
    (out_l, out_r)
}

/// Behåller bara urvalet.
pub fn trim(l: &[f32], r: &[f32], a: usize, b: usize) -> (Vec<f32>, Vec<f32>) {
    let (a, b) = selection(l.len().min(r.len()), a, b);
    (l[a..b].to_vec(), r[a..b].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(n: usize) -> (Vec<f32>, Vec<f32>) {
        let l: Vec<f32> = (0..n).map(|i| i as f32 / n as f32).collect();
        let r: Vec<f32> = l.iter().map(|v| -v).collect();
        (l, r)
    }

    #[test]
    fn reversing_twice_restores_the_selection() {
        let (mut l, mut r) = ramp(64);
        let before = (l.clone(), r.clone());
        reverse(&mut l, &mut r, 10, 50);
        reverse(&mut l, &mut r, 10, 50);
        assert_eq!(l, before.0);
        assert_eq!(r, before.1);
    }

    #[test]
    fn normalize_reaches_the_requested_peak_and_leaves_silence_alone() {
        let (mut l, mut r) = (vec![0.1_f32, -0.25], vec![0.05_f32, 0.0]);
        normalize(&mut l, &mut r, 0, 2, 0.5);
        let top = l.iter().chain(r.iter()).fold(0.0_f32, |m, v| m.max(v.abs()));
        assert!((top - 0.5).abs() < 1e-6, "toppen blev {top}");

        let (mut quiet_l, mut quiet_r) = (vec![0.0_f32; 4], vec![0.0_f32; 4]);
        normalize(&mut quiet_l, &mut quiet_r, 0, 4, 1.0);
        assert!(quiet_l.iter().all(|v| *v == 0.0), "tystnad ska förbli tystnad");
    }

    #[test]
    fn fade_in_starts_at_zero_and_ends_at_full() {
        let (mut l, mut r) = (vec![1.0_f32; 5], vec![1.0_f32; 5]);
        fade(&mut l, &mut r, 0, 5, true);
        assert_eq!(l[0], 0.0, "första sampeln ska vara tyst");
        assert!((l[4] - 1.0).abs() < 1e-6, "sista ska vara orörd");
        assert!(l.windows(2).all(|w| w[0] <= w[1]), "toningen ska gå uppåt");
    }

    #[test]
    fn cut_removes_exactly_the_selection_and_trim_keeps_it() {
        let (l, r) = ramp(10);
        let (cut_l, cut_r) = cut(&l, &r, 3, 5);
        assert_eq!(cut_l.len(), 8);
        assert_eq!(&cut_l[..3], &l[..3]);
        assert_eq!(&cut_l[3..], &l[5..]);
        assert_eq!(cut_r.len(), 8);

        let (trim_l, trim_r) = trim(&l, &r, 3, 5);
        assert_eq!(trim_l, l[3..5].to_vec());
        assert_eq!(trim_r, r[3..5].to_vec());
    }

    /// En ren ton ska hamna i den bin som ligger närmast dess frekvens — log-axeln är
    /// annars bara en bild som ser trovärdig ut.
    #[test]
    fn the_spectrum_puts_a_tone_where_its_frequency_is() {
        let sr = 48_000.0;
        let n = 8192;
        let tone: Vec<f32> = (0..n)
            .map(|i| 0.5 * (std::f32::consts::TAU * 1_000.0 * i as f32 / sr).sin())
            .collect();
        let bins = 96;
        let spec = spectrum_db(&tone, sr, bins);
        let (loudest, level) = spec
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(i, v)| (i, *v))
            .unwrap();
        let (lo, hi) = (40.0_f32, 16_000.0_f32);
        let centre = lo * (hi / lo).powf((loudest as f32 + 0.5) / bins as f32);
        assert!(
            (centre - 1_000.0).abs() < 120.0,
            "toppen hamnade på {centre:.0} Hz (bin {loudest}, nivå {level:.2})"
        );
        assert!(level > 0.5, "en halvskalig ton ska synas tydligt: {level:.2}");
    }

    #[test]
    fn silence_has_no_spectrum_and_noise_is_broad() {
        let sr = 48_000.0;
        let silence = vec![0.0f32; 8192];
        assert!(spectrum_db(&silence, sr, 32).iter().all(|v| *v < 1e-6));

        // Deterministiskt brus: varje band ska ha energi, ingen ska dominera helt.
        let mut seed = 12345u32;
        let noise: Vec<f32> = (0..8192)
            .map(|_| {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 9) as f32 / 4_194_304.0 - 1.0
            })
            .collect();
        let spec = spectrum_db(&noise, sr, 32);
        let above = spec.iter().filter(|v| **v > 0.3).count();
        assert!(above >= 24, "brus ska fylla de flesta band: {above} av 32");
    }
}

/// **Spektrum för urvalet** — `bins` logaritmiskt fördelade band från 40 Hz till 16 kHz,
/// normaliserade till 0…1 över −60…0 dB. Logaritmisk axel därför att ett linjärt spektrum
/// lägger tre fjärdedelar av bredden över 8 kHz, där det sällan finns något att titta på.
///
/// Fönstret är Hann och längden närmaste tvåpotens under 8192 sampel: en FFT på hela
/// klippet hade gett 0,02 Hz per bin och en bild som inte går att läsa.
pub fn spectrum_db(samples: &[f32], sample_rate: f32, bins: usize) -> Vec<f32> {
    let bins = bins.clamp(4, 256);
    let mut out = vec![0.0f32; bins];
    if samples.len() < 64 || sample_rate <= 0.0 {
        return out;
    }
    let n = {
        let mut p = 1usize;
        while p * 2 <= samples.len().min(8192) {
            p *= 2;
        }
        p
    };
    if n < 64 {
        return out;
    }
    let start = (samples.len() - n) / 2;
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    for (i, slot) in re.iter_mut().enumerate() {
        let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos();
        *slot = samples[start + i] * w;
    }
    crate::audio::vocal_harmonizer::fft_radix2(&mut re, &mut im, false);
    // En fullskalig sinus i sin egen bin ger |X| ≈ n/4 med Hann-fönster.
    let norm = (n as f32 / 4.0).max(1.0);
    let hz_per_bin = sample_rate / n as f32;
    let (lo, hi) = (40.0_f32, 16_000.0_f32);
    for (i, slot) in out.iter_mut().enumerate() {
        let f0 = lo * (hi / lo).powf(i as f32 / bins as f32);
        let f1 = lo * (hi / lo).powf((i + 1) as f32 / bins as f32);
        let b0 = (f0 / hz_per_bin).floor().max(1.0) as usize;
        let b1 = (f1 / hz_per_bin).ceil().max(b0 as f32 + 1.0) as usize;
        let mut peak = 0.0_f32;
        for b in b0..b1.min(n / 2) {
            peak = peak.max((re[b] * re[b] + im[b] * im[b]).sqrt() / norm);
        }
        let db = 20.0 * peak.max(1e-6).log10();
        *slot = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
    }
    out
}
