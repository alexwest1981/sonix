//! **JSFX som plugin-instans.** Bryggan mellan tolken (`engine.rs`) och effektkedjan: en
//! `.jsfx`-fil laddas där en CLAP/VST-plugin hade laddats, och blir en `PluginProcessor`.
//!
//! Sliderna blir parametrar, med filens eget intervall — en effekt som säger `-60..0 dB` får en
//! parameter som säger samma sak. `shortcut: instrument tar ingen MIDI härifrån (traitet har ingen
//! notingång), så JSFX-instrument låter först när MIDI-vägen är på plats; effekter fungerar fullt ut.`

use std::path::Path;

use super::{Jsfx, MidiEvent};
use crate::audio::plugin_host_live::{
    PluginInfo, PluginInspection, PluginParameter, PluginProcessor, PARAM_IS_STEPPED,
};

/// En `.jsfx`-fil känns igen på ändelsen. REAPERs äldre filer saknar ändelse, men de laddas inte
/// härifrån — de nås via `--jsfx-run`/`--jsfx-note`, där användaren pekar ut filen.
pub fn is_jsfx_path(path: &str) -> bool {
    path.to_ascii_lowercase().ends_with(".jsfx")
}

pub fn inspect(path: &str) -> PluginInspection {
    let Ok(bytes) = std::fs::read(path) else {
        return PluginInspection {
            path: path.to_string(),
            backend: "JSFX".to_string(),
            info: None,
            parameters: Vec::new(),
            gui: None,
            error: Some(format!("kunde inte läsa {path}")),
        };
    };
    let source = String::from_utf8_lossy(&bytes);
    let expanded = expand(path, &source);
    let (desc, sliders, _) = super::header_of(&expanded);
    PluginInspection {
        path: path.to_string(),
        backend: "JSFX".to_string(),
        info: Some(info_for(path, &desc)),
        parameters: parameters_for(&sliders),
        gui: None,
        error: None,
    }
}

/// Laddar en `.jsfx`-fil som en processor i effektkedjan.
pub fn load(path: &str, sample_rate: f32) -> Result<Box<dyn PluginProcessor>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let source = String::from_utf8_lossy(&bytes);
    let vm = Jsfx::compile(&expand(path, &source), sample_rate as f64)
        .map_err(|e| format!("{path}: {e}"))?;
    let parameters = parameters_for(vm.slider_defs());
    let info = info_for(path, vm.desc());
    Ok(Box::new(JsfxProcessor {
        vm,
        pending: Vec::new(),
        info,
        parameters,
    }))
}

/// `import`-filer inlästa, samma väg som `jsfx::load` använder.
fn expand(path: &str, source: &str) -> String {
    let base = Path::new(path).parent().unwrap_or(Path::new("."));
    let (header, body) = super::split_header(source);
    format!("{header}{}", super::expand_imports(&body, base, 0))
}

fn info_for(path: &str, desc: &str) -> PluginInfo {
    let name = Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string());
    PluginInfo {
        id: format!("jsfx:{path}"),
        name: name.clone(),
        vendor: "JSFX".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        description: if desc.is_empty() { name } else { desc.to_string() },
        features: vec!["effect".to_string(), "jsfx".to_string()],
    }
}

fn parameters_for(sliders: &[super::SliderDef]) -> Vec<PluginParameter> {
    sliders
        .iter()
        .map(|def| PluginParameter {
            // Slider 1 är parameter 0, som i `slider1..64` → minnesplats 0–63.
            id: def.index.saturating_sub(1) as u32,
            name: if def.name.is_empty() {
                format!("slider{}", def.index)
            } else {
                def.name.clone()
            },
            module: String::new(),
            min_value: def.min,
            max_value: def.max,
            default_value: def.default,
            flags: if def.choices.is_empty() {
                0
            } else {
                PARAM_IS_STEPPED
            },
        })
        .collect()
}

struct JsfxProcessor {
    vm: Jsfx,
    /// Noter som kommit sedan förra blocket. Töms i `process_stereo`.
    pending: Vec<MidiEvent>,
    info: PluginInfo,
    parameters: Vec<PluginParameter>,
}

impl PluginProcessor for JsfxProcessor {
    fn backend(&self) -> &'static str {
        "JSFX"
    }

    fn info(&self) -> &PluginInfo {
        &self.info
    }

    fn parameters(&self) -> &[PluginParameter] {
        &self.parameters
    }

    /// En JSFX har ingen egen latensrapport; blockbufferten är värdsidans.
    fn latency_frames(&self) -> u32 {
        0
    }

    fn process_stereo(&mut self, left: &mut [f32], right: &mut [f32]) {
        // Noterna som kom sedan förra blocket ligger först i kön; motorn tar dem på offset 0.
        let notes = std::mem::take(&mut self.pending);
        // Felet ligger kvar i `vm.errors` och effekten har tystnat — kedjan ska inte stanna.
        let _ = self.vm.process(left, right, &notes);
    }

    /// Ett JSFX är instrument när filen vill ha noter. Vi vet inte det förrän det körts, och en
    /// MIDI-effekt vill också ha noter — så svaret är ja, och en effekt som inte lyssnar på
    /// `midirecv` struntar i dem.
    fn accepts_notes(&self) -> bool {
        true
    }

    fn note_on(&mut self, note: u8, velocity: u8) {
        self.pending.push(MidiEvent { offset: 0, status: 0x90, data1: note, data2: velocity });
    }

    fn note_off(&mut self, note: u8) {
        self.pending.push(MidiEvent { offset: 0, status: 0x80, data1: note, data2: 0 });
    }

    fn set_parameter(&mut self, id: u32, value: f64) -> bool {
        // `set_slider` kör `@slider`, som REAPER gör när man vrider.
        self.vm.set_slider(id as usize + 1, value).is_ok()
    }

    /// Nollställer effekten genom att köra `@init` igen — samma som att ta bort och lägga tillbaka
    /// den, vilket är vad `reset` betyder för en delay- eller reverbkedja.
    fn reset(&mut self) {
        let _ = self.vm.reset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// En effekt med en slider, skriven till en temporär fil: provet ska gå genom samma väg som
    /// effektkedjan använder, inte genom `Jsfx::compile` direkt.
    fn temp_jsfx(name: &str, source: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("sonix-jsfx-{name}-{}.jsfx", std::process::id()));
        std::fs::write(&path, source).expect("skriver provfil");
        path
    }

    #[test]
    fn a_jsfx_file_loads_where_a_plugin_would_and_processes_audio() {
        let path = temp_jsfx(
            "gain",
            "desc:Gain test\nslider1:0.5<0,1,0.01>Gain\n@slider\ng = slider1;\n@sample\nspl0 = spl0 * g;\nspl1 = spl1 * g;\n",
        );
        let mut processor =
            load(path.to_str().unwrap(), 44_100.0).expect("ska laddas som processor");
        assert_eq!(processor.backend(), "JSFX");
        assert_eq!(processor.parameters().len(), 1);
        let param = &processor.parameters()[0];
        assert_eq!(param.name, "Gain");
        assert!(param.max_value == 1.0);

        let mut left = vec![1.0f32; 8];
        let mut right = vec![1.0f32; 8];
        processor.process_stereo(&mut left, &mut right);
        assert!(left.iter().all(|v| (v - 0.5).abs() < 1e-6), "{left:?}");

        assert!(processor.set_parameter(0, 0.25));
        let mut left = vec![1.0f32; 8];
        let mut right = vec![1.0f32; 8];
        processor.process_stereo(&mut left, &mut right);
        assert!(left.iter().all(|v| (v - 0.25).abs() < 1e-6), "{left:?}");

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn a_broken_jsfx_is_an_error_not_a_panic() {
        let path = temp_jsfx("broken", "@sample\nspl0 = ((( ;\n");
        assert!(load(path.to_str().unwrap(), 44_100.0).is_err());
        let _ = std::fs::remove_file(path);
    }

    /// Ett instrument i kedjan ska låta först när det får en not — och tystna när noten släpps.
    /// Provet går genom `PluginProcessor`, som är den väg kedjan använder.
    #[test]
    fn a_jsfx_instrument_plays_only_when_it_gets_a_note() {
        let path = temp_jsfx(
            "note-gate",
            "@init\ng = 0;\n@sample\nwhile (midirecv(offset, m1, m2, m3)) (\n  st = m1 & 240;\n  st == 144 ? g = 0.5;\n  st == 128 ? g = 0;\n);\nspl0 = g; spl1 = g;\n",
        );
        let mut proc = load(path.to_str().unwrap(), 44_100.0).expect("instrument");
        assert!(proc.accepts_notes(), "ett JSFX ska kunna ta noter");

        let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
        proc.process_stereo(&mut l, &mut r);
        assert!(l.iter().all(|s| *s == 0.0), "tyst utan not");

        proc.note_on(60, 100);
        let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
        proc.process_stereo(&mut l, &mut r);
        assert!(l[0] > 0.4, "noten ska höras i nästa block, fick {}", l[0]);

        proc.note_off(60);
        let (mut l, mut r) = ([0.0f32; 256], [0.0f32; 256]);
        proc.process_stereo(&mut l, &mut r);
        assert!(l.iter().all(|s| *s == 0.0), "tyst efter not av");
        let _ = std::fs::remove_file(path);
    }
}
