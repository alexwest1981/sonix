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
) {
    ui.group(|ui| {
        // ============================================================
        // 1. TOP HEADER & AUDIO ROUTING STATUS
        // ============================================================
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🎙 SONIX STUDIO RECORDER").strong().size(15.0).color(Theme::FL_ORANGE));
            ui.separator();
            ui.label(egui::RichText::new("Mikrofoninspelning i realtid  •  Live WAV Capture  •  Waveform Editor  •  Melodyne ARA2").size(11.0).color(Theme::TEXT_MUTED));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(egui::RichText::new("⚙ Mik-panel").size(11.0).strong().color(Color32::BLACK)).fill(Theme::FL_CYAN)).clicked() {
                    *show_mic_settings = true;
                }
                let dev_name = vocal_track.mic_capture.as_ref().map(|m| m.device_name.as_str()).unwrap_or("Standard Mikrofon");
                let sr = vocal_track.mic_capture.as_ref().map(|m| m.sample_rate).unwrap_or(44100);
                ui.label(egui::RichText::new(format!("🎛 {} | {} Hz", dev_name, sr)).size(10.5).color(Theme::FL_CYAN));
            });
        });

        ui.add_space(4.0);

        // Sub-tabs for the Studio
        ui.horizontal(|ui| {
            let voc_tab = ui.selectable_label(vocal_track.recording_mode == RecordingMode::LeadVocals, "🎙 1. Sång & Mikrofoninspelning (Take Lanes & Studio Deck)");
            if voc_tab.clicked() { vocal_track.recording_mode = RecordingMode::LeadVocals; }

            let custom_tab = ui.selectable_label(vocal_track.recording_mode == RecordingMode::CustomSounds, "🎤 2. Spela in Egna Ljud & Sampler");
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
                        ui.label(egui::RichText::new("🔴 INSPELNINGSDECK").strong().size(12.0).color(Theme::FL_CYAN));
                        ui.separator();

                        ui.checkbox(&mut vocal_track.is_armed, "🔴 Armera");
                        ui.checkbox(&mut vocal_track.monitoring_on, "🎧 Direct Monitoring");

                        ui.separator();
                        rotary_knob(ui, &mut vocal_track.input_gain, 0.0, 2.5, "GAIN", Theme::FL_YELLOW, 18.0);
                        rotary_knob(ui, &mut harmonizer.autotune_speed, 0.0, 1.0, "AUTO-TUNE", Theme::FL_ORANGE, 18.0);

                        ui.separator();

                        // VU METER WITH PEAK dB DISPLAY
                        ui.vertical(|ui| {
                            let vu_db = if vocal_track.mic_vu_level > 0.0001 {
                                (20.0 * vocal_track.mic_vu_level.log10()).max(-48.0)
                            } else {
                                -48.0
                            };
                            ui.label(egui::RichText::new(format!("MIC IN: {:.1} dB", vu_db)).size(9.0).color(Theme::TEXT_MUTED));

                            let (vu_rect, _) = ui.allocate_exact_size(Vec2::new(100.0, 14.0), Sense::hover());
                            ui.painter().rect_filled(vu_rect, Rounding::same(2.0), Color32::from_rgb(14, 18, 24));
                            ui.painter().rect_stroke(vu_rect, Rounding::same(2.0), Stroke::new(1.0_f32, Color32::from_rgb(32, 42, 54)));

                            let vu_fill_w = (vu_rect.width() * vocal_track.mic_vu_level.clamp(0.0, 1.0)).max(2.0);
                            let vu_col = if vocal_track.mic_vu_level > 0.90 {
                                Color32::from_rgb(255, 60, 60) // Red clip
                            } else if vocal_track.mic_vu_level > 0.70 {
                                Theme::FL_ORANGE // Yellow/Orange warm
                            } else {
                                Theme::FL_GREEN // Green safe
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
                                if vocal_track.is_paused { Theme::FL_YELLOW } else { Color32::from_rgb(255, 80, 80) }
                            } else {
                                Theme::TEXT_MUTED
                            };

                            let state_icon = if vocal_track.is_recording {
                                if vocal_track.is_paused { "⏸ PAUSAD" } else { "🔴 REC" }
                            } else {
                                "⏹ STANDBY"
                            };

                            ui.label(egui::RichText::new(format!("{:02}:{:02}.{:02} (Takt {:.1})", mins, secs, centis, current_bars)).strong().size(12.0).color(timer_color));
                            ui.label(egui::RichText::new(state_icon).size(9.5).color(timer_color));
                        });

                        ui.add_space(8.0);

                        // ==========================================
                        // TRANSPORT CONTROLS: REC, PAUSE, RESUME, STOP
                        // ==========================================
                        if !vocal_track.is_recording {
                            if ui.add(
                                egui::Button::new(egui::RichText::new(" 🔴 SPELA IN (REC) ").strong().size(12.0).color(Color32::WHITE))
                                    .fill(Color32::from_rgb(210, 30, 30))
                                    .min_size(Vec2::new(130.0, 32.0)),
                            ).clicked() {
                                vocal_track.start_recording();
                                *status_msg = "🔴 Spelar in mikrofonljud i realtid... Sjung eller spela nu!".to_string();
                            }
                        } else {
                            if vocal_track.is_paused {
                                if ui.add(
                                    egui::Button::new(egui::RichText::new(" ▶ ÅTERUPPTA ").strong().size(11.0).color(Color32::BLACK))
                                        .fill(Theme::FL_YELLOW)
                                        .min_size(Vec2::new(90.0, 32.0)),
                                ).clicked() {
                                    vocal_track.resume_recording();
                                    *status_msg = "▶ Återupptog inspelningen!".to_string();
                                }
                            } else {
                                if ui.add(
                                    egui::Button::new(egui::RichText::new(" ⏸ PAUSA ").strong().size(11.0).color(Color32::BLACK))
                                        .fill(Theme::FL_YELLOW)
                                        .min_size(Vec2::new(80.0, 32.0)),
                                ).clicked() {
                                    vocal_track.pause_recording();
                                    *status_msg = "⏸ Inspelning pausad.".to_string();
                                }
                            }

                            ui.add_space(4.0);

                            if ui.add(
                                egui::Button::new(egui::RichText::new(" ⏹ STOPPA & SKAPA TAGNING ").strong().size(11.0).color(Color32::WHITE))
                                    .fill(Color32::from_rgb(30, 160, 70))
                                    .min_size(Vec2::new(140.0, 32.0)),
                            ).clicked() {
                                if let Some(idx) = vocal_track.stop_recording(bpm) {
                                    let name = vocal_track.takes[idx].name.clone();
                                    *status_msg = format!("✔ Sparade tagning: {}! Klar för redigering och tidslinje.", name);
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
                        painter.rect_filled(live_rect, Rounding::same(4.0), Color32::from_rgb(12, 16, 22));
                        painter.rect_stroke(live_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(220, 50, 50)));

                        let mid_y = live_rect.center().y;
                        painter.line_segment([Pos2::new(live_rect.min.x, mid_y), Pos2::new(live_rect.max.x, mid_y)], Stroke::new(1.0_f32, Color32::from_rgb(30, 45, 60)));

                        let peaks = &vocal_track.live_recording_peaks;
                        if !peaks.is_empty() {
                            let step_x = (live_rect.width() / peaks.len().max(1) as f32).max(1.5);
                            for (i, &p) in peaks.iter().enumerate() {
                                let x = live_rect.min.x + i as f32 * step_x;
                                if x > live_rect.max.x { break; }
                                let h = p * (live_rect.height() * 0.45);
                                let col = if vocal_track.is_paused {
                                    Theme::FL_YELLOW
                                } else if p > 0.85 {
                                    Color32::from_rgb(255, 100, 100)
                                } else {
                                    Color32::from_rgb(0, 220, 255)
                                };
                                painter.line_segment([Pos2::new(x, mid_y - h), Pos2::new(x, mid_y + h)], Stroke::new(1.5_f32, col));
                            }
                        }

                        let status_text = if vocal_track.is_paused { "⏸ INSPELNING PAUSAD" } else { "🔴 SPELAR IN LIVE FRÅN MIKROFON..." };
                        painter.text(
                            Pos2::new(live_rect.min.x + 8.0, live_rect.min.y + 6.0),
                            egui::Align2::LEFT_TOP,
                            status_text,
                            egui::FontId::proportional(10.0),
                            if vocal_track.is_paused { Theme::FL_YELLOW } else { Color32::from_rgb(255, 120, 120) },
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
                    ui.group(|ui| {
                        let take = &vocal_track.takes[selected_idx];
                        let take_name = take.name.clone();
                        let duration = take.duration_secs;
                        let sample_count = take.pcm_samples.len();
                        let take_color = take.color;

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("✂ LJUDVÅGSEDITOR & BESKÄRNING (STUDIO TRIMMER)").strong().size(12.0).color(Theme::FL_YELLOW));
                            ui.separator();
                            ui.label(egui::RichText::new(format!("Aktiv tagning: {} ({:.2}s, {} samples)", take_name, duration, sample_count)).size(11.0).color(Theme::TEXT_BRIGHT));

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                // INSERT INTO PLAYLIST BUTTON
                                if ui.add(
                                    egui::Button::new(egui::RichText::new("📥 Infoga i Tidslinje (Arranger)").strong().size(11.0).color(Color32::BLACK))
                                        .fill(Theme::FL_GREEN),
                                ).clicked() {
                                    if !playlist_tracks.is_empty() {
                                        let region_id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as usize;
                                        let length_bars = ((duration / ((60.0 / bpm.max(40.0)) * 4.0)).ceil()).max(1.0);
                                        let region = AudioRegion {
                                            id: region_id,
                                            name: format!("🎙 {}", take_name),
                                            start_bar: 0.0,
                                            length_bars,
                                            sample_offset_sec: 0.0,
                                            source_path: None,
                                            waveform_peaks: take.waveform_data.clone(),
                                            volume: 1.0,
                                            fade_in_bars: 0.0,
                                            fade_out_bars: 0.0,
                                            muted: false,
                                            color: take_color,
                                        };
                                        playlist_tracks[0].regions.push(region);
                                        *status_msg = format!("✔ Infogade '{}' som ljudregion i Tidslinjen (Spår 1)!", take_name);
                                    }
                                }

                                // EXPORT TO WAV BUTTON
                                if ui.add(
                                    egui::Button::new(egui::RichText::new("💾 Exportera WAV").strong().size(11.0).color(Color32::WHITE))
                                        .fill(Color32::from_rgb(50, 110, 180)),
                                ).clicked() {
                                    let filename = format!("recordings/take_{}.wav", selected_idx + 1);
                                    match vocal_track.export_take_to_wav(selected_idx, &filename) {
                                        Ok(msg) => *status_msg = msg,
                                        Err(err) => *status_msg = format!("❌ Fel vid export: {}", err),
                                    }
                                }
                            });
                        });

                        ui.add_space(4.0);

                        // WAVEFORM CANVAS WITH SELECTION BOUNDARIES
                        let canvas_w = ui.available_width().max(200.0);
                        let (canvas_rect, _) = ui.allocate_exact_size(Vec2::new(canvas_w, 90.0), Sense::click_and_drag());
                        let painter = ui.painter();

                        // Canvas background & grid
                        painter.rect_filled(canvas_rect, Rounding::same(4.0), Color32::from_rgb(14, 18, 24));
                        painter.rect_stroke(canvas_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(32, 44, 58)));

                        let mid_y = canvas_rect.center().y;
                        painter.line_segment([Pos2::new(canvas_rect.min.x, mid_y), Pos2::new(canvas_rect.max.x, mid_y)], Stroke::new(1.0_f32, Color32::from_rgb(26, 36, 48)));

                        let crop_s = vocal_track.selected_crop_start_norm.clamp(0.0, 0.99);
                        let crop_e = vocal_track.selected_crop_end_norm.clamp(crop_s + 0.01, 1.0);

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

                        // Draw Waveform bars
                        let wave = &take.waveform_data;
                        if !wave.is_empty() {
                            let step_x = canvas_rect.width() / wave.len() as f32;
                            for (i, &amp) in wave.iter().enumerate() {
                                let x = canvas_rect.min.x + i as f32 * step_x;
                                let norm_pos = i as f32 / wave.len() as f32;
                                let in_sel = norm_pos >= crop_s && norm_pos <= crop_e;

                                let h = amp * (canvas_rect.height() * 0.44);
                                let col = if in_sel { take_color } else { Color32::from_rgb(60, 75, 95) };
                                painter.rect_filled(
                                    Rect::from_min_max(Pos2::new(x, mid_y - h), Pos2::new((x + step_x * 0.85).max(x + 1.0), mid_y + h)),
                                    Rounding::same(1.0),
                                    col,
                                );
                            }
                        }

                        // Left & Right boundary markers
                        painter.line_segment([Pos2::new(sel_start_x, canvas_rect.min.y), Pos2::new(sel_start_x, canvas_rect.max.y)], Stroke::new(2.0_f32, Theme::FL_YELLOW));
                        painter.line_segment([Pos2::new(sel_end_x, canvas_rect.min.y), Pos2::new(sel_end_x, canvas_rect.max.y)], Stroke::new(2.0_f32, Theme::FL_YELLOW));

                        // Start & End handle flags
                        painter.rect_filled(Rect::from_min_size(Pos2::new(sel_start_x - 3.0, canvas_rect.min.y), Vec2::new(6.0, 12.0)), Rounding::same(2.0), Theme::FL_YELLOW);
                        painter.rect_filled(Rect::from_min_size(Pos2::new(sel_end_x - 3.0, canvas_rect.max.y - 12.0), Vec2::new(6.0, 12.0)), Rounding::same(2.0), Theme::FL_YELLOW);

                        ui.add_space(4.0);

                        // DUAL TRIM SLIDERS & EDIT ACTIONS
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Start (In):").size(10.5).color(Theme::TEXT_MUTED));
                            let s_val = vocal_track.selected_crop_start_norm * duration;
                            ui.label(egui::RichText::new(format!("{:.2}s", s_val)).strong().size(10.5).color(Theme::FL_YELLOW));
                            ui.add_sized(Vec2::new(120.0, 18.0), egui::Slider::new(&mut vocal_track.selected_crop_start_norm, 0.0..=0.98).show_value(false));

                            ui.separator();

                            ui.label(egui::RichText::new("Slut (Ut):").size(10.5).color(Theme::TEXT_MUTED));
                            let e_val = vocal_track.selected_crop_end_norm * duration;
                            ui.label(egui::RichText::new(format!("{:.2}s", e_val)).strong().size(10.5).color(Theme::FL_YELLOW));
                            ui.add_sized(Vec2::new(120.0, 18.0), egui::Slider::new(&mut vocal_track.selected_crop_end_norm, 0.02..=1.0).show_value(false));

                            ui.separator();

                            // CROP BUTTON
                            if ui.add(egui::Button::new(egui::RichText::new("✂ Beskär till Markering").strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_YELLOW)).clicked() {
                                match vocal_track.crop_selected_take() {
                                    Ok(msg) => *status_msg = msg,
                                    Err(err) => *status_msg = format!("❌ {}", err),
                                }
                            }

                            // SLICE BUTTON
                            if ui.add(egui::Button::new(egui::RichText::new("✂ Dela Tagning").size(11.0).color(Theme::TEXT_BRIGHT))).clicked() {
                                let split_point = vocal_track.selected_crop_start_norm.max(0.2);
                                match vocal_track.slice_selected_take_at(split_point) {
                                    Ok(msg) => *status_msg = msg,
                                    Err(err) => *status_msg = format!("❌ {}", err),
                                }
                            }

                            // NORMALIZE BUTTON
                            if ui.add(egui::Button::new(egui::RichText::new("🎚 Normalisera").size(11.0).color(Theme::TEXT_BRIGHT))).clicked() {
                                match vocal_track.normalize_selected_take() {
                                    Ok(msg) => *status_msg = msg,
                                    Err(err) => *status_msg = format!("❌ {}", err),
                                }
                            }

                            // DELETE BUTTON
                            if ui.add(egui::Button::new(egui::RichText::new("🗑 Ta bort").size(11.0).color(Color32::from_rgb(240, 90, 90)))).clicked() {
                                match vocal_track.delete_selected_take() {
                                    Ok(msg) => *status_msg = msg,
                                    Err(err) => *status_msg = format!("❌ {}", err),
                                }
                            }
                        });
                    });

                    ui.add_space(6.0);
                }

                // ============================================================
                // 4. SÅNGTAGNINGAR (TAKE LANES & COMPING)
                // ============================================================
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("📁 SÅNGTAGNINGAR & COMPING LANES").strong().size(12.0).color(Theme::FL_CYAN));
                        ui.separator();
                        ui.label(egui::RichText::new(format!("Totalt {} tagningar sparade i projektet", vocal_track.takes.len())).size(10.5).color(Theme::TEXT_MUTED));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("➕ Skapa Demo-Tagning").clicked() {
                                let name = format!("Tagning {}", vocal_track.takes.len() + 1);
                                vocal_track.add_new_take(&name, 0, 8);
                                *status_msg = format!("✔ Skapade {}", name);
                            }
                        });
                    });

                    ui.add_space(4.0);

                    for t_idx in 0..vocal_track.takes.len() {
                        let is_active = vocal_track.active_comp_take == t_idx;
                        let take_color = vocal_track.takes[t_idx].color;
                        let take_name = vocal_track.takes[t_idx].name.clone();
                        let take_duration = vocal_track.takes[t_idx].duration_secs;
                        let wave_data = vocal_track.takes[t_idx].waveform_data.clone();

                        ui.horizontal(|ui| {
                            let btn_bg = if is_active { take_color } else { Theme::PANEL_BG };
                            let btn_text_col = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };

                            if ui.add(
                                egui::Button::new(egui::RichText::new(&take_name).strong().size(11.0).color(btn_text_col))
                                    .fill(btn_bg)
                                    .min_size(Vec2::new(180.0, 30.0)),
                            ).clicked() {
                                vocal_track.active_comp_take = t_idx;
                                for (i, t) in vocal_track.takes.iter_mut().enumerate() {
                                    t.is_selected = i == t_idx;
                                }
                                vocal_track.selected_crop_start_norm = 0.0;
                                vocal_track.selected_crop_end_norm = 1.0;
                                *status_msg = format!("Valde {} för aktiv redigering och comping", take_name);
                            }

                            // Waveform display for each lane
                            let w_w = (ui.available_width() - 8.0).max(10.0);
                            let (w_rect, _) = ui.allocate_exact_size(Vec2::new(w_w, 30.0), Sense::click());
                            let painter = ui.painter();

                            painter.rect_filled(w_rect, Rounding::same(3.0), Color32::from_rgb(16, 20, 26));
                            painter.rect_stroke(w_rect, Rounding::same(3.0), Stroke::new(1.0_f32, if is_active { take_color } else { Color32::from_rgb(30, 40, 52) }));

                            let num_samples = wave_data.len();
                            if num_samples > 0 {
                                let step_w = (w_rect.width() - 6.0).max(1.0) / num_samples as f32;
                                let mid_y = w_rect.center().y;

                                for (s_i, &amp) in wave_data.iter().enumerate() {
                                    let sx = w_rect.min.x + 3.0 + s_i as f32 * step_w;
                                    let h = amp * (w_rect.height() * 0.44);
                                    let bar_rect = Rect::from_min_max(Pos2::new(sx, mid_y - h), Pos2::new(sx + step_w * 0.85, mid_y + h));
                                    painter.rect_filled(bar_rect, Rounding::same(1.0), if is_active { take_color } else { Color32::from_rgb(60, 75, 95) });
                                }
                            }

                            // Badge label showing duration
                            painter.text(
                                Pos2::new(w_rect.max.x - 8.0, w_rect.center().y),
                                egui::Align2::RIGHT_CENTER,
                                format!("{:.1}s", take_duration),
                                egui::FontId::proportional(10.0),
                                if is_active { Theme::TEXT_BRIGHT } else { Theme::TEXT_MUTED },
                            );
                        });
                        ui.add_space(2.0);
                    }
                });

                ui.add_space(6.0);

                // ============================================================
                // 5. MELODYNE & HARMONIZER
                // ============================================================
                ui.columns(2, |cols| {
                    cols[0].group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("🎼 MELODYNE ARA2 TONKORRIGERING").strong().size(11.0).color(Theme::FL_YELLOW));
                        });
                        ui.add_space(4.0);

                        let m_w = ui.available_width().max(10.0);
                        let (m_rect, _) = ui.allocate_exact_size(Vec2::new(m_w, 120.0), Sense::click());
                        let painter = ui.painter();
                        painter.rect_filled(m_rect, Rounding::same(4.0), Color32::from_rgb(14, 18, 24));
                        painter.rect_stroke(m_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(34, 48, 64)));

                        let note_rows = 6;
                        let row_h = m_rect.height() / note_rows as f32;
                        for r in 0..note_rows {
                            let ry = m_rect.min.y + r as f32 * row_h;
                            painter.line_segment([Pos2::new(m_rect.min.x, ry), Pos2::new(m_rect.max.x, ry)], Stroke::new(1.0_f32, Color32::from_rgb(22, 28, 38)));
                        }

                        let step_width = (m_rect.width() - 10.0).max(1.0) / 16.0;
                        for blob in &harmonizer.blobs {
                            let bx = m_rect.min.x + 5.0 + blob.start_step as f32 * step_width;
                            let bw = (blob.length_steps as f32 * step_width - 3.0).max(4.0);
                            let by = m_rect.max.y - ((blob.midi_note - 58) as f32 * (row_h * 0.75)).clamp(15.0, (m_rect.height() - 25.0).max(15.0));
                            let blob_rect = Rect::from_min_size(Pos2::new(bx, by), Vec2::new(bw, 18.0));
                            painter.rect_filled(blob_rect, Rounding::same(9.0), blob.color);
                            painter.rect_stroke(blob_rect, Rounding::same(9.0), Stroke::new(1.5_f32, Color32::WHITE));
                            painter.text(blob_rect.center(), egui::Align2::CENTER_CENTER, blob.text_lyric, egui::FontId::proportional(10.0), Color32::BLACK);
                        }

                        if is_playing {
                            let px = m_rect.min.x + 5.0 + current_step as f32 * step_width;
                            painter.line_segment([Pos2::new(px, m_rect.min.y), Pos2::new(px, m_rect.max.y)], Stroke::new(2.0_f32, Color32::WHITE));
                        }
                    });

                    cols[1].group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("✨ 4-VOICE HARMONIZER (KÖR)").strong().size(11.0).color(Theme::FL_GREEN));
                        });
                        ui.add_space(4.0);

                        for voice in &mut harmonizer.voices {
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut voice.enabled, voice.name);
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    rotary_knob(ui, &mut voice.volume, 0.0, 1.25, "VOL", Theme::FL_GREEN, 18.0);
                                    rotary_knob(ui, &mut voice.formant_shift, -12.0, 12.0, "FORMANT", Theme::FL_PURPLE, 18.0);
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
                        ui.label(egui::RichText::new("🎤 SPELA IN EGET LJUD / SAMPLING").strong().size(12.0).color(Theme::FL_ORANGE));
                        ui.separator();
                        ui.label(egui::RichText::new("Spela in instrument, klappar, rösteffekter eller miljöljud direkt via mik").size(11.0).color(Theme::TEXT_MUTED));
                    });

                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Namn på ljudet:").size(11.0).color(Theme::TEXT_MUTED));
                        ui.add(egui::TextEdit::singleline(&mut vocal_track.custom_sample_name_input).desired_width(180.0));

                        ui.separator();

                        if ui.add(egui::Button::new(egui::RichText::new(" 🔴 SPELA IN LJUDKLIPP ").strong().size(11.0).color(Color32::WHITE)).fill(Color32::from_rgb(220, 50, 50))).clicked() {
                            let name = vocal_track.custom_sample_name_input.clone();
                            vocal_track.add_custom_sound(&name, "Eget Ljud");
                            *status_msg = format!("✔ Sparade eget ljud '{}' i samplingsbiblioteket!", name);
                        }

                        if ui.button("➕ Skapa Demo Percussion-Ljud").clicked() {
                            vocal_track.add_custom_sound("Akustisk Shaker", "Perkussion");
                            *status_msg = "✔ Lade till Akustisk Shaker i biblioteket".to_string();
                        }
                    });
                });

                ui.add_space(8.0);

                // List of custom sound clips
                ui.group(|ui| {
                    ui.label(egui::RichText::new("📁 MINA INSPELADE LJUD & SAMPLINGAR").strong().size(12.0).color(Theme::FL_CYAN));
                    ui.add_space(4.0);

                    for sound in &vocal_track.custom_sounds {
                        ui.horizontal(|ui| {
                            let (c_rect, _) = ui.allocate_exact_size(Vec2::new(160.0, 32.0), Sense::hover());
                            ui.painter().rect_filled(c_rect, Rounding::same(3.0), Theme::PANEL_BG);
                            ui.painter().rect_stroke(c_rect, Rounding::same(3.0), Stroke::new(1.0_f32, sound.color));
                            ui.painter().text(
                                Pos2::new(c_rect.min.x + 8.0, c_rect.min.y + 8.0),
                                egui::Align2::LEFT_TOP,
                                &sound.name,
                                egui::FontId::proportional(11.0),
                                sound.color,
                            );

                            // Waveform mini preview
                            let (w_rect, _) = ui.allocate_exact_size(Vec2::new(180.0, 32.0), Sense::hover());
                            ui.painter().rect_filled(w_rect, Rounding::same(2.0), Color32::from_rgb(16, 20, 26));
                            let num_s = sound.waveform_data.len();
                            let sw = w_rect.width() / num_s as f32;
                            let my = w_rect.center().y;
                            for (si, &amp) in sound.waveform_data.iter().enumerate() {
                                let sx = w_rect.min.x + si as f32 * sw;
                                let h = amp * 12.0;
                                ui.painter().line_segment([Pos2::new(sx, my - h), Pos2::new(sx, my + h)], Stroke::new(1.0_f32, sound.color));
                            }

                            ui.label(egui::RichText::new(format!("{:.1}s | {}", sound.duration_secs, sound.category)).size(10.0).color(Theme::TEXT_MUTED));

                            if ui.button("▶ Provlyssna").clicked() {
                                *status_msg = format!("Spelar upp: {}", sound.name);
                            }
                        });
                        ui.add_space(3.0);
                    }
                });
            }
        }
    });
}

