//! Kreativa dialoger: ackord, dice, mod-matrix, tidsverktyg.
//!
//! Utdelad ur `modals.rs` 2026-11-02; filen var 4 486 rader och höll 34 dialoger.

use super::*;
use crate::ui::theme::Theme;

impl SonixApp {
pub(crate) fn render_chord_generator_modal_view(&mut self, ctx: &egui::Context) {
    let p_idx = self.selected_pattern;
    let c_idx = self.selected_channel;
    let num_chans = self.channels.len();

    let mut insert_chords: Option<Vec<GeneratedChord>> = None;
    let mut status = self.status_message.clone();

    render_chord_generator_modal(
        ctx,
        &mut self.show_chord_generator_modal,
        &mut self.chord_generator_state,
        &mut self.engine,
        &mut status,
        |chords| {
            insert_chords = Some(chords.to_vec());
        },
    );
    self.status_message = status;

    if let Some(chords) = insert_chords
        && p_idx < self.patterns.len() && c_idx < num_chans {
            for (step_i, chord) in chords.iter().enumerate() {
                let s_start = (step_i * 4) % 16;
                self.channels[c_idx].steps[s_start] = true;
                self.channels[c_idx].notes[s_start] = chord.root_midi;
                // Insert the full voicing into the piano roll grid so every
                // chord tone is stored (and played back), not just the root.
                for &n in &chord.notes {
                    let row = n as i32 - 48;
                    if (0..24).contains(&row) {
                        self.piano_roll_grid[row as usize][s_start] = true;
                    }
                }
                if let Some(pat_steps) = self.patterns[p_idx].channel_steps.get_mut(c_idx) {
                    pat_steps[s_start] = true;
                }
                if let Some(pat_notes) = self.patterns[p_idx].channel_notes.get_mut(c_idx) {
                    pat_notes[s_start] = chord.root_midi;
                }
            }
            self.patterns[p_idx].piano_roll_grid = self.piano_roll_grid;
            self.status_message = crate::tstatus!("🎹 Infogade {} ackord (fulla voicings) i Mönster {} (Kanal: {})!", chords.len(), p_idx + 1, self.channels[c_idx].name);
        }
}
}

impl SonixApp {
pub(crate) fn render_tuner_modal_view(&mut self, ctx: &egui::Context) {
    let mic_vu = self.vocal_studio.mic_vu_level;
    let live_pitch = self.vocal_studio.detect_live_pitch_hz();
    render_tuner_modal(
        ctx,
        &mut self.show_tuner_modal,
        &mut self.tuner_state,
        &mut self.engine,
        mic_vu,
        live_pitch,
    );
}
}

impl SonixApp {
    /// **Newtime**: ritar tidseditorn och tar emot dess beslut. Renderingen sker först när
    /// *Skapa spår* trycks — rutan arbetar på en kopia hela vägen.
    pub(crate) fn render_newtime_modal_view(&mut self, ctx: &egui::Context) {
        let Some(state) = self.newtime.as_mut() else {
            return;
        };
        match crate::ui::newtime_modal::render_newtime_modal(ctx, state) {
            crate::ui::newtime_modal::NewtimeAction::None => {}
            crate::ui::newtime_modal::NewtimeAction::Close => self.newtime = None,
            crate::ui::newtime_modal::NewtimeAction::Create => self.newtime_create_track(),
        }
    }
}

impl SonixApp {
    /// **Källans tempo**: ritar rutan och tar emot dess beslut. Ingenting ändras förrän
    /// *Använd* trycks, och då via `apply_source_tempo`.
    pub(crate) fn render_source_tempo_modal_view(&mut self, ctx: &egui::Context) {
        let Some(mut st) = self.source_tempo.take() else {
            return;
        };
        match crate::ui::source_tempo_modal::render_source_tempo_modal(ctx, &mut st) {
            crate::ui::source_tempo_modal::SourceTempoAction::None => {
                self.source_tempo = Some(st);
            }
            crate::ui::source_tempo_modal::SourceTempoAction::Close => {}
            crate::ui::source_tempo_modal::SourceTempoAction::Measure => {
                // Klippet är det valda — menyn som öppnade rutan kräver det.
                self.selected_audio_region = Some((st.track, st.region));
                if let Some((_, left, right, sample_rate, _)) = self.selected_region_audio() {
                    let bpm = crate::audio::stem_separator::estimate_bpm(
                        &left,
                        &right,
                        sample_rate as f32,
                    );
                    if bpm > 0.0 {
                        st.value = bpm;
                    } else {
                        self.status_message = crate::tstatus!(
                            "🎵 Hittade ingen takt i '{}' — klippet står still vid tempobyte",
                            st.name
                        );
                    }
                }
                self.source_tempo = Some(st);
            }
            crate::ui::source_tempo_modal::SourceTempoAction::Apply => {
                self.source_tempo = Some(st);
                self.apply_source_tempo();
            }
        }
    }
}

impl SonixApp {
    /// **Sätter källans tempo och följer med i längd.** Talet styr sträckningen (Fas 8.10), och
    /// klippets längd i takter skalas med `nytt/gammalt`: annars kapas låten när ett tempo som
    /// mätts en oktav fel rättas — halva filen låg utanför tidslinjen.
    pub(crate) fn apply_source_tempo(&mut self) {
        let Some(st) = self.source_tempo.take() else {
            return;
        };
        let value = st.value.clamp(20.0, 300.0);
        if st.old > 0.0 && (value - st.old).abs() < 0.001 {
            return;
        }
        self.push_undo("Källans tempo");
        let mut touched: Vec<usize> = Vec::new();
        let mut count = 0usize;
        for ti in 0..self.playlist_tracks.len() {
            let mut changed = false;
            for ri in 0..self.playlist_tracks[ti].regions.len() {
                let r = &mut self.playlist_tracks[ti].regions[ri];
                let hit = (ti == st.track && ri == st.region)
                    || (st.apply_all && st.old > 0.0 && (r.source_bpm - st.old).abs() < 0.01);
                if !hit {
                    continue;
                }
                // Längden följer tempot: samma källstycke ska ligga kvar på tidslinjen.
                // Regeln (och skälet) bor i `stretch::bars_after_source_tempo_change`.
                let old_bpm = r.source_bpm;
                r.length_bars =
                    crate::ui::app::stretch::bars_after_source_tempo_change(r.length_bars, old_bpm, value);
                r.source_bpm = value;
                changed = true;
                count += 1;
            }
            if changed {
                touched.push(ti);
            }
        }
        for ti in touched {
            self.sync_track_regions(ti);
        }
        let played = if value > 0.0 {
            self.bpm / value
        } else {
            1.0
        };
        self.status_message =
            crate::tstatus!("🎯 Källtempo {:.2} BPM på {} klipp — spelas {:.2}×", value, count, played);
    }
}

impl SonixApp {
    /// **Newtone**: ritar tonhöjdseditorn och tar emot dess beslut. Analysen gjordes när
    /// rutan öppnades; ingenting skrivs förrän *Skapa spår* trycks.
    pub(crate) fn render_newtone_modal_view(&mut self, ctx: &egui::Context) {
        let Some(state) = self.newtone.as_mut() else {
            return;
        };
        match crate::ui::newtone_modal::render_newtone_modal(ctx, state) {
            crate::ui::newtone_modal::NewtoneAction::None => {}
            crate::ui::newtone_modal::NewtoneAction::Close => self.newtone = None,
            crate::ui::newtone_modal::NewtoneAction::Create => self.newtone_create_track(),
        }
    }
}

impl SonixApp {
    /// **Edison**: ritar klippets ljudeditor och tar emot dess beslut. Ingen fil skrivs
    /// förrän *Skapa spår* trycks — rutan är en kopia hela vägen.
    pub(crate) fn render_edison_modal_view(&mut self, ctx: &egui::Context) {
        let Some(state) = self.edison.as_mut() else {
            return;
        };
        match crate::ui::edison_modal::render_edison_modal(ctx, state) {
            crate::ui::edison_modal::EdisonAction::None => {}
            crate::ui::edison_modal::EdisonAction::Close => self.edison = None,
            crate::ui::edison_modal::EdisonAction::Create => self.edison_create_track(),
        }
    }
}

impl SonixApp {
pub(crate) fn render_dice_generator_modal_view(&mut self, ctx: &egui::Context) {
    let p_idx = self.selected_pattern;
    let c_idx = self.selected_channel;
    let num_chans = self.channels.len();

    let mut insert_dice: Option<DiceGeneratorState> = None;
    let mut status = self.status_message.clone();

    render_dice_generator_modal(
        ctx,
        &mut self.show_dice_generator_modal,
        &mut self.dice_generator_state,
        &mut self.engine,
        &mut status,
        |state| {
            insert_dice = Some(state.clone());
        },
    );
    self.status_message = status;

    if let Some(dice) = insert_dice {
        if dice.category == crate::ui::dice_generator_modal::DiceCategory::DrumBeat {
            for d in 0..4.min(num_chans) {
                self.channels[d].steps[..16].copy_from_slice(&dice.generated_drum_grid[d]);
                if p_idx < self.patterns.len() && d < self.patterns[p_idx].channel_steps.len() {
                    self.patterns[p_idx].channel_steps[d][..16].copy_from_slice(&dice.generated_drum_grid[d]);
                }
            }
            self.status_message = crate::i18n::t("🎲 Klistrade in slumpat trumgroove på de 4 första trumspåren!").to_string();
        } else if p_idx < self.patterns.len() && c_idx < num_chans {
            self.channels[c_idx].steps = [false; crate::ui::app::state::MAX_STEPS];
            for n in &dice.generated_notes {
                if n.step < 16 {
                    self.channels[c_idx].steps[n.step] = true;
                    self.channels[c_idx].notes[n.step] = n.note;
                }
            }
            if let Some(pat_steps) = self.patterns[p_idx].channel_steps.get_mut(c_idx) {
                *pat_steps = self.channels[c_idx].steps;
            }
            if let Some(pat_notes) = self.patterns[p_idx].channel_notes.get_mut(c_idx) {
                *pat_notes = self.channels[c_idx].notes;
            }
            self.status_message = crate::tstatus!("🎲 Klistrade in slumpad melodi på '{}' i Mönster {}!", self.channels[c_idx].name, p_idx + 1);
        }
    }
}
}

impl SonixApp {
pub(crate) fn render_fx_rack_modal_view(&mut self, ctx: &egui::Context) {
    let mut status = self.status_message.clone();
    render_fx_rack_modal(
        ctx,
        &mut self.show_fx_rack_modal,
        &mut self.fx_rack_state,
        &mut self.engine,
        &mut status,
    );
    self.status_message = status;
}
}

impl SonixApp {
/// Modulationsmatrisen: FL:s Envelope/Peak/Keyboard Controller i en tabell.
pub(crate) fn render_mod_matrix_view(&mut self, ctx: &egui::Context) {
    use crate::audio::modulation::{CURVE_POINTS, LfoShape, ModSlot, ModSource, ModTarget};
    if !self.show_mod_matrix {
        return;
    }
    let mut open = true;
    let mut changed = false;
    egui::Window::new(crate::i18n::t("🕹 Modulationsmatris"))
        .open(&mut open)
        .default_width(520.0)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(crate::i18n::t(
                    "Källa → mål. LFO:n har egen takt per rad, envelopen följer masterns nivå, anslag och tonhöjd läser senaste noten.",
                ))
                .size(Theme::FONT_S)
                .color(crate::ui::theme::palette().text_muted()),
            );
            ui.separator();
            if self.mod_slots.is_empty() {
                ui.label(crate::i18n::t("Inga modulationer än."));
            }
            let mut remove: Option<usize> = None;
            for (i, slot) in self.mod_slots.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut slot.enabled, "").changed() {
                        changed = true;
                    }
                    // Källa
                    let src_is_lfo = matches!(slot.source, ModSource::Lfo { .. });
                    egui::ComboBox::from_id_salt(format!("mod_src_{i}"))
                        .selected_text(slot.source.label())
                        .width(96.0)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(!src_is_lfo, crate::i18n::t("Envelop")).clicked() {
                                slot.source = ModSource::Env;
                                changed = true;
                            }
                            if ui.selectable_label(src_is_lfo, "LFO").clicked() {
                                slot.source = ModSource::Lfo { rate_hz: 1.0, shape: LfoShape::Sine };
                                changed = true;
                            }
                            if ui.selectable_label(false, crate::i18n::t("Anslag")).clicked() {
                                slot.source = ModSource::Velocity;
                                changed = true;
                            }
                            if ui.selectable_label(false, crate::i18n::t("Tonhöjd")).clicked() {
                                slot.source = ModSource::Key;
                                changed = true;
                            }
                            // Spänning: ett fast läge, som en ratt på en X-Y-pad.
                            if ui.selectable_label(matches!(slot.source, ModSource::Voltage { .. }), crate::i18n::t("Spänning")).clicked() {
                                slot.source = ModSource::Voltage { value: 0.5 };
                                changed = true;
                            }
                            // ADSR (VFX Envelope): en envelop som triggas av noter.
                            if ui.selectable_label(matches!(slot.source, ModSource::Adsr { .. }), crate::i18n::t("ADSR")).clicked() {
                                slot.source = ModSource::Adsr { attack: 0.01, decay: 0.2, sustain: 0.6, release: 0.3 };
                                changed = true;
                            }
                            // Ritbar kurva (Envelope Controller): sexton punkter som går runt.
                            if ui.selectable_label(matches!(slot.source, ModSource::Curve { .. }), crate::i18n::t("Kurva")).clicked() {
                                let mut points = [0.0_f32; CURVE_POINTS];
                                for (i, p) in points.iter_mut().enumerate() {
                                    *p = -1.0 + 2.0 * i as f32 / (CURVE_POINTS - 1) as f32;
                                }
                                slot.source = ModSource::Curve { rate_hz: 1.0, points };
                                changed = true;
                            }
                        });
                    if let ModSource::Voltage { value } = &mut slot.source
                        && ui
                            .add(
                                egui::DragValue::new(value)
                                    .range(-1.0..=1.0)
                                    .speed(0.01)
                                    .custom_formatter(|v, _| format!("{:+.0} %", v * 100.0)),
                            )
                            .on_hover_text(crate::i18n::t("Fast läge — ingen rörelse, bara ett värde"))
                            .changed()
                    {
                        changed = true;
                    }
                    // ADSR-tider, bara när källan är en ADSR.
                    if let ModSource::Adsr { attack, decay, sustain, release } = &mut slot.source {
                        ui.horizontal(|ui| {
                            let mut secs = |ui: &mut egui::Ui, value: &mut f32, label: &str| {
                                if ui
                                    .add(egui::DragValue::new(value).range(0.001..=10.0).speed(0.005).prefix(format!("{label} ")).suffix(" s"))
                                    .changed()
                                {
                                    changed = true;
                                }
                            };
                            secs(ui, attack, "A");
                            secs(ui, decay, "D");
                            secs(ui, release, "R");
                            if ui
                                .add(egui::DragValue::new(sustain).range(0.0..=1.0).speed(0.01).prefix("S ").custom_formatter(|v, _| format!("{:.0} %", v * 100.0)))
                                .changed()
                            {
                                changed = true;
                            }
                        });
                    }
                    // Ritbar kurva: 16 punkter, klicka och dra i bilden.
                    if let ModSource::Curve { rate_hz, points } = &mut slot.source {
                        if ui
                            .add(egui::DragValue::new(rate_hz).range(0.01..=30.0).speed(0.05).suffix(" Hz"))
                            .changed()
                        {
                            changed = true;
                        }
                        let (rect, resp) = ui.allocate_exact_size(Vec2::new(170.0, 42.0), Sense::click_and_drag());
                        let painter = ui.painter_at(rect);
                        painter.rect_filled(rect, Rounding::same(3.0), crate::ui::theme::palette().bg_deep());
                        for (i, v) in points.iter().enumerate() {
                            let x = rect.min.x + rect.width() * (i as f32 + 0.5) / CURVE_POINTS as f32;
                            let y = rect.center().y - v.clamp(-1.0, 1.0) * rect.height() * 0.45;
                            painter.line_segment(
                                [Pos2::new(x, rect.center().y), Pos2::new(x, y)],
                                Stroke::new(2.0_f32, crate::ui::theme::palette().orange()),
                            );
                        }
                        if (resp.clicked() || resp.dragged())
                            && let Some(pos) = resp.interact_pointer_pos()
                        {
                            let rel = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 0.999);
                            let idx = (rel * CURVE_POINTS as f32) as usize;
                            let v = ((rect.center().y - pos.y) / (rect.height() * 0.5)).clamp(-1.0, 1.0);
                            points[idx.min(CURVE_POINTS - 1)] = v;
                            changed = true;
                        }
                    }
                    // LFO-parametrar bara när källan är en LFO.
                    if let ModSource::Lfo { rate_hz, shape } = &mut slot.source {
                        if ui
                            .add(egui::DragValue::new(rate_hz).range(0.01..=30.0).speed(0.05).suffix(" Hz"))
                            .changed()
                        {
                            changed = true;
                        }
                        egui::ComboBox::from_id_salt(format!("mod_shape_{i}"))
                            .selected_text(crate::i18n::t(shape.label()))
                            .width(84.0)
                            .show_ui(ui, |ui| {
                                for s in LfoShape::ALL {
                                    if ui.selectable_label(*shape == s, crate::i18n::t(s.label())).clicked() {
                                        *shape = s;
                                        changed = true;
                                    }
                                }
                            });
                    }
                    ui.label(egui::RichText::new("→").size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                    egui::ComboBox::from_id_salt(format!("mod_tgt_{i}"))
                        .selected_text(crate::i18n::t(slot.target.label()))
                        .width(104.0)
                        .show_ui(ui, |ui| {
                            for t in ModTarget::ALL {
                                if ui.selectable_label(slot.target == t, crate::i18n::t(t.label())).clicked() {
                                    slot.target = t;
                                    changed = true;
                                }
                            }
                        });
                    if ui
                        .add(egui::Slider::new(&mut slot.amount, -1.0..=1.0).fixed_decimals(2).show_value(true))
                        .changed()
                    {
                        changed = true;
                    }
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.mod_slots.remove(i);
                changed = true;
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::t("➕ Lägg till rad")).clicked() {
                    self.mod_slots.push(ModSlot::new(
                        ModSource::Lfo { rate_hz: 1.0, shape: LfoShape::Sine },
                        ModTarget::Volume,
                        0.2,
                    ));
                    changed = true;
                }
                if ui
                    .button(crate::i18n::t("🧹 Rensa"))
                    .on_hover_text(crate::i18n::t("Ta bort alla rader"))
                    .clicked()
                {
                    self.mod_slots.clear();
                    changed = true;
                }
                ui.label(
                    egui::RichText::new(crate::tstatus!("{} rad(er)", self.mod_slots.len()))
                        .size(Theme::FONT_S)
                        .color(crate::ui::theme::palette().text_muted()),
                );
            });
        });
    self.show_mod_matrix = open;
    if changed {
        self.mod_dirty = true;
    }
    if self.mod_dirty {
        let slots = self.mod_slots.clone();
        let _ = self.engine.send_command(crate::audio::AudioCommand::SetModulation { slots });
        self.mod_dirty = false;
    }
}

/// Ackordspåret: ackord per takt. Snappen flyttar noter till närmaste ackordston, alltså
/// behålls melodins rörelse — den transponeras inte med ett fast intervall.
pub(crate) fn render_chord_track_view(&mut self, ctx: &egui::Context) {
    use crate::ui::app::state::{ChordEvent, ChordQuality};
    if !self.show_chord_track {
        return;
    }
    let mut open = true;
    egui::Window::new(crate::i18n::t("🎼 Ackordspår"))
        .open(&mut open)
        .default_width(400.0)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(crate::i18n::t(
                    "Ackordet gäller från sin takt tills nästa tar över. Noter snappas till närmaste ackordston.",
                ))
                .size(Theme::FONT_S)
                .color(crate::ui::theme::palette().text_muted()),
            );
            ui.separator();
            if ui
                .button(crate::i18n::t("➕ Lägg till i aktuell takt"))
                .on_hover_text(crate::i18n::t("Lägger ett ackord på takten spelhuvudet står på, med projektets grundton"))
                .clicked()
            {
                let bar = self.song_bar;
                let root = self.song_key_root % 12;
                self.song_chords.retain(|c| c.bar != bar);
                self.song_chords.push(ChordEvent { bar, root, quality: ChordQuality::Major });
                self.song_chords.sort_by_key(|c| c.bar);
                self.status_message = crate::tstatus!("🎼 Ackord tillagt i takt {}", bar + 1);
            }
            ui.separator();
            if self.song_chords.is_empty() {
                ui.label(crate::i18n::t("Inga ackord än."));
            }
            let mut remove: Option<usize> = None;
            for (i, chord) in self.song_chords.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{}:{}", i + 1, chord.name())).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                    ui.label(crate::i18n::t("takt"));
                    let mut bar = chord.bar as u32 + 1;
                    if ui.add(egui::DragValue::new(&mut bar).range(1..=9999)).changed() {
                        chord.bar = (bar as usize).saturating_sub(1);
                    }
                    egui::ComboBox::from_id_salt(format!("chord_root_{i}"))
                        .selected_text(crate::ui::app::state::note_name_of(chord.root))
                        .width(52.0)
                        .show_ui(ui, |ui| {
                            for r in 0..12u8 {
                                if ui
                                    .selectable_label(chord.root == r, crate::ui::app::state::note_name_of(r))
                                    .clicked()
                                {
                                    chord.root = r;
                                }
                            }
                        });
                    egui::ComboBox::from_id_salt(format!("chord_q_{i}"))
                        .selected_text(chord.quality.suffix())
                        .width(56.0)
                        .show_ui(ui, |ui| {
                            for q in ChordQuality::ALL {
                                if ui.selectable_label(chord.quality == q, q.suffix()).clicked() {
                                    chord.quality = q;
                                }
                            }
                        });
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                self.song_chords.remove(i);
            }
            ui.separator();
            ui.label(
                egui::RichText::new(crate::tstatus!("{} ackord", self.song_chords.len()))
                    .size(Theme::FONT_S)
                    .color(crate::ui::theme::palette().text_muted()),
            );
        });
    self.show_chord_track = open;
}

/// Sektionskedjan (arranger track): vilken ordning sektionerna spelas i. Samma sektion får
/// förekomma flera gånger — A B A C är hela poängen, inte ett specialfall.
pub(crate) fn render_arranger_chain_view(&mut self, ctx: &egui::Context) {
    if !self.show_arranger_chain {
        return;
    }
    let mut open = true;
    let names: Vec<String> = self.song_markers.iter().map(|m| m.name.clone()).collect();
    egui::Window::new(crate::i18n::t("🧩 Sektionskedja"))
        .open(&mut open)
        .default_width(340.0)
        .show(ctx, |ui| {
            ui.checkbox(
                &mut self.arranger_chain_active,
                crate::i18n::t("Följ kedjan vid uppspelning"),
            );
            ui.label(
                egui::RichText::new(crate::i18n::t(
                    "Kedjan går före loop-punkterna: när en sektion tar slut hoppar spelhuvudet till nästa i kedjan.",
                ))
                .size(Theme::FONT_S)
                .color(crate::ui::theme::palette().text_muted()),
            );
            ui.separator();
            if names.is_empty() {
                ui.label(crate::i18n::t("Inga sektioner — skapa låtstruktur först."));
                return;
            }
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::t("Bygg om från sektionerna")).clicked() {
                    self.arranger_chain = (0..names.len()).collect();
                    self.status_message =
                        crate::tstatus!("🧩 Kedja med {} sektion(er)", names.len());
                }
                if ui.button(crate::i18n::t("Töm kedjan")).clicked() {
                    self.arranger_chain.clear();
                }
            });
            ui.separator();
            let mut move_up: Option<usize> = None;
            let mut move_down: Option<usize> = None;
            let mut remove: Option<usize> = None;
            for (i, sec) in self.arranger_chain.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("{}.", i + 1)).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                    if ui.small_button("▲").on_hover_text(crate::i18n::t("Flytta tidigare i kedjan")).clicked() {
                        move_up = Some(i);
                    }
                    if ui.small_button("▼").on_hover_text(crate::i18n::t("Flytta senare i kedjan")).clicked() {
                        move_down = Some(i);
                    }
                    if ui.small_button("✖").on_hover_text(crate::i18n::t("Ta bort ur kedjan")).clicked() {
                        remove = Some(i);
                    }
                    let label = names
                        .get(*sec)
                        .cloned()
                        .unwrap_or_else(|| crate::i18n::t("(borttagen sektion)").to_string());
                    ui.label(label);
                });
            }
            // Ändringarna görs efter raden, så kedjan inte ändras medan den ritas.
            if let Some(i) = move_up
                && i > 0
                && i < self.arranger_chain.len()
            {
                self.arranger_chain.swap(i - 1, i);
            }
            if let Some(i) = move_down
                && i + 1 < self.arranger_chain.len()
            {
                self.arranger_chain.swap(i, i + 1);
            }
            if let Some(i) = remove
                && i < self.arranger_chain.len()
            {
                self.arranger_chain.remove(i);
            }
            ui.separator();
            egui::ComboBox::from_id_salt("chain_add")
                .selected_text(crate::i18n::t("Lägg till sektion"))
                .width(200.0)
                .show_ui(ui, |ui| {
                    for (k, n) in names.iter().enumerate() {
                        if ui.selectable_label(false, n).clicked() {
                            self.arranger_chain.push(k);
                        }
                    }
                });
            let steps = self.arranger_chain.len();
            ui.label(
                egui::RichText::new(crate::tstatus!("{} steg i kedjan", steps))
                    .size(Theme::FONT_S)
                    .color(crate::ui::theme::palette().text_muted()),
            );
        });
    self.show_arranger_chain = open;
}

pub(crate) fn render_song_structure_modal_view(&mut self, ctx: &egui::Context) {
    let mut apply_sections: Option<Vec<SongSectionItem>> = None;
    let mut status = self.status_message.clone();

    render_song_structure_modal(
        ctx,
        &mut self.show_song_structure_modal,
        &mut self.song_structure_state,
        self.bpm,
        &mut status,
        |sections| {
            apply_sections = Some(sections.to_vec());
        },
    );
    self.status_message = status;

    if let Some(sections) = apply_sections {
        let total_bars: usize = sections.iter().map(|s| s.length_bars).sum();
        self.loop_start_bar = 0;
        self.loop_end_bar = total_bars.max(16);

        self.song_markers.clear();
        let mut bar = 0usize;
        for s in &sections {
            let (_, color) = s.section_type.name_and_color();
            self.song_markers.push(SongMarker {
                start_bar: bar,
                length_bars: s.length_bars,
                name: s.name.clone(),
                color,
            });
            bar += s.length_bars;
        }

        self.status_message = crate::tstatus!("📑 Applicerade låtstruktur med {} sektioner och skapade {} markörer (Totalt {} takter)!", sections.len(), self.song_markers.len(), total_bars);
    }
}
}
