//! **JSFX-hosten.** Ett bibliotek av effekter och instrument i REAPERs JSFX-format, körda av
//! vår egen tolk (`parser.rs` + `engine.rs`).
//!
//! Varför en tolk och inte 265 portar: REAPER 7.83 skeppar 265 JSFX-filer med ~80 000 rader DSP
//! (`InstallData/Effects`). Att skriva av dem en och en är 265 chanser att skriva fel, och nästa
//! REAPER-version har nya. En tolk är en sak att underhålla — och den som skriver egen JSFX får
//! den att köra direkt.
//!
//! Mappen läses på samma sätt som REAPER:s: en katalog med `.jsfx`-filer (eller filer utan
//! ändelse, vilket de äldre i REAPER-lådan är), i undermappar per upphovsman.

pub mod engine;
pub mod parser;

pub use engine::{Jsfx, MidiEvent, RunError};
pub use parser::SliderDef;

use std::path::{Path, PathBuf};

/// En fil i biblioteket, med det den säger om sig själv.
#[derive(Debug, Clone)]
pub struct Entry {
    pub path: PathBuf,
    /// `desc:` ur filhuvudet (kan vara tom).
    pub desc: String,
    pub sliders: Vec<SliderDef>,
    /// Filens egna licensrader ur huvudet, om de finns — de ska följa med en port.
    pub licence: Option<String>,
}

/// Letar JSFX-filer i en mapp (och dess undermappar). REAPER:s äldre filer har **ingen**
/// filändelse, så en fil utan punkt räknas också — men bara om den ser ut som JSFX (har en
/// `@init`/`@sample`-sektion), annars hade varje README blivit en effekt.
pub fn list(dir: &Path) -> Vec<Entry> {
    let mut out = Vec::new();
    collect(dir, &mut out, 0);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

fn collect(dir: &Path, out: &mut Vec<Entry>, depth: usize) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out, depth + 1);
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let looks_like_jsfx = name.ends_with(".jsfx")
            || (!name.contains('.') && !name.starts_with('.'));
        if !looks_like_jsfx {
            continue;
        }
        // **Inte alla filer är UTF-8** (några har latin-1 i kommentarerna). Vi läser bytes och
        // ersätter det som inte går att avkoda — en trasig kommentar ska inte kosta en effekt.
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let source = String::from_utf8_lossy(&bytes).to_string();
        if !source.contains("@init") && !source.contains("@sample") {
            continue;
        }
        let (desc, sliders, licence) = header_of(&source);
        out.push(Entry {
            path,
            desc,
            sliders,
            licence,
        });
    }
}

/// Läser `desc:`, sliderraderna och en licensrad ur filhuvudet utan att tolka hela filen.
pub fn header_of(source: &str) -> (String, Vec<SliderDef>, Option<String>) {
    let mut desc = String::new();
    let mut sliders = Vec::new();
    let mut licence = None;
    for line in source.lines().take(80) {
        let trimmed = line.trim().trim_start_matches("//").trim();
        if let Some(rest) = line.trim().strip_prefix("desc:") {
            if !desc.is_empty() {
                desc.push(' ');
            }
            desc.push_str(rest.trim());
        } else if let Some(def) = parser::parse_slider_header(line.trim()) {
            sliders.push(def);
        } else if licence.is_none()
            && (trimmed.contains("GPL") || trimmed.contains("MIT") || trimmed.contains("LGPL"))
        {
            licence = Some(trimmed.to_string());
        }
    }
    (desc, sliders, licence)
}

/// Kör ett block genom en instans och lämnar tillbaka MIDI ut. Fasad så att `MidiEvent` och
/// `RunError` syns i API:t i stället för att bara finnas som återexporter.
pub fn process(
    vm: &mut Jsfx,
    left: &mut [f32],
    right: &mut [f32],
    midi: &[MidiEvent],
) -> Result<Vec<MidiEvent>, RunError> {
    vm.process(left, right, midi)
}

/// Standardmappen för egna JSFX: `~/.config/sonix/jsfx`.
pub fn user_dir() -> PathBuf {
    crate::paths::paths().config_dir().join("jsfx")
}

/// Delar en JSFX-text i filhuvud (desc/slider/pins) och kod (från första `@`-sektionen).
fn split_header(text: &str) -> (String, String) {
    let mut header = String::new();
    let mut body = String::new();
    let mut in_body = false;
    for line in text.lines() {
        if !in_body && line.trim_start().starts_with('@') {
            in_body = true;
        }
        if in_body {
            body.push_str(line);
            body.push('\n');
        } else {
            header.push_str(line);
            header.push('\n');
        }
    }
    (header, body)
}

/// `import Dependencies/ChamberOrganStops.jsfx-inc` — REAPER klistrar in filen på plats, och
/// sektionerna flyter ihop (två `@init` blir en).
///
/// **Filhuvudet i den importerade filen hoppas över.** ChamberOrgan importerar sin stopptabell på
/// rad 8 och har sina egna sliders på rad 201; klistrade rakt in hade de egna sliders blivit kod.
/// Den importerade filens `desc:`/`slider`-rader hör till dess egen effekt, inte till den här.
pub fn expand_imports(body: &str, base: &Path, depth: usize) -> String {
    if depth > 8 {
        return body.to_string();
    }
    let mut out = String::with_capacity(body.len());
    for line in body.lines() {
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("import ") {
            let name = rest.split("//").next().unwrap_or(rest).trim();
            let path = base.join(name);
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let included = String::from_utf8_lossy(&bytes).to_string();
                    let (_, included_body) = split_header(&included);
                    let dir = path.parent().unwrap_or(base).to_path_buf();
                    out.push_str(&expand_imports(&included_body, &dir, depth + 1));
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                }
                Err(e) => {
                    // En import vi inte hittar får inte tyst försvinna.
                    out.push_str(&format!("// import saknas: {} ({e})\n", path.display()));
                }
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Kompilerar en fil från disk, med `import`-filer inlästa.
pub fn load(path: &Path, sample_rate: f64) -> Result<Jsfx, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("kunde inte läsa {}: {e}", path.display()))?;
    let source = String::from_utf8_lossy(&bytes).to_string();
    let base = path.parent().unwrap_or(Path::new("."));
    // Huvudet läses ur originalet, koden ur originalet + importerade filer.
    let (header, body) = split_header(&source);
    let mut expanded = String::with_capacity(source.len() * 2);
    expanded.push_str(&header);
    expanded.push_str(&expand_imports(&body, base, 0));
    Jsfx::compile(&expanded, sample_rate)
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod corpus_tests {
    use super::*;

    /// **Hur mycket av REAPER-lådan som faktiskt tolkas.** Opt-in med `SONIX_JSFX_DIR` satt till
    /// `InstallData/Effects`; annars hoppar provet över, eftersom filerna inte skeppas med Sonix.
    ///
    /// Provet svarar på frågan som filnamnen inte kan svara på: hur många av filerna vi kan tolka,
    /// och vilka som faller — med orsak.
    #[test]
    fn the_engine_reads_a_share_of_a_real_jsfx_library() {
        let Ok(dir) = std::env::var("SONIX_JSFX_DIR") else {
            return;
        };
        let entries = list(Path::new(&dir));
        assert!(entries.len() > 100, "biblioteket ska vara stort: {}", entries.len());
        let mut ok = 0;
        let mut failures: Vec<(String, String)> = Vec::new();
        for entry in &entries {
            match load(&entry.path, 44_100.0) {
                Ok(_) => ok += 1,
                Err(e) => failures.push((entry.path.display().to_string(), e)),
            }
        }
        println!(
            "JSFX: {ok}/{} filer tolkas ({} faller)",
            entries.len(),
            failures.len()
        );
        for (path, error) in failures.iter().take(12) {
            println!("  ✗ {path}: {error}");
        }
        // **Grinden är alla filer.** REAPER 7.83:s 256 filer tolkas alla; en regression som
        // tappar en enda fil är ett fel, inte en nyans.
        assert_eq!(
            ok,
            entries.len(),
            "bara {ok}/{} filer tolkas",
            entries.len()
        );
    }

    /// **Instrumenten ska göra det de kan.** De som är rena syntar ska ge ljud från en nedtryckt
    /// tangent; de som laddar samplingar ska *säga* att de saknar filerna i stället för att
    /// tystna utan förklaring. Provet prövar alltså både ljudet och förklaringen — en effekt som
    /// tystnar ska gå att skilja från en som inte kunde läsas.
    #[test]
    fn instruments_sound_or_say_why_not() {
        let Ok(dir) = std::env::var("SONIX_JSFX_DIR") else {
            return;
        };
        let instruments = [
            "chmaha-inst/TolerableTines.jsfx",
            "chmaha-inst/MajorGeneralMIDI.jsfx",
            "chmaha-inst/DrNine.jsfx",
            "chmaha-inst/JamSynth.jsfx",
            "chmaha-inst/SynthyMcSynthface.jsfx",
        ];
        let mut sounding = 0;
        let mut explained = 0;
        for rel in instruments {
            let path = Path::new(&dir).join(rel);
            if !path.is_file() {
                continue;
            }
            let started = std::time::Instant::now();
            let mut vm = match load(&path, 44_100.0) {
                Ok(vm) => vm,
                Err(e) => {
                    println!("  ✗ {rel}: {e}");
                    continue;
                }
            };
            let init_ms = started.elapsed().as_millis();
            for def in vm.slider_defs().to_vec() {
                let _ = vm.set_slider(def.index, def.default);
            }
            let frames = 1024;
            let mut left = vec![0.0f32; frames];
            let mut right = vec![0.0f32; frames];
            let note = [MidiEvent {
                offset: 0,
                status: 0x90,
                data1: 60,
                data2: 100,
            }];
            if let Err(e) = vm.process(&mut left, &mut right, &note) {
                println!("  ✗ {rel}: {e}");
                continue;
            }
            let peak = left.iter().fold(0.0f32, |a, v| a.max(v.abs()));
            let finite = left.iter().all(|v| v.is_finite());
            let run_ms = started.elapsed().as_millis() - init_ms;
            let needs_files = vm.stubs.contains("file_open");
            let verdict = if peak > 1e-4 {
                sounding += 1;
                "låter"
            } else if needs_files {
                explained += 1;
                "tyst: laddar samplingar vi inte har (file_open är stubbad)"
            } else {
                "tyst utan förklaring"
            };
            println!(
                "  {rel}: topp {peak:.4}, ändlig {finite}, init {init_ms} ms, {frames} prov {run_ms} ms — {verdict}"
            );
        }
        assert!(sounding >= 2, "bara {sounding} instrument gav ljud");
        assert!(
            sounding + explained >= 4,
            "{} instrument saknar både ljud och förklaring",
            5 - sounding - explained
        );
    }

    /// Kör **riktiga effekter** på en testsignal och kräver att de gör något med ljudet.
    #[test]
    fn real_effects_process_audio() {
        let Ok(dir) = std::env::var("SONIX_JSFX_DIR") else {
            return;
        };
        // En kompressor, en EQ-splitter och en limiter: tre olika sorters DSP.
        let candidates = [
            "sstillwell/1175",
            "loser/3BandSplitter",
            "chmaha-fx/Maximizer.jsfx",
            "Liteon/deesser",
        ];
        let mut ran = 0;
        for rel in candidates {
            let path = Path::new(&dir).join(rel);
            if !path.is_file() {
                continue;
            }
            let mut vm = match load(&path, 44_100.0) {
                Ok(vm) => vm,
                Err(e) => {
                    println!("  ✗ {rel}: {e}");
                    continue;
                }
            };
            // Sätt varje slider på sitt standardvärde → kör @slider.
            let defs = vm.slider_defs().to_vec();
            for def in &defs {
                let _ = vm.set_slider(def.index, def.default);
            }
            let frames = 4096;
            let mut left: Vec<f32> = (0..frames)
                .map(|i| 0.5 * (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 44_100.0).sin())
                .collect();
            let mut right = left.clone();
            if let Err(e) = vm.process(&mut left, &mut right, &[]) {
                println!("  ✗ {rel}: {e}");
                continue;
            }
            let out_energy: f32 = left.iter().map(|v| v * v).sum();
            let finite = left.iter().all(|v| v.is_finite());
            println!("  ✓ {rel}: energi {out_energy:.4}, ändlig {finite}");
            assert!(finite, "{rel} gav NaN/Inf");
            ran += 1;
        }
        assert!(ran >= 2, "minst två riktiga effekter ska köra, körde {ran}");
    }
}
