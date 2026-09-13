//! Registret i utgången — vilka frekvensband som bär energi — och mätarskalan (8.13).
//!
//! **Varför en egen modul och inte i UI:t:** regeln ("vilket band bär energin", och
//! hur en amplitud blir en stapel) är ren matematik. Den ska kunna prövas utan
//! fönster; UI:t ritar bara siffrorna den får.
//!
//! **Pröva instrumentet på en jämn ton först.** Samma läxa som slagletningen i 8.7,
//! där tre rimliga varianter gav fel svar på en *ren sinuston* medan ett
//! klickmönster såg rätt ut: ett instrument man inte prövat på det enklaste fallet
//! vet man ingenting om. `a_steady_tone_lands_in_its_own_register` är det testet.
//!
//! FFT:n är repots egen radix-2 (i `vocal_harmonizer`) — **en** FFT, inte två.
//!
//! Status: ny (8.13) — toppradens nivå- och registermätare

use super::vocal_harmonizer::fft_radix2;

/// Registren och deras gränser i Hz. **En tabell, ett index** — namnen och
/// gränserna kan inte driva ifrån varandra, för de ligger i samma post (repots
/// läxa från 8.11: två listor för samma sak gav fel ton för fyra av tolv val).
pub const REGISTERS: [(&str, f32, f32); 6] = [
    ("SUB", 20.0, 80.0),
    ("BAS", 80.0, 300.0),
    ("L-MID", 300.0, 1000.0),
    ("MID", 1000.0, 3000.0),
    ("DISK", 3000.0, 8000.0),
    ("TOPP", 8000.0, 16000.0),
];

/// Fönstrets längd i sampel: 1024 vid 44,1 kHz ≈ 23 ms — kort nog att följa
/// musiken, långt nog att en baston hinner bli en tydlig puckel.
pub const WINDOW: usize = 1024;

/// Skalans botten. En mätare behöver ett golv, och det här är valt ur en mätning,
/// inte ur tycke: Alex' egna Suno-stämmor toppar mellan **0,005 och 0,36**
/// (trummorna 0,026) — med ett golv på -60 dB låg varje band utom det starkaste i
/// botten och registret såg dött ut på riktig musik. -72 dB rymmer hans material
/// med marginal, och det är samma golv för nivåmätaren och registret: två skalor
/// för samma ljud vore två svar på samma fråga.
pub const FLOOR_DB: f32 = -72.0;

/// Skalans topp. Över 0 dB finns headroom kvar att se — en mätare som tar slut
/// exakt vid 0 dB kan inte visa att man ligger över.
pub const CEIL_DB: f32 = 6.0;

/// dB som mätarställning 0..1. **En mappning, en plats** — både nivåmätaren och
/// registerstapel använder den, så de två kan inte visa olika höjd för samma ljud.
pub fn db_to_unit(db: f32) -> f32 {
    ((db - FLOOR_DB) / (CEIL_DB - FLOOR_DB)).clamp(0.0, 1.0)
}

/// Amplitud (0..1) som mätarställning 0..1, i dB. Örat hör i dB, och på en linjär
/// skala ser allt utom det starkaste registret tomt ut.
pub fn amp_to_unit(amp: f32) -> f32 {
    db_to_unit(20.0 * amp.max(1e-6).log10())
}

/// Närmaste tvåpotens som ryms i `v`, minst 1 — FFT:n kräver tvåpotens.
fn prev_pow2(v: usize) -> usize {
    let mut p = 1usize;
    while p * 2 <= v {
        p *= 2;
    }
    p
}

/// Nivån per register, 0..1, ur en buffert med utgångsljud.
///
/// **Absolut nivå, inte en normaliserad balans.** Första försöket normaliserade
/// varje fönster mot sin egen starkaste delton. Mätt på Alex' stämmor 2026-09-13
/// blev resultatet att *nästan varje band stod på full höjd* — varje 23 ms-fönster
/// har någon delton som är starkast, så en normalisering kastar bort just den
/// informationen man vill se. Med en absolut skala (golvet i `FLOOR_DB`, valt ur
/// hans material) syns det i stället att bastrumman ligger i SUB/BAS och att
/// hi-haten är svagare. Registret och nivåmätaren bredvid talar därför samma språk.
///
/// Fönstret (Hann) läggs över de **sista** samplen — det som hörs nu, inte det som
/// hördes för en sekund sedan. Utan fönstret läcker en baston ut i grannbanden, och
/// då visar mätaren fel register.
///
/// Varje register rapporterar sitt **starkaste** värde. Mätt: med medelvärdet över
/// bandet gav en fullskalig 1981 Hz-ton 0,68 i stället för 0,91 — en enda delton
/// dränks av bandets bredd, och felet växer med bandet (TOPP är sexton gånger
/// bredare än SUB). En ton är det enkla fallet, och där ska instrumentet vara rätt.
pub fn band_levels(samples: &[f32], sample_rate: f32) -> Vec<f32> {
    let mut out = vec![0.0f32; REGISTERS.len()];
    if sample_rate <= 0.0 || samples.len() < 64 {
        return out;
    }
    let n = prev_pow2(samples.len().min(WINDOW));
    if n < 64 {
        return out;
    }
    let tail = &samples[samples.len() - n..];
    let mut re = vec![0.0f32; n];
    let mut im = vec![0.0f32; n];
    for (i, sample) in tail.iter().enumerate() {
        let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos();
        re[i] = sample * w;
    }
    fft_radix2(&mut re, &mut im, false);

    let bins = n / 2;
    let hz_per_bin = sample_rate / n as f32;
    // En fullskalig sinus i sin egen bin ger |X| ≈ N/4 med Hann-fönster: skalans nolla.
    let full_scale = n as f32 * 0.25;
    for (i, (_, lo, hi)) in REGISTERS.iter().enumerate() {
        // Bin 0 är likströmmen och hoppas över; ett band får aldrig vara tomt.
        let b0 = ((lo / hz_per_bin).floor() as usize).clamp(1, bins.saturating_sub(1));
        let b1 = ((hi / hz_per_bin).ceil() as usize).clamp(b0 + 1, bins);
        let mut peak = 0.0f32;
        for b in b0..b1 {
            let mag = (re[b] * re[b] + im[b] * im[b]).sqrt();
            if mag > peak {
                peak = mag;
            }
        }
        out[i] = amp_to_unit(peak / full_scale);
    }
    out
}

/// Följer en nivå uppåt fort och nedåt långsamt: en mätare som studsar tillbaka
/// lika fort som ljudet är omöjlig att läsa. `ups`/`downs` är andelen av avståndet
/// som tas per bildruta (0..1).
pub fn smooth(levels: &mut [f32], new: &[f32], ups: f32, downs: f32) {
    let ups = ups.clamp(0.0, 1.0);
    let downs = downs.clamp(0.0, 1.0);
    for (i, level) in levels.iter_mut().enumerate() {
        let target = new.get(i).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let k = if target >= *level { ups } else { downs };
        *level += (target - *level) * k;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine(hz: f32, amp: f32, rate: f32, len: usize) -> Vec<f32> {
        (0..len)
            .map(|i| (std::f32::consts::TAU * hz * i as f32 / rate).sin() * amp)
            .collect()
    }

    fn band_index(name: &str) -> usize {
        REGISTERS
            .iter()
            .position(|r| r.0 == name)
            .unwrap_or_else(|| panic!("registret {name} finns inte"))
    }

    fn loudest(levels: &[f32]) -> &'static str {
        REGISTERS
            .iter()
            .zip(levels.iter())
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .map(|(r, _)| r.0)
            .expect("minst ett register")
    }

    /// En tabell med fel längd flyttar tyst fel val (8.11). Här räknas posterna och
    /// gränserna kontrolleras: stigande, utan glapp och utan överlapp.
    #[test]
    fn the_register_table_is_ordered_and_complete() {
        assert_eq!(REGISTERS.len(), 6, "sex register — räkna dem, gissa inte");
        for w in REGISTERS.windows(2) {
            assert!(
                w[0].1 < w[0].2,
                "{}: nedre gränsen ska ligga under den övre",
                w[0].0
            );
            assert_eq!(w[0].2, w[1].1, "{} och {} ska mötas utan glapp", w[0].0, w[1].0);
        }
    }

    /// Det viktigaste testet: en jämn ton ska hamna i sitt EGET register.
    #[test]
    fn a_steady_tone_lands_in_its_own_register() {
        let rate = 44_100.0;
        for (hz, want) in [(150.0f32, "BAS"), (2000.0, "MID"), (5000.0, "DISK")] {
            let levels = band_levels(&sine(hz, 0.5, rate, WINDOW), rate);
            assert_eq!(
                loudest(&levels),
                want,
                "{hz} Hz hamnade i {}: {levels:?}",
                loudest(&levels)
            );
        }
    }

    /// Tystnad är tom — och det gäller även nästan-tystnad: under golvet visas
    /// ingenting, i stället för att brus lyfts upp och ser ut som musik.
    #[test]
    fn silence_is_empty_and_so_is_what_sits_under_the_floor() {
        let rate = 44_100.0;
        let quiet = band_levels(&vec![0.0f32; WINDOW], rate);
        assert!(quiet.iter().all(|l| *l == 0.0), "tystnad: {quiet:?}");

        let whisper = vec![1e-4f32; WINDOW]; // ≈ -80 dBFS
        let levels = band_levels(&whisper, rate);
        assert!(levels.iter().all(|l| *l == 0.0), "under golvet: {levels:?}");
    }

    /// Skalan är absolut: en stark delton läses högt och en svag läses lågt, i
    /// SAMMA fönster. Det är skillnaden mot att normalisera (då står allt på topp).
    #[test]
    fn a_strong_partial_reads_high_and_a_weak_one_reads_low() {
        let rate = 44_100.0;
        let samples: Vec<f32> = (0..WINDOW)
            .map(|i| {
                let t = i as f32 / rate;
                (std::f32::consts::TAU * 1500.0 * t).sin()
                    + 0.001 * (std::f32::consts::TAU * 9000.0 * t).sin() // -60 dB
            })
            .collect();
        let levels = band_levels(&samples, rate);
        assert!(
            levels[band_index("MID")] > 0.85,
            "den starka deltonen: {}",
            levels[band_index("MID")]
        );
        let weak = levels[band_index("TOPP")];
        assert!(
            (0.1..0.25).contains(&weak),
            "-60 dB ska ligga en bit över golvet och långt under nollan: {weak}"
        );
    }

    /// Skalan är densamma hur ful vågformen än är: allt ligger inom 0..1.
    #[test]
    fn levels_stay_within_the_unit_interval() {
        let mut samples = vec![0.0f32; WINDOW];
        for (i, s) in samples.iter_mut().enumerate() {
            *s = if (i / 40) % 2 == 0 { 1.0 } else { -1.0 };
        }
        let levels = band_levels(&samples, 44_100.0);
        assert!(levels.iter().all(|l| (0.0..=1.0).contains(l)), "{levels:?}");
        assert_eq!(amp_to_unit(0.0), 0.0);
        assert!(amp_to_unit(1.0) > 0.85, "0 dB ska ligga högt: {}", amp_to_unit(1.0));
        assert_eq!(db_to_unit(100.0), 1.0, "över toppen klipps, inte rinner över");
    }

    /// Mätning mot riktiga filer — `#[ignore]`, för den läser Alex' egna stämmor
    /// och ska inte krävas av CI:
    ///
    /// `cargo test --release --bin sonix the_register_view -- --ignored --nocapture`
    ///
    /// Poängen är inte en siffra i CI utan att se att mätaren visar det örat hör:
    /// en bas i BAS/SUB, en sång i MID, en hi-hat i TOPP. Utan den kontrollen vore
    /// "registret" ett påstående om en bild ingen prövat mot ljud.
    #[test]
    #[ignore]
    fn the_register_view_on_real_stems() {
        let dir = std::env::var("SONIX_STEM_DIR")
            .unwrap_or_else(|_| "imported_stems/Broken".to_string());
        let Ok(entries) = std::fs::read_dir(&dir) else {
            println!("hittar ingen stämmapp: {dir} — hoppar över");
            return;
        };
        let mut paths: Vec<std::path::PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().map(|e| e == "wav").unwrap_or(false))
            .collect();
        paths.sort();
        for path in paths {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let Ok((left, right, rate)) = crate::audio::load_audio_pcm(&path.to_string_lossy())
            else {
                println!("  {name}: kunde inte läsas");
                continue;
            };
            let mut mono = left.clone();
            for (i, r) in right.iter().enumerate() {
                if let Some(m) = mono.get_mut(i) {
                    *m = (*m + r) * 0.5;
                }
            }
            // Tolv fönster jämnt ut över låten och HÖGSTA värdet per band: en mätare
            // står och faller över tid, så en enda 23 ms-bild säger ingenting om
            // stämman. Ett enda fönster kan dessutom råka hamna i en paus.
            let mut top = vec![0.0f32; REGISTERS.len()];
            let mut peak_amp = 0.0f32;
            let windows = 12usize;
            for w in 0..windows {
                let start = (mono.len() * w) / windows;
                let end = (start + WINDOW).min(mono.len());
                if end <= start {
                    continue;
                }
                let window = &mono[start..end];
                peak_amp = peak_amp.max(window.iter().fold(0.0f32, |m, s| m.max(s.abs())));
                for (i, level) in band_levels(window, rate as f32).iter().enumerate() {
                    if *level > top[i] {
                        top[i] = *level;
                    }
                }
            }
            let loudest = REGISTERS
                .iter()
                .zip(top.iter())
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .map(|(r, _)| r.0)
                .unwrap_or("-");
            println!(
                "  {name}: toppnivå {peak_amp:.3} → starkast {loudest} — {}",
                REGISTERS
                    .iter()
                    .zip(top.iter())
                    .map(|(r, l)| format!("{} {:.2}", r.0, l))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
    }

    /// Mätaren går upp fort och ner långsamt, och slår aldrig över.
    #[test]
    fn smoothing_rises_fast_and_falls_slowly_without_overshooting() {
        let mut levels = vec![0.0f32, 1.0];
        smooth(&mut levels, &[1.0, 0.0], 0.5, 0.25);
        assert!((levels[0] - 0.5).abs() < 1e-6, "halva vägen upp: {}", levels[0]);
        assert!((levels[1] - 0.75).abs() < 1e-6, "fjärdedelen ned: {}", levels[1]);
        for _ in 0..50 {
            smooth(&mut levels, &[1.0, 0.0], 0.5, 0.25);
        }
        assert!(levels[0] <= 1.0 && levels[1] >= 0.0, "ingen översläng: {levels:?}");
    }
}
