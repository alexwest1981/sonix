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
}
