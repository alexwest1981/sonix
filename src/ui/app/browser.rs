//! Sound Browser och biblioteket.
//!
//! Biblioteket är en **delad** skanning (\`merge_library\`): fabrikssamplar och användarens egna
//! filer i samma lista, och en kanal kan tilldelas ett sample utan att själva ljudfilen öppnas.
//!
//! Status: byggs — Sound Browser och biblioteksskanningen.
//! Rör inte: \`merge_library\` är enda vägen till listan; bygg inte en andra.

use super::*;

/// Startar ljudbiblioteket (Fas 7.4).
///
/// Finns cachen läses den direkt — det tar en bråkdel av en sekund. Saknas den
/// startas en **bakgrundsskanning** i stället för att fönstret ska vänta: den
/// kalla skanningen läser ~10 GB och tog 125 sekunder.
pub(crate) fn init_sample_library_start() -> (
    Vec<LibrarySampleItem>,
    Option<crate::audio::factory_samples::LibraryScan>,
) {
    if let Some(cached) = crate::audio::factory_samples::read_library_cache_items() {
        eprintln!("🎵 Ljudbibliotek läst från cache: {} samplar", cached.len());
        (library_items_from_scanned(cached), None)
    } else {
        eprintln!("🎵 Ingen cache — skannar ljudbiblioteket i bakgrunden (fönstret visas direkt)");
        (
            Vec::new(),
            Some(crate::audio::factory_samples::LibraryScan::spawn()),
        )
    }
}

/// Slår ihop en färdig biblioteksskanning med det som redan låg i biblioteket
/// (Fas 7.4).
///
/// Skanningen tar minuter, och under tiden kan användaren ha importerat ett eget
/// sample. En rak ersättning skulle tysta tappa den importen — alltså behålls
/// poster som inte finns i skanningen.
pub(crate) fn merge_library(
    scanned: Vec<LibrarySampleItem>,
    existing: &[LibrarySampleItem],
) -> Vec<LibrarySampleItem> {
    let mut merged = scanned;
    for old in existing {
        if !merged.iter().any(|new| new.file_path == old.file_path) {
            merged.push(old.clone());
        }
    }
    merged
}

pub(crate) fn library_items_from_scanned(scanned: Vec<crate::audio::factory_samples::ScannedSampleItem>) -> Vec<LibrarySampleItem> {
    let mut items = Vec::new();

    for (idx, sc) in scanned.into_iter().enumerate() {
        let color = Color32::from_rgb(sc.color_rgb.0, sc.color_rgb.1, sc.color_rgb.2);
        items.push(LibrarySampleItem {
            id: idx,
            name: sc.name,
            category: sc.category,
            icon: sc.icon,
            default_note: sc.default_note,
            color,
            waveform: sc.waveform,
            file_path: Some(sc.file_path),
        });
    }

    items
}

/// Assigns a library sample to a Channel Rack channel strip. Loads the real
/// PCM into memory so step playback triggers the actual WAV instead of the
/// built-in synthesizer drum voices.
///
/// Lämnar `false` och rör **ingenting** när filen inte går att läsa. Förut tog
/// kanalen samplen ändå: namn, färg, steg och bibliotekets grova vågform flyttades
/// in medan `pcm_audio` blev `None`, så kanalen såg ut att ha ett eget sample och
/// spelade den inbyggda synten i stället. Det är samma familj som de tysta
/// klippen, i kanalracket i stället för på tidslinjen.
pub(crate) fn assign_library_sample_to_channel(ch: &mut ChannelStrip, item: &LibrarySampleItem) -> bool {
    // Ljudet läses FÖRST. En guard som ligger efter ändringarna lämnar en
    // halvflyttad kanal efter sig — den ser ut att ha ett sample den inte har.
    let pcm = match item.file_path.as_deref() {
        Some(path) => match load_sample_pcm_arcs(path) {
            Some(pcm) => Some(pcm),
            None => return false,
        },
        None => None,
    };
    ch.name = item.name.clone();
    ch.icon = item.icon.clone();
    ch.color = item.color;
    ch.notes = [item.default_note; 16];
    ch.waveform_preview = item.waveform.clone();
    ch.sample_start = 0.0;
    ch.sample_end = 1.0;
    ch.pitch_semitones = 0;
    ch.pitch_fine_cents = 0.0;
    ch.sample_base_note = item.default_note;
    ch.sample_path = item.file_path.clone();
    ch.pcm_audio = pcm;
    true
}

/// Picks a fitting real WAV from the library for each built-in drum channel
/// (Kick/Snare/Clap/Hat/Crash) when the user has downloaded sample packs, so the
/// Channel Rack plays actual recorded samples out of the box.
pub(crate) fn auto_assign_default_kit(channels: &mut [ChannelStrip], library: &[LibrarySampleItem]) {
    const KICK: &[&str] = &["bass drum", "bd0", "bd-", "kick"];
    const SNARE: &[&str] = &["snare"];
    const CLAP: &[&str] = &["handclap", "clap"];
    const HAT_CLOSED: &[&str] = &["closed hat", "chh", "hat closed"];
    const HAT_OPEN: &[&str] = &["open hat", "ohh", "hat open"];
    const CRASH: &[&str] = &["crash", "cymbal", "cy"];
    let rules: [(&[&str], usize); 6] = [
        (KICK, 0),
        (SNARE, 1),
        (CLAP, 2),
        (HAT_CLOSED, 3),
        (HAT_OPEN, 4),
        (CRASH, 5),
    ];

    let mut used_paths: Vec<String> = Vec::new();
    for (keywords, ch_idx) in rules {
        if ch_idx >= channels.len() {
            continue;
        }
        if channels[ch_idx].pcm_audio.is_some() {
            continue;
        }
        let found = library.iter().find(|item| {
            item.file_path.as_deref().is_some_and(|p| !used_paths.iter().any(|u| u == p))
                && keywords.iter().any(|k| item.name.to_lowercase().contains(k))
        });
        if let Some(item) = found {
            if let Some(p) = item.file_path.clone() {
                used_paths.push(p);
            }
            // Går filen inte att läsa tas samplen inte alls (se funktionen) — då
            // behåller kanalen sin inbyggda röst i stället för ett namn utan ljud.
            let _ = assign_library_sample_to_channel(&mut channels[ch_idx], item);
        }
    }
}

/// Builds the audio command that plays a channel's loaded WAV sample for one
/// sequencer step. Returns None when the channel has no PCM loaded, in which
/// case the caller falls back to the built-in synthesizer.
pub(crate) fn channel_sample_trigger_command(
    ch: &ChannelStrip,
    channel: usize,
    note: u8,
    velocity: f32,
    hold_secs: f32,
) -> Option<AudioCommand> {
    let (start01, end01) = crate::audio::onset::window_for_note(
        &ch.slices,
        ch.sample_base_note,
        note,
        (ch.sample_start, ch.sample_end),
    );
    ch.pcm_audio.as_ref().map(|(l, r, sr)| AudioCommand::TriggerSampleVoice {
        left: l.clone(),
        right: r.clone(),
        sample_rate: *sr,
        base_note: ch.sample_base_note,
        note,
        pitch_semitones: ch.pitch_semitones,
        pitch_cents: ch.pitch_fine_cents,
        velocity,
        volume: ch.volume,
        reverse: ch.is_reverse,
        // Slicekartan (8.7): noten **är** adressen — `bas + i` spelar slice `i`,
        // och samma regel används i exporten. Utan den hade en kanal med slicar
        // låtit olika i filen och i högtalarna.
        start01,
        end01,
        // Samplern (Fas 8.4): kanalen (för not-av), loopläget med sina punkter,
        // riktningen, envelopen och notens längd — stegets egen längd, så att en
        // lopande not håller lika länge som steget varar.
        channel: channel as u32,
        loop_mode: ch.loop_mode,
        loop_start01: ch.sample_loop_start,
        loop_end01: ch.sample_loop_end,
        ping_pong: ch.ping_pong,
        amp_env: ch.amp_env,
        hold_secs,
    })
}

pub(crate) fn ui_dbg(msg: &str) {
    let paths = crate::paths::paths();
    let on = std::env::var("SONIX_AUDIO_DEBUG").is_ok()
        || paths.debug_marker_file().exists()
        || paths.legacy_library_file("debug_on").exists();
    if !on {
        return;
    }
    let path = paths.log_file("audio_debug.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = writeln!(f, "[{}] UI: {}", std::process::id(), msg);
    }
}

impl SonixApp {
/// Öppnar ett bibliotekssample i Sångstudion som en tagning.
///
/// **Ingen påhittad ton.** Vägen byggde förut en syntetisk sinuston (två
/// sekunder, ur samplens `default_note`) när filen inte gick att läsa, och
/// öppnade Sångstudion med den. En mp3 — som appen inte kan avkoda — blev
/// alltså en påkittad tagning i stället för ett besked, och den som lyssnade
/// hörde något som varken var samplen eller tystnad. Nu gäller samma regel som
/// `import_audio_file_as_track`: säg det och avbryt.
///
/// Provspelningen får också **filens egen** samplerate. Förut sades 44100
/// oavsett vad filen innehöll, så en tagning ur ett 48 kHz-sample spelades i
/// fel hastighet.
pub fn open_sample_in_vocal_studio(&mut self, item: &LibrarySampleItem) {
    let Some(ref path) = item.file_path else {
        self.status_message = crate::tstatus!(
            "⚠ '{}' har ingen ljudfil — kan inte öppnas i Sångstudion",
            item.name
        );
        return;
    };
    let Some((pcm, _, sample_rate)) = load_audio_or_report(path) else {
        self.status_message = crate::tstatus!(
            "⚠ Kunde inte läsa '{}' — öppnas inte i Sångstudion (mp3 stöds inte, konvertera till wav)",
            item.name
        );
        return;
    };
    let new_take_idx = self.vocal_studio.load_sample_or_region_as_take(&item.name, pcm, sample_rate, item.color);
    self.view_mode = ViewMode::VocalStudio;
    self.status_message = crate::tstatus!("🎙 Öppnade sample '{}' i Sångstudion (Tagning {}) för isolerad provspelning & formning!", item.name, new_take_idx + 1);
}
}

impl SonixApp {
#[allow(clippy::too_many_arguments)]
pub fn import_custom_sample(
    &mut self,
    name: String,
    category: String,
    icon: String,
    default_note: u8,
    color: Color32,
    freq: f32,
    decay: f32,
) {
    let id = self.sample_library.len() + 1;
    let wave: Vec<f32> = (0..32).map(|i| {
        let t = i as f32 / 32.0;
        let env = (-t * decay * 4.0).exp();
        (t * freq).sin() * env
    }).collect();
    let item = LibrarySampleItem {
        id,
        name: name.clone(),
        category,
        icon,
        default_note,
        color,
        waveform: wave,
        file_path: None,
    };
    self.sample_library.push(item);
    self.status_message = crate::tstatus!("Importerade '{}' till ljudbiblioteket!", name);
}
}

impl SonixApp {
/// Läser av en pågående biblioteksskanning (Fas 7.4).
///
/// Statusraden visar förloppet medan den kör, och när den är klar fylls
/// biblioteket — och de inbyggda trumkanalerna får riktiga inspelade samplar
/// om de fortfarande står på standardljudet. Att tilldela i efterhand går
/// bra: kanalens PCM skickas till motorn vid varje anslag.
pub(crate) fn poll_library_scan(&mut self) {
    let Some(scan) = self.library_scan.as_mut() else {
        return;
    };
    let poll = scan.poll();
    let elapsed = self.library_scan_started.elapsed().as_secs_f32();
    match poll {
        crate::audio::factory_samples::ScanPoll::Idle => {
            // Bara om inget viktigare står där: en skanning får inte skriva
            // över en ångrings- eller felrad.
            if self.status_message.is_empty() || self.status_message.starts_with("🎵") {
                self.status_message =
                    crate::tstatus!("🎵 Läser in ljudbiblioteket i bakgrunden… ({:.0} s)", elapsed);
            }
        }
        crate::audio::factory_samples::ScanPoll::Progress { found, dir } => {
            self.status_message = crate::tstatus!(
                "🎵 Skannar ljudbiblioteket: {} samplar ({}, {:.0} s)",
                found,
                dir,
                elapsed
            );
        }
        crate::audio::factory_samples::ScanPoll::Done(items) => {
            let count = items.len();
            let scanned = library_items_from_scanned(items);
            let existing = self.sample_library.clone();
            self.sample_library = merge_library(scanned, &existing);
            // Ge de inbyggda trumkanalerna riktiga samplar — men rör inte en
            // kanal användaren redan lagt ett eget ljud på. Kit-tilldelaren tar
            // en hel slice, så de orörda kanalerna lyfts ut, tilldelas och
            // läggs tillbaka på samma platser.
            if !self.sample_library.is_empty() {
                let untouched: Vec<usize> = (0..self.channels.len().min(6))
                    .filter(|i| self.channels[*i].sample_path.is_none())
                    .collect();
                if !untouched.is_empty() {
                    let mut scope: Vec<ChannelStrip> =
                        untouched.iter().map(|i| self.channels[*i].clone()).collect();
                    auto_assign_default_kit(&mut scope, &self.sample_library);
                    for (slot, i) in untouched.iter().enumerate() {
                        self.channels[*i] = scope[slot].clone();
                    }
                }
            }
            self.status_message = crate::tstatus!(
                "🎵 Ljudbiblioteket klart: {} samplar på {:.1} s",
                count,
                elapsed
            );
            self.library_scan = None;
        }
        crate::audio::factory_samples::ScanPoll::Aborted => {
            self.status_message = crate::i18n::t(
                "⚠ Kunde inte läsa in ljudbiblioteket — skanningen avbröts",
            )
            .to_string();
            self.library_scan = None;
        }
    }
}
}

impl SonixApp {
pub(crate) fn render_browser_sidebar(&mut self, ui: &mut egui::Ui) {
    ui.vertical(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("📁 SOUND BROWSER")).strong().size(13.0).color(Theme::FL_ORANGE));
            if self.sample_drag_item.is_some() {
                ui.label(egui::RichText::new(crate::i18n::t("✋ Drar sample...")).size(10.0).color(Theme::FL_CYAN));
            }
        });
        ui.add_space(2.0);

        ui.horizontal(|ui| {
            ui.label(crate::i18n::t("🔍"));
            ui.text_edit_singleline(&mut self.browser_search);
        });

        ui.separator();

        let mut to_delete_sample_id = None;
        let mut sample_to_add_timeline: Option<LibrarySampleItem> = None;
        let mut sample_to_vocal_studio: Option<LibrarySampleItem> = None;
        let mut sample_to_channel: Option<(usize, LibrarySampleItem)> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            // Group library samples by category
            let mut categories_map: std::collections::BTreeMap<String, Vec<LibrarySampleItem>> = std::collections::BTreeMap::new();
            for item in &self.sample_library {
                if !self.browser_search.is_empty() && !item.name.to_lowercase().contains(&self.browser_search.to_lowercase()) && !item.category.to_lowercase().contains(&self.browser_search.to_lowercase()) {
                    continue;
                }
                categories_map.entry(item.category.clone()).or_default().push(item.clone());
            }

            // Render each category
            for (cat_name, items) in categories_map {
                let (cat_icon, cat_color) = match cat_name.as_str() {
                    "Trumloopar" | "Drum Loops" => ("🥁", Theme::FL_ORANGE),
                    "Pads & Atmosfär" | "Pads & Atmosphere" => ("✨", Color32::from_rgb(100, 200, 255)),
                    "Bas & 808" | "Bass & 808" => ("🎸", Color32::from_rgb(255, 80, 140)),
                    "Keys & Melodier" | "Keys & Melodies" => ("🎹", Color32::from_rgb(255, 200, 100)),
                    "FX & Övergångar" | "FX & Transitions" => ("🌪", Color32::from_rgb(140, 220, 255)),
                    "Kicks" => ("💥", Theme::FL_ORANGE),
                    "Snares" => ("🥁", Theme::FL_CYAN),
                    "Claps" => ("👏", Theme::FL_CYAN),
                    "Hi-Hats" => ("🧂", Theme::FL_YELLOW),
                    "Percussion" => ("🪘", Color32::from_rgb(255, 140, 60)),
                    "Egna Samples" | "Egna Importerade" | "📂 Egna Samples" | "📂 My Samples" => ("⭐", Theme::FL_GREEN),
                    _ => ("🎵", Theme::FL_CYAN),
                };

                ui.collapsing(egui::RichText::new(format!("{} {} ({})", cat_icon, cat_name.to_uppercase(), items.len())).strong().color(cat_color), |ui| {
                    for item in &items {
                        let _item_id = ui.make_persistent_id(format!("browser_item_{}", item.id));
                        let (card_rect, card_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width() - 4.0, 52.0), Sense::click_and_drag());

                        if card_resp.drag_started() || (card_resp.dragged() && self.sample_drag_item.is_none()) {
                            self.sample_drag_item = Some(item.clone());
                            self.status_message = crate::tstatus!("✋ Drar '{}' till tidslinjen... Släpp på önskat spår och takt!", item.name);
                        }

                        let is_hovered = card_resp.hovered();
                        let bg_col = if is_hovered { Color32::from_rgb(28, 34, 46) } else { Theme::PANEL_BG };
                        ui.painter().rect_filled(card_rect, Rounding::same(3.0), bg_col);
                        ui.painter().rect_stroke(card_rect, Rounding::same(3.0), Stroke::new(1.0_f32, if is_hovered { item.color } else { Color32::from_rgb(32, 40, 52) }));

                        // Card Content
                        let mut card_ui = ui.new_child(egui::UiBuilder::new().max_rect(card_rect).layout(egui::Layout::top_down(egui::Align::Min)));
                        card_ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);

                        card_ui.horizontal(|ui| {
                            ui.label(&item.icon);
                            ui.label(egui::RichText::new(&item.name).strong().size(11.0).color(item.color));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let drag_btn = ui.add(
                                    egui::Button::new(egui::RichText::new(crate::i18n::t("✋ Dra")).size(9.5).color(Color32::WHITE))
                                        .fill(Color32::from_rgb(45, 55, 75))
                                        .sense(Sense::click_and_drag())
                                ).on_hover_text(crate::i18n::t("Klicka & håll ned för att dra till tidslinjen"));
                                if drag_btn.drag_started() || (drag_btn.dragged() && self.sample_drag_item.is_none()) {
                                    self.sample_drag_item = Some(item.clone());
                                    self.status_message = crate::tstatus!("✋ Drar '{}' till tidslinjen... Släpp på önskat spår och takt!", item.name);
                                }
                            });
                        });

                        card_ui.horizontal_wrapped(|ui| {
                            if ui.button(egui::RichText::new(crate::i18n::t("▶")).size(9.5)).on_hover_text(crate::i18n::t("Provspela sample med äkta ljud")).clicked() {
                                // Samma tre fall som `audition_library_sample`: ingen fil →
                                // appens egen syntröst; fil som inte går att läsa → ett besked,
                                // aldrig en syntetisk trumma i samplens ställe.
                                let mut handled = false;
                                if let Some(ref path) = item.file_path {
                                    match load_audio_or_report(path) {
                                        Some((l, r, sr)) => {
                                            let _ = self.engine.send_command(AudioCommand::PlayAudition {
                                                left: std::sync::Arc::new(l),
                                                right: std::sync::Arc::new(r),
                                                sample_rate: sr as f32,
                                                volume: 0.95,
                                                pitch_ratio: 1.0,
                                                time_stretch_ratio: 1.0,
                                                is_reverse: false,
                                                loop_playback: false,
                                            });
                                            handled = true;
                                        }
                                        None => {
                                            self.status_message = crate::tstatus!(
                                                "⚠ Kunde inte provspela '{}' — filen går inte att läsa: {} (mp3 stöds inte, konvertera till wav)",
                                                item.name,
                                                path
                                            );
                                            handled = true;
                                        }
                                    }
                                }
                                if !handled {
                                    match item.category.as_str() {
                                        "Kicks" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick)); },
                                        "Snares" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Snare)); },
                                        "Claps" => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Clap)); },
                                        "Hi-Hats" => {
                                            if item.name.to_lowercase().contains("open") {
                                                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen));
                                            } else {
                                                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed));
                                            }
                                        },
                                        "Percussion" => {
                                            if item.name.to_lowercase().contains("high") {
                                                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::TomHigh));
                                            } else {
                                                let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::TomLow));
                                            }
                                        },
                                        _ => {
                                            let note = item.default_note;
                                            let freq = midi_to_freq(note);
                                            let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: 0.9 });
                                        }
                                    }
                                }
                            }
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("➕ Tidslinje")).size(9.5)).fill(Color32::from_rgb(30, 65, 90))).on_hover_text(crate::i18n::t("Skapa nytt spår på tidslinjen med denna sample")).clicked() {
                                sample_to_add_timeline = Some(item.clone());
                            }
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 Sångstudio")).size(9.5)).fill(Color32::from_rgb(50, 45, 80))).on_hover_text(crate::i18n::t("Öppna i Sångstudion för isolerad solo-provspelning & formning")).clicked() {
                                sample_to_vocal_studio = Some(item.clone());
                            }
                            if ui.button(egui::RichText::new(crate::i18n::t("🎛 Kanal")).size(9.0)).on_hover_text(crate::i18n::t("Ladda till Channel Rack")).clicked() {
                                let ch_idx = self.active_sample_picker_channel.unwrap_or(0);
                                sample_to_channel = Some((ch_idx, item.clone()));
                            }
                            if (item.category.starts_with("Egna") || item.category.starts_with("📂 My Samples")) && ui.button(egui::RichText::new(crate::i18n::t("🗑")).size(9.0)).on_hover_text(crate::i18n::t("Ta bort från bibliotek")).clicked() {
                                to_delete_sample_id = Some(item.id);
                            }
                        });

                        ui.add_space(2.0);
                    }
                });
                ui.add_space(4.0);
            }

            if let Some(del_id) = to_delete_sample_id {
                self.sample_library.retain(|s| s.id != del_id);
            }

            // Synth Presets
            ui.collapsing("🎹 SYNTH PRESETS", |ui| {
                let synths = [
                    ("Clean Pluck (80s Poly)", Preset::CleanPluck),
                    ("Warm 80s Pad (Lush)", Preset::WarmPad),
                    ("303 Acid Bass (Resonant)", Preset::AcidBass),
                    ("8-Bit Chiptune (Retro)", Preset::ChiptuneLead),
                    ("Cosmic Brass (Synthwave)", Preset::CosmicBrass),
                ];
                for (name, p) in synths {
                    if !self.browser_search.is_empty() && !name.to_lowercase().contains(&self.browser_search.to_lowercase()) {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui.button(crate::i18n::t("▶")).clicked() {
                            self.current_preset = p;
                            let (wf, adsr, flt) = p.settings();
                            self.waveform = wf;
                            self.adsr = adsr;
                            self.filter = flt;
                            let _ = self.engine.send_command(AudioCommand::LoadPreset(p));
                            let _ = self.engine.send_command(AudioCommand::NoteOn { note: 60, freq: 261.63, velocity: 0.85 });
                        }
                        if ui.button(crate::i18n::t("Ladda")).clicked() {
                            self.current_preset = p;
                            let (wf, adsr, flt) = p.settings();
                            self.waveform = wf;
                            self.adsr = adsr;
                            self.filter = flt;
                            let _ = self.engine.send_command(AudioCommand::LoadPreset(p));
                            self.status_message = crate::tstatus!("Laddade preset: {}", name);
                        }
                        ui.label(name);
                    });
                }
            });

            ui.add_space(4.0);

            // Groove Templates
            ui.collapsing("🎵 GROOVE INSPIRATION LOOPS", |ui| {
                let grooves = [
                    ("Neon Synthwave (126 BPM)", 126.0, 0.15),
                    ("Cyberpunk Dark (128 BPM)", 128.0, 0.0),
                    ("Lo-Fi Chill Hop (88 BPM)", 88.0, 0.35),
                    ("House 4x4 (124 BPM)", 124.0, 0.10),
                ];
                for (name, bpm, swing) in grooves {
                    if !self.browser_search.is_empty() && !name.to_lowercase().contains(&self.browser_search.to_lowercase()) {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui.button(crate::i18n::t("⚡ Sätt BPM")).clicked() {
                            self.bpm = bpm;
                            self.swing = swing;
                            self.status_message = crate::tstatus!("Satte tempo till {} BPM med {:.0}% swing", bpm, swing * 100.0);
                        }
                        ui.label(name);
                    });
                }
            });

            ui.add_space(4.0);

            // Sound FX
            ui.collapsing("🌊 SOUND FX & RISERS", |ui| {
                let sfx = [
                    ("🌀 White Noise Riser", 60),
                    ("⚡ Laser Beam Zap", 72),
                    ("💥 Sub Impact Drop", 36),
                ];
                for (name, note) in sfx {
                    if !self.browser_search.is_empty() && !name.to_lowercase().contains(&self.browser_search.to_lowercase()) {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui.button(crate::i18n::t("▶")).clicked() {
                            let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq: midi_to_freq(note), velocity: 0.9 });
                        }
                        ui.label(name);
                    });
                }
            });
        });

        if let Some(item) = sample_to_add_timeline {
            self.add_sample_item_to_timeline(&item);
        }
        if let Some(item) = sample_to_vocal_studio {
            self.open_sample_in_vocal_studio(&item);
        }
        if let Some((ch_idx, item)) = sample_to_channel {
            self.select_sound_for_channel(ch_idx, &item);
        }
    });
}
}

