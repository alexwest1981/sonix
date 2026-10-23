//! Piano roll, trummisen och skalorna.
//!
//! Rutnätet har två axlar som betyder olika saker: **rader är tonhöjd, steg är tid** —
//! och taket är det mindre av de två antalen (\`grid_capacity\`). Skalmarkeringen följer
//! projektets tonart; \`key_transpose\` ger det kortaste skiftet som sätter flest toner i skalan.
//!
//! Status: byggs — piano roll och trummisen.
//! Rör inte: en tabell och ett index för tonarten; två listor för samma sak driver isär.

use super::*;
use crate::ui::theme::Theme;

impl SonixApp {
/// Tonerna i projektets tonart. Regeln bor i `crate::audio::scale` — här finns
/// bara projektets grundton och skala (Fas 8.11).
pub fn get_scale_notes(&self) -> Vec<u8> {
    crate::audio::scale::scale_notes(self.song_key_root, self.song_key_scale)
}
}

impl SonixApp {
/// Snappar rutnätet (urvalet om något är markerat) mot ackordet i den aktuella takten.
pub(crate) fn snap_to_song_chord(&mut self) {
    let bar = self.song_bar;
    let Some(chord) = crate::ui::app::state::chord_at(&self.song_chords, bar) else {
        self.status_message = crate::tstatus!(
            "🎼 Inget ackord i takt {} — lägg till ett i ackordspåret",
            bar + 1
        );
        return;
    };
    let moved = snap_grid_to_chord(&mut self.piano_roll_grid, &self.piano_roll_sel, &chord);
    self.status_message = crate::tstatus!(
        "🎹 {} not(er) snappade till {} i takt {}",
        moved,
        chord.name(),
        bar + 1
    );
    if moved > 0 {
        self.resync_grid_channels();
    }
}

/// Kör regeln i UI:t. Urvalet gäller om något är markerat, annars hela rutnätet.
pub(crate) fn apply_current_rule(&mut self) {
    let field = RuleField::ALL[self.rule_field.min(RuleField::ALL.len() - 1)];
    let cmp = RuleCmp::ALL[self.rule_cmp.min(RuleCmp::ALL.len() - 1)];
    let action = match self.rule_action {
        1 => RuleAction::Transpose(self.rule_action_value.round() as i32),
        2 => RuleAction::SetVelocity(self.rule_action_value.clamp(0.0, 1.0)),
        _ => RuleAction::Delete,
    };
    let rule = Rule { field, cmp, value: self.rule_value, action };
    let scope = if count_selected(&self.piano_roll_sel) > 0 { "urvalet" } else { "hela rutnätet" };
    let result = apply_rule(
        &mut self.piano_roll_grid,
        &self.piano_roll_sel,
        &mut self.step_velocities,
        &rule,
    );
    self.status_message = crate::tstatus!(
        "⚙ Regel på {}: {} matchade, {} ändrade",
        scope,
        result.matched,
        result.changed
    );
    // Regeln kan ha tagit bort eller flyttat noter — kanalens steg måste följa med.
    if result.changed > 0 {
        self.resync_grid_channels();
        self.piano_roll_sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
    }
}

/// Efter en operation på rutnätet: kanalens steg och not följer rutnätets översta ton,
/// och mönstret skrivs om — annars spelar kanalen kvar på den gamla raden.
pub(crate) fn resync_grid_channels(&mut self) {
    for step in 0..crate::audio::tempo::MAX_STEPS {
        let mut top = None;
        for row in 0..24 {
            if self.piano_roll_grid[row][step] {
                top = Some(row);
            }
        }
        self.channels[6].steps[step] = top.is_some();
        if let Some(row) = top {
            self.channels[6].notes[step] = crate::audio::scale::PIANO_ROLL_BASE_MIDI + row as u8;
        }
    }
    self.sync_active_pattern_from_ui();
}

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
    let mut new_grid = [[false; crate::ui::app::state::MAX_STEPS]; 24];
    for offset in 0..24 {
        for step in 0..crate::audio::tempo::MAX_STEPS {
            if self.piano_roll_grid[offset][step] {
                let target = offset as i8 + semitones;
                if target >= 0 && target < 24 {
                    new_grid[target as usize][step] = true;
                }
            }
        }
    }
    self.piano_roll_grid = new_grid;
    for step in 0..crate::audio::tempo::MAX_STEPS {
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
    self.piano_roll_grid = [[false; crate::ui::app::state::MAX_STEPS]; 24];
    for step in 0..crate::audio::tempo::MAX_STEPS {
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
            ui.label(egui::RichText::new(crate::i18n::t("🎹 SONIX PIANO ROLL")).strong().size(Theme::FONT_M).color(Theme::FL_GREEN));
            ui.separator();

            ui.label(egui::RichText::new(crate::i18n::t("Mönster:")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
            let p_len = self.patterns.len();
            for pat_idx in 0..p_len {
                let is_active = self.selected_pattern == pat_idx;
                let p_color = self.patterns[pat_idx].color;
                let fill = if is_active { p_color } else { Theme::PANEL_BG };
                let text_color = if is_active { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                let btn_text = format!("{} {}", pat_idx + 1, self.patterns[pat_idx].name);
                if ui.add(egui::Button::new(egui::RichText::new(btn_text).strong().size(Theme::FONT_S).color(text_color)).fill(fill)).clicked() {
                    self.select_pattern(pat_idx);
                }
            }

            ui.separator();

            // Root Key Picker
            ui.label(egui::RichText::new(crate::i18n::t("Grundton:")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
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
            ui.label(egui::RichText::new(crate::i18n::t("Skala:")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
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

            let snap_bg = if self.piano_roll_snap_to_scale { Theme::FL_CYAN } else { Theme::CHANNEL_BG };
            let snap_fg = if self.piano_roll_snap_to_scale { Color32::BLACK } else { Theme::TEXT_BRIGHT };
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔒 Skal-lås")).strong().size(Theme::FONT_S).color(snap_fg)).fill(snap_bg)).on_hover_text(crate::i18n::t("Lås tangenter till vald skala")).clicked() {
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
                .size(Theme::FONT_S)
                .color(if take_notes == 0 { Theme::TEXT_MUTED } else { Theme::TEXT_BRIGHT }),
            );
            ui.label(egui::RichText::new(crate::i18n::t("Rutnät:")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
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
            ui.label(egui::RichText::new(crate::i18n::t("Styrka")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
            ui.add(
                egui::Slider::new(&mut self.take_strength, 0.0..=1.0)
                    .show_value(false)
                    .fixed_decimals(2),
            )
            .on_hover_text(crate::i18n::t("Hur hårt noterna dras mot rutnätet (0 % = oförändrat, 100 % = exakt på rutnätet)"));

            let take_btn = Theme::CHANNEL_BG;
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎯 Kvantisera")).strong().size(Theme::FONT_S).color(Theme::TEXT_BRIGHT)).fill(take_btn)).on_hover_text(crate::i18n::t("Dra tagningens noter till närmaste rutnätslinje, med projektets sväng")).clicked() {
                let grid = crate::midi_take::TakeGrid::from_index(self.take_grid_idx);
                let strength = self.take_strength;
                self.quantize_take(strength, grid);
            }
            ui.label(egui::RichText::new(crate::i18n::t("Humanisering")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
            ui.add(
                egui::Slider::new(&mut self.take_humanize, 0.0..=1.0)
                    .show_value(false)
                    .fixed_decimals(2),
            )
            .on_hover_text(crate::i18n::t("Hur mycket mänsklig otajthet och dynamik som läggs på"));
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🌀 Humanisera")).strong().size(Theme::FONT_S).color(Theme::TEXT_BRIGHT)).fill(take_btn)).on_hover_text(crate::i18n::t("Lägg på mänsklig otajthet och dynamik (varierar mellan trycken)")).clicked() {
                let amount = self.take_humanize;
                self.humanize_take(amount);
            }

            ui.separator();
            ui.label(
                egui::RichText::new(crate::i18n::t("Strumning"))
                    .size(Theme::FONT_S)
                    .color(Theme::TEXT_MUTED),
            );
            ui.add(
                egui::Slider::new(&mut self.take_strum, 0.0..=0.5)
                    .show_value(false)
                    .fixed_decimals(2),
            )
            .on_hover_text(crate::i18n::t(
                "Hur många steg isär noterna i ett ackord slås (0 = som en orgel)",
            ));
            ui.add(egui::Checkbox::new(
                &mut self.take_strum_down,
                crate::i18n::t("Ner"),
            ))
            .on_hover_text(crate::i18n::t("Nedstrum: högsta tonen först"));
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(crate::i18n::t("🪕 Strumma ackord"))
                            .strong()
                            .size(Theme::FONT_S)
                            .color(Theme::TEXT_BRIGHT),
                    )
                    .fill(take_btn),
                )
                .on_hover_text(crate::i18n::t(
                    "Sprider noter som ligger på samma slag i tonhöjdsordning, som en gitarr",
                ))
                .clicked()
            {
                let spread = self.take_strum;
                let down = self.take_strum_down;
                self.strum_take(spread, down);
            }

            ui.separator();
            egui::CollapsingHeader::new(
                egui::RichText::new(crate::i18n::t("📜 Skript")).size(Theme::FONT_S),
            )
            .id_salt("piano_roll_script")
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.script_text)
                        .desired_rows(3)
                        .desired_width(240.0)
                        .hint_text(crate::i18n::t("transpose 12")),
                );
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(crate::i18n::t("▶ Kör skript"))
                                .strong()
                                .size(Theme::FONT_S)
                                .color(Theme::TEXT_BRIGHT),
                        )
                        .fill(take_btn),
                    )
                    .on_hover_text(crate::i18n::t(
                        "select all / select key > 60 / transpose 12 / velocity *0.8 / humanize 0.5 / strum 0.05 / quantize 100 1 / delete",
                    ))
                    .clicked()
                {
                    self.run_piano_script();
                }
            });

            ui.separator();
            ui.label(
                egui::RichText::new(crate::i18n::t("Groove"))
                    .size(Theme::FONT_S)
                    .color(Theme::TEXT_MUTED),
            );
            ui.add(
                egui::Slider::new(&mut self.take_groove_amount, 0.0..=1.0)
                    .show_value(false)
                    .fixed_decimals(2),
            )
            .on_hover_text(crate::i18n::t(
                "Hur mycket av mallens förskjutning som läggs på (0 % = rak)",
            ));
            let groove_new = if self.groove_pool.is_empty() {
                crate::i18n::t("🎵 Groove (.agr)").to_string()
            } else {
                crate::tstatus!("🎵 + mall ({})", self.groove_pool.len())
            };
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(groove_new)
                            .strong()
                            .size(Theme::FONT_S)
                            .color(Theme::TEXT_BRIGHT),
                    )
                    .fill(take_btn),
                )
                .on_hover_text(crate::i18n::t(
                    "Läs en Ableton-groove (.agr) och lägg den i poolen",
                ))
                .clicked()
            {
                self.apply_groove_from_file();
            }
            // **Groove-biblioteket**: en mapp med .agr läses en gång, och mallarna väljs ur en
            // lista. FL:s 220 mallar är för många för en knapp per mall — och för många att
            // lägga i poolen på en gång.
            if self.groove_library.grooves.is_empty() {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(crate::i18n::t("📚 Malmapp"))
                                .strong()
                                .size(Theme::FONT_S)
                                .color(Theme::TEXT_BRIGHT),
                        )
                        .fill(take_btn),
                    )
                    .on_hover_text(crate::i18n::t("Läs alla .agr-mallar i en mapp och välj bland dem"))
                    .clicked()
                {
                    self.load_groove_library();
                }
            } else {
                let mut chosen: Option<usize> = None;
                let grooves = &self.groove_library.grooves;
                egui::ComboBox::from_id_salt("groove_library")
                    .selected_text(crate::tstatus!("📚 {} mallar", grooves.len()))
                    .show_ui(ui, |ui| {
                        for (i, groove) in grooves.iter().enumerate() {
                            if ui.selectable_label(false, &groove.name).clicked() {
                                chosen = Some(i);
                            }
                        }
                    });
                if ui
                    .small_button("📂")
                    .on_hover_text(crate::i18n::t("Välj en annan mapp"))
                    .clicked()
                {
                    self.load_groove_library();
                }
                if ui
                    .small_button("🔄")
                    .on_hover_text(crate::i18n::t("Läs om mappen"))
                    .clicked()
                {
                    self.rescan_groove_library();
                }
                if let Some(i) = chosen {
                    let amount = self.take_groove_amount.clamp(0.0, 1.0);
                    let groove = self.groove_library.grooves[i].clone();
                    let name = groove.name.clone();
                    self.groove_pool
                        .push(crate::groove::GrooveEntry::new(groove, amount));
                    self.status_message = crate::tstatus!(
                        "🎵 '{}' i poolen ({} mallar)",
                        name,
                        self.groove_pool.len()
                    );
                }
            }
            // **Frasbanken** (FL): fraser på disk i stället för en tom ruta. Frasen infogas
            // genom MIDI-importens väg, så taktbytet och rutnätet är samma.
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(crate::i18n::t("🎼 Frasbank"))
                            .strong()
                            .size(Theme::FONT_S)
                            .color(Theme::TEXT_BRIGHT),
                    )
                    .fill(take_btn),
                )
                .on_hover_text(crate::i18n::t("Infoga en MIDI- eller .fsc-fras från en mapp i det valda patternet"))
                .clicked()
            {
                if self.phrases.is_empty() {
                    self.load_phrase_bank();
                } else {
                    self.show_phrase_bank = true;
                }
            }
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(crate::i18n::t("🎧 Extrahera ur spåret"))
                            .strong()
                            .size(Theme::FONT_S)
                            .color(Theme::TEXT_BRIGHT),
                    )
                    .fill(take_btn),
                )
                .on_hover_text(crate::i18n::t(
                    "Läs spårets anslag och gör en groove-mall av dem — samma väg som en .agr",
                ))
                .clicked()
            {
                self.extract_groove_from_track();
            }
            // **Riff-generator** (FL): en fras ur en skala i stället för en tom ruta. Fröet
            // kommer ur klockan, så varje tryck ger en ny fras — och frasen går att spela in
            // över, vilket är hela poängen.
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(crate::i18n::t("🎲 Riff")).strong().size(Theme::FONT_S).color(Theme::TEXT_BRIGHT))
                        .fill(take_btn),
                )
                .on_hover_text(crate::i18n::t("Fyller tagningen med en fras ur en mollskala. Tryck igen för en annan."))
                .clicked()
            {
                self.generate_riff();
            }
            egui::ComboBox::from_id_salt("riff_scale")
                .selected_text(crate::i18n::t(self.riff_scale.label()))
                .width(84.0)
                .show_ui(ui, |ui| {
                    for scale in crate::riff::Scale::ALL {
                        if ui.selectable_label(self.riff_scale == scale, crate::i18n::t(scale.label())).clicked() {
                            self.riff_scale = scale;
                        }
                    }
                });

            // **Urval enligt villkor** (Cubase): anslag under ett tak, en viss ton, inom ett
            // intervall. Räkna först, radera sedan — samma villkor, så det som raderas är det
            // som räknades.
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Urval:")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
                let mut max_vel = self.note_filter.max_velocity.unwrap_or(1.0);
                if ui
                    .add(egui::DragValue::new(&mut max_vel).range(0.02..=1.0).speed(0.01).custom_formatter(|v, _| format!("< {:.0}", v * 127.0)))
                    .on_hover_text(crate::i18n::t("Anslag under det här värdet"))
                    .changed()
                {
                    self.note_filter.max_velocity = if max_vel >= 0.999 { None } else { Some(max_vel) };
                }
                let mut key_on = self.note_filter.key.is_some();
                if ui.checkbox(&mut key_on, crate::i18n::t("ton")).changed() {
                    self.note_filter.key = if key_on { Some(60) } else { None };
                }
                if let Some(key) = &mut self.note_filter.key {
                    ui.add(egui::DragValue::new(key).range(0..=127).speed(0.2));
                }
                ui.add(egui::DragValue::new(&mut self.note_filter.pos_from).range(0.0..=15.9).speed(0.1).prefix("från "));
                ui.add(egui::DragValue::new(&mut self.note_filter.pos_to).range(0.1..=16.0).speed(0.1).prefix("till "));
                if ui.button(crate::i18n::t("Räkna")).clicked() {
                    let hits = self
                        .patterns
                        .get(self.selected_pattern)
                        .map(|p| p.take.count_matching(&self.note_filter))
                        .unwrap_or(0);
                    self.note_filter_hits = Some(hits);
                    self.status_message = format!("Urvalet fångar {} noter", hits);
                }
                if let Some(hits) = self.note_filter_hits {
                    ui.label(egui::RichText::new(format!("{hits}")).monospace().size(Theme::FONT_S).color(Theme::FL_CYAN));
                }
                // **Notsannolikhet** (Ableton): gäller urvalet, alltså samma villkor som
                // raderingen — det som ändras är det som räknades.
                ui.add(egui::DragValue::new(&mut self.note_probability).range(0.0..=1.0).speed(0.01).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)))
                    .on_hover_text(crate::i18n::t("Sannolikheten att noterna i urvalet spelas"));
                if ui.button(crate::i18n::t("Använd på urvalet")).clicked() {
                    let p = self.note_probability;
                    let touched = self
                        .patterns
                        .get_mut(self.selected_pattern)
                        .map(|pat| pat.take.set_probability(&self.note_filter, p))
                        .unwrap_or(0);
                    self.status_message = format!("Satte {:.0} % på {} noter", p * 100.0, touched);
                }
                // **Nudge** (Ableton): skjut urvalet i tid, ett sextondelssteg i taget.
                ui.horizontal(|ui| {
                    for (label, steps) in [("-1/16", -1.0f32), ("+1/16", 1.0)] {
                        if ui.button(egui::RichText::new(label).size(Theme::FONT_S)).clicked() {
                            let mut moved = 0;
                            if let Some(p) = self.patterns.get_mut(self.selected_pattern) {
                                moved = p.take.nudge(&self.note_filter, steps);
                            }
                            self.status_message = crate::tstatus!("→ flyttade {} noter", moved);
                        }
                    }
                });
                // **Join / Fit** (Ableton): städa dubbletter och drag ihop frasen.
                if ui.button(egui::RichText::new(crate::i18n::t("⧉ Slå ihop dubbletter")).size(Theme::FONT_S))
                    .on_hover_text(crate::i18n::t("Två noter på samma position och ton blir en — den starkaste."))
                    .clicked()
                {
                    let mut joined = 0;
                    if let Some(p) = self.patterns.get_mut(self.selected_pattern) {
                        joined = p.take.dedupe();
                    }
                    self.status_message = crate::tstatus!("⧉ slog ihop {} dubbletter", joined);
                }
                if ui.button(egui::RichText::new(crate::i18n::t("⇥ Tryck ihop frasen ×2")).size(Theme::FONT_S))
                    .on_hover_text(crate::i18n::t("Halverar avstånden mellan noterna, räknat från den första."))
                    .clicked()
                {
                    if let Some(p) = self.patterns.get_mut(self.selected_pattern) {
                        p.take.scale_positions(2.0);
                    }
                    self.status_message = crate::i18n::t("⇥ frasen hopdragen till halva längden").to_string();
                }
                // **Spegla** (FL: `Invert`): tonhöjderna vänds kring mitten av urvalets intervall,
                // så en uppgång blir en nedgång med samma intervall.
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("↕ Spegla tonhöjder")).size(Theme::FONT_S)))
                    .on_hover_text(crate::i18n::t("Speglar noterna i urvalet kring C4 (MIDI 60). En not sju halvtoner över hamnar sju under."))
                    .clicked()
                {
                    let mut touched = 0;
                    if let Some(p) = self.patterns.get_mut(self.selected_pattern) {
                        touched = p.take.invert(&self.note_filter, 60);
                    }
                    self.status_message = crate::tstatus!("↕ Speglade {} noter", touched);
                }
                // **Capture MIDI** (Ableton): noterna du nyss spelade, utan att ha spelat in.
                let captured = self.note_capture.len();
                if ui.add_enabled(
                        !self.note_capture.is_empty(),
                        egui::Button::new(egui::RichText::new(crate::i18n::t("⏺ Fånga nyss spelade noter")).size(Theme::FONT_S).color(Theme::FL_ORANGE)),
                    )
                    .on_hover_text(crate::tstatus!(
                        "Lägger de {} noterna du spelat sedan den första i den valda tagningen, räknade till taktens steg.",
                        captured
                    ))
                    .clicked()
                {
                    let bpm = self.bpm;
                    let steps = self.note_capture.drain_to_steps(bpm);
                    self.capture_clock = None;
                    let mut added = 0;
                    if let Some(p) = self.patterns.get_mut(self.selected_pattern) {
                        for (pos, key, vel) in steps {
                            p.take.push(pos, key, vel);
                            added += 1;
                        }
                    }
                    self.status_message = if added == 0 {
                        crate::i18n::t("Inget att fånga — spela några noter först").to_string()
                    } else {
                        crate::tstatus!("⏺ Fångade {} noter till tagningen", added)
                    };
                }
                // **Score-loggen** (FL:s Auto dump score): allt spelat sedan förra dumpen,
                // utlagt över de takter det tar. Capture MIDI viker ihop; den här gör det inte.
                let logged = self.score_log.len();
                if ui
                    .add_enabled(
                        !self.score_log.is_empty(),
                        egui::Button::new(
                            egui::RichText::new(crate::tstatus!("📜 Score-logg ({})", logged))
                                .size(Theme::FONT_S)
                                .color(Theme::FL_CYAN),
                        ),
                    )
                    .on_hover_text(crate::i18n::t(
                        "Lägger allt du spelat sedan förra dumpen i patternet, över de takter det tar",
                    ))
                    .clicked()
                {
                    let placed = self.dump_score_log();
                    self.status_message = if placed == 0 {
                        crate::i18n::t("📜 Score-loggen hade inget som gick att lägga in").to_string()
                    } else {
                        crate::tstatus!("📜 Score-loggen: {} noter in i patternet", placed)
                    };
                }
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("Ta bort valda")).size(Theme::FONT_S).color(Theme::FL_RED))).clicked() {
                    let mut removed = 0;
                    if let Some(p) = self.patterns.get_mut(self.selected_pattern) {
                        removed = p.take.remove_matching(&self.note_filter);
                    }
                    self.note_filter_hits = Some(0);
                    self.status_message = format!("Tog bort {} noter enligt villkoret", removed);
                }
            });
            // **Artikulation**: sätter formen på urvalet och formar noterna en gång —
            // samma villkor som raderingen, så det som formas är det som räknades.
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Artikulation:")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
                for art in crate::midi_take::Articulation::ALL {
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(art.label())).size(Theme::FONT_S)))
                        .on_hover_text(crate::i18n::t("Sätter artikulationen på urvalet och formar noterna en gång"))
                        .clicked()
                    {
                        let mut applied = 0;
                        if let Some(p) = self.patterns.get_mut(self.selected_pattern) {
                            p.take.set_articulation(&self.note_filter, art);
                            applied = p.take.apply_articulations();
                        }
                        self.status_message =
                            crate::tstatus!("🖋 {} noter formade ({})", applied, crate::i18n::t(art.label()));
                    }
                }
            });

            if !self.groove_pool.is_empty() {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(crate::i18n::t("▶ Använd poolen"))
                                .strong()
                                .size(Theme::FONT_S)
                                .color(Theme::FL_GREEN),
                        )
                        .fill(take_btn),
                    )
                    .on_hover_text(crate::i18n::t("Lägg alla aktiva mallar på tagningen"))
                    .clicked()
                {
                    self.apply_groove_pool_to_take();
                }
                let mut remove: Option<usize> = None;
                let mut preview: Option<usize> = None;
                for (i, entry) in self.groove_pool.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        if ui
                            .small_button("▶")
                            .on_hover_text(crate::i18n::t("Lyssna på mallen"))
                            .clicked()
                        {
                            preview = Some(i);
                        }
                        ui.checkbox(&mut entry.enabled, "");
                        ui.label(egui::RichText::new(&entry.name).size(Theme::FONT_S).color(Theme::TEXT_BRIGHT));
                        ui.add(
                            egui::Slider::new(&mut entry.amount, 0.0..=1.0)
                                .show_value(false)
                                .fixed_decimals(2),
                        );
                        if ui
                            .small_button("✖")
                            .on_hover_text(crate::i18n::t("Ta bort mallen från poolen"))
                            .clicked()
                        {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    self.groove_pool.remove(i);
                }
                if let Some(i) = preview {
                    self.preview_groove(i);
                }
            }

            ui.separator();
            ui.label(
                egui::RichText::new(crate::i18n::t("Transponera"))
                    .size(Theme::FONT_S)
                    .color(Theme::TEXT_MUTED),
            );
            for (label, semitones) in [("−12", -12), ("−1", -1), ("+1", 1), ("+12", 12)] {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(label).strong().size(Theme::FONT_S).color(Theme::TEXT_BRIGHT),
                        )
                        .fill(take_btn),
                    )
                    .on_hover_text(crate::i18n::t("Transponera hela tagningen"))
                    .clicked()
                {
                    self.transpose_take(semitones);
                }
            }
            ui.label(
                egui::RichText::new(crate::i18n::t("Anslag")).size(Theme::FONT_S).color(Theme::TEXT_MUTED),
            );
            for (label, factor) in [("−10 %", 0.9), ("+10 %", 1.1)] {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(label).strong().size(Theme::FONT_S).color(Theme::TEXT_BRIGHT),
                        )
                        .fill(take_btn),
                    )
                    .on_hover_text(crate::i18n::t("Ändra anslaget på hela tagningen"))
                    .clicked()
                {
                    self.scale_take_velocity(factor);
                }
            }

            ui.separator();

            // Chord Stamp Selector
            ui.label(egui::RichText::new(crate::i18n::t("Ackord:")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
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
            ui.label(egui::RichText::new(crate::i18n::t("VERKTYG:")).strong().size(Theme::FONT_S).color(Theme::TEXT_MUTED));

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎲 Humanize")).strong().size(Theme::FONT_S).color(Color32::WHITE)).fill(Color32::from_rgb(180, 80, 20))).on_hover_text(crate::i18n::t("Variera anslagsdynamik (velocity) för levande sväng")).clicked() {
                self.humanize_active_pattern();
            }

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⚡ Arpeggio")).strong().size(Theme::FONT_S).color(Color32::WHITE)).fill(Color32::from_rgb(30, 110, 160))).on_hover_text(crate::i18n::t("Skapa automatiskt arpeggiomönster från vald skala")).clicked() {
                self.arpeggiate_pattern();
            }

            // **Färgkartan** (FL:s VFX Color Mapper): bara rutnätets färg, inget ljud.
            let color_bg = if self.color_notes {
                Color32::from_rgb(110, 60, 150)
            } else {
                Theme::BORDER_DARK
            };
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎨 Färg")).strong().size(Theme::FONT_S).color(Color32::WHITE)).fill(color_bg)).on_hover_text(crate::i18n::t("Färga noterna efter tonhöjd")).clicked() {
                self.color_notes = !self.color_notes;
            }

            // **Patternlängd** (1–4 takter). Stegen utanför längden ligger kvar i rutnätet,
            // så en ändring kastar aldrig noter — de hörs bara när patternet blir längre.
            let mut bars_pick = self
                .patterns
                .get(self.selected_pattern)
                .map_or(1, |p| p.bars)
                .clamp(1, 4);
            ui.label(
                egui::RichText::new(crate::i18n::t("Takter"))
                    .size(Theme::FONT_S)
                    .color(Theme::TEXT_MUTED),
            );
            egui::ComboBox::from_id_salt("pattern_bars")
                .selected_text(format!("{bars_pick}"))
                .width(48.0)
                .show_ui(ui, |ui| {
                    for b in 1..=4 {
                        if ui.selectable_label(bars_pick == b, format!("{b}")).clicked() {
                            bars_pick = b;
                        }
                    }
                });
            if let Some(p) = self.patterns.get_mut(self.selected_pattern) {
                p.bars = bars_pick;
            }

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⬆ +Okt")).size(Theme::FONT_S)).fill(Theme::BORDER_DARK)).on_hover_text(crate::i18n::t("Transponera upp 1 oktav (+12 halvtoner)")).clicked() {
                self.transpose_active_pattern(12);
            }

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⬇ -Okt")).size(Theme::FONT_S)).fill(Theme::BORDER_DARK)).on_hover_text(crate::i18n::t("Transponera ner 1 oktav (-12 halvtoner)")).clicked() {
                self.transpose_active_pattern(-12);
            }

            // **Till tonarten** (Fas 8.11): flyttar hela mönstret till det skift som sätter
            // flest toner i projektets skala. Regeln räknar bara ut *skiftet*; själva
            // flytten går genom `transpose_active_pattern` — samma provade väg som
            // oktavknapparna, alltså en väg och inte två.
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎵 Till tonarten")).size(Theme::FONT_S)).fill(Theme::BORDER_DARK)).on_hover_text(crate::i18n::t("Flytta hela mönstret till tonarten — det kortaste skiftet som sätter flest toner i skalan")).clicked() {
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

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁 Backa")).size(Theme::FONT_S)).fill(Theme::BORDER_DARK)).on_hover_text(crate::i18n::t("Vänd tonföljden baklänges")).clicked() {
                self.reverse_pattern();
            }

            let poly_bg = if self.piano_roll_poly_mode { Theme::FL_ORANGE } else { Theme::CHANNEL_BG };
            let poly_fg = if self.piano_roll_poly_mode { Color32::BLACK } else { Theme::TEXT_BRIGHT };
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎹 Polyfoni")).strong().size(Theme::FONT_S).color(poly_fg)).fill(poly_bg)).on_hover_text(crate::i18n::t("Tillåt flera toner samtidigt i samma steg (ackordmålning)")).clicked() {
                self.piano_roll_poly_mode = !self.piano_roll_poly_mode;
            }

            // Real MIDI keyboard input indicator + record arm (Fas 5.3)
            let midi_col = if self.midi_keyboard_connected { Theme::FL_GREEN } else { Theme::TEXT_MUTED };
            ui.label(egui::RichText::new(if self.midi_keyboard_connected { "🎹 MIDI ●" } else { "🎹 MIDI ○" }).strong().size(Theme::FONT_S).color(midi_col))
                .on_hover_text(if self.midi_keyboard_connected { crate::i18n::t("MIDI-klaviatur ansluten (ALSA Seq).") } else { crate::i18n::t("Ingen MIDI-klaviatur ansluten. Öppna Hårdvarukontroller för att koppla upp.") });
            let mrec_bg = if self.midi_record_armed { Color32::from_rgb(180, 40, 40) } else { Theme::BORDER_DARK };
            let mrec_fg = if self.midi_record_armed { Color32::WHITE } else { Theme::TEXT_BRIGHT };
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⏺ MIDI-REC")).strong().size(Theme::FONT_S).color(mrec_fg)).fill(mrec_bg)).on_hover_text(crate::i18n::t("Spela in hållna MIDI-toner i rutnätet under uppspelning.")).clicked() {
                self.midi_record_armed = !self.midi_record_armed;
                if self.midi_record_armed {
                    self.begin_new_take();
                }
            }

            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🧹 Töm")).size(Theme::FONT_S).color(Color32::from_rgb(255, 120, 120))).fill(Color32::from_rgb(50, 20, 25))).on_hover_text(crate::i18n::t("Rensa mönster")).clicked() {
                self.piano_roll_grid = [[false; crate::ui::app::state::MAX_STEPS]; 24];
                self.piano_roll_sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
                self.channels[6].steps = [false; crate::ui::app::state::MAX_STEPS];
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

        // Urvalsraden: antal markerade celler och de operationer Cubases förval mest gör.
        let sel_count = count_selected(&self.piano_roll_sel);
        let sel_row_bg = Theme::CHANNEL_BG;
        ui.horizontal(|ui| {
            if sel_count == 0 {
                ui.label(
                    egui::RichText::new(crate::i18n::t("Shift+klick markerar celler"))
                        .size(Theme::FONT_S)
                        .color(Theme::TEXT_MUTED),
                );
            } else {
                ui.label(
                    egui::RichText::new(crate::tstatus!("{} markerade", sel_count))
                        .size(Theme::FONT_S)
                        .strong()
                        .color(Theme::FL_CYAN),
                );
                for (label, rows, hint) in [
                    ("▲", 1, "Transponera markerade en rad upp"),
                    ("▼", -1, "Transponera markerade en rad ned"),
                    ("▲▲", 12, "Transponera markerade en oktav upp"),
                    ("▼▼", -12, "Transponera markerade en oktav ned"),
                ] {
                    if ui
                        .add(egui::Button::new(egui::RichText::new(label).size(Theme::FONT_S).color(Theme::TEXT_BRIGHT)).fill(sel_row_bg))
                        .on_hover_text(crate::i18n::t(hint))
                        .clicked()
                    {
                        let moved = transpose_cells(&mut self.piano_roll_grid, &self.piano_roll_sel, rows);
                        let from = if rows > 0 { "▲" } else { "▼" };
                        self.status_message = crate::tstatus!("{} {} cell(er) flyttade", from, moved);
                        self.resync_grid_channels();
                        // Markeringen följer med noterna, annars pekar den på tomma rutor.
                        self.piano_roll_sel = transpose_selection(&self.piano_roll_sel, rows);
                    }
                }
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(crate::i18n::t("🗑 Ta bort valda"))
                                .size(Theme::FONT_S)
                                .color(Color32::from_rgb(255, 120, 120)),
                        )
                        .fill(Color32::from_rgb(50, 20, 25)),
                    )
                    .on_hover_text(crate::i18n::t("Ta bort de markerade cellerna"))
                    .clicked()
                {
                    let removed = delete_cells(&mut self.piano_roll_grid, &self.piano_roll_sel);
                    self.piano_roll_sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
                    self.status_message = crate::tstatus!("🗑 {} cell(er) borttagna", removed);
                    self.resync_grid_channels();
                }
                if ui.button(crate::i18n::t("✖ Avmarkera")).clicked() {
                    self.piano_roll_sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
                }
            }
        });
        // Villkorsmarkeringen har egen rad: urvalsraden är redan full, och egui radbryter inte.
        ui.horizontal(|ui| {
            if ui.button(crate::i18n::t("Markera svaga")).on_hover_text(crate::i18n::t("Markera alla noter i steg som spelar svagare än tröskeln")).clicked() {
                let threshold = self.piano_roll_quiet_threshold;
                self.piano_roll_sel = select_quiet(&self.piano_roll_grid, &self.step_velocities, threshold);
                self.status_message = crate::tstatus!("Markerade {} cell(er) under anslag {:.0} %", count_selected(&self.piano_roll_sel), threshold * 100.0);
            }
            ui.add(
                egui::Slider::new(&mut self.piano_roll_quiet_threshold, 0.05..=1.0)
                    .show_value(true)
                    .fixed_decimals(2),
            )
            .on_hover_text(crate::i18n::t("Tröskel för markerade svaga steg"));
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(crate::i18n::t("🎹 Följ ackordet"))
                            .strong()
                            .size(Theme::FONT_S)
                            .color(Theme::TEXT_BRIGHT),
                    )
                    .fill(sel_row_bg),
                )
                .on_hover_text(crate::i18n::t("Flytta noterna till närmaste ackordston i ackordspåret för takten spelhuvudet står på"))
                .clicked()
            {
                self.snap_to_song_chord();
            }
        });
        // Logical Editor-lite: en regel, på urvalet om det finns, annars allt.
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(crate::i18n::t("Regel:"))
                    .size(Theme::FONT_S)
                    .color(Theme::TEXT_MUTED),
            );
            let field = RuleField::ALL[self.rule_field.min(RuleField::ALL.len() - 1)];
            egui::ComboBox::from_id_salt("rule_field")
                .selected_text(crate::i18n::t(field.label()))
                .width(78.0)
                .show_ui(ui, |ui| {
                    for (i, f) in RuleField::ALL.iter().enumerate() {
                        if ui.selectable_label(self.rule_field == i, crate::i18n::t(f.label())).clicked() {
                            self.rule_field = i;
                        }
                    }
                });
            let cmp = RuleCmp::ALL[self.rule_cmp.min(RuleCmp::ALL.len() - 1)];
            egui::ComboBox::from_id_salt("rule_cmp")
                .selected_text(cmp.label())
                .width(44.0)
                .show_ui(ui, |ui| {
                    for (i, c) in RuleCmp::ALL.iter().enumerate() {
                        if ui.selectable_label(self.rule_cmp == i, c.label()).clicked() {
                            self.rule_cmp = i;
                        }
                    }
                });
            let (min, max, speed) = match field {
                RuleField::Velocity => (0.0, 1.0, 0.01),
                RuleField::Key => (0.0, 127.0, 1.0),
                RuleField::Step => (1.0, 16.0, 1.0),
            };
            ui.add(egui::DragValue::new(&mut self.rule_value).range(min..=max).speed(speed))
                .on_hover_text(crate::i18n::t("Jämförelsevärde: anslag 0–1, tonhöjd som MIDI-nummer, steg 1–16"));
            ui.label(egui::RichText::new("→").size(Theme::FONT_S).color(Theme::TEXT_MUTED));
            let actions = [
                RuleAction::Delete,
                RuleAction::Transpose(0),
                RuleAction::SetVelocity(0.0),
            ];
            let action = actions[self.rule_action.min(actions.len() - 1)];
            egui::ComboBox::from_id_salt("rule_action")
                .selected_text(crate::i18n::t(action.label()))
                .width(96.0)
                .show_ui(ui, |ui| {
                    for (i, a) in actions.iter().enumerate() {
                        if ui.selectable_label(self.rule_action == i, crate::i18n::t(a.label())).clicked() {
                            self.rule_action = i;
                        }
                    }
                });
            if self.rule_action != 0 {
                let (min, max, speed) = if self.rule_action == 1 {
                    (-36.0, 36.0, 1.0)
                } else {
                    (0.0, 1.0, 0.01)
                };
                ui.add(
                    egui::DragValue::new(&mut self.rule_action_value)
                        .range(min..=max)
                        .speed(speed),
                )
                .on_hover_text(if self.rule_action == 1 {
                    crate::i18n::t("Rader att transponera (12 = en oktav)")
                } else {
                    crate::i18n::t("Anslaget som ska sättas (0–1)")
                });
            }
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(crate::i18n::t("⚙ Använd regel"))
                            .strong()
                            .size(Theme::FONT_S)
                            .color(Theme::TEXT_BRIGHT),
                    )
                    .fill(sel_row_bg),
                )
                .on_hover_text(crate::i18n::t("Kör regeln på urvalet, eller på hela rutnätet om inget är markerat"))
                .clicked()
            {
                self.apply_current_rule();
            }
        });

        let base_midi = crate::audio::scale::PIANO_ROLL_BASE_MIDI; // C3
        let semitones_count = 24;
        let key_root = self.song_key_root;
        let key_scale = self.song_key_scale;
        let root_note_val = key_root % 12;
        // Ctrl + +/− zoomar rutnätet; både radhöjd och cellbredd följer med, annars ser
        // en förstorad rad ut som en trasig tabell.
        let zoom = self.roll_zoom.clamp(0.6, 2.0);
        // Fyra takter får plats i rutnätet; panels bredd räcker inte, så båda riktningarna
        // rullar. Längden läses ur patternet, inte ur en inställning i pianorullen.
        let bars = self
            .patterns
            .get(self.selected_pattern)
            .map_or(1, |p| p.bars)
            .clamp(1, 4);
        let steps = crate::ui::app::state::pattern_steps(bars);

        egui::ScrollArea::both().max_height(250.0).show(ui, |ui| {
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
                        Theme::BG_DARK
                    } else if is_sharp {
                        Color32::from_rgb(32, 42, 58)
                    } else {
                        Color32::from_rgb(220, 228, 240)
                    };
                    let key_text = if is_root || is_sharp || !note_in_scale { Color32::WHITE } else { Color32::BLACK };
                    let (k_rect, _) = ui.allocate_exact_size(Vec2::new(52.0, 16.0 * zoom), Sense::hover());
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
                        egui::FontId::proportional(Theme::FONT_S),
                        key_text,
                    );

                    // 16 kolumner per takt, 1–4 takter
                    for step in 0..steps {
                        let (cell_rect, response) = ui.allocate_exact_size(Vec2::new(26.0 * zoom, 16.0 * zoom), Sense::click());
                        let is_on = self.piano_roll_grid[note_offset][step];
                        let cell_key = base_midi + note_offset as u8;
                        let is_playhead = self.is_playing && self.current_step == step;
                        let is_beat_start = step % 4 == 0;

                        if response.clicked() {
                            // Shift-klick markerar cellen (Cubase: 86 av 610 förval väljer).
                            if ui.input(|i| i.modifiers.shift) {
                                toggle_cell_selection(&mut self.piano_roll_sel, note_offset, step);
                            } else if is_on {
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
                            if self.color_notes {
                                crate::ui::note_color::pitch_color(cell_key)
                            } else {
                                Theme::FL_GREEN
                            }
                        } else if is_root {
                            Color32::from_rgb(36, 32, 22)
                        } else if !note_in_scale {
                            Color32::from_rgb(14, 16, 22)
                        } else if is_sharp {
                            Theme::HEADER_BG
                        } else if is_beat_start {
                            Theme::BORDER_DARK
                        } else {
                            Theme::PANEL_BG
                        };

                        if is_playhead {
                            bg = Color32::from_rgb(255, 230, 100);
                        }
                        if self.piano_roll_sel[note_offset][step] {
                            bg = Color32::from_rgb(90, 150, 255);
                        }

                        ui.painter().rect_filled(cell_rect, Rounding::same(1.5), bg);
                        ui.painter().rect_stroke(cell_rect, Rounding::same(1.5), Stroke::new(0.5_f32, Theme::BORDER_DARK));
                    }
                });
            }
        });

        ui.add_space(4.0);

        // Velocity Editor Lane (FL Studio Note Velocity Stalks)
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("VELOCITY")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
            ui.allocate_exact_size(Vec2::new(10.0, 24.0), Sense::hover());
            for s in 0..steps {
                let (v_rect, v_resp) = ui.allocate_exact_size(Vec2::new(26.0 * zoom, 24.0 * zoom), Sense::click_and_drag());
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

    let mut kick = [false; crate::ui::app::state::MAX_STEPS];
    let mut snare = [false; crate::ui::app::state::MAX_STEPS];
    let mut clap = [false; crate::ui::app::state::MAX_STEPS];
    let mut hat_c = [false; crate::ui::app::state::MAX_STEPS];
    let mut hat_o = [false; crate::ui::app::state::MAX_STEPS];
    let mut crash = [false; crate::ui::app::state::MAX_STEPS];

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
            for step in 0..crate::audio::tempo::MAX_STEPS {
                if step % 4 == 2 {
                    hat_o[step] = true;
                } else {
                    hat_c[step] = true;
                }
            }
        }
        2 => {
            for step in 0..crate::audio::tempo::MAX_STEPS {
                if step % 2 == 0 {
                    hat_c[step] = true;
                } else if comp > 0.4 {
                    hat_c[step] = true;
                }
            }
        }
        _ => {
            for step in 0..crate::audio::tempo::MAX_STEPS {
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
            ui.label(egui::RichText::new(crate::i18n::t("🥁 SONIX SESSION DRUMMER")).strong().size(Theme::FONT_L).color(Theme::FL_ORANGE));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Intelligent trummis med 2D XY-radar för realtids-generering av trumspår")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));

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
                    ui.label(egui::RichText::new(crate::i18n::t("DRUMMERS (STILAR)")).strong().size(Theme::FONT_S).color(Theme::FL_CYAN));
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
                    ui.label(egui::RichText::new(crate::i18n::t("BEAT PRESETS")).strong().size(Theme::FONT_S).color(Theme::FL_ORANGE));
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
                    ui.label(egui::RichText::new(crate::i18n::t("XY PERFORMANCE RADAR")).strong().size(Theme::FONT_S).color(Theme::FL_YELLOW));
                    ui.label(egui::RichText::new(crate::i18n::t("Dra den gula pucken för att ändra dynamik & komplexitet i realtid")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
                    ui.add_space(4.0);

                    if drummer_xy_matrix(ui, &mut self.drummer_complexity, &mut self.drummer_loudness, Vec2::new(300.0, 260.0)) {
                        self.apply_drummer_generation();
                    }

                    ui.add_space(4.0);
                    ui.label(egui::RichText::new(format!("{} {:.0}%  •  {} {:.0}%", crate::i18n::t("Komplexitet:"), self.drummer_complexity * 100.0, crate::i18n::t("Ljudstyrka:"), self.drummer_loudness * 100.0)).size(Theme::FONT_S).color(Theme::FL_YELLOW));
                });
            });

            ui.separator();

            // Right Column: Drum Kit Parts & Fine Controls
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("TRUMSET & INSTRUMENT")).strong().size(Theme::FONT_S).color(Theme::FL_ORANGE));
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
                        ui.label(egui::RichText::new(crate::i18n::t("Kick & Snare:")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
                        for v in 1..=4 {
                            if ui.selectable_label(self.drummer_kick_snare_var == v, format!("{}", v)).clicked() {
                                self.drummer_kick_snare_var = v;
                                self.apply_drummer_generation();
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Hi-Hat Rytm:")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
                        for v in 1..=4 {
                            if ui.selectable_label(self.drummer_hihat_var == v, format!("{}", v)).clicked() {
                                self.drummer_hihat_var = v;
                                self.apply_drummer_generation();
                            }
                        }
                    });

                    ui.add_space(8.0);
                    ui.separator();
                    ui.label(egui::RichText::new(crate::i18n::t("FILLS & GROOVE")).strong().size(Theme::FONT_S).color(Theme::FL_CYAN));

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
                        ui.label(egui::RichText::new(crate::i18n::t("💡 TIPS")).strong().size(Theme::FONT_S).color(Theme::FL_GREEN));
                        ui.label(egui::RichText::new(crate::i18n::t("Alla förändringar i Session Drummer speglas direkt i FL Channel Rack och tidslinjen!")).size(Theme::FONT_XS).color(Theme::TEXT_MUTED));
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
            ui.label(egui::RichText::new(crate::i18n::t("🎹 SONIX PIANO")).strong().size(Theme::FONT_M).color(Theme::TEXT_BRIGHT));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Spela med tangentbordet [A, S, D...] eller klicka med musen")).size(Theme::FONT_S).color(Theme::TEXT_MUTED));
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
                Theme::TEXT_BRIGHT
            };

            ui.painter().rect_filled(key_rect, Rounding::same(3.0), bg_color);
            ui.painter().rect_stroke(key_rect, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(170, 178, 190)));

            ui.painter().text(
                Pos2::new(key_rect.center().x, key_rect.max.y - 12.0),
                egui::Align2::CENTER_CENTER,
                format!("{}{}", note_name(note), (note / 12) as i8 - 1),
                egui::FontId::proportional(Theme::FONT_S),
                Theme::CHANNEL_BG,
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
                Theme::PANEL_BG
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

// ---- Urval i rutnätet ---------------------------------------------------------
// Cubase: 86 av 610 förval *väljer*, 29 *tar bort*, 11 *transponerar*. Urvalet är
// förutsättningen för de andra två, och rutnätet är vår enda yta där noter syns.

/// Växlar markeringen på en cell. Returnerar antalet markerade celler.
pub(crate) fn toggle_cell_selection(sel: &mut [[bool; crate::ui::app::state::MAX_STEPS]; 24], row: usize, step: usize) -> usize {
    if row < 24 && step < 16 {
        sel[row][step] = !sel[row][step];
    }
    count_selected(sel)
}

pub(crate) fn count_selected(sel: &[[bool; crate::ui::app::state::MAX_STEPS]; 24]) -> usize {
    sel.iter().map(|r| r.iter().filter(|c| **c).count()).sum()
}

/// Transponerar markerade celler `rows` rader. Celler som skulle hamna utanför
/// rutnätet **stannar där de är** i stället för att tappas.
pub(crate) fn transpose_cells(
    grid: &mut [[bool; crate::ui::app::state::MAX_STEPS]; 24],
    sel: &[[bool; crate::ui::app::state::MAX_STEPS]; 24],
    rows: i32,
) -> usize {
    let mut next = *grid;
    for row in 0..24 {
        for step in 0..crate::audio::tempo::MAX_STEPS {
            if sel[row][step] {
                next[row][step] = false;
            }
        }
    }
    let mut moved = 0;
    for row in 0..24 {
        for step in 0..crate::audio::tempo::MAX_STEPS {
            if !sel[row][step] {
                continue;
            }
            let target = row as i32 + rows;
            if (0..24).contains(&target) {
                next[target as usize][step] = true;
                moved += 1;
            } else {
                next[row][step] = true;
            }
        }
    }
    *grid = next;
    moved
}

/// Vad regeln jämför. Anslaget hör till **steget**, inte cellen — både villkoret och
/// åtgärden läser och skriver samma stegvelocitet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RuleField {
    Velocity,
    Key,
    Step,
}

impl RuleField {
    pub const ALL: [RuleField; 3] = [RuleField::Velocity, RuleField::Key, RuleField::Step];

    pub fn label(self) -> &'static str {
        match self {
            RuleField::Velocity => "Anslag",
            RuleField::Key => "Tonhöjd",
            RuleField::Step => "Steg",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleCmp {
    Less,
    Greater,
    Equal,
}

impl RuleCmp {
    pub const ALL: [RuleCmp; 3] = [RuleCmp::Less, RuleCmp::Greater, RuleCmp::Equal];

    pub fn label(self) -> &'static str {
        match self {
            RuleCmp::Less => "<",
            RuleCmp::Greater => ">",
            RuleCmp::Equal => "=",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RuleAction {
    Delete,
    Transpose(i32),
    SetVelocity(f32),
}

impl RuleAction {
    pub fn label(self) -> &'static str {
        match self {
            RuleAction::Delete => "radera",
            RuleAction::Transpose(_) => "transponera",
            RuleAction::SetVelocity(_) => "sätt anslag",
        }
    }
}

/// En regel: `fält cmp värde` → `åtgärd`. Cubases Logical Editor har flera rader och
/// fältet `lePosition/leType/leValue`; vi har en rad, vilket räcker för de vanligaste förvalen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    pub field: RuleField,
    pub cmp: RuleCmp,
    /// Jämförelsevärde: anslag 0–1, tonhöjd som MIDI-nummer, steg 1–16.
    pub value: f32,
    pub action: RuleAction,
}

impl Rule {
    fn matches(&self, row: usize, step: usize, velocity: f32) -> bool {
        let left = match self.field {
            RuleField::Velocity => velocity,
            RuleField::Key => (crate::audio::scale::PIANO_ROLL_BASE_MIDI + row as u8) as f32,
            RuleField::Step => (step + 1) as f32,
        };
        match self.cmp {
            RuleCmp::Less => left < self.value,
            RuleCmp::Greater => left > self.value,
            RuleCmp::Equal => (left - self.value).abs() < 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleResult {
    pub matched: usize,
    pub changed: usize,
}

/// Kör regeln. Finns ett urval gäller det, annars hela rutnätet — samma ordning som
/// radera/transponera-knapparna, så regeln inte blir en tredje sorts sanning.
pub fn apply_rule(
    grid: &mut [[bool; crate::ui::app::state::MAX_STEPS]; 24],
    sel: &[[bool; crate::ui::app::state::MAX_STEPS]; 24],
    velocities: &mut [f32; crate::audio::tempo::MAX_STEPS],
    rule: &Rule,
) -> RuleResult {
    let only_selection = count_selected(sel) > 0;
    let mut matched = [[false; crate::ui::app::state::MAX_STEPS]; 24];
    for row in 0..24 {
        for step in 0..crate::audio::tempo::MAX_STEPS {
            if !grid[row][step] || (only_selection && !sel[row][step]) {
                continue;
            }
            if rule.matches(row, step, velocities[step]) {
                matched[row][step] = true;
            }
        }
    }
    let matched_count = count_selected(&matched);
    let changed = match rule.action {
        RuleAction::Delete => delete_cells(grid, &matched),
        RuleAction::Transpose(rows) => {
            transpose_cells(grid, &matched, rows);
            matched_count
        }
        RuleAction::SetVelocity(v) => {
            let mut n = 0;
            for step in 0..crate::audio::tempo::MAX_STEPS {
                let hit = (0..24).any(|row| matched[row][step]);
                if hit && (velocities[step] - v).abs() > 1e-4 {
                    velocities[step] = v;
                    n += 1;
                }
            }
            n
        }
    };
    RuleResult { matched: matched_count, changed }
}

/// Markerar celler i steg vars anslag ligger under tröskeln.
///
/// Cubase väljer enligt ett villkor och låter sedan åtgärden gå på urvalet — vi har samma
/// ordning: markera de svaga, sedan 🗑 eller transponera. Ingen egen raderingsväg.
pub(crate) fn select_quiet(
    grid: &[[bool; crate::ui::app::state::MAX_STEPS]; 24],
    velocities: &[f32; crate::audio::tempo::MAX_STEPS],
    threshold: f32,
) -> [[bool; crate::ui::app::state::MAX_STEPS]; 24] {
    let mut sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
    for step in 0..crate::audio::tempo::MAX_STEPS {
        if velocities[step] >= threshold {
            continue;
        }
        for row in 0..24 {
            if grid[row][step] {
                sel[row][step] = true;
            }
        }
    }
    sel
}

/// Snappar noter till ackordets toner — melodins karaktär, inte ett fast intervall.
///
/// Noter som skulle hamna utanför rutnätet stannar, samma regel som transponeringen.
pub fn snap_grid_to_chord(
    grid: &mut [[bool; crate::ui::app::state::MAX_STEPS]; 24],
    sel: &[[bool; crate::ui::app::state::MAX_STEPS]; 24],
    chord: &crate::ui::app::state::ChordEvent,
) -> usize {
    let only_selection = count_selected(sel) > 0;
    let mut next = *grid;
    let mut moved = 0;
    for row in 0..24 {
        for step in 0..crate::audio::tempo::MAX_STEPS {
            if !grid[row][step] || (only_selection && !sel[row][step]) {
                continue;
            }
            let midi = crate::audio::scale::PIANO_ROLL_BASE_MIDI + row as u8;
            let snapped = crate::ui::app::state::snap_to_chord(midi, chord);
            let delta = snapped as i32 - midi as i32;
            if delta == 0 {
                continue;
            }
            let target = row as i32 + delta;
            if (0..24).contains(&target) {
                next[row][step] = false;
                next[target as usize][step] = true;
                moved += 1;
            }
        }
    }
    *grid = next;
    moved
}

/// Flyttar markeringen med noterna, så den pekar på samma noter efter en transponering.
pub(crate) fn transpose_selection(sel: &[[bool; crate::ui::app::state::MAX_STEPS]; 24], rows: i32) -> [[bool; crate::ui::app::state::MAX_STEPS]; 24] {
    let mut next = [[false; crate::ui::app::state::MAX_STEPS]; 24];
    for row in 0..24 {
        for step in 0..crate::audio::tempo::MAX_STEPS {
            if !sel[row][step] {
                continue;
            }
            let target = row as i32 + rows;
            // Samma regel som cellerna: får den inte plats stannar markeringen kvar på raden,
            // annars pekar den på en annan not än den som ligger i rutnätet.
            if (0..24).contains(&target) {
                next[target as usize][step] = true;
            } else {
                next[row][step] = true;
            }
        }
    }
    next
}

/// Tar bort markerade celler. Returnerar antalet borttagna.
pub(crate) fn delete_cells(grid: &mut [[bool; crate::ui::app::state::MAX_STEPS]; 24], sel: &[[bool; crate::ui::app::state::MAX_STEPS]; 24]) -> usize {
    let mut removed = 0;
    for row in 0..24 {
        for step in 0..crate::audio::tempo::MAX_STEPS {
            if sel[row][step] && grid[row][step] {
                grid[row][step] = false;
                removed += 1;
            }
        }
    }
    removed
}

#[cfg(test)]
mod selection_tests {
    use super::*;

    #[test]
    fn transpose_moves_selected_cells_and_keeps_the_ones_that_do_not_fit() {
        let mut grid = [[false; crate::ui::app::state::MAX_STEPS]; 24];
        let mut sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
        grid[23][0] = true; // rad 23 är överst: kan inte flyttas upp
        grid[10][4] = true;
        grid[11][4] = true; // orörd, inte markerad
        sel[23][0] = true;
        sel[10][4] = true;

        assert_eq!(transpose_cells(&mut grid, &sel, 2), 1, "bara den som får plats flyttas");
        assert!(grid[23][0], "cellen som inte fick plats stannar");
        assert!(!grid[10][4]);
        assert!(grid[12][4]);
        assert!(grid[11][4], "omarkerade celler rörs inte");

        let moved_sel = transpose_selection(&sel, 2);
        assert!(moved_sel[23][0], "markeringen stannar med noten som inte fick plats");
        assert!(moved_sel[12][4]);

        assert_eq!(delete_cells(&mut grid, &sel), 1);
        assert!(!grid[23][0]);
        assert!(grid[12][4], "markeringen gäller ursprungsraden, inte den flyttade");
    }

    #[test]
    fn quiet_steps_are_selected_by_their_velocity() {
        let mut grid = [[false; crate::ui::app::state::MAX_STEPS]; 24];
        grid[5][0] = true; // starkt steg
        grid[6][1] = true; // svagt steg
        grid[7][1] = true;
        let mut vel = [0.9f32; crate::audio::tempo::MAX_STEPS];
        vel[1] = 0.2;

        let sel = select_quiet(&grid, &vel, 0.3);
        assert!(!sel[5][0], "starka steget ska inte markeras");
        assert!(sel[6][1] && sel[7][1], "båda noterna i det svaga steget markeras");
        assert_eq!(count_selected(&sel), 2);
        assert_eq!(count_selected(&select_quiet(&grid, &vel, 0.1)), 0, "lägre tröskel ger inget");
    }

    #[test]
    fn chord_snap_moves_notes_to_the_nearest_chord_tone() {
        use crate::ui::app::state::{ChordEvent, ChordQuality};
        let mut grid = [[false; crate::ui::app::state::MAX_STEPS]; 24];
        let mut sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
        // C3 = bas. Rad 0 = C, rad 4 = E, rad 5 = F.
        grid[0][0] = true; // C — ackordston, står still
        grid[5][1] = true; // F — en halvton över E
        grid[8][2] = true; // G# — en halvton under G, men omarkerad när urval finns
        sel[0][0] = true;
        sel[5][1] = true;

        let c = ChordEvent { bar: 0, root: 0, quality: ChordQuality::Major };
        assert_eq!(snap_grid_to_chord(&mut grid, &sel, &c), 1, "bara F flyttas");
        assert!(grid[0][0], "C står kvar");
        assert!(!grid[5][1]);
        assert!(grid[4][1], "F blev E");
        assert!(grid[8][2], "omarkerad not rörs inte");
    }

    #[test]
    fn a_rule_matches_by_key_and_only_touches_the_selection() {
        let mut grid = [[false; crate::ui::app::state::MAX_STEPS]; 24];
        let mut sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
        let mut vel = [0.8f32; crate::audio::tempo::MAX_STEPS];
        grid[12][0] = true; // C3 + 12
        grid[12][2] = true;
        grid[5][1] = true; // annan tonhöjd
        sel[12][0] = true; // bara den första är markerad

        let rule = Rule {
            field: RuleField::Key,
            cmp: RuleCmp::Equal,
            value: (crate::audio::scale::PIANO_ROLL_BASE_MIDI + 12) as f32,
            action: RuleAction::Delete,
        };
        let r = apply_rule(&mut grid, &sel, &mut vel, &rule);
        assert_eq!(r.matched, 1, "bara den markerade cellen med rätt tonhöjd");
        assert!(!grid[12][0], "markerad och matchande tas bort");
        assert!(grid[12][2], "samma tonhöjd men omarkerad rörs inte");
        assert!(grid[5][1]);
    }

    #[test]
    fn a_rule_can_set_the_velocity_of_matching_steps_and_transpose() {
        let mut grid = [[false; crate::ui::app::state::MAX_STEPS]; 24];
        let sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
        let mut vel = [0.9f32; crate::audio::tempo::MAX_STEPS];
        vel[3] = 0.2;
        grid[10][3] = true;
        grid[10][4] = true;

        // Sätt anslaget på de steg som spelar svagare än 0,3.
        let set = Rule {
            field: RuleField::Velocity,
            cmp: RuleCmp::Less,
            value: 0.3,
            action: RuleAction::SetVelocity(0.7),
        };
        let r = apply_rule(&mut grid, &sel, &mut vel, &set);
        assert_eq!((r.matched, r.changed), (1, 1));
        assert!((vel[3] - 0.7).abs() < 1e-4, "det svaga steget fick nytt anslag");
        assert!((vel[4] - 0.9).abs() < 1e-4, "övriga steg orörda");

        // Transponera steg 4 en oktav upp.
        let tr = Rule {
            field: RuleField::Step,
            cmp: RuleCmp::Equal,
            value: 5.0,
            action: RuleAction::Transpose(12),
        };
        apply_rule(&mut grid, &sel, &mut vel, &tr);
        assert!(grid[22][4], "noten på steg 5 flyttades tolv rader upp");
        assert!(!grid[10][4]);
        assert!(grid[10][3], "steg 4 är orört");
    }

    #[test]
    fn selection_counts_and_toggles() {
        let mut sel = [[false; crate::ui::app::state::MAX_STEPS]; 24];
        assert_eq!(count_selected(&sel), 0);
        assert_eq!(toggle_cell_selection(&mut sel, 5, 3), 1);
        assert!(sel[5][3]);
        assert_eq!(toggle_cell_selection(&mut sel, 5, 3), 0, "samma cell växlar av");
        assert_eq!(toggle_cell_selection(&mut sel, 99, 99), 0, "utanför rutnätet gör inget");
    }
}
