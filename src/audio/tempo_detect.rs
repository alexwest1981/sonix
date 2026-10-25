//! **Tempo ur ljudet** — vad en importerad mp3:a går i.
//!
//! Utan den här siffran kan ett klipp aldrig följa projektets tempo: `source_bpm = 0` betyder
//! "okänt", och då rörs ljudet inte alls när tempot ändras. FL gör samma mätning när en fil
//! importeras; den är därför inte en gissning utan en förutsättning.
//!
//! Metoden är den vanliga och billiga: energi per bildruta → ökning mellan bildrutor (onsets) →
//! autokorrelation över de fördröjningar som svarar mot 60–190 BPM. Vid oktaffel ska 125 slås
//! fast och inte 250, så kandidaterna halveras och dubbleras och den som ligger i 80–160 vinner
//! om den har minst 60 % av toppens styrka.

const FRAME: usize = 512;
const HOP: usize = 256;
const MIN_BPM: f32 = 60.0;
const MAX_BPM: f32 = 190.0;
/// Bara de första sekunderna behövs; längre analys gör bara importen långsam.
const MAX_SECS: f32 = 60.0;

/// Tempot i BPM, eller `None` när ljudet inte bär någon takt (tystnad, en utdragen ton,
/// prat). Tröskeln är relativ: en topp som inte skiljer sig från medelvärdet är ingen takt.
pub fn detect_bpm(samples: &[f32], sample_rate: f32) -> Option<f32> {
    if sample_rate <= 0.0 || samples.len() < FRAME * 8 {
        return None;
    }
    let used = samples.len().min((sample_rate * MAX_SECS) as usize);

    // 1. Energi per bildruta.
    let frames = (used - FRAME) / HOP + 1;
    let mut energy = Vec::with_capacity(frames);
    for f in 0..frames {
        let start = f * HOP;
        let mut sum = 0.0_f32;
        for s in &samples[start..start + FRAME] {
            sum += s * s;
        }
        energy.push((sum / FRAME as f32).sqrt());
    }
    if energy.iter().all(|&e| e < 1e-5) {
        return None;
    }

    // 2. Onsetstyrka: bara ökningar räknas, annars blir varje tonavslut en puls.
    let mut onset = Vec::with_capacity(energy.len());
    let mut prev = 0.0;
    for &e in &energy {
        onset.push((e - prev).max(0.0));
        prev = e;
    }
    let mean = onset.iter().sum::<f32>() / onset.len() as f32;
    if mean <= 1e-6 {
        return None;
    }
    for o in onset.iter_mut() {
        *o -= mean; // nollställ så autokorrelationen inte dras upp av medelnivån
    }

    // 3. Autokorrelation över taktfördröjningarna.
    let frames_per_sec = sample_rate / HOP as f32;
    let min_lag = (frames_per_sec * 60.0 / MAX_BPM).round().max(1.0) as usize;
    let max_lag = (frames_per_sec * 60.0 / MIN_BPM).round() as usize;
    if max_lag >= onset.len() {
        return None;
    }
    let mut best = (0.0_f32, 0_usize);
    let mut total = 0.0_f32;
    let mut count = 0_usize;
    for lag in min_lag..=max_lag {
        let mut acc = 0.0_f32;
        for i in lag..onset.len() {
            acc += onset[i] * onset[i - lag];
        }
        total += acc.abs();
        count += 1;
        if acc > best.0 {
            best = (acc, lag);
        }
    }
    if count == 0 || best.1 == 0 {
        return None;
    }
    // En takt ska sticka ut; annars är det bara brus som korrelerar med sig själv.
    let avg = total / count as f32;
    if best.0 < avg * 1.6 {
        return None;
    }

    // 4. Oktaffel: 250 BPM är oftast 125 med dubbelt så många pulsslag.
    let bpm_at = |lag: usize| 60.0 * frames_per_sec / lag as f32;
    let mut chosen = bpm_at(best.1);
    let strength = |lag: usize| {
        if lag >= onset.len() {
            return 0.0;
        }
        onset[lag..].iter().zip(&onset[..onset.len() - lag]).map(|(a, b)| a * b).sum::<f32>()
    };
    for factor in [0.5_f32, 2.0] {
        let lag = (best.1 as f32 * factor).round() as usize;
        let candidate = bpm_at(lag.max(1));
        if lag > 0 && (80.0..=160.0).contains(&candidate) && strength(lag) > best.0 * 0.6 {
            chosen = candidate;
            break;
        }
    }
    if (MIN_BPM..=MAX_BPM).contains(&chosen) { Some(chosen) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Klick på jämna avstånd: en kort brusburst med snabb avklingning, som en kick.
    fn click_track(bpm: f32, secs: f32, sr: f32) -> Vec<f32> {
        let n = (sr * secs) as usize;
        let mut out = vec![0.0_f32; n];
        let period = (60.0 / bpm * sr) as usize;
        let mut seed = 0x1234_5678_u32;
        let mut at = 0;
        while at < n {
            for i in 0..(sr * 0.05) as usize {
                if at + i >= n {
                    break;
                }
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let noise = (seed >> 9) as f32 / (1 << 23) as f32 - 1.0;
                let env = (-(i as f32) / (sr * 0.008)).exp();
                out[at + i] += 0.6 * noise * env;
            }
            at += period;
        }
        out
    }

    #[test]
    fn a_click_track_is_measured_to_its_own_tempo() {
        for bpm in [90.0_f32, 125.0, 140.0] {
            let audio = click_track(bpm, 12.0, 44_100.0);
            let found = detect_bpm(&audio, 44_100.0).expect("takten ska hittas");
            assert!(
                (found - bpm).abs() <= 2.0,
                "mätte {found:.1} BPM, väntade {bpm:.0}"
            );
        }
    }

    #[test]
    fn silence_and_a_held_tone_have_no_tempo() {
        let sr = 44_100.0;
        assert!(detect_bpm(&vec![0.0; 44_100], sr).is_none());
        let tone: Vec<f32> = (0..44_100)
            .map(|i| (std::f32::consts::TAU * 220.0 * i as f32 / sr).sin() * 0.3)
            .collect();
        assert!(detect_bpm(&tone, sr).is_none(), "en utdragen ton har ingen takt");
    }
}
