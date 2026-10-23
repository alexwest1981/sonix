use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};
use crate::audio::stem_separator::StemProject;
use crate::ui::theme::Theme;
use crate::ui::widgets::rotary_knob_full;

#[derive(Default)]
pub struct StemViewActions {
    pub request_separation: bool,
    pub export_stems: bool,
    /// Index of a stem whose SPEED (time-stretch) control was changed and must
    /// be re-rendered by the app.
    pub speed_changed: Option<usize>,
}

pub fn render_stem_separator_view(
    ui: &mut Ui,
    project: &mut StemProject,
    current_time: f32,
    is_playing: bool,
    status_msg: &mut String,
    model_download: &std::sync::Arc<std::sync::Mutex<(bool, f32, String)>>,
    stem_six: &mut bool,
) -> StemViewActions {
    // Fyra stämmor är standard; sex lägger till gitarr och piano och kräver 6s-modellen.
    let model = if *stem_six {
        crate::audio::neural_separator::StemModel::Six
    } else {
        crate::audio::neural_separator::StemModel::Four
    };
    let mut actions = StemViewActions::default();
    let _ = &status_msg;
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🧠 AI STEM SEPARATION STUDIO (DSP + Neural ONNX)")).strong().size(Theme::FONT_L).color(Theme::FL_ORANGE));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Isolera och extrahera sång, trummor, bas och instrument direkt ur färdiga mixar")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));

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
                ui.label(egui::RichText::new(crate::i18n::t("Källfil:")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
                ui.label(egui::RichText::new(&project.track_title).strong().size(Theme::FONT_S).color(Theme::FL_CYAN));
                ui.separator();

                let tempo_txt = if project.bpm > 0.0 {
                    format!("{:.0} BPM", project.bpm)
                } else {
                    crate::i18n::t("— BPM").to_string()
                };
                ui.label(egui::RichText::new(format!("{} {:.1}s  •  {} {}", crate::i18n::t("Längd:"), project.duration_seconds, crate::i18n::t("Tempo:"), tempo_txt)).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
                ui.separator();

                ui.label(egui::RichText::new(format!("Modell: {}", project.model_name)).size(Theme::FONT_S).color(Theme::FL_YELLOW));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(!project.is_separating, egui::Button::new(egui::RichText::new(crate::i18n::t("⚡ Välj fil & separera")).strong().color(Color32::WHITE)).fill(Color32::from_rgb(140, 40, 180))).clicked() {
                        actions.request_separation = true;
                    }
                });
            });
        });

        ui.add_space(8.0);

        // Neural backend status + live progress.
        ui.group(|ui| {
            ui.horizontal(|ui| {
                if crate::audio::neural_available() {
                    ui.label(egui::RichText::new(format!("🧠 {} {}", crate::i18n::t("Neural modell:"), crate::audio::neural_model_status())).size(Theme::FONT_S).color(Theme::FL_GREEN));
                } else {
                    ui.label(egui::RichText::new(format!("🧠 {} {} ({})", crate::i18n::t("Neural: ingen modell hittad — använder DSP. Lägg htdemucs.onnx i"), crate::audio::neural_model_status(), crate::i18n::t("bygg med --features neural"))).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
                }
            });
            ui.horizontal(|ui| {
                if ui
                    .checkbox(stem_six, crate::i18n::t("6 stämmor (gitarr + piano)"))
                    .on_hover_text(crate::i18n::t(
                        "Sex stämmor kräver 6s-modellen (136 MB). Fyra stämmor hittar nästan alltid gitarr och piano i 'instrument' i stället.",
                    ))
                    .changed()
                {
                    // Ett modelldownload för det andra valet kan inte återanvändas.
                    if let Ok(mut g) = model_download.lock() {
                        *g = (false, 0.0, String::new());
                    }
                }
                ui.label(
                    egui::RichText::new(crate::tstatus!(
                        "Modell: {}",
                        model.label()
                    ))
                    .size(Theme::FONT_S)
                    .color(Theme::TEXT_MUTED),
                );
            });
            // Modellen skeppas inte med: den som vill köra neural separering hämtar den.
            if crate::audio::neural_separator::find_model_for(model).is_none() {
                let state = model_download.lock().map(|g| g.clone()).unwrap_or((false, 0.0, String::new()));
                let (running, progress, message) = state;
                ui.horizontal(|ui| {
                    if !running
                        && ui
                            .button(crate::i18n::t("⬇ Ladda ner modell från repot"))
                            .on_hover_text(crate::i18n::t(
                                "Hämtar HTDemucs (158 MB, MIT) till modellmappen. Modellen ligger inte i installationen.",
                            ))
                            .clicked()
                    {
                        let slot = model_download.clone();
                        if let Ok(mut g) = slot.lock() {
                            *g = (true, 0.0, crate::i18n::t("Hämtar modellen...").to_string());
                        }
                        std::thread::spawn(move || {
                            let slot_for_progress = slot.clone();
                            let result = crate::audio::neural_separator::download_model_for(model, |p| {
                                if let Ok(mut g) = slot_for_progress.lock() {
                                    g.1 = p;
                                }
                            });
                            if let Ok(mut g) = slot.lock() {
                                *g = match result {
                                    Ok(path) => (false, 1.0, crate::tstatus!("Klar: {}", path.display())),
                                    Err(e) => (false, 0.0, crate::tstatus!("Fel: {}", e)),
                                };
                            }
                        });
                    }
                    if running {
                        ui.add(
                            egui::ProgressBar::new(progress.clamp(0.0, 1.0))
                                .desired_width(200.0)
                                .text(format!("{:.0}%", progress * 100.0)),
                        );
                    }
                    if !message.is_empty() {
                        ui.label(egui::RichText::new(message).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
                    }
                });
            }
            if project.is_separating {
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("⏳ Separerar...")).strong().size(Theme::FONT_S).color(Theme::FL_ORANGE));
                    ui.add(egui::ProgressBar::new(project.progress.clamp(0.0, 1.0)).desired_width(260.0).text(format!("{:.0}%", project.progress * 100.0)));
                });
            }
        });

        ui.add_space(8.0);

        if project.stems.is_empty() {
            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Ingen mix separerad ännu. Klicka på \"Välj fil & separera\" och välj en WAV/MP3/FLAC-fil — Sonix delar upp den i fyra spelbara stämspår.")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
            });
        }

        // 4 Isolated Stem Waveform Channels
        for (stem_idx, stem) in project.stems.iter_mut().enumerate() {
            let col = stem.stem_type.color();
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Left Stem Header & Mixer Controls
                    ui.group(|ui| {
                        ui.set_width(170.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(stem.stem_type.name()).strong().size(Theme::FONT_S).color(col));
                            ui.add_space(2.0);

                            ui.horizontal(|ui| {
                                let m_col = if stem.muted { Color32::from_rgb(80, 20, 20) } else { Theme::FL_GREEN };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("MUTE")).size(Theme::FONT_S)).fill(m_col)).clicked() {
                                    stem.muted = !stem.muted;
                                }
                                let s_col = if stem.solo { Theme::FL_ORANGE } else { Color32::from_rgb(40, 35, 25) };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("SOLO")).size(Theme::FONT_S)).fill(s_col)).clicked() {
                                    stem.solo = !stem.solo;
                                }
                            });

                            ui.horizontal(|ui| {
                                rotary_knob_full(ui, &mut stem.volume, 0.0, 1.25, "VOL", col, 22.0);
                                rotary_knob_full(ui, &mut stem.pan, -1.0, 1.0, "PAN", Color32::WHITE, 22.0);
                                if rotary_knob_full(ui, &mut stem.time_stretch, 0.5, 2.0, "SPEED", Theme::FL_YELLOW, 22.0).1 {
                                    actions.speed_changed = Some(stem_idx);
                                }
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
                    // Exakt väg (8.3): (min, max) per kolumn ur stämmans egna samplar.
                    // Den gamla vägen ritade `mid ± amp`, alltså ett symmetriskt hölje
                    // ur en översikt på 512 punkter — en vågform som såg trovärdig ut
                    // men inte var signalen. Finns paren ritas de; annars gamla vägen.
                    let pairs = &stem.waveform_pairs;
                    if !pairs.is_empty() {
                        let col_w = (wave_rect.width() - 8.0).max(1.0) / pairs.len() as f32;
                        let half = wave_rect.height() * 0.46;
                        for (idx, (lo, hi)) in pairs.iter().enumerate() {
                            let bx = wave_rect.min.x + 4.0 + idx as f32 * col_w;
                            let y_hi = mid_y - hi.clamp(-1.0, 1.0) * half;
                            let y_lo = mid_y - lo.clamp(-1.0, 1.0) * half;
                            let (top, bot) = (y_hi.min(y_lo), y_hi.max(y_lo));
                            let col_rect = Rect::from_min_max(
                                Pos2::new(bx, top),
                                Pos2::new(bx + col_w.max(0.7), (bot).max(top + 0.6)),
                            );
                            let fill = if stem.muted {
                                Theme::CHANNEL_BG
                            } else {
                                col
                            };
                            painter.rect_filled(col_rect, Rounding::same(0.5), fill);
                        }
                    } else if num_bars > 0 {
                        let bar_w = (wave_rect.width() - 8.0).max(1.0) / num_bars as f32;

                        for (idx, &amp) in stem.waveform_data.iter().enumerate() {
                            let bx = wave_rect.min.x + 4.0 + idx as f32 * bar_w;
                            let h = amp * (wave_rect.height() * 0.42);
                            let bar_rect = Rect::from_min_max(Pos2::new(bx, mid_y - h), Pos2::new(bx + bar_w * 0.8, mid_y + h));
                            let fill = if stem.muted { Theme::CHANNEL_BG } else { col };
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
