//! Bildrutan — `update` uppdelad i sina faser (2026-09-14).
//!
//! `update` ligger kvar i `app.rs` och är numera en innehållsförteckning: den
//! anropar faserna här i ordning. Varje fas är flyttad **ordagrant** — samma kod,
//! samma ordning, bara ett namn. Blocket ligger därför på samma indrag som förut.
//!
//! Status: byggs — bildrutans faser.
//! Rör inte: ordningen. Pollarna ligger före ritningen med flit (motorn ska veta
//! sitt tempo innan något ritas), och autosparna ligger före mixerns viloläge.

use super::*;
use crate::ui::theme::Theme;


/// Vilken flik en vy hör till: 0 = SKAPA, 1 = ARRANGERA, 2 = MIXA, 3 = VERKTYG.
///
/// Fliken **härleds** ur vyn i stället för att sparas löst: då kan en genväg eller meny inte
/// lämna flikmarkeringen på en grupp som inte är den man tittar på.
pub(crate) fn nav_group_of(view: ViewMode) -> u8 {
    match view {
        ViewMode::PlaylistArranger | ViewMode::Video => 1,
        ViewMode::EffectsMixer
        | ViewMode::RemixFx
        | ViewMode::StemSeparator
        | ViewMode::PluginManager => 2,
        _ => 0,
    }
}

/// Namnet som panelen visar i sin rubrik.
fn view_name(view: ViewMode) -> &'static str {
    match view {
        ViewMode::ChannelRack => "🥁 Kanalrack",
        ViewMode::PianoRoll => "🎹 Pianorulle",
        ViewMode::PlaylistArranger => "🎼 Tidslinje",
        ViewMode::SessionDrummer => "🥁 Session",
        ViewMode::AlchemySynth => "🧪 Alchemy",
        ViewMode::RemixFx => "🎛 Remix FX",
        ViewMode::VocalStudio => "🎙 Sångstudio",
        ViewMode::AiMusicAssistant => "🤖 AI",
        ViewMode::ModularPatcher => "🔌 Patcher",
        ViewMode::StemSeparator => "🪄 Stämmor",
        ViewMode::Video => "🎬 Video",
        ViewMode::PluginManager => "🧩 Pluggar",
        ViewMode::EffectsMixer => "🎚 Mixer",
    }
}

/// Vyerna som kan ligga som panel bredvid arrangemanget, i tangentbordsordning Ctrl+1…5.
pub(crate) const DOCKABLE: [ViewMode; 5] = [
    ViewMode::PianoRoll,
    ViewMode::EffectsMixer,
    ViewMode::ChannelRack,
    ViewMode::VocalStudio,
    ViewMode::ModularPatcher,
];

impl SonixApp {
    /// Ritar en vy. Samma funktion används både när vyn är en egen sida och när den
    /// ligger som panel bredvid arrangemanget (Abletons modell: ytorna står kvar).
    pub(crate) fn render_view(&mut self, view: ViewMode, ui: &mut egui::Ui, ctx: &egui::Context) {
        match view {
                ViewMode::PlaylistArranger => {
                    self.render_playlist_arranger(ui);
                }
                ViewMode::ChannelRack => {
                    self.render_channel_rack(ui);
                    ui.add_space(10.0);
                    self.render_synth_hardware_rack(ui);
                }
                ViewMode::PianoRoll => {
                    self.render_piano_roll_editor(ui);
                    ui.add_space(10.0);
                    self.render_touch_piano_keyboard(ui, ctx);
                    ui.add_space(10.0);
                    self.render_synth_hardware_rack(ui);
                }
                ViewMode::SessionDrummer => {
                    self.render_session_drummer(ui);
                }
                ViewMode::AlchemySynth => {
                    self.render_alchemy_morph_pad(ui);
                }
                ViewMode::RemixFx => {
                    self.render_remix_fx_view(ui);
                }
                ViewMode::ModularPatcher => {
                    let actions = render_patcher_view(
                        ui,
                        &mut self.modular_graph,
                        self.anim_phase,
                        &mut self.patcher_enabled,
                    );
                    if actions.enabled_changed {
                        let _ = self.engine.send_command(AudioCommand::SetPatcherEnabled(self.patcher_enabled));
                    }
                    if let Some((freq, vel)) = actions.note_on {
                        let _ = self.engine.send_command(AudioCommand::PatcherNoteOn { freq, velocity: vel });
                    } else if actions.note_off {
                        let _ = self.engine.send_command(AudioCommand::PatcherNoteOff);
                    }
                }
                ViewMode::Video => {
                    crate::ui::video_view::render_video_view(
                        ui,
                        &mut self.video,
                        self.song_time as f64,
                        self.is_playing,
                        &mut self.status_message,
                    );
                }
                ViewMode::StemSeparator => {
                    let actions = render_stem_separator_view(ui, &mut self.stem_project, self.song_time, self.is_playing, &mut self.status_message, &self.model_download, &mut self.stem_six);
                    if actions.request_separation {
                        self.pending_stem_separation = true;
                        self.spawn_async_file_picker(
                            "Ljudfiler",
                            &[
                                "wav", "WAV", "mp3", "MP3", "flac", "FLAC", "ogg", "OGG",
                                "m4a", "aiff",
                            ],
                            "Välj mix att separera i stämspår",
                        );
                    }
                    if actions.export_stems {
                        self.export_separated_stems();
                    }
                    if let Some(idx) = actions.speed_changed {
                        self.apply_stem_time_stretch(idx);
                        self.status_message = crate::tstatus!("SPEED: tidssträckning tillämpad på stämspår {} (tonhöjd bevarad)", idx + 1);
                    }
                }
                ViewMode::PluginManager => {
                    let active_plugins: Vec<Option<String>> = self
                        .plugin_slots
                        .iter()
                        .map(|s| s.as_ref().map(|p| p.name.clone()))
                        .collect();
                    let stem_track_count = self
                        .stem_project
                        .stems
                        .len()
                        .max(self.playlist_tracks.len())
                        .max(active_plugins.len());
                    let gui_open: Vec<bool> = (0..active_plugins.len())
                        .map(|i| self.is_plugin_gui_open(i))
                        .collect();
                    #[cfg(feature = "plugin-host")]
                    let sandbox_status = self.sandbox_status_text();
                    #[cfg(not(feature = "plugin-host"))]
                    let sandbox_status: Option<String> = None;
                    // Offseten som de **sparade** slotarna bär (Fas 8.6), inte en
                    // egen kopia i vyn: det som ritas ska vara det som kompenserar.
                    let latency_offsets: Vec<i32> = self
                        .plugin_slots
                        .iter()
                        .map(|s| {
                            s.as_ref().map(|p| p.latency_offset_frames).unwrap_or(0)
                        })
                        .collect();
                    let smart_flags: Vec<bool> = self
                        .plugin_slots
                        .iter()
                        .map(|s| s.as_ref().map(|p| p.smart_disable).unwrap_or(false))
                        .collect();
                    // **Antalet egna utbussar frågas hos pluginen** (Fas 8.6), inte hos
                    // en kopia: vyn ska rita en koppling per buss som faktiskt finns.
                    let extra_out_counts: Vec<usize> = self
                        .plugin_handles
                        .iter()
                        .map(|h| h.as_ref().map(|h| h.extra_output_ports()).unwrap_or(0))
                        .collect();
                    let extra_out_targets: Vec<Vec<Option<usize>>> = self
                        .plugin_slots
                        .iter()
                        .map(|s| {
                            s.as_ref()
                                .map(|p| p.extra_out_targets.clone())
                                .unwrap_or_default()
                        })
                        .collect();
                    let actions = render_plugins_view(
                        ui,
                        &mut self.plugin_manager,
                        &mut self.status_message,
                        stem_track_count,
                        self.selected_channel,
                        &active_plugins,
                        &gui_open,
                        sandbox_status.as_deref(),
                        &latency_offsets,
                        &smart_flags,
                        &extra_out_counts,
                        &extra_out_targets,
                        self.engine.sample_rate as f32,
                    );
                    if let Some((path, track)) = actions.load_into_track {
                        self.load_plugin_into_track(&path, track);
                    }
                    #[cfg(feature = "plugin-host")]
                    if let Some((path, track)) = actions.load_into_sandbox {
                        self.load_plugin_into_sandbox_track(&path, track);
                    }
                    #[cfg(not(feature = "plugin-host"))]
                    let _ = actions.load_into_sandbox;
                    if let Some((path, track, location)) = actions.load_preset_into_track {
                        self.load_plugin_preset_into_track(&path, track, &location);
                    }
                    if let Some((track, frames)) = actions.set_latency_offset {
                        self.set_plugin_latency_offset(track, frames);
                    }
                    if let Some((track, on)) = actions.set_smart_disable {
                        self.set_plugin_smart_disable(track, on);
                    }
                    if let Some((track, port, target)) = actions.set_extra_out_target {
                        self.set_plugin_extra_out_routing(track, port, target);
                    }
                    if let Some(track) = actions.remove_track {
                        self.remove_plugin_from_track(track);
                    }
                    if let Some(track) = actions.open_gui {
                        self.open_plugin_gui(track);
                    }
                    if let Some(track) = actions.close_gui {
                        self.close_plugin_gui(track);
                    }
                    #[cfg(feature = "plugin-host")]
                    if let Some(path) = actions.sandbox_inspect {
                        self.sandbox_inspect(&path);
                    }
                    #[cfg(not(feature = "plugin-host"))]
                    let _ = actions.sandbox_inspect;
                }
                ViewMode::VocalStudio => {
                    render_vocal_studio_view(
                        ui,
                        &mut self.vocal_studio,
                        &mut self.vocal_harmonizer,
                        self.is_playing,
                        self.current_step,
                        self.bpm,
                        self.playlist_tracks.as_mut(),
                        &mut self.status_message,
                        &mut self.show_mic_settings_modal,
                        &mut self.engine,
                    );
                }
                ViewMode::AiMusicAssistant => {
                    self.refresh_ai_context();
                    let action = render_ai_assistant_view(
                        ui,
                        &mut self.ai_assistant,
                        &mut self.channels,
                        &mut self.status_message,
                        self.ai_pending_plan.is_some(),
                    );
                    if action.ask {
                        self.ai_send_prompt();
                    }
                    if action.apply_plan {
                        self.apply_pending_plan();
                    }
                }
                ViewMode::EffectsMixer => {
                    self.render_effects_mixer_rack(ui);
                }
            }
    }
}

impl SonixApp {
    /// Bakgrundsarbetet: det som händer mellan bildrutorna — oavslutade filer som ska sägas,
    /// MIDI-porten som ansluts en gång, pollarna (hårdvara, bibliotek, patcher, separatorn,
    /// tempot, sträckningen, plugin-GUI:na, sandlådan) och de asynkrona svaren (skärmdump,
    /// filväljaren, genereringen, separatorn).
    ///
    /// Ordningen är avsiktlig: tempoföljningen ligger före `ensure_stretched`, så motorn
    /// redan vet vilket tempo klippen ska följa när filerna beställs.
    pub(crate) fn poll_background_work(&mut self, ctx: &egui::Context) {
        // Källfiler som inte gick att läsa: säg det i statusraden i stället för att tiga.
        // En gång per bildruta, på ett ställe — alla elva anrop omfattas, inte bara
        // projektinläsningen. Ett klipp som ser ut att ha ljud men är tyst är värre än
        // ett som är tomt; då ska det stå varför.
        let unreadable = take_unreadable_sources();
        if !unreadable.is_empty() {
            self.status_message = crate::tstatus!(
                "⚠ {} fil(er) kunde inte läsas och är tysta: {}",
                unreadable.len(),
                unreadable.join(", ")
            )
            .to_string();
        }
        // Auto-open the MIDI keyboard input port once, so external keyboards
        // work as soon as the app is opened — alla in-portar ansluts automatiskt (Fas 7.1:
        // den gamla ALSA-vägen krävde `aconnect`, midir-vägen gör det själv).
        if !self.midi_auto_connect_attempted {
            self.midi_auto_connect_attempted = true;
            if let Ok(midi) = MidiKeyboardInput::connect(self.control_tx.clone()) {
                self.midi_input = Some(midi);
            }
        }
        self.poll_hardware_control();
        self.poll_library_scan();
        self.sync_patcher_graph();
        self.sync_stem_separator_engine();
        self.sync_tempo_follow();
        // Sträckningen är offline (Fas 8.10 steg 2): beställ de filer som saknas och
        // ta emot dem som blivit klara. Ligger efter tempoföljningen, så att motorn
        // redan vet vilket tempo klippen ska följa.
        self.ensure_stretched();
        // Keep embedded plugin editors responsive (Fas 4.4b).
        self.poll_plugin_guis();
        // Supervise the out-of-process sandbox worker (Fas 4.5a).
        #[cfg(feature = "plugin-host")]
        self.poll_plugin_sandbox();

        // Screenshot event listener
        let mut received_screenshot = None;
        ctx.input(|i| {
            for event in &i.raw.events {
                if let egui::Event::Screenshot { image, .. } = event {
                    received_screenshot = Some(image.clone());
                }
            }
        });

        if let Some(img) = received_screenshot
            && let ScreenshotState::AwaitingCapture { dest, .. } = &self.screenshot_state {
                let w = img.size[0] as u32;
                let h = img.size[1] as u32;
                let raw_bytes: Vec<u8> = img.pixels.iter().flat_map(|c| [c.r(), c.g(), c.b(), c.a()]).collect();
                if let Some(rgba) = image::RgbaImage::from_raw(w, h, raw_bytes) {
                    if let Some(parent) = dest.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if let Err(e) = rgba.save(dest) {
                        eprintln!("❌ Kunde inte spara skärmdump till {:?}: {}", dest, e);
                    } else {
                        println!("📸 Sparade skärmdump: {:?}", dest);
                    }
                }
                self.screenshot_state = ScreenshotState::Idle;
            }

        // Check for async file dialog results
        let mut picked_file = None;
        if let Ok(mut lock) = self.pending_file_dialog_result.try_lock()
            && let Some(path) = lock.take() {
                picked_file = Some(path);
            }
        if let Some(path) = picked_file {
            let p_lower = path.to_lowercase();
            let import_tmpl = self.pending_add_track_import.take();
            let beepmap = std::mem::take(&mut self.pending_beepmap);
            if beepmap {
                self.apply_beepmap(&path);
            } else if let Some(t_idx) = self.pending_swap_source.take() {
                self.swap_track_source(t_idx, &path);
            } else if self.pending_stem_separation {
                self.pending_stem_separation = false;
                self.start_stem_separation(&path);
            } else if p_lower.ends_with(".sonix") {
                self.load_project_file(&path);
                self.show_project_manager_modal = false;
            } else if p_lower.ends_with(".zip") {
                self.start_suno_zip_import(&path);
                self.show_suno_import_modal = false;
            } else if let Some(tmpl) = import_tmpl {
                self.import_audio_file_as_track(&path, &tmpl);
            }
        }

        self.poll_remote_audio_generation();
        self.poll_stem_separation();
    }

    /// Klockan och husbytet för en bildruta: sequencern, automationen, animationsfasen,
    /// mätarna, autosparna (både timern och den strukturella ändringen) och mixerns
    /// ångrings-viloläge.
    ///
    /// Mixer-undon är en egen sak: ett reglage ändras kontinuerligt medan man drar, så en
    /// ångringspunkt per bildruta hade fyllt historiken. Viloläget är läget **före** draget.
    pub(crate) fn tick_frame(&mut self, ctx: &egui::Context) {
        self.advance_sequencer();
        self.apply_automation();
        self.anim_phase += 0.08;
        self.update_scope_history();
        self.vocal_studio.update_live_stream();

        // Autosave (Fas 6.1): kontrollera med jämna mellanrum om projektet
        // ändrats sedan det senast skyddades. Fingeravtrycket gör att ett
        // orörligt projekt inte roterar bort sin egen historik.
        self.autosave_accum += ctx.input(|i| i.stable_dt).min(0.5);
        if self.autosave_accum >= crate::autosave::INTERVAL_SECS {
            self.autosave_accum = 0.0;
            self.maybe_autosave();
        }
        // En strukturell ändring (märkt i `push_undo`) skrivs här i stället, när
        // ändringen är genomförd. Ett frame senare är max ~33 ms när fönstret är
        // fokuserat (200 ms oanvänt) — jämfört med upp till 60 s via timern.
        if self.autosave_pending {
            self.autosave_pending = false;
            self.autosave_after_structural_change();
        }

        // Mixer-undon (Fas 6.2). Ett reglage ändras kontinuerligt medan man drar,
        // så en ångringspunkt per frame skulle fylla historiken på ett drag. I
        // stället hålls mixerns "viloläge" — läget från senaste frameen där inget
        // rördes — och läggs som ångringspunkt första gången mixern ändras under
        // en interaktion. Det är exakt läget före draget. Ett nytt viloläge tas
        // först när pekaren är släppt, så ett drag ger en ångring, inte sextio.
        let mixer_now = self.mixer_state_digest();
        let mixer_pointer_down = ctx.input(|i| i.pointer.any_down());
        let mixer_touching =
            mixer_pointer_down || self.mixer_pointer_was_down || self.mixer_control_event;
        if mixer_now != self.mixer_settled_digest {
            let pending = self.mixer_settled_snapshot.take();
            if mixer_touching {
                // Användaren rör mixern: lägg läget före ändringen på historiken.
                if let Some(mut prev) = pending {
                    prev.description = crate::i18n::t("🎚 Mixerändring").to_string();
                    self.push_undo_snapshot(prev);
                }
            } else {
                // Programmatisk ändring (projektladdning, preset, ångring): inget
                // att ångra, men viloläget måste följa med.
                self.mixer_settled_snapshot = Some(self.current_snapshot("mixer"));
                self.mixer_settled_digest = mixer_now;
            }
        }
        self.mixer_pointer_was_down = mixer_pointer_down;
        self.mixer_control_event = false;
        self.sync_mic_monitoring();
    }

    /// Indata för bildrutan: bildfrekvensen (60/30/200 ms), om ett fönster är minimerat eller
    /// ofokuserat, vad som är öppet (`modal_open`), om tangentbordet vill ha tangenterna, och
    /// sedan tangentbordsgenvägarna.
    ///
    /// Besluten stannar här: `piano_active` och `modal_open` behövs bara av genvägarna och
    /// av att släppa hängda toner — ingenting senare i bildrutan läser dem.
    pub(crate) fn handle_input(&mut self, ctx: &egui::Context) {
        // Check if window is minimized or not focused (Wayland / Hyprland safety)
        let is_minimized = ctx.input(|i| i.viewport().minimized.unwrap_or(false));
        let is_focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));

        // Smooth 60 FPS while playing/recording & focused, 30 FPS when idle, gentle 200ms when unfocused/minimized to prevent Wayland crashes
        if self.is_playing || self.vocal_studio.is_recording {
            if is_minimized || !is_focused {
                ctx.request_repaint_after(std::time::Duration::from_millis(33));
            } else {
                ctx.request_repaint_after(std::time::Duration::from_millis(16));
            }
        } else if is_focused && !is_minimized {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(200));
        }

        // Check if user is typing into an input field or if a modal is open
        let is_loading_proj = self.project_load_progress.try_lock().map(|p| p.is_loading).unwrap_or(false);
        let is_importing_stems = self.stem_import_progress.try_lock().map(|p| p.is_importing).unwrap_or(false);
        let modal_open = self.any_modal_open() || is_loading_proj || is_importing_stems;

        let wants_keyboard = ctx.wants_keyboard_input();
        let piano_active = self.view_mode == ViewMode::PianoRoll && !modal_open && !wants_keyboard;

        // Release any stuck notes if piano was switched away or keyboard input was captured
        if !piano_active && !self.active_keys.is_empty() {
            let keys_to_release: Vec<u8> = self.active_keys.iter().copied().collect();
            for note in keys_to_release {
                self.release_note(note);
            }
        }
        if !piano_active && !self.midi_held_notes.is_empty() {
            self.midi_held_notes.clear();
        }

        // **Tab byter yta** (Shift+Tab bakåt). Görs före tangentbordsblocket nedan, så att
        // ytan är bytt när pianots tangenter läses av i samma bildruta.
        if !modal_open
            && !wants_keyboard
            && ctx.input(|i| i.key_pressed(egui::Key::Tab))
        {
            let backwards = ctx.input(|i| i.modifiers.shift);
            self.view_mode = if backwards {
                self.view_mode.prev()
            } else {
                self.view_mode.next()
            };
        }

        // **Zoom i pianorullen**: Ctrl + +/− i 10-procentssteg, Ctrl+0 tillbaka till 1,0.
        if !modal_open
            && !wants_keyboard
            && ctx.input(|i| i.modifiers.ctrl || i.modifiers.command)
        {
            let (plus, minus, reset) = ctx.input(|i| {
                (
                    i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals),
                    i.key_pressed(egui::Key::Minus),
                    i.key_pressed(egui::Key::Num0),
                )
            });
            if plus {
                self.roll_zoom = super::state::zoom_step(self.roll_zoom, 0.1);
            } else if minus {
                self.roll_zoom = super::state::zoom_step(self.roll_zoom, -0.1);
            } else if reset {
                self.roll_zoom = 1.0;
            }
        }

        // Keyboard Shortcuts
        ctx.input(|i| {
            // Virtual Piano keys ONLY when Piano Roll is active on screen, no modal is open, and not typing text
            if piano_active && !i.modifiers.ctrl && !i.modifiers.alt && !i.modifiers.command {
                let key_map = [
                    (egui::Key::A, 0),
                    (egui::Key::W, 1),
                    (egui::Key::S, 2),
                    (egui::Key::E, 3),
                    (egui::Key::D, 4),
                    (egui::Key::F, 5),
                    (egui::Key::T, 6),
                    (egui::Key::G, 7),
                    (egui::Key::Y, 8),
                    (egui::Key::H, 9),
                    (egui::Key::U, 10),
                    (egui::Key::J, 11),
                    (egui::Key::K, 12),
                    (egui::Key::O, 13),
                    (egui::Key::L, 14),
                    (egui::Key::P, 15),
                ];

                for (k, semi) in key_map {
                    let note = ((self.octave + 1) * 12 + semi) as u8;
                    if i.key_pressed(k) {
                        self.play_note(note);
                    } else if i.key_released(k) {
                        self.release_note(note);
                    }
                }
            }

            // Paneler bredvid arrangemanget: Ctrl+1…5 väljer, samma tangent stänger.
            // Ableton har tio `Focus*`-kommandon för samma sak; vi nöjer oss med de fem
            // ytor man faktiskt växlar mellan medan man spelar in.
            let is_cmd0 = i.modifiers.ctrl || i.modifiers.command;
            if is_cmd0 {
                let picks = [
                    (egui::Key::Num1, 0),
                    (egui::Key::Num2, 1),
                    (egui::Key::Num3, 2),
                    (egui::Key::Num4, 3),
                    (egui::Key::Num5, 4),
                ];
                for (key, idx) in picks {
                    if i.key_pressed(key) {
                        let target = DOCKABLE[idx];
                        self.docked_panel = if self.docked_panel == Some(target) {
                            None
                        } else {
                            Some(target)
                        };
                        if self.view_mode != ViewMode::PlaylistArranger {
                            self.view_mode = ViewMode::PlaylistArranger;
                        }
                        self.status_message = match self.docked_panel {
                            Some(v) => crate::tstatus!("🧩 Panel: {} (Ctrl+{} stänger)", view_name(v), idx + 1),
                            None => crate::i18n::t("🧩 Panelerna är stängda").to_string(),
                        };
                    }
                }
            }
            // Zoom: Ableton har ~20 zoomkommandon (Fit, In/Out Time, valt område …).
            // Vi har presets i menyn; här får de tre vanligaste tangenter.
            if is_cmd0 && !modal_open {
                if i.key_pressed(egui::Key::Equals) || i.key_pressed(egui::Key::Plus) {
                    self.suno_zoom_level = (self.suno_zoom_level * 1.25).clamp(0.25, 20.0);
                    self.status_message = crate::tstatus!("🔍 Zoom {:.0} %", self.suno_zoom_level * 100.0);
                }
                if i.key_pressed(egui::Key::Minus) {
                    self.suno_zoom_level = (self.suno_zoom_level * 0.8).clamp(0.25, 20.0);
                    self.status_message = crate::tstatus!("🔍 Zoom {:.0} %", self.suno_zoom_level * 100.0);
                }
                if i.key_pressed(egui::Key::Num0) {
                    self.suno_zoom_level = 1.0;
                    self.status_message = crate::i18n::t("🔍 Zoom 100 %").to_string();
                }
            }

            if i.key_pressed(egui::Key::Escape) && self.docked_panel.is_some() && !modal_open {
                self.docked_panel = None;
            }

            // Global Command / Ctrl shortcuts (ALWAYS active)
            let is_cmd = i.modifiers.ctrl || i.modifiers.command;
            if is_cmd {
                if i.key_pressed(egui::Key::Z) {
                    if i.modifiers.shift {
                        self.redo();
                    } else {
                        self.undo();
                    }
                }
                if i.key_pressed(egui::Key::Y) {
                    self.redo();
                }
                if i.key_pressed(egui::Key::C) {
                    self.copy_selected_region();
                }
                if i.key_pressed(egui::Key::V) {
                    self.paste_region();
                }
                if i.key_pressed(egui::Key::B) || i.key_pressed(egui::Key::X) {
                    self.split_selected_region_at_playhead();
                }
                if i.key_pressed(egui::Key::D) {
                    if i.modifiers.shift || self.selected_audio_region.is_none() {
                        self.duplicate_track(self.selected_timeline_track);
                    } else {
                        self.duplicate_selected_region();
                    }
                }
                if i.key_pressed(egui::Key::L) {
                    self.repeat_selected_region_loop(2.0);
                }
                if i.key_pressed(egui::Key::K) {
                    self.reverse_selected_region();
                }
                if i.key_pressed(egui::Key::N) {
                    self.new_empty_project();
                }
                if i.key_pressed(egui::Key::O) || i.key_pressed(egui::Key::P) {
                    self.show_project_manager_modal = true;
                }
                if i.key_pressed(egui::Key::S) {
                    let p_name = self.project_name.clone();
                    self.save_project(&p_name);
                }
                if i.key_pressed(egui::Key::I) {
                    self.show_suno_import_modal = true;
                }
                if i.key_pressed(egui::Key::E) {
                    self.open_export_modal();
                }
            }

            // Transport & Arranger shortcuts
            if !is_cmd {
                if i.key_pressed(egui::Key::Space) {
                    self.toggle_playback();
                }

                if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                    self.delete_selected_region();
                }

                // Single key shortcuts when piano keys aren't intercepting (or in Arranger view)
                if !piano_active && !i.modifiers.alt {
                    if i.key_pressed(egui::Key::S) || i.key_pressed(egui::Key::B) || i.key_pressed(egui::Key::C) {
                        self.split_selected_region_at_playhead();
                    }
                    if i.key_pressed(egui::Key::M) {
                        self.toggle_mute_selected_region();
                    }
                    if i.key_pressed(egui::Key::R) && !self.is_recording_timeline {
                        self.reverse_selected_region();
                    }
                    if i.key_pressed(egui::Key::Num1) {
                        self.arranger_tool = ArrangerTool::Select;
                        self.status_message = crate::i18n::t("Verktyg: ⇱ Välj / Flytta / Trimma (1)").to_string();
                    }
                    if i.key_pressed(egui::Key::Num2) {
                        self.arranger_tool = ArrangerTool::Paint;
                        self.status_message = crate::i18n::t("Verktyg: ✎ Rita (2)").to_string();
                    }
                    if i.key_pressed(egui::Key::Num3) {
                        self.arranger_tool = ArrangerTool::Slice;
                        self.status_message = crate::i18n::t("Verktyg: ✂ Klipp / Sax (3)").to_string();
                    }
                    if i.key_pressed(egui::Key::Num4) {
                        self.arranger_tool = ArrangerTool::Mute;
                        self.status_message = crate::i18n::t("Verktyg: 🔇 Muta (4)").to_string();
                    }
                    if i.key_pressed(egui::Key::Num5) {
                        self.arranger_tool = ArrangerTool::Erase;
                        self.status_message = crate::i18n::t("Verktyg: 🗑 Radera (5)").to_string();
                    }
                }
            }

            // Global View Shortcuts (F-keys)
            if i.key_pressed(egui::Key::F1) {
                self.show_help_guide = !self.show_help_guide;
            }
            if i.key_pressed(egui::Key::F3) {
                self.view_mode = ViewMode::PlaylistArranger;
            }
            if i.key_pressed(egui::Key::F4) {
                self.view_mode = ViewMode::ChannelRack;
            }
            if i.key_pressed(egui::Key::F5) {
                self.view_mode = ViewMode::PianoRoll;
            }
            if i.key_pressed(egui::Key::F6) {
                self.view_mode = ViewMode::EffectsMixer;
            }
            if i.key_pressed(egui::Key::F7) {
                self.view_mode = ViewMode::AlchemySynth;
            }
            if i.key_pressed(egui::Key::F8) {
                self.view_mode = ViewMode::VocalStudio;
            }
            if i.key_pressed(egui::Key::F9) {
                self.view_mode = ViewMode::AiMusicAssistant;
            }
            if i.key_pressed(egui::Key::F10) {
                self.view_mode = ViewMode::ModularPatcher;
            }
            if i.key_pressed(egui::Key::F11) {
                self.show_chord_generator_modal = !self.show_chord_generator_modal;
            }
            if i.key_pressed(egui::Key::F12) {
                self.show_dice_generator_modal = !self.show_dice_generator_modal;
            }
        });
    }

    /// Filer som släpps på fönstret: ett Suno-zip importeras, en mapp läses som stämmor.
    pub(crate) fn handle_dropped_files(&mut self, ctx: &egui::Context) {
        // Drag & Drop support for Suno ZIP stems or folders
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            for file in dropped_files {
                if let Some(ref path) = file.path {
                    let path_str = path.to_string_lossy().to_string();
                    if path_str.to_lowercase().ends_with(".zip") {
                        self.import_suno_zip(&path_str);
                    } else if path.is_dir() {
                        let (title, bpm) = Self::parse_suno_zip_info(&path_str);
                        self.import_suno_stems_from_folder(&path_str, &title, bpm);
                    }
                }
            }
        }
    }

    /// Ramen kring arbetsytan: menyraden (Arkiv, Redigera, Vy, AI, Inställningar, Hjälp),
    /// verktygsfältet och statusraden.
    pub(crate) fn draw_chrome(&mut self, ctx: &egui::Context) {
        // ====================================================================
        // 0. PERMANENT TOP DESKTOP MENU BAR (ARKIV, REDIGERA, VY, AI, INSTÄLLNINGAR, HJÄLP)
        // ====================================================================
        egui::TopBottomPanel::top("desktop_menu_bar")
            .frame(egui::Frame::none().fill(crate::ui::theme::palette().bg()).inner_margin(egui::Margin::symmetric(8.0, 3.0)))
            .show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    // Arkiv
                    ui.menu_button(self.tr("📁 Arkiv"), |ui| {
                        if ui.button(self.tr("📄 Nytt tomt projekt (Ctrl+N)")).clicked() {
                            self.new_empty_project();
                            ui.close_menu();
                        }
                        // Mallar: ett sparat projekt i `Templates/`. Cubase skeppar 38; vi
                        // har ingen egen uppsättning, men vägen att göra sina egna.
                        let templates = SonixApp::list_templates(&crate::paths::paths().templates_dir());
                        let mut open_template: Option<String> = None;
                        ui.add_enabled_ui(!templates.is_empty(), |ui| {
                            ui.menu_button(self.tr("📄 Nytt från mall"), |ui| {
                                for (name, path) in &templates {
                                    if ui.button(name).on_hover_text(path.display().to_string()).clicked() {
                                        open_template = Some(path.to_string_lossy().to_string());
                                        ui.close_menu();
                                    }
                                }
                            });
                        });
                        if let Some(path) = open_template {
                            self.new_from_template(&path);
                        }
                        if ui.button(self.tr("💾 Spara som mall")).on_hover_text(self.tr("Spara det aktuella läget som en mall i Templates-mappen")).clicked() {
                            self.save_as_template();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("📂 Öppna projekt... (Ctrl+O)")).clicked() {
                            self.show_project_manager_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("💾 Spara projekt (Ctrl+S)")).clicked() {
                            let p_name = self.project_name.clone();
                            self.save_project(&p_name);
                            ui.close_menu();
                        }
                        if ui.button(self.tr("💾 Spara som... (Ctrl+Shift+S)")).clicked() {
                            self.show_project_manager_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("📁 Filhanterare / Projektbläddrare (Ctrl+P)")).clicked() {
                            self.show_project_manager_modal = true;
                            ui.close_menu();
                        }

                        // 🕘 Senaste projekt (Fas 6.1): läser recent.json. Läslistan
                        // är en bekvämlighet — stängs av sig själv om filen är tom.
                        let recent = load_recent_projects();
                        let mut open_recent: Option<String> = None;
                        ui.add_enabled_ui(!recent.is_empty(), |ui| {
                            ui.menu_button(self.tr("🕘 Senaste projekt"), |ui| {
                                for entry in &recent {
                                    let label = format!(
                                        "{}  ({})",
                                        entry.name,
                                        crate::autosave::relative_age(
                                            entry.opened,
                                            crate::autosave::now_stamp()
                                        )
                                    );
                                    if ui.button(label).on_hover_text(&entry.path).clicked() {
                                        open_recent = Some(entry.path.clone());
                                        ui.close_menu();
                                    }
                                }
                            });
                        });
                        if let Some(path) = open_recent {
                            self.load_project_file(&path);
                        }

                        if ui.button(self.tr("📂 Visa projektmappen i filhanteraren")).clicked() {
                            // Öppnar den kanoniska projektmappen i systemets
                            // filhanterare. Ingen egen filbläddrare — det är
                            // användarens filhanterare som gäller.
                            let dir = crate::paths::paths().projects_dir();
                            let _ = std::fs::create_dir_all(&dir);
                            // Plattformens egen filhanterare: xdg-open på Linux,
                            // explorer/open på Windows/macOS (Fas 7.1). Att titta på
                            // starten och inte på slutkoden är med flit — explorer
                            // avslutar med 1 även när den lyckas.
                            if let Err(e) = crate::platform::open_dir(&dir) {
                                self.status_message = crate::tstatus!(
                                    "⚠ Kunde inte öppna '{}' i filhanteraren: {}",
                                    dir.display(),
                                    e
                                );
                            }
                            ui.close_menu();
                        }

                        if ui.button(self.tr("⚡ Ladda Demo-projekt")).clicked() {
                            self.load_demo_project();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎼 Importera Stämmor / Stems... (Ctrl+I)")).clicked() {
                            self.show_suno_import_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("↗ Exportera projekt (Ctrl+E)")).clicked() {
                            self.open_export_modal();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎼 Exportera sång som MIDI (.mid)")).clicked() {
                            self.export_song_midi();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎼 Importera MIDI-fil (.mid)...")).clicked() {
                            self.show_midi_import_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🚪 Avsluta")).clicked() {
                            std::process::exit(0);
                        }
                    });

                    // Redigera
                    ui.menu_button(self.tr("✏ Redigera"), |ui| {
                        if ui.button(self.tr("↶ Ångra (Ctrl+Z)")).clicked() {
                            self.undo();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("↷ Gör om (Ctrl+Y)")).clicked() {
                            self.redo();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("✂ Klipp vid spelhuvud (Ctrl+B / S)")).clicked() {
                            self.split_selected_region_at_playhead();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("📋 Duplicera markerat (Ctrl+D)")).clicked() {
                            self.duplicate_selected_region();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🗑 Ta bort markerat (Del)")).clicked() {
                            self.delete_selected_region();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🧹 Rensa alla spår")).clicked() {
                            self.push_undo(crate::i18n::t("Rensa alla spår"));
                            for t in &mut self.playlist_tracks {
                                t.regions.clear();
                                t.clips = [None; 32];
                                t.clip_bars = [1; 32];
                            }
                            self.selected_audio_region = None;
                            self.status_message = self.tr("Rensade alla spår och tidslinjeklipp.").to_string();
                            ui.close_menu();
                        }
                    });

                    // Vy
                    ui.menu_button(self.tr("👁 Vy"), |ui| {
                        if ui.button(self.tr("📊 Tidslinje / Arranger (F3)")).clicked() {
                            self.view_mode = ViewMode::PlaylistArranger;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🥁 Sonix Channel Rack (F4)")).clicked() {
                            self.view_mode = ViewMode::ChannelRack;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎹 Sonix Piano Roll (F5)")).clicked() {
                            self.view_mode = ViewMode::PianoRoll;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎛 Mixer Console (F6)")).clicked() {
                            self.view_mode = ViewMode::EffectsMixer;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎛 Analog Synthesizer (F7)")).clicked() {
                            self.view_mode = ViewMode::AlchemySynth;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎙 Vocal Studio & Harmonizer (F8)")).clicked() {
                            self.view_mode = ViewMode::VocalStudio;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🤖 AI Music Studio (F9)")).clicked() {
                            self.view_mode = ViewMode::AiMusicAssistant;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🔌 Modulär Synt & Patcher (F10)")).clicked() {
                            self.view_mode = ViewMode::ModularPatcher;
                            ui.close_menu();
                        }
                        ui.separator();
                        let _browser_toggle = if self.show_browser { self.tr("📁 Växla Webbläsare / Bibliotek: PÅ") } else { self.tr("📁 Växla Webbläsare / Bibliotek: AV") };
                        if ui.button(_browser_toggle).clicked() {
                            self.show_browser = !self.show_browser;
                            ui.close_menu();
                        }
                    });

                    // AI & Providers
                    ui.menu_button(self.tr("🤖 AI & Providers"), |ui| {
                        if ui.button(self.tr("⚙ AI-inställningar & API-nycklar...")).clicked() {
                            self.show_ai_settings_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎼 Importera Stämmor / Multi-Track Stems...")).clicked() {
                            self.show_suno_import_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("✨ ACE-Step & Stable Audio Generator...")).clicked() {
                            self.view_mode = ViewMode::PlaylistArranger;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🤖 AI Co-Producer Assistent...")).clicked() {
                            self.view_mode = ViewMode::AiMusicAssistant;
                            ui.close_menu();
                        }
                    });

                    // Verktyg & Kreativa Generatorer
                    ui.menu_button(self.tr("🎛 Verktyg"), |ui| {
                        if ui.button(self.tr("🖼 BeepMap (bild → ljud)…")).clicked() {
                            self.pending_beepmap = true;
                            self.spawn_async_file_picker("Bild eller ljud", &["png", "jpg", "jpeg", "bmp", "wav"], "BeepMap");
                            ui.close_menu();
                        }
                        if ui.button(self.tr("📓 Anteckningar…")).clicked() {
                            self.show_notes_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎲 Melodi- & Beat-Tärning... (F12)")).clicked() {
                            self.show_dice_generator_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎹 Smart Ackord- & Skalgenerator... (F11)")).clicked() {
                            self.show_chord_generator_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎛 Modulärt FX-Pedalbord & Stompboxes...")).clicked() {
                            self.show_fx_rack_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎯 Hårdvarustämapparat & Pitch Scope (Tuner)...")).clicked() {
                            self.show_tuner_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("⏱ Stor klocka...")).clicked() {
                            self.show_big_clock = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🌊 Visualizer...")).clicked() {
                            self.show_visualizer = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("📜 Skript...")).clicked() {
                            self.show_script_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("💃 Fruity Dance")).clicked() {
                            self.show_dance = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🧵 Tagningar (komp)...")).clicked() {
                            self.show_takes_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("📑 Låtstruktur & Formdelar...")).clicked() {
                            self.show_song_structure_modal = true;
                            ui.close_menu();
                        }
                        ui.menu_button(self.tr("🎚 Warp-läge"), |ui| {
                            for mode in crate::audio::warp::WarpMode::ALL {
                                let picked = ui
                                    .selectable_label(self.warp_mode == mode, crate::i18n::t(mode.label()))
                                    .clicked();
                                if picked {
                                    self.warp_mode = mode;
                                    self.status_message = crate::tstatus!(
                                        "🎚 Warp-läge: {} — gäller nästa sträckning",
                                        mode.label()
                                    );
                                    ui.close_menu();
                                }
                            }
                        });
                        if ui.button(self.tr("🎼 Stämning (.ascl)...")).clicked() {
                            self.load_tuning_from_file();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎼 Tillbaka till 12-TET")).clicked() {
                            self.reset_tuning();
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🕹 Modulationsmatris...")).clicked() {
                            self.show_mod_matrix = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎼 Ackordspår...")).clicked() {
                            self.show_chord_track = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🧩 Sektionskedja (Arranger)...")).clicked() {
                            self.show_arranger_chain = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎛 Scenrutnät (mjuk Launchpad)...")).clicked() {
                            // Rutnätet byggs om ur sektionerna när rutan öppnas: låtstrukturen
                            // kan ha ändrats sedan förra gången.
                            self.rebuild_launcher_from_sections();
                            self.show_launcher = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🔗 Makron (kedja över filer)...")).clicked() {
                            // Mappen läses om när dialogen öppnas — en kedja man sparat
                            // utanför appen ska synas utan att starta om.
                            self.macro_state
                                .reload(&crate::paths::paths().macros_dir());
                            self.show_macros_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("🎙 Mikrofonjustering & Inmatningspanel...")).clicked() {
                            self.show_mic_settings_modal = true;
                            ui.close_menu();
                        }
                    });

                    // Inställningar
                    ui.menu_button(self.tr("⚙ Inställningar"), |ui| {
                        if ui.button(self.tr("🎙 Mikrofoninställningar & Inmatningsenhet (Samson/USB)...")).clicked() {
                            self.show_mic_settings_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎛 Ljud- & MIDI-inställningar (PipeWire/ALSA/JACK)...")).clicked() {
                            self.show_audio_settings_modal = true;
                            ui.close_menu();
                        }
                        if ui.button(self.tr("🎛 Hårdvarukontroller (MCU / OSC)...")).clicked() {
                            self.show_controller_modal = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        let current_mode = crate::ui::theme::palette().mode;
                        ui.menu_button(format!("🎨 {} ({})", self.tr("Tema"), self.tr(match current_mode {
                            crate::ui::theme::ThemeMode::Light => "Ljust",
                            crate::ui::theme::ThemeMode::Dark => "Mörkt",
                        })), |ui| {
                            for (mode, label) in [
                                (crate::ui::theme::ThemeMode::Dark, "Mörkt"),
                                (crate::ui::theme::ThemeMode::Light, "Ljust"),
                            ] {
                                if ui.selectable_label(current_mode == mode, self.tr(label)).clicked() {
                                    crate::ui::theme::set_mode(mode);
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            for (name, path) in crate::ui::theme::saved_themes() {
                                if ui.button(format!("🎨 {name}")).clicked() {
                                    self.status_message = match crate::ui::theme::apply_saved_theme(&path) {
                                        Ok(()) => crate::tstatus!("🎨 Temat '{}' laddat", name),
                                        Err(e) => crate::tstatus!("⚠ Kunde inte läsa temat '{}': {}", name, e),
                                    };
                                    ui.close_menu();
                                }
                            }
                            ui.separator();
                            if ui.button(self.tr("📂 Öppna temamappen")).clicked() {
                                let dir = crate::ui::theme::themes_dir();
                                let _ = std::fs::create_dir_all(&dir);
                                if let Err(e) = crate::platform::open_dir(&dir) {
                                    self.status_message = crate::tstatus!("⚠ Kunde inte öppna temamappen: {}", e);
                                }
                                ui.close_menu();
                            }
                            if ui.button(self.tr("⬇ Hämta teman...")).clicked() {
                                // Nätet får aldrig blockera bildrutan: hämta i en tråd och plocka upp.
                                let slot = std::sync::Arc::new(std::sync::Mutex::new(None));
                                self.theme_registry_pending = Some(slot.clone());
                                std::thread::spawn(move || {
                                    let result = crate::ui::theme::fetch_registry();
                                    if let Ok(mut guard) = slot.lock() {
                                        *guard = Some(result);
                                    }
                                });
                                ui.close_menu();
                            }
                            for remote in self.theme_registry.clone() {
                                if ui.button(format!("⬇ {}", remote.name)).clicked() {
                                    self.status_message = match crate::ui::theme::install_theme(&remote) {
                                        Ok(p) => crate::tstatus!(
                                            "🎨 Temat '{}' hämtat till {}",
                                            remote.name,
                                            p.display()
                                        ),
                                        Err(e) => crate::tstatus!(
                                            "⚠ Kunde inte hämta '{}': {}",
                                            remote.name,
                                            e
                                        ),
                                    };
                                    ui.close_menu();
                                }
                            }
                            if ui.button(self.tr("💾 Exportera tema...")).clicked() {
                                let dir = crate::ui::theme::themes_dir();
                                let _ = std::fs::create_dir_all(&dir);
                                // Dialogen pekar i temamappen; filnamnet blir temats namn.
                                if let Some(path) = rfd::FileDialog::new()
                                    .set_directory(&dir)
                                    .set_file_name("mitt-tema.json")
                                    .save_file()
                                {
                                    let name = path
                                        .file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("tema")
                                        .to_string();
                                    self.status_message = match crate::ui::theme::export_current(&name) {
                                        Ok(p) => crate::tstatus!("💾 Temat sparat: {}", p.display()),
                                        Err(e) => crate::tstatus!("⚠ Kunde inte spara temat: {}", e),
                                    };
                                }
                                ui.close_menu();
                            }
                        });
                        ui.separator();
                        ui.menu_button(
                            format!("🌐 {} ({})", self.tr("Språk"), self.language.native_name()),
                            |ui| {
                                for lang in crate::i18n::Language::shipped() {
                                    if ui.selectable_label(self.language == lang, lang.native_name()).clicked() {
                                        self.language = lang;
                                        crate::i18n::set_current(lang);
                                        ui.close_menu();
                                    }
                                }
                            },
                        );
                    });

                    // Hjälp
                    ui.menu_button(self.tr("❓ Hjälp"), |ui| {
                        if ui.button(self.tr("📖 Snabbguide & Manual (F1)")).clicked() {
                            self.show_help_guide = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button(self.tr("ℹ Om Sonix Studio...")).clicked() {
                            self.show_about_modal = true;
                            ui.close_menu();
                        }
                    });

                    // Project indicator & quick status
                    ui.separator();
                    let p_label = format!("{} {}", self.tr("📁 Projekt:"), self.project_name);
                    if ui.add(egui::Button::new(egui::RichText::new(p_label).size(Theme::FONT_S).color(crate::ui::theme::palette().orange())).frame(false)).on_hover_text(self.tr("Klicka för att hantera projekt")).clicked() {
                        self.show_project_manager_modal = true;
                    }
                });
            });

        // ====================================================================
        // 1. TOP METALLIC TOOLBAR
        // ====================================================================
        egui::TopBottomPanel::top("fl_toolbar")
            .frame(egui::Frame::none().fill(crate::ui::theme::palette().header()).inner_margin(6.0))
            .show(ctx, |ui| {
                // ROW 1: Logo, Transport, Master Knobs, Clock, Export & Guide (Responsive Wrapped)
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

                    // Browser Toggle
                    let browser_btn = ui.selectable_label(self.show_browser, self.tr("📁 Browser"));
                    if browser_btn.clicked() { self.show_browser = !self.show_browser; }

                    ui.separator();

                    // **Logotypen, inte en apelsin** (Alex 2026-09-14): samma bild som
                    // fönsterikonen. Faller tillbaka på ordet om bilden inte kan läsas.
                    let logo = self.logo_texture_for(ui.ctx());
                    if let Some(tex) = &logo {
                        ui.add(egui::Image::new(tex).fit_to_exact_size(egui::Vec2::splat(18.0)));
                    }
                    ui.label(egui::RichText::new(crate::i18n::t("SONIX")).strong().size(Theme::FONT_L).color(crate::ui::theme::palette().cyan()));
                    ui.label(egui::RichText::new(crate::i18n::t("STUDIO")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                    ui.separator();

                    // Transport Buttons
                    // **Transportknapparna är ikoner, inte ord** (0.9.82): Ableton har tre stora
                    // knappar där Sonix hade tre ord. Verktygstipset bär texten.
                    let play_color = if self.is_playing { crate::ui::theme::palette().green() } else { crate::ui::theme::palette().panel_alt() };
                    if ui.add(egui::Button::new(egui::RichText::new("▶").strong().size(Theme::FONT_L).color(if self.is_playing { egui::Color32::BLACK } else { crate::ui::theme::palette().text() })).fill(play_color).min_size(egui::vec2(48.0, 30.0))).on_hover_text(crate::i18n::t("Spela upp")).clicked()
                        && !self.is_playing {
                            self.toggle_playback();
                        }

                    let pause_color = if !self.is_playing { crate::ui::theme::palette().orange() } else { crate::ui::theme::palette().panel_alt() };
                    if ui.add(egui::Button::new(egui::RichText::new("⏸").strong().size(Theme::FONT_L).color(if self.is_playing { crate::ui::theme::palette().text() } else { egui::Color32::BLACK })).fill(pause_color).min_size(egui::vec2(48.0, 30.0))).on_hover_text(crate::i18n::t("Pausa")).clicked()
                        && self.is_playing {
                            self.toggle_playback();
                        }

                    if ui.add(egui::Button::new(egui::RichText::new("⏹").strong().size(Theme::FONT_L).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().border()).min_size(egui::vec2(48.0, 30.0))).on_hover_text(crate::i18n::t("Stopp")).clicked() {
                        self.stop_playback();
                    }

                    ui.separator();

                    // Digital LCD Display & Clock (Compact, monospace)
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("BPM")).size(Theme::FONT_XS).color(crate::ui::theme::palette().text_muted()));
                            ui.add_sized(Vec2::new(38.0, 18.0), egui::Label::new(egui::RichText::new(format!("{:.1}", self.bpm)).monospace().strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan())));
                            if ui.button(crate::i18n::t("▲")).clicked() && self.bpm < 240.0 { self.bpm += 1.0; }
                            if ui.button(crate::i18n::t("▼")).clicked() && self.bpm > 40.0 { self.bpm -= 1.0; }

                            ui.separator();

                            let mode_text = if self.pattern_mode { "PAT" } else { "SONG" };
                            let mode_color = if self.pattern_mode { crate::ui::theme::palette().orange() } else { crate::ui::theme::palette().cyan() };
                            if ui.add_sized(Vec2::new(38.0, 18.0), egui::Button::new(egui::RichText::new(mode_text).strong().size(Theme::FONT_XS).color(Color32::BLACK)).fill(mode_color)).clicked() {
                                self.pattern_mode = !self.pattern_mode;
                                self.status_message = (if self.pattern_mode { self.tr("Läge ändrat till: PAT (Mönsterloop)") } else { self.tr("Läge ändrat till: SONG (Låtläge)") }).to_string();
                            }

                            // **Follow action:** syns bara i mönsterläget, där en slinga kan
                            // spela flera varv. Knappen växlar åtgärd, och AV är ett eget läge i
                            // varvet — att stänga av ska inte kräva en kryssruta till.
                            if self.pattern_mode {
                                ui.separator();
                                let current = self.follow.map(|f| f.kind);
                                let label = match current {
                                    Some(kind) => format!("FÖLJ: {}", self.tr(kind.label())),
                                    None => self.tr("FÖLJ: AV").to_string(),
                                };
                                let color = if current.is_some() { crate::ui::theme::palette().orange() } else { crate::ui::theme::palette().text_muted() };
                                if ui.add(egui::Button::new(egui::RichText::new(label).size(Theme::FONT_XS).color(color))).clicked() {
                                    use crate::follow::{FollowAction, FollowKind};
                                    self.follow = match current {
                                        None => Some(FollowAction { kind: FollowKind::Next, ..FollowAction::default() }),
                                        Some(FollowKind::Next) => Some(FollowAction { kind: FollowKind::Previous, ..FollowAction::default() }),
                                        Some(FollowKind::Previous) => Some(FollowAction { kind: FollowKind::First, ..FollowAction::default() }),
                                        Some(FollowKind::First) => Some(FollowAction { kind: FollowKind::Last, ..FollowAction::default() }),
                                        Some(FollowKind::Last) => Some(FollowAction { kind: FollowKind::Again, ..FollowAction::default() }),
                                        Some(FollowKind::Again) => Some(FollowAction { kind: FollowKind::Stop, ..FollowAction::default() }),
                                        Some(FollowKind::Stop) => None,
                                        Some(FollowKind::Jump) => None,
                                    };
                                    self.follow_bars = 0.0;
                                }
                                if let Some(action) = &mut self.follow {
                                    ui.add(egui::DragValue::new(&mut action.after_bars).range(0.25..=64.0).speed(0.25).suffix(" takter").max_decimals(2));
                                    ui.add(egui::DragValue::new(&mut action.chance).range(0.0..=1.0).speed(0.01).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
                                }
                            }

                            ui.separator();

                            let time_str = format_time_hundredths(self.song_time);
                            let (txt, color) = if self.pattern_mode {
                                let bar = (self.current_step / 4) + 1;
                                let step_in_bar = (self.current_step % 4) + 1;
                                (format!("⏱ {} [B{:02}:S{}]", time_str, bar, step_in_bar), crate::ui::theme::palette().orange())
                            } else {
                                let bar = self.song_bar + 1;
                                let step = (self.song_step_in_bar / 4) + 1;
                                (format!("⏱ {} [T{:02}.{}]", time_str, bar, step), crate::ui::theme::palette().cyan())
                            };
                            ui.label(egui::RichText::new(txt).monospace().strong().size(Theme::FONT_S).color(color));
                        });
                    });

                    ui.separator();

                    // Pattern Quick Selector
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("PAT:")).size(Theme::FONT_XS).color(crate::ui::theme::palette().text_muted()));
                            let p_len = self.patterns.len();
                            for pat_idx in 0..p_len {
                                let is_active = self.selected_pattern == pat_idx;
                                let p_color = self.patterns[pat_idx].color;
                                let fill = if is_active { p_color } else { crate::ui::theme::palette().panel() };
                                let text_color = if is_active { Color32::BLACK } else { crate::ui::theme::palette().text() };
                                if ui.add(egui::Button::new(egui::RichText::new(format!("P{}", pat_idx + 1)).strong().size(Theme::FONT_XS).color(text_color)).fill(fill)).clicked() {
                                    self.select_pattern(pat_idx);
                                }
                            }
                            // Fånga in det som spelas som en ny slinga (Abletons
                            // Capture and Insert Scene). Källan blir kvar.
                            if ui
                                .add(egui::Button::new(egui::RichText::new("🎯").size(Theme::FONT_XS)).fill(crate::ui::theme::palette().panel()))
                                .on_hover_text(crate::i18n::t("Fånga in den valda slingan som en ny slinga"))
                                .clicked()
                            {
                                self.capture_pattern();
                            }
                            // FL:s Clone pattern: en kopia att göra varianten i.
                            if ui
                                .add(egui::Button::new(egui::RichText::new("⧉").size(Theme::FONT_XS)).fill(crate::ui::theme::palette().panel()))
                                .on_hover_text(crate::i18n::t("Kopiera mönstret — originalet rörs inte"))
                                .clicked()
                            {
                                self.duplicate_selected_pattern();
                            }
                        });
                    });

                    ui.separator();

                    // Swing Knob
                    rotary_knob(ui, &mut self.swing, 0.0, 1.0, "SWING", crate::ui::theme::palette().yellow(), 20.0);

                    ui.separator();

                    // Oscilloscope Display (real output waveform from audio thread)
                    let peak = self.engine.get_peak_level();
                    oscilloscope_display(ui, &self.scope_history, peak, Vec2::new(50.0, 22.0));

                    ui.separator();

                    // Nivå per kanal och registret (Fas 8.13, Alex 2026-09-13):
                    // scopen visar VÅGFORMEN, mätaren visar NIVÅN per kanal, och
                    // registret visar VAR i frekvensbanden energin ligger.
                    let (peak_l, peak_r) = self.engine.get_stereo_peaks();
                    stereo_meter(ui, peak_l, peak_r, Vec2::new(48.0, 22.0));
                    register_eq(
                        ui,
                        &self.spectrum_levels,
                        &crate::audio::spectrum::REGISTERS.map(|r| r.0),
                        Vec2::new(128.0, 22.0),
                    );

                    ui.separator();

                    // Master Volume & Pan Knobs
                    if rotary_knob(ui, &mut self.master_volume, 0.0, 1.0, "MASTER", crate::ui::theme::palette().orange(), 20.0) {
                        let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                    }
                    rotary_knob(ui, &mut self.master_pan, -1.0, 1.0, "PAN", crate::ui::theme::palette().text_muted(), 20.0);

                    ui.separator();

                    // Export WAV Button
                    let export_label = if self.pattern_mode { "💾 EXPORT PAT" } else { "💾 EXPORT SONG" };
                    if ui.add(egui::Button::new(egui::RichText::new(export_label).strong().size(Theme::FONT_S).color(crate::ui::theme::readable_on(crate::ui::theme::palette().text_muted()))).fill(crate::ui::theme::palette().text_muted())).clicked() {
                        self.export_wav();
                    }

                    // Batch Export & Render Queue Button
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📤 BATCH EXPORT")).strong().size(Theme::FONT_S).color(crate::ui::theme::readable_on(crate::ui::theme::palette().border_soft()))).fill(crate::ui::theme::palette().border_soft())).clicked() {
                        self.open_export_modal();
                    }

                    // Stems Importer Button
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📦 STEMS")).strong().size(Theme::FONT_S).color(crate::ui::theme::readable_on(crate::ui::theme::palette().purple()))).fill(crate::ui::theme::palette().purple())).clicked() {
                        self.scan_for_suno_stems();
                        self.show_suno_import_modal = true;
                    }

                    // Hardware Controller MCU / OSC Button
                    let mcu_col = if self.mcu_connected { crate::ui::theme::palette().cable() } else { crate::ui::theme::palette().panel() };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎛 MCU")).strong().size(Theme::FONT_S).color(crate::ui::theme::readable_on(mcu_col))).fill(mcu_col)).clicked() {
                        self.show_controller_modal = true;
                    }

                });

                ui.add_space(2.0);
                ui.separator();
                ui.add_space(2.0);

                // ROW 2: Primary Workspaces & Advanced Studios Navigation Tabs (Responsive Wrapped)
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(3.0, 3.0);
                    // **Flikar i stället för en lång rad** (GUI-planen, punkt 1): bara den valda
                    // gruppens vyer visas. Samma knappar som förut, färre att leta bland.
                    // **Flikar efter vad man gör** (GUI-planen, punkt 1): Skapa, Arrangera,
                    // Mixa, Verktyg. Bara den valda gruppens knappar ritas, och varje vy har
                    // en hemvist — inte en rubrik.
                    let active_group = nav_group_of(self.view_mode);
                    for (group, label) in
                        [(0u8, "SKAPA"), (1, "ARRANGERA"), (2, "MIXA"), (3, "VERKTYG")]
                    {
                        if ui
                            .selectable_label(active_group == group, self.tr(label))
                            .clicked()
                            && group != active_group
                        {
                            // **Fliken minns senast valda vy i sin grupp** (0.9.78), så ett
                            // klick hit går tillbaka dit man var — inte till gruppens första.
                            self.view_mode = self.last_view[group as usize];
                        }
                    }
                    ui.separator();

                    const VIEWS: [(u8, ViewMode, &str); 13] = [
                        (0, ViewMode::ChannelRack, "🥁 Rack"),
                        (0, ViewMode::PianoRoll, "🎹 Piano"),
                        (0, ViewMode::VocalStudio, "🎙 Sångstudio"),
                        (0, ViewMode::AlchemySynth, "✨ Alchemy"),
                        (0, ViewMode::SessionDrummer, "🥁 Trummor"),
                        (0, ViewMode::ModularPatcher, "🧩 Patcher"),
                        (0, ViewMode::AiMusicAssistant, "🤖 AI"),
                        (1, ViewMode::PlaylistArranger, "🎼 Tidslinje"),
                        (1, ViewMode::Video, "🎬 Video"),
                        (2, ViewMode::EffectsMixer, "🎚 Mixer"),
                        (2, ViewMode::RemixFx, "🎛 Remix"),
                        (2, ViewMode::StemSeparator, "🧠 Stems"),
                        (2, ViewMode::PluginManager, "🔌 Plugins"),
                    ];
                    for (group, mode, label) in VIEWS {
                        if active_group == group
                            && ui
                                .selectable_label(self.view_mode == mode, self.tr(label))
                                .clicked()
                        {
                            self.view_mode = mode;
                            self.last_view[group as usize] = mode;
                        }
                    }

                    if active_group == 3 {
                        if ui.add(egui::Button::new(egui::RichText::new(self.tr("🎲 Tärning")).strong().size(Theme::FONT_XS).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().red())).on_hover_text(self.tr("Idé- & Slumptärning för Melodier & Beats (F12)")).clicked() {
                            self.show_dice_generator_modal = true;
                        }
                        if ui.add(egui::Button::new(egui::RichText::new(self.tr("🎹 Ackord")).strong().size(Theme::FONT_XS).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().text_muted())).on_hover_text(self.tr("Smart Ackord- & Skalgenerator (F11)")).clicked() {
                            self.show_chord_generator_modal = true;
                        }
                        if ui.add(egui::Button::new(egui::RichText::new(self.tr("🎛 FX-Rack")).strong().size(Theme::FONT_XS).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().purple())).on_hover_text(self.tr("Modulärt FX-Pedalbord & Stompbox Rack (Ctrl+R)")).clicked() {
                            self.show_fx_rack_modal = true;
                        }
                        if ui.add(egui::Button::new(egui::RichText::new(self.tr("🎯 Stämmare")).strong().size(Theme::FONT_XS).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().green())).on_hover_text(self.tr("Hårdvarustämapparat & Pitch Analyzer (Ctrl+T)")).clicked() {
                            self.show_tuner_modal = true;
                        }
                        if ui.add(egui::Button::new(egui::RichText::new(self.tr("📑 Formdelar")).strong().size(Theme::FONT_XS).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().amber())).on_hover_text(self.tr("Låtstruktur & Formdelar")).clicked() {
                            self.show_song_structure_modal = true;
                        }
                    }
                });
            });

        // Bottom Status Bar
        egui::TopBottomPanel::bottom("fl_status_bar")
            .frame(egui::Frame::none().fill(crate::ui::theme::palette().panel()).inner_margin(4.0))
            .show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(8.0, 2.0);
                    ui.label(egui::RichText::new(&self.status_message).size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                    let drops = self.engine.command_drops();
                    if drops > 0 {
                        ui.label(
                            egui::RichText::new(format!("⚠ {} {}", drops, self.tr("tappade kommandon")))
                                .size(Theme::FONT_S)
                                .color(crate::ui::theme::palette().yellow()),
                        )
                        .on_hover_text(self.tr(
                            "Ljudtråden hann inte ta emot kommandon. Spela om handlingen om gränssnittet och ljudet inte stämmer.",
                        ));
                    }
                    ui.separator();
                    ui.label(egui::RichText::new(self.tr("PipeWire / ALSA 44.1kHz • 16-Stämmor Polyfoni • Sonix Studio Pro DAW")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                });
            });
    }

    /// Arbetsytan själv: Sound Browser-panelen till vänster och mittpanelen, som ritar den vy
    /// som är vald (arrangör, kanalrack, mixern, piano roll, patchern, Sångstudion …).
    pub(crate) fn draw_workspace(&mut self, ctx: &egui::Context) {
        // ====================================================================
        // 2. LEFT SIDEBAR BROWSER (FL STUDIO & APPLE LOOPS LIBRARY)
        // ====================================================================
        if self.show_browser {
            egui::SidePanel::left("fl_browser")
                .frame(egui::Frame::none().fill(crate::ui::theme::palette().panel()).inner_margin(8.0))
                .resizable(true)
                .default_width(200.0)
                .show(ctx, |ui| {
                    self.render_browser_sidebar(ui);
                });
        }

        // ====================================================================
        // 3. MAIN WORKSPACE (VIEW MODES)
        // ====================================================================
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(crate::ui::theme::palette().bg()).inner_margin(12.0))
            .show(ctx, |ui| {
                // Paneler i stället för sidor (Abletons modell): i arrangemanget står ytan kvar
                // och den valda vyn ritas bredvid i stället för att ersätta den.
                let mut close_dock = false;
                let dock = if self.view_mode == ViewMode::PlaylistArranger {
                    self.docked_panel
                } else {
                    None
                };
                if let Some(panel) = dock {
                    egui::SidePanel::right("sonix_dock")
                        .resizable(true)
                        .default_width(470.0)
                        .min_width(320.0)
                        .frame(
                            egui::Frame::none()
                                .fill(crate::ui::theme::palette().panel())
                                .inner_margin(8.0)
                                .stroke(egui::Stroke::new(1.0_f32, crate::ui::theme::palette().cyan())),
                        )
                        .show_inside(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(view_name(panel))
                                        .strong()
                                        .size(Theme::FONT_S)
                                        .color(crate::ui::theme::palette().cyan()),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui
                                            .small_button("✕")
                                            .on_hover_text(crate::i18n::t("Stäng panelen (Esc)"))
                                            .clicked()
                                        {
                                            close_dock = true;
                                        }
                                    },
                                );
                            });
                            ui.separator();
                            egui::ScrollArea::vertical()
                                .id_salt("sonix_dock_scroll")
                                .auto_shrink([false, false])
                                .show(ui, |ui| {
                                    self.render_view(panel, ui, ctx);
                                });
                        });
                }
                if close_dock {
                    self.docked_panel = None;
                }
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.render_view(self.view_mode, ui, ctx);
                });
            });
    }

    /// Skärmdumpsloopen (`--capture-screenshots`): väntar på en ruta, och **avbryter med
    /// besked** om ingen kommer — den väntar inte för evigt.
    pub(crate) fn screenshot_tick(&mut self, ctx: &egui::Context) {
        // Screenshot automated capture loop
        if self.screenshot_mode_active {
            match self.screenshot_state.clone() {
                ScreenshotState::Idle => {
                    if !self.screenshot_queue.is_empty() {
                        let (target, dest) = self.screenshot_queue.remove(0);
                        self.apply_screenshot_target(target.clone());
                        self.screenshot_state = ScreenshotState::Preparing { target, dest, frames_left: 4 };
                        ctx.request_repaint();
                    } else {
                        // Antalet står inte här: listan växer när en dialog läggs
                        // till, och en siffra i en utskrift blir fel tyst.
                        println!("🎉 Alla skärmdumpar har genererats och sparats framgångsrikt!");
                        self.screenshot_mode_active = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                ScreenshotState::Preparing { target, dest, frames_left } => {
                    if frames_left > 0 {
                        self.screenshot_state = ScreenshotState::Preparing { target, dest, frames_left: frames_left - 1 };
                        ctx.request_repaint();
                    } else {
                        self.screenshot_state = ScreenshotState::AwaitingCapture {
                            dest,
                            frames_left: SCREENSHOT_WAIT_FRAMES,
                        };
                        ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot);
                        ctx.request_repaint();
                    }
                }
                ScreenshotState::AwaitingCapture { dest, frames_left } => {
                    if frames_left == 0 {
                        // Svaret kom aldrig. Säg det, städa och stäng — en loop som
                        // väntar för evigt ser ut att arbeta, och det är värre än
                        // ett tydligt fel.
                        eprintln!(
                            "❌ Ingen skärmdump kom tillbaka för {:?} efter {} bildrutor. \
                             Ingen ruta har besvarat begäran — körningen avbryts så att felet syns.",
                            dest, SCREENSHOT_WAIT_FRAMES
                        );
                        self.screenshot_queue.clear();
                        self.screenshot_state = ScreenshotState::Idle;
                        self.screenshot_mode_active = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.screenshot_state =
                            ScreenshotState::AwaitingCapture { dest, frames_left: frames_left - 1 };
                        ctx.request_repaint();
                    }
                }
            }
        }
    }
}
