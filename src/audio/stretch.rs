//! Tempoföljning med bevarad tonhöjd (Fas 8.10, steg 2).
//!
//! **Problemet.** Steg 1 gjorde klippen till bandspelare: när projektets BPM ändras följer
//! tonhöjden med, och en sångröst går från Leonard Cohen till smurfarna. Det här modulen
//! räknar i stället om ljudet med **bevarad tonhöjd**.
//!
//! **Varför offline till en fil och inte per sample i ljudtråden.** Beslutet är fattat med
//! researchunderlag (ROADMAP § 8.10 och `references/daw-research/08`–`10`):
//!
//! - Ingen av de undersökta DAW:erna lägger tempo-stretch på en hostad plugin i
//!   uppspelningsvägen; mönstret är inbäddad licensierad DSP eller egen DSP, och Pro Tools'
//!   tyngsta läge (X-Form) är uttryckligen *Rendered Only*.
//! - Att stretcha per sample i ljudtråden är dyrt (WSOLA-sökning per grain), allokerar och
//!   kan ge xrun. Att räkna **en gång** och spela en vanlig fil ger ingen ny felkälla i
//!   uppspelningen alls — och resultatet går att **verifiera** (längd, ändliga sampel, inte
//!   tyst) innan det används.
//!
//! Filen skrivs **atomiskt** (temp + rename): en halvskriven cache får aldrig kunna läsas som
//! ett giltigt klipp.
//!
//! **Formanter är inte problemet.** En korrekt pitch-bevarande tidsskalning flyttar inte
//! formanterna — formant-reglage hör till *pitch-shift*. Det som återstår efter en korrekt
//! sträckning är motorns artefakter. Läs `references/daw-research/10` innan du byter motor.

use std::path::{Path, PathBuf};

/// Samma spann som sångstudiens reglage och `stretch_ratio_for` i `app.rs`.
pub const MIN_RATIO: f32 = 0.25;
pub const MAX_RATIO: f32 = 4.0;

/// Vad som ska räknas fram för ett klipp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StretchPlan {
    /// Ut-tid / in-tid. 1,25 = klippet blir 25 % längre.
    pub ratio: f32,
    /// Antal stereoframes den färdiga filen ska ha.
    pub out_frames: usize,
}

impl StretchPlan {
    /// Är planen en faktisk ändring? (Faktor 1,0 ska aldrig rendera något.)
    pub fn changes_anything(&self) -> bool {
        (self.ratio - 1.0).abs() > 1e-4
    }
}

/// Räknar fram vad ett klipp behöver, eller `None` när inget ska göras.
///
/// **`None` är ett svar, inte ett fel.** Det betyder "rör inte ljudet", och det gäller:
///
/// - **okänt inspelningstempo** (`source_bpm <= 0`) — 8.10:s regel: en källa utan känt tempo
///   sträcks inte i smyg,
/// - **samma tempo** (`ratio ≈ 1,0`) — då är vägen bit-exakt som förut och ingen fil behövs,
/// - **tomt klipp** — det finns inget att sträcka.
///
/// Faktorn kläms till [`MIN_RATIO`]–[`MAX_RATIO`], samma spann som sångstudiens reglage.
pub fn plan(source_bpm: f32, project_bpm: f32, source_frames: usize) -> Option<StretchPlan> {
    if source_frames == 0 || source_bpm <= 0.0 || project_bpm <= 0.0 {
        return None;
    }
    let ratio = (project_bpm / source_bpm).clamp(MIN_RATIO, MAX_RATIO);
    if (ratio - 1.0).abs() <= 1e-4 {
        return None;
    }
    let out_frames = ((source_frames as f64) * (ratio as f64)).round().max(1.0) as usize;
    Some(StretchPlan { ratio, out_frames })
}

/// Vad som ska hända med **ett** klipp när tempot rör sig.
///
/// Tre utfall, och bara tre — det är hela användarvända regeln bakom "ett dumhuvud ska klara
/// det". Ingen algoritm väljs manuellt; klippets typ och switchen avgör.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FollowMode {
    /// Ljudet rörs inte: okänt inspelningstempo, samma tempo, eller switchen av.
    Untouched,
    /// Bandspelaren: tonhöjden följer med. Ett **aktivt** val per klipp (den som vill ha
    /// effekten), aldrig standard — det är smurfen.
    Tape,
    /// Tonhöjdsbevarande sträckning till en fil, spelad med faktor 1,0.
    Stretch,
}

/// Beslutet för ett klipp, med faktorn när en sträckning behövs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FollowDecision {
    pub mode: FollowMode,
    /// Ut-tid / in-tid. 1,0 när inget ska hända.
    pub ratio: f32,
}

/// Regeln: **följ tempot, bevara tonhöjden** — utom där klippet självt säger tape.
///
/// Ordningen är vald så att inget kan hända av misstag:
/// 1. **Okänt inspelningstempo** (`source_bpm <= 0`) → rör inte ljudet (8.10:s regel).
/// 2. **Switchen av** → rör inte ljudet. "Följ tempot" är en switch, inte en halv.
/// 3. **Klippet vill bandspelare** (`per_clip_tape`) → [`FollowMode::Tape`], tonhöjden följer.
/// 4. **Samma tempo** → inget att göra (faktor 1,0, bit-exakt väg).
/// 5. Annars → [`FollowMode::Stretch`] med projekt/källa, klämt till samma spann som
///    sångstudiens reglage.
pub fn decide(
    source_bpm: f32,
    project_bpm: f32,
    follow_tempo: bool,
    per_clip_tape: bool,
) -> FollowDecision {
    let untouched = FollowDecision {
        mode: FollowMode::Untouched,
        ratio: 1.0,
    };
    if source_bpm <= 0.0 || project_bpm <= 0.0 || !follow_tempo {
        return untouched;
    }
    let raw = (project_bpm / source_bpm).clamp(MIN_RATIO, MAX_RATIO);
    if (raw - 1.0).abs() <= 1e-4 {
        return untouched;
    }
    if per_clip_tape {
        return FollowDecision {
            mode: FollowMode::Tape,
            ratio: raw,
        };
    }
    FollowDecision {
        mode: FollowMode::Stretch,
        ratio: raw,
    }
}

/// Sträcker ett stereopar med bevarad tonhöjd.
///
/// Kanalerna går genom **samma** WSOLA-instans, så grainsökningen (som avgör var varje korn
/// läggs) görs en gång för båda kanalerna. Att köra två separata instanser skulle kunna välja
/// olika skiften för vänster och höger och därmed sprida ut stereobilden — samma fälla som ett
/// separat monoklipp per kanal.
///
/// Vid faktor 1,0 returneras ingången oförändrad: ingen grain-process, ingen förlust.
pub fn stretch_stereo(
    left: &[f32],
    right: &[f32],
    ratio: f32,
    sample_rate: f32,
) -> (Vec<f32>, Vec<f32>) {
    let frames = left.len().min(right.len());
    if frames == 0 || sample_rate <= 0.0 {
        return (Vec::new(), Vec::new());
    }
    let ratio = ratio.clamp(MIN_RATIO, MAX_RATIO);
    if (ratio - 1.0).abs() <= 1e-4 {
        return (left[..frames].to_vec(), right[..frames].to_vec());
    }
    let out_frames = ((frames as f64) * (ratio as f64)).round().max(1.0) as usize;

    let mut wsola = super::vocal_harmonizer::Wsola::new(sample_rate);
    wsola.set_ratio(ratio);
    let mut out_l = Vec::with_capacity(out_frames);
    let mut out_r = Vec::with_capacity(out_frames);
    while out_l.len() < out_frames {
        let (l, r) = wsola.next_resampled(left, right, false, 1.0);
        // WSOLA:n är slut och har inget mer att ge: avbryt i stället för att snurra. Ett
        // avbrott här syns i valideringen efteråt, och en för kort fil används aldrig.
        if wsola.available() <= 0.0 && wsola.is_finished() {
            break;
        }
        out_l.push(l);
        out_r.push(r);
    }
    (out_l, out_r)
}

/// Prövar en renderad sträckning **innan** den får användas.
///
/// Det här är 8.5-regeln i siffror: en fil som är för kort, innehåller NaN/Inf eller är tyst
/// ska avvisas med ett skäl i klartext — aldrig spelas. En trasig sträckning som spelas är
/// exakt "en möjlig spricka i rustningen".
pub fn check_rendered(left: &[f32], right: &[f32], expected_frames: usize) -> Result<(), String> {
    if left.is_empty() || right.is_empty() {
        return Err(crate::i18n::t("sträckningen blev tom").to_string());
    }
    if left.len() != right.len() {
        return Err(crate::tstatus!(
            "kanalerna har olika längd ({} mot {})",
            left.len(),
            right.len()
        ));
    }
    // Rundningen i `plan` ger som mest en frame fel; två frames är marginal för WSOLA:ns sista
    // korn. Mer än så betyder att något gick fel på vägen.
    let diff = left.len().abs_diff(expected_frames);
    if diff > 2 {
        return Err(crate::tstatus!(
            "fel längd: {} frames i stället för {}",
            left.len(),
            expected_frames
        ));
    }
    if !left.iter().chain(right.iter()).all(|s| s.is_finite()) {
        return Err(crate::i18n::t("sträckningen innehåller ogiltiga sampel (NaN)").to_string());
    }
    let peak = left
        .iter()
        .chain(right.iter())
        .fold(0.0f32, |m, s| m.max(s.abs()));
    if peak <= 1e-6 {
        return Err(crate::i18n::t("sträckningen blev tyst").to_string());
    }
    Ok(())
}

/// Filnamnet för en sträckning: källans namn **och båda tempon**.
///
/// Båda tempon måste med, annars kan en gammal cache från ett annat tempo läsas som giltig —
/// samma sorts föråldrade sammanfattning som `visual_peaks_from`-fällan.
pub fn cache_key(source: &str, source_bpm: f32, project_bpm: f32) -> String {
    let stem = Path::new(source)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "klipp".to_string());
    format!(
        "{}-{:.2}-till-{:.2}",
        crate::autosave::slug(&stem),
        source_bpm,
        project_bpm
    )
}

/// Sträcker, prövar och skriver en sträckt version av ett klipp. Lämnar sökvägen.
///
/// Skrivningen är **atomisk**: först en tempfil, sedan `rename`. En avbruten skrivning lämnar
/// aldrig en fil som ser ut som en giltig cache.
pub fn render_to_file(
    dir: &Path,
    key: &str,
    left: &[f32],
    right: &[f32],
    ratio: f32,
    sample_rate: f32,
) -> Result<String, String> {
    let frames = left.len().min(right.len());
    let out_frames = ((frames as f64) * (ratio.clamp(MIN_RATIO, MAX_RATIO) as f64))
        .round()
        .max(1.0) as usize;
    let (out_l, out_r) = stretch_stereo(left, right, ratio, sample_rate);
    check_rendered(&out_l, &out_r, out_frames)?;

    std::fs::create_dir_all(dir)
        .map_err(|e| crate::tstatus!("kunde inte skapa '{}': {}", dir.display(), e))?;
    let final_path: PathBuf = dir.join(format!("{}.wav", crate::autosave::slug(key)));
    let temp_path = dir.join(format!("{}.wav.tmp", crate::autosave::slug(key)));
    let temp_str = temp_path.to_string_lossy().to_string();
    super::exporter::write_stem_wav(&temp_str, &out_l, &out_r, sample_rate as u32)?;
    std::fs::rename(&temp_path, &final_path).map_err(|e| {
        let _ = std::fs::remove_file(&temp_path);
        crate::tstatus!(
            "kunde inte flytta den färdiga sträckningen till '{}': {}",
            final_path.display(),
            e
        )
    })?;
    Ok(final_path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f32 = 44_100.0;

    fn tone(freq: f32, secs: f32, amp: f32) -> Vec<f32> {
        let n = (SR * secs) as usize;
        (0..n)
            .map(|i| (std::f32::consts::TAU * freq * i as f32 / SR).sin() * amp)
            .collect()
    }

    /// Energin vid en frekvens (Goertzel) — används för att bevisa att tonhöjden står still.
    fn energy(buf: &[f32], freq: f32) -> f32 {
        let w = std::f32::consts::TAU * freq / SR;
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in buf {
            let s0 = x + 2.0 * w.cos() * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        ((s1 * s1 + s2 * s2 - 2.0 * w.cos() * s1 * s2).max(0.0)).sqrt() / buf.len() as f32
    }

    #[test]
    fn the_plan_is_none_when_nothing_should_change() {
        // Okänt inspelningstempo (8.10:s regel): rör inte ljudet.
        assert_eq!(plan(0.0, 128.0, 44_100), None);
        // Samma tempo: bit-exakt väg, ingen fil behövs.
        assert_eq!(plan(120.0, 120.0, 44_100), None);
        // Tomt klipp.
        assert_eq!(plan(120.0, 140.0, 0), None);
        // Och ett projekt utan tempo är inget att sträcka mot.
        assert_eq!(plan(120.0, 0.0, 44_100), None);
    }

    #[test]
    fn the_plan_is_the_tempo_ratio_and_the_frame_count_follows() {
        let p = plan(120.0, 150.0, 44_100).expect("150 mot 120 ska sträckas");
        assert!((p.ratio - 1.25).abs() < 1e-6, "faktorn: {}", p.ratio);
        assert_eq!(p.out_frames, 55_125, "1,25 × 44 100 frames");
        assert!(p.changes_anything());

        let down = plan(150.0, 120.0, 44_100).expect("120 mot 150 ska sträckas ned");
        assert!((down.ratio - 0.8).abs() < 1e-6);
        assert_eq!(down.out_frames, 35_280);
    }

    #[test]
    fn the_plan_clamps_extreme_tempos_instead_of_producing_a_monster() {
        let fast = plan(120.0, 10_000.0, 1_000).expect("en plan finns");
        assert!((fast.ratio - MAX_RATIO).abs() < 1e-6);
        let slow = plan(120.0, 1.0, 1_000).expect("en plan finns");
        assert!((slow.ratio - MIN_RATIO).abs() < 1e-6);
    }

    /// **Acceptanskriterium 1:** vid faktor 1,0 händer ingenting alls — bit-exakt.
    #[test]
    fn a_ratio_of_one_returns_the_input_untouched() {
        let l = tone(220.0, 0.2, 0.7);
        let r: Vec<f32> = l.iter().map(|s| s * 0.5).collect();
        let (out_l, out_r) = stretch_stereo(&l, &r, 1.0, SR);
        assert_eq!(out_l, l, "vänster ska vara oförändrad");
        assert_eq!(out_r, r, "höger ska vara oförändrad");
    }

    /// **Acceptanskriterium 2:** tonhöjden står still när tempot ändras, och längden följer
    /// faktorn. Mätt med Goertzel vid grundtonen — och vid den frekvens en smurf hade hamnat på.
    #[test]
    fn the_pitch_stays_put_while_the_length_changes() {
        let l = tone(220.0, 0.5, 0.7);
        let r = l.clone();
        let (out_l, _out_r) = stretch_stereo(&l, &r, 1.5, SR);

        let expected = (l.len() as f64 * 1.5).round() as usize;
        assert!(
            out_l.len().abs_diff(expected) <= 2,
            "längden ska följa faktorn: {} mot {}",
            out_l.len(),
            expected
        );

        let mid = &out_l[out_l.len() / 4..out_l.len() * 3 / 4];
        let at_pitch = energy(mid, 220.0);
        let at_smurf = energy(mid, 220.0 * 1.5);
        assert!(
            at_pitch > at_smurf * 4.0,
            "grundtonen ska dominera (220 Hz: {at_pitch:.5} mot smurfen 330 Hz: {at_smurf:.5})"
        );
    }

    /// Och nedåt: en sänkning får inte heller flytta tonhöjden.
    #[test]
    fn slowing_down_does_not_deepen_the_voice() {
        let l = tone(300.0, 0.5, 0.6);
        let (out_l, _) = stretch_stereo(&l, &l.clone(), 0.75, SR);
        let expected = (l.len() as f64 * 0.75).round() as usize;
        assert!(
            out_l.len().abs_diff(expected) <= 2,
            "längden: {}",
            out_l.len()
        );
        let mid = &out_l[out_l.len() / 4..out_l.len() * 3 / 4];
        assert!(
            energy(mid, 300.0) > energy(mid, 300.0 * 0.75) * 4.0,
            "grundtonen ska stå kvar"
        );
    }

    /// **Stereobilden:** körs båda kanalerna genom samma grainsökning ska förhållandet mellan
    /// dem stå kvar. Här är höger exakt hälften av vänster, och det ska den vara efteråt också.
    #[test]
    fn both_channels_go_through_the_same_grains_so_the_stereo_image_survives() {
        let l = tone(220.0, 0.4, 0.8);
        let r: Vec<f32> = l.iter().map(|s| s * 0.5).collect();
        let (out_l, out_r) = stretch_stereo(&l, &r, 1.25, SR);

        let mut worst = 0.0f32;
        for (a, b) in out_l.iter().zip(out_r.iter()) {
            if a.abs() > 0.05 {
                worst = worst.max((b / a - 0.5).abs());
            }
        }
        assert!(
            worst < 0.05,
            "kanalerna ska hänga ihop, största avvikelse {worst}"
        );
    }

    /// **Acceptanskriterium 3:** en trasig sträckning avvisas — tom, tyst, NaN eller fel längd.
    #[test]
    fn a_broken_stretch_is_refused_and_never_played() {
        let good = tone(220.0, 0.1, 0.5);
        assert!(check_rendered(&good, &good, good.len()).is_ok());

        let empty: Vec<f32> = Vec::new();
        assert!(
            check_rendered(&empty, &empty, 0).is_err(),
            "tom ska avvisas"
        );

        let silent = vec![0.0f32; 4_410];
        assert!(
            check_rendered(&silent, &silent, 4_410).is_err(),
            "tyst ska avvisas"
        );

        let mut nan = good.clone();
        nan[10] = f32::NAN;
        assert!(
            check_rendered(&nan, &good, good.len()).is_err(),
            "NaN ska avvisas"
        );

        assert!(
            check_rendered(&good, &good, good.len() * 2).is_err(),
            "fel längd ska avvisas"
        );
        assert!(
            check_rendered(&good, &good[..good.len() - 1], good.len()).is_err(),
            "olika långa kanaler ska avvisas"
        );
    }

    /// Hela vägen: räkna, pröva, skriv — och läs tillbaka med appens egen avkodare.
    #[test]
    fn a_rendered_stretch_lands_on_disk_and_reads_back() {
        let dir = std::env::temp_dir().join(format!("sonix_stretch_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let l = tone(220.0, 0.3, 0.6);
        let r: Vec<f32> = l.iter().map(|s| -s * 0.8).collect();
        let key = cache_key("/tmp/Min Låt (Suno).mp3", 120.0, 150.0);
        let path = render_to_file(&dir, &key, &l, &r, 1.25, SR).expect("skrivningen ska lyckas");

        assert!(Path::new(&path).is_file(), "filen ska finnas: {path}");
        let (read_l, read_r, sr) = crate::audio::wav_reader::load_wav_pcm(&path)
            .expect("den skrivna filen ska gå att läsa");
        assert_eq!(sr, 44_100);
        let expected = (l.len() as f64 * 1.25).round() as usize;
        assert!(
            read_l.len().abs_diff(expected) <= 2,
            "längden i filen: {} mot {}",
            read_l.len(),
            expected
        );
        assert!(read_l.iter().all(|s| s.is_finite()));
        assert!(
            read_l.iter().fold(0.0f32, |m, s| m.max(s.abs())) > 0.05,
            "och den ska ha ljud"
        );
        // Osymmetrin mellan kanalerna ska bevaras (höger är inverterad och lägre).
        let mid = read_l.len() / 2;
        assert!(
            read_l[mid] * read_r[mid] <= 0.0,
            "kanalerna ska inte blandas ihop"
        );

        // Namnet innehåller BÅDA tempon, så en cache från ett annat tempo inte kan läsas.
        assert!(
            path.contains("120") && path.contains("150"),
            "nyckeln: {path}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// En plan på ett tomt klipp får inte skriva en tom fil.
    #[test]
    fn an_empty_clip_never_produces_a_file() {
        let dir = std::env::temp_dir().join(format!("sonix_stretch_empty_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let err = render_to_file(&dir, "tomt", &[], &[], 1.5, SR).expect_err("ska avvisas");
        assert!(!err.is_empty(), "felet ska förklara sig: {err}");
        assert!(!dir.exists(), "ingen katalog ska ha skapats i onödan");
    }

    /// **Policyn**, i en tabell: vad som händer med ett klipp i varje läge.
    #[test]
    fn the_policy_never_surprises_the_user() {
        let untouched = FollowDecision {
            mode: FollowMode::Untouched,
            ratio: 1.0,
        };

        // 1. Okänt inspelningstempo (8.10) — även med switchen på.
        assert_eq!(decide(0.0, 160.0, true, false), untouched);
        // 2. Switchen av: ingen följning alls, varken sträckning eller bandspelare.
        assert_eq!(decide(120.0, 160.0, false, false), untouched);
        assert_eq!(decide(120.0, 160.0, false, true), untouched);
        // 3. Klippet vill ha bandspelaren: tonhöjden följer, och det är ett aktivt val.
        let tape = decide(120.0, 150.0, true, true);
        assert_eq!(tape.mode, FollowMode::Tape);
        assert!((tape.ratio - 1.25).abs() < 1e-6);
        // 4. Samma tempo: ingenting att göra.
        assert_eq!(decide(128.0, 128.0, true, false), untouched);
        // 5. Standardvägen: sträck, bevara tonhöjden.
        let stretch = decide(120.0, 150.0, true, false);
        assert_eq!(stretch.mode, FollowMode::Stretch);
        assert!((stretch.ratio - 1.25).abs() < 1e-6);
        // Och nedåt lika självklart.
        let down = decide(150.0, 120.0, true, false);
        assert_eq!(down.mode, FollowMode::Stretch);
        assert!((down.ratio - 0.8).abs() < 1e-6);
    }

    /// En extrem tempodiff får inte ge en orimlig faktor, och inte heller en tyst väg runt
    /// klämningen: samma gränser som sångstudiens reglage gäller.
    #[test]
    fn the_policy_clamps_like_the_vocal_studio_does() {
        assert!((decide(120.0, 10_000.0, true, false).ratio - MAX_RATIO).abs() < 1e-6);
        assert!((decide(120.0, 1.0, true, false).ratio - MIN_RATIO).abs() < 1e-6);
    }
}
