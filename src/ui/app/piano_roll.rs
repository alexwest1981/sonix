//! Piano roll, trummisen och skalorna.
//!
//! Rutnätet har två axlar som betyder olika saker: **rader är tonhöjd, steg är tid** —
//! och taket är det mindre av de två antalen (\`grid_capacity\`). Skalmarkeringen följer
//! projektets tonart; \`key_transpose\` ger det kortaste skiftet som sätter flest toner i skalan.
//!
//! Status: byggs — piano roll och trummisen.
//! Rör inte: en tabell och ett index för tonarten; två listor för samma sak driver isär.

use super::*;

impl SonixApp {
/// Tonerna i projektets tonart. Regeln bor i `crate::audio::scale` — här finns
/// bara projektets grundton och skala (Fas 8.11).
pub fn get_scale_notes(&self) -> Vec<u8> {
    crate::audio::scale::scale_notes(self.song_key_root, self.song_key_scale)
}
}

impl SonixApp {
pub fn humanize_active_pattern(&mut self) {
    let mut rng_state: u32 = 987654321;
    for v in self.step_velocities.iter_mut() {
        rng_state ^= rng_state << 13;
        rng_state ^= rng_state >> 17;
        rng_state ^= rng_state << 5;
        let delta = (rng_state as f32 / u32::MAX as f32) * 0.3 - 0.15;
        *v = (*v + delta).clamp(0.2, 1.0);
    }
    self.status_message = crate::i18n::t("🎲 Humanize: Anslagsdynamik och sväng varierat!").to_string();
}
}

impl SonixApp {
pub fn transpose_active_pattern(&mut self, semitones: i8) {
    let mut new_grid = [[false; 16]; 24];
    for offset in 0..24 {
        for step in 0..16 {
            if self.piano_roll_grid[offset][step] {
                let target = offset as i8 + semitones;
                if target >= 0 && target < 24 {
                    new_grid[target as usize][step] = true;
                }
            }
        }
    }
    self.piano_roll_grid = new_grid;
    for step in 0..16 {
        for offset in 0..24 {
            if self.piano_roll_grid[offset][step] {
                self.channels[6].notes[step] = 48 + offset as u8;
                break;
            }
        }
    }
    self.sync_active_pattern_from_ui();
    self.status_message = crate::tstatus!("⬆️ Transponerade mönster {} halvtoner", semitones);
}
}

impl SonixApp {
pub fn arpeggiate_pattern(&mut self) {
    let base_scale = self.get_scale_notes();
    let scale_len = base_scale.len().max(1);
    self.piano_roll_grid = [[false; 16]; 24];
    for step in 0..16 {
        let note_in_scale = base_scale[step % scale_len];
        let octave = (step / 4) % 2;
        let note_offset = (note_in_scale as usize + octave * 12).min(23);
        self.piano_roll_grid[note_offset][step] = true;
        self.channels[6].steps[step] = true;
        self.channels[6].notes[step] = 48 + note_offset as u8;
    }
    self.sync_active_pattern_from_ui();
    self.status_message = crate::i18n::t("⚡ Arpeggiator: Skapade melodiskt arpeggio!").to_string();
}
}

impl SonixApp {
pub fn reverse_pattern(&mut self) {
    for row in self.piano_roll_grid.iter_mut() {
        row.reverse();
    }
    self.step_velocities.reverse();
    self.channels[6].steps.reverse();
    self.channels[6].notes.reverse();
    self.sync_active_pattern_from_ui();
    self.status_message = crate::i18n::t("🔁 Vände mönster baklänges (Reverse)").to_string();
}
}

impl SonixApp {
pub(crate) fn render_piano_roll_editor(&mut self, ui: &mut egui::Ui) {
    ui.group(|ui| {
        // ROW 1: Pattern Selector, Root Key, Scale, Snap Toggle
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎹 SONIX PIANO ROLL")).strong().size(13.0).color(Theme::FL_GREEN));
            ui.separator();

            ui.label(egui::RichText::new(crate::i18n::t("Mönster:")).size(11.0).color(Theme::TEXT_MUTED));
            let p_len = self.patterns.len();
            for pat_idx in 0..p_len {
                let is_active = self.selected_pattern == pat_idx;
                let p_color = self.patterns[pat_idx].color;
                let fill = if is_active { p_color } else { Theme::PANEL_BG };
                let text_color = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                let btn_text = format!("{} {}", pat_idx + 1, self.patterns[pat_idx].name);
                if ui.add(egui::Button::new(egui::RichText::new(btn_text).strong().size(11.0).color(text_color)).fill(fill)).clicked() {
                    self.select_pattern(pat_idx);
                }
            }

            ui.separator();

            // Root Key Picker
            ui.label(egui::RichText::new(crate::i18n::t("Grundton:")).size(11.0).color(Theme::TEXT_MUTED));
            let cur_root = crate::audio::scale::root_name(self.song_key_root);
            egui::ComboBox::from_id_salt("pr_root_combo")
                .selected_text(cur_root)
                .width(45.0)
                .show_ui(ui, |ui| {
                    for (idx, &r_name) in crate::audio::scale::ROOT_NAMES.iter().enumerate() {
                        if ui.selectable_label(self.song_key_root as usize == idx, r_name).clicked() {
                            self.song_key_root = idx as u8;
                        }
                    }
                });

            // Scale Snapping Selector
            ui.label(egui::RichText::new(crate::i18n::t("Skala:")).size(11.0).color(Theme::TEXT_MUTED));
            let cur_scale = crate::i18n::t(crate::audio::scale::scale_at(self.song_key_scale).name);
            egui::ComboBox::from_id_salt("pr_scale_combo")
                .selected_text(cur_scale)
                .width(120.0)
                .show_ui(ui, |ui| {
                    for (s_idx, sc) in crate::audio::scale::SCALES.iter().enumerate() {
                        if ui.selectable_label(self.song_key_scale == s_idx, crate::i18n::t(sc.name)).clicked() {
                            self.song_key_scale = s_idx;
                        }
                    }
                });

            let snap_bg = if self.piano_roll_snap_to_scale { Theme::FL_CYAN } else { Color32::from_rgb(32, 38, 48) };
            let snap_fg = if self.piano_roll_snap_to_scale { Color32::BLACK } else { Theme::TEXT_BRIGHT };
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔒 Skal-lås")).strong().size(10.5).color(snap_fg)).fill(snap_bg)).on_hover_text(crate::i18n::t("Lås tangenter till vald skala")).clicked() {
                self.piano_roll_snap_to_scale = !self.piano_roll_snap_to_scale;
            }

            // Tagningen (Fas 6.4): kvantisera eller humanisera det som spelades in.
            let take_notes = self
                .patterns
                .get(self.selected_pattern)
                .map(|p| p.take.len())
                .unwrap_or(0);
            let take_tightness = self
                .patterns
                .get(self.selected_pattern)
                .map(|p| p.take.tightness())
                .unwrap_or(0.0);
            // Spannet och hur många noter som faktiskt ligger utanför rutnätet:
            // "4 med tajming" säger att humaniseringen hörs, inte bara mäts.
            let (take_first, take_last, take_moved) = self
                .patterns
                .get(self.selected_pattern)
                .map(|p| {
                    let first = p.take.notes.iter().map(crate::midi_take::Take::slot).min();
                    let last = p.take.notes.iter().map(crate::midi_take::Take::slot).max();
                    let moved = p
                        .take
                        .notes
                        .iter()
                        .filter(|n| crate::midi_take::Take::playback_slot(n).1 > 0.001)
                        .count();
                    (first, last, moved)
                })
                .unwrap_or((None, None, 0));
            let take_span = match (take_first, take_last) {
                (Some(a), Some(b)) => crate::tstatus!("steg {}–{}", a, b),
                _ => crate::i18n::t("tom").to_string(),
            };
            ui.separator();
            ui.label(
                egui::RichText::new(crate::tstatus!(
                    "Tagning: {} noter ({}, {} med tajming) · {:.2} steg otajt",
                    take_notes,
                    take_span,
                    take_moved,
                    take_tightness
                ))
                .size(10.5)
                .color(if take_notes == 0 { Theme::TEXT_MUTED } else { Theme::TEXT_BRIGHT }),
            );
            ui.label(egui::RichText::new(crate::i18n::t("Rutnät:")).size(11.0).color(Theme::TEXT_MUTED));
            let cur_grid = crate::midi_take::TakeGrid::from_index(self.take_grid_idx);
            egui::ComboBox::from_id_salt("take_grid_combo")
                .selected_text(cur_grid.label())
                .width(74.0)
                .show_ui(ui, |ui| {
                    for grid in crate::midi_take::TakeGrid::ALL {
                        if ui.selectable_label(grid == cur_grid, grid.label()).clicked() {
                            self.take_grid_idx = grid.index();
                        }
                    }
                });
            ui.label(egui::RichText::new(crate::i18n::t("Styrka")).size(11.0).color(Theme::TEXT_MUTED));
            ui.add(
                egui::Slider::new(&mut self.take_strength, 0.0..=1.0)
                    .show_value(false)
                    .fixed_decimals(2),
            )
            .on_hover_text(crate::i18n::t("Hur hårt noterna dras mot rutnätet (0 % = oförändrat, 100 % = exakt på rutnätet)"));

            let take_btn = Color32::from_rgb(32, 38, 48);
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎯 Kvantisera")).strong().size(10.5).color(Theme::TEXT_BRIGHT)).fill(take_btn)).on_hover_text(crate::i18n::t("Dra tagningens noter till närmaste rutnätslinje, med projektets sväng")).clicked() {
                let grid = crate::midi_take::TakeGrid::from_index(self.take_grid_idx);
                let strength = self.take_strength;
                self.quantize_take(strength, grid);
            }
            ui.label(egui::RichText::new(crate::i18n::t("Humanisering")).size(11.0).color(Theme::TEXT_MUTED));
            ui.add(
                egui::Slider::new(&mut self.take_humanize, 0.0..=1.0)
                    .show_value(false)
                    .fixed_decimals(2),
            )
            .on_hover_text(crate::i18n::t("Hur mycket mänsklig otajthet och dynamik som läggs på"));
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🌀 Humanisera")).strong().size(10.5).color(Theme::TEXT_BRIGHT)).fill(take_btn)).on_hover_text(crate::i18n::t("Lägg på mänsklig otajthet och dynamik (varierar mellan trycken)")).clicked() {
                let amount = self.take_humanize;
                self.humanize_take(amount);
            }

            ui.separator();

            // Chord Stamp Selector
            ui.label(egui::RichText::new(crate::i18n::t("Ackord:")).size(11.0).color(Theme::TEXT_MUTED));
            let chords = ["Enkel", "Dur", "Moll", "7th", "Sus4", "Oktav"];
            let cur_chord = chords.get(self.chord_stamp).unwrap_or(&"Enkel");
            egui::ComboBox::from_id_salt("pr_chord_combo")
                .selected_text(*cur_chord)
                .width(60.0)
                .show_ui(ui, |ui| {
                    for (c_idx, &c_name) in chords.iter().enumerate() {
                        if ui.selectable_label(self.chord_stamp == c_idx, c_name).clicked() {
                            self.chord_stamp = c_idx;
                        }
                    }
                });
        });

        ui.add_space(4.0);

        // ROW 2: MIDI Transformation Tools (Humanize, Arp, Transpose, Invert, Clear)
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("VERKTYG:")).strong().size(10.5).color(Theme::TEXT_MUTED));

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎲 Humanize")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(180, 80, 20))).on_hover_text(crate::i18n::t("Variera anslagsdynamik (velocity) för levande sväng")).clicked() {
                self.humanize_active_pattern();
            }

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⚡ Arpeggio")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(30, 110, 160))).on_hover_text(crate::i18n::t("Skapa automatiskt arpeggiomönster från vald skala")).clicked() {
                self.arpeggiate_pattern();
            }

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⬆ +Okt")).size(10.5)).fill(Color32::from_rgb(38, 45, 56))).on_hover_text(crate::i18n::t("Transponera upp 1 oktav (+12 halvtoner)")).clicked() {
                self.transpose_active_pattern(12);
            }

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⬇ -Okt")).size(10.5)).fill(Color32::from_rgb(38, 45, 56))).on_hover_text(crate::i18n::t("Transponera ner 1 oktav (-12 halvtoner)")).clicked() {
                self.transpose_active_pattern(-12);
            }

            // **Till tonarten** (Fas 8.11): flyttar hela mönstret till det skift som sätter
            // flest toner i projektets skala. Regeln räknar bara ut *skiftet*; själva
            // flytten går genom `transpose_active_pattern` — samma provade väg som
            // oktavknapparna, alltså en väg och inte två.
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎵 Till tonarten")).size(10.5)).fill(Color32::from_rgb(38, 45, 56))).on_hover_text(crate::i18n::t("Flytta hela mönstret till tonarten — det kortaste skiftet som sätter flest toner i skalan")).clicked() {
                let rows: Vec<usize> = (0..crate::audio::scale::PIANO_ROLL_ROWS)
                    .filter(|&o| self.piano_roll_grid[o].iter().any(|&b| b))
                    .collect();
                let shift = crate::audio::scale::key_transpose(
                    &rows,
                    crate::audio::scale::PIANO_ROLL_BASE_MIDI,
                    self.song_key_root,
                    self.song_key_scale,
                );
                if shift == 0 {
                    self.status_message =
                        crate::i18n::t("🎵 Mönstret står redan i tonarten").to_string();
                } else {
                    self.transpose_active_pattern(shift);
                    self.status_message = crate::tstatus!(
                        "🎵 {} halvtoner till {}",
                        shift,
                        crate::audio::scale::key_label(self.song_key_root, self.song_key_scale)
                    );
                }
            }

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁 Backa")).size(10.5)).fill(Color32::from_rgb(38, 45, 56))).on_hover_text(crate::i18n::t("Vänd tonföljden baklänges")).clicked() {
                self.reverse_pattern();
            }

            let poly_bg = if self.piano_roll_poly_mode { Theme::FL_ORANGE } else { Color32::from_rgb(32, 38, 48) };
            let poly_fg = if self.piano_roll_poly_mode { Color32::BLACK } else { Theme::TEXT_BRIGHT };
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎹 Polyfoni")).strong().size(10.5).color(poly_fg)).fill(poly_bg)).on_hover_text(crate::i18n::t("Tillåt flera toner samtidigt i samma steg (ackordmålning)")).clicked() {
                self.piano_roll_poly_mode = !self.piano_roll_poly_mode;
            }

            // Real MIDI keyboard input indicator + record arm (Fas 5.3)
            let midi_col = if self.midi_keyboard_connected { Theme::FL_GREEN } else { Theme::TEXT_MUTED };
            ui.label(egui::RichText::new(if self.midi_keyboard_connected { "🎹 MIDI ●" } else { "🎹 MIDI ○" }).strong().size(10.0).color(midi_col))
                .on_hover_text(if self.midi_keyboard_connected { crate::i18n::t("MIDI-klaviatur ansluten (ALSA Seq).") } else { crate::i18n::t("Ingen MIDI-klaviatur ansluten. Öppna Hårdvarukontroller för att koppla upp.") });
            let mrec_bg = if self.midi_record_armed { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(38, 45, 56) };
            let mrec_fg = if self.midi_record_armed { Color32::WHITE } else { Theme::TEXT_BRIGHT };
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⏺ MIDI-REC")).strong().size(10.0).color(mrec_fg)).fill(mrec_bg)).on_hover_text(crate::i18n::t("Spela in hållna MIDI-toner i rutnätet under uppspelning.")).clicked() {
                self.midi_record_armed = !self.midi_record_armed;
                if self.midi_record_armed {
                    self.begin_new_take();
                }
            }

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🧹 Töm")).size(10.5).color(Color32::from_rgb(255, 120, 120))).fill(Color32::from_rgb(50, 20, 25))).on_hover_text(crate::i18n::t("Rensa mönster")).clicked() {
                self.piano_roll_grid = [[false; 16]; 24];
                self.channels[6].steps = [false; 16];
                self.sync_active_pattern_from_ui();
                self.status_message = crate::i18n::t("🧹 Pianorullens mönster rensat!").to_string();
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(crate::i18n::t("🎼 Arranger")).clicked() {
                    self.view_mode = ViewMode::PlaylistArranger;
                }
                if ui.button(crate::i18n::t("◀ Channel Rack")).clicked() {
                    self.view_mode = ViewMode::ChannelRack;
                }
            });
        });

        ui.add_space(6.0);

        let base_midi = crate::audio::scale::PIANO_ROLL_BASE_MIDI; // C3
        let semitones_count = 24;
        let key_root = self.song_key_root;
        let key_scale = self.song_key_scale;
        let root_note_val = key_root % 12;

        egui::ScrollArea::vertical().max_height(250.0).show(ui, |ui| {
            for note_offset in (0..semitones_count).rev() {
                let midi_note = base_midi + note_offset as u8;
                let note_val = midi_note % 12;
                let note_in_scale = crate::audio::scale::in_scale(note_val, key_root, key_scale);
                let is_root = note_val == root_note_val;
                let is_sharp = [1, 3, 6, 8, 10].contains(&note_val);
                let n_name = note_name(midi_note);
                let octave_num = (midi_note / 12) as i8 - 1;

                ui.horizontal(|ui| {
                    // Piano Key label on left with Root and Scale Highlighting
                    let key_bg = if is_root {
                        Color32::from_rgb(180, 110, 20) // Amber for root note
                    } else if !note_in_scale {
                        Color32::from_rgb(18, 20, 26)
                    } else if is_sharp {
                        Color32::from_rgb(32, 42, 58)
                    } else {
                        Color32::from_rgb(220, 228, 240)
                    };
                    let key_text = if is_root || is_sharp || !note_in_scale { Color32::WHITE } else { Color32::BLACK };
                    let (k_rect, _) = ui.allocate_exact_size(Vec2::new(52.0, 16.0), Sense::hover());
                    ui.painter().rect_filled(k_rect, Rounding::same(2.0), key_bg);

                    let label_str = if is_root {
                        format!("✦ {}{}", n_name, octave_num)
                    } else {
                        format!("{}{}", n_name, octave_num)
                    };
                    ui.painter().text(
                        k_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        label_str,
                        egui::FontId::proportional(10.0),
                        key_text,
                    );

                    // 16 Grid Columns
                    for step in 0..16 {
                        let (cell_rect, response) = ui.allocate_exact_size(Vec2::new(26.0, 16.0), Sense::click());
                        let is_on = self.piano_roll_grid[note_offset][step];
                        let is_playhead = self.is_playing && self.current_step == step;
                        let is_beat_start = step % 4 == 0;

                        if response.clicked() {
                            if is_on {
                                self.piano_roll_grid[note_offset][step] = false;
                                let any_left = (0..24).any(|o| self.piano_roll_grid[o][step]);
                                self.channels[6].steps[step] = any_left;
                            } else {
                                // Skal-låset (Fas 8.11). Knappen fanns förut men
                                // lästes aldrig — den gjorde ingenting. Nu flyttas
                                // en klickad rad utanför skalan till **närmaste** rad
                                // som är i den, och markeringen i rutnätet visar
                                // vilken rad det blev. Att tysta klicket vore att
                                // låtsas att tangenten fanns.
                                let row = if self.piano_roll_snap_to_scale {
                                    crate::audio::scale::snap_row(
                                        note_offset,
                                        base_midi,
                                        semitones_count,
                                        self.song_key_root,
                                        self.song_key_scale,
                                    )
                                } else {
                                    note_offset
                                };
                                let row_midi = base_midi + row as u8;
                                if !self.piano_roll_poly_mode {
                                    for o in 0..24 {
                                        self.piano_roll_grid[o][step] = false;
                                    }
                                }

                                // Check chord stamp
                                let stamp_offsets: &[usize] = match self.chord_stamp {
                                    1 => &[0, 4, 7],      // Major
                                    2 => &[0, 3, 7],      // Minor
                                    3 => &[0, 4, 7, 10],  // 7th
                                    4 => &[0, 5, 7],      // Sus4
                                    5 => &[0, 12],        // Octave
                                    _ => &[0],            // Single
                                };

                                for &c_off in stamp_offsets {
                                    let target_off = row + c_off;
                                    if target_off < 24 {
                                        self.piano_roll_grid[target_off][step] = true;
                                    }
                                }
                                self.channels[6].steps[step] = true;
                                self.channels[6].notes[step] = row_midi;
                            }
                            self.sync_active_pattern_from_ui();
                        }

                        let mut bg = if is_on {
                            Theme::FL_GREEN
                        } else if is_root {
                            Color32::from_rgb(36, 32, 22)
                        } else if !note_in_scale {
                            Color32::from_rgb(14, 16, 22)
                        } else if is_sharp {
                            Color32::from_rgb(24, 28, 36)
                        } else if is_beat_start {
                            Color32::from_rgb(38, 44, 56)
                        } else {
                            Color32::from_rgb(28, 32, 42)
                        };

                        if is_playhead {
                            bg = Color32::from_rgb(255, 230, 100);
                        }

                        ui.painter().rect_filled(cell_rect, Rounding::same(1.5), bg);
                        ui.painter().rect_stroke(cell_rect, Rounding::same(1.5), Stroke::new(0.5_f32, Color32::from_rgb(45, 52, 65)));
                    }
                });
            }
        });

        ui.add_space(4.0);

        // Velocity Editor Lane (FL Studio Note Velocity Stalks)
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("VELOCITY")).size(10.0).color(Theme::TEXT_MUTED));
            ui.allocate_exact_size(Vec2::new(10.0, 24.0), Sense::hover());
            for s in 0..16 {
                let (v_rect, v_resp) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::click_and_drag());
                if v_resp.dragged() {
                    let dy = v_resp.drag_delta().y;
                    self.step_velocities[s] = (self.step_velocities[s] - dy * 0.05).clamp(0.1, 1.0);
                }
                let v = self.step_velocities[s];
                let bar_h = v * v_rect.height();
                let bar_rect = Rect::from_min_max(Pos2::new(v_rect.center().x - 2.5, v_rect.max.y - bar_h), Pos2::new(v_rect.center().x + 2.5, v_rect.max.y));
                ui.painter().rect_filled(v_rect, Rounding::same(1.0), Color32::from_rgb(18, 22, 28));
                ui.painter().rect_filled(bar_rect, Rounding::same(1.0), Theme::FL_CYAN);
            }
        });
    });
}
}

impl SonixApp {
pub fn apply_drummer_generation(&mut self) {
    let comp = self.drummer_complexity;
    let loud = self.drummer_loudness;
    let ksv = self.drummer_kick_snare_var;
    let hv = self.drummer_hihat_var;
    let fills = self.drummer_fills.clamp(0.0, 1.0);

    let mut kick = [false; 16];
    let mut snare = [false; 16];
    let mut clap = [false; 16];
    let mut hat_c = [false; 16];
    let mut hat_o = [false; 16];
    let mut crash = [false; 16];

    // 1. Kick Pattern (variation controlled by drummer_kick_snare_var)
    kick[0] = true;
    match ksv {
        0 => {
            if comp >= 0.3 {
                kick[8] = true;
            }
        }
        1 => {
            kick[8] = true;
            if comp > 0.4 {
                kick[6] = true;
            }
            if comp > 0.6 {
                kick[14] = true;
            }
        }
        2 => {
            kick[4] = true;
            kick[8] = true;
            kick[12] = true;
            if comp > 0.5 {
                kick[10] = true;
            }
        }
        _ => {
            kick[3] = true;
            kick[6] = true;
            kick[8] = true;
            if comp > 0.4 {
                kick[10] = true;
            }
            if comp > 0.6 {
                kick[14] = true;
            }
        }
    }
    // Fills add extra kick hits at the end of the bar.
    if fills > 0.5 {
        kick[15] = true;
    }
    if fills > 0.75 {
        kick[13] = true;
    }

    // 2. Snare / Clap (variation controlled by drummer_kick_snare_var)
    if self.drummer_profile == 3 {
        // Trap / Hip Hop uses claps on 4 and 12
        clap[4] = true;
        clap[12] = true;
        if comp > 0.6 {
            clap[7] = true;
            clap[15] = true;
        }
    } else {
        snare[4] = true;
        snare[12] = true;
        match ksv {
            0 => {}
            1 => {
                if comp > 0.5 {
                    snare[15] = true;
                }
            }
            2 => {
                if comp > 0.5 {
                    snare[7] = true;
                }
                if comp > 0.7 {
                    snare[11] = true;
                }
            }
            _ => {
                snare[7] = true;
                snare[11] = true;
                snare[15] = true;
            }
        }
        if self.drummer_percussion_on && comp > 0.4 {
            clap[12] = true;
        }
    }

    // 3. Hi-Hats (style controlled by drummer_hihat_var)
    match hv {
        0 => {
            for step in (0..16).step_by(2) {
                hat_c[step] = true;
            }
        }
        1 => {
            for step in 0..16 {
                if step % 4 == 2 {
                    hat_o[step] = true;
                } else {
                    hat_c[step] = true;
                }
            }
        }
        2 => {
            for step in 0..16 {
                if step % 2 == 0 {
                    hat_c[step] = true;
                } else if comp > 0.4 {
                    hat_c[step] = true;
                }
            }
        }
        _ => {
            for step in 0..16 {
                hat_c[step] = true;
            }
            if comp > 0.5 {
                hat_o[14] = true;
            }
        }
    }

    // 4. Crash Cymbal
    if self.drummer_cymbals_on {
        crash[0] = true;
        if comp > 0.75 {
            crash[14] = true;
        }
    }

    // 5. Toms – a fill in the final beat, only when enabled.
    let mut toms = [false; 16];
    if self.drummer_toms_on {
        toms[12] = true;
        toms[13] = true;
        toms[14] = true;
        if fills > 0.5 {
            toms[11] = true;
        }
        if fills > 0.75 {
            toms[15] = true;
        }
        self.drummer_tom_high = comp > 0.5;
    }
    self.drummer_tom_steps = toms;

    self.channels[0].steps = kick;
    self.channels[1].steps = snare;
    self.channels[2].steps = clap;
    self.channels[3].steps = hat_c;
    self.channels[4].steps = hat_o;
    self.channels[5].steps = crash;

    let vel_mult = 0.5 + loud * 0.5;
    self.step_velocities.fill(vel_mult);

    self.sync_active_pattern_from_ui();
    self.status_message = crate::tstatus!("🥁 Drummer genererade mönster (Komplexitet: {:.0}%, Volym: {:.0}%)", comp * 100.0, loud * 100.0);
}
}

impl SonixApp {
pub(crate) fn render_session_drummer(&mut self, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🥁 SONIX SESSION DRUMMER")).strong().size(14.0).color(Theme::FL_ORANGE));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Intelligent trummis med 2D XY-radar för realtids-generering av trumspår")).size(11.0).color(Theme::TEXT_MUTED));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⚡ Skriv till Mönster")).strong().color(Color32::BLACK)).fill(Theme::FL_YELLOW)).clicked() {
                    self.apply_drummer_generation();
                }
            });
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            // Left Column: Drummer Profiles & Beat Presets
            ui.group(|ui| {
                ui.set_width(220.0);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("DRUMMERS (STILAR)")).strong().size(11.0).color(Theme::FL_CYAN));
                    let drummers = [
                        ("Kyle", "Pop Rock (Punchy)", "🎸"),
                        ("Logan", "Retro Synthwave", "🕹"),
                        ("Anders", "Electronic / House", "⚡"),
                        ("Jesse", "Hip Hop / 808 Trap", "🎤"),
                        ("Max", "Punk & Hard Rock", "🔥"),
                        ("Ian", "Funk & Neo-Soul", "🎷"),
                    ];

                    for (idx, (name, desc, ico)) in drummers.iter().enumerate() {
                        let is_sel = self.drummer_profile == idx;
                        let bg = if is_sel { Color32::from_rgb(40, 55, 75) } else { Theme::PANEL_BG };
                        let stroke = if is_sel { Stroke::new(1.0_f32, Theme::FL_CYAN) } else { Stroke::NONE };

                        egui::Frame::none().fill(bg).stroke(stroke).rounding(Rounding::same(4.0)).inner_margin(4.0).show(ui, |ui| {
                            if ui.selectable_label(is_sel, format!("{} {} - {}", ico, name, desc)).clicked() {
                                self.drummer_profile = idx;
                                self.apply_drummer_generation();
                            }
                        });
                    }

                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::t("BEAT PRESETS")).strong().size(11.0).color(Theme::FL_ORANGE));
                    let presets = [
                        "Golden State (Standard 4/4)",
                        "Half-Pipe (Skate Punk)",
                        "Mixtape 808 (Trap Rolls)",
                        "Synth City (Driving 16ths)",
                        "Neon Drive (Outrun)",
                        "Slow Jam R&B",
                    ];
                    for (p_idx, p_name) in presets.iter().enumerate() {
                        let is_p = self.drummer_preset == p_idx;
                        if ui.selectable_label(is_p, *p_name).clicked() {
                            self.drummer_preset = p_idx;
                            match p_idx {
                                0 => { self.drummer_complexity = 0.35; self.drummer_loudness = 0.8; }
                                1 => { self.drummer_complexity = 0.85; self.drummer_loudness = 0.95; }
                                2 => { self.drummer_complexity = 0.70; self.drummer_loudness = 0.9; }
                                3 => { self.drummer_complexity = 0.50; self.drummer_loudness = 0.85; }
                                4 => { self.drummer_complexity = 0.60; self.drummer_loudness = 0.88; }
                                _ => { self.drummer_complexity = 0.25; self.drummer_loudness = 0.65; }
                            }
                            self.apply_drummer_generation();
                        }
                    }
                });
            });

            ui.separator();

            // Center Column: Interactive 2D XY Matrix Radar Pad
            ui.group(|ui| {
                ui.set_width(320.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("XY PERFORMANCE RADAR")).strong().size(11.0).color(Theme::FL_YELLOW));
                    ui.label(egui::RichText::new(crate::i18n::t("Dra den gula pucken för att ändra dynamik & komplexitet i realtid")).size(10.0).color(Theme::TEXT_MUTED));
                    ui.add_space(4.0);

                    if drummer_xy_matrix(ui, &mut self.drummer_complexity, &mut self.drummer_loudness, Vec2::new(300.0, 260.0)) {
                        self.apply_drummer_generation();
                    }

                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("{} {:.0}%  •  {} {:.0}%", crate::i18n::t("Komplexitet:"), self.drummer_complexity * 100.0, crate::i18n::t("Ljudstyrka:"), self.drummer_loudness * 100.0)).size(11.0).color(Theme::FL_YELLOW));
                });
            });

            ui.separator();

            // Right Column: Drum Kit Parts & Fine Controls
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("TRUMSET & INSTRUMENT")).strong().size(11.0).color(Theme::FL_ORANGE));
                    ui.add_space(4.0);

                    if ui.checkbox(&mut self.drummer_percussion_on, "👏 Percussion & Handclaps").changed() {
                        self.apply_drummer_generation();
                    }
                    if ui.checkbox(&mut self.drummer_cymbals_on, "✨ Crash Cymbals & Accents").changed() {
                        self.apply_drummer_generation();
                    }
                    if ui.checkbox(&mut self.drummer_toms_on, "🔊 Acoustic & Electronic Toms").changed() {
                        self.apply_drummer_generation();
                    }

                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Kick & Snare:")).size(10.0).color(Theme::TEXT_MUTED));
                        for v in 1..=4 {
                            if ui.selectable_label(self.drummer_kick_snare_var == v, format!("{}", v)).clicked() {
                                self.drummer_kick_snare_var = v;
                                self.apply_drummer_generation();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Hi-Hat Rytm:")).size(10.0).color(Theme::TEXT_MUTED));
                        for v in 1..=4 {
                            if ui.selectable_label(self.drummer_hihat_var == v, format!("{}", v)).clicked() {
                                self.drummer_hihat_var = v;
                                self.apply_drummer_generation();
                            }
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::t("FILLS & GROOVE")).strong().size(11.0).color(Theme::FL_CYAN));

                    ui.horizontal(|ui| {
                        let mut ch = false;
                        ch |= rotary_knob(ui, &mut self.drummer_fills, 0.0, 1.0, "FILLS", Theme::FL_CYAN, 30.0);
                        ch |= rotary_knob(ui, &mut self.swing, 0.0, 1.0, "SWING", Theme::FL_YELLOW, 30.0);
                        if ch {
                            self.apply_drummer_generation();
                        }
                    });

                    ui.add_space(12.0);
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("💡 TIPS")).strong().size(10.0).color(Theme::FL_GREEN));
                        ui.label(egui::RichText::new(crate::i18n::t("Alla förändringar i Session Drummer speglas direkt i FL Channel Rack och tidslinjen!")).size(9.5).color(Theme::TEXT_MUTED));
                    });
                });
            });
        });
    });
}
}

impl SonixApp {
pub(crate) fn render_touch_piano_keyboard(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎹 SONIX PIANO")).strong().size(13.0).color(Theme::TEXT_BRIGHT));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Spela med tangentbordet [A, S, D...] eller klicka med musen")).size(11.0).color(Theme::TEXT_MUTED));
        });

        ui.add_space(6.0);

        let key_width = 38.0;
        let white_key_height = 100.0;
        let black_key_height = 64.0;
        let black_key_width = 24.0;

        let start_note = (self.octave + 1) as u8 * 12;

        let (keyboard_rect, _) = ui.allocate_exact_size(
            Vec2::new(key_width * 14.0, white_key_height),
            Sense::hover(),
        );

        let white_offsets = [0, 2, 4, 5, 7, 9, 11, 12, 14, 16, 17, 19, 21, 23];
        let black_defs = [
            (1, 0), (3, 1), (6, 3), (8, 4), (10, 5),
            (13, 7), (15, 8), (18, 10), (20, 11), (22, 12),
        ];

        let mut hovered_note = None;
        let pointer_pos = ctx.input(|i| i.pointer.hover_pos());
        let pointer_down = ctx.input(|i| i.pointer.primary_down());

        // 1. White Keys
        for (i, &semi) in white_offsets.iter().enumerate() {
            let note = start_note + semi as u8;
            let key_rect = Rect::from_min_size(
                Pos2::new(keyboard_rect.min.x + i as f32 * key_width, keyboard_rect.min.y),
                Vec2::new(key_width - 2.0, white_key_height),
            );

            let is_active = self.active_keys.contains(&note);
            let bg_color = if is_active {
                Theme::FL_CYAN
            } else {
                Color32::from_rgb(240, 243, 250)
            };

            ui.painter().rect_filled(key_rect, Rounding::same(3.0), bg_color);
            ui.painter().rect_stroke(key_rect, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(170, 178, 190)));

            ui.painter().text(
                Pos2::new(key_rect.center().x, key_rect.max.y - 12.0),
                egui::Align2::CENTER_CENTER,
                format!("{}{}", note_name(note), (note / 12) as i8 - 1),
                egui::FontId::proportional(10.0),
                Color32::from_rgb(40, 45, 55),
            );

            if let Some(pos) = pointer_pos
                && key_rect.contains(pos) {
                    hovered_note = Some(note);
                }
        }

        // 2. Black Keys
        for &(semi, white_idx) in &black_defs {
            let note = start_note + semi as u8;
            let x_pos = keyboard_rect.min.x + (white_idx as f32 + 0.68) * key_width;
            let key_rect = Rect::from_min_size(
                Pos2::new(x_pos, keyboard_rect.min.y),
                Vec2::new(black_key_width, black_key_height),
            );

            let is_active = self.active_keys.contains(&note);
            let bg_color = if is_active {
                Theme::FL_ORANGE
            } else {
                Color32::from_rgb(28, 32, 40)
            };

            ui.painter().rect_filled(key_rect, Rounding::same(2.0), bg_color);
            ui.painter().rect_stroke(key_rect, Rounding::same(2.0), Stroke::new(1.0_f32, Color32::BLACK));

            if let Some(pos) = pointer_pos
                && key_rect.contains(pos) {
                    hovered_note = Some(note);
                }
        }

        // Mouse Touch Interaction
        if pointer_down {
            if let Some(note) = hovered_note
                && self.active_mouse_note != Some(note) {
                    if let Some(old) = self.active_mouse_note {
                        self.release_note(old);
                    }
                    self.active_mouse_note = Some(note);
                    self.play_note(note);
                }
        } else if let Some(old) = self.active_mouse_note {
            self.release_note(old);
            self.active_mouse_note = None;
        }
    });
}
}

