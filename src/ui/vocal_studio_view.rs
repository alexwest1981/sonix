use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use crate::audio::recorder::{RecordingMode, VocalStudioTrack};
use crate::audio::vocal_harmonizer::VocalHarmonizer;
use crate::ui::theme::Theme;
use crate::ui::widgets::rotary_knob;

pub fn render_vocal_studio_view(
    ui: &mut Ui,
    vocal_track: &mut VocalStudioTrack,
    harmonizer: &mut VocalHarmonizer,
    is_playing: bool,
    current_step: usize,
    status_msg: &mut String,
) {
    ui.group(|ui| {
        // Top Header
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🎙 INSPELNINGSSTUDIO: SÅNG & EGNA LJUD").strong().size(14.0).color(Theme::FL_ORANGE));
            ui.separator();
            ui.label(egui::RichText::new("Spela in sång, mikrofonljud, akustiska instrument och egna samplingar").size(11.0).color(Theme::TEXT_MUTED));
        });

        ui.add_space(6.0);

        // Sub-tabs for the Studio
        ui.horizontal(|ui| {
            let voc_tab = ui.selectable_label(vocal_track.recording_mode == RecordingMode::LeadVocals, "🎙 1. Sång & Mikrofon (Take Lanes)");
            if voc_tab.clicked() { vocal_track.recording_mode = RecordingMode::LeadVocals; }

            let custom_tab = ui.selectable_label(vocal_track.recording_mode == RecordingMode::CustomSounds, "🎤 2. Spela in Egna Ljud & Sampler");
            if custom_tab.clicked() { vocal_track.recording_mode = RecordingMode::CustomSounds; }
        });

        ui.separator();
        ui.add_space(4.0);

        match vocal_track.recording_mode {
            RecordingMode::LeadVocals => {
                // ============================================================
                // VOCAL RECORDING CONSOLE
                // ============================================================
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🔴 INSPELNINGSKONSOL").strong().size(12.0).color(Theme::FL_CYAN));
                        ui.separator();

                        ui.label(egui::RichText::new(format!("Källa: {}", vocal_track.input_channel)).size(11.0).color(Theme::TEXT_MUTED));
                        ui.checkbox(&mut vocal_track.monitoring_on, "🎧 Monitoring");
                        ui.checkbox(&mut vocal_track.is_armed, "🔴 Armerad");

                        ui.separator();
                        rotary_knob(ui, &mut vocal_track.input_gain, 0.0, 2.0, "GAIN", Theme::FL_YELLOW, 18.0);
                        rotary_knob(ui, &mut harmonizer.autotune_speed, 0.0, 1.0, "AUTO-TUNE", Theme::FL_ORANGE, 18.0);

                        ui.separator();

                        // VU METER
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("MIC IN").size(9.0).color(Theme::TEXT_MUTED));
                            let (vu_rect, _) = ui.allocate_exact_size(Vec2::new(70.0, 14.0), Sense::hover());
                            ui.painter().rect_filled(vu_rect, Rounding::same(2.0), Color32::from_rgb(18, 22, 28));
                            let vu_fill_w = if vocal_track.is_recording {
                                (vu_rect.width() * vocal_track.mic_vu_level).clamp(4.0, vu_rect.width())
                            } else {
                                vu_rect.width() * 0.2
                            };
                            let vu_col = if vocal_track.mic_vu_level > 0.85 { Theme::FL_ORANGE } else { Theme::FL_GREEN };
                            ui.painter().rect_filled(Rect::from_min_size(vu_rect.min, Vec2::new(vu_fill_w, vu_rect.height())), Rounding::same(2.0), vu_col);
                        });

                        ui.add_space(8.0);

                        // BIG RECORD BUTTON
                        if !vocal_track.is_recording {
                            if ui.add(egui::Button::new(egui::RichText::new(" 🔴 STARTA INSPELNING ").strong().size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(200, 30, 30))).clicked() {
                                vocal_track.is_recording = true;
                                vocal_track.recording_elapsed_secs = 0.0;
                                *status_msg = "🔴 Spelar in sång... Sjung eller prata i mikrofonen!".to_string();
                            }
                        } else {
                            if ui.add(egui::Button::new(egui::RichText::new(" ⏹ STOPPA & SPARA TAGNING ").strong().size(12.0).color(Color32::WHITE)).fill(Color32::from_rgb(40, 160, 60))).clicked() {
                                vocal_track.is_recording = false;
                                let name = format!("Tagning {}", vocal_track.takes.len() + 1);
                                vocal_track.add_new_take(&name, 0, 8);
                                *status_msg = format!("✔ Sparade {}! Ny sångtagning har lagts till i Take Lanes.", name);
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                // Take Lanes Section
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("📁 SÅNGTAGNINGAR (TAKE LANES)").strong().size(12.0).color(Theme::FL_CYAN));
                        ui.separator();
                        ui.label(egui::RichText::new("Välj den bästa tagningen eller kombinera delar").size(10.5).color(Theme::TEXT_MUTED));

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("➕ Spela in snabb-demo tagning").clicked() {
                                let name = format!("Tagning {}", vocal_track.takes.len() + 1);
                                vocal_track.add_new_take(&name, 0, 8);
                                *status_msg = format!("✔ Skapade {}", name);
                            }
                        });
                    });

                    ui.add_space(4.0);

                    for (t_idx, take) in vocal_track.takes.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let is_active = take.is_selected;
                            let btn_bg = if is_active { take.color } else { Theme::PANEL_BG };
                            let btn_text_col = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };

                            if ui.add(egui::Button::new(egui::RichText::new(&take.name).strong().size(11.0).color(btn_text_col)).fill(btn_bg).min_size(Vec2::new(170.0, 32.0))).clicked() {
                                take.is_selected = true;
                                vocal_track.active_comp_take = t_idx;
                                *status_msg = format!("Valde {} för aktiv comping", take.name);
                            }

                            // Waveform display
                            let (w_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width() - 8.0, 32.0), Sense::click());
                            let painter = ui.painter();

                            painter.rect_filled(w_rect, Rounding::same(3.0), Color32::from_rgb(16, 20, 26));
                            painter.rect_stroke(w_rect, Rounding::same(3.0), Stroke::new(1.0_f32, if is_active { take.color } else { Color32::from_rgb(30, 40, 52) }));

                            let num_samples = take.waveform_data.len();
                            let step_w = (w_rect.width() - 6.0) / num_samples as f32;
                            let mid_y = w_rect.center().y;

                            for (s_i, &amp) in take.waveform_data.iter().enumerate() {
                                let sx = w_rect.min.x + 3.0 + s_i as f32 * step_w;
                                let h = amp * (w_rect.height() * 0.45);
                                let bar_rect = Rect::from_min_max(Pos2::new(sx, mid_y - h), Pos2::new(sx + step_w * 0.85, mid_y + h));
                                painter.rect_filled(bar_rect, Rounding::same(1.0), if is_active { take.color } else { Color32::from_rgb(60, 75, 95) });
                            }
                        });
                        ui.add_space(2.0);
                    }
                });

                ui.add_space(8.0);

                // Melodyne & Harmonizer
                ui.columns(2, |cols| {
                    cols[0].group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("🎼 MELODYNE ARA2 TONKORRIGERING").strong().size(11.0).color(Theme::FL_YELLOW));
                        });
                        ui.add_space(4.0);

                        let (m_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 130.0), Sense::click());
                        let painter = ui.painter();
                        painter.rect_filled(m_rect, Rounding::same(4.0), Color32::from_rgb(14, 18, 24));
                        painter.rect_stroke(m_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(34, 48, 64)));

                        let note_rows = 6;
                        let row_h = m_rect.height() / note_rows as f32;
                        for r in 0..note_rows {
                            let ry = m_rect.min.y + r as f32 * row_h;
                            painter.line_segment([Pos2::new(m_rect.min.x, ry), Pos2::new(m_rect.max.x, ry)], Stroke::new(1.0_f32, Color32::from_rgb(22, 28, 38)));
                        }

                        let step_width = (m_rect.width() - 10.0) / 16.0;
                        for blob in &harmonizer.blobs {
                            let bx = m_rect.min.x + 5.0 + blob.start_step as f32 * step_width;
                            let bw = blob.length_steps as f32 * step_width - 3.0;
                            let by = m_rect.max.y - ((blob.midi_note - 58) as f32 * (row_h * 0.75)).clamp(15.0, m_rect.height() - 25.0);
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
                        ui.label(egui::RichText::new("Spela in instrument, klappar, rösteffekter eller miljöljud").size(11.0).color(Theme::TEXT_MUTED));
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

