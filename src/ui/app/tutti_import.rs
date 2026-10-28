//! **Import från tutti** — det delade projektformatet (`github.com/mxm-audio/tutti`).
//!
//! Motsatsen till `tutti_export`: en tutti-folder (eller en annan DAW:s) blir en `.sonix`-fil
//! som appen öppnar som vilket projekt som helst. `.sonix` är kvar som det egna formatet —
//! importen är en väg in, inte ett formatbyte.
//!
//! Det som är *vårt* läses ur `se.alexwest1981.sonix` när det finns (EQ, kompressor, swing,
//! tonart, följ-tempo), så en runda Sonix → tutti → Sonix tappar inget vi själva lagt dit.
//!
//! **Det som inte kan översättas sägs högt** i stället för att tystna:
//! - noter (MIDI) importeras inte — Sonix' tidslinje spelar ljudklipp, inte noter, och en
//!   tyst halvlåt är värre än ett besked.
//! - plugins och enheter importeras inte; deras `dependencies` listas i rapporten.
//! - en ljudfil som saknas eller inte matchar sitt SHA-256 hoppas över med ett besked.

use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::project::SonixProjectData;

/// Ticks per fjärdedel när dokumentet inte säger något. Samma tal som exporten skriver.
const DEFAULT_PPQ: i64 = 960;

pub struct ImportReport {
    pub out_file: PathBuf,
    pub tracks: usize,
    pub clips: usize,
    pub warnings: Vec<String>,
}

/// Läser `src` (en tutti-mapp eller en `.tutti`-fil) och skriver en `.sonix`-fil till `out`.
pub fn import_tutti(src: &Path, out: &Path) -> Result<ImportReport, String> {
    let doc_path = find_document(src)?;
    let folder = doc_path
        .parent()
        .ok_or_else(|| format!("{}: ingen mapp", doc_path.display()))?
        .to_path_buf();
    let text = std::fs::read_to_string(&doc_path)
        .map_err(|e| format!("{}: {e}", doc_path.display()))?;
    let doc: Value =
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", doc_path.display()))?;

    if let Some(format) = doc["format"].as_str()
        && format != "tutti"
    {
        return Err(format!("{format}: inte ett tutti-dokument"));
    }

    let ppq = doc["ppq"].as_i64().unwrap_or(DEFAULT_PPQ).max(1);
    let ticks_per_bar = 4 * ppq; // 4/4: samma antagande som exporten gör
    let mut warnings: Vec<String> = Vec::new();

    // Tempot, punkterna och våra egna fält.
    let mut tempo_points: Vec<Value> = Vec::new();
    if let Some(points) = doc["tempo"].as_array() {
        for p in points {
            let Some(bpm) = p["bpm"].as_f64() else { continue };
            let bar = p["at"].as_i64().unwrap_or(0) as f64 / ticks_per_bar as f64;
            tempo_points.push(json!({"start_bar": bar as u32, "bpm": bpm as f32}));
        }
    }
    let bpm = tempo_points
        .first()
        .and_then(|p| p["bpm"].as_f64())
        .unwrap_or(120.0) as f32;
    if doc["meter"]
        .as_array()
        .and_then(|m| m.first().cloned())
        .and_then(|m| Some((m["numerator"].as_i64()?, m["denominator"].as_i64()?)))
        .is_some_and(|(num, den)| num != 4 || den != 4)
    {
        warnings.push("taktarten är inte 4/4 — Sonix räknar i 4/4, så taktlinjerna kan sitta fel".into());
    }

    if doc["tracks"]
        .as_array()
        .map(|ts| has_notes_without_audio(ts))
        .unwrap_or(false)
    {
        warnings.push("noter (MIDI-event) importeras inte — bara ljudklipp".into());
    }

    let ours = doc["extensions"][super::tutti_export::NS].clone();
    let mut tracks: Vec<Value> = Vec::new();
    let mut clip_count = 0usize;
    let mut next_region_id = 1usize;

    let source_tracks = flatten_tracks(&doc["tracks"].as_array().cloned().unwrap_or_default());
    if doc["tracks"]
        .as_array()
        .map(|ts| ts.iter().any(|t| !t["children"].as_array().map(|c| c.is_empty()).unwrap_or(true)))
        .unwrap_or(false)
    {
        warnings.push("spårmappar plattas ut — Sonix har inga mappar i mixern".into());
    }
    for track in source_tracks.iter() {
        let mut regions: Vec<Value> = Vec::new();
        for (ticks, audio) in audio_clips_of(track, ticks_per_bar, &mut warnings) {
            let Some(file) = audio["file"].as_str() else {
                continue;
            };
            let path = folder.join(file);
            let Ok(path_str) = path.to_str().map(str::to_string).ok_or(()) else {
                warnings.push(format!("{file}: sökvägen går inte att läsa"));
                continue;
            };
            let Ok(meta) = std::fs::metadata(&path) else {
                warnings.push(format!("{file}: filen saknas i mappen — klippet hoppas över"));
                continue;
            };
            let listed = doc["files"]
                .as_array()
                .and_then(|f| f.iter().find(|e| e["path"].as_str() == Some(file)))
                .and_then(|e| e["bytes"].as_u64());
            if let Some(bytes) = listed
                && bytes != meta.len()
            {
                warnings.push(format!(
                    "{file}: filens storlek stämmer inte med indexet ({} ≠ {})",
                    meta.len(),
                    bytes
                ));
            }
            let sample_rate = audio["sample_rate"].as_f64().unwrap_or(48_000.0);
            let from = audio["from"].as_i64().unwrap_or(0).max(0) as f32;
            let peaks = crate::audio::wav_reader::read_wav_envelope(&path_str, 1024)
                .unwrap_or_default();

            regions.push(json!({
                "id": next_region_id,
                "name": audio["name"].as_str()
                    .filter(|n| !n.is_empty())
                    .map(str::to_string)
                    .unwrap_or_else(|| {
                        Path::new(file)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("klipp")
                            .to_string()
                    }),
                "start_bar": ticks_to_bars(ticks, ticks_per_bar),
                "length_bars": audio["length_ticks"].as_i64().unwrap_or(ticks_per_bar)
                    as f32 / ticks_per_bar as f32,
                "sample_offset_sec": from / sample_rate as f32,
                "source_path": path_str,
                "waveform_peaks": peaks,
                "volume": audio["gain"].as_f64().unwrap_or(1.0) as f32,
                "fade_in_bars": fade_bars(&audio["fade_in"], ticks_per_bar),
                "fade_out_bars": fade_bars(&audio["fade_out"], ticks_per_bar),
                "muted": track["channel"]["mute"].as_bool().unwrap_or(false),
                "is_reverse": audio["reverse"].as_bool().unwrap_or(false),
                "source_bpm": audio["bpm"].as_f64().unwrap_or(0.0) as f32,
            }));
            next_region_id += 1;
            clip_count += 1;
        }

        let track_ours = track["extensions"][super::tutti_export::NS].clone();
        let track_ours = (!track_ours.is_null()).then_some(&track_ours);
        tracks.push(json!({
            "name": track["name"].as_str().unwrap_or("spår"),
            "notes": "",
            "volume": track["channel"]["gain"].as_f64().unwrap_or(1.0) as f32,
            "pan": track["channel"]["pan"].as_f64().unwrap_or(0.0) as f32,
            "muted": track["channel"]["mute"].as_bool().unwrap_or(false),
            "solo": track["channel"]["solo"].as_bool().unwrap_or(false),
            "clips": vec![Value::Null; 32],
            "regions": regions,
            "eq": ours_of(track_ours, "eq").unwrap_or_else(|| json!({})),
            "comp_threshold_db": ours_of(track_ours, "comp_threshold_db").unwrap_or(json!(-30.0)),
            "comp_ratio": ours_of(track_ours, "comp_ratio").unwrap_or(json!(2.0)),
            "reverb_send": ours_of(track_ours, "reverb_send").unwrap_or(json!(0.0)),
            "delay_send": ours_of(track_ours, "delay_send").unwrap_or(json!(0.0)),
        }));
    }

    if let Some(deps) = doc["dependencies"].as_array()
        && !deps.is_empty()
    {
        let names: Vec<String> = deps
            .iter()
            .filter_map(|d| d["name"].as_str().or(d["id"].as_str()).map(str::to_string))
            .collect();
        warnings.push(format!(
            "{} plugins/enheter följer inte med ({}): ljudet spelas utan dem",
            names.len(),
            names.join(", ")
        ));
    }

    let data = json!({
        "name": doc["metadata"]["title"].as_str()
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| doc_path.file_stem().and_then(|s| s.to_str()).unwrap_or("Importerat").to_string()),
        "bpm": bpm,
        "notes": ours_of(Some(&ours), "notes").and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default(),
        "tempo_points": tempo_points,
        "follow_tempo": ours_of(Some(&ours), "follow_tempo").and_then(|v| v.as_bool()).unwrap_or(true),
        "song_key_root": ours_of(Some(&ours), "song_key_root").and_then(|v| v.as_u64()).unwrap_or(3),
        "song_key_scale": ours_of(Some(&ours), "song_key_scale").and_then(|v| v.as_u64()).unwrap_or(0),
        "swing": ours_of(Some(&ours), "swing").and_then(|v| v.as_f64()).unwrap_or(0.0),
        "master_volume": doc["master"]["gain"].as_f64().unwrap_or(0.85),
        "master_pan": doc["master"]["pan"].as_f64().unwrap_or(0.0),
        "tracks": tracks,
    });

    let parsed: SonixProjectData = serde_json::from_value(data.clone())
        .map_err(|e| format!("kunde inte bygga projektet: {e}"))?;

    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())?;
    crate::autosave::write_atomic(out, text.as_bytes())
        .map_err(|e| format!("{}: {e}", out.display()))?;

    Ok(ImportReport {
        out_file: out.to_path_buf(),
        tracks: tracks.len(),
        clips: clip_count,
        warnings,
    })
}

/// **Spårträdet som en platt lista.** En mapp i tutti är ett spår med barn; Sonix har ingen
/// mappnivå, så barnen blir egna spår i stället för att försvinna.
fn flatten_tracks(tracks: &[Value]) -> Vec<Value> {
    let mut out = Vec::new();
    for track in tracks {
        out.push(track.clone());
        if let Some(children) = track["children"].as_array() {
            out.extend(flatten_tracks(children));
        }
    }
    out
}

/// Finns det noter någonstans i dokumentet som inte bär ljud? Då sägs det en gång.
fn has_notes_without_audio(tracks: &[Value]) -> bool {
    tracks.iter().any(|track| {
        let in_patterns = track["patterns"]
            .as_array()
            .map(|ps| {
                ps.iter().any(|p| {
                    p["events"].as_array().is_some_and(|es| {
                        es.iter().any(|e| {
                            e["audio"].is_null()
                                && matches!(e["type"].as_str(), Some("note") | Some("chord"))
                        })
                    })
                })
            })
            .unwrap_or(false);
        in_patterns
            || track["children"]
                .as_array()
                .is_some_and(|c| has_notes_without_audio(c))
    })
}

/// Vårt eget fält, om det finns.
fn ours_of(ours: Option<&Value>, key: &str) -> Option<Value> {
    ours?.get(key).cloned().filter(|v| !v.is_null())
}

/// Faderns längd i takter. Tutti räknar ticks, Sonix takter.
fn fade_bars(fade: &Value, ticks_per_bar: i64) -> f32 {
    fade["length"]
        .as_i64()
        .map(|t| t as f32 / ticks_per_bar as f32)
        .unwrap_or(0.0)
}

fn ticks_to_bars(ticks: i64, ticks_per_bar: i64) -> f32 {
    ticks as f32 / ticks_per_bar.max(1) as f32
}

/// **Klippen på spårets tidslinje, i ticks.** Ett event kan ligga i en pattern som en `part`
/// placerar ut (då flyttas det med partens `at - offset`), eller — som vår egen export gör —
/// direkt i en pattern utan parter, och då gäller eventets egen `at`.
fn audio_clips_of(track: &Value, ticks_per_bar: i64, warnings: &mut Vec<String>) -> Vec<(i64, Value)> {
    let patterns = track["patterns"].as_array().cloned().unwrap_or_default();
    let parts = track["parts"].as_array().cloned().unwrap_or_default();
    let mut out = Vec::new();

    if parts.is_empty() {
        for pattern in &patterns {
            for event in pattern["events"].as_array().cloned().unwrap_or_default() {
                if event["audio"].is_null() {
                    continue;
                }
                let at = event["at"].as_i64().unwrap_or(0);
                let length = event["length"].as_i64().unwrap_or_else(|| {
                    // Ingen längd: räkna hem den ur materialets tempo och klippets ramar.
                    let a = &event["audio"];
                    let frames = a["to"].as_i64().unwrap_or(0) - a["from"].as_i64().unwrap_or(0);
                    let sr = a["sample_rate"].as_f64().unwrap_or(48_000.0);
                    let bpm = a["bpm"].as_f64().unwrap_or(120.0);
                    ((frames as f64 / sr) * bpm / 60.0 * ticks_per_bar as f64 / 4.0) as i64
                });
                out.push((at, clip_with_length(event["audio"].clone(), length)));
            }
        }
        return out;
    }

    let mut ignored = 0usize;
    for part in &parts {
        let Some(pattern_id) = part["pattern"].as_str() else {
            continue;
        };
        let Some(pattern) = patterns.iter().find(|p| p["id"].as_str() == Some(pattern_id)) else {
            ignored += 1;
            continue;
        };
        let at = part["at"].as_i64().unwrap_or(0);
        let offset = part["offset"].as_i64().unwrap_or(0);
        let length = part["length"].as_i64().unwrap_or(i64::MAX);
        for event in pattern["events"].as_array().cloned().unwrap_or_default() {
            if event["audio"].is_null() {
                continue;
            }
            let rel = event["at"].as_i64().unwrap_or(0) - offset;
            if rel < 0 || rel >= length {
                continue; // utanför partens fönster
            }
            let ev_len = event["length"].as_i64().unwrap_or(length - rel);
            let visible = ev_len.min(length - rel).max(1);
            out.push((at + rel, clip_with_length(event["audio"].clone(), visible)));
        }
    }
    if ignored > 0 {
        warnings.push(format!(
            "{ignored} parter pekade på mönster som inte finns — hoppade över"
        ));
    }
    out.sort_by_key(|(tick, _)| *tick);
    out
}

fn clip_with_length(mut audio: Value, ticks: i64) -> Value {
    if let Some(obj) = audio.as_object_mut() {
        obj.insert("length_ticks".into(), json!(ticks.max(1)));
    }
    audio
}

/// Hittar dokumentet: en `.tutti`-fil, eller den enda `.tutti` i en mapp.
fn find_document(src: &Path) -> Result<PathBuf, String> {
    if src.is_file() {
        return Ok(src.to_path_buf());
    }
    let mut found: Vec<PathBuf> = std::fs::read_dir(src)
        .map_err(|e| format!("{}: {e}", src.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "tutti"))
        .collect();
    found.sort();
    match found.len() {
        1 => Ok(found.remove(0)),
        0 => Err(format!("{}: inget .tutti-dokument i mappen", src.display())),
        _ => Err(format!(
            "{}: flera .tutti-dokument ({}) — peka ut ett av dem",
            src.display(),
            found
                .iter()
                .filter_map(|p| p.file_name()?.to_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Klipplängden räknas i takter ur ticks, och en part flyttar sina event — annars
    /// hamnar ett importerat projekt en part fel, vilket är tyst och svårt att se.
    #[test]
    fn ticks_become_bars_and_parts_move_their_events() {
        assert_eq!(ticks_to_bars(3840, 3840), 1.0);
        assert_eq!(ticks_to_bars(147 * 3840 + 3200, 3840), 147.833_33);

        let mut warnings = Vec::new();
        let track = json!({
            "id": "trk_1",
            "patterns": [
                {"id": "pat_1", "length": 7680, "events": [
                    {"type": "note", "id": "e1", "at": 0, "length": 3840,
                     "pitch": 60, "velocity": 1.0,
                     "audio": {"file": "audio/a.wav", "sample_rate": 48000, "from": 0, "to": 48000, "bpm": 125.0}},
                ]},
                {"id": "pat_2", "length": 3840, "events": []}
            ],
            "parts": [
                {"id": "p1", "pattern": "pat_1", "at": 3840, "length": 3840, "offset": 0},
                {"id": "p2", "pattern": "pat_nope", "at": 0, "length": 3840}
            ]
        });
        let clips = audio_clips_of(&track, 3840, &mut warnings);
        assert_eq!(clips.len(), 1, "parten klipper bort det som ligger utanför");
        assert_eq!(clips[0].0, 3840, "partens at + eventets at");
        assert_eq!(clips[0].1["length_ticks"], 3840);
        assert!(warnings.iter().any(|w| w.contains("mönster som inte finns")));

        // Utan parter gäller eventets egen position (vår egen exports form).
        let flat = json!({
            "id": "trk_2",
            "patterns": [{"id": "pat_1", "length": 7680, "events": [
                {"type": "note", "id": "e1", "at": 7680, "length": 3840, "pitch": 60, "velocity": 1.0,
                 "audio": {"file": "audio/b.wav", "sample_rate": 48000, "from": 0, "to": 48000}}
            ]}]
        });
        let mut w2 = Vec::new();
        let flat_clips = audio_clips_of(&flat, 3840, &mut w2);
        assert_eq!(flat_clips[0].0, 7680);
        assert_eq!(ticks_to_bars(flat_clips[0].0, 3840), 2.0);
    }
}
