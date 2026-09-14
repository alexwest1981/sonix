//! Makrokedjan (Fas 8.9): en sparad sekvens av kommandon som körs på filer.
//!
//! **Vad en makrokedja är.** Audacitys *Macros* kör en sekvens förkonfigurerade kommandon
//! automatiskt — på ett projekt eller i **batch över filer** — och nyttan är densamma här:
//! samma behandling på tjugo tagningar utan tjugo handgrepp. Vår form är filvägen: kedjan
//! läser en ljudfil, kör sina steg i ordning och skriver resultatet. Utan kedja vore
//! arbetsflödet "importera stämman, normalisera, exportera" upprepat för hand.
//!
//! **Tyst-ljud-doktrinen gäller dubbelt här** (`references/silent-audio-doctrine.md` i
//! `sonix`-skillen): den här vägen *skriver* ljudfiler, och en fil som ser ut som ett resultat
//! utan att vara det är värre än ett fel. Tre regler följer, och de är byggda, inte lovade:
//! 1. En fil som **inte går att läsa** rapporteras och avbryts. Ingen utdata skrivs för den.
//! 2. Utdata **läses tillbaka** efter skrivningen. En batch som skriver tomma filer får inte se
//!    ut som ett lyckat pass.
//! 3. En kedja som gjorde filen **tyst** är ett fel, inte ett resultat: hade indata signal och
//!    utdata ingen, rapporteras det i stället för att skrivas.
//!
//! **Stegen är maskinerna som redan finns.** `Normalize` går genom `loudness.rs` (Fas 5.5) och
//! `Export` genom `exporter.rs` — samma vägar som appens export använder. Den här modulen
//! lägger ingen egen ljudbehandling till; den kedjar ihop dem och skriver ner vad som hände.
//!
//! Status: byggs — modellen, den rena körningen och batch-vägen (8.9 steg 1).
//! Rör inte: en andra ljudväg. Stegen ska anropa de befintliga maskinerna, aldrig kopiera dem.

use crate::audio::exporter::{DitherSettings, ExportFormat, ExportMeta};
use crate::audio::loudness::{integrated_lufs, normalize_loudness, true_peak_db};
use std::path::{Path, PathBuf};

/// Katalognamnet en batch skriver i när användaren inte pekat ut en egen (Audacitys konvention).
pub const DEFAULT_OUT_DIR: &str = "macro-output";

/// Namnet på loggfilen en batch lämnar efter sig.
pub const LOG_FILE: &str = "macro-log.txt";

fn default_true() -> bool {

    true
}

/// **Ett steg i en makrokedja.** Filformen är `{"command": "…", …}` — engelska fältnamn, som
/// resten av projekt- och konfigurationsfilerna.
///
/// Stegen gjordes medvetet **få och hela**: varje steg motsvarar en maskin som finns och är
/// mätt (loudness-normaliseringen, exporteraren). Ett steg som "tar bort brus" hade varit en
/// ny ljudväg, och den sortens väg hörs som en annan låt — se modulhuvudet.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum MacroStep {
    /// Normalisera till ett loudness-mål, med ett tak för den sanna toppen.
    Normalize {
        target_lufs: f32,
        ceiling_dbtp: f32,
    },
    /// Trimma tystnad i början och slutet.
    ///
    /// Mätt på Alex' egna Suno-stämmor: en tagning kunde vara **noll till 5,0 s** och första
    /// frasen börja vid 7,155 s. Att trimma bort det är inte kosmetika — det är skillnaden
    /// mellan en fil som börjar när musiken börjar och en som börjar med fyra tysta sekunder.
    TrimSilence {
        threshold_db: f32,
        /// Hur mycket tystnad som sparas kvar vid varje kant, i millisekunder. Noll vore att
        /// klippa mitt i andningen före första tonen.
        keep_ms: f32,
    },
    /// Skriv filen i ett format.
    Export {
        format: ExportFormat,
        /// Läggs på filnamnet, så en kedja kan köras utan att skriva över sitt eget indata.
        #[serde(default)]
        suffix: String,
        #[serde(default = "default_true")]
        dither: bool,
        #[serde(default)]
        noise_shaping: bool,
    },
}

impl MacroStep {
    /// Stegets namn, så som det står i en logg eller i en lista.
    pub fn label(&self) -> &'static str {
        match self {
            MacroStep::Normalize { .. } => "Normalisera",
            MacroStep::TrimSilence { .. } => "Trimma tystnad",
            MacroStep::Export { .. } => "Exportera",
        }
    }

    /// Är det här steget som **skriver**? Bara exporten rör filsystemet.
    pub fn writes_a_file(&self) -> bool {
        matches!(self, MacroStep::Export { .. })
    }
}

/// En hel kedja: ett namn och en ordnad lista steg.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MacroChain {
    pub name: String,
    #[serde(default)]
    pub steps: Vec<MacroStep>,
}

impl MacroChain {
    /// En ny, tom kedja med ett namn.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            steps: Vec::new(),
        }
    }

    /// **Har kedjan något som helst att skriva?** En kedja utan export steg är inte farlig —
    /// den gör bara ingenting som syns. Anroparen får säga det i stället för att köra tyst.
    pub fn has_export(&self) -> bool {
        self.steps.iter().any(MacroStep::writes_a_file)
    }
}

/// Vad **ett** steg gjorde, i mätbara ord. Loggen är det användaren får se, så den får inte
/// säga "klart" utan ett tal.
#[derive(Clone, Debug, PartialEq)]
pub struct StepReport {
    pub step: String,
    pub detail: String,
}

/// Resultatet för **en** fil.
///
/// `error` satt betyder att filen inte blev färdig, och att **ingen utdata skrevs** för den.
/// En fil som faller stoppar inte de andra i en batch (Audacity går vidare), men räknas som fel
/// i sammanfattningen så körningen kan lämna en felkod.
#[derive(Clone, Debug)]
pub struct FileRun {
    pub input: String,
    pub outputs: Vec<String>,
    pub reports: Vec<StepReport>,
    pub error: Option<String>,
}

impl FileRun {
    /// En körning som stannade före något skrevs.
    fn aborted(input: &str, error: String) -> Self {
        Self {
            input: input.to_string(),
            outputs: Vec::new(),
            reports: Vec::new(),
            error: Some(error),
        }
    }

    /// Gick filen hela vägen genom kedjan?
    pub fn succeeded(&self) -> bool {
        self.error.is_none() && !self.outputs.is_empty()
    }
}

/// **Tystnaden i kanterna, mätt.** Returnerar `(första, sista)` bildrutan som ligger över
/// tröskeln, med `keep_ms` marginal kvar vid varje kant — eller `None` när hela filen ligger
/// under den.
///
/// Ren funktion: ingen fil, inget fönster. `None` är ett svar i sig (allt är tyst) och får
/// **inte** bli "behåll allt": anroparen säger det högt i stället.
pub fn signal_span_frames(
    samples_lr: &[f32],
    sample_rate: u32,
    threshold_db: f32,
    keep_ms: f32,
) -> Option<(usize, usize)> {
    let frames = samples_lr.len() / 2;
    if frames == 0 || sample_rate == 0 {
        return None;
    }
    let threshold = 10f32.powf(threshold_db / 20.0);
    let over = |f: usize| samples_lr[f * 2].abs() > threshold || samples_lr[f * 2 + 1].abs() > threshold;

    let first = (0..frames).find(|&f| over(f))?;
    let last = (0..frames).rev().find(|&f| over(f))?;

    let keep = ((keep_ms.max(0.0) / 1000.0) * sample_rate as f32).round() as usize;
    let start = first.saturating_sub(keep);
    let end = (last + keep).min(frames - 1);
    Some((start, end))
}

/// Klipper ut `start..=end` (bildrutor) ur en interleaved stereobuffert.
pub fn trim_to_span(samples_lr: &[f32], start: usize, end: usize) -> Vec<f32> {
    let frames = samples_lr.len() / 2;
    if frames == 0 || start > end || end >= frames {
        return samples_lr.to_vec();
    }
    samples_lr[start * 2..=(end * 2 + 1)].to_vec()
}

/// **Utdata-namnet — en ren funktion, för det är här en batch kan skriva över fel fil.**
///
/// `<stam><suffix>.<ändelse>`. Blir namnet **identiskt med indatafilens** läggs `_1`, `_2` …
/// på tills det skiljer sig: att köra en kedja utan suffix ska inte äta sin egen indatafil.
pub fn output_file_name(input: &Path, suffix: &str, format: ExportFormat) -> String {
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "utdata".to_string());
    let ext = format.ext();
    let mut name = format!("{stem}{suffix}.{ext}");
    let input_name = input
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let mut n = 1usize;
    while name == input_name {
        name = format!("{stem}{suffix}_{n}.{ext}");
        n += 1;
    }
    name
}

/// Utmappen för en indatafil: den angivna, annars indatafilens egen mapp + [`DEFAULT_OUT_DIR`].
pub fn output_dir_for(input: &Path, out_dir: Option<&Path>) -> PathBuf {
    match out_dir {
        Some(d) => d.to_path_buf(),
        None => input
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(DEFAULT_OUT_DIR),
    }
}

/// Vilka filer i en mapp som är ljudfiler vi kan läsa. Sorterad, så en körning blir
/// reproducerbar — och utan att följa mappar (en batch ska inte råka gå genom hela disken).
pub fn audio_files_in(dir: &Path) -> Result<Vec<String>, String> {
    const EXTS: [&str; 8] = ["wav", "wave", "flac", "mp3", "ogg", "oga", "m4a", "aac"];
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {}", crate::i18n::t("Kunde inte läsa mappen"), e))?;
    let mut files: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if EXTS.contains(&ext.as_str()) {
            files.push(path.to_string_lossy().to_string());
        }
    }
    files.sort();
    Ok(files)
}

/// **Kör en kedja på en fil.** Det här är den enda vägen som rör filsystemet.
///
/// Ordningen är doktrinen: läs (avbryt vid fel) → stegen i tur och ordning → skriv → **läs
/// tillbaka**. Ett fel någonstans ger en `FileRun` med `error` satt och inga `outputs`.
pub fn run_on_file(
    chain: &MacroChain,
    input: &str,
    out_dir: Option<&Path>,
    meta: &ExportMeta,
) -> FileRun {
    let (left, right, sample_rate) = match crate::audio::load_audio_pcm(input) {
        Ok(t) => t,
        Err(e) => {
            // Doktrinen: rapportera och avbryt. Ingen tom fil skrivs "för säkerhets skull".
            return FileRun::aborted(
                input,
                format!("{} — {}: {}", input, crate::i18n::t("kan inte läsas"), e),
            );
        }
    };

    let mut buf: Vec<f32> = Vec::with_capacity(left.len() * 2);
    for (l, r) in left.iter().zip(right.iter()) {
        buf.push(*l);
        buf.push(*r);
    }
    if buf.is_empty() {
        return FileRun::aborted(
            input,
            format!("{} — {}", input, crate::i18n::t("filen är tom (inget ljud)")),
        );
    }
    let input_peak = true_peak_db(&buf);
    let input_path = Path::new(input);

    let mut run = FileRun {
        input: input.to_string(),
        outputs: Vec::new(),
        reports: Vec::new(),
        error: None,
    };

    for step in &chain.steps {
        match step {
            MacroStep::Normalize {
                target_lufs,
                ceiling_dbtp,
            } => {
                let before = true_peak_db(&buf);
                let measured = normalize_loudness(&mut buf, sample_rate, *target_lufs, *ceiling_dbtp);
                let after_lufs = integrated_lufs(&buf, sample_rate);
                let after = true_peak_db(&buf);
                // **Målet och taket är två löften, och taket vinner.** Utan tak hade en stämma
                // kunnat tvingas till målet och klippt. Att bara skriva "mål −14" när filen
                // hamnade på −17,6 vore halva sanningen — mätt på Alex' egen Suno-stämma
                // (Broken/Vocals: 11,76 s tystnad in, topp −2,8 dBTP) band taket vid −1 och
                // loudnessen stannade 3,6 LU under målet. Därför mäts resultatet efteråt, och
                // avvikelsen sägs högt.
                let short = after_lufs < target_lufs - 0.1;
                run.reports.push(StepReport {
                    step: step.label().to_string(),
                    detail: if short {
                        format!(
                            "{measured:.1} → {after_lufs:.1} LUFS (mål {target_lufs:.1}, {}), topp {before:.1} → {after:.1} dBTP",
                            crate::i18n::t("taket satte gränsen")
                        )
                    } else {
                        format!(
                            "{measured:.1} → {after_lufs:.1} LUFS (mål {target_lufs:.1}), topp {before:.1} → {after:.1} dBTP"
                        )
                    },
                });
            }
            MacroStep::TrimSilence {
                threshold_db,
                keep_ms,
            } => {
                let frames = buf.len() / 2;
                let Some((start, end)) = signal_span_frames(&buf, sample_rate, *threshold_db, *keep_ms)
                else {
                    return FileRun::aborted(
                        input,
                        format!(
                            "{} — {} ({threshold_db:.0} dB): {}",
                            input,
                            crate::i18n::t("hela filen ligger under tröskeln"),
                            crate::i18n::t("inget skrevs")
                        ),
                    );
                };
                let removed_start = start as f32 / sample_rate as f32;
                let removed_end = (frames - 1 - end) as f32 / sample_rate as f32;
                buf = trim_to_span(&buf, start, end);
                run.reports.push(StepReport {
                    step: step.label().to_string(),
                    detail: format!(
                        "-{removed_start:.3} s i början, -{removed_end:.3} s i slutet ({:.3} s kvar)",
                        buf.len() as f32 / 2.0 / sample_rate as f32
                    ),
                });
            }
            MacroStep::Export {
                format,
                suffix,
                dither,
                noise_shaping,
            } => {
                let dir = output_dir_for(input_path, out_dir);
                // **Utmappen är vår att bygga.** Batchen skriver i `macro-output` som standard,
                // och den mappen finns inte förrän första körningen. `write_wav` skapar den
                // inte själv — utan det här steget faller varje första körning på
                // "No such file or directory".
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    return FileRun::aborted(
                        input,
                        format!(
                            "{} — {} {}: {e}",
                            input,
                            crate::i18n::t("kunde inte skapa utmappen"),
                            dir.display()
                        ),
                    );
                }
                let name = output_file_name(input_path, suffix, *format);
                let path = dir.join(name);
                // Ett steg som gör filen tyst är ett fel, inte ett resultat.
                let out_peak = true_peak_db(&buf);
                if input_peak.is_finite() && out_peak < -120.0 {
                    return FileRun::aborted(
                        input,
                        format!(
                            "{} — {}",
                            input,
                            crate::i18n::t("kedjan gjorde filen tyst; inget skrevs")
                        ),
                    );
                }
                let settings = DitherSettings {
                    enabled: *dither,
                    noise_shaping: *noise_shaping,
                    seed: crate::audio::dither::DEFAULT_SEED,
                };
                let path_str = path.to_string_lossy().to_string();
                if let Err(e) = crate::audio::write_export_with(
                    &path_str,
                    *format,
                    &buf,
                    sample_rate,
                    meta,
                    settings,
                ) {
                    return FileRun::aborted(
                        input,
                        format!(
                            "{} — {} {}: {e}",
                            input,
                            crate::i18n::t("kunde inte skriva"),
                            path_str
                        ),
                    );
                }
                // **Läs tillbaka.** En skrivning som "lyckades" men lämnade en tom eller tyst
                // fil är precis det ett batch-pass inte får se ut som.
                match crate::audio::load_audio_pcm(&path_str) {
                    Ok((l, r, _)) => {
                        let frames = l.len().min(r.len());
                        if frames == 0 {
                            return FileRun::aborted(
                                input,
                                format!(
                                    "{} — {} {path_str}",
                                    input,
                                    crate::i18n::t("utdata är tom; filen skrevs inte korrekt")
                                ),
                            );
                        }
                        let back: Vec<f32> = l
                            .iter()
                            .zip(r.iter())
                            .flat_map(|(l, r)| [*l, *r])
                            .collect();
                        let back_peak = true_peak_db(&back);
                        if back_peak < -120.0 {
                            return FileRun::aborted(
                                input,
                                format!(
                                    "{} — {} {path_str}",
                                    input,
                                    crate::i18n::t("utdata är tyst; filen skrevs inte korrekt")
                                ),
                            );
                        }
                        run.reports.push(StepReport {
                            step: step.label().to_string(),
                            detail: format!(
                                "{path_str} ({} sampel, topp {back_peak:.1} dBTP, {} läser tillbaka)",
                                back.len(),
                                crate::i18n::t("och den")
                            ),
                        });
                    }
                    Err(e) => {
                        return FileRun::aborted(
                            input,
                            format!(
                                "{} — {} {path_str}: {e}",
                                input,
                                crate::i18n::t("utdata går inte att läsa tillbaka")
                            ),
                        );
                    }
                }
                run.outputs.push(path_str);
            }
        }
    }

    run
}

/// **Batch över filer.** En fil som faller stoppar inte de andra — men den får ett fel i sin
/// rad, och sammanfattningen räknar den.
pub fn run_batch(
    chain: &MacroChain,
    inputs: &[String],
    out_dir: Option<&Path>,
    meta: &ExportMeta,
) -> Vec<FileRun> {
    inputs
        .iter()
        .map(|input| run_on_file(chain, input, out_dir, meta))
        .collect()
}

/// Hur många filer som gick hela vägen.
pub fn succeeded(runs: &[FileRun]) -> usize {
    runs.iter().filter(|r| r.succeeded()).count()
}

/// Loggen som text — det som gör en körning på hundra filer granskningsbar i efterhand.
pub fn batch_log(chain: &MacroChain, runs: &[FileRun]) -> String {
    let mut out = String::new();
    out.push_str(&format!("Makro: {}\n", chain.name));
    out.push_str(&format!("Steg: {}\n", chain.steps.len()));
    for step in &chain.steps {
        out.push_str(&format!("  - {}\n", step.label()));
    }
    out.push_str(&format!(
        "Filer: {} varav {} klara, {} med fel\n",
        runs.len(),
        succeeded(runs),
        runs.len() - succeeded(runs)
    ));
    for run in runs {
        out.push_str(&format!("\n{}\n", run.input));
        for r in &run.reports {
            out.push_str(&format!("  {}: {}\n", r.step, r.detail));
        }
        if let Some(e) = &run.error {
            out.push_str(&format!("  FEL: {e}\n"));
        }
    }
    out
}

/// Skriver loggen i utmappen (`macro-output` som standard). Temp + rename, som resten av appen.
pub fn write_batch_log(
    chain: &MacroChain,
    runs: &[FileRun],
    first_input: &Path,
    out_dir: Option<&Path>,
) -> Result<String, String> {
    let dir = output_dir_for(first_input, out_dir);
    let path = dir.join(LOG_FILE);
    crate::autosave::write_atomic(&path, batch_log(chain, runs).as_bytes())
        .map_err(|e| format!("{} {}: {e}", crate::i18n::t("kunde inte skriva"), path.display()))?;
    Ok(path.to_string_lossy().to_string())
}

/// **En färdig startkedja.** Audacity levererar exempel-makron av samma skäl: den som inte
/// har en kedja än ska inte behöva kunna filformen utantill för att komma igång. Det här är
/// arbetsflödet för en Suno-stämma — trimma tystnaden, normalisera, skriv FLAC 24 bitar —
/// och varje tal går att ändra i filen efteråt.
pub fn example_chain() -> MacroChain {
    let mut chain = MacroChain::new("Suno-stämma");
    chain.steps = vec![
        MacroStep::TrimSilence {
            threshold_db: -55.0,
            keep_ms: 60.0,
        },
        MacroStep::Normalize {
            target_lufs: -14.0,
            ceiling_dbtp: -1.0,
        },
        MacroStep::Export {
            format: ExportFormat::Flac,
            suffix: "_master".to_string(),
            dither: true,
            noise_shaping: false,
        },
    ];
    chain
}

/// Läser en kedja från en JSON-fil. Felet namnger filen och skälet.
pub fn load_chain(path: &str) -> Result<MacroChain, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("{} {path}: {e}", crate::i18n::t("kunde inte läsa")))?;
    serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))
}

/// Skriver en kedja till en JSON-fil (temp + rename).
pub fn save_chain(path: &Path, chain: &MacroChain) -> Result<(), String> {
    let json = serde_json::to_string_pretty(chain)
        .map_err(|e| format!("{}: {e}", crate::i18n::t("kunde inte serialisera kedjan")))?;
    crate::autosave::write_atomic(path, json.as_bytes())
        .map_err(|e| format!("{} {}: {e}", crate::i18n::t("kunde inte skriva"), path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// En buffert med en ton i mitten och tystnad runt om: `(lead_in, tone, tail)` i sekunder.
    fn tone_with_silence(sample_rate: u32, lead_in: f32, tone: f32, tail: f32) -> Vec<f32> {
        let frames = ((lead_in + tone + tail) * sample_rate as f32) as usize;
        let mut buf = vec![0.0f32; frames * 2];
        let from = (lead_in * sample_rate as f32) as usize;
        let to = from + (tone * sample_rate as f32) as usize;
        for f in from..to.min(frames) {
            let phase = 2.0 * std::f32::consts::PI * 220.0 * (f - from) as f32 / sample_rate as f32;
            buf[f * 2] = phase.sin() * 0.5;
            buf[f * 2 + 1] = phase.sin() * 0.5;
        }
        buf
    }

    #[test]
    fn the_span_keeps_the_margin_around_the_music() {
        let sr = 48_000;
        let buf = tone_with_silence(sr, 2.0, 1.0, 3.0);
        let (start, end) = signal_span_frames(&buf, sr, -60.0, 100.0).expect("tonen ska hittas");
        // 2 s in, med 100 ms marginal: 1,9 s.
        assert!(
            (start as f32 / sr as f32 - 1.9).abs() < 0.01,
            "start blev {} s",
            start as f32 / sr as f32
        );
        // Slutet: tonen tar slut vid 2,0 + 1,0 = 3,0 s, plus 100 ms marginal.
        assert!(
            (end as f32 / sr as f32 - 3.1).abs() < 0.01,
            "slut blev {} s",
            end as f32 / sr as f32
        );
    }

    /// **Ett svar som inte får bli "behåll allt".** En fil utan signal över tröskeln ska ge
    /// `None` — anroparen rapporterar det, och skriver ingenting.
    #[test]
    fn a_silent_file_has_no_span_at_all() {
        let sr = 44_100;
        let tyst = vec![0.0f32; sr as usize * 2];
        assert_eq!(signal_span_frames(&tyst, sr, -60.0, 100.0), None);
        assert_eq!(signal_span_frames(&[], sr, -60.0, 100.0), None);
        assert_eq!(signal_span_frames(&tyst, 0, -60.0, 100.0), None);
        // Svag signal under tröskeln är också "ingen signal" — och samma buffert hittas när
        // tröskeln flyttas under den, så det är tröskeln som avgör och inte en slump.
        let svag = vec![0.001f32; sr as usize * 2];
        assert_eq!(signal_span_frames(&svag, sr, -20.0, 100.0), None);
        assert!(signal_span_frames(&svag, sr, -61.0, 0.0).is_some());
    }

    #[test]
    fn trimming_keeps_the_left_and_right_channels_together() {
        let buf = vec![0.0, 0.0, 1.0, -1.0, 2.0, -2.0, 0.0, 0.0];
        let cut = trim_to_span(&buf, 1, 2);
        assert_eq!(cut, vec![1.0, -1.0, 2.0, -2.0]);
        // Skräpindex lämnar bufferten orörd i stället för att panika.
        assert_eq!(trim_to_span(&buf, 3, 1), buf);
        assert_eq!(trim_to_span(&buf, 0, 99), buf);
    }

    /// **Namnet får inte äta sin egen indatafil.** Utan suffix blir namnet identiskt med
    /// indata — då läggs ett nummer på i stället.
    #[test]
    fn an_output_name_never_collides_with_its_own_input() {
        let input = Path::new("/tmp/Stem_01.wav");
        assert_eq!(
            output_file_name(input, "_master", ExportFormat::Wav24),
            "Stem_01_master.wav"
        );
        assert_eq!(
            output_file_name(input, "", ExportFormat::Wav24),
            "Stem_01_1.wav",
            "identiskt namn ska flyttas undan"
        );
        // Samma stam men annan ändelse krockar inte, och ska inte heller numreras.
        assert_eq!(output_file_name(input, "", ExportFormat::Flac), "Stem_01.flac");
        assert_eq!(
            output_file_name(Path::new("/tmp/Stem_01.mp3"), "", ExportFormat::Mp3),
            "Stem_01_1.mp3"
        );
    }

    #[test]
    fn the_output_dir_defaults_to_macro_output_beside_the_input() {
        let input = Path::new("/home/alex/Music/Sonix/Stems/Vocal.wav");
        assert_eq!(
            output_dir_for(input, None),
            PathBuf::from("/home/alex/Music/Sonix/Stems/macro-output")
        );
        assert_eq!(
            output_dir_for(input, Some(Path::new("/tmp/ut"))),
            PathBuf::from("/tmp/ut")
        );
    }

    /// **Filtypen läses som den ser ut**, och en kedja utan `suffix`/`dither` ska få rimliga
    /// standardvärden i stället för att saknas.
    #[test]
    fn a_chain_reads_from_its_file_form() {
        let json = r#"{
            "name": "Suno-stämma",
            "steps": [
                {"command": "trim_silence", "threshold_db": -55.0, "keep_ms": 60.0},
                {"command": "normalize", "target_lufs": -14.0, "ceiling_dbtp": -1.0},
                {"command": "export", "format": "flac"}
            ]
        }"#;
        let chain: MacroChain = serde_json::from_str(json).expect("kedjan ska gå att läsa");
        assert_eq!(chain.name, "Suno-stämma");
        assert_eq!(chain.steps.len(), 3);
        assert!(chain.has_export());
        match &chain.steps[2] {
            MacroStep::Export {
                format,
                suffix,
                dither,
                noise_shaping,
            } => {
                assert_eq!(*format, ExportFormat::Flac);
                assert_eq!(suffix, "");
                assert!(*dither, "dither ska vara på när fältet saknas (standardpraxis)");
                assert!(!*noise_shaping);
            }
            other => panic!("fel steg: {other:?}"),
        }
        // Och en kedja utan export säger det — den skriver ingenting.
        let utan = MacroChain::new("bara analys");
        assert!(!utan.has_export());
        // Rundgång: filformen är stabil.
        let back: MacroChain = serde_json::from_str(&serde_json::to_string(&chain).unwrap()).unwrap();
        assert_eq!(back, chain);
    }

    /// **En fil som inte går att läsa ska inte bli en utdatafil.** Doktrinen i ett prov: felet
    /// namnger filen, och ingen fil skrivs i utmappen.
    #[test]
    fn an_unreadable_input_is_reported_and_writes_nothing() {
        let dir = std::env::temp_dir().join("sonix-macro-tests-unreadable");
        let out = dir.join("ut");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("finns_inte.wav");

        let chain = MacroChain {
            name: "t".into(),
            steps: vec![MacroStep::Export {
                format: ExportFormat::Wav16,
                suffix: "_x".into(),
                dither: true,
                noise_shaping: false,
            }],
        };
        let run = run_on_file(
            &chain,
            &missing.to_string_lossy(),
            Some(&out),
            &ExportMeta::default(),
        );
        assert!(!run.succeeded());
        let err = run.error.expect("felet ska stå i raden");
        assert!(err.contains("finns_inte.wav"), "felet ska namnge filen: {err}");
        assert!(run.outputs.is_empty());
        assert!(!out.exists(), "ingen utmapp ska ha skapats");
    }

    /// **Hela kedjan mot en riktig fil** — skriv, läs tillbaka, och mät att stegen gjorde vad
    /// de sa. Det här är provet som gör 8.9 mätt i stället för påstått.
    #[test]
    fn a_chain_normalizes_and_exports_a_real_file() {
        let dir = std::env::temp_dir().join("sonix-macro-tests-real");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("Tagning.wav");
        // En tyst inledning och en ton: trimning och normalisering ska båda synas i svaret.
        let sr = 44_100u32;
        let buf = tone_with_silence(sr, 1.5, 2.0, 0.5);
        crate::audio::write_export_with(
            &input.to_string_lossy(),
            ExportFormat::Wav32,
            &buf,
            sr,
            &ExportMeta::default(),
            DitherSettings::default(),
        )
        .expect("indatafilen ska skrivas");

        let chain = MacroChain {
            name: "Stämma".into(),
            steps: vec![
                MacroStep::TrimSilence {
                    threshold_db: -60.0,
                    keep_ms: 50.0,
                },
                MacroStep::Normalize {
                    target_lufs: -14.0,
                    ceiling_dbtp: -1.0,
                },
                MacroStep::Export {
                    format: ExportFormat::Wav24,
                    suffix: "_klar".into(),
                    dither: true,
                    noise_shaping: false,
                },
            ],
        };
        let out = dir.join("ut");
        let run = run_on_file(
            &chain,
            &input.to_string_lossy(),
            Some(&out),
            &ExportMeta::default(),
        );
        assert!(run.succeeded(), "körningen föll: {:?}", run.error);
        assert_eq!(run.reports.len(), 3, "varje steg ska ha en rad");
        let ut = PathBuf::from(run.outputs[0].clone());
        assert!(ut.exists(), "utdatafilen ska finnas: {}", ut.display());
        assert_eq!(ut.file_name().unwrap(), "Tagning_klar.wav");

        // Läs tillbaka och mäta: filen ska vara kortare än indata (tystnaden borta) och
        // starkare (normaliserad).
        let (l, r, _) = crate::audio::load_audio_pcm(&ut.to_string_lossy()).unwrap();
        let frames_ut = l.len().min(r.len());
        let frames_in = buf.len() / 2;
        assert!(
            frames_ut < frames_in,
            "utdata ({frames_ut}) ska vara kortare än indata ({frames_in}) efter trimningen"
        );
        // Drygt 3,5 s in minus 1,5 s tystnad + marginal ≈ 2,1 s.
        let secs = frames_ut as f32 / sr as f32;
        assert!(
            (secs - 2.1).abs() < 0.2,
            "utdatan blev {secs:.3} s, väntat kring 2,1 s"
        );
        let back: Vec<f32> = l.iter().zip(r.iter()).flat_map(|(l, r)| [*l, *r]).collect();
        // Kedjan lovar två saker: målet i LUFS och taket. En sinus ligger långt under taket
        // även vid −14 LUFS (loudness-normalisering är inte samma sak som att toppa), så
        // provet mäter **löftet** — målet — i stället för att anta en topp.
        let lufs = crate::audio::loudness::integrated_lufs(&back, sr);
        assert!(
            (lufs - (-14.0)).abs() < 0.5,
            "utdatan ligger på {lufs:.2} LUFS, målet var −14"
        );
        let peak = true_peak_db(&back);
        assert!(
            peak <= -1.0 + 0.05,
            "taket på −1 dBTP överskreds: {peak:.2} dBTP"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **En kedja som gör filen tyst rapporteras, den skrivs inte.** Tröskeln över allt
    /// innehåll är det scenario där en tyst fil annars hade blivit ett "resultat".
    #[test]
    fn a_chain_that_would_write_silence_stops_instead() {
        let dir = std::env::temp_dir().join("sonix-macro-tests-silent");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("Tyst.wav");
        let sr = 44_100u32;
        let svag = vec![0.0001f32; sr as usize * 2];
        crate::audio::write_export_with(
            &input.to_string_lossy(),
            ExportFormat::Wav32,
            &svag,
            sr,
            &ExportMeta::default(),
            DitherSettings::default(),
        )
        .expect("indatafilen ska skrivas");

        let chain = MacroChain {
            name: "över tröskeln".into(),
            steps: vec![
                MacroStep::TrimSilence {
                    threshold_db: -20.0,
                    keep_ms: 0.0,
                },
                MacroStep::Export {
                    format: ExportFormat::Wav16,
                    suffix: "_tyst".into(),
                    dither: true,
                    noise_shaping: false,
                },
            ],
        };
        let out = dir.join("ut");
        let run = run_on_file(
            &chain,
            &input.to_string_lossy(),
            Some(&out),
            &ExportMeta::default(),
        );
        assert!(!run.succeeded());
        let err = run.error.expect("felet ska stå i raden");
        assert!(err.contains("under tröskeln"), "fel text: {err}");
        assert!(!out.join("Tyst_tyst.wav").exists(), "ingen tyst fil ska skrivas");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// **Taket vinner över målet — och det ska stå i loggen.** En gles signal (en kort ton,
    /// resten tyst) kan inte nå −14 LUFS utan att toppen går över taket. Då är rätt svar att
    /// stanna under målet och säga det, inte att tvinga fram målet och klippa. Före den här
    /// raden stod det bara "mål −14" i loggen medan filen låg 3,6 LU under — mätt på Alex'
    /// egen Suno-stämma.
    #[test]
    fn when_the_ceiling_binds_the_log_says_so() {
        let dir = std::env::temp_dir().join("sonix-macro-tests-ceiling");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let input = dir.join("Gles.wav");
        let sr = 44_100u32;
        let buf = tone_with_silence(sr, 0.5, 0.05, 2.0);
        crate::audio::write_export_with(
            &input.to_string_lossy(),
            ExportFormat::Wav32,
            &buf,
            sr,
            &ExportMeta::default(),
            DitherSettings::default(),
        )
        .expect("indatafilen ska skrivas");

        let chain = MacroChain {
            name: "taket".into(),
            steps: vec![
                // Målet ligger **över** vad toppen tillåter (toppen är −6 dBTP, taket −1), så
                // taket måste binda. Det är regeln som prövas, inte en viss signal.
                MacroStep::Normalize {
                    target_lufs: -5.0,
                    ceiling_dbtp: -1.0,
                },
                MacroStep::Export {
                    format: ExportFormat::Wav16,
                    suffix: "_n".into(),
                    dither: true,
                    noise_shaping: false,
                },
            ],
        };
        let run = run_on_file(
            &chain,
            &input.to_string_lossy(),
            Some(&dir.join("ut")),
            &ExportMeta::default(),
        );
        assert!(run.succeeded(), "körningen föll: {:?}", run.error);
        let rad = &run.reports[0].detail;
        assert!(
            rad.contains("taket satte gränsen"),
            "loggen måste säga att taket band: {rad}"
        );
        assert!(
            rad.contains("→ -1.0 dBTP"),
            "taket ska ha träffats exakt: {rad}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Startkedjan ska vara körbar rakt av: den har ett exportsteg, och den överlever en
    /// tur genom filformen (det är så en användare kommer att ändra den).
    #[test]
    fn the_example_chain_is_runnable_and_survives_a_round_trip() {
        let chain = example_chain();
        assert!(chain.has_export(), "en startkedja utan export gör ingenting");
        let dir = std::env::temp_dir().join("sonix-macro-tests-example");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("suno.json");
        save_chain(&path, &chain).expect("kedjan ska skrivas");
        let back = load_chain(&path.to_string_lossy()).expect("kedjan ska läsas tillbaka");
        assert_eq!(back, chain);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// En mapp läses i **sorterad** ordning och bara ljudfiler kommer med — en batch ska vara
    /// reproducerbar och inte råka plocka upp loggen från förra körningen.
    #[test]
    fn a_batch_reads_audio_files_in_order() {
        let dir = std::env::temp_dir().join("sonix-macro-tests-dir");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["b.wav", "a.WAV", "c.flac", "anteckningar.txt", "mapp"] {
            let p = dir.join(name);
            if name == "mapp" {
                std::fs::create_dir_all(&p).unwrap();
            } else {
                std::fs::write(&p, b"x").unwrap();
            }
        }
        let files = audio_files_in(&dir).expect("mappen ska gå att läsa");
        let names: Vec<String> = files
            .iter()
            .map(|f| Path::new(f).file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["a.WAV", "b.wav", "c.flac"]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
