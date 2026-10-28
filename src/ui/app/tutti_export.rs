//! **Export till tutti** — det delade projektformatet (`github.com/mxm-audio/tutti`).
//!
//! `.sonix` är källan och förblir det förvalda formatet. Den här modulen skriver en
//! *kopia* i en form andra program kan öppna, så att en låt går att skicka vidare utan
//! att någon behöver Sonix. Mätt mot `tutti check` (0 fel, 0 varningar, Level 0) på
//! A box of you, 8 stämmor, 2026-10-28 — se skillens `references/tutti-format.md`.
//!
//! Tre saker formatet kräver som Sonix inte har i dag, och som därför är synliga här:
//!
//! - **Ljudet kopieras in i `audio/`.** Projektmappen blir lika stor som stämmorna
//!   (416 MB för åtta). Sonix refererar dem på plats; tutti kräver dem innanför mappen.
//! - **Filnamnen måste vara `[A-Za-z0-9_-]`** (schema: `reference`), så Sunos
//!   `4 Guitar.wav` blir `4-Guitar.wav`. Namnet i dokumentet är det som gäller.
//! - **Taktarten antas vara 4/4.** Sonix modellerar ingen taktart; `bars * 4 * PPQ` är
//!   samma räkning som `bars_holding_seconds` gör för sekunder.
//!
//! Det vi inte delar med formatet — warp-lägen, EQ/kompressor, AI-klipp, mic-monitor —
//! följer med under `se.alexwest1981.sonix`. Varje läsare bär det vidare utan att förstå
//! det (spec 8.2), så en runda genom ett annat program tappar inte våra val.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::project::SonixProjectData;

/// Ticks per fjärdedel. 960 är vad tuttimodellens exempel använder och delar jämnt i både
/// 4- och 8-delsunderdelning.
pub const PPQ: i64 = 960;

/// Vårt eget namespace: allt formatet inte har ett fält för hamnar här.
pub const NS: &str = "se.alexwest1981.sonix";

pub struct ExportReport {
    pub folder: PathBuf,
    pub tracks: usize,
    pub clips: usize,
    pub audio_files: usize,
    pub bytes: u64,
}

/// Läser en `.sonix`-fil (eller en autosave) och skriver en tutti-folder.
pub fn export_project_file(src: &Path, out_dir: &Path) -> Result<ExportReport, String> {
    let text =
        std::fs::read_to_string(src).map_err(|e| format!("{}: {e}", src.display()))?;
    let mut data: SonixProjectData =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", src.display()))?;
    // Samma lagning som vid inläsning: en gammal fil med fel källtempo ska inte exporteras
    // med 4,2 % sträckning bara för att den ligger på disk.
    super::project::heal_project_tempo(&mut data);
    export_project(&data, out_dir)
}

/// Skriver `data` som en tutti-folder i `out_dir`. Ljudfilerna kopieras in.
pub fn export_project(data: &SonixProjectData, out_dir: &Path) -> Result<ExportReport, String> {
    let audio_dir = out_dir.join("audio");
    std::fs::create_dir_all(&audio_dir).map_err(|e| format!("{}: {e}", audio_dir.display()))?;

    // Kartan är det som hörs (se references/tempo-and-stretch): fältet `bpm` är en avbild
    // och kan stå i 120 medan kartan säger 125.
    let bpm = data
        .tempo_points
        .first()
        .map(|p| p.bpm)
        .filter(|b| *b > 0.0)
        .unwrap_or(data.bpm)
        .max(1.0);

    let mut files: Vec<Value> = Vec::new();
    let mut copied: std::collections::HashMap<PathBuf, (String, crate::audio::wav_reader::WavInfo, u64)> =
        Default::default();
    let mut tracks: Vec<Value> = Vec::new();
    let mut clip_count = 0usize;
    let mut bytes = 0u64;

    for (ti, track) in data.tracks.iter().enumerate() {
        let mut events: Vec<Value> = Vec::new();
        for (ri, region) in track.regions.iter().enumerate() {
            let Some(source) = region.source_path.as_deref().map(PathBuf::from) else {
                continue;
            };
            let Some(source) = resolve_source(&source) else {
                // En saknad fil får inte stoppa hela exporten: resten av låten är värd mer
                // än att inget blir skrivet.
                continue;
            };
            let (rel, info, n) = match copied.get(&source) {
                Some(hit) => hit.clone(),
                None => {
                    let name = slug_file_name(
                        source
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("audio.wav"),
                    );
                    let rel = format!("audio/{name}");
                    let dst = out_dir.join(&rel);
                    std::fs::copy(&source, &dst).map_err(|e| {
                        format!("kunde inte kopiera {}: {e}", source.display())
                    })?;
                    let info = crate::audio::wav_reader::wav_info(&dst.to_string_lossy())?;
                    let sha = sha256_hex(&dst)?;
                    let n = std::fs::metadata(&dst).map(|m| m.len()).unwrap_or(0);
                    files.push(json!({"path": rel, "sha256": sha, "bytes": n}));
                    let hit = (rel.clone(), info, n);
                    copied.insert(source.clone(), hit.clone());
                    hit
                }
            };
            bytes += n;

            let start_tick = bars_to_ticks(region.start_bar);
            let length_ticks = bars_to_ticks(region.length_bars).max(1);
            let from = (region.sample_offset_sec.max(0.0) as f64 * info.sample_rate as f64) as i64;

            // Materialets tempo placerar `(frame, tick)`-punkterna: två punkter blir en
            // konstant sträckning, en punkt blir "ospänt". Det är samma regel som vår
            // `source_bpm` — därför behövs ingen egen sträckningsfaktor i filen.
            let material_bpm = region.source_bpm;
            let known = material_bpm > 0.0;
            let to = if known {
                let frames_per_tick = 60.0 / (material_bpm as f64 * PPQ as f64);
                from + (length_ticks as f64 * frames_per_tick * info.sample_rate as f64) as i64
            } else {
                let frames_per_tick = 60.0 / (bpm as f64 * PPQ as f64);
                from + (length_ticks as f64 * frames_per_tick * info.sample_rate as f64) as i64
            }
            .clamp(from + 1, (info.frames as i64).max(from + 1));

            let mut audio = json!({
                "file": rel,
                "sha256": files.last().and_then(|f| f["sha256"].as_str()).unwrap_or_default(),
                "sample_rate": info.sample_rate,
                "channels": info.channels,
                "frames": info.frames,
                "from": from,
                "to": to,
                "map": if known {
                    json!([{"frame": from, "tick": 0}, {"frame": to, "tick": length_ticks}])
                } else {
                    json!([{"frame": from, "tick": 0}])
                },
                "bpm": if known { json!(material_bpm) } else { Value::Null },
                "gain": region.volume,
                "polarity": false,
                "channel_mode": "as_recorded",
                "reverse": region.is_reverse,
            });
            // sha256 måste vara filens, inte den senast tillagdas (samma fil kan återanvändas).
            if let Some(sha) = files.iter().find(|f| f["path"] == rel.as_str())
                && let Some(obj) = audio.as_object_mut()
            {
                obj.insert("sha256".into(), sha["sha256"].clone());
            }
            if region.fade_in_bars > 0.0
                && let Some(obj) = audio.as_object_mut()
            {
                obj.insert(
                    "fade_in".into(),
                    json!({"length": bars_to_ticks(region.fade_in_bars).max(1), "shape": "equal_power"}),
                );
            }
            if region.fade_out_bars > 0.0
                && let Some(obj) = audio.as_object_mut()
            {
                obj.insert(
                    "fade_out".into(),
                    json!({"length": bars_to_ticks(region.fade_out_bars).max(1), "shape": "equal_power"}),
                );
            }

            events.push(json!({
                "type": "note",
                "id": format!("evt_{:02}_{:04}", ti, ri),
                "at": start_tick,
                "length": length_ticks,
                "pitch": 60,
                "velocity": 1.0,
                "audio": audio,
            }));
            clip_count += 1;
        }

        let pattern_len = events
            .iter()
            .filter_map(|e| Some(e["at"].as_i64()? + e["length"].as_i64()?))
            .max()
            .unwrap_or(4 * PPQ)
            .max(4 * PPQ);

        tracks.push(json!({
            "id": format!("trk_{ti:02}"),
            "name": track.name,
            "channel": {
                "gain": track.volume,
                "pan": track.pan,
                "mute": track.muted,
                "solo": track.solo,
            },
            "patterns": [{
                "id": format!("pat_{ti:02}"),
                "name": "arrangement",
                "length": pattern_len,
                "events": events,
            }],
            "extensions": {NS: sonix_track_extension(track)},
        }));
    }

    let mut doc = json!({
        "format": "tutti",
        "version": "1.0",
        "about": "An open audio project (tutti 1.0): this folder holds every file it uses. \
                  Time is integer ticks, 'ppq' per quarter note, placed in seconds by 'tempo'. \
                  Audio is placed by (frame, tick) points. Keep what you don't understand.",
        "spec": "https://github.com/mxm-audio/tutti/blob/main/spec/1.0/tutti.md",
        "schema": "https://github.com/mxm-audio/tutti/blob/main/schema/1.0/tutti.schema.json",
        "generator": {"name": "sonix", "version": env!("CARGO_PKG_VERSION")},
        "extensions_used": [NS],
        "metadata": {"title": data.name},
        "ppq": PPQ,
        "tempo": [{"at": 0, "bpm": bpm}],
        "meter": [{"at": 0, "numerator": 4, "denominator": 4}],
        "master": {"gain": data.master_volume, "pan": data.master_pan},
        "files": files,
        "tracks": tracks,
        "extensions": {NS: {
            "swing": data.swing,
            "follow_tempo": data.follow_tempo,
            "song_key_root": data.song_key_root,
            "song_key_scale": data.song_key_scale,
            "notes": data.notes,
            // Stretch-cachen ligger utanför projektet och räknas om av Sonix.
            "stretch_cache": "outside-project",
        }},
    });
    // Filsindexet ska vara sorterat på sökväg (spec 3.4).
    if let Some(list) = doc["files"].as_array_mut() {
        list.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    }

    let file_name = slug_file_name(&format!("{}.tutti", data.name)).replace(".tutti", ".tutti");
    let doc_path = out_dir.join(file_name);
    let text = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&doc_path, text).map_err(|e| format!("{}: {e}", doc_path.display()))?;

    Ok(ExportReport {
        folder: out_dir.to_path_buf(),
        tracks: tracks_len(&doc),
        clips: clip_count,
        audio_files: copied.len(),
        bytes,
    })
}

fn tracks_len(doc: &Value) -> usize {
    doc["tracks"].as_array().map(|a| a.len()).unwrap_or(0)
}

/// Sonix' egna spårfält som formatet inte har: de följer med utan att tolkas.
fn sonix_track_extension(track: &super::project::SavedTrackData) -> Value {
    json!({
        // Hela EQ:n, inte bara gainarna: frekvenser och Q är också inställningar.
        "eq": serde_json::to_value(&track.eq).unwrap_or(Value::Null),
        "comp_threshold_db": track.comp_threshold_db,
        "comp_ratio": track.comp_ratio,
        "reverb_send": track.reverb_send,
        "delay_send": track.delay_send,
    })
}

/// `4 Guitar.wav` → `4-Guitar.wav`. Schema `reference` tillåter bara `[A-Za-z0-9_-]` i
/// namnet, så mellanslag och punkter måste bort — filen i mappen döps om på samma sätt.
pub fn slug_file_name(name: &str) -> String {
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s, e),
        None => (name, "wav"),
    };
    let mut out = String::with_capacity(stem.len() + ext.len() + 1);
    let mut last_dash = true; // inget inledande bindestreck
    for c in stem.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    let ext: String = ext
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect();
    if out.is_empty() {
        out.push_str("audio");
    }
    format!("{out}.{}", if ext.is_empty() { "wav".to_string() } else { ext })
}

/// Takt → ticks i 4/4. Samma räkning som `bars_holding_seconds` gör för sekunder.
pub fn bars_to_ticks(bars: f32) -> i64 {
    if !bars.is_finite() || bars <= 0.0 {
        return 0;
    }
    (bars as f64 * 4.0 * PPQ as f64).round() as i64
}

/// Autosaven sparar relativa sökvägar (`./imported_stems/...`), relativa till katalogen
/// Sonix startades i — i praktiken hemkatalogen. En absolut sökväg används som den är.
fn resolve_source(path: &Path) -> Option<PathBuf> {
    if path.exists() {
        return Some(path.to_path_buf());
    }
    if path.is_absolute() {
        return None;
    }
    let cleaned = path.to_string_lossy().trim_start_matches("./").to_string();
    let candidate = PathBuf::from(std::env::var("HOME").ok()?).join(cleaned);
    candidate.exists().then_some(candidate)
}

fn sha256_hex(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
    Ok(digest
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Suno-namnen har mellanslag och tuttis `reference` tillåter dem inte. Provet håller
    /// både omskrivningen och det som inte får hända (tomt namn, dubbla bindestreck).
    #[test]
    fn file_names_become_references() {
        assert_eq!(slug_file_name("4 Guitar.wav"), "4-Guitar.wav");
        assert_eq!(slug_file_name("0 Lead Vocals.wav"), "0-Lead-Vocals.wav");
        assert_eq!(slug_file_name("redan_bra.wav"), "redan_bra.wav");
        assert_eq!(slug_file_name("  ...wav"), "audio.wav", "tomt namn får en platshållare");
        assert_eq!(slug_file_name("låt (svensk).wav"), "l-t-svensk.wav");
        // Schema: ^(audio|...)/[A-Za-z0-9_-]{1,120}\.[A-Za-z0-9]{1,8}$
        for n in ["4 Guitar.wav", "låt (svensk).wav", "  ...wav"] {
            let slug = slug_file_name(n);
            assert!(
                slug.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.'),
                "{slug}"
            );
        }
    }

    /// Takt → ticks måste stämma med sekundräkningen, annars hamnar klippen på fel takt
    /// i det andra programmet. 147,83 takter i 125 BPM är 283,8 s — samma tal som
    /// `bars_holding_seconds` ger.
    #[test]
    fn bars_become_ticks_at_four_four() {
        assert_eq!(bars_to_ticks(1.0), 3840);
        assert_eq!(bars_to_ticks(0.5), 1920);
        let ticks = bars_to_ticks(147.833_33);
        let seconds = ticks as f64 / PPQ as f64 / 4.0 * 4.0 * 60.0 / 125.0;
        assert!((seconds - 283.84).abs() < 0.1, "{seconds}");
        assert_eq!(bars_to_ticks(-1.0), 0);
        assert_eq!(bars_to_ticks(f32::NAN), 0);
    }
}
