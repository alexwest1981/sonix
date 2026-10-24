//! Sound Browser och biblioteket.
//!
//! Biblioteket är en **delad** skanning (\`merge_library\`): fabrikssamplar och användarens egna
//! filer i samma lista, och en kanal kan tilldelas ett sample utan att själva ljudfilen öppnas.
//!
//! Status: byggs — Sound Browser och biblioteksskanningen.
//! Rör inte: \`merge_library\` är enda vägen till listan; bygg inte en andra.

use super::*;
use crate::ui::theme::Theme;

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
    ch.notes = [item.default_note; crate::ui::app::state::MAX_STEPS];
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
    // **Multi-samples** (Fas 8.4/7): en keymap med zoner väljer sampel och grundton för noten
    // och anslaget. **Utan zoner** — eller när ingen zon täcker noten — är vägen exakt som förut:
    // kanalens eget sampel med sin slicekarta. Det är därför ett projekt från före keymappen
    // låter identiskt.
    //
    // En zon utan läsbart ljud hoppas över här i stället för att tysta noten: filen kan ha
    // flyttats, och då är kanalens eget sampel det ärliga svaret (samma regel som 8.5).
    let zon = crate::audio::keymap::zone_for_note(&ch.zones, note, velocity)
        .and_then(|i| ch.zones.get(i))
        .and_then(|z| {
            z.pcm
                .as_ref()
                .filter(|(l, _, _)| !l.is_empty())
                .map(|p| (p, z.root, z.loop_frames))
        });
    let (left, right, sample_rate, base_note, start01, end01) = match zon {
        Some(((l, r, sr), root, _)) => (l.clone(), r.clone(), *sr, root, 0.0, 1.0),
        None => {
            let (start01, end01) = crate::audio::onset::window_for_note(
                &ch.slices,
                ch.sample_base_note,
                note,
                (ch.sample_start, ch.sample_end),
            );
            let (l, r, sr) = ch.pcm_audio.as_ref()?;
            (l.clone(), r.clone(), *sr, ch.sample_base_note, start01, end01)
        }
    };
    let zon_loop = crate::audio::keymap::zone_loop_01(
        zon.map(|(_, _, lf)| lf).flatten(),
        left.len(),
    );
    Some(AudioCommand::TriggerSampleVoice {
        left,
        right,
        sample_rate,
        base_note,
        note,
        pitch_semitones: ch.pitch_semitones,
        pitch_cents: ch.pitch_fine_cents,
        velocity,
        velocity_sensitivity: ch.velocity_sensitivity,
        velocity_curve: ch.velocity_curve,
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
        // **SF2-loopens punkter vinner** när zonen bär dem: det är så filen säger att
        // instrumentet ska klinga. Utan dem gäller kanalens inställning som förut.
        loop_mode: zon_loop.map_or(ch.loop_mode, |_| crate::audio::command::LoopMode::Forever),
        loop_start01: zon_loop.map_or(ch.sample_loop_start, |(a, _)| a),
        loop_end01: zon_loop.map_or(ch.sample_loop_end, |(_, b)| b),
        ping_pong: ch.ping_pong,
        amp_env: ch.amp_env,
        filter: ch.filter,
        hold_secs,
        release: ch.release_pcm.clone(),
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
    // `Preset` importeras i modulens topp.
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
            ui.label(egui::RichText::new(crate::i18n::t("📁 SOUND BROWSER")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().orange()));
            if self.sample_drag_item.is_some() {
                ui.label(egui::RichText::new(crate::i18n::t("✋ Drar sample...")).size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
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
                    "Trumloopar" | "Drum Loops" => ("🥁", crate::ui::theme::palette().orange()),
                    "Pads & Atmosfär" | "Pads & Atmosphere" => ("✨", crate::ui::theme::palette().cyan()),
                    "Bas & 808" | "Bass & 808" => ("🎸", crate::ui::theme::palette().track_rose()),
                    "Keys & Melodier" | "Keys & Melodies" => ("🎹", crate::ui::theme::palette().amber()),
                    "FX & Övergångar" | "FX & Transitions" => ("🌪", crate::ui::theme::palette().cyan()),
                    "Kicks" => ("💥", crate::ui::theme::palette().orange()),
                    "Snares" => ("🥁", crate::ui::theme::palette().cyan()),
                    "Claps" => ("👏", crate::ui::theme::palette().cyan()),
                    "Hi-Hats" => ("🧂", crate::ui::theme::palette().yellow()),
                    "Percussion" => ("🪘", crate::ui::theme::palette().orange()),
                    "Egna Samples" | "Egna Importerade" | "📂 Egna Samples" | "📂 My Samples" => ("⭐", crate::ui::theme::palette().green()),
                    _ => ("🎵", crate::ui::theme::palette().cyan()),
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
                        let bg_col = if is_hovered { crate::ui::theme::palette().grid() } else { crate::ui::theme::palette().panel() };
                        ui.painter().rect_filled(card_rect, Rounding::same(3.0), bg_col);
                        ui.painter().rect_stroke(card_rect, Rounding::same(3.0), Stroke::new(1.0_f32, if is_hovered { item.color } else { crate::ui::theme::palette().grid() }));

                        // Card Content
                        let mut card_ui = ui.new_child(egui::UiBuilder::new().max_rect(card_rect).layout(egui::Layout::top_down(egui::Align::Min)));
                        card_ui.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);

                        card_ui.horizontal(|ui| {
                            ui.label(&item.icon);
                            ui.label(egui::RichText::new(&item.name).strong().size(Theme::FONT_S).color(item.color));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                let drag_btn = ui.add(
                                    egui::Button::new(egui::RichText::new(crate::i18n::t("✋ Dra")).size(Theme::FONT_XS).color(crate::ui::theme::palette().text()))
                                        .fill(crate::ui::theme::palette().border())
                                        .sense(Sense::click_and_drag())
                                ).on_hover_text(crate::i18n::t("Klicka & håll ned för att dra till tidslinjen"));
                                if drag_btn.drag_started() || (drag_btn.dragged() && self.sample_drag_item.is_none()) {
                                    self.sample_drag_item = Some(item.clone());
                                    self.status_message = crate::tstatus!("✋ Drar '{}' till tidslinjen... Släpp på önskat spår och takt!", item.name);
                                }
                            });
                        });

                        card_ui.horizontal_wrapped(|ui| {
                            if ui.button(egui::RichText::new("🖐").size(Theme::FONT_XS)).on_hover_text(crate::i18n::t("Scratcha samplet fram och tillbaka")).clicked() {
                                self.scratch_library_sample(item);
                            }
                            if ui.button(egui::RichText::new(crate::i18n::t("▶")).size(Theme::FONT_XS)).on_hover_text(crate::i18n::t("Provspela sample med äkta ljud")).clicked() {
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
                                            let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: 0.9, channel: 0 });
                                        }
                                    }
                                }
                            }
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("➕ Tidslinje")).size(Theme::FONT_XS)).fill(crate::ui::theme::palette().border_soft())).on_hover_text(crate::i18n::t("Skapa nytt spår på tidslinjen med denna sample")).clicked() {
                                sample_to_add_timeline = Some(item.clone());
                            }
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 Sångstudio")).size(Theme::FONT_XS)).fill(crate::ui::theme::palette().border())).on_hover_text(crate::i18n::t("Öppna i Sångstudion för isolerad solo-provspelning & formning")).clicked() {
                                sample_to_vocal_studio = Some(item.clone());
                            }
                            if ui.button(egui::RichText::new(crate::i18n::t("🎛 Kanal")).size(Theme::FONT_XS)).on_hover_text(crate::i18n::t("Ladda till Channel Rack")).clicked() {
                                let ch_idx = self.active_sample_picker_channel.unwrap_or(0);
                                sample_to_channel = Some((ch_idx, item.clone()));
                            }
                            if (item.category.starts_with("Egna") || item.category.starts_with("📂 My Samples")) && ui.button(egui::RichText::new(crate::i18n::t("🗑")).size(Theme::FONT_XS)).on_hover_text(crate::i18n::t("Ta bort från bibliotek")).clicked() {
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

            // **FLEX** — användarens egna ljud.
            ui.collapsing(crate::i18n::t("🎛 FLEX — MINA LJUD"), |ui| {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(crate::i18n::t("💾 Spara nuvarande ljud"))
                                .size(Theme::FONT_S),
                        )
                        .fill(crate::ui::theme::palette().panel()),
                    )
                    .on_hover_text(crate::i18n::t("Sparar reglagen som ett förval i din egen bank"))
                    .clicked()
                {
                    self.save_flex_preset();
                }
                if self.flex_presets.is_empty() {
                    ui.label(
                        egui::RichText::new(crate::i18n::t("Inga sparade ljud än"))
                            .size(Theme::FONT_XS)
                            .color(crate::ui::theme::palette().text_muted()),
                    );
                }
                let mut to_delete: Option<usize> = None;
                // Kopia: knapparna ändrar reglagen, och då kan listan inte vara lånad.
                let presets = self.flex_presets.clone();
                for (i, preset) in presets.iter().enumerate() {
                    let name = preset.name.clone();
                    let settings = preset.settings;
                    if !self.browser_search.is_empty()
                        && !name.to_lowercase().contains(&self.browser_search.to_lowercase())
                    {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .button(crate::i18n::t("▶"))
                            .on_hover_text(crate::i18n::t("Lyssna på preset med en testton"))
                            .clicked()
                        {
                            self.apply_flex_settings(settings);
                            let _ = self.engine.send_command(AudioCommand::NoteOn {
                                note: 60,
                                freq: 261.63,
                                velocity: 0.85,
                                channel: 0,
                            });
                        }
                        if ui.button(crate::i18n::t("Ladda")).clicked() {
                            self.apply_flex_settings(settings);
                            self.status_message = crate::tstatus!("Laddade ljudet '{}'", name);
                        }
                        if ui.button("🗑").clicked() {
                            to_delete = Some(i);
                        }
                        ui.label(name);
                    });
                }
                if let Some(i) = to_delete
                    && i < self.flex_presets.len() {
                        let removed = self.flex_presets.remove(i);
                        if let Some(path) = crate::audio::flex::store_path() {
                            let _ = crate::audio::flex::save_all(&path, &self.flex_presets);
                        }
                        self.status_message = crate::tstatus!("🗑 Tog bort '{}'", removed.name);
                    }
            });

            // Synth Presets
            ui.collapsing("🎹 SYNTH PRESETS", |ui| {
                let synths = [
                    ("Clean Pluck (80s Poly)", Preset::CleanPluck),
                    ("Warm 80s Pad (Lush)", Preset::WarmPad),
                    ("303 Acid Bass (Resonant)", Preset::AcidBass),
                    ("8-Bit Chiptune (Retro)", Preset::ChiptuneLead),
                    ("Cosmic Brass (Synthwave)", Preset::CosmicBrass),
                    ("Kepler (analog)", Preset::Kepler),
                    ("Kepler Exo (puls)", Preset::KeplerExo),
                ];
                if ui
                    .add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎲 Autogun (slumpat ljud)")).size(Theme::FONT_S)).fill(crate::ui::theme::palette().panel()))
                    .on_hover_text(crate::i18n::t("Slumpar fram ett komplett ljud ur syntens egna parametrar"))
                    .clicked()
                {
                    self.load_autogun();
                }
                for (name, p) in synths {
                    if !self.browser_search.is_empty() && !name.to_lowercase().contains(&self.browser_search.to_lowercase()) {
                        continue;
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .button(crate::i18n::t("▶"))
                            .on_hover_text(crate::i18n::t("Lyssna på preset med en testton"))
                            .clicked()
                        {
                            self.current_preset = p;
                            self.apply_preset_settings(p);
                            let _ = self.engine.send_command(AudioCommand::LoadPreset(p));
                            let _ = self.engine.send_command(AudioCommand::NoteOn { note: 60, freq: 261.63, velocity: 0.85, channel: 0 });
                        }
                        if ui.button(crate::i18n::t("Ladda")).clicked() {
                            self.current_preset = p;
                            self.apply_preset_settings(p);
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
                        if ui
                            .button(crate::i18n::t("▶"))
                            .on_hover_text(crate::i18n::t("Lyssna på samplet"))
                            .clicked()
                        {
                            let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq: midi_to_freq(note), velocity: 0.9, channel: 0 });
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

impl SonixApp {
    /// Ställer appens egna reglage till förvalets värden, så att det som syns i
    /// gränssnittet är det som spelar. Motorn får samma tal via `LoadPreset`.
    /// **FLEX**: lägger ett sparat ljud på synten — reglagen och motorn, samma väg som
    /// Autogun använder. Ingen `Preset` finns för användarljud, så varje parameter skickas.
    pub(crate) fn apply_flex_settings(&mut self, s: crate::audio::command::PresetSettings) {
        self.apply_settings(s);
        let _ = self.engine.send_command(AudioCommand::SetWaveform(s.waveform));
        let _ = self.engine.send_command(AudioCommand::SetAdsr(s.adsr));
        let _ = self.engine.send_command(AudioCommand::SetFilter(s.filter));
        let _ = self.engine.send_command(AudioCommand::SetFilterEnv {
            amount: s.filter_env_amount,
            adsr: self.filter_env,
        });
        let _ = self.engine.send_command(AudioCommand::SetSub { level: s.sub });
        let _ = self.engine.send_command(AudioCommand::SetUnison {
            voices: s.unison.clamp(1, 8),
            detune_cents: s.detune,
        });
        let _ = self.engine.send_command(AudioCommand::SetHarmonics { levels: s.harmonics });
    }

    /// Ljudet som reglagen står på nu, i samma form som ett sparat förval.
    pub(crate) fn current_flex_settings(&self) -> crate::audio::command::PresetSettings {
        crate::audio::command::PresetSettings {
            waveform: self.waveform,
            adsr: self.adsr,
            filter: self.filter,
            filter_env_amount: self.filter_env_amount,
            sub: self.sub_level,
            unison: self.unison_voices,
            detune: self.unison_detune_cents,
            glide: self.glide_time,
            drive: self.filter_drive,
            harmonics: self.harmonics,
        }
    }

    /// Sparar nuvarande ljud i FLEX-biblioteket. Namnet får man av vågformen och numret —
    /// ett namnbyte är en egen liten sak, inte värt en textruta här.
    pub(crate) fn save_flex_preset(&mut self) {
        let name = format!("{} {}", self.waveform.name(), self.flex_presets.len() + 1);
        self.flex_presets.push(crate::audio::flex::FlexPreset {
            name: name.clone(),
            settings: self.current_flex_settings(),
        });
        match crate::audio::flex::store_path() {
            Some(path) => match crate::audio::flex::save_all(&path, &self.flex_presets) {
                Ok(()) => self.status_message = crate::tstatus!("💾 Sparade ljudet '{}'", name),
                Err(e) => self.status_message = crate::tstatus!("⚠ Kunde inte spara: {}", e),
            },
            None => self.status_message = crate::tstatus!("⚠ Hittar ingen data-mapp att spara i"),
        }
    }

    pub(crate) fn apply_preset_settings(&mut self, preset: Preset) {
        self.apply_settings(preset.settings());
    }

    /// **Autogun**: ett framslumpat ljud, applicerat på samma väg som ett fabriksförval.
    pub(crate) fn load_autogun(&mut self) {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(1);
        let settings = crate::instruments::autogun(seed);
        self.apply_settings(settings);
        // Motorn får samma tal som reglagen: det finns inget `Preset` för Autogun, så varje
        // parameter skickas för sig.
        let _ = self.engine.send_command(AudioCommand::SetWaveform(settings.waveform));
        let _ = self.engine.send_command(AudioCommand::SetAdsr(settings.adsr));
        let _ = self.engine.send_command(AudioCommand::SetFilter(settings.filter));
        let _ = self.engine.send_command(AudioCommand::SetFilterEnv {
            amount: settings.filter_env_amount,
            adsr: self.filter_env,
        });
        let _ = self.engine.send_command(AudioCommand::SetSub { level: settings.sub });
        let _ = self.engine.send_command(AudioCommand::SetUnison {
            voices: settings.unison.clamp(1, 8),
            detune_cents: settings.detune,
        });
        let _ = self.engine.send_command(AudioCommand::SetHarmonics { levels: settings.harmonics });
        self.status_message = crate::tstatus!("🎲 Autogun: {}", settings.waveform.name());
    }

    /// **BeepMap**: filens byte blir deltonsnivåer. Additiv vågform, så dragstängerna syns och
    /// kan dras vidare för hand efteråt.
    pub(crate) fn apply_beepmap(&mut self, path: &str) {
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                self.status_message = crate::tstatus!("⚠️ Kunde inte läsa '{}': {}", path, e);
                return;
            }
        };
        let levels = crate::instruments::beepmap_amplitudes(&bytes);
        self.harmonics = levels;
        self.waveform = Waveform::Additive;
        let _ = self.engine.send_command(AudioCommand::SetHarmonics { levels });
        let _ = self.engine.send_command(AudioCommand::SetWaveform(Waveform::Additive));
        self.status_message = crate::tstatus!("🖼 BeepMap: {} byte → 16 deltoner", bytes.len());
    }

    fn apply_settings(&mut self, s: crate::audio::command::PresetSettings) {
        self.waveform = s.waveform;
        self.adsr = s.adsr;
        self.filter = s.filter;
        self.filter_env_amount = s.filter_env_amount;
        self.sub_level = s.sub;
        self.unison_voices = s.unison.clamp(1, 8);
        self.unison_detune_cents = s.detune;
        self.glide_time = s.glide;
        self.filter_drive = s.drive;
        self.harmonics = s.harmonics;
    }
}
