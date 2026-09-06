use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Vec2};
use crate::audio::{AudioCommand, AudioEngine};
use crate::ui::theme::Theme;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiceCategory {
    MelodyLead,
    Bassline,
    DrumBeat,
    ChordArp,
    SoundFx,
}

#[derive(Clone, Debug)]
pub struct GeneratedStepNote {
    pub step: usize,
    pub note: u8,
    pub velocity: f32,
    pub length_steps: usize,
}

#[derive(Clone, Debug)]
pub struct DiceGeneratorState {
    pub category: DiceCategory,
    pub genre_preset: usize, // 0=Trap, 1=Synthwave, 2=LoFi, 3=Drill, 4=Cyberpunk, 5=House
    pub density: f32, // 0.0 .. 1.0
    pub humanize_pct: f32, // 0.0 .. 100.0
    pub syncopation_pct: f32,
    pub octave_range: u8, // 1..3
    pub generated_notes: Vec<GeneratedStepNote>,
    pub generated_drum_grid: [[bool; 16]; 4], // 0=Kick, 1=Snare, 2=HiHat, 3=Perc
    pub is_previewing: bool,
}

impl Default for DiceGeneratorState {
    fn default() -> Self {
        let mut s = Self {
            category: DiceCategory::MelodyLead,
            genre_preset: 0,
            density: 0.65,
            humanize_pct: 25.0,
            syncopation_pct: 40.0,
            octave_range: 2,
            generated_notes: Vec::new(),
            generated_drum_grid: [[false; 16]; 4],
            is_previewing: false,
        };
        s.roll_dice();
        s
    }
}

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

impl DiceGeneratorState {
    pub fn roll_dice(&mut self) {
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_micros();
        let scale = [0, 2, 3, 5, 7, 8, 10]; // Minor scale intervals
        let base_root = 60; // C4

        match self.category {
            DiceCategory::MelodyLead | DiceCategory::ChordArp => {
                self.generated_notes.clear();
                for step in 0..16 {
                    let r = ((seed >> (step * 2)) % 100) as f32 / 100.0;
                    if r < self.density {
                        let scale_idx = ((seed >> (step * 3 + 1)) % scale.len() as u128) as usize;
                        let oct = ((seed >> (step + 4)) % self.octave_range.max(1) as u128) as u8;
                        let note = base_root + scale[scale_idx] + (oct * 12);
                        let vel = 0.60 + (((seed >> (step + 7)) % 40) as f32 / 100.0);
                        let len = if (seed >> (step + 2)) % 3 == 0 { 2 } else { 1 };
                        self.generated_notes.push(GeneratedStepNote {
                            step,
                            note,
                            velocity: vel,
                            length_steps: len,
                        });
                    }
                }
            }
            DiceCategory::Bassline => {
                self.generated_notes.clear();
                let bass_root = 36; // C2
                for step in [0, 3, 6, 8, 10, 12, 14] {
                    let r = ((seed >> step) % 100) as f32 / 100.0;
                    if r < self.density + 0.15 {
                        let scale_idx = ((seed >> (step + 2)) % 4) as usize; // Root, b3, 4, 5
                        let note = bass_root + scale[scale_idx];
                        self.generated_notes.push(GeneratedStepNote {
                            step,
                            note,
                            velocity: 0.90,
                            length_steps: 2,
                        });
                    }
                }
            }
            DiceCategory::DrumBeat => {
                self.generated_drum_grid = [[false; 16]; 4];
                // Kick: standard beats 0, 8 + syncopated
                self.generated_drum_grid[0][0] = true;
                self.generated_drum_grid[0][8] = true;
                if ((seed >> 2) % 10) > 3 { self.generated_drum_grid[0][6] = true; }
                if ((seed >> 5) % 10) > 4 { self.generated_drum_grid[0][10] = true; }

                // Snare: beats 4, 12
                self.generated_drum_grid[1][4] = true;
                self.generated_drum_grid[1][12] = true;
                if ((seed >> 8) % 10) > 6 { self.generated_drum_grid[1][15] = true; } // Ghost note

                // HiHat: 8th or 16th notes + rolls
                for s in 0..16 {
                    if s % 2 == 0 || ((seed >> s) % 10) > 4 {
                        self.generated_drum_grid[2][s] = true;
                    }
                }

                // Perc / Claps
                for s in [2, 7, 11, 14] {
                    if ((seed >> (s + 3)) % 10) > 5 {
                        self.generated_drum_grid[3][s] = true;
                    }
                }
            }
            DiceCategory::SoundFx => {
                self.generated_notes.clear();
                for step in [0, 4, 8, 12] {
                    self.generated_notes.push(GeneratedStepNote {
                        step,
                        note: 72 + (step as u8 * 2),
                        velocity: 0.80,
                        length_steps: 4,
                    });
                }
            }
        }
    }
}

pub fn render_dice_generator_modal(
    ctx: &egui::Context,
    open: &mut bool,
    state: &mut DiceGeneratorState,
    engine: &mut AudioEngine,
    status_msg: &mut String,
    mut on_insert_to_pattern: impl FnMut(&DiceGeneratorState),
) {
    if !*open {
        return;
    }

    let mut close = false;
    let mut trigger_insert = false;

    egui::Window::new("🎲 Melodi- & Beat-Tärning (Idea Spark & Randomizer)")
        .open(open)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(640.0, 520.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Header
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("🎲 SPARK & GENERATOR").strong().size(14.0).color(Theme::FL_ORANGE));
                    ui.separator();
                    ui.label(egui::RichText::new("Slumpa vilda melodier, unika grooves, trap-rolls och basgångar med 1 klick").size(10.5).color(Theme::TEXT_MUTED));
                });

                ui.add_space(4.0);

                // Category Buttons
                ui.horizontal(|ui| {
                    let cats = [
                        (DiceCategory::MelodyLead, "🎼 Melodi / Lead"),
                        (DiceCategory::Bassline, "🎸 Basgång (Bass)"),
                        (DiceCategory::DrumBeat, "🥁 Trumgroove & Beat"),
                        (DiceCategory::ChordArp, "✨ Arpeggio"),
                        (DiceCategory::SoundFx, "🎛 Sound FX / Riser"),
                    ];
                    for (cat, label) in cats {
                        let is_sel = state.category == cat;
                        let bg = if is_sel { Theme::FL_ORANGE } else { Color32::from_rgb(26, 30, 38) };
                        let fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                        if ui.add(egui::Button::new(egui::RichText::new(label).strong().size(11.0).color(fg)).fill(bg)).clicked() {
                            state.category = cat;
                            state.roll_dice();
                        }
                    }
                });

                ui.add_space(6.0);

                // Genre Presets & Style Tuning
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Genre / Stil:").strong().color(Theme::FL_CYAN));
                        let genres = ["Trap & 808", "Synthwave 80s", "Lo-Fi Hip Hop", "UK Drill", "Cyberpunk Industrial", "House 4x4"];
                        egui::ComboBox::from_id_salt("dice_genre_picker")
                            .selected_text(*genres.get(state.genre_preset).unwrap_or(&"Trap"))
                            .width(180.0)
                            .show_ui(ui, |ui| {
                                for (g_idx, &g_name) in genres.iter().enumerate() {
                                    if ui.selectable_label(state.genre_preset == g_idx, g_name).clicked() {
                                        state.genre_preset = g_idx;
                                        state.roll_dice();
                                    }
                                }
                            });

                        ui.separator();

                        ui.label("Oktavomfång:");
                        for oct in 1..=3 {
                            if ui.selectable_label(state.octave_range == oct, format!("{} Okt", oct)).clicked() {
                                state.octave_range = oct;
                                state.roll_dice();
                            }
                        }
                    });

                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        let dens_str = format!("Täthet: {:.0}%", state.density * 100.0);
                        if ui.add(egui::Slider::new(&mut state.density, 0.1..=1.0).text(dens_str)).changed() {
                            state.roll_dice();
                        }

                        ui.separator();
                        let hum_str = format!("Humanize: {:.0}%", state.humanize_pct);
                        ui.add(egui::Slider::new(&mut state.humanize_pct, 0.0..=100.0).text(hum_str));
                    });
                });

                ui.add_space(8.0);

                // Visual Pattern Preview Canvas
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("📊 GENERERAT MÖNSTER (16 STEG)").strong().color(Theme::FL_GREEN));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.add(egui::Button::new(egui::RichText::new("🎲 KASTA TÄRNINGEN IGEN").strong().color(Color32::BLACK)).fill(Theme::FL_YELLOW)).clicked() {
                                state.roll_dice();
                                *status_msg = "🎲 Rullade ny slumpmässig idé!".to_string();
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // Step visualization grid
                    let (canvas_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 90.0), Sense::hover());
                    ui.painter().rect_filled(canvas_rect, Rounding::same(4.0), Color32::from_rgb(16, 19, 26));
                    ui.painter().rect_stroke(canvas_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(36, 42, 54)));

                    let step_w = canvas_rect.width() / 16.0;

                    // Draw 16 step lanes
                    for s in 0..16 {
                        let sx = canvas_rect.min.x + s as f32 * step_w;
                        let is_beat = s % 4 == 0;
                        let line_col = if is_beat { Color32::from_rgb(55, 65, 85) } else { Color32::from_rgb(26, 32, 42) };
                        ui.painter().line_segment([Pos2::new(sx, canvas_rect.min.y), Pos2::new(sx, canvas_rect.max.y)], Stroke::new(1.0_f32, line_col));

                        // Step number label
                        if is_beat {
                            ui.painter().text(
                                Pos2::new(sx + 3.0, canvas_rect.min.y + 10.0),
                                egui::Align2::LEFT_CENTER,
                                format!("{}", (s / 4) + 1),
                                egui::FontId::proportional(9.0),
                                Theme::TEXT_MUTED,
                            );
                        }
                    }

                    if state.category == DiceCategory::DrumBeat {
                        // Render 4 drum lanes: Kick, Snare, HiHat, Perc
                        let lane_h = (canvas_rect.height() - 16.0) / 4.0;
                        let drum_names = ["KICK", "SNARE", "HIHAT", "PERC"];
                        let drum_colors = [Theme::FL_ORANGE, Theme::FL_CYAN, Theme::FL_YELLOW, Theme::FL_PURPLE];

                        for d in 0..4 {
                            let ly = canvas_rect.min.y + 16.0 + d as f32 * lane_h;
                            ui.painter().text(
                                Pos2::new(canvas_rect.min.x + 4.0, ly + lane_h * 0.5),
                                egui::Align2::LEFT_CENTER,
                                drum_names[d],
                                egui::FontId::proportional(8.5),
                                drum_colors[d],
                            );

                            for s in 0..16 {
                                if state.generated_drum_grid[d][s] {
                                    let bx = canvas_rect.min.x + s as f32 * step_w;
                                    let b_rect = Rect::from_min_size(Pos2::new(bx + 2.0, ly + 2.0), Vec2::new(step_w - 4.0, lane_h - 4.0));
                                    ui.painter().rect_filled(b_rect, Rounding::same(2.0), drum_colors[d]);
                                }
                            }
                        }
                    } else {
                        // Render Melodic Notes
                        for note in &state.generated_notes {
                            let bx = canvas_rect.min.x + note.step as f32 * step_w;
                            let n_len = (note.length_steps as f32 * step_w) - 3.0;
                            let n_norm = ((note.note.saturating_sub(36)) as f32 / 48.0).clamp(0.05, 0.90);
                            let ny = canvas_rect.max.y - (n_norm * (canvas_rect.height() - 24.0)) - 14.0;

                            let note_rect = Rect::from_min_size(Pos2::new(bx + 2.0, ny), Vec2::new(n_len, 12.0));
                            ui.painter().rect_filled(note_rect, Rounding::same(3.0), Theme::FL_CYAN);

                            let n_name = format!("{}{}", NOTE_NAMES[(note.note % 12) as usize], (note.note / 12) - 1);
                            ui.painter().text(
                                Pos2::new(note_rect.min.x + 3.0, note_rect.center().y),
                                egui::Align2::LEFT_CENTER,
                                n_name,
                                egui::FontId::proportional(8.5),
                                Color32::BLACK,
                            );
                        }
                    }
                });

                ui.add_space(8.0);

                // Action Buttons
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new("📥 Klistra in Mönster i Spår").strong().color(Color32::BLACK)).fill(Theme::FL_GREEN).min_size(Vec2::new(200.0, 32.0))).clicked() {
                        trigger_insert = true;
                        close = true;
                    }

                    if ui.add(egui::Button::new(egui::RichText::new("▶ Provspela Mönster").strong().color(Color32::WHITE)).fill(Color32::from_rgb(45, 90, 140)).min_size(Vec2::new(160.0, 32.0))).clicked() {
                        if state.category == DiceCategory::DrumBeat {
                            let _ = engine.send_command(AudioCommand::TriggerDrum(crate::audio::DrumType::Kick));
                            let _ = engine.send_command(AudioCommand::TriggerDrum(crate::audio::DrumType::Snare));
                        } else {
                            for n in &state.generated_notes {
                                let freq = 440.0 * 2.0_f32.powf((n.note as f32 - 69.0) / 12.0);
                                let _ = engine.send_command(AudioCommand::NoteOn { note: n.note, freq, velocity: n.velocity });
                            }
                        }
                        *status_msg = "▶ Provspelar slumpat mönster...".to_string();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Stäng").clicked() {
                            close = true;
                        }
                    });
                });
            });
        });

    if trigger_insert {
        on_insert_to_pattern(state);
    }

    if close {
        *open = false;
    }
}
