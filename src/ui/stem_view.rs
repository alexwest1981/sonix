use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use crate::audio::stem_separator::StemProject;
use crate::ui::theme::Theme;
use crate::ui::widgets::rotary_knob;

#[derive(Default)]
pub struct StemViewActions {
    pub request_separation: bool,
    pub export_stems: bool,
}

pub fn render_stem_separator_view(
    ui: &mut Ui,
    project: &mut StemProject,
    current_time: f32,
    is_playing: bool,
    status_msg: &mut String,
) -> StemViewActions {
    let mut actions = StemViewActions::default();
    let _ = &status_msg;
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🧠 AI STEM SEPARATION STUDIO (Spectral DSP Engine)")).strong().size(14.0).color(Theme::FL_ORANGE));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Isolera och extrahera sång, trummor, bas och instrument direkt ur färdiga mixar")).size(11.0).color(Theme::TEXT_MUTED));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📥 Exportera Stems till Song Arranger")).strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                    actions.export_stems = true;
                }
            });
        });

        ui.add_space(8.0);

        // Project Info Bar
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Källfil:")).size(11.0).color(Theme::TEXT_MUTED));
                ui.label(egui::RichText::new(&project.track_title).strong().size(12.0).color(Theme::FL_CYAN));
                ui.separator();

                let tempo_txt = if project.bpm > 0.0 {
                    format!("{:.0} BPM", project.bpm)
                } else {
                    crate::i18n::t("— BPM").to_string()
                };
                ui.label(egui::RichText::new(format!("{} {:.1}s  •  {} {}", crate::i18n::t("Längd:"), project.duration_seconds, crate::i18n::t("Tempo:"), tempo_txt)).size(11.0).color(Theme::TEXT_MUTED));
                ui.separator();

                ui.label(egui::RichText::new(format!("Modell: {}", project.model_name)).size(10.5).color(Theme::FL_YELLOW));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⚡ Välj fil & separera")).strong().color(Color32::WHITE)).fill(Color32::from_rgb(140, 40, 180))).clicked() {
                        actions.request_separation = true;
                    }
                });
            });
        });

        ui.add_space(8.0);

        if project.stems.is_empty() {
            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Ingen mix separerad ännu. Klicka på \"Välj fil & separera\" och välj en WAV/MP3/FLAC-fil — Sonix delar upp den i fyra spelbara stämspår.")).size(11.5).color(Theme::TEXT_MUTED));
            });
        }

        // 4 Isolated Stem Waveform Channels
        for stem in &mut project.stems {
            let col = stem.stem_type.color();
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Left Stem Header & Mixer Controls
                    ui.group(|ui| {
                        ui.set_width(170.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(stem.stem_type.name()).strong().size(12.0).color(col));
                            ui.add_space(2.0);

                            ui.horizontal(|ui| {
                                let m_col = if stem.muted { Color32::from_rgb(80, 20, 20) } else { Theme::FL_GREEN };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("MUTE")).size(10.0)).fill(m_col)).clicked() {
                                    stem.muted = !stem.muted;
                                }
                                let s_col = if stem.solo { Theme::FL_ORANGE } else { Color32::from_rgb(40, 35, 25) };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("SOLO")).size(10.0)).fill(s_col)).clicked() {
                                    stem.solo = !stem.solo;
                                }
                            });

                            ui.horizontal(|ui| {
                                rotary_knob(ui, &mut stem.volume, 0.0, 1.25, "VOL", col, 22.0);
                                rotary_knob(ui, &mut stem.pan, -1.0, 1.0, "PAN", Color32::WHITE, 22.0);
                                rotary_knob(ui, &mut stem.time_stretch, 0.5, 2.0, "SPEED", Theme::FL_YELLOW, 22.0);
                            });
                        });
                    });

                    // Right Interactive Waveform Track Display
                    let wave_width = (ui.available_width() - 10.0).max(10.0);
                    let (wave_rect, _) = ui.allocate_exact_size(Vec2::new(wave_width, 70.0), Sense::click());
                    let painter = ui.painter();

                    // Track Background
                    painter.rect_filled(wave_rect, Rounding::same(4.0), Color32::from_rgb(18, 22, 30));
                    painter.rect_stroke(wave_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(34, 44, 58)));

                    let mid_y = wave_rect.center().y;
                    let num_bars = stem.waveform_data.len();
                    if num_bars > 0 {
                        let bar_w = (wave_rect.width() - 8.0).max(1.0) / num_bars as f32;

                        for (idx, &amp) in stem.waveform_data.iter().enumerate() {
                            let bx = wave_rect.min.x + 4.0 + idx as f32 * bar_w;
                            let h = amp * (wave_rect.height() * 0.42);
                            let bar_rect = Rect::from_min_max(Pos2::new(bx, mid_y - h), Pos2::new(bx + bar_w * 0.8, mid_y + h));
                            let fill = if stem.muted { Color32::from_rgb(40, 45, 55) } else { col };
                            painter.rect_filled(bar_rect, Rounding::same(1.0), fill);
                        }
                    }

                    // Playhead needle
                    if is_playing {
                        let play_norm = (current_time / project.duration_seconds).fract();
                        let px = wave_rect.min.x + 4.0 + play_norm * (wave_rect.width() - 8.0);
                        painter.line_segment([Pos2::new(px, wave_rect.min.y), Pos2::new(px, wave_rect.max.y)], Stroke::new(2.0_f32, Color32::WHITE));
                    }
                });
            });
            ui.add_space(4.0);
        }
    });
    actions
}
