use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use crate::audio::recorder::{RecordingMode, VocalStudioTrack};
use crate::audio::vocal_harmonizer::VocalHarmonizer;
use crate::ui::app::{AudioRegion, PlaylistTrack};
use crate::ui::theme::Theme;
use crate::ui::widgets::rotary_knob;

pub fn render_vocal_studio_view(
    ui: &mut Ui,
    vocal_track: &mut VocalStudioTrack,
    harmonizer: &mut VocalHarmonizer,
    is_playing: bool,
    current_step: usize,
    bpm: f32,
    playlist_tracks: &mut Vec<PlaylistTrack>,
    status_msg: &mut String,
    show_mic_settings: &mut bool,
    engine: &mut crate::audio::AudioEngine,
) {
    // Clear stale "playing" indicators once the isolated audition has ended.
    if !engine.is_audition_playing() {
        for t in &mut vocal_track.takes {
            t.is_playing = false;
        }
    }

    ui.group(|ui| {
        // ============================================================
        // 1. TOP HEADER & AUDIO ROUTING STATUS
        // ============================================================
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎙 SONIX STUDIO RECORDER")).strong().size(Theme::FONT_L).color(crate::ui::theme::palette().orange()));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Mikrofoninspelning i realtid  •  Live WAV Capture  •  Waveform Editor  •  Pitch Editor")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⚙ Mik-panel")).size(Theme::FONT_S).strong().color(Color32::BLACK)).fill(crate::ui::theme::palette().cyan())).clicked() {
                    *show_mic_settings = true;
                }
                let dev_name = vocal_track.mic_capture.as_ref().map(|m| m.device_name.as_str()).unwrap_or("Standard Mikrofon");
                let sr = vocal_track.mic_capture.as_ref().map(|m| m.sample_rate).unwrap_or(44100);
                ui.label(egui::RichText::new(format!("🎛 {} | {} Hz", dev_name, sr)).size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
            });
        });

        ui.add_space(4.0);

        // Sub-tabs for the Studio
        ui.horizontal(|ui| {
            let voc_tab = ui.selectable_label(vocal_track.recording_mode == RecordingMode::LeadVocals, crate::i18n::t("🎙 1. Sång & Mikrofoninspelning (Take Lanes & Studio Deck)"));
            if voc_tab.clicked() { vocal_track.recording_mode = RecordingMode::LeadVocals; }

            let custom_tab = ui.selectable_label(vocal_track.recording_mode == RecordingMode::CustomSounds, crate::i18n::t("🎤 2. Spela in Egna Ljud & Sampler"));
            if custom_tab.clicked() { vocal_track.recording_mode = RecordingMode::CustomSounds; }
        });

        ui.separator();
        ui.add_space(4.0);

        match vocal_track.recording_mode {
            RecordingMode::LeadVocals => {
                // ============================================================
                // 2. STUDIO RECORDING DECK (TRANSPORT, VU METER, LIVE MONITOR)
                // ============================================================
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        // Channel & Arm status
                        ui.label(egui::RichText::new(crate::i18n::t("🔴 INSPELNINGSDECK")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                        ui.separator();

                        ui.checkbox(&mut vocal_track.is_armed, "🔴 Armera");
                        ui.checkbox(&mut vocal_track.monitoring_on, "🎧 Direct Monitoring")
                            .on_hover_text(crate::i18n::t(
                                "Spelar upp mikrofonen genom mastern (zero-latency). Använd hörlurar — med högtalare uppstår rundgång.",
                            ));
                        if vocal_track.monitoring_on {
                            rotary_knob(ui, &mut vocal_track.monitor_level, 0.0, 1.0, "MONITOR", crate::ui::theme::palette().cyan(), 18.0);
                        }
                        ui.checkbox(&mut vocal_track.realtime_autotune, crate::i18n::t("🎙️ Realtids-Auto-Tune"))
                            .on_hover_text(crate::i18n::t("Korrigerar mikrofonen i realtid mot vald skala (styrka = AUTO-TUNE-ratten)"));

                        ui.separator();
                        rotary_knob(ui, &mut vocal_track.input_gain, 0.0, 2.5, "GAIN", crate::ui::theme::palette().yellow(), 18.0);
                        rotary_knob(ui, &mut harmonizer.autotune_speed, 0.0, 1.0, "AUTO-TUNE", crate::ui::theme::palette().orange(), 18.0);

                        ui.separator();

                        // VU METER WITH PEAK dB DISPLAY
                        ui.vertical(|ui| {
                            let vu_db = if vocal_track.mic_vu_level > 0.0001 {
                                (20.0 * vocal_track.mic_vu_level.log10()).max(-48.0)
                            } else {
                                -48.0
                            };
                            ui.label(egui::RichText::new(format!("MIC IN: {:.1} dB", vu_db)).size(Theme::FONT_XS).color(crate::ui::theme::palette().text_muted()));

                            let (vu_rect, _) = ui.allocate_exact_size(Vec2::new(100.0, 14.0), Sense::hover());
                            ui.painter().rect_filled(vu_rect, Rounding::same(2.0), crate::ui::theme::palette().bg_deep());
                            ui.painter().rect_stroke(vu_rect, Rounding::same(2.0), Stroke::new(1.0_f32, crate::ui::theme::palette().grid()));

                            let vu_fill_w = (vu_rect.width() * vocal_track.mic_vu_level.clamp(0.0, 1.0)).max(2.0);
                            let vu_col = if vocal_track.mic_vu_level > 0.90 {
                                crate::ui::theme::palette().red() // Red clip
                            } else if vocal_track.mic_vu_level > 0.70 {
                                crate::ui::theme::palette().orange() // Yellow/Orange warm
                            } else {
                                crate::ui::theme::palette().green() // Green safe
                            };
                            ui.painter().rect_filled(Rect::from_min_size(vu_rect.min, Vec2::new(vu_fill_w, vu_rect.height())), Rounding::same(2.0), vu_col);
                        });

                        ui.separator();

                        // TIME COUNTER & STATE
                        ui.vertical(|ui| {
                            let mins = (vocal_track.recording_elapsed_secs / 60.0).floor() as u32;
                            let secs = (vocal_track.recording_elapsed_secs % 60.0).floor() as u32;
                            let centis = ((vocal_track.recording_elapsed_secs * 100.0) % 100.0) as u32;
                            let sec_per_bar = (60.0 / bpm.max(40.0)) * 4.0;
                            let current_bars = (vocal_track.recording_elapsed_secs / sec_per_bar) + 1.0;

                            let timer_color = if vocal_track.is_recording {
                                if vocal_track.is_paused { crate::ui::theme::palette().yellow() } else { crate::ui::theme::palette().red() }
                            } else {
                                crate::ui::theme::palette().text_muted()
                            };

                            let state_icon = if vocal_track.is_recording {
                                if vocal_track.is_paused { "⏸ PAUSAD" } else { "🔴 REC" }
                            } else {
                                "⏹ STANDBY"
                            };

                            ui.label(egui::RichText::new(format!("{:02}:{:02}.{:02} ({} {:.1})", mins, secs, centis, crate::i18n::t("Takt"), current_bars)).strong().size(Theme::FONT_S).color(timer_color));
                            ui.label(egui::RichText::new(state_icon).size(Theme::FONT_XS).color(timer_color));
                        });

                        ui.add_space(8.0);

                        // ==========================================
                        // TRANSPORT CONTROLS: REC, PAUSE, RESUME, STOP
                        // ==========================================
                        if !vocal_track.is_recording {
                            if ui.add(
                                egui::Button::new(egui::RichText::new(crate::i18n::t(" 🔴 SPELA IN (REC) ")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text()))
                                    .fill(crate::ui::theme::palette().red())
                                    .min_size(Vec2::new(130.0, 32.0)),
                            ).clicked() {
                                match vocal_track.start_recording() {
                                    Ok(()) => *status_msg = crate::i18n::t("🔴 Spelar in mikrofonljud i realtid... Sjung eller spela nu!").to_string(),
                                    Err(e) => *status_msg = format!("❌ {}", e),
                                }
                            }
                        } else {
                            if vocal_track.is_paused {
                                if ui.add(
                                    egui::Button::new(egui::RichText::new(crate::i18n::t(" ▶ ÅTERUPPTA ")).strong().size(Theme::FONT_S).color(Color32::BLACK))
                                        .fill(crate::ui::theme::palette().yellow())
                                        .min_size(Vec2::new(90.0, 32.0)),
                                ).clicked() {
                                    vocal_track.resume_recording();
                                    *status_msg = crate::i18n::t("▶ Återupptog inspelningen!").to_string();
                                }
                            } else {
                                if ui.add(
                                    egui::Button::new(egui::RichText::new(crate::i18n::t(" ⏸ PAUSA ")).strong().size(Theme::FONT_S).color(Color32::BLACK))
                                        .fill(crate::ui::theme::palette().yellow())
                                        .min_size(Vec2::new(80.0, 32.0)),
                                ).clicked() {
                                    vocal_track.pause_recording();
                                    *status_msg = crate::i18n::t("⏸ Inspelning pausad.").to_string();
                                }
                            }

                            ui.add_space(4.0);

                            if ui.add(
                                egui::Button::new(egui::RichText::new(crate::i18n::t(" ⏹ STOPPA & SKAPA TAGNING ")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text()))
                                    .fill(crate::ui::theme::palette().green())
                                    .min_size(Vec2::new(140.0, 32.0)),
                            ).clicked() {
                                match vocal_track.stop_recording() {
                                    Ok(idx) => {
                                        let name = vocal_track.takes[idx].name.clone();
                                        *status_msg = crate::tstatus!("✔ Sparade tagning: {}! Klar för redigering och tidslinje.", name);
                                    }
                                    Err(e) => *status_msg = format!("❌ {}", e),
                                }
                            }
                        }
                    });

                    ui.add_space(4.0);

                    // LIVE SCROLLING WAVEFORM MONITOR (DURING RECORDING)
                    if vocal_track.is_recording {
                        let w_width = ui.available_width().max(200.0);
                        let (live_rect, _) = ui.allocate_exact_size(Vec2::new(w_width, 42.0), Sense::hover());
                        let painter = ui.painter();
                        painter.rect_filled(live_rect, Rounding::same(4.0), crate::ui::theme::palette().bg_deep());
                        painter.rect_stroke(live_rect, Rounding::same(4.0), Stroke::new(1.0_f32, crate::ui::theme::palette().red()));

                        let mid_y = live_rect.center().y;
                        painter.line_segment([Pos2::new(live_rect.min.x, mid_y), Pos2::new(live_rect.max.x, mid_y)], Stroke::new(1.0_f32, crate::ui::theme::palette().border()));

                        let peaks = &vocal_track.live_recording_peaks;
                        if !peaks.is_empty() {
                            let step_x = (live_rect.width() / peaks.len().max(1) as f32).max(1.5);
                            for (i, &p) in peaks.iter().enumerate() {
                                let x = live_rect.min.x + i as f32 * step_x;
                                if x > live_rect.max.x { break; }
                                let h = p * (live_rect.height() * 0.45);
                                let col = if vocal_track.is_paused {
                                    crate::ui::theme::palette().yellow()
                                } else if p > 0.85 {
                                    crate::ui::theme::palette().red()
                                } else {
                                    crate::ui::theme::palette().cyan()
                                };
                                painter.line_segment([Pos2::new(x, mid_y - h), Pos2::new(x, mid_y + h)], Stroke::new(1.5_f32, col));
                            }
                        }

                        let status_text = if vocal_track.is_paused { crate::i18n::t("⏸ INSPELNING PAUSAD") } else { "🔴 SPELAR IN LIVE FRÅN MIKROFON..." };
                        painter.text(
                            Pos2::new(live_rect.min.x + 8.0, live_rect.min.y + 6.0),
                            egui::Align2::LEFT_TOP,
                            status_text,
                            egui::FontId::proportional(Theme::FONT_S),
                            if vocal_track.is_paused { crate::ui::theme::palette().yellow() } else { crate::ui::theme::palette().red() },
                        );
                    }
                });

                ui.add_space(6.0);

                // ============================================================
                // 3. INTERACTIVE AUDIO TAKE EDITOR & WAVEFORM TRIMMER / CROPPER
                // ============================================================
                let selected_idx = vocal_track.active_comp_take;
                let num_takes = vocal_track.takes.len();
                if num_takes > 0 && selected_idx < num_takes {
                    let mut do_import_pcm: Option<(String, Vec<f32>, u32)> = None;
                    let mut do_export_wav = false;
                    let mut do_crop = false;
                    let mut do_comp_add = false;
                    let mut do_comp_build = false;
                    let mut do_slice = false;
                    let mut do_normalize = false;
                    let mut do_delete = false;
                    let mut crop_start_norm = vocal_track.selected_crop_start_norm;
                    let mut crop_end_norm = vocal_track.selected_crop_end_norm;

                    ui.group(|ui| {
                        let take = &mut vocal_track.takes[selected_idx];
                        let take_name = take.name.clone();
                        let duration = take.duration_secs;
                        let sample_count = take.pcm_samples.len();
                        let take_color = take.color;
                        let take_sr = take.sample_rate;

                        // Row 1: Header + Action Buttons
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("✂ LJUDVÅGSEDITOR & SAMPLE FORMARE")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().yellow()));
                            ui.separator();
                            ui.label(egui::RichText::new(format!("Aktiv: {} ({:.2}s, {} samples)", take_name, duration, sample_count)).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // INSERT INTO PLAYLIST BUTTON
                                if ui.add(
                                    egui::Button::new(egui::RichText::new(crate::i18n::t("📥 Infoga i Tidslinje (Arranger)")).strong().size(Theme::FONT_S).color(Color32::BLACK))
                                        .fill(crate::ui::theme::palette().green()),
                                ).clicked() {
                                    if !playlist_tracks.is_empty() {
                                        let region_id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as usize;
                                        let sec_per_bar = (60.0 / bpm.max(40.0)) * 4.0;
                                        let effective_len_sec = duration * take.time_stretch;
                                        let length_bars = (effective_len_sec / sec_per_bar).max(0.25);
                                        let target_idx = playlist_tracks.iter().position(|t| t.is_rec_armed || t.kind == crate::ui::app::TrackKind::VocalAudio).unwrap_or(0);
                                        let pcm_arc = std::sync::Arc::new(take.pcm_samples.clone());
                                        playlist_tracks[target_idx].pcm_audio = Some((pcm_arc.clone(), pcm_arc, take.sample_rate));

                                        let region = AudioRegion {
                                            source_bpm: bpm, // tagningen gjordes i projektets tempo (8.10)
                                            // En tagning är inte en bandspelare: blir tempot ett
                                            // annat sträcks den med bevarad tonhöjd (8.10 steg 2).
                                            tape: false,
                                            id: region_id,
                                            name: format!("🎙 {}", take_name),
                                            start_bar: 0.0,
                                            length_bars,
                                            sample_offset_sec: 0.0,
                                            source_path: None,
                                            waveform_peaks: take.waveform_data.clone(),
                                            volume: take.gain_linear,
                                            fade_in_bars: 0.0,
                                            fade_out_bars: 0.0,
                                            muted: false,
                                            is_reverse: take.is_reverse,
                                            color: playlist_tracks[target_idx].color,
                                            loop_length_bars: 0.0,
                                            warp_mode: Default::default(),
};
                                        playlist_tracks[target_idx].regions.push(region);
                                        *status_msg = crate::tstatus!("✔ Infogade '{}' ({:.2}s) som ljudregion i Spår {} ({})!", take_name, duration, target_idx + 1, playlist_tracks[target_idx].name);
                                    }
                                }

                                // EXPORT TO WAV BUTTON
                                if ui.add(
                                    egui::Button::new(egui::RichText::new(crate::i18n::t("💾 Exportera WAV")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text()))
                                        .fill(crate::ui::theme::palette().track_blue()),
                                ).clicked() {
                                    do_export_wav = true;
                                }

                                // IMPORT AUDIO FILE BUTTON
                                if ui.add(
                                    egui::Button::new(egui::RichText::new(crate::i18n::t("📂 Importera Fil")).size(Theme::FONT_S).color(crate::ui::theme::palette().text()))
                                        .fill(crate::ui::theme::palette().border_soft()),
                                ).on_hover_text(crate::i18n::t("Ladda in extern WAV/Audio-fil som tagning")).clicked() {
                                    // Plattformens egen dialog (rfd). Anropet är
                                    // synkront, precis som zenity var — en modal
                                    // filväljare håller kvar tills användaren svarat.
                                    let picked = rfd::FileDialog::new()
                                        .set_title(crate::i18n::t("Välj Ljudfil"))
                                        .add_filter("Ljudfiler", &["wav", "mp3", "flac", "ogg"])
                                        .pick_file();
                                    if let Some(picked) = picked {
                                        let path = picked.to_string_lossy().into_owned();
                                        if !path.is_empty() {
                                            if let Ok((l, _, sr)) = crate::audio::load_wav_pcm(&path) {
                                                let p = std::path::Path::new(&path);
                                                let f_name = p.file_stem().and_then(|s| s.to_str()).unwrap_or("Importerat Ljud").to_string();
                                                do_import_pcm = Some((f_name, l, sr));
                                            } else {
                                                *status_msg = crate::tstatus!("❌ Kunde inte läsa ljudfilen: {}", path);
                                            }
                                        }
                                    }
                                }
                            });
                        });

                        ui.add_space(4.0);

                        // Row 2: Isolated Audition & Playback Deck (Does not start timeline!)
                        ui.horizontal(|ui| {
                            let is_auditioning = take.is_playing;
                            if ui.add(
                                egui::Button::new(egui::RichText::new(if is_auditioning { "🔊 PROVSPELAR..." } else { "▶ Provspela (Solo)" }).strong().size(Theme::FONT_S).color(if is_auditioning { Color32::BLACK } else { crate::ui::theme::palette().text() }))
                                    .fill(if is_auditioning { crate::ui::theme::palette().yellow() } else { crate::ui::theme::palette().green() })
                                    .min_size(Vec2::new(140.0, 24.0)),
                            ).on_hover_text(crate::i18n::t("Spela upp ENBART denna tagning/sample utan att starta låtens tidslinje")).clicked() {
                                take.is_playing = true;
                                let pitch_mul = ((take.pitch_semitones + take.pitch_cents / 100.0) / 12.0).exp2();
                                let total_s = take.pcm_samples.len();
                                let crop_s = vocal_track.selected_crop_start_norm.clamp(0.0, 0.99);
                                let crop_e = vocal_track.selected_crop_end_norm.clamp(crop_s + 0.01, 1.0);
                                let s_idx = (total_s as f32 * crop_s) as usize;
                                let e_idx = ((total_s as f32 * crop_e) as usize).min(total_s);
                                let slice_pcm = if s_idx < e_idx && e_idx <= total_s {
                                    take.pcm_samples[s_idx..e_idx].to_vec()
                                } else {
                                    take.pcm_samples.clone()
                                };
                                let pcm_arc = std::sync::Arc::new(slice_pcm);
                                let _ = engine.send_command(crate::audio::AudioCommand::PlayAudition {
                                    left: pcm_arc.clone(),
                                    right: pcm_arc,
                                    sample_rate: take_sr as f32,
                                    volume: take.gain_linear,
                                    pitch_ratio: pitch_mul,
                                    time_stretch_ratio: take.time_stretch,
                                    is_reverse: take.is_reverse,
                                    loop_playback: take.loop_audition,
                                });
                                *status_msg = crate::tstatus!("▶ Provspelar '{}' isolerat i Sångstudion (Pitch: {:+.1} st, Stretch: {:.2}x, Vol: {:.0}%)", take_name, take.pitch_semitones, take.time_stretch, take.gain_linear * 100.0);
                            }

                            if ui.add(
                                egui::Button::new(egui::RichText::new(crate::i18n::t("⏹ Stoppa")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text()))
                                    .fill(crate::ui::theme::palette().red())
                                    .min_size(Vec2::new(70.0, 24.0)),
                            ).on_hover_text(crate::i18n::t("Stoppa provspelning")).clicked() {
                                take.is_playing = false;
                                let _ = engine.send_command(crate::audio::AudioCommand::StopAudition);
                            }

                            ui.checkbox(&mut take.loop_audition, "🔁 Loopa provspelning");

                            ui.separator();

                            // Reverse toggle
                            let rev_bg = if take.is_reverse { crate::ui::theme::palette().orange() } else { crate::ui::theme::palette().border() };
                            if ui.add(egui::Button::new(egui::RichText::new(if take.is_reverse { crate::i18n::t("🔄 Baklänges (PÅ)") } else { crate::i18n::t("🔄 Normal") }).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(rev_bg)).clicked() {
                                take.is_reverse = !take.is_reverse;
                                if take.is_playing {
                                    // Trigger re-play with reverse
                                    let pitch_mul = ((take.pitch_semitones + take.pitch_cents / 100.0) / 12.0).exp2();
                                    let pcm_arc = std::sync::Arc::new(take.pcm_samples.clone());
                                    let _ = engine.send_command(crate::audio::AudioCommand::PlayAudition {
                                        left: pcm_arc.clone(),
                                        right: pcm_arc,
                                        sample_rate: take_sr as f32,
                                        volume: take.gain_linear,
                                        pitch_ratio: pitch_mul,
                                        time_stretch_ratio: take.time_stretch,
                                        is_reverse: take.is_reverse,
                                        loop_playback: take.loop_audition,
                                    });
                                }
                            }

                            if ui.button(crate::i18n::t("↩ Nollställ effekter")).on_hover_text(crate::i18n::t("Återställ volym, pitch och stretch till standard")).clicked() {
                                take.gain_linear = 1.0;
                                take.pitch_semitones = 0.0;
                                take.pitch_cents = 0.0;
                                take.time_stretch = 1.0;
                                take.is_reverse = false;
                                let _ = engine.send_command(crate::audio::AudioCommand::SetAuditionParams {
                                    volume: 1.0,
                                    pitch_ratio: 1.0,
                                    time_stretch_ratio: 1.0,
                                });
                            }
                        });

                        ui.add_space(4.0);

                        // Row 3: Audio Shaping Knobs & Sliders (Volume, Time Dilation / Stretch, Pitch Shift)
                        ui.group(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);
                            ui.horizontal_wrapped(|ui| {
                                // 1. Volume
                                ui.label(egui::RichText::new(crate::i18n::t("🎚 Volym:")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                                let vol_db = if take.gain_linear <= 0.001 { -60.0 } else { 20.0 * take.gain_linear.log10() };
                                ui.label(egui::RichText::new(format!("{:.1}dB ({:.0}%)", vol_db, take.gain_linear * 100.0)).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                                if ui.add_sized(Vec2::new(100.0, 18.0), egui::Slider::new(&mut take.gain_linear, 0.0..=2.0).show_value(false)).changed() {
                                    let pitch_mul = ((take.pitch_semitones + take.pitch_cents / 100.0) / 12.0).exp2();
                                    let _ = engine.send_command(crate::audio::AudioCommand::SetAuditionParams {
                                        volume: take.gain_linear,
                                        pitch_ratio: pitch_mul,
                                        time_stretch_ratio: take.time_stretch,
                                    });
                                }

                                ui.separator();

                                // 2. Time Dilation / Stretch (Hastighet / Tidssträckning)
                                ui.label(egui::RichText::new(crate::i18n::t("⏳ Time Dilation (Stretch):")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                                ui.label(egui::RichText::new(format!("{:.2}x ({:.0}%)", take.time_stretch, take.time_stretch * 100.0)).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                                if ui.add_sized(Vec2::new(110.0, 18.0), egui::Slider::new(&mut take.time_stretch, 0.25..=3.0).show_value(false)).on_hover_text(crate::i18n::t("Justera tidssträckning / uppspelningshastighet (0.25x = snabbare, 2.0x = långsammare)")).changed() {
                                    let pitch_mul = ((take.pitch_semitones + take.pitch_cents / 100.0) / 12.0).exp2();
                                    let _ = engine.send_command(crate::audio::AudioCommand::SetAuditionParams {
                                        volume: take.gain_linear,
                                        pitch_ratio: pitch_mul,
                                        time_stretch_ratio: take.time_stretch,
                                    });
                                }
                                if ui.button(crate::i18n::t("1.0x")).on_hover_text(crate::i18n::t("Återställ till normal hastighet")).clicked() {
                                    take.time_stretch = 1.0;
                                    let pitch_mul = ((take.pitch_semitones + take.pitch_cents / 100.0) / 12.0).exp2();
                                    let _ = engine.send_command(crate::audio::AudioCommand::SetAuditionParams {
                                        volume: take.gain_linear,
                                        pitch_ratio: pitch_mul,
                                        time_stretch_ratio: 1.0,
                                    });
                                }

                                ui.separator();

                                // 3. Pitch Shift (Tonhöjd i halvtoner & cents)
                                ui.label(egui::RichText::new(crate::i18n::t("🎵 Pitch:")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                                ui.label(egui::RichText::new(format!("{:+.0} st / {:+.0} ct", take.pitch_semitones, take.pitch_cents)).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                                if ui.add_sized(Vec2::new(100.0, 18.0), egui::Slider::new(&mut take.pitch_semitones, -24.0..=24.0).step_by(1.0).show_value(false)).on_hover_text(crate::i18n::t("Justera tonhöjd i halvtoner (-24 .. +24)")).changed() {
                                    let pitch_mul = ((take.pitch_semitones + take.pitch_cents / 100.0) / 12.0).exp2();
                                    let _ = engine.send_command(crate::audio::AudioCommand::SetAuditionParams {
                                        volume: take.gain_linear,
                                        pitch_ratio: pitch_mul,
                                        time_stretch_ratio: take.time_stretch,
                                    });
                                }
                                if ui.add_sized(Vec2::new(70.0, 18.0), egui::Slider::new(&mut take.pitch_cents, -50.0..=50.0).show_value(false)).on_hover_text(crate::i18n::t("Finjustera tonhöjd i cents (-50 .. +50)")).changed() {
                                    let pitch_mul = ((take.pitch_semitones + take.pitch_cents / 100.0) / 12.0).exp2();
                                    let _ = engine.send_command(crate::audio::AudioCommand::SetAuditionParams {
                                        volume: take.gain_linear,
                                        pitch_ratio: pitch_mul,
                                        time_stretch_ratio: take.time_stretch,
                                    });
                                }
                                if ui.button(crate::i18n::t("0 st")).on_hover_text(crate::i18n::t("Nollställ tonhöjd")).clicked() {
                                    take.pitch_semitones = 0.0;
                                    take.pitch_cents = 0.0;
                                    let _ = engine.send_command(crate::audio::AudioCommand::SetAuditionParams {
                                        volume: take.gain_linear,
                                        pitch_ratio: 1.0,
                                        time_stretch_ratio: take.time_stretch,
                                    });
                                }
                            });
                        });

                        ui.add_space(4.0);

                        // WAVEFORM CANVAS WITH SELECTION BOUNDARIES
                        let canvas_w = ui.available_width().max(200.0);
                        let (canvas_rect, _) = ui.allocate_exact_size(Vec2::new(canvas_w, 90.0), Sense::click_and_drag());
                        let painter = ui.painter();

                        // Canvas background & grid
                        painter.rect_filled(canvas_rect, Rounding::same(4.0), crate::ui::theme::palette().bg_deep());
                        painter.rect_stroke(canvas_rect, Rounding::same(4.0), Stroke::new(1.0_f32, crate::ui::theme::palette().grid()));

                        let mid_y = canvas_rect.center().y;
                        painter.line_segment([Pos2::new(canvas_rect.min.x, mid_y), Pos2::new(canvas_rect.max.x, mid_y)], Stroke::new(1.0_f32, crate::ui::theme::palette().grid()));

                        let crop_s = crop_start_norm.clamp(0.0, 0.99);
                        let crop_e = crop_end_norm.clamp(crop_s + 0.01, 1.0);

                        let sel_start_x = canvas_rect.min.x + crop_s * canvas_rect.width();
                        let sel_end_x = canvas_rect.min.x + crop_e * canvas_rect.width();

                        // Dimmed unselected regions
                        if sel_start_x > canvas_rect.min.x {
                            let left_dim = Rect::from_min_max(canvas_rect.min, Pos2::new(sel_start_x, canvas_rect.max.y));
                            painter.rect_filled(left_dim, Rounding::ZERO, Color32::from_black_alpha(140));
                        }
                        if sel_end_x < canvas_rect.max.x {
                            let right_dim = Rect::from_min_max(Pos2::new(sel_end_x, canvas_rect.min.y), canvas_rect.max);
                            painter.rect_filled(right_dim, Rounding::ZERO, Color32::from_black_alpha(140));
                        }

                        // Selected region highlight overlay
                        let sel_rect = Rect::from_min_max(Pos2::new(sel_start_x, canvas_rect.min.y), Pos2::new(sel_end_x, canvas_rect.max.y));
                        painter.rect_filled(sel_rect, Rounding::ZERO, Color32::from_rgba_unmultiplied(take_color.r(), take_color.g(), take_color.b(), 25));

                        // Exakt väg (8.3): (min, max) per kolumn ur tagningens EGNA
                        // samplar. Den gamla vägen ritade `mid ± h` ur en grov översikt
                        // (max(abs), tak 512) — ett symmetriskt hölje som såg trovärdigt
                        // ut men inte var signalen. Finns paren ritas de; annars gamla.
                        let wave = &take.waveform_data;
                        let pairs = &take.waveform_pairs;
                        if !pairs.is_empty() {
                            let col_w = canvas_rect.width() / pairs.len() as f32;
                            let half = canvas_rect.height() * 0.44;
                            let gain = take.gain_linear.clamp(0.0, 4.0);
                            for (i, (lo, hi)) in pairs.iter().enumerate() {
                                let x = canvas_rect.min.x + i as f32 * col_w;
                                let norm_pos = i as f32 / pairs.len() as f32;
                                let in_sel = norm_pos >= crop_s && norm_pos <= crop_e;
                                let y_hi = mid_y - (hi * gain).clamp(-1.0, 1.0) * half;
                                let y_lo = mid_y - (lo * gain).clamp(-1.0, 1.0) * half;
                                let (top, bot) = (y_hi.min(y_lo), y_hi.max(y_lo));
                                let col = if in_sel { take_color } else { crate::ui::theme::palette().border_soft() };
                                painter.rect_filled(
                                    Rect::from_min_max(
                                        Pos2::new(x, top),
                                        Pos2::new((x + col_w * 0.85).max(x + 1.0), (bot).max(top + 0.8)),
                                    ),
                                    Rounding::same(1.0),
                                    col,
                                );
                            }
                        } else if !wave.is_empty() {
                            let step_x = canvas_rect.width() / wave.len() as f32;
                            for (i, &amp) in wave.iter().enumerate() {
                                let x = canvas_rect.min.x + i as f32 * step_x;
                                let norm_pos = i as f32 / wave.len() as f32;
                                let in_sel = norm_pos >= crop_s && norm_pos <= crop_e;

                                let h = (amp * take.gain_linear).clamp(0.0, 1.0) * (canvas_rect.height() * 0.44);
                                let col = if in_sel { take_color } else { crate::ui::theme::palette().border_soft() };
                                painter.rect_filled(
                                    Rect::from_min_max(Pos2::new(x, mid_y - h), Pos2::new((x + step_x * 0.85).max(x + 1.0), mid_y + h)),
                                    Rounding::same(1.0),
                                    col,
                                );
                            }
                        }

                        // Left & Right boundary markers
                        painter.line_segment([Pos2::new(sel_start_x, canvas_rect.min.y), Pos2::new(sel_start_x, canvas_rect.max.y)], Stroke::new(2.0_f32, crate::ui::theme::palette().yellow()));
                        painter.line_segment([Pos2::new(sel_end_x, canvas_rect.min.y), Pos2::new(sel_end_x, canvas_rect.max.y)], Stroke::new(2.0_f32, crate::ui::theme::palette().yellow()));

                        // Start & End handle flags
                        painter.rect_filled(Rect::from_min_size(Pos2::new(sel_start_x - 3.0, canvas_rect.min.y), Vec2::new(6.0, 12.0)), Rounding::same(2.0), crate::ui::theme::palette().yellow());
                        painter.rect_filled(Rect::from_min_size(Pos2::new(sel_end_x - 3.0, canvas_rect.max.y - 12.0), Vec2::new(6.0, 12.0)), Rounding::same(2.0), crate::ui::theme::palette().yellow());

                        ui.add_space(4.0);

                        // DUAL TRIM SLIDERS & EDIT ACTIONS
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Start (In):")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            let s_val = crop_start_norm * duration;
                            ui.label(egui::RichText::new(format!("{:.2}s", s_val)).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().yellow()));
                            ui.add_sized(Vec2::new(120.0, 18.0), egui::Slider::new(&mut crop_start_norm, 0.0..=0.98).show_value(false));

                            ui.separator();

                            ui.label(egui::RichText::new(crate::i18n::t("Slut (Ut):")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            let e_val = crop_end_norm * duration;
                            ui.label(egui::RichText::new(format!("{:.2}s", e_val)).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().yellow()));
                            ui.add_sized(Vec2::new(120.0, 18.0), egui::Slider::new(&mut crop_end_norm, 0.02..=1.0).show_value(false));

                            ui.separator();

                            // CROP BUTTON
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✂ Beskär till Markering")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().yellow())).clicked() {
                                do_crop = true;
                            }

                            // SLICE BUTTON
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✂ Dela Tagning")).size(Theme::FONT_S).color(crate::ui::theme::palette().text()))).clicked() {
                                do_slice = true;
                            }

                            ui.separator();
                            ui.label(egui::RichText::new(crate::i18n::t("Kompfönster")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("➕ Lägg partiet i kompen")).size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().green())).on_hover_text(crate::i18n::t("Markerar det valda intervallet på den valda tagningen som en del av kompen. Partierna läggs i ordning — senare vinner över tidigare där de överlappar.")).clicked() {
                                do_comp_add = true;
                            }
                            if ui.add_enabled(!vocal_track.comp_regions.is_empty(), egui::Button::new(egui::RichText::new(crate::tstatus!("🎚 Bygg komp ({})", vocal_track.comp_regions.len())).size(Theme::FONT_S)).fill(crate::ui::theme::palette().cyan())).clicked() {
                                do_comp_build = true;
                            }

                            // NORMALIZE BUTTON
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎚 Normalisera")).size(Theme::FONT_S).color(crate::ui::theme::palette().text()))).clicked() {
                                do_normalize = true;
                            }

                            // DELETE BUTTON
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🗑 Ta bort")).size(Theme::FONT_S).color(crate::ui::theme::palette().red()))).clicked() {
                                do_delete = true;
                            }
                        });
                    });

                    vocal_track.selected_crop_start_norm = crop_start_norm;
                    vocal_track.selected_crop_end_norm = crop_end_norm;

                    if do_export_wav {
                        let filename = format!("recordings/take_{}.wav", selected_idx + 1);
                        match vocal_track.export_take_to_wav(selected_idx, &filename) {
                            Ok(msg) => *status_msg = msg,
                            Err(err) => *status_msg = crate::tstatus!("❌ Fel vid export: {}", err),
                        }
                    } else if do_crop {
                        match vocal_track.crop_selected_take() {
                            Ok(msg) => *status_msg = msg,
                            Err(err) => *status_msg = format!("❌ {}", err),
                        }
                    } else if do_comp_add {
                        match vocal_track.add_comp_region() {
                            Ok(msg) => *status_msg = msg,
                            Err(err) => *status_msg = format!("❌ {}", err),
                        }
                    } else if do_comp_build {
                        match vocal_track.render_comp_take() {
                            Ok(msg) => {
                                *status_msg = msg;
                                vocal_track.comp_regions.clear();
                            }
                            Err(err) => *status_msg = format!("❌ {}", err),
                        }
                    } else if do_slice {
                        let split_point = crop_start_norm.max(0.2);
                        match vocal_track.slice_selected_take_at(split_point) {
                            Ok(msg) => *status_msg = msg,
                            Err(err) => *status_msg = format!("❌ {}", err),
                        }
                    } else if do_normalize {
                        match vocal_track.normalize_selected_take() {
                            Ok(msg) => *status_msg = msg,
                            Err(err) => *status_msg = format!("❌ {}", err),
                        }
                    } else if do_delete {
                        match vocal_track.delete_selected_take() {
                            Ok(msg) => *status_msg = msg,
                            Err(err) => *status_msg = format!("❌ {}", err),
                        }
                    } else if let Some((f_name, l, sr)) = do_import_pcm {
                        let new_idx = vocal_track.load_sample_or_region_as_take(&f_name, l, sr, crate::ui::theme::palette().cyan());
                        *status_msg = crate::tstatus!("✔ Importerade '{}' till Sångstudion (Tagning {})!", f_name, new_idx + 1);
                    }

                    ui.add_space(6.0);
                }

                // ============================================================
                // 4. SÅNGTAGNINGAR (TAKE LANES & COMPING)
                // ============================================================
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("📁 SÅNGTAGNINGAR & COMPING LANES")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                        ui.separator();
                        ui.label(egui::RichText::new(format!("{} {} {}", crate::i18n::t("Totalt:"), vocal_track.takes.len(), crate::i18n::t("tagningar sparade i projektet"))).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                        ui.separator();
                        // **Lanes**: infoga en bana efter den valda, ta bort den valda, och
                        // lägg hela den valda banan i kompen utan att gå via markeringen.
                        if ui.button(egui::RichText::new(crate::i18n::t("➕ bana")).size(Theme::FONT_S))
                            .on_hover_text(crate::i18n::t("Lägger en tom bana efter den valda"))
                            .clicked()
                        {
                            let at = vocal_track.insert_lane(vocal_track.active_comp_take);
                            *status_msg = crate::tstatus!("➕ Ny bana {} av {}", at + 1, vocal_track.takes.len());
                        }
                        if ui.add_enabled(vocal_track.takes.len() > 1, egui::Button::new(egui::RichText::new(crate::i18n::t("➖ bana")).size(Theme::FONT_S)))
                            .on_hover_text(crate::i18n::t("Tar bort den valda banan"))
                            .clicked()
                        {
                            let lane = vocal_track.active_comp_take;
                            *status_msg = if vocal_track.delete_lane(lane) {
                                crate::tstatus!("➖ Tog bort bana {}", lane + 1)
                            } else {
                                crate::i18n::t("Spåret måste ha minst en bana").to_string()
                            };
                        }
                        if ui.button(egui::RichText::new(crate::i18n::t("→ komp")).size(Theme::FONT_S))
                            .on_hover_text(crate::i18n::t("Lägger hela den valda banan i kompen"))
                            .clicked()
                        {
                            let lane = vocal_track.active_comp_take;
                            // shortcut: hela banan i 16-delssteg vid 120 BPM (samma antagande som COMP_STEP_SECS), räkna om när kompen ska följa projektets tempo
                            let steps = vocal_track.takes.get(lane).map(|t| t.duration_secs * 8.0).unwrap_or(0.0);
                            *status_msg = if vocal_track.copy_into_comp(lane, 0.0, steps) {
                                crate::tstatus!("→ Kompen: la till hela bana {}", lane + 1)
                            } else {
                                crate::i18n::t("Banan har inget ljud att kompa").to_string()
                            };
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(crate::i18n::t("➕ Skapa Testton (2s)")).on_hover_text(crate::i18n::t("Lägger till en hörbar referenston (220 Hz) som du kan redigera")).clicked() {
                                let name = format!("{} {}", crate::i18n::t("Tagning"), vocal_track.takes.len() + 1);
                                vocal_track.add_new_take(&name);
                                *status_msg = crate::tstatus!("✔ Skapade hörbar testton: {}", name);
                            }
                        });
                    });

                    ui.add_space(4.0);

                    // Kompraden: hur kompen ser ut när partierna läggs i ordning. Varje band
                    // får sin tagnings färg, så det syns vilken tagning som ligger var.
                    if !vocal_track.comp_regions.is_empty() {
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(crate::i18n::t("Komp"))
                                    .strong()
                                    .size(Theme::FONT_S)
                                    .color(crate::ui::theme::palette().green()),
                            );
                            let w_w = (ui.available_width() - 8.0).max(10.0);
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(w_w, 22.0), Sense::hover());
                            let painter = ui.painter();
                            painter.rect_filled(rect, Rounding::same(3.0), crate::ui::theme::palette().bg_deep());
                            let length = vocal_track
                                .comp_regions
                                .iter()
                                .map(|(_, _, end)| *end)
                                .max()
                                .unwrap_or(1)
                                .max(1);
                            for (r_take, r_start, r_end) in &vocal_track.comp_regions {
                                let color = vocal_track
                                    .takes
                                    .get(*r_take)
                                    .map(|t| t.color)
                                    .unwrap_or(crate::ui::theme::palette().cyan());
                                let x0 = rect.min.x + rect.width() * (*r_start as f32 / length as f32);
                                let x1 = rect.min.x + rect.width() * (*r_end as f32 / length as f32);
                                painter.rect_filled(
                                    Rect::from_min_max(
                                        Pos2::new(x0, rect.min.y + 1.0),
                                        Pos2::new(x1.max(x0 + 1.0), rect.max.y - 1.0),
                                    ),
                                    Rounding::same(2.0),
                                    color,
                                );
                            }
                        });
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new(crate::i18n::t("Töm kompen")).size(Theme::FONT_S),
                            ))
                            .clicked()
                        {
                            vocal_track.comp_regions.clear();
                        }
                        ui.add_space(4.0);
                    }

                    for t_idx in 0..vocal_track.takes.len() {
                        let is_active = vocal_track.active_comp_take == t_idx;
                        let take_color = vocal_track.takes[t_idx].color;
                        let take_name = vocal_track.takes[t_idx].name.clone();
                        let take_duration = vocal_track.takes[t_idx].duration_secs;
                        let wave_data = vocal_track.takes[t_idx].waveform_data.clone();

                        ui.horizontal(|ui| {
                            let btn_bg = if is_active { take_color } else { crate::ui::theme::palette().panel() };
                            let btn_text_col = if is_active { Color32::BLACK } else { crate::ui::theme::palette().text() };

                            if ui.add(
                                egui::Button::new(egui::RichText::new(&take_name).strong().size(Theme::FONT_S).color(btn_text_col))
                                    .fill(btn_bg)
                                    .min_size(Vec2::new(180.0, 30.0)),
                            ).clicked() {
                                vocal_track.active_comp_take = t_idx;
                                for (i, t) in vocal_track.takes.iter_mut().enumerate() {
                                    t.is_selected = i == t_idx;
                                }
                                vocal_track.selected_crop_start_norm = 0.0;
                                vocal_track.selected_crop_end_norm = 1.0;
                                *status_msg = crate::tstatus!("Valde {} för aktiv redigering och comping", take_name);
                            }

                            // Waveform display for each lane
                            let w_w = (ui.available_width() - 8.0).max(10.0);
                            let (w_rect, w_resp) = ui.allocate_exact_size(Vec2::new(w_w, 30.0), Sense::click());
                            let painter = ui.painter();

                            painter.rect_filled(w_rect, Rounding::same(3.0), crate::ui::theme::palette().bg());
                            painter.rect_stroke(w_rect, Rounding::same(3.0), Stroke::new(1.0_f32, if is_active { take_color } else { crate::ui::theme::palette().grid() }));

                            let num_samples = wave_data.len();
                            if num_samples > 0 {
                                let step_w = (w_rect.width() - 6.0).max(1.0) / num_samples as f32;
                                let mid_y = w_rect.center().y;

                                for (s_i, &amp) in wave_data.iter().enumerate() {
                                    let sx = w_rect.min.x + 3.0 + s_i as f32 * step_w;
                                    let h = amp * (w_rect.height() * 0.44);
                                    let bar_rect = Rect::from_min_max(Pos2::new(sx, mid_y - h), Pos2::new(sx + step_w * 0.85, mid_y + h));
                                    painter.rect_filled(bar_rect, Rounding::same(1.0), if is_active { take_color } else { crate::ui::theme::palette().border_soft() });
                                }
                            }

                            // Kompens partier på den här lanen: samma band som kompen byggs av,
                            // så man ser vilka ställen som är tagna härifrån.
                            let total_samples = vocal_track.takes[t_idx].pcm_samples.len().max(1);
                            for (r_take, r_start, r_end) in &vocal_track.comp_regions {
                                if *r_take != t_idx {
                                    continue;
                                }
                                let x0 = w_rect.min.x
                                    + 3.0
                                    + w_rect.width() * (*r_start as f32 / total_samples as f32);
                                let x1 = w_rect.min.x
                                    + 3.0
                                    + w_rect.width() * (*r_end as f32 / total_samples as f32);
                                let band = Rect::from_min_max(
                                    Pos2::new(x0, w_rect.min.y + 1.0),
                                    Pos2::new(x1.max(x0 + 1.0), w_rect.max.y - 1.0),
                                );
                                painter.rect_filled(
                                    band,
                                    Rounding::same(2.0),
                                    Color32::from_rgba_unmultiplied(
                                        crate::ui::theme::palette().green().r(),
                                        crate::ui::theme::palette().green().g(),
                                        crate::ui::theme::palette().green().b(),
                                        70,
                                    ),
                                );
                            }

                            // Klick på ett markerat band tar bort det partiet — det är så man
                            // ångrar en kompning utan att bygga om allt.
                            if w_resp.clicked() {
                                if let Some(pos) = w_resp.interact_pointer_pos() {
                                    let frac = ((pos.x - w_rect.min.x - 3.0)
                                        / (w_rect.width() - 6.0).max(1.0))
                                        .clamp(0.0, 1.0);
                                    let at = (frac * total_samples as f32) as usize;
                                    let before = vocal_track.comp_regions.len();
                                    vocal_track
                                        .comp_regions
                                        .retain(|(rt, rs, re)| !(*rt == t_idx && at >= *rs && at < *re));
                                    if vocal_track.comp_regions.len() != before {
                                        *status_msg = crate::tstatus!(
                                            "Tog bort ett komp-parti ur {}",
                                            take_name
                                        );
                                    }
                                }
                            }

                            // Badge label showing duration
                            painter.text(
                                Pos2::new(w_rect.max.x - 8.0, w_rect.center().y),
                                egui::Align2::RIGHT_CENTER,
                                format!("{:.1}s", take_duration),
                                egui::FontId::proportional(Theme::FONT_S),
                                if is_active { crate::ui::theme::palette().text() } else { crate::ui::theme::palette().text_muted() },
                            );
                        });
                        ui.add_space(2.0);
                    }
                });

                ui.add_space(6.0);

                // ============================================================
                // 5. MELODYNE & HARMONIZER
                // ============================================================
                let act_idx = vocal_track.active_comp_take.min(vocal_track.takes.len().saturating_sub(1));
                ui.columns(2, |cols| {
                    cols[0].group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("🎼 PITCH EDITOR – TONKORRIGERING (NOTBLOCK)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().yellow()));
                        });
                        ui.add_space(4.0);

                        // Real analysis / correction actions for the selected take
                        ui.horizontal_wrapped(|ui| {
                            let has_take = !vocal_track.takes.is_empty();
                            if ui.add_enabled(has_take, egui::Button::new(egui::RichText::new(crate::i18n::t("🎯 Analysera tonhöjd")).size(Theme::FONT_S))).on_hover_text(crate::i18n::t("Detekterar tonhöjden i den valda tagningen och ritar riktiga notblock")).clicked() {
                                let take = &vocal_track.takes[act_idx];
                                let pcm = take.pcm_samples.clone();
                                let sr = take.sample_rate as f32;
                                harmonizer.analyze_take(&pcm, sr, 16);
                                *status_msg = crate::tstatus!("🎯 Analyserade {} notblock ur tagningen", harmonizer.blobs.len());
                            }

                            if ui.add_enabled(has_take, egui::Button::new(egui::RichText::new(crate::i18n::t("✨ Auto-Tune")).color(Color32::BLACK).size(Theme::FONT_S)).fill(crate::ui::theme::palette().orange())).on_hover_text(crate::i18n::t("Korrigerar tonhöjden mot vald skala (styrka = AUTO-TUNE-ratten)")).clicked() {
                                let corrected = {
                                    let take = &vocal_track.takes[act_idx];
                                    harmonizer.apply_autotune(&take.pcm_samples, take.sample_rate as f32)
                                };
                                vocal_track.replace_active_take_pcm("(Auto-Tune)", corrected);
                                *status_msg = crate::i18n::t("✨ Auto-Tune applicerad på vald tagning").to_string();
                            }

                            let has_edits = harmonizer.blobs.iter().any(|b| b.pitch_offset.abs() > 0.001);
                            if ui.add_enabled(has_take && has_edits, egui::Button::new(egui::RichText::new(crate::i18n::t("🎯 Applicera notkorrigering")).color(Color32::BLACK).size(Theme::FONT_S)).fill(crate::ui::theme::palette().cyan())).on_hover_text(crate::i18n::t("Pitch-shiftar tagningen per notblock enligt dina dragningar")).clicked() {
                                let corrected = {
                                    let take = &vocal_track.takes[act_idx];
                                    harmonizer.apply_blob_corrections(&take.pcm_samples, take.sample_rate as f32)
                                };
                                vocal_track.replace_active_take_pcm("(Pitch Edit)", corrected);
                                for b in &mut harmonizer.blobs { b.pitch_offset = 0.0; }
                                *status_msg = crate::i18n::t("🎯 Notkorrigering applicerad på tagningen").to_string();
                            }

                            if ui.add_enabled(!harmonizer.blobs.is_empty(), egui::Button::new(egui::RichText::new(crate::i18n::t("↩ Nollställ notblock")).size(Theme::FONT_S))).clicked() {
                                for b in &mut harmonizer.blobs { b.pitch_offset = 0.0; }
                                harmonizer.selected_blob = None;
                            }

                            ui.label(egui::RichText::new(crate::i18n::t("Skala:")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            egui::ComboBox::from_id_salt("tuner_scale_combo")
                                .selected_text(match harmonizer.target_scale {
                                    1 => crate::i18n::t("Dur"),
                                    2 => crate::i18n::t("Moll"),
                                    3 => crate::i18n::t("Dur-pentatonisk"),
                                    4 => crate::i18n::t("Moll-pentatonisk"),
                                    5 => crate::i18n::t("Dorisk"),
                                    6 => crate::i18n::t("Mixolydisk"),
                                    7 => crate::i18n::t("Blues"),
                                    _ => crate::i18n::t("Kromatisk"),
                                })
                                .show_ui(ui, |ui| {
                                    for (idx, name) in [
                                        (0usize, crate::i18n::t("Kromatisk")),
                                        (1, crate::i18n::t("Dur")),
                                        (2, crate::i18n::t("Moll")),
                                        (3, crate::i18n::t("Dur-pentatonisk")),
                                        (4, crate::i18n::t("Moll-pentatonisk")),
                                        (5, crate::i18n::t("Dorisk")),
                                        (6, crate::i18n::t("Mixolydisk")),
                                        (7, crate::i18n::t("Blues")),
                                    ] {
                                        ui.selectable_value(&mut harmonizer.target_scale, idx, name);
                                    }
                                });
                        });


                        let m_w = ui.available_width().max(10.0);
                        let (m_rect, m_resp) = ui.allocate_exact_size(Vec2::new(m_w, 120.0), Sense::click_and_drag());
                        let painter = ui.painter();
                        painter.rect_filled(m_rect, Rounding::same(4.0), crate::ui::theme::palette().bg_deep());
                        painter.rect_stroke(m_rect, Rounding::same(4.0), Stroke::new(1.0_f32, crate::ui::theme::palette().border()));

                        let note_rows = 6;
                        let row_h = m_rect.height() / note_rows as f32;
                        for r in 0..note_rows {
                            let ry = m_rect.min.y + r as f32 * row_h;
                            painter.line_segment([Pos2::new(m_rect.min.x, ry), Pos2::new(m_rect.max.x, ry)], Stroke::new(1.0_f32, crate::ui::theme::palette().panel()));
                        }

                        let total_steps = harmonizer.blob_steps.max(1);
                        let step_width = (m_rect.width() - 10.0).max(1.0) / total_steps as f32;
                        let pitch_to_y = |note: f32| -> f32 {
                            m_rect.max.y - ((note - 58.0) * (row_h * 0.75)).clamp(15.0, (m_rect.height() - 25.0).max(15.0))
                        };
                        for (i, blob) in harmonizer.blobs.iter().enumerate() {
                            let bx = m_rect.min.x + 5.0 + blob.start_step as f32 * step_width;
                            let bw = (blob.length_steps as f32 * step_width - 3.0).max(4.0);
                            let by = pitch_to_y(blob.midi_note as f32 + blob.pitch_offset);
                            let blob_rect = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(bw, 18.0));
                            painter.rect_filled(blob_rect, Rounding::same(9.0), blob.color);
                            let selected = harmonizer.selected_blob == Some(i);
                            painter.rect_stroke(blob_rect, Rounding::same(9.0), Stroke::new(if selected { 3.0_f32 } else { 1.5_f32 }, if selected { crate::ui::theme::palette().yellow() } else { crate::ui::theme::palette().text() }));
                            if blob.pitch_offset.abs() > 0.001 {
                                painter.text(blob_rect.center(), egui::Align2::CENTER_CENTER, format!("{:+}", blob.pitch_offset.round() as i32), egui::FontId::proportional(Theme::FONT_S), Color32::BLACK);
                            }
                        }

                        // Click a note to select it; drag vertically to retune it.
                        if m_resp.clicked() || m_resp.dragged() {
                            if let Some(pos) = m_resp.interact_pointer_pos() {
                                let step = ((pos.x - (m_rect.min.x + 5.0)) / step_width).floor();
                                let mut hit: Option<usize> = None;
                                for (i, b) in harmonizer.blobs.iter().enumerate() {
                                    if step >= b.start_step as f32 && step < (b.start_step + b.length_steps) as f32 {
                                        hit = Some(i);
                                        break;
                                    }
                                }
                                harmonizer.selected_blob = hit;
                                if let Some(i) = hit {
                                    if m_resp.dragged() {
                                        let delta = -m_resp.drag_delta().y / (row_h * 0.75);
                                        let b = &mut harmonizer.blobs[i];
                                        b.pitch_offset = (b.pitch_offset + delta).clamp(-12.0, 12.0);
                                    }
                                }
                            }
                        }

                        if is_playing {
                            let px = m_rect.min.x + 5.0 + current_step as f32 * step_width;
                            painter.line_segment([Pos2::new(px, m_rect.min.y), Pos2::new(px, m_rect.max.y)], Stroke::new(2.0_f32, crate::ui::theme::palette().text()));
                        }

                        // Selected-note editor.
                        if let Some(i) = harmonizer.selected_blob {
                            if let Some(b) = harmonizer.blobs.get_mut(i) {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(crate::tstatus!("Not {} – MIDI {}", i + 1, b.midi_note)).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                                    ui.add(egui::Slider::new(&mut b.pitch_offset, -12.0..=12.0).text(crate::i18n::t("korrigering (st)")).step_by(0.5));
                                    if ui.button(crate::i18n::t("0")).clicked() {
                                        b.pitch_offset = 0.0;
                                    }
                                });
                            }
                        } else if !harmonizer.blobs.is_empty() {
                            ui.label(egui::RichText::new(crate::i18n::t("Klicka på ett notblock och dra lodrätt för att justera tonhöjden.")).size(Theme::FONT_XS).color(crate::ui::theme::palette().text_muted()));
                        }
                    });

                    cols[1].group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("✨ 4-VOICE HARMONIZER (KÖR)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                        });
                        ui.add_space(4.0);

                        if ui.add_enabled(!vocal_track.takes.is_empty(), egui::Button::new(egui::RichText::new(crate::i18n::t("➕ Skapa körstämmor som tagningar")).size(Theme::FONT_S)).fill(crate::ui::theme::palette().green())).on_hover_text(crate::i18n::t("Pitch-shiftar vald tagning per aktiverad stämma och lägger till dem som egna tagningar")).clicked() {
                            let (pcm, sr) = {
                                let take = &vocal_track.takes[act_idx];
                                (take.pcm_samples.clone(), take.sample_rate)
                            };
                            let rendered = harmonizer.render_harmony(&pcm, sr as f32);
                            let count = rendered.len();
                            for (i, (name, voice_pcm)) in rendered.into_iter().enumerate() {
                                let color = Color32::from_rgb(80, 220 - (i as u8 * 30), 160);
                                vocal_track.add_take_from_pcm(name, voice_pcm, sr, color);
                            }
                            *status_msg = crate::tstatus!("➕ Skapade {} körstämmor från tagningen", count);
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(crate::i18n::t("Bärvåg:"))
                                    .size(Theme::FONT_S)
                                    .color(crate::ui::theme::palette().text_muted()),
                            );
                            let mut note = harmonizer.vocoder_note as i32;
                            if ui
                                .add(egui::DragValue::new(&mut note).range(24..=84).speed(0.2))
                                .on_hover_text(crate::i18n::t(
                                    "Noten vocoderns bärvåg ligger på. A2 (45) är standard.",
                                ))
                                .changed()
                            {
                                harmonizer.vocoder_note = note.clamp(24, 84) as u8;
                            }
                            ui.add(
                                egui::Checkbox::new(
                                    &mut harmonizer.vocoder_follow_pitch,
                                    crate::i18n::t("Följ sången"),
                                ),
                            )
                            .on_hover_text(crate::i18n::t(
                                "Bärvågen följer sångens tonhöjd i stället för att ligga på den valda noten.",
                            ));
                            let freq =
                                crate::audio::tuning::midi_to_freq(harmonizer.vocoder_note);
                            let name = crate::ui::app::state::note_name_of(harmonizer.vocoder_note);
                            let octave = harmonizer.vocoder_note as i32 / 12 - 1;
                            ui.label(
                                egui::RichText::new(format!("{name}{octave}  ({freq:.1} Hz)"))
                                    .size(Theme::FONT_S)
                                    .color(crate::ui::theme::palette().cyan()),
                            );
                        });
                        if ui
                            .add_enabled(
                                !vocal_track.takes.is_empty(),
                                egui::Button::new(
                                    egui::RichText::new(crate::i18n::t("🤖 Vocoder (A2-bärvåg)"))
                                        .size(Theme::FONT_S)
                                        // Cyan knapp kräver mörk text, annars blir den
                                        // oläsbar när den är avstängd (grå på cyan).
                                        .color(egui::Color32::BLACK),
                                )
                                .fill(crate::ui::theme::palette().cyan()),
                            )
                            .on_hover_text(crate::i18n::t(
                                "Lägger sångens form på en sågtandsvåg: en robot som sjunger orden. Bärvågen är A2 med oktav och kvint.",
                            ))
                            .clicked()
                        {
                            let (pcm, sr, take_name) = {
                                let take = &vocal_track.takes[act_idx];
                                (take.pcm_samples.clone(), take.sample_rate, take.name.clone())
                            };
                            let sr_f = sr as f32;
                            // Bärvågen är noten med oktav och kvint — en sågtand vardera, så
                            // vocodern har övertoner i varje band den delar upp.
                            let carrier = if harmonizer.vocoder_follow_pitch {
                                crate::audio::vocal_harmonizer::pitch_following_carrier(&pcm, sr_f)
                            } else {
                                let root = crate::audio::tuning::midi_to_freq(harmonizer.vocoder_note);
                                let mut carrier = Vec::with_capacity(pcm.len());
                                for i in 0..pcm.len() {
                                    let t = i as f32 / sr_f;
                                    let saw = |f: f32| 2.0 * ((t * f) % 1.0) - 1.0;
                                    carrier
                                        .push(saw(root) + 0.5 * saw(root * 2.0) + 0.35 * saw(root * 1.5));
                                }
                                carrier
                            };
                            let vocoded =
                                crate::audio::vocal_harmonizer::vocode(&pcm, &carrier, sr_f, 16);
                            vocal_track.add_take_from_pcm(
                                format!("{take_name} (Vocoder)"),
                                vocoded,
                                sr,
                                crate::ui::theme::palette().cyan(),
                            );
                            *status_msg =
                                crate::tstatus!("🤖 Vocoder: la till en tagning ur {}", take_name);
                        }
                        ui.add_space(4.0);

                        for voice in &mut harmonizer.voices {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut voice.enabled, voice.name);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    rotary_knob(ui, &mut voice.volume, 0.0, 1.25, "VOL", crate::ui::theme::palette().green(), 18.0);
                                    rotary_knob(ui, &mut voice.formant_shift, -12.0, 12.0, "FORMANT", crate::ui::theme::palette().purple(), 18.0);
                                });
                            });
                            ui.add_space(2.0);
                        }
                    });
                });
            }

            RecordingMode::CustomSounds => {
                // ============================================================
                // CUSTOM SOUND & SAMPLE RECORDER
                // ============================================================
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("🎤 SPELA IN EGET LJUD / SAMPLING")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                        ui.separator();
                        ui.label(egui::RichText::new(crate::i18n::t("Spela in instrument, klappar, rösteffekter eller miljöljud direkt via mik")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                    });

                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Namn på ljudet:")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                        ui.add(egui::TextEdit::singleline(&mut vocal_track.custom_sample_name_input).desired_width(180.0));

                        ui.separator();

                        if vocal_track.is_recording_custom {
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(" ⏹ STOPPA & SPARA KLIPP ")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().green())).clicked() {
                                match vocal_track.stop_custom_recording() {
                                    Ok(idx) => {
                                        let name = vocal_track.custom_sounds[idx].name.clone();
                                        *status_msg = crate::tstatus!("✔ Sparade eget ljud '{}' i samplingsbiblioteket!", name);
                                    }
                                    Err(e) => *status_msg = format!("❌ {}", e),
                                }
                            }
                            ui.label(egui::RichText::new(format!("🔴 {:.1}s", vocal_track.custom_recording_elapsed_secs)).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().red()));
                        } else if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(" 🔴 SPELA IN LJUDKLIPP ")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().red())).clicked() {
                            match vocal_track.start_custom_recording() {
                                Ok(()) => *status_msg = crate::i18n::t("🔴 Spelar in eget ljud...").to_string(),
                                Err(e) => *status_msg = format!("❌ {}", e),
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                // List of custom sound clips
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("📁 MINA INSPELADE LJUD & SAMPLINGAR")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                    ui.add_space(4.0);

                    for sound in &vocal_track.custom_sounds {
                        ui.horizontal(|ui| {
                            let (c_rect, _) = ui.allocate_exact_size(Vec2::new(160.0, 32.0), Sense::hover());
                            ui.painter().rect_filled(c_rect, Rounding::same(3.0), crate::ui::theme::palette().panel());
                            ui.painter().rect_stroke(c_rect, Rounding::same(3.0), Stroke::new(1.0_f32, sound.color));
                            ui.painter().text(
                                Pos2::new(c_rect.min.x + 8.0, c_rect.min.y + 8.0),
                                egui::Align2::LEFT_TOP,
                                &sound.name,
                                egui::FontId::proportional(Theme::FONT_S),
                                sound.color,
                            );

                            // Waveform mini preview
                            let (w_rect, _) = ui.allocate_exact_size(Vec2::new(180.0, 32.0), Sense::hover());
                            ui.painter().rect_filled(w_rect, Rounding::same(2.0), crate::ui::theme::palette().bg());
                            let num_s = sound.waveform_data.len();
                            let sw = w_rect.width() / num_s as f32;
                            let my = w_rect.center().y;
                            for (si, &amp) in sound.waveform_data.iter().enumerate() {
                                let sx = w_rect.min.x + si as f32 * sw;
                                let h = amp * 12.0;
                                ui.painter().line_segment([Pos2::new(sx, my - h), Pos2::new(sx, my + h)], Stroke::new(1.0_f32, sound.color));
                            }

                            ui.label(egui::RichText::new(format!("{:.1}s | {}", sound.duration_secs, sound.category)).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));

                            if ui.button(crate::i18n::t("▶ Provlyssna")).clicked() {
                                let pcm_arc = std::sync::Arc::new(sound.pcm_samples.clone());
                                let _ = engine.send_command(crate::audio::AudioCommand::PlayAudition {
                                    left: pcm_arc.clone(),
                                    right: pcm_arc,
                                    sample_rate: sound.sample_rate as f32,
                                    volume: 1.0,
                                    pitch_ratio: 1.0,
                                    time_stretch_ratio: 1.0,
                                    is_reverse: false,
                                    loop_playback: false,
                                });
                                *status_msg = crate::tstatus!("▶ Spelar upp: {}", sound.name);
                            }
                        });
                        ui.add_space(3.0);
                    }
                });
            }
        }
    });
}

