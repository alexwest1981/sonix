// En GUI-app ska inte öppna en konsolruta bredvid fönstret på Windows. Mätt på
// artefakten: utan det här stod det "Subsystem: Windows CUI" i den färdiga
// .exe-filen. I debug behålls konsolen, så att utskrifter och panik går att se.
// Innerattributet måste stå först i filen, före alla moduler.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod ai_service;
mod audio;
mod autosave;

// CLAP-värden använder X11 för plugin-fönster och memfd för sandlådan — båda
// är Linux-specifika. I stället för att falla på `libc`/X11 med kryptiska fel
// säger vi det rakt ut (Fas 7.1; porten är en egen uppgift).
#[cfg(all(feature = "plugin-host", not(target_os = "linux")))]
compile_error!(
    "plugin-host (CLAP-värd med X11-fönster och memfd-sandlåda) är Linux-only i den här versionen. Bygg utan --features plugin-host, eller se Fas 7.1 i ROADMAP.md."
);

mod follow;
mod formula;
mod groove;
mod i18n;
mod instruments;
mod midi_take;
mod musicxml;
mod note_capture;
mod piano_script;
mod riff;
mod rng;
mod paths;
mod platform;
mod selftest;
mod ui;

use audio::{AudioEngine, AudioSettings};
use ui::SonixApp;

/// `sonix --paths`: den kanoniska filkartan, utan att starta ljudmotor eller GUI.
fn print_paths() {
    let p = paths::paths();
    println!("Sonix – filkarta (Fas 6.0)");
    println!("  hem: {}", p.home().display());
    println!();
    for (label, path) in paths::table() {
        let mark = if path.exists() { '✓' } else { '·' };
        println!("  {mark} {label:<22} {}", path.display());
    }
    println!();
    println!("  ✓ = finns   · = skapas vid behov");
    println!("  Överstyr med SONIX_PROJECTS_DIR, SONIX_SAMPLES_DIR, SONIX_CONFIG_DIR,");
    println!("  SONIX_DATA_DIR, SONIX_STATE_DIR, SONIX_CACHE_DIR (annars följs XDG).");
}

/// One note into a plugin, wherever it lives. A sandboxed plugin belongs to the
/// worker process, so its notes go over the control plane; in process they go
/// straight to the instance. Returns false when the plugin is not an instrument.
#[cfg(feature = "plugin-host")]
fn plugin_note(
    processor: &mut dyn audio::plugin_host_live::PluginProcessor,
    host: &mut Option<audio::plugin_sandbox::SandboxHost>,
    on: bool,
    note: u8,
    velocity: u8,
) -> bool {
    use audio::plugin_sandbox::{SandboxRequest, SandboxResponse};
    if let Some(host) = host {
        return matches!(
            host.request(&SandboxRequest::Note { on, note, velocity }),
            Ok(SandboxResponse::Ok)
        );
    }
    if !processor.accepts_notes() {
        return false;
    }
    if on {
        processor.note_on(note, velocity);
    } else {
        processor.note_off(note);
    }
    true
}

/// `sonix --render-plugin <plugin> <seconds> <out.wav> [--sandbox]`: one note
/// through a real plugin, rendered offline and written as a stereo WAV.
///
/// The numbers are the point: a plugin that loads only proves the ABI, a
/// non-zero RMS proves its DSP ran. `--sandbox` renders the same note through
/// `plugin_sandbox`, i.e. in the worker process.
#[cfg(feature = "plugin-host")]
fn render_plugin(path: &str, seconds: f32, out: &str, sandbox: bool, sine: bool) -> Result<(), String> {
    use audio::plugin_host_live::PluginProcessor;
    use audio::plugin_sandbox::SandboxHost;

    const RATE: u32 = 48_000;
    const BLOCK: usize = 512;
    const NOTE: u8 = 60;
    const VELOCITY: u8 = 102; // ≈ 0.8 in 0..127
    const TONE_HZ: f32 = 440.0;
    const TONE_PEAK: f32 = 0.2;

    let frames = (seconds * RATE as f32).round() as usize;
    let mut left = vec![0.0f32; frames];
    let mut right = vec![0.0f32; frames];

    let (mut processor, mut host): (Box<dyn PluginProcessor>, Option<SandboxHost>) = if sandbox {
        let opened = audio::plugin_sandbox::open(path, RATE as f32, BLOCK as u32)?;
        (opened.processor, opened.host)
    } else {
        (
            audio::plugin_host_live::load_processor(path, RATE as f32, BLOCK as u32)?,
            None,
        )
    };

    // An effect is driven by audio, not by notes: `--sine` is what the old
    // `--render-plugin` always sent, kept because it is the only way to hear a
    // plugin that has no instrument in it.
    if sine {
        for i in 0..frames {
            let v = (i as f32 * TONE_HZ * std::f32::consts::TAU / RATE as f32).sin() * TONE_PEAK;
            left[i] = v;
            right[i] = v;
        }
    }

    let notes = plugin_note(&mut *processor, &mut host, true, NOTE, VELOCITY);
    let mut released = false;
    for start in (0..frames).step_by(BLOCK) {
        let end = (start + BLOCK).min(frames);
        // Note off in the last quarter, so the release is in the render too.
        if !released && start * 4 >= frames * 3 {
            plugin_note(&mut *processor, &mut host, false, NOTE, 0);
            released = true;
        }
        processor.render_block(&mut left[start..end], &mut right[start..end]);
    }

    let rms = ((left.iter().chain(right.iter()))
        .map(|s| (*s as f64) * (*s as f64))
        .sum::<f64>()
        / (2 * frames) as f64)
        .sqrt();
    let peak = left
        .iter()
        .chain(right.iter())
        .fold(0.0f32, |max, s| max.max(s.abs()));
    let interleaved: Vec<f32> = left
        .iter()
        .zip(right.iter())
        .flat_map(|(l, r)| [*l, *r])
        .collect();
    audio::wav_writer::write_pcm_f32_to_wav(out, &interleaved, RATE, 2)
        .map_err(|e| e.to_string())?;

    let backend = processor.backend();
    println!("📦 {path}");
    println!("  backend:    {backend}");
    println!(
        "  sandbox:    {}",
        if backend.contains("sandbox") { "yes" } else { "no" }
    );
    println!("  latency:    {} frames (reported)", processor.latency_frames());
    println!(
        "  notes:      {}",
        if notes {
            "note-on 60 vel 102"
        } else {
            "none — the plugin is not an instrument"
        }
    );
    if sine {
        println!("  input:      440 Hz sine at {TONE_PEAK}");
    }
    println!(
        "  frames:     {frames} ({:.2} s @ {RATE} Hz)",
        frames as f32 / RATE as f32
    );
    println!("  RMS:        {rms:.6}");
    println!("  peak:       {peak:.6}");
    println!("  wrote:      {out}");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Out-of-process plugin sandbox worker (Fas 4.5a): re-executed self with a
    // flag. This must run before any banner output so stdout stays a clean
    // protocol channel.
    #[cfg(feature = "plugin-host")]
    {
        let args: Vec<String> = std::env::args().collect();
        if let Some(pos) = args.iter().position(|a| a == audio::plugin_sandbox::WORKER_FLAG) {
            let result = audio::plugin_sandbox::serve_from_args(&args[pos + 1..]);
            if let Err(err) = result {
                eprintln!("{err}");
                std::process::exit(2);
            }
            return Ok(());
        }
    }

    // `sonix --version`: en rad om vilken build som körs, **före** fönstret och ljudenheten.
    // About-dialogen visar samma nummer, men först efter att GUI:t startat — den som fyller i en
    // felrapport vill ha raden först.
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!("Sonix Studio {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Filkartan skrivs ut före ljudmotorn så att kommandot fungerar utan ljudkort.
    if std::env::args().any(|a| a == "--paths") {
        print_paths();
        return Ok(());
    }
    // `sonix --selftest`: mäter det Windows-kriteriet som CI inte kan svara på — att
    // ljudenheten spelar och vad MIDI-ingången hittar. Fönsterlöst med flit: det som
    // bara en människa kan se står sist i utskriften, inte bland det som är mätt.
    // (Fönster på Windows-substantiv skriver till en omdirigering — se ROADMAP 7.1.)
    if std::env::args().any(|a| a == "--selftest") {
        // Tyst är standard (2026-09-15): testet slår kicken med flit, och ett autonomt
        // pass som körde det i tid och otid spelade trummor i rummet där Alex arbetade.
        // Mixen mäts lika hårt som förut; `--audible` är den medvetna vägen till att
        // höra slagen. Regeln bor i `selftest::audible_from_args` (med prov).
        let args: Vec<String> = std::env::args().collect();
        selftest::run(selftest::audible_from_args(&args))?;
        return Ok(());
    }
    // `sonix --ai-features <fil.wav>`: mäter de tre särdrag som gör att en fil ser AI-gjord ut i
    // en detektor — spektralt tak, brusgolv och dynamik. Inget omdöme, bara siffror.
    if let Some(pos) = std::env::args().position(|a| a == "--ai-features") {
        let Some(path) = std::env::args().nth(pos + 1) else {
            eprintln!("användning: sonix --ai-features <fil.wav> [bpm]");
            std::process::exit(2);
        };
        // Ett **angivet** tempo (`--ai-features fil.wav 120`) är den enda vägen till ett svar om
        // anslagen som går att lita på; utan det uppskattas tempot och raden märks som osäker.
        let bpm: Option<f32> = std::env::args().nth(pos + 2).and_then(|v| v.parse().ok());
        match crate::audio::wav_reader::load_wav_pcm(&path) {
            Ok((l, r, sr)) => {
                let f = crate::audio::ai_features::analyse(&l, &r, sr, bpm);
                println!("{path}");
                for line in f.lines() {
                    println!("  {line}");
                }
            }
            Err(err) => {
                eprintln!("kunde inte läsa {path}: {err}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // `sonix --jsfx-note <fil.jsfx> [not]`: spelar en ton genom ett JSFX-instrument och mäter
    // toppen. Frågan en port måste svara på är inte "tolkar vi filen?" utan "låter den?".
    if let Some(pos) = std::env::args().position(|a| a == "--jsfx-note") {
        let Some(path) = std::env::args().nth(pos + 1) else {
            eprintln!("användning: sonix --jsfx-note <fil.jsfx> [not]");
            std::process::exit(2);
        };
        let note: u8 = std::env::args()
            .nth(pos + 2)
            .and_then(|n| n.parse().ok())
            .unwrap_or(60);
        let mut vm = audio::jsfx::load(std::path::Path::new(&path), 44_100.0)?;
        for def in vm.slider_defs().to_vec() {
            let _ = vm.set_slider(def.index, def.default);
        }
        let seconds = 1.0;
        let frames = (44_100.0 * seconds) as usize;
        let mut left = vec![0.0f32; frames];
        let mut right = vec![0.0f32; frames];
        let started = std::time::Instant::now();
        let events = [
            audio::jsfx::MidiEvent {
                offset: 0,
                status: 0x90,
                data1: note,
                data2: 100,
            },
        ];
        let mut peak = 0.0f32;
        let mut peak_at = 0usize;
        let mut energy = 0.0f64;
        for chunk in 0..frames.div_ceil(512) {
            let start = chunk * 512;
            let end = (start + 512).min(frames);
            let midi: &[audio::jsfx::MidiEvent] = if chunk == 0 { &events } else { &[] };
            audio::jsfx::process(&mut vm, &mut left[start..end], &mut right[start..end], midi)
                .map_err(|e| e.to_string())?;
            for (i, v) in left[start..end].iter().enumerate() {
                energy += (*v as f64) * (*v as f64);
                if v.abs() > peak {
                    peak = v.abs();
                    peak_at = start + i;
                }
            }
        }
        let elapsed = started.elapsed().as_millis();
        let rms = (energy / frames as f64).sqrt();
        println!(
            "{}: {} slider, not {note}, topp {peak:.4} vid prov {peak_at}, rms {rms:.4}, {} prov på {} ms{}",
            std::path::Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default(),
            vm.slider_defs().len(),
            frames,
            elapsed,
            if vm.has_failed() {
                let first = vm.errors.first().map(|e| e.to_string()).unwrap_or_default();
                format!("  [fel: {first}]")
            } else if vm.stubs.is_empty() {
                String::new()
            } else {
                format!("  [stubbar: {}]", vm.stubs.iter().cloned().collect::<Vec<_>>().join(", "))
            }
        );
        return Ok(());
    }

    // `sonix --theme-palette [dark|light]`: skriver paletten som JSON med hex-färger. Det är
    // listan en temaskapare behöver — och svaret på "vilka nycklar finns?" utan att läsa koden.
    if std::env::args().any(|a| a == "--theme-palette") {
        let light = std::env::args().any(|a| a == "light");
        let palette = if light {
            crate::ui::theme::Palette::light()
        } else {
            crate::ui::theme::Palette::default()
        };
        let mut value = serde_json::to_value(&palette).map_err(|e| e.to_string())?;
        // Arrayer blir hex här: samma fil ska kunna sparas och läsas tillbaka för hand.
        if let Some(object) = value.as_object_mut() {
            for (key, entry) in object.iter_mut() {
                if key == "mode" {
                    continue;
                }
                if let Some(bytes) = entry.as_array() {
                    let channel = |i: usize| bytes.get(i).and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                    *entry = serde_json::Value::String(format!(
                        "#{:02x}{:02x}{:02x}{:02x}",
                        channel(0),
                        channel(1),
                        channel(2),
                        channel(3)
                    ));
                }
            }
        }
        println!(
            "{}",
            serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?
        );
        return Ok(());
    }

    // `sonix --separate <in.wav> <ut-mapp> [modell-id]`: kör stämsepareringen över en fil och
    // skriver varje stämma som wav. Utan det går en modell bara att pröva i fönstret, och frågan
    // "fungerar den här modellen?" ska gå att svara på utan att titta.
    if let Some(pos) = std::env::args().position(|a| a == "--separate") {
        let args: Vec<String> = std::env::args().skip(pos + 1).collect();
        if args.len() < 2 {
            eprintln!("användning: sonix --separate <in.wav> <ut-mapp> [modell-id]");
            std::process::exit(2);
        }
        let (input, out_dir) = (&args[0], &args[1]);
        let model = match args.get(2) {
            Some(id) => audio::neural_separator::MODELS
                .iter()
                .find(|m| m.id == id.as_str())
                .ok_or_else(|| {
                    let ids: Vec<&str> = audio::neural_separator::MODELS.iter().map(|m| m.id).collect();
                    format!("okänd modell '{id}'; välj bland {}", ids.join(", "))
                })?,
            None => audio::neural_separator::default_model(),
        };
        let (left, right, sample_rate) = audio::wav_reader::load_audio_pcm(input)?;
        println!(
            "{} ({}), {:.1} s, {} Hz — modell {}",
            input,
            audio::neural_separator::find_model_for(model)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "modellfil saknas".to_string()),
            left.len() as f32 / sample_rate as f32,
            sample_rate,
            model.id
        );
        let started = std::time::Instant::now();
        let result = audio::stem_separator::run_separation_with(
            model,
            &left,
            &right,
            sample_rate,
            &|_| {},
        );
        std::fs::create_dir_all(out_dir).map_err(|e| format!("{out_dir}: {e}"))?;
        for (index, stem) in result.stems.iter().enumerate() {
            let name = model.sources.get(index).copied().unwrap_or("stem");
            let mut interleaved = Vec::with_capacity(stem.left.len() * 2);
            for (l, r) in stem.left.iter().zip(stem.right.iter()) {
                interleaved.push(*l);
                interleaved.push(*r);
            }
            let path = std::path::Path::new(out_dir).join(format!("{index}-{name}.wav"));
            audio::wav_writer::write_pcm_f32_to_wav(
                &path.display().to_string(),
                &interleaved,
                sample_rate,
                2,
            )
            .map_err(|e| e.to_string())?;
            // Energin per stämma är svaret på "hände något?": en tyst stämma är en tom fil.
            let rms = (stem.left.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>()
                / stem.left.len().max(1) as f64)
                .sqrt();
            println!("  {}: rms {rms:.4} → {}", path.display(), stem.left.len());
        }
        println!(
            "{} stämmor på {} ms; neural: {}{}",
            result.stems.len(),
            started.elapsed().as_millis(),
            result.used_neural,
            match &result.fallback_reason {
                Some(reason) => format!("  (föll tillbaka: {reason})"),
                None => String::new(),
            }
        );
        return Ok(());
    }

    // `sonix --jsfx-dump <fil.jsfx> [sektion]`: sektionernas påståenden med nummer. Till för att
    // hitta det påstående ett felmeddelande pekar på.
    if let Some(pos) = std::env::args().position(|a| a == "--jsfx-dump") {
        let Some(path) = std::env::args().nth(pos + 1) else {
            eprintln!("användning: sonix --jsfx-dump <fil.jsfx> [init|slider|block|sample]");
            std::process::exit(2);
        };
        let bytes = std::fs::read(&path).map_err(|e| format!("{path}: {e}"))?;
        let source = String::from_utf8_lossy(&bytes);
        let program = audio::jsfx::parser::parse(&source).map_err(|e| e.to_string())?;
        let section = std::env::args().nth(pos + 2).unwrap_or_else(|| "init".to_string());
        let stmts = match section.as_str() {
            "init" => &program.init,
            "slider" => &program.slider,
            "block" => &program.block,
            "sample" => &program.sample,
            other => {
                eprintln!("okänd sektion '{other}'");
                std::process::exit(2);
            }
        };
        for (index, stmt) in stmts.iter().enumerate() {
            println!("{}: {:?}", index + 1, stmt);
        }
        return Ok(());
    }

    // `sonix --jsfx-list <mapp>`: vad som finns i ett JSFX-bibliotek och hur mycket av det vi
    // tolkar. Samma fråga som `--selftest` ställer om ljudet, men för effektbiblioteket.
    if let Some(pos) = std::env::args().position(|a| a == "--jsfx-list") {
        // Utan mapp läses biblioteket i användarens egen mapp.
        let dir = std::env::args()
            .nth(pos + 1)
            .unwrap_or_else(|| audio::jsfx::user_dir().display().to_string());
        let entries = audio::jsfx::list(std::path::Path::new(&dir));
        if entries.is_empty() {
            println!("inga JSFX-filer i {dir}");
            return Ok(());
        }
        let mut ok = 0usize;
        for entry in &entries {
            match audio::jsfx::load(&entry.path, 44_100.0) {
                Ok(vm) => {
                    ok += 1;
                    let desc = if entry.desc.is_empty() { vm.desc() } else { &entry.desc };
                    println!(
                        "✓ {:4} slider  {}{}",
                        entry.sliders.len(),
                        if desc.is_empty() { "(ingen desc)" } else { desc },
                        entry
                            .licence
                            .as_ref()
                            .map(|l| format!("  [{}]", l.chars().take(40).collect::<String>()))
                            .unwrap_or_default()
                    );
                }
                Err(e) => println!("✗ {}\n    {e}", entry.path.display()),
            }
        }
        println!(
            "\n{ok}/{} filer tolkas i {dir}",
            entries.len()
        );
        return Ok(());
    }

    // `sonix --jsfx-run <fil.jsfx> <in.wav> <ut.wav> [slider=värde ...]`: kör en JSFX-effekt över
    // en fil. Det här är beviset att en tolkad effekt gör samma sak som en inbyggd: in och ut är
    // ljudfiler, och resultatet går att mäta.
    if let Some(pos) = std::env::args().position(|a| a == "--jsfx-run") {
        let args: Vec<String> = std::env::args().skip(pos + 1).collect();
        if args.len() < 3 {
            eprintln!("användning: sonix --jsfx-run <fil.jsfx> <in.wav> <ut.wav> [slider=värde ...]");
            std::process::exit(2);
        }
        let (effect, input, output) = (&args[0], &args[1], &args[2]);
        let (mut left, mut right, sample_rate) = audio::wav_reader::load_audio_pcm(input)?;
        let mut vm = audio::jsfx::load(std::path::Path::new(effect), sample_rate as f64)?;
        // Sliderar kan sättas från kommandoraden; de som inte nämns behåller filens standard.
        for (index, def) in vm.slider_defs().to_vec().iter().enumerate() {
            let _ = vm.set_slider(def.index, def.default);
            let _ = index;
        }
        for arg in &args[3..] {
            if let Some((key, value)) = arg.split_once('=') {
                if let (Ok(index), Ok(value)) = (key.trim().parse::<usize>(), value.trim().parse::<f64>()) {
                    vm.set_slider(index, value).map_err(|e| e.to_string())?;
                }
            }
        }
        // Block om 512 prov, som en ljudmotor.
        let mut energy_in = 0.0f64;
        let mut energy_out = 0.0f64;
        for chunk in 0..left.len().div_ceil(512) {
            let start = chunk * 512;
            let end = (start + 512).min(left.len());
            energy_in += left[start..end].iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>();
            audio::jsfx::process(
                &mut vm,
                &mut left[start..end],
                &mut right[start..end],
                &[],
            )
            .map_err(|e| e.to_string())?;
            energy_out += left[start..end].iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>();
        }
        if let Some(error) = vm.errors.first() {
            eprintln!("effekten kastade fel: {error}");
        }
        // Interleaved stereo, som skrivaren vill ha.
        let mut interleaved = Vec::with_capacity(left.len() * 2);
        for (l, r) in left.iter().zip(right.iter()) {
            interleaved.push(*l);
            interleaved.push(*r);
        }
        audio::wav_writer::write_pcm_f32_to_wav(output, &interleaved, sample_rate, 2)
            .map_err(|e| e.to_string())?;
        let ratio = if energy_in > 0.0 { (energy_out / energy_in).sqrt() } else { 0.0 };
        println!(
            "{} → {}: {} prov, energiförhållande ut/in {:.3}",
            effect,
            output,
            left.len(),
            ratio
        );
        return Ok(());
    }

    // `sonix --clean-tags <fil...>`: visar vad filerna bär och tar bort
    // AI-/Sunohärkomsten — inte musiken. Titel, artist, låttext och omslag lämnas
    // (Alex 2026-09-13: den första versionen tog hela taggen, och det var för
    // mycket). Ljudet rörs inte; bara härkomsten försvinner. (Att en kontroll
    // läser LJUDET och inte taggen står i metadata.rs — och är skälet att det här
    // inte är ett verktyg för att dölja något.)
    if let Some(pos) = std::env::args().position(|a| a == "--clean-tags") {
        let files: Vec<String> = std::env::args().skip(pos + 1).collect();
        if files.is_empty() {
            eprintln!("användning: sonix --clean-tags <fil> [fler filer]");
            std::process::exit(2);
        }
        let mut cleaned = 0usize;
        for f in &files {
            match audio::metadata::scan(f) {
                Ok(report) => {
                    for k in &report.kept {
                        println!("  {f} — lämnas: {k}");
                    }
                    for n in &report.notes {
                        println!("  {f} — ⚠ {n}");
                    }
                    if report.removable_bytes == 0 {
                        println!("  ren: {f} (ingen härkomst att ta)");
                        continue;
                    }
                    println!(
                        "  {f}: {} byte härkomst (ID3v2={} ID3v1={} RIFF={} FLAC={})",
                        report.removable_bytes,
                        report.has_id3v2,
                        report.has_id3v1,
                        report.has_riff_metadata,
                        report.has_flac_metadata
                    );
                    for e in &report.excerpts {
                        println!("      tas bort: {e}");
                    }
                    match audio::metadata::strip_tags(f) {
                        Ok(n) => {
                            println!("      ✅ {n} byte borttagna, ljudet orört");
                            cleaned += 1;
                        }
                        Err(e) => println!("      ⚠ {e}"),
                    }
                }
                Err(e) => println!("  ⚠ {e}"),
            }
        }
        println!("Städat: {cleaned} fil(er).");
        std::process::exit(0);
    }

    // `sonix --export-tutti <projekt.sonix|autosave> <utmapp>`: skriver en kopia av
    // projektet i det delade formatet tutti. `.sonix` är kvar som källan — det här är
    // vägen ut till andra program, inte ett formatbyte. Se skillens
    // `references/tutti-format.md` (mätt: `tutti check` säger 0 fel).
    if let Some(pos) = std::env::args().position(|a| a == "--export-tutti") {
        let args: Vec<String> = std::env::args().skip(pos + 1).collect();
        if args.len() < 2 {
            eprintln!("användning: sonix --export-tutti <projekt.sonix> <utmapp>");
            std::process::exit(2);
        }
        match ui::app::tutti_export::export_project_file(
            std::path::Path::new(&args[0]),
            std::path::Path::new(&args[1]),
        ) {
            Ok(r) => {
                println!(
                    "tutti: {} spår, {} klipp, {} ljudfiler ({} MB) → {}",
                    r.tracks,
                    r.clips,
                    r.audio_files,
                    r.bytes / 1_048_576,
                    r.folder.display()
                );
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("tutti-export misslyckades: {e}");
                std::process::exit(1);
            }
        }
    }

    // `sonix --import-tutti <mapp|.tutti> [ut.sonix]`: läser en tutti-folder och skriver en
    // `.sonix`-fil som appen öppnar som vanligt. Motsatsen till `--export-tutti`; `.sonix`
    // är kvar som det egna formatet.
    if let Some(pos) = std::env::args().position(|a| a == "--import-tutti") {
        let args: Vec<String> = std::env::args().skip(pos + 1).collect();
        if args.is_empty() {
            eprintln!("användning: sonix --import-tutti <tutti-mapp|fil.tutti> [ut.sonix]");
            std::process::exit(2);
        }
        let src = std::path::Path::new(&args[0]);
        let out = match args.get(1) {
            Some(path) => std::path::PathBuf::from(path),
            None => {
                let name = src
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Importerat");
                paths::paths().project_file(name)
            }
        };
        match ui::app::tutti_import::import_tutti(src, &out) {
            Ok(r) => {
                println!(
                    "sonix: {} spår, {} klipp → {}",
                    r.tracks,
                    r.clips,
                    r.out_file.display()
                );
                for w in &r.warnings {
                    println!("  ⚠ {w}");
                }
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("tutti-import misslyckades: {e}");
                std::process::exit(1);
            }
        }
    }

    // `sonix --export-video <klipp|bild> <ljud.wav> <ut.mp4> [fps]`: writes one MP4 from a
    // video or still image plus an audio file. Frames follow the audio length; a short clip
    // holds its last frame. `finish` is bounded, so a stuck encoder cannot hang the process.
    if let Some(pos) = std::env::args().position(|a| a == "--export-video") {
        let args: Vec<String> = std::env::args().skip(pos + 1).collect();
        if args.len() < 3 {
            eprintln!("usage: sonix --export-video <clip-or-image> <audio.wav> <out.mp4> [fps]");
            std::process::exit(2);
        }
        let fps = args.get(3).and_then(|f| f.parse::<f64>().ok()).unwrap_or(25.0);
        match audio::video_export::export_clip(
            &args[0],
            &args[1],
            &args[2],
            fps,
            &mut audio::video_fx::VideoFxChain::default(),
        ) {
            Ok(frames) => {
                println!("exported {frames} frames at {fps} fps to {}", args[2]);
                std::process::exit(0);
            }
            Err(err) => {
                eprintln!("--export-video: {err}");
                std::process::exit(1);
            }
        }
    }

    // `sonix --export-ddp <fil.wav> <utmapp> [titel:start_sampel[:isrc] ...]`: writes a
    // Red Book DDP 2.00 image from one 44.1 kHz stereo WAV. `--export-cue` writes the
    // same program as CUE/BIN instead. Neither burns anything — the image is data.
    if let Some(pos) = std::env::args().position(|a| a == "--export-ddp" || a == "--export-cue") {
        let cue = std::env::args().nth(pos).as_deref() == Some("--export-cue");
        let flag = if cue { "--export-cue" } else { "--export-ddp" };
        let args: Vec<String> = std::env::args().skip(pos + 1).collect();
        if args.len() < 2 {
            eprintln!("usage: sonix {flag} <file.wav> <out-dir> [title:start_sample[:isrc] ...]");
            std::process::exit(2);
        }
        let mut tracks: Vec<audio::ddp::TrackSpec> = Vec::new();
        for spec in &args[2..] {
            let mut parts = spec.splitn(3, ':');
            let (Some(title), Some(start)) = (parts.next(), parts.next()) else {
                eprintln!("{flag}: \"{spec}\" is not title:start_sample[:isrc]");
                std::process::exit(2);
            };
            let Ok(start_sample) = start.parse::<u64>() else {
                eprintln!("{flag}: \"{start}\" in \"{spec}\" is not a sample count");
                std::process::exit(2);
            };
            tracks.push(audio::ddp::TrackSpec {
                title: title.to_string(),
                start_sample,
                isrc: parts.next().map(str::to_string),
            });
        }
        if tracks.is_empty() {
            let stem = std::path::Path::new(&args[0])
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            tracks.push(audio::ddp::TrackSpec {
                title: stem,
                start_sample: 0,
                isrc: None,
            });
        }
        let meta = audio::exporter::ExportMeta::default();
        let written = if cue {
            audio::ddp::export_cue_bin(&args[0], &args[1], &tracks, &meta)
        } else {
            audio::ddp::export_ddp(&args[0], &args[1], &tracks, &meta)
        };
        match written {
            Ok(r) => {
                println!(
                    "{}: {} track(s), {} sectors ({} frames) → {}",
                    if cue { "CUE/BIN" } else { "DDP" },
                    r.tracks,
                    r.sectors,
                    r.frames,
                    args[1]
                );
                for f in &r.files {
                    println!("  {f}");
                }
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("{flag} failed: {e}");
                std::process::exit(1);
            }
        }
    }

    // `sonix --detect-bpm <fil...>`: uppskattar tempot ur en ljudfil. Verktyget finns
    // för att analysen ska kunna MÄTAS mot riktiga stämmor innan den får styra ett
    // projekt — ett gissat tempo ställer hela låten fel.
    if let Some(pos) = std::env::args().position(|a| a == "--detect-bpm") {
        let files: Vec<String> = std::env::args().skip(pos + 1).collect();
        if files.is_empty() {
            eprintln!("användning: sonix --detect-bpm <fil> [fler filer]");
            std::process::exit(2);
        }
        for f in &files {
            match audio::stem_separator::detect_bpm_from_file(f) {
                Ok(bpm) => println!("  {bpm:7.2} BPM  {}", f),
                Err(e) => println!("  ⚠ {f}: {e}"),
            }
        }
        std::process::exit(0);
    }



    // `sonix --inspect-plugin <fil...>`: öppnar en plugin **huvudlöst** och skriver ut vad
    // den rapporterar — identitet, portar, latens och parametrar. Verktyget finns för att
    // Fas 8.6 ska kunna MÄTAS mot en riktig, nedladdad plugin i stället för bara mot vår egen
    // mock: pluginens egna portar (sidokedja in, egna utbussar) och dess latens går inte att
    // gissa, och en gissning hade stämt för mocken och varit fel för allt annat.
    //
    // **Detta är den enda vägen som öppnar en plugin utan fönster** — en krasch tar därför
    // kommandot med sig (och syns som en utgångskod), vilket är ärligare än att en skanning
    // ser ut att ha lyckats. I appen går samma plugin genom sandlådan i stället.
    if let Some(pos) = std::env::args().position(|a| a == "--inspect-plugin") {
        let files: Vec<String> = std::env::args().skip(pos + 1).collect();
        if files.is_empty() {
            eprintln!("användning: sonix --inspect-plugin <plugin.clap|plugin.vst3|mapp> [...]");
            std::process::exit(2);
        }
        let mut failed = 0usize;
        for f in &files {
            match audio::plugin_host_live::load_processor(f, 48_000.0, 512) {
                Ok(processor) => {
                    let info = processor.info();
                    println!("📦 {}", f);
                    println!("  backend:    {}", processor.backend());
                    println!("  namn:       {} ({})", info.name, info.id);
                    println!("  tillverkare:{} v{}", info.vendor, info.version);
                    println!("  latens:     {} ramar", processor.latency_frames());
                    // Portarna, i pluginens egen ordning — det är den enda källan till svaret
                    // på "hur många utbussar har den?" (Fas 8.6:s multi-out).
                    let ports = processor.port_layout();
                    if ports.is_empty() {
                        println!("  portar:     (ingen uppgift från backend)");
                    } else {
                        let ins = ports.iter().filter(|p| p.0).count();
                        let outs = ports.len() - ins;
                        println!("  portar:     {ins} in, {outs} ut");
                        for (is_input, channels, kind) in &ports {
                            println!(
                                "    {:<3} {:<9} {channels} kanaler",
                                if *is_input { "in" } else { "ut" },
                                kind
                            );
                        }
                    }
                    // Fas 8.6: det här är siffrorna som avgör om en plugin kan köras i en
                    // sidokedja eller ha egna utbussar — lästa ur pluginens egen beskrivning.
                    println!(
                        "  sidokedja:  {} ingång(ar)",
                        processor.sidechain_inputs()
                    );
                    println!("  egna utbussar: {}", processor.extra_outputs());
                    let params = processor.parameters();
                    println!("  parametrar: {} (automatiserbara: {})", params.len(), params.iter().filter(|p| p.is_automatable()).count());
                    for p in params.iter().take(8) {
                        println!(
                            "    {:>4}  {:<28} {} .. {} (standard {})",
                            p.id, p.name, p.min_value, p.max_value, p.default_value
                        );
                    }
                    if params.len() > 8 {
                        println!("    ... {} fler", params.len() - 8);
                    }
                }
                Err(e) => {
                    println!("⚠ {}: {e}", f);
                    failed += 1;
                }
            }
        }
        std::process::exit(if failed == 0 { 0 } else { 1 });
    }

    // `sonix --render-plugin <plugin> <sekunder> <ut.wav> [--sandbox]`: skickar en not
    // genom en **riktig** plugin utan GUI och skriver det den spelar. En fil som laddar
    // säger bara att ABI:n stämmer — att det kommer ljud säger att DSP:n faktiskt kör.
    // `--sandbox` kör samma render i arbetsprocessen i stället för här.
    // Utan `plugin-host` finns ingen VST3-motor i binären; säg det i stället för att tiga.
    #[cfg(not(feature = "plugin-host"))]
    if std::env::args().any(|a| a == "--render-plugin") {
        eprintln!("⚠ --render-plugin kräver en build med --features plugin-host");
        std::process::exit(1);
    }
    // `sonix --suno-clip <id>`: skriver ut alla nycklar i klattens metadata (letar bl.a. BPM).
    if let Some(pos) = std::env::args().position(|a| a == "--suno-clip") {
        let id = std::env::args().nth(pos + 1).unwrap_or_default();
        match audio::suno::SunoSession::connect_cached() {
            Ok(mut session) => match session.call("audio-retrieve", serde_json::json!({ "id": id })) {
                Ok(reply) => {
                    let text = reply
                        .get("content")
                        .and_then(|c| c.as_array())
                        .and_then(|i| i.first())
                        .and_then(|i| i.get("text"))
                        .and_then(|t| t.as_str())
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| reply.to_string());
                    let parsed: serde_json::Value =
                        serde_json::from_str(&text).unwrap_or(serde_json::Value::Null);
                    fn walk(v: &serde_json::Value, path: &str) {
                        match v {
                            serde_json::Value::Object(map) => {
                                for (k, sub) in map {
                                    walk(sub, &format!("{path}.{k}"));
                                }
                            }
                            serde_json::Value::Array(items) => {
                                for (i, sub) in items.iter().enumerate() {
                                    walk(sub, &format!("{path}[{i}]"));
                                }
                            }
                            other => {
                                let shown = other.to_string();
                                println!("{path} = {}", &shown[..shown.len().min(70)]);
                            }
                        }
                    }
                    walk(&parsed, "klatt");
                }
                Err(e) => eprintln!("⚠ {e}"),
            },
            Err(e) => eprintln!("⚠ {e}"),
        }
        std::process::exit(0);
    }

    // `sonix --suno-flow "<beskrivning>"`: hela kedjan — skapa, vänta, hämta filen. Kostar en
    // nedladdningskredit. Separationen sker i appen (den behöver projektet), men filen ska landa.
    if let Some(pos) = std::env::args().position(|a| a == "--suno-flow") {
        let prompt = std::env::args().nth(pos + 1).unwrap_or_default();
        match audio::suno::SunoSession::connect_cached() {
            Ok(mut session) => match session.generate(&prompt, None, None) {
                Ok(song) if !song.files.is_empty() => {
                    println!("klar utan hämtning: {} fil(er)", song.files.len());
                }
                Ok(song) => match song.clip_ids.first() {
                    Some(id) => match session.download_song_confirmed(id) {
                        Ok(done) => {
                            println!(
                                "hämtad: \"{}\" — {} fil(er), stämmor {}",
                                done.title,
                                done.files.len(),
                                done.stems
                            );
                            for f in &done.files {
                                println!("  fil: {} ({} byte)", f.display(), std::fs::metadata(f).map(|m| m.len()).unwrap_or(0));
                            }
                        }
                        Err(e) => eprintln!("⚠ hämtning: {e}"),
                    },
                    None => eprintln!("⚠ inga clip-id i svaret"),
                },
                Err(e) => eprintln!("⚠ {e}"),
            },
            Err(e) => eprintln!("⚠ {e}"),
        }
        std::process::exit(0);
    }

    // `sonix --suno-wait <id,id>`: väntar på redan beställda clips utan att skapa nya.
    if let Some(pos) = std::env::args().position(|a| a == "--suno-wait") {
        let ids: Vec<String> = std::env::args()
            .nth(pos + 1)
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        match audio::suno::SunoSession::connect_cached() {
            Ok(mut session) => match session
                .call("audio-wait", serde_json::json!({ "ids": ids }))
            {
                Ok(reply) => {
                    let shown = reply.to_string();
                    println!("väntan: {}", &shown[..shown.len().min(1500)]);
                }
                Err(e) => {
                    eprintln!("⚠ {e}");
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("⚠ {e}");
                std::process::exit(1);
            }
        }
        std::process::exit(0);
    }

    // `sonix --suno-create-args '<json>'`: skickar exakt den kroppen till audio-create. Används
    // för att ringa in vilket fält servern kallar "invalid request body".
    if let Some(pos) = std::env::args().position(|a| a == "--suno-create-args") {
        let text = std::env::args().nth(pos + 1).unwrap_or_default();
        let args: serde_json::Value = serde_json::from_str(&text).unwrap_or_default();
        let tool = std::env::args().nth(pos + 2).filter(|a| !a.starts_with("--"));
        match audio::suno::SunoSession::connect_cached() {
            Ok(mut session) => {
                println!("skickar: {args} till {}", tool.as_deref().unwrap_or("audio-create"));
                let outcome = match tool.as_deref() {
                    Some(name) => session.generate_named(name, args),
                    None => session.generate_with(args),
                };
                match outcome {
                    Ok(song) => println!("skapad: {}", song.title),
                    Err(e) => println!("svar: {e}"),
                }
            }
            Err(e) => eprintln!("⚠ {e}"),
        }
        std::process::exit(0);
    }

    // `sonix --suno-create "<beskrivning>"`: skapar en låt och väntar på att den blir klar.
    // Spenderar genereringskrediter men **inte** nedladdningskrediter (ingen hämtning).
    if let Some(pos) = std::env::args().position(|a| a == "--suno-create") {
        let prompt = std::env::args().nth(pos + 1).unwrap_or_default();
        match audio::suno::SunoSession::connect_cached() {
            Ok(mut session) => match session.generate(&prompt, None, None) {
                Ok(song) => {
                    println!("skapad: \"{}\" — ljud {} — stämmor {}", song.title, song.files.len(), song.stems);
                    for file in &song.files {
                        println!("  fil: {}", file.display());
                    }
                }
                Err(e) => {
                    eprintln!("⚠ {e}");
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("⚠ {e}");
                std::process::exit(1);
            }
        }
        std::process::exit(0);
    }

    // `sonix --suno-tools`: frågar den inloggade Suno-servern vad den erbjuder, utan att öppna
    // någon webbläsare. Svarar på "vilka fält finns?" innan panelen är byggd för dem.
    if std::env::args().any(|a| a == "--suno-tools") {
        match audio::suno::SunoSession::connect_cached() {
            Ok(mut session) => {
                println!("endpoint: {}", session.endpoint());
                let tools: Vec<String> = session.tools().iter().map(|t| t.name.clone()).collect();
                println!("verktyg ({}): {:?}", tools.len(), tools);
                if let Some(tool) = session.generation_tool() {
                    println!("generation: {}", tool.name);
                }
                if let Some(pos) = std::env::args().position(|a| a == "--suno-schema") {
                    // `--suno-schema <verktyg>` dumpar det verktygets schema, inte bara create.
                    let named = std::env::args().nth(pos + 1).filter(|a| !a.starts_with("--"));
                    let schema = match named.as_deref() {
                        Some(name) => session.tool_schema(name),
                        None => session.generation_schema(),
                    };
                    if let Some(schema) = schema {
                        println!("{}", serde_json::to_string_pretty(&schema).unwrap_or_default());
                    }
                    std::process::exit(0);
                }
                if false
                    && let Some(schema) = session.generation_schema()
                {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&schema).unwrap_or_default()
                    );
                    std::process::exit(0);
                }
                if let Some(schema) = session.generation_schema() {
                    if let Some(props) = schema.get("properties").and_then(|p| p.as_object()) {
                        for (name, spec) in props {
                            let kind = spec.get("type").and_then(|t| t.as_str()).unwrap_or("?");
                            let choices = spec
                                .get("enum")
                                .map(|e| format!(" {e}"))
                                .unwrap_or_default();
                            println!("  fält {name}: {kind}{choices}");
                            if let Some(nested) = spec.get("properties").and_then(|p| p.as_object()) {
                                for (sub, sub_spec) in nested {
                                    let sub_kind =
                                        sub_spec.get("type").and_then(|t| t.as_str()).unwrap_or("?");
                                    let sub_choices = sub_spec
                                        .get("enum")
                                        .map(|e| format!(" {e}"))
                                        .unwrap_or_default();
                                    println!("    {name}.{sub}: {sub_kind}{sub_choices}");
                                }
                            }
                        }
                    }
                }
                match session.voices() {
                    Ok(voices) => println!(
                        "voices ({}): {:?}",
                        voices.len(),
                        voices.iter().map(|v| v.name.clone()).collect::<Vec<_>>()
                    ),
                    Err(e) => println!("voices: {e}"),
                }
                if let Some(hint) = session.voices_hint() {
                    println!("{hint}");
                }
                match session.playlists() {
                    Ok(list) => {
                        println!("spellistor ({}):", list.len());
                        for p in &list {
                            println!("  {} — {} — {} spår", p.name, p.id, p.clip_count.unwrap_or(0));
                        }
                    }
                    Err(e) => println!("spellistor: {e}"),
                }
                match session.workspaces() {
                    Ok(list) => {
                        println!("arbetsytor ({}):", list.len());
                        for w in &list {
                            println!("  {} — {}", w.name, w.id);
                        }
                    }
                    Err(e) => println!("arbetsytor: {e}"),
                }
                match session.account() {
                    Ok(acc) => {
                        println!("konto: plan {:?}, credits {:?}", acc.plan, acc.credits);
                        let raw = acc.raw.to_string();
                        println!("konto rått: {}", &raw[..raw.len().min(1200)]);
                    }
                    Err(e) => println!("konto: {e}"),
                }
                match session.library(None, 5) {
                    Ok(songs) => {
                        println!("bibliotek ({} första):", songs.len());
                        for song in &songs {
                            println!(
                                "  {} — id {} — ljud {} — stämmor {}",
                                song.title,
                                song.id,
                                song.audio_urls.len(),
                                song.stem_urls.len()
                            );
                        }
                        // Hämtar den första låtens riktiga URL:er — det Importera behöver.
                        if let Some(first) = songs.first() {
                            match session.retrieve_song(&first.id) {
                                Ok(full) => println!(
                                    "  hämtad: \"{}\" — ljud {} — stämmor {}",
                                    full.title,
                                    full.audio_urls.len(),
                                    full.stem_urls.len()
                                ),
                                Err(e) => println!("  hämtning: {e}"),
                            }
                            // Om formen inte gav några URL:er: visa råsvaret så nästa parser blir rätt.
                            if let Some(raw) = session
                                .call(
                                    "audio-retrieve",
                                    serde_json::json!({ "id": first.id }),
                                )
                                .ok()
                            {
                                // Alla nycklar vars namn innehåller "url" — det är de vi behöver.
                                fn walk(value: &serde_json::Value, path: &str) {
                                    match value {
                                        serde_json::Value::Object(map) => {
                                            for (key, sub) in map {
                                                let here = format!("{path}.{key}");
                                                if key.to_lowercase().contains("url") {
                                                    let text = sub.to_string();
                                                    println!("  url {here} = {}", &text[..text.len().min(90)]);
                                                }
                                                walk(sub, &here);
                                            }
                                        }
                                        serde_json::Value::Array(items) => {
                                            for (i, sub) in items.iter().enumerate() {
                                                walk(sub, &format!("{path}[{i}]"));
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                // `content[].text` är en JSON-sträng; packa upp den först.
                                let inner = raw
                                    .get("content")
                                    .and_then(|c| c.as_array())
                                    .and_then(|items| items.first())
                                    .and_then(|item| item.get("text"))
                                    .cloned()
                                    .unwrap_or(serde_json::Value::Null);
                                let parsed = match inner.as_str() {
                                    Some(text) => serde_json::from_str(text)
                                        .unwrap_or(serde_json::Value::Null),
                                    None => inner,
                                };
                                walk(&parsed, "svar");
                                if let Ok(dl) = session.call(
                                    "audio-download",
                                    serde_json::json!({ "id": first.id }),
                                ) {
                                    let inner = dl
                                        .get("content")
                                        .and_then(|c| c.as_array())
                                        .and_then(|i| i.first())
                                        .and_then(|i| i.get("text"))
                                        .cloned()
                                        .unwrap_or(serde_json::Value::Null);
                                    let parsed = match inner.as_str() {
                                        Some(text) => serde_json::from_str(text)
                                            .unwrap_or(serde_json::Value::Null),
                                        None => inner,
                                    };
                                    walk(&parsed, "nedladdning");
                                    let shown = parsed.to_string();
                                    println!(
                                        "  nedladdning rått: {}",
                                        &shown[..shown.len().min(300)]
                                    );
                                }
                            }
                        }
                    }
                    Err(e) => println!("bibliotek: {e}"),
                }
            }
            Err(e) => {
                eprintln!("⚠ {e}");
                std::process::exit(1);
            }
        }
        std::process::exit(0);
    }

    #[cfg(feature = "plugin-host")]
    if let Some(pos) = std::env::args().position(|a| a == "--render-plugin") {
        let rest: Vec<String> = std::env::args().skip(pos + 1).collect();
        let sandbox = rest.iter().any(|a| a == "--sandbox");
        let sine = rest.iter().any(|a| a == "--sine");
        let args: Vec<&String> = rest
            .iter()
            .filter(|a| *a != "--sandbox" && *a != "--sine")
            .collect();
        let seconds = args
            .get(1)
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|s| *s > 0.0);
        let (Some(path), Some(seconds), Some(out)) = (args.first(), seconds, args.get(2)) else {
            eprintln!(
                "usage: sonix --render-plugin <plugin> <seconds> <out.wav> [--sandbox] [--sine]"
            );
            std::process::exit(2);
        };
        if let Err(e) = render_plugin(path, seconds, out, sandbox, sine) {
            eprintln!("⚠ {path}: {e}");
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    // `sonix --macro-example <fil>`: skriver en färdig startkedja. Kedjan är en fil man kan
    // rätta för hand, så kommandot behövs för att komma igång utan att kunna filformen.
    if let Some(pos) = std::env::args().position(|a| a == "--macro-example") {
        let rest: Vec<String> = std::env::args().skip(pos + 1).collect();
        let Some(path) = rest.first() else {
            eprintln!("användning: sonix --macro-example <fil.json>");
            std::process::exit(2);
        };
        let chain = audio::macro_chain::example_chain();
        match audio::macro_chain::save_chain(std::path::Path::new(path), &chain) {
            Ok(()) => {
                println!("Skrev startkedjan \"{}\" till {path}:", chain.name);
                for step in &chain.steps {
                    println!("  - {}", step.label());
                }
                std::process::exit(0);
            }
            Err(e) => {
                eprintln!("⚠ {e}");
                std::process::exit(2);
            }
        }
    }

    // `sonix --run-macro <kedja.json> <fil|mapp>... [--out <mapp>]` (Fas 8.9): kör en
    // sparad kedja över filer — samma behandling på många tagningar. Fönsterlöst med flit,
    // så en batch går att köra och MÄTAS från en terminal (och från CI, om vi vill).
    // Felkoderna är en del av kontraktet: 2 = användningsfel, 1 = någon fil föll.
    if let Some(pos) = std::env::args().position(|a| a == "--run-macro") {
        let rest: Vec<String> = std::env::args().skip(pos + 1).collect();
        let mut out_dir: Option<std::path::PathBuf> = None;
        let mut inputs: Vec<String> = Vec::new();
        let mut chain_path: Option<String> = None;
        let mut i = 0usize;
        while i < rest.len() {
            match rest[i].as_str() {
                "--out" => {
                    i += 1;
                    if i >= rest.len() {
                        eprintln!("--out behöver en mapp");
                        std::process::exit(2);
                    }
                    out_dir = Some(std::path::PathBuf::from(&rest[i]));
                }
                arg => {
                    if chain_path.is_none() {
                        chain_path = Some(arg.to_string());
                    } else {
                        inputs.push(arg.to_string());
                    }
                }
            }
            i += 1;
        }
        let Some(chain_path) = chain_path else {
            eprintln!("användning: sonix --run-macro <kedja.json> <fil|mapp>... [--out <mapp>]");
            std::process::exit(2);
        };
        let chain = match audio::macro_chain::load_chain(&chain_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("kedjan gick inte att läsa: {e}");
                std::process::exit(2);
            }
        };
        // En mapp blir sina ljudfiler. En fil blir sig själv. En sökväg som inte finns
        // behålls med flit: då blir den ett **fel i loggen** i stället för en tyst tom körning.
        let mut files: Vec<String> = Vec::new();
        for input in &inputs {
            let path = std::path::Path::new(input);
            if path.is_dir() {
                match audio::macro_chain::audio_files_in(path) {
                    Ok(found) => files.extend(found),
                    Err(e) => {
                        eprintln!("⚠ {e}");
                        std::process::exit(2);
                    }
                }
            } else {
                files.push(input.clone());
            }
        }
        if files.is_empty() {
            eprintln!("användning: sonix --run-macro <kedja.json> <fil|mapp>... [--out <mapp>]");
            std::process::exit(2);
        }
        if !chain.has_export() {
            // En kedja utan exportsteg gör ingenting som syns. Att köra den tyst vore att
            // lämna ett pass som ser lyckat ut utan att ha skrivit något.
            eprintln!(
                "kedjan \"{}\" har inget exportsteg — den skriver ingenting. Lägg till ett export-steg.",
                chain.name
            );
            std::process::exit(2);
        }
        println!("Makro: {} ({} steg)", chain.name, chain.steps.len());
        for step in &chain.steps {
            println!("  - {}", step.label());
        }
        println!("Filer: {}", files.len());
        let meta = audio::exporter::ExportMeta::default();
        let runs = audio::macro_chain::run_batch(&chain, &files, out_dir.as_deref(), &meta);
        for run in &runs {
            println!("\n{}", run.input);
            for r in &run.reports {
                println!("  {}: {}", r.step, r.detail);
            }
            if let Some(e) = &run.error {
                println!("  ⚠ {e}");
            }
        }
        let ok = audio::macro_chain::succeeded(&runs);
        let failed = runs.len() - ok;
        match audio::macro_chain::write_batch_log(
            &chain,
            &runs,
            std::path::Path::new(&runs[0].input),
            out_dir.as_deref(),
        ) {
            Ok(path) => println!("\nLogg: {path}"),
            Err(e) => println!("\n⚠ loggen kunde inte skrivas: {e}"),
        }
        println!("Klara: {ok} av {} ({failed} med fel)", runs.len());
        std::process::exit(if failed > 0 { 1 } else { 0 });
    }


    // Fas 6.0: flytta äldre platser hit FÖRST (ensure_dirs skapar annars ett tomt
    // mål, vilket skulle blockera flytten), därefter skapa resten.
    let path_notes = paths::migrate();
    for (dir, err) in paths::paths().ensure_dirs() {
        eprintln!("⚠ Kunde inte skapa {}: {err}", dir.display());
    }

    // Install a robust panic hook that logs detailed backtrace to stderr and /tmp/sonix_crash.log
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = std::backtrace::Backtrace::capture();
        eprintln!("💥 [SONIX CRASH CAUGHT] {}", panic_info);
        eprintln!("Stack Backtrace:\n{}", backtrace);
        if let Ok(mut f) = std::fs::File::create("/tmp/sonix_crash.log") {
            use std::io::Write;
            let _ = writeln!(f, "=== SONIX CRASH REPORT ===");
            let _ = writeln!(f, "Panic: {}", panic_info);
            let _ = writeln!(f, "Backtrace:\n{}", backtrace);
        }
    }));

    println!("=========================================================");
    println!("  🎹 SONIX - Native Linux Digital Audio Workstation     ");
    println!("=========================================================");
    println!("🔊 Starting PipeWire / ALSA audio engine...");

    let audio_settings = AudioSettings::load();
    let engine = AudioEngine::new_with(
        512,
        audio_settings.sample_rate,
        audio_settings.buffer_frames,
    )?;

    println!("  • Audio device: {}", engine.device_name);
    println!("  • Sample rate:       {} Hz", engine.sample_rate);
    if let Some(frames) = engine.buffer_frames {
        println!("  • Buffer size:       {} frames", frames);
    }
    println!("  • Channels:    Stereo");

    // `sonix --web [port]`: a loopback HTTP control surface (default 8787). It starts
    // before the window so the printed token is the first thing in the log.
    let web_port: Option<u16> = {
        let args: Vec<String> = std::env::args().collect();
        args.iter().position(|a| a == "--web").map(|pos| {
            args.get(pos + 1).and_then(|p| p.parse().ok()).unwrap_or(8787)
        })
    };
    let web = match web_port {
        Some(port) => match audio::web_control::WebServer::start(port) {
            Ok(server) => {
                println!("🌐 Web control: {}", server.url());
                println!("   token: {} (every request must carry it)", server.token);
                Some(server)
            }
            Err(e) => {
                eprintln!("⚠ Web control could not bind 127.0.0.1:{port}: {e}");
                None
            }
        },
        None => None,
    };

    println!("🚀 Starting graphical interface (GUI)...");

    let args: Vec<String> = std::env::args().collect();
    let screenshot_dir = if let Some(idx) = args.iter().position(|a| a == "--capture-screenshots" || a == "--screenshots" || a == "-s") {
        args.get(idx + 1).cloned().unwrap_or_else(|| "screenshots".to_string())
    } else {
        String::new()
    };

    let cli_arg = if screenshot_dir.is_empty() {
        args.get(1).cloned()
    } else {
        None
    };

    let is_fullscreen = !screenshot_dir.is_empty() || args.contains(&"--fullscreen".to_string());

    let viewport = if is_fullscreen {
        eframe::egui::ViewportBuilder::default()
            .with_fullscreen(true)
            .with_inner_size([1920.0, 1200.0])
            .with_title("🎹 Sonix DAW - Linux Audio Workstation")
            .with_app_id("sonix-daw")
    } else {
        eframe::egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            // **1024 × 600, inte 780 × 540** (EGM 3, fynd 8): browser 200 + dock 320 +
            // spårhuvud 228 lämnade 32 px tidslinje vid 780, alltså ett arrangemang man
            // bara kunde scrolla blint i. 1024 ger 276 px, och docken får krympa till 280.
            .with_min_inner_size([1024.0, 600.0])
            .with_title("🎹 Sonix DAW - Linux Audio Workstation")
            .with_app_id("sonix-daw")
    };

    let viewport = viewport.with_icon(
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/sonix.png"))
            .expect("embedded app icon (assets/sonix.png) must be a valid PNG"),
    );

    let options = eframe::NativeOptions {
        viewport,
        // **Utan vsync.** Med vsync väntar `present` på att compositorn skickar nästa
        // bildrute-callback, och på Wayland kommer den inte när ytan är dold, minimerad eller
        // flyttad mellan skärmar — då står UI-tråden still och fönstret blir "not responding".
        // Hängproben mätte sex sådana blockeringar i en session (Alex 2026-11-04).
        //
        // Farten styrs i stället av appens egna `request_repaint_after` (16/33/200 ms), så
        // detta kostar ingen CPU. Priset är att X11 kan riva bilden (tearing); Wayland kan
        // inte det — compositorn synkar ändå.
        vsync: false,
        ..Default::default()
    };

    eframe::run_native(
        "Sonix DAW",
        options,
        Box::new(move |cc| {
            // **Typsnitten sätts här, inte i `update`.** Ikonerna behöver ett monokromt
            // emoji-typsnitt, och `set_fonts` bygger om glyfatlasen — den får bara köras en gång.
            crate::ui::fonts::install(cc);
            let mut app = SonixApp::new(engine);
            app.web = web;
            // **Suno-inloggningen kontrolleras i bakgrunden vid start — utan webbläsare.**
            // Sparade tokens används, access-token förnyas ur refresh-token om den gått ut.
            // Går det inte står det i inställningarna; ingenting öppnas av sig själv.
            app.suno_connect_silently();
            // Fas 6.0: berätta en gång om något flyttades eller läses från en
            // äldre plats — annars undrar användaren var filerna tog vägen.
            if !path_notes.is_empty() {
                for note in &path_notes {
                    println!("{note}");
                }
                app.status_message = path_notes.join("   ");
            }
            if !screenshot_dir.is_empty() {
                println!("📸 Enabling automatic screenshot capture to: {}", screenshot_dir);
                app.enable_screenshot_mode(std::path::Path::new(&screenshot_dir));
            } else if let Some(ref arg) = cli_arg {
                if arg.to_lowercase().ends_with(".zip") {
                    app.import_suno_zip(arg);
                } else if std::path::Path::new(arg).is_dir() {
                    let (title, bpm) = SonixApp::parse_suno_zip_info(arg);
                    app.import_suno_stems_from_folder(arg, &title, bpm);
                }
            }
            Ok(Box::new(app))
        }),
    ).map_err(|e| format!("Could not start the window: {}", e).into())
}

#[cfg(all(test, feature = "plugin-host"))]
mod render_plugin_tests {
    use super::render_plugin;

    /// One note through the bridged Massive X, in process — the measurement the
    /// command exists for. Skips with a printed message where the plugin is not
    /// installed: the suite must not depend on a user's plugin folder.
    #[test]
    fn massive_x_renders_a_note_in_process() {
        let home = std::env::var("HOME").unwrap_or_default();
        let plugin = format!("{home}/.vst3/yabridge/Massive XL.vst3");
        if !std::path::Path::new(&plugin).exists() {
            println!("skipped: {plugin} is not installed");
            return;
        }
        let out = std::env::temp_dir().join("sonix_render_plugin_test.wav");
        let out = out.to_str().expect("a UTF-8 temp path");
        render_plugin(&plugin, 0.5, out, false, false).expect("the render must not fail");
        let written = std::fs::metadata(out).expect("the WAV must exist").len();
        assert!(written > 44, "expected a WAV header and audio, got {written} bytes");
        std::fs::remove_file(out).ok();
    }
}
