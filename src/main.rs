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
            // **LV2: säg sanningen i stället för att försöka som CLAP** (2026-09-15). Sonix har
            // ingen LV2-värd än; utan den här raden blev svaret `undefined symbol: clap_entry`,
            // vilket beskriver vår laddare i stället för användarens fil.
            if audio::plugin_host::is_lv2_path(f) {
                println!("⚠ {f}: LV2 stöds inte av Sonix än (ingen LV2-värd i programmet)");
                failed += 1;
                continue;
            }
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

    // `sonix --render-plugin <plugin> <ut.wav>`: kör en ton genom en **riktig** plugin utan
    // GUI och skriver resultatet. Det är kvittensen för bryggan: att en fil laddar säger bara
    // att ABI:n stämmer, att ljudet ändras säger att DSP:n faktiskt kör.
    if let Some(pos) = std::env::args().position(|a| a == "--render-plugin") {
        let path = match std::env::args().nth(pos + 1) {
            Some(p) => p,
            None => {
                eprintln!("användning: sonix --render-plugin <plugin.vst3|.clap> [ut.wav]");
                std::process::exit(2);
            }
        };
        let out = std::env::args()
            .nth(pos + 2)
            .unwrap_or_else(|| "plugin_render.wav".to_string());
        let frames = 48_000usize;
        let block = 512usize;
        let mut left = vec![0.0f32; frames];
        let mut right = vec![0.0f32; frames];
        for i in 0..frames {
            let v = (i as f32 * 440.0 * std::f32::consts::TAU / 48_000.0).sin() * 0.2;
            left[i] = v;
            right[i] = v;
        }
        let dry: Vec<f32> = left.clone();
        match audio::plugin_host_live::load_processor(&path, 48_000.0, block as u32) {
            Ok(mut proc) => {
                for start in (0..frames).step_by(block) {
                    let end = (start + block).min(frames);
                    proc.process_stereo(&mut left[start..end], &mut right[start..end]);
                }
                let rms = |v: &[f32]| (v.iter().map(|x| x * x).sum::<f32>() / v.len() as f32).sqrt();
                let diff: f32 = left
                    .iter()
                    .zip(dry.iter())
                    .map(|(a, b)| (a - b).abs())
                    .fold(0.0, f32::max);
                let _ = audio::wav_writer::write_pcm_f32_to_wav(&out, &left, 48_000, 1);
                println!("📦 {path}");
                println!("  backend: {}", proc.backend());
                println!("  RMS in:  {:.5}", rms(&dry));
                println!("  RMS ut:  {:.5}", rms(&left));
                println!("  största skillnad mot torrt: {diff:.5}");
                println!("  skrev:   {out}");
            }
            Err(e) => {
                eprintln!("⚠ {path}: {e}");
                std::process::exit(1);
            }
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
            .with_min_inner_size([780.0, 540.0])
            .with_title("🎹 Sonix DAW - Linux Audio Workstation")
            .with_app_id("sonix-daw")
    };

    let viewport = viewport.with_icon(
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/sonix.png"))
            .expect("embedded app icon (assets/sonix.png) must be a valid PNG"),
    );

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "Sonix DAW",
        options,
        Box::new(move |_cc| {
            let mut app = SonixApp::new(engine);
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
