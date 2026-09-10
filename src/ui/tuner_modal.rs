use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use crate::audio::{AudioCommand, AudioEngine};
use crate::ui::theme::Theme;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TuningPreset {
    GuitarStandard,
    GuitarDropD,
    Guitar7String,
    BassStandard,
    Ukulele,
    Violin,
    ChromaticVocal,
}

impl TuningPreset {
    pub fn all() -> &'static [(TuningPreset, &'static str)] {
        &[
            (TuningPreset::GuitarStandard, "🎸 Gitarr Standard (E A D G B E)"),
            (TuningPreset::GuitarDropD, "🎸 Gitarr Drop D (D A D G B E)"),
            (TuningPreset::Guitar7String, "🎸 7-Strängad Gitarr (B E A D G B E)"),
            (TuningPreset::BassStandard, "🎸 Elbas 4-strängad (E A D G)"),
            (TuningPreset::Ukulele, "🪕 Ukulele (G C E A)"),
            (TuningPreset::Violin, "🎻 Violin / Fiol (G D A E)"),
            (TuningPreset::ChromaticVocal, "🎙 Kromatisk / Sång (Alla toner)"),
        ]
    }

    pub fn strings(&self) -> &'static [(&'static str, f32, u8)] {
        match self {
            TuningPreset::GuitarStandard => &[
                ("1: E4", 329.63, 64),
                ("2: B3", 246.94, 59),
                ("3: G3", 196.00, 55),
                ("4: D3", 146.83, 50),
                ("5: A2", 110.00, 45),
                ("6: E2", 82.41, 40),
            ],
            TuningPreset::GuitarDropD => &[
                ("1: E4", 329.63, 64),
                ("2: B3", 246.94, 59),
                ("3: G3", 196.00, 55),
                ("4: D3", 146.83, 50),
                ("5: A2", 110.00, 45),
                ("6: D2", 73.42, 38),
            ],
            TuningPreset::Guitar7String => &[
                ("1: E4", 329.63, 64),
                ("2: B3", 246.94, 59),
                ("3: G3", 196.00, 55),
                ("4: D3", 146.83, 50),
                ("5: A2", 110.00, 45),
                ("6: E2", 82.41, 40),
                ("7: B1", 61.74, 35),
            ],
            TuningPreset::BassStandard => &[
                ("1: G2", 98.00, 43),
                ("2: D2", 73.42, 38),
                ("3: A1", 55.00, 33),
                ("4: E1", 41.20, 28),
            ],
            TuningPreset::Ukulele => &[
                ("1: A4", 440.00, 69),
                ("2: E4", 329.63, 64),
                ("3: C4", 261.63, 60),
                ("4: G4", 392.00, 67),
            ],
            TuningPreset::Violin => &[
                ("1: E5", 659.25, 76),
                ("2: A4", 440.00, 69),
                ("3: D4", 293.66, 62),
                ("4: G3", 196.00, 55),
            ],
            TuningPreset::ChromaticVocal => &[
                ("C4", 261.63, 60),
                ("E4", 329.63, 64),
                ("G4", 392.00, 67),
                ("A4 (440Hz)", 440.00, 69),
                ("C5", 523.25, 72),
            ],
        }
    }
}

#[derive(Clone, Debug)]
pub struct TunerState {
    pub preset: TuningPreset,
    pub reference_hz: f32, // 440.0 or 432.0
    pub selected_string_idx: usize,
    pub simulated_pitch_cents: f32, // -50.0 .. +50.0
    pub is_tone_playing: bool,
    pub auto_detect_closest_string: bool,
}

impl Default for TunerState {
    fn default() -> Self {
        Self {
            preset: TuningPreset::GuitarStandard,
            reference_hz: 440.0,
            selected_string_idx: 0,
            simulated_pitch_cents: 0.0,
            is_tone_playing: false,
            auto_detect_closest_string: true,
        }
    }
}

pub fn render_tuner_modal(
    ctx: &egui::Context,
    open: &mut bool,
    state: &mut TunerState,
    engine: &mut AudioEngine,
    mic_vu_level: f32,
    live_pitch_hz: Option<f32>,
) {
    if !*open {
        return;
    }

    let mut close = false;

    egui::Window::new(crate::i18n::t("🎯 Hårdvarustämapparat & Pitch Scope (Tuner)"))
        .open(open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(540.0, 480.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("STÄMAPPARAT & PITCH ANALYZER")).strong().size(14.0).color(Theme::FL_CYAN));
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::t("Stäm gitarr, bas eller träna röstintonation i realtid")).size(10.5).color(Theme::TEXT_MUTED));
                });

                ui.add_space(4.0);

                // Instrument & Preset Selector
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Instrument:")).strong().color(Theme::FL_ORANGE));
                        let presets = TuningPreset::all();
                        let current_label = presets.iter().find(|(p, _)| *p == state.preset).map(|(_, l)| crate::i18n::t(l)).unwrap_or("Gitarr");

                        egui::ComboBox::from_id_salt("tuner_preset_picker")
                            .selected_text(current_label)
                            .width(260.0)
                            .show_ui(ui, |ui| {
                                for (p, label) in presets {
                                    if ui.selectable_label(state.preset == *p, crate::i18n::t(*label)).clicked() {
                                        state.preset = p.clone();
                                        state.selected_string_idx = 0;
                                    }
                                }
                            });

                        ui.separator();

                        ui.label(crate::i18n::t("Ref:"));
                        if ui.selectable_label((state.reference_hz - 440.0).abs() < 0.1, "440 Hz").clicked() {
                            state.reference_hz = 440.0;
                        }
                        if ui.selectable_label((state.reference_hz - 432.0).abs() < 0.1, "432 Hz").clicked() {
                            state.reference_hz = 432.0;
                        }
                    });
                });

                ui.add_space(6.0);

                // String Selector Tabs
                let strings = state.preset.strings();
                ui.horizontal(|ui| {
                    for (s_idx, (name, _, _)) in strings.iter().enumerate() {
                        let is_sel = state.selected_string_idx == s_idx;
                        let bg = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(26, 30, 38) };
                        let fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                        if ui.add(egui::Button::new(egui::RichText::new(*name).strong().size(11.0).color(fg)).fill(bg)).clicked() {
                            state.selected_string_idx = s_idx;
                        }
                    }
                });

                ui.add_space(8.0);

                let current_str = strings.get(state.selected_string_idx).cloned().unwrap_or(("A4", 440.0, 69));

                // Real pitch detection: snap the live microphone reading to the
                // closest string (when auto-detect is on) and compute cents.
                let detected = live_pitch_hz.map(|f| {
                    let mut chosen = current_str;
                    if state.auto_detect_closest_string {
                        let mut best = chosen;
                        let mut best_dist = f32::MAX;
                        for s in strings {
                            let dist = (1200.0 * (f / s.1).log2()).abs();
                            if dist < best_dist {
                                best_dist = dist;
                                best = *s;
                            }
                        }
                        chosen = best;
                    }
                    let cents = 1200.0 * (f / chosen.1).log2();
                    (chosen, cents, f)
                });

                let (active_str, cents) = match detected {
                    Some((s, c, _)) => (s, c.clamp(-50.0, 50.0)),
                    None => (current_str, state.simulated_pitch_cents),
                };
                let target_name = active_str.0;
                let target_freq = active_str.1 * (state.reference_hz / 440.0);
                let midi_note = active_str.2;
                let live_hz = detected.map(|(_, _, f)| f);

                // MAIN STROBE / NEEDLE DISPLAY
                ui.group(|ui| {
                    let in_tune = cents.abs() <= 3.0;

                    // Large Note display
                    ui.vertical_centered(|ui| {
                        let note_color = if in_tune {
                            Theme::FL_GREEN
                        } else if cents > 0.0 {
                            Theme::FL_ORANGE // Sharp
                        } else {
                            Theme::FL_CYAN // Flat
                        };

                        ui.label(egui::RichText::new(target_name).strong().size(36.0).color(note_color));

                        let status_text = if in_tune {
                            crate::i18n::t("✨ PERFEKT STÄMD (IN TUNE)")
                        } else if cents > 0.0 {
                            crate::i18n::t("▲ FÖR HÖG (SHARP) - Släpp efter")
                        } else {
                            crate::i18n::t("▼ FÖR LÅG (FLAT) - Spänn strängen")
                        };
                        ui.label(egui::RichText::new(status_text).strong().size(12.0).color(note_color));

                        ui.label(egui::RichText::new(crate::tstatus!("Målfrekvens: {:.2} Hz  •  Offset: {:+.1} Cents", target_freq, cents)).size(11.0).color(Theme::TEXT_MUTED));

                        // Live signal readout: detected frequency + input level
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(match live_hz {
                                    Some(f) => crate::tstatus!("🎤 Live: {:.1} Hz", f),
                                    None => crate::i18n::t("🎤 Live: — (tyst)").to_string(),
                                })
                                .size(10.5)
                                .color(if live_hz.is_some() { Theme::FL_GREEN } else { Theme::TEXT_MUTED }),
                            );
                            ui.add(
                                egui::ProgressBar::new(mic_vu_level.clamp(0.0, 1.0))
                                    .desired_width(120.0)
                                    .text(crate::i18n::t("Ingång")),
                            );
                        });
                    });

                    ui.add_space(8.0);

                    // Large Arc / Meter Needle Visualization
                    let (m_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 48.0), Sense::hover());
                    ui.painter().rect_filled(m_rect, Rounding::same(6.0), Color32::from_rgb(14, 18, 24));
                    ui.painter().rect_stroke(m_rect, Rounding::same(6.0), Stroke::new(1.0_f32, Color32::from_rgb(38, 45, 60)));

                    // Center 0 cent mark
                    let center_x = m_rect.center().x;
                    ui.painter().line_segment(
                        [Pos2::new(center_x, m_rect.min.y + 4.0), Pos2::new(center_x, m_rect.max.y - 4.0)],
                        Stroke::new(2.5_f32, if in_tune { Theme::FL_GREEN } else { Color32::from_rgb(90, 100, 120) }),
                    );

                    // In-tune safe zone box
                    let safe_zone_w = m_rect.width() * (6.0 / 100.0);
                    let safe_rect = Rect::from_center_size(Pos2::new(center_x, m_rect.center().y), Vec2::new(safe_zone_w, m_rect.height() - 8.0));
                    ui.painter().rect_filled(safe_rect, Rounding::same(2.0), Color32::from_rgba_unmultiplied(46, 204, 113, 30));

                    // Scale tick marks: -50, -25, 0, +25, +50
                    for tick in [-50, -35, -20, -10, 10, 20, 35, 50] {
                        let tx = center_x + (tick as f32 / 50.0) * (m_rect.width() * 0.45);
                        ui.painter().line_segment(
                            [Pos2::new(tx, m_rect.min.y + 8.0), Pos2::new(tx, m_rect.min.y + 20.0)],
                            Stroke::new(1.0_f32, Color32::from_rgb(50, 60, 80)),
                        );
                    }

                    // Needle pointer position
                    let needle_x = center_x + (cents / 50.0).clamp(-1.0, 1.0) * (m_rect.width() * 0.45);
                    let needle_col = if in_tune { Theme::FL_GREEN } else if cents > 0.0 { Theme::FL_ORANGE } else { Theme::FL_CYAN };

                    ui.painter().line_segment(
                        [Pos2::new(needle_x, m_rect.min.y + 2.0), Pos2::new(needle_x, m_rect.max.y - 2.0)],
                        Stroke::new(3.0_f32, needle_col),
                    );
                    ui.painter().circle_filled(Pos2::new(needle_x, m_rect.min.y + 6.0), 4.5, needle_col);

                    ui.add_space(8.0);

                    // Quick simulated fine adjustment slider for manual check or testing
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Finjustering (Test):")).size(10.5).color(Theme::TEXT_MUTED));
                        ui.add(egui::Slider::new(&mut state.simulated_pitch_cents, -50.0..=50.0).text("Cents"));
                        if ui.button(crate::i18n::t("🎯 Nollställ")).clicked() {
                            state.simulated_pitch_cents = 0.0;
                        }
                    });
                });

                ui.add_space(8.0);

                // Reference Tone & Calibration Controls
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        let btn_txt = if state.is_tone_playing { crate::i18n::t("⏹ Stoppa Referenston") } else { crate::i18n::t("🔊 Spela Referenston (Sinuston)") };
                        let btn_col = if state.is_tone_playing { Color32::from_rgb(220, 40, 40) } else { Theme::FL_GREEN };
                        if ui.add(egui::Button::new(egui::RichText::new(btn_txt).strong().color(Color32::WHITE)).fill(btn_col).min_size(Vec2::new(200.0, 30.0))).clicked() {
                            state.is_tone_playing = !state.is_tone_playing;
                            if state.is_tone_playing {
                                let _ = engine.send_command(AudioCommand::NoteOn { note: midi_note, freq: target_freq, velocity: 0.70 });
                            } else {
                                let _ = engine.send_command(AudioCommand::NoteOff { note: midi_note });
                            }
                        }

                        ui.separator();
                        ui.checkbox(&mut state.auto_detect_closest_string, crate::i18n::t("🔍 Automatisk strängdetektering"));
                    });
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::t("Stäng")).clicked() {
                        if state.is_tone_playing {
                            let _ = engine.send_command(AudioCommand::NoteOff { note: midi_note });
                            state.is_tone_playing = false;
                        }
                        close = true;
                    }
                });
            });
        });

    if close {
        *open = false;
    }
}
