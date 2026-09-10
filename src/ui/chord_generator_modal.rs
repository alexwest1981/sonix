use eframe::egui::{self, Color32, Pos2, Rounding, Sense, Vec2};
use crate::audio::{AudioCommand, AudioEngine};
use crate::ui::theme::Theme;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScaleType {
    Major,
    NaturalMinor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    HarmonicMinor,
    MinorPentatonic,
    MajorPentatonic,
    Blues,
    JapaneseHirajoshi,
    CyberpunkPhrygianDominant,
}

impl ScaleType {
    pub fn all() -> &'static [(ScaleType, &'static str)] {
        &[
            (ScaleType::NaturalMinor, "Moll (Natural Minor)"),
            (ScaleType::Major, "Dur (Major)"),
            (ScaleType::Dorian, "Dorisk (Dorian / Funk & Synthwave)"),
            (ScaleType::Phrygian, "Frygisk (Phrygian / Dark EDM & Metal)"),
            (ScaleType::Lydian, "Lydisk (Lydian / Dreamy & Space)"),
            (ScaleType::Mixolydian, "Mixolydisk (Mixolydian / Rock & Blues)"),
            (ScaleType::HarmonicMinor, "Harmonisk Moll (Harmonic Minor)"),
            (ScaleType::MinorPentatonic, "Moll Pentatonisk (Trap & Rock)"),
            (ScaleType::MajorPentatonic, "Dur Pentatonisk (Soul & Pop)"),
            (ScaleType::Blues, "Blues Skala (Blues & Soul)"),
            (ScaleType::JapaneseHirajoshi, "Hirajoshi (Japansk / Ambient)"),
            (ScaleType::CyberpunkPhrygianDominant, "Frygisk Dominant (Cyberpunk / Flamenco)"),
        ]
    }

    pub fn intervals(&self) -> &'static [u8] {
        match self {
            ScaleType::Major => &[0, 2, 4, 5, 7, 9, 11],
            ScaleType::NaturalMinor => &[0, 2, 3, 5, 7, 8, 10],
            ScaleType::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            ScaleType::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            ScaleType::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            ScaleType::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            ScaleType::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            ScaleType::MinorPentatonic => &[0, 3, 5, 7, 10],
            ScaleType::MajorPentatonic => &[0, 2, 4, 7, 9],
            ScaleType::Blues => &[0, 3, 5, 6, 7, 10],
            ScaleType::JapaneseHirajoshi => &[0, 2, 3, 7, 8],
            ScaleType::CyberpunkPhrygianDominant => &[0, 1, 4, 5, 7, 8, 10],
        }
    }
}

#[derive(Clone, Debug)]
pub struct GeneratedChord {
    pub roman: String,
    pub name: String,
    pub root_midi: u8,
    pub notes: Vec<u8>,
    pub color: Color32,
}

#[derive(Clone, Debug)]
pub struct ChordGeneratorState {
    pub root_note: u8, // 0 = C, 1 = C#, ... 11 = B
    pub scale_type: ScaleType,
    pub octave: u8, // 3, 4, 5
    pub chord_voicing: usize, // 0 = Triad, 1 = 7th, 2 = 9th, 3 = Sus4, 4 = Inversion 1, 5 = Open
    pub current_progression: Vec<GeneratedChord>,
    pub selected_progression_preset: usize,
    pub strum_spread_ms: f32,
    pub arpeggiator_mode: usize, // 0 = Block, 1 = Up, 2 = Down, 3 = Random
}

impl Default for ChordGeneratorState {
    fn default() -> Self {
        let mut s = Self {
            root_note: 0, // C
            scale_type: ScaleType::NaturalMinor, // C Minor
            octave: 4,
            chord_voicing: 1, // 7th chords
            current_progression: Vec::new(),
            selected_progression_preset: 0,
            strum_spread_ms: 15.0,
            arpeggiator_mode: 0,
        };
        s.load_progression_preset(0);
        s
    }
}

const NOTE_NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

impl ChordGeneratorState {
    pub fn get_chord_pads(&self) -> Vec<GeneratedChord> {
        let intervals = self.scale_type.intervals();
        let num_deg = intervals.len();
        let base_midi = 12 * (self.octave + 1) + self.root_note;

        let roman_numerals_maj = ["I", "ii", "iii", "IV", "V", "vi", "vii°", "VIII"];
        let roman_numerals_min = ["i", "ii°", "III", "iv", "v", "VI", "VII", "VIII"];

        let mut pads = Vec::new();
        let pad_colors = [
            Theme::FL_CYAN,
            Theme::FL_ORANGE,
            Theme::FL_GREEN,
            Theme::FL_PURPLE,
            Theme::FL_YELLOW,
            Color32::from_rgb(255, 100, 150),
            Color32::from_rgb(100, 200, 255),
            Color32::from_rgb(255, 180, 80),
        ];

        for (deg_idx, &interval) in intervals.iter().enumerate() {
            let chord_root = base_midi + interval;
            let root_name = NOTE_NAMES[(chord_root % 12) as usize];

            // Build chord degrees from scale
            let n1 = chord_root;
            let n2 = base_midi + intervals[(deg_idx + 2) % num_deg] + if (deg_idx + 2) >= num_deg { 12 } else { 0 };
            let n3 = base_midi + intervals[(deg_idx + 4) % num_deg] + if (deg_idx + 4) >= num_deg { 12 } else { 0 };
            let n4 = base_midi + intervals[(deg_idx + 6) % num_deg] + if (deg_idx + 6) >= num_deg { 12 } else { 0 };

            let (notes, chord_ext) = match self.chord_voicing {
                0 => (vec![n1, n2, n3], ""), // Triad
                1 => (vec![n1, n2, n3, n4], "7"), // 7th
                2 => {
                    let n5 = base_midi + intervals[(deg_idx + 8) % num_deg] + if (deg_idx + 8) >= num_deg { 24 } else { 12 };
                    (vec![n1, n2, n3, n4, n5], "9")
                }
                3 => {
                    let sus = base_midi + intervals[(deg_idx + 3) % num_deg] + if (deg_idx + 3) >= num_deg { 12 } else { 0 };
                    (vec![n1, sus, n3], "sus4")
                }
                4 => (vec![n2, n3, n1 + 12], " inv1"), // First inversion
                _ => (vec![n1 - 12, n3, n2 + 12, n4], " open"),
            };

            let roman = match self.scale_type {
                ScaleType::Major => roman_numerals_maj.get(deg_idx).unwrap_or(&"I").to_string(),
                _ => roman_numerals_min.get(deg_idx).unwrap_or(&"i").to_string(),
            };

            let is_minor_deg = roman.chars().next().map_or(false, |c| c.is_lowercase());
            let chord_name = format!("{}{}{}", root_name, match self.chord_voicing {
                0 => if is_minor_deg { "m" } else { "" },
                1 => if is_minor_deg { "m7" } else { "maj7" },
                2 => if is_minor_deg { "m9" } else { "maj9" },
                3 => "sus4",
                _ => "",
            }, chord_ext);

            pads.push(GeneratedChord {
                roman,
                name: chord_name,
                root_midi: chord_root,
                notes,
                color: pad_colors[deg_idx % pad_colors.len()],
            });
        }
        pads
    }

    pub fn load_progression_preset(&mut self, preset_idx: usize) {
        self.selected_progression_preset = preset_idx;
        let pads = self.get_chord_pads();
        if pads.is_empty() { return; }

        let indices: Vec<usize> = match preset_idx {
            0 => vec![0, 3, 4, 0], // i - iv - v - i (Dark Trap & Pop)
            1 => vec![5, 3, 0, 4], // VI - IV - I - V (EDM Festival Anthem)
            2 => vec![0, 5, 2, 6], // i - VI - III - VII (Moody Cyberpunk / Synthwave)
            3 => vec![1, 4, 0, 5], // ii - V - I - vi (Neo-Soul & Jazz Fusion)
            4 => vec![0, 3, 0, 4], // i - iv - i - V (Blues & Funk Groove)
            _ => vec![0, 2, 3, 4], // i - III - iv - v (Epic Cinematic)
        };

        self.current_progression = indices.into_iter().filter_map(|idx| pads.get(idx % pads.len()).cloned()).collect();
    }

    pub fn play_chord_sound(&self, chord: &GeneratedChord, engine: &mut AudioEngine) {
        let spread_samples = ((self.strum_spread_ms.max(0.0) / 1000.0)
            * engine.sample_rate as f32) as u32;
        let _ = engine.send_command(AudioCommand::StrumChord {
            notes: chord.notes.clone(),
            velocity: 0.85,
            start_samples: 0,
            spread_samples,
            mode: self.arpeggiator_mode as u8,
        });
    }

    /// Plays the whole progression as a real time sequence: each chord starts
    /// one "step" after the previous, honouring the strum/arpeggio settings.
    pub fn audition_progression(&self, engine: &mut AudioEngine) {
        if self.current_progression.is_empty() {
            return;
        }
        let spread_samples = ((self.strum_spread_ms.max(0.0) / 1000.0)
            * engine.sample_rate as f32) as u32;
        // One chord per beat at 90 BPM (~0.667s) so the sequence is musical.
        let chord_step = (engine.sample_rate as f32 * 0.667) as u32;
        for (i, chord) in self.current_progression.iter().enumerate() {
            let _ = engine.send_command(AudioCommand::StrumChord {
                notes: chord.notes.clone(),
                velocity: 0.85,
                start_samples: chord_step.saturating_mul(i as u32),
                spread_samples,
                mode: self.arpeggiator_mode as u8,
            });
        }
    }
}

pub fn render_chord_generator_modal(
    ctx: &egui::Context,
    open: &mut bool,
    state: &mut ChordGeneratorState,
    engine: &mut AudioEngine,
    status_msg: &mut String,
    mut on_insert_to_piano_roll: impl FnMut(&[GeneratedChord]),
) {
    if !*open {
        return;
    }

    let mut close = false;
    let mut trigger_insert = false;

    egui::Window::new(crate::i18n::t("🎹 Smart Ackord- & Skalgenerator (Harmony Matrix)"))
        .open(open)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(720.0, 560.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Header bar
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("MUSIKTEORI & HARMONIK")).strong().size(14.0).color(Theme::FL_ORANGE));
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::t("Klicka på ackordplattorna för att spela live • Skapa magiska ackordföljder")).size(11.0).color(Theme::TEXT_MUTED));
                });

                ui.add_space(4.0);

                // Top Controls: Root Key, Scale, Octave, Voicing
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Grundton (Key):")).strong().color(Theme::FL_CYAN));
                        for (idx, &name) in NOTE_NAMES.iter().enumerate() {
                            let is_sel = state.root_note == idx as u8;
                            let btn_col = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(32, 36, 44) };
                            let txt_col = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                            if ui.add(egui::Button::new(egui::RichText::new(name).strong().size(11.0).color(txt_col)).fill(btn_col)).clicked() {
                                state.root_note = idx as u8;
                                state.load_progression_preset(state.selected_progression_preset);
                            }
                        }
                    });

                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Skala / Tonart:")).strong().color(Theme::FL_YELLOW));
                        let scales = ScaleType::all();
                        let current_label = scales.iter().find(|(s, _)| *s == state.scale_type).map(|(_, l)| *l).unwrap_or("Moll");

                        egui::ComboBox::from_id_salt("chord_scale_picker")
                            .selected_text(current_label)
                            .width(260.0)
                            .show_ui(ui, |ui| {
                                for (st, label) in scales {
                                    if ui.selectable_label(state.scale_type == *st, *label).clicked() {
                                        state.scale_type = st.clone();
                                        state.load_progression_preset(state.selected_progression_preset);
                                    }
                                }
                            });

                        ui.separator();

                        ui.label(crate::i18n::t("Oktav:"));
                        for oct in 2..=5 {
                            if ui.selectable_label(state.octave == oct, format!("C{}", oct)).clicked() {
                                state.octave = oct;
                            }
                        }

                        ui.separator();

                        ui.label(crate::i18n::t("Voicing:"));
                        let voicings = ["Triad (3-ton)", "7th (Jazz/Pop)", "9th (Neo-Soul)", "Sus4", "Inversion 1", "Open Spread"];
                        egui::ComboBox::from_id_salt("chord_voicing_picker")
                            .selected_text(*voicings.get(state.chord_voicing).unwrap_or(&"7th"))
                            .width(130.0)
                            .show_ui(ui, |ui| {
                                for (v_idx, &v_name) in voicings.iter().enumerate() {
                                    if ui.selectable_label(state.chord_voicing == v_idx, v_name).clicked() {
                                        state.chord_voicing = v_idx;
                                        state.load_progression_preset(state.selected_progression_preset);
                                    }
                                }
                            });
                    });
                });

                ui.add_space(6.0);

                // INTERACTIVE CHORD PADS MATRIX (The core fun!)
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("🎹 INTERAKTIVA ACKORDPLATTOR (Klicka för att spela & bygga)")).strong().color(Theme::FL_GREEN));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(crate::i18n::t("🎲 Slumpa Skala")).on_hover_text(crate::i18n::t("Slumpa ny skala och grundton för inspiration")).clicked() {
                                state.root_note = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() % 12) as u8;
                                let all_scales = ScaleType::all();
                                let rand_s = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_micros() % all_scales.len() as u128) as usize;
                                state.scale_type = all_scales[rand_s].0.clone();
                                state.load_progression_preset(state.selected_progression_preset);
                            }
                        });
                    });

                    ui.add_space(6.0);

                    let pads = state.get_chord_pads();
                    let pad_w = ((ui.available_width() - 30.0) / 4.0).max(120.0);
                    let pad_h = 64.0;

                    egui::Grid::new("chord_pads_grid").spacing([8.0, 8.0]).show(ui, |ui| {
                        for (p_idx, chord) in pads.iter().enumerate() {
                            let (rect, resp) = ui.allocate_exact_size(Vec2::new(pad_w, pad_h), Sense::click());
                            let is_hovered = resp.hovered();

                            let bg_color = if resp.is_pointer_button_down_on() {
                                Theme::FL_ORANGE
                            } else if is_hovered {
                                Color32::from_rgb(45, 52, 68)
                            } else {
                                Color32::from_rgb(26, 30, 38)
                            };

                            ui.painter().rect_filled(rect, Rounding::same(6.0), bg_color);
                            ui.painter().rect_stroke(rect, Rounding::same(6.0), egui::Stroke::new(1.5_f32, chord.color));

                            // Top Roman numeral pill
                            ui.painter().text(
                                rect.min + Vec2::new(8.0, 14.0),
                                egui::Align2::LEFT_CENTER,
                                &chord.roman,
                                egui::FontId::proportional(12.0),
                                chord.color,
                            );

                            // Big Chord Name
                            ui.painter().text(
                                rect.center() + Vec2::new(0.0, -2.0),
                                egui::Align2::CENTER_CENTER,
                                &chord.name,
                                egui::FontId::proportional(15.0),
                                Color32::WHITE,
                            );

                            // Note list string
                            let note_names_str = chord.notes.iter().map(|&n| NOTE_NAMES[(n % 12) as usize]).collect::<Vec<_>>().join(" - ");
                            ui.painter().text(
                                rect.min + Vec2::new(8.0, pad_h - 12.0),
                                egui::Align2::LEFT_CENTER,
                                note_names_str,
                                egui::FontId::proportional(9.5),
                                Theme::TEXT_MUTED,
                            );

                            if resp.clicked() {
                                state.play_chord_sound(chord, engine);
                                state.current_progression.push(chord.clone());
                                if state.current_progression.len() > 8 {
                                    state.current_progression.remove(0);
                                }
                                *status_msg = crate::tstatus!("🎶 Spelade ackord: {} ({})", chord.name, chord.roman);
                            }

                            if (p_idx + 1) % 4 == 0 {
                                ui.end_row();
                            }
                        }
                    });
                });

                ui.add_space(6.0);

                // CURRENT PROGRESSION TIMELINE & PRESETS
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("🎼 AKTIV ACKORDFÖLJD (PROGRESSION)")).strong().color(Theme::FL_CYAN));
                        ui.separator();

                        let presets = ["Trap & Dark (i-iv-v-i)", "EDM Anthem (VI-IV-I-V)", "Cyberpunk (i-VI-III-VII)", "Neo-Soul (ii-V-I-vi)", "Blues Groove (i-iv-i-V)", "Cinematic (i-III-iv-v)"];
                        egui::ComboBox::from_id_salt("progression_preset_picker")
                            .selected_text(*presets.get(state.selected_progression_preset).unwrap_or(&"Standard"))
                            .width(200.0)
                            .show_ui(ui, |ui| {
                                for (pr_idx, &pr_name) in presets.iter().enumerate() {
                                    if ui.selectable_label(state.selected_progression_preset == pr_idx, pr_name).clicked() {
                                        state.load_progression_preset(pr_idx);
                                    }
                                }
                            });

                        if ui.button(crate::i18n::t("🗑 Rensa")).clicked() {
                            state.current_progression.clear();
                        }
                    });

                    ui.add_space(4.0);

                    // Horizontal Chord Sequence Cards
                    ui.horizontal(|ui| {
                        if state.current_progression.is_empty() {
                            ui.label(egui::RichText::new(crate::i18n::t("Inga ackord valda än. Klicka på plattorna ovan eller välj en färdig mall!")).color(Theme::TEXT_MUTED));
                        } else {
                            let card_w = 110.0;
                            let card_h = 44.0;
                            for (c_idx, ch) in state.current_progression.iter().enumerate() {
                                let (c_rect, c_resp) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::click());
                                ui.painter().rect_filled(c_rect, Rounding::same(4.0), Color32::from_rgb(22, 26, 34));
                                ui.painter().rect_stroke(c_rect, Rounding::same(4.0), egui::Stroke::new(1.0_f32, ch.color));

                                ui.painter().text(
                                    Pos2::new(c_rect.min.x + 8.0, c_rect.min.y + 12.0),
                                    egui::Align2::LEFT_CENTER,
                                    format!("{} {}", crate::i18n::t("Takt"), c_idx + 1),
                                    egui::FontId::proportional(9.5),
                                    Theme::TEXT_MUTED,
                                );

                                ui.painter().text(
                                    Pos2::new(c_rect.min.x + 8.0, c_rect.min.y + 28.0),
                                    egui::Align2::LEFT_CENTER,
                                    &ch.name,
                                    egui::FontId::proportional(13.0),
                                    Color32::WHITE,
                                );

                                if c_resp.clicked() {
                                    state.play_chord_sound(ch, engine);
                                }

                                if c_idx < state.current_progression.len() - 1 {
                                    ui.label(egui::RichText::new(crate::i18n::t("➔")).strong().color(Theme::FL_ORANGE));
                                }
                            }
                        }
                    });
                });

                ui.add_space(8.0);

                // Strum / Arpeggiator performance controls
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Strum-utbredning:")).strong().color(Theme::FL_CYAN));
                        ui.add(egui::Slider::new(&mut state.strum_spread_ms, 0.0..=120.0).suffix(" ms").show_value(true));
                        ui.separator();
                        ui.label(egui::RichText::new(crate::i18n::t("Arpeggio:")).strong().color(Theme::FL_YELLOW));
                        let modes = ["Block", "Upp", "Ner", "Slump"];
                        egui::ComboBox::from_id_salt("chord_arp_mode")
                            .selected_text(*modes.get(state.arpeggiator_mode).unwrap_or(&"Block"))
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                for (m_idx, &m_name) in modes.iter().enumerate() {
                                    if ui.selectable_label(state.arpeggiator_mode == m_idx, m_name).clicked() {
                                        state.arpeggiator_mode = m_idx;
                                    }
                                }
                            });
                    });
                });

                ui.add_space(8.0);

                // Bottom Action Buttons
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📥 Infoga Ackord i Piano Roll & Spår")).strong().color(Color32::BLACK)).fill(Theme::FL_GREEN).min_size(Vec2::new(220.0, 32.0))).clicked() {
                        trigger_insert = true;
                        close = true;
                    }

                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("▶ Provspela Hela Sekvensen")).strong().color(Color32::WHITE)).fill(Color32::from_rgb(45, 90, 140)).min_size(Vec2::new(180.0, 32.0))).clicked() {
                        state.audition_progression(engine);
                        *status_msg = crate::i18n::t("▶ Provspelar ackordföljd...").to_string();
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(crate::i18n::t("Stäng")).clicked() {
                            close = true;
                        }
                    });
                });
            });
        });

    if trigger_insert && !state.current_progression.is_empty() {
        on_insert_to_piano_roll(&state.current_progression);
    }

    if close {
        *open = false;
    }
}
