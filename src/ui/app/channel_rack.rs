//! Kanalracket: patterns, steg och mixerns remsor.
//!
//! Utdelad ur `mixer.rs` 2026-11-02; filen var 4 656 rader och blandade
//! fyra ytor med mixerns logik.

use super::*;
use crate::ui::theme::Theme;

impl SonixApp {
pub(crate) fn render_channel_rack(&mut self, ui: &mut egui::Ui) {
    ui.group(|ui| {
        // Pattern Selector Bar (FL Studio Style)
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎹 SONIX CHANNEL RACK")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().orange()));
            ui.separator();

            ui.label(egui::RichText::new(crate::i18n::t("Mönster:")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
            let p_len = self.patterns.len();
            // Many patterns wrap inside the bar; in one `horizontal` they ran past the
            // right edge where the last ones could not be clicked.
            ui.horizontal_wrapped(|ui| {
                for pat_idx in 0..p_len {
                    let is_active = self.selected_pattern == pat_idx;
                    let p_color = self.patterns[pat_idx].color;
                    let fill = if is_active { p_color } else { crate::ui::theme::palette().panel() };
                    let text_color = if is_active { Color32::BLACK } else { crate::ui::theme::palette().text() };
                    let btn_text = format!("{} {}", pat_idx + 1, self.patterns[pat_idx].name);
                    if ui.add(egui::Button::new(egui::RichText::new(btn_text).strong().size(Theme::FONT_S).color(text_color)).fill(fill)).clicked() {
                        self.select_pattern(pat_idx);
                    }
                }
            });

            ui.separator();

            // Swing Knob
            rotary_knob(ui, &mut self.swing, 0.0, 1.0, "SWING", crate::ui::theme::palette().yellow(), 22.0);

            ui.separator();

            if ui.button(egui::RichText::new(crate::i18n::t("➕ Importera Ljud")).color(crate::ui::theme::palette().green())).clicked() {
                self.show_import_modal = true;
            }

            // **Transmitter**: kanalens valda sampelfil blir hörbara händelser på ett nytt spår.
            if ui.button(crate::i18n::t("📡 Transmitter…"))
                .on_hover_text(crate::i18n::t("Gör kanalens valda sampelfil till ett nytt spår med Transmitterns hörbara händelser"))
                .clicked()
            {
                self.add_transmitter_track();
            }

            if ui.button(crate::i18n::t("⚡ Slumpa Beats")).clicked() {
                for ch in &mut self.channels {
                    for s in 0..16 {
                        ch.steps[s] = (s % 4 == 0) || (rand_simple(s * 13 + ch.name.len()) > 0.68);
                    }
                }
                self.sync_active_pattern_from_ui();
            }
            if ui.button(crate::i18n::t("🗑 Rensa Steg")).clicked() {
                for ch in &mut self.channels {
                    ch.steps = [false; crate::ui::app::state::MAX_STEPS];
                }
                self.sync_active_pattern_from_ui();
            }
            if ui.button(crate::i18n::t("🎹 Öppna Piano Roll")).clicked() {
                self.view_mode = ViewMode::PianoRoll;
            }
            if ui.button(crate::i18n::t("🎼 Öppna Arranger")).clicked() {
                self.view_mode = ViewMode::PlaylistArranger;
            }
        });

        ui.add_space(8.0);

        let mut toggle_picker_for = None;
        let mut toggle_chopper_for = None;
        // Högerklicksmenyn på en kanal: (kanal, 0 = mute av/på, 1 = nollställ volym/pan).
        let mut channel_action: Option<(usize, u8)> = None;
        // A picked .sf2 is parked until its preset is chosen, then imported below the rack.
        let mut sf2_open: Option<Sf2Pick> = None;

        for ch_idx in 0..self.channels.len() {
            let is_selected = self.selected_channel == ch_idx;
            let is_picker_open = self.active_sample_picker_channel == Some(ch_idx);
            let is_chopper_open = self.active_chopper_channel == Some(ch_idx);
            let ch = &mut self.channels[ch_idx];

            // The row is ~980 px wide (the 16 steps alone are 416 px) and the dock can be
            // 320 px, so it scrolls sideways instead of losing its last steps past the edge.
            let row_content_w = 2.0 * 18.0 + 2.0 * 24.0 + 200.0 + 42.0 + 52.0 + 4.0
                + 16.0 * 26.0
                + 22.0 * ui.spacing().item_spacing.x;
            crate::ui::widgets::horizontal_scroll_row(ui, ("channel_steps", ch_idx), row_content_w, |ui| {
                // Mute & Solo LEDs
                let mute_color = if ch.muted { crate::ui::theme::palette().bg() } else { crate::ui::theme::palette().green() };
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("M")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(mute_color).min_size(Vec2::new(18.0, 18.0))).on_hover_text(crate::i18n::t("Tysta kanalen (M)")).clicked() {
                    ch.muted = !ch.muted;
                }
                let solo_color = if ch.solo { crate::ui::theme::palette().orange() } else { crate::ui::theme::palette().header() };
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("S")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(solo_color).min_size(Vec2::new(18.0, 18.0))).on_hover_text(crate::i18n::t("Solo: hör bara den här kanalen (S)")).clicked() {
                    ch.solo = !ch.solo;
                }

                // VOL & PAN Knobs
                // Ratten vet sitt eget standardläge: dubbelklick ger tillbaka 80 % och mitten.
                crate::ui::widgets::rotary_knob_reset(ui, &mut ch.volume, 0.0, 1.0, "VOL", ch.color, 24.0, 0.8);
                crate::ui::widgets::rotary_knob_reset(ui, &mut ch.pan, -1.0, 1.0, "PAN", crate::ui::theme::palette().text_muted(), 24.0, 0.0);

                // Channel Select Button
                let btn_fill = if is_selected { crate::ui::theme::palette().panel() } else { crate::ui::theme::palette().panel_alt() };
                // **Namnfältet har fast bredd.** Förut växte knappen med namnet, så "Byt", "Chop" och
                // stegrutorna hamnade olika långt ut i varje rad — racket såg osynkat ut. Nu är
                // varje rad en tabellrad: samma kolumner oavsett namn, långa namn klipps med "…".
                let ch_btn = egui::Button::new(egui::RichText::new(format!("{} {}", ch.icon, ch.name)).strong().color(ch.color))
                    .fill(btn_fill)
                    .wrap_mode(egui::TextWrapMode::Truncate);
                // Ett namn som slutar i "…" ska gå att läsa någonstans — ledtråden bär hela
                // namnet (EGM 3, fynd 19).
                let full_channel_name = format!("{} {}", ch.icon, ch.name);
                let ch_resp = ui
                    .add_sized(Vec2::new(200.0, 26.0), ch_btn)
                    .on_hover_text(full_channel_name);
                if ch_resp.clicked() {
                    self.selected_channel = ch_idx;
                }
                // **Högerklick på kanalen:** ett par åtgärder i stället för att leta i panelen.
                ch_resp.context_menu(|ui| {
                    ui.set_min_width(200.0);
                    let muted = ch.muted;
                    if ui
                        .button(if muted {
                            crate::i18n::t("🔊 Slå på kanalen")
                        } else {
                            crate::i18n::t("🔇 Tysta kanalen")
                        })
                        .clicked()
                    {
                        channel_action = Some((ch_idx, 0));
                        ui.close_menu();
                    }
                    if ui
                        .button(crate::i18n::t("🎚 Nollställ volym och pan"))
                        .on_hover_text(crate::i18n::t("Volym 80 %, pan i mitten"))
                        .clicked()
                    {
                        channel_action = Some((ch_idx, 1));
                        ui.close_menu();
                    }
                });

                // Quick Switch Sound Button [🔄 Byt]
                let byt_fill = if is_picker_open { crate::ui::theme::palette().cyan() } else { crate::ui::theme::palette().panel_alt() };
                let byt_text_color = if is_picker_open { Color32::BLACK } else { crate::ui::theme::palette().cyan() };
                let byt_btn = egui::Button::new(egui::RichText::new(crate::i18n::t("🔄 Byt")).size(Theme::FONT_S).color(byt_text_color))
                    .fill(byt_fill)
                    .min_size(Vec2::new(42.0, 24.0));
                if ui.add(byt_btn).on_hover_text(crate::i18n::t("Byt ut detta beat/ljud direkt från biblioteket")).clicked() {
                    toggle_picker_for = Some(ch_idx);
                }

                // Chop & Pitch Inspector Button [✂ Chop]
                let chop_fill = if is_chopper_open { crate::ui::theme::palette().yellow() } else { crate::ui::theme::palette().header() };
                let chop_text_color = if is_chopper_open { Color32::BLACK } else { crate::ui::theme::palette().yellow() };
                let chop_btn = egui::Button::new(egui::RichText::new(crate::i18n::t("✂ Chop")).size(Theme::FONT_S).color(chop_text_color))
                    .fill(chop_fill)
                    .min_size(Vec2::new(52.0, 24.0));
                if ui.add(chop_btn).on_hover_text(crate::i18n::t("Öppna Waveform Chopper & Pitch Slicer")).clicked() {
                    toggle_chopper_for = Some(ch_idx);
                }

                ui.add_space(4.0);

                // 16 FL-Studio Style Tactile Step Buttons
                for step_idx in 0..16 {
                    let is_active = ch.steps[step_idx];
                    let is_playhead = self.is_playing && self.current_step == step_idx;
                    let beat_group_a = (step_idx / 4) % 2 == 0;

                    let resp = fl_step_button(
                        ui,
                        is_active,
                        is_playhead,
                        beat_group_a,
                        ch.color,
                        Vec2::new(26.0, 24.0),
                    );

                    if resp.clicked() {
                        ch.steps[step_idx] = !is_active;
                    }
                }
            });
            ui.add_space(3.0);
        }

        if let Some(ch_idx) = toggle_picker_for {
            self.active_sample_picker_channel = if self.active_sample_picker_channel == Some(ch_idx) { None } else { Some(ch_idx) };
        }
        if let Some(ch_idx) = toggle_chopper_for {
            self.active_chopper_channel = if self.active_chopper_channel == Some(ch_idx) { None } else { Some(ch_idx) };
        }
        if let Some((ch_idx, what)) = channel_action
            && let Some(ch) = self.channels.get_mut(ch_idx)
        {
            self.status_message = match what {
                0 => {
                    ch.muted = !ch.muted;
                    if ch.muted {
                        crate::tstatus!("🔇 {} tystad", ch.name)
                    } else {
                        crate::tstatus!("🔊 {} på igen", ch.name)
                    }
                }
                _ => {
                    ch.volume = 0.8;
                    ch.pan = 0.0;
                    crate::tstatus!("🎚 {}: volym 80 %, pan i mitten", ch.name)
                }
            };
        }

        // Interactive Sound Library Picker Overlay
        if let Some(picker_ch) = self.active_sample_picker_channel {
            ui.add_space(6.0);
            ui.group(|ui| {
                let cur_name = self.channels.get(picker_ch).map(|c| c.name.clone()).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::tstatus!("📚 BYT LJUD: Välj sample för Kanal {} ({})", picker_ch + 1, cur_name)).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().cyan()));
                    ui.separator();
                    if ui.button(egui::RichText::new(crate::i18n::t("➕ Importera Eget Ljud")).color(crate::ui::theme::palette().green())).clicked() {
                        self.show_import_modal = true;
                    }
                    if ui.button(egui::RichText::new(crate::i18n::t("✖ Stäng Väljare")).color(crate::ui::theme::palette().red())).clicked() {
                        self.active_sample_picker_channel = None;
                    }
                });

                ui.add_space(4.0);

                // Category Filter Tabs
                let categories = ["Alla", "Kicks", "Snares", "Claps", "Hi-Hats", "Percussion", "Vocal Chops", "808 & Bass", "Egna Importerade"];
                ui.horizontal_wrapped(|ui| {
                    for (cat_i, &cat_name) in categories.iter().enumerate() {
                        let is_sel = self.selected_lib_category == cat_i;
                        let fill = if is_sel { crate::ui::theme::palette().cyan() } else { crate::ui::theme::palette().panel() };
                        let text_col = if is_sel { Color32::BLACK } else { crate::ui::theme::palette().text() };
                        if ui.add(egui::Button::new(egui::RichText::new(cat_name).strong().size(Theme::FONT_S).color(text_col)).fill(fill)).clicked() {
                            self.selected_lib_category = cat_i;
                        }
                    }
                });

                ui.add_space(4.0);

                // Sample Items Grid
                let sel_cat = categories[self.selected_lib_category.min(categories.len() - 1)];
                let items_to_show: Vec<LibrarySampleItem> = self.sample_library.iter()
                    .filter(|item| sel_cat == "Alla" || item.category == sel_cat)
                    .cloned()
                    .collect();

                let mut chosen_item = None;
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    egui::Grid::new("sample_picker_grid").num_columns(4).spacing([8.0, 8.0]).show(ui, |ui| {
                        for (i, item) in items_to_show.iter().enumerate() {
                            ui.group(|ui| {
                                ui.set_width(190.0);
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(&item.icon).size(Theme::FONT_L));
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(&item.name).strong().size(Theme::FONT_S).color(item.color));
                                        ui.label(egui::RichText::new(&item.category).size(Theme::FONT_XS).color(crate::ui::theme::palette().text_muted()));
                                    });
                                });

                                 // Mini wave preview
                                let (wf_rect, _) = ui.allocate_exact_size(Vec2::new(175.0, 18.0), egui::Sense::hover());
                                ui.painter().rect_filled(wf_rect, Rounding::same(2.0), crate::ui::theme::palette().bg());
                                let mid_y = wf_rect.center().y;
                                let num_pts = item.waveform.len().max(1);
                                let dx = wf_rect.width() / num_pts as f32;
                                for (wi, &val) in item.waveform.iter().enumerate() {
                                    let sx = wf_rect.min.x + wi as f32 * dx + dx * 0.5;
                                    let h = (val.abs() * 7.5).max(1.0);
                                    ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.3_f32, item.color));
                                }

                                ui.horizontal(|ui| {
                                    if ui.button(egui::RichText::new(crate::i18n::t("▶ Prov")).size(Theme::FONT_XS)).on_hover_text(crate::i18n::t("Provspela med riktigt ljud")).clicked() {
                                        self.audition_library_sample(item);
                                    }
                                    if ui.button(egui::RichText::new("🖐").size(Theme::FONT_XS)).on_hover_text(crate::i18n::t("Scratcha samplet fram och tillbaka")).clicked() {
                                        self.scratch_library_sample(item);
                                    }
                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✅ Välj")).strong().size(Theme::FONT_XS).color(Color32::BLACK)).fill(crate::ui::theme::palette().green())).clicked() {
                                        chosen_item = Some(item.clone());
                                    }
                                });
                            });

                            if (i + 1) % 4 == 0 {
                                ui.end_row();
                            }
                        }
                    });
                });

                if let Some(item) = chosen_item {
                    self.select_sound_for_channel(picker_ch, &item);
                    self.active_sample_picker_channel = None;
                }
            });
        }

        // Interactive Sample Chopper & Pitch Transpose Inspector
        if let Some(chop_ch) = self.active_chopper_channel
            && chop_ch < self.channels.len() {
                ui.add_space(6.0);
                ui.group(|ui| {
                    let ch = &mut self.channels[chop_ch];
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(format!("✂ SAMPLE CHOPPER & PITCH INSPECTOR - Kanal {} ({} {})", chop_ch + 1, ch.icon, ch.name)).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().yellow()));
                        ui.separator();
                        if ui.button(egui::RichText::new(crate::i18n::t("▶ Provspela Chop")).color(crate::ui::theme::palette().green())).clicked() {
                            let note = (ch.notes[0] as i16 + ch.pitch_semitones as i16).clamp(0, 127) as u8;
                            let base_freq = midi_to_freq(note);
                            let fine_mul = (ch.pitch_fine_cents / 1200.0).exp2();
                            let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq: base_freq * fine_mul, velocity: ch.volume, channel: 0 });
                        }
                        if ui.button(egui::RichText::new(crate::i18n::t("✖ Stäng Chopper")).color(crate::ui::theme::palette().red())).clicked() {
                            self.active_chopper_channel = None;
                        }
                    });

                    ui.add_space(4.0);

                    ui.horizontal(|ui| {
                        // Waveform Chopper Slicer
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Waveform Chop & Slice Trim:")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            let (wf_rect, _) = ui.allocate_exact_size(Vec2::new(380.0, 65.0), egui::Sense::click_and_drag());
                            ui.painter().rect_filled(wf_rect, Rounding::same(3.0), crate::ui::theme::palette().bg_deep());
                            ui.painter().rect_stroke(wf_rect, Rounding::same(3.0), Stroke::new(1.0_f32, crate::ui::theme::palette().border()));

                            let mid_y = wf_rect.center().y;
                            let num_pts = ch.waveform_preview.len().max(1);
                            let dx = wf_rect.width() / num_pts as f32;
                            for (wi, &val) in ch.waveform_preview.iter().enumerate() {
                                let sx = wf_rect.min.x + wi as f32 * dx + dx * 0.5;
                                let h = (val.abs() * 26.0).max(1.0);
                                ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.8_f32, ch.color));
                            }

                            let start_x = wf_rect.min.x + ch.sample_start * wf_rect.width();
                            let end_x = wf_rect.min.x + ch.sample_end * wf_rect.width();

                            if ch.sample_start > 0.0 {
                                let dim_rect = Rect::from_min_max(wf_rect.min, Pos2::new(start_x, wf_rect.max.y));
                                ui.painter().rect_filled(dim_rect, Rounding::ZERO, Color32::from_black_alpha(150));
                            }
                            if ch.sample_end < 1.0 {
                                let dim_rect = Rect::from_min_max(Pos2::new(end_x, wf_rect.min.y), wf_rect.max);
                                ui.painter().rect_filled(dim_rect, Rounding::ZERO, Color32::from_black_alpha(150));
                            }

                            // Slicekartan (Fas 8.7): varje hittad gräns som ett tunt
                            // streck. Den aktiva slicens ram ritas ovanpå.
                            for (si, (s0, _s1)) in ch.slices.iter().enumerate() {
                                if si == 0 {
                                    continue;
                                }
                                let x = wf_rect.min.x + s0 * wf_rect.width();
                                ui.painter().line_segment(
                                    [Pos2::new(x, wf_rect.min.y), Pos2::new(x, wf_rect.max.y)],
                                    Stroke::new(1.0_f32, crate::ui::theme::palette().cyan()),
                                );
                            }

                            let active_slice_rect = Rect::from_min_max(Pos2::new(start_x, wf_rect.min.y), Pos2::new(end_x, wf_rect.max.y));
                            ui.painter().rect_stroke(active_slice_rect, Rounding::ZERO, Stroke::new(1.5_f32, crate::ui::theme::palette().yellow()));
                            ui.painter().line_segment([Pos2::new(start_x, wf_rect.min.y), Pos2::new(start_x, wf_rect.max.y)], Stroke::new(2.5_f32, crate::ui::theme::palette().green()));
                            ui.painter().line_segment([Pos2::new(end_x, wf_rect.min.y), Pos2::new(end_x, wf_rect.max.y)], Stroke::new(2.5_f32, crate::ui::theme::palette().red()));

                            // **Nudge: dra en gräns** (Fas 8.7 steg 2). Gränsen ligger
                            // mellan två slicar, så `nudge_slice_boundary` flyttar **båda** —
                            // flyttas bara den ena uppstår ett glapp (tyst i slicen) eller
                            // ett överlapp (samma ljud två gånger). Filens kanter (den
                            // första och den sista) ritas men går inte att dra: en karta som
                            // inte täcker hela filen spelar inte hela filen längre.
                            //
                            // Minsta slice i kartans enhet (andel av filen): 0,5 %. Den
                            // klämningen finns för att en gräns inte ska kunna korsa sin
                            // granne — detektionens 30 ms är en annan regel (när två slag
                            // slås ihop) och blandas inte in här.
                            const MIN_SLICE: f32 = 0.005;
                            for bi in 1..ch.slices.len() {
                                let bx = wf_rect.min.x + ch.slices[bi].0 * wf_rect.width();
                                let zone = Rect::from_min_max(
                                    Pos2::new(bx - 3.0, wf_rect.min.y),
                                    Pos2::new(bx + 3.0, wf_rect.max.y),
                                );
                                let resp = ui.interact(
                                    zone,
                                    egui::Id::new(("slice-nudge", bi)),
                                    egui::Sense::drag(),
                                );
                                if resp.dragged() {
                                    let delta = resp.drag_delta().x / wf_rect.width();
                                    if delta != 0.0 {
                                        ch.slices = crate::audio::onset::nudge_slice_boundary(
                                            &ch.slices, bi, delta, MIN_SLICE,
                                        );
                                    }
                                }
                                if resp.hovered() || resp.dragged() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                                }
                            }

                            // **Loopens punkter går att dra** (Fas 8.4/7). Regeln bor i
                            // `command::move_loop_point`: punkten kläms till filen och
                            // **stannar** en minsta loop från sin granne. Utan den klämningen
                            // kunde ett drag skapa ett bakvänt par — och motorn läser ett
                            // bakvänt par som "ingen loop", så loopen hade **försvunnit mitt i
                            // ett drag**. Markörerna ritas och interageras **efter**
                            // slicekartans gränser, för de ligger ovanpå dem och ska vinna.
                            //
                            // Reglaget finns kvar (Loopstart/Loopslut nedanför): samma två tal,
                            // två dörrar — ett drag för att leta, en siffra för att sätta exakt.
                            if ch.loop_mode != crate::audio::LoopMode::Off {
                                let ls_x = wf_rect.min.x + ch.sample_loop_start * wf_rect.width();
                                let le_x = wf_rect.min.x + ch.sample_loop_end * wf_rect.width();
                                let loop_rect = Rect::from_min_max(
                                    Pos2::new(ls_x.min(le_x), wf_rect.min.y),
                                    Pos2::new(ls_x.max(le_x), wf_rect.max.y),
                                );
                                ui.painter().rect_filled(
                                    loop_rect,
                                    Rounding::ZERO,
                                    crate::ui::theme::Theme::alpha(crate::ui::theme::palette().cyan(), 30),
                                );
                                for (pi, is_start) in [(0usize, true), (1usize, false)] {
                                    let (px, value, other) = if is_start {
                                        (ls_x, ch.sample_loop_start, ch.sample_loop_end)
                                    } else {
                                        (le_x, ch.sample_loop_end, ch.sample_loop_start)
                                    };
                                    ui.painter().line_segment(
                                        [
                                            Pos2::new(px, wf_rect.min.y),
                                            Pos2::new(px, wf_rect.max.y),
                                        ],
                                        Stroke::new(2.0_f32, crate::ui::theme::palette().cyan()),
                                    );
                                    let zone = Rect::from_min_max(
                                        Pos2::new(px - 4.0, wf_rect.min.y),
                                        Pos2::new(px + 4.0, wf_rect.max.y),
                                    );
                                    let resp = ui.interact(
                                        zone,
                                        egui::Id::new(("loop-point", pi)),
                                        egui::Sense::drag(),
                                    );
                                    if resp.dragged() {
                                        let delta = resp.drag_delta().x / wf_rect.width();
                                        if delta != 0.0 {
                                            let moved = crate::audio::command::move_loop_point(
                                                value + delta,
                                                is_start,
                                                other,
                                            );
                                            if is_start {
                                                ch.sample_loop_start = moved;
                                            } else {
                                                ch.sample_loop_end = moved;
                                            }
                                        }
                                    }
                                    if resp.hovered() || resp.dragged() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                                        resp.on_hover_text(crate::i18n::t("Dra looppunkten. Den stannar en minsta loop från den andra punkten — en loop som försvann mitt i ett drag vore ingen interaktion."));
                                    }
                                }
                            }

                            // Quick Slicer Presets
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("Chop Snabbval:")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                                // Samma ledtråd på alla sju: knapparna väljer **vilken del** av
                                // samplet som spelas, och etiketten säger redan vilken.
                                let chop_hint = crate::i18n::t("Spela bara den här delen av samplet");
                                if ui.button(crate::i18n::t("Fullt")).on_hover_text(chop_hint).clicked() { ch.sample_start = 0.0; ch.sample_end = 1.0; }
                                if ui.button(crate::i18n::t("1/2")).on_hover_text(chop_hint).clicked() { ch.sample_start = 0.0; ch.sample_end = 0.5; }
                                if ui.button(crate::i18n::t("2/2")).on_hover_text(chop_hint).clicked() { ch.sample_start = 0.5; ch.sample_end = 1.0; }
                                if ui.button(crate::i18n::t("1/4")).on_hover_text(chop_hint).clicked() { ch.sample_start = 0.0; ch.sample_end = 0.25; }
                                if ui.button(crate::i18n::t("2/4")).on_hover_text(chop_hint).clicked() { ch.sample_start = 0.25; ch.sample_end = 0.5; }
                                if ui.button(crate::i18n::t("3/4")).on_hover_text(chop_hint).clicked() { ch.sample_start = 0.50; ch.sample_end = 0.75; }
                                if ui.button(crate::i18n::t("4/4")).on_hover_text(chop_hint).clicked() { ch.sample_start = 0.75; ch.sample_end = 1.0; }
                            });

                            // Slagletning (Fas 8.7). Knappen som förut hette
                            // "⚡ Transient" satte bara slutet till 18 % av filen —
                            // ett påhittat svar på en riktig fråga. Nu letas slagen
                            // i filen, och resultatet är en slicekarta.
                            ui.horizontal(|ui| {
                                let mut detected: Option<(Vec<(f32, f32)>, usize)> = None;
                                let mut message: Option<String> = None;
                                if ui
                                    .button(egui::RichText::new(crate::i18n::t("🔍 Hitta slag")).color(crate::ui::theme::palette().cyan()))
                                    .on_hover_text(crate::i18n::t("Leta transients i filen och dela den i slicar"))
                                    .clicked()
                                {
                                    match ch.pcm_audio.as_ref() {
                                        // 8.5-regeln: går ljudet inte att läsa säger vi det.
                                        None => {
                                            message = Some(crate::i18n::t("⚠ Kanalens ljud går inte att läsa — inga slag kan hittas").to_string());
                                        }
                                        Some((left, _right, sr)) => {
                                            // **En regel, två detektorer** (Fas 8.7 steg 2):
                                            // kroppen bor i `detect_slice_map`, så båda
                                            // knapparna går genom samma väg — valet byter
                                            // detektor och inget annat. Att skriva av
                                            // blocket en gång per detektor hade varit två
                                            // vägar till samma sak.
                                            match crate::audio::onset::detect_slice_map(
                                                left,
                                                *sr as f32,
                                                false, // höljesdetektorn: rätt för trummor
                                                &crate::audio::onset::OnsetParams::default(),
                                            ) {
                                                Some((map, count)) => detected = Some((map, count)),
                                                None => message = Some(crate::i18n::t("Inga slag hittades i filen").to_string()),
                                            }
                                        }
                                    }
                                }
                                // **Andra dörren in i samma regel**: spektral flux. Kroppen
                                // är densamma, bara detektorn skiljer — alltså inte en andra
                                // väg, bara ett andra val.
                                if ui
                                    .button(crate::i18n::t("🔍 Slagletning (spektral)"))
                                    .on_hover_text(crate::i18n::t(
                                        "Samma sak, men letar klangförändring i stället för nivå — starkare på melodiöst material och sång, onödigt för rena trumloopar.",
                                    ))
                                    .clicked()
                                {
                                    match ch.pcm_audio.as_ref() {
                                        None => {
                                            message = Some(crate::i18n::t("⚠ Kanalens ljud går inte att läsa — inga slag").to_string())
                                        }
                                        Some((left, _right, sr)) => {
                                            match crate::audio::onset::detect_slice_map(
                                                left,
                                                *sr as f32,
                                                true, // spektral flux
                                                &crate::audio::onset::OnsetParams::default(),
                                            ) {
                                                Some((map, count)) => detected = Some((map, count)),
                                                None => message = Some(crate::i18n::t("Inga slag hittades i filen").to_string()),
                                            }
                                        }
                                    }
                                }
                                if let Some(msg) = message {
                                    self.status_message = msg;
                                }
                                if let Some((slices, onsets)) = detected {
                                    ch.sample_start = slices[0].0;
                                    ch.sample_end = slices[0].1;
                                    ch.slices = slices;
                                    self.status_message = crate::tstatus!(
                                        "🔍 {} slag hittade — {} slicar (den första spelas nu)",
                                        onsets,
                                        ch.slices.len()
                                    );
                                }
                                // **Dumpa slicarna till stegraden** (Fas 8.7 steg 2). En not
                                // per slice, kromatiskt från basnoten — samma adressering som
                                // `window_for_note` redan spelar efter, alltså behövs ingen
                                // motorändring: noten *är* adressen. Fler slicar än steg
                                // kapas, och det sägs rakt ut i stället för att tigas.
                                // **Dumpa slicarna till piano rollen** (Fas 8.7 steg 2).
                                // Samma sak mot ett annat rutnät: `piano_roll_grid` är
                                // `[[bool; 16]; 24]`, alltså samma form som stegraden med 24
                                // rader i stället för 16 — så samma `slices_to_steps` duger,
                                // med radantalet som tak. Raden en slice hamnar på är
                                // **slicens index**, precis som noten är `bas + index`; därför
                                // slås inga slicar ihop, och adresseringen stämmer fortfarande
                                // med `window_for_note`. (Att folda noten med
                                // `note_to_roll_offset` vore fel här: den är gjord för *spelade*
                                // noter, och en foldning hade lagt två slicar på samma rad.)
                                if ui
                                    .button(crate::i18n::t("⬇ Slicar → piano roll"))
                                    .on_hover_text(crate::i18n::t(
                                        "Lägger en not per slice i piano rollen, kromatiskt från basnoten. Fler slicar än rutnätet rymmer kapas.",
                                    ))
                                    .clicked()
                                {
                                    // **Rutnätet har två axlar, och de betyder olika saker:**
                                    // raderna är tonhöjd och stegen är tid. Slice `i` hamnar
                                    // därför på **steg i** i **rad i** — tid och tonhöjd är båda
                                    // slicens nummer — och taket är det *mindre* av de två
                                    // antalen, inte radantalet. (Första försöket tog radantalet
                                    // och lappade tiden med `% 16`; det hade lagt slice 17 på
                                    // samma steg som slice 1.)
                                    let step_count = self
                                        .patterns
                                        .get(self.selected_pattern)
                                        .map_or(0, |p| p.piano_roll_grid.first().map_or(0, |row| row.len()));
                                    let row_count = self
                                        .patterns
                                        .get(self.selected_pattern)
                                        .map_or(0, |p| p.piano_roll_grid.len());
                                    let capacity = crate::audio::onset::grid_capacity(row_count, step_count);
                                    let dumped = crate::audio::onset::slices_to_steps(
                                        &ch.slices,
                                        ch.sample_base_note,
                                        capacity,
                                    );
                                    if dumped.is_empty() {
                                        self.status_message = crate::i18n::t(
                                            "⚠ Inga slicar att dumpa — kör en slagletning först.",
                                        )
                                        .to_string();
                                    } else if let Some(pattern) =
                                        self.patterns.get_mut(self.selected_pattern)
                                    {
                                        let skipped = ch.slices.len().saturating_sub(dumped.len());
                                        // Rutnätet töms för de rader dumpen rör, så en
                                        // gammal dump inte ligger kvar och pekar på slicar
                                        // som inte längre finns.
                                        for row in pattern.piano_roll_grid.iter_mut() {
                                            row.fill(false);
                                        }
                                        // Steg och rad är båda slicens nummer: slicarna hamnar
                                        // i ordning i tiden, och adresseringen (`bas + i`
                                        // spelar slice `i`) stämmer fortfarande.
                                        for (i, _note) in &dumped {
                                            pattern.piano_roll_grid[*i][*i] = true;
                                        }
                                        self.status_message = if skipped > 0 {
                                            crate::tstatus!(
                                                "⬇ {} slicar till piano rollen ({} kapades — rutnätet rymmer {}).",
                                                dumped.len(),
                                                skipped,
                                                capacity
                                            )
                                        } else {
                                            crate::tstatus!("⬇ {} slicar till piano rollen.", dumped.len())
                                        };
                                    }
                                }
                                if ui
                                    .button(crate::i18n::t("⬇ Slicar → steg"))
                                    .on_hover_text(crate::i18n::t(
                                        "Lägger en not per slice på stegraden, kromatiskt från basnoten. Fler slicar än steg kapas.",
                                    ))
                                    .clicked()
                                {
                                    let dumped = crate::audio::onset::slices_to_steps(
                                        &ch.slices,
                                        ch.sample_base_note,
                                        ch.steps.len(),
                                    );
                                    if dumped.is_empty() {
                                        self.status_message = crate::i18n::t(
                                            "⚠ Inga slicar att dumpa — kör en slagletning först.",
                                        )
                                        .to_string();
                                    } else {
                                        let skipped = ch.slices.len().saturating_sub(dumped.len());
                                        // Raden byggs om från grunden: en dump beskriver
                                        // **hela** kartan, så gamla steg kan inte ligga kvar
                                        // och peka på slicar som inte längre är med.
                                        // `fill` i stället för ett magiskt 16: radens
                                        // längd står i typen, och då kan talet inte driva isär.
                                        ch.steps.fill(false);
                                        for (step, note) in &dumped {
                                            ch.steps[*step] = true;
                                            ch.notes[*step] = *note;
                                        }
                                        self.status_message = if skipped > 0 {
                                            crate::tstatus!(
                                                "⬇ {} slicar till stegraden ({} kapades — raden har {} steg).",
                                                dumped.len(),
                                                skipped,
                                                ch.steps.len()
                                            )
                                        } else {
                                            crate::tstatus!("⬇ {} slicar till stegraden.", dumped.len())
                                        };
                                    }
                                }
                                if !ch.slices.is_empty() {
                                    ui.label(
                                        egui::RichText::new(format!("{} {}", ch.slices.len(), crate::i18n::t("slicar")))
                                            .size(Theme::FONT_S)
                                            .color(crate::ui::theme::palette().text_muted()),
                                    );
                                }
                            });

                            // Slicarna som knappar: att välja en sätter trimfönstret,
                            // och trimfönstret är vad provspelningen spelar.
                            if !ch.slices.is_empty() {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(
                                        egui::RichText::new(crate::i18n::t("Slicar (klicka för att höra):"))
                                            .size(Theme::FONT_S)
                                            .color(crate::ui::theme::palette().text_muted()),
                                    );
                                    let active = (0..ch.slices.len()).find(|&i| {
                                        (ch.sample_start - ch.slices[i].0).abs() < 1e-4
                                            && (ch.sample_end - ch.slices[i].1).abs() < 1e-4
                                    });
                                    for i in 0..ch.slices.len() {
                                        if ui.selectable_label(active == Some(i), format!("{}", i + 1)).clicked() {
                                            ch.sample_start = ch.slices[i].0;
                                            ch.sample_end = ch.slices[i].1;
                                        }
                                    }
                                    if ui.button(crate::i18n::t("Rensa slicar")).clicked() {
                                        ch.slices.clear();
                                    }
                                });
                            }
                        });

                        ui.separator();

                        // Pitch & Envelope Controls
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Pitch & Tonhöjd:")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Semitoner:"));
                                ui.add(egui::Slider::new(&mut ch.pitch_semitones, -24..=24).text("st"));
                                if ui.button(crate::i18n::t("0")).clicked() { ch.pitch_semitones = 0; }
                            });
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Fine Cents:"));
                                ui.add(egui::Slider::new(&mut ch.pitch_fine_cents, -100.0..=100.0).text("ct"));
                                if ui.button(crate::i18n::t("0")).clicked() { ch.pitch_fine_cents = 0.0; }
                            });

                            let total_cents = ch.pitch_semitones as f32 * 100.0 + ch.pitch_fine_cents;
                            let semitone_text = if ch.pitch_semitones >= 0 { format!("+{}", ch.pitch_semitones) } else { format!("{}", ch.pitch_semitones) };
                            ui.label(egui::RichText::new(format!("Totalt: {} st ({:+.0} ct)", semitone_text, total_cents)).color(crate::ui::theme::palette().cyan()).size(Theme::FONT_S));

                            ui.separator();

                            ui.label(egui::RichText::new(crate::i18n::t("Klipp & Sampler:")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Start Trim:"));
                                ui.add(egui::Slider::new(&mut ch.sample_start, 0.0..=1.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("End Trim:"));
                                ui.add(egui::Slider::new(&mut ch.sample_end, 0.0..=1.0));
                            });
                            ui.checkbox(&mut ch.is_reverse, "🔄 Reverse Waveform");

                            // **Samplern** (Fas 8.4). Före den här panelen fanns en
                            // "Attack / Decay"-ratt som *ingen* DSP läste — fältet fanns,
                            // reglaget syntes, och ingenting hördes. Den är ersatt av en
                            // riktig envelop, och det gamla värdet flyttas in i attacken
                            // när ett projekt från förr öppnas (se `from_saved`).
                            ui.separator();
                            ui.label(egui::RichText::new(crate::i18n::t("Loop:")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            ui.vertical(|ui| {
                                for (mode, label) in [
                                    (crate::audio::LoopMode::Off, crate::i18n::t("Av (en skott)")),
                                    (crate::audio::LoopMode::UntilRelease, crate::i18n::t("Till not-av")),
                                    (crate::audio::LoopMode::Forever, crate::i18n::t("För evigt")),
                                ] {
                                    if ui
                                        .radio_value(&mut ch.loop_mode, mode, label)
                                        .on_hover_text(crate::i18n::t("Av = spela en gång. Till not-av = loopen går medan noten hålls och resten efter loopen spelas vid not-av. För evigt = loopen fortsätter även efter not-avet."))
                                        .changed()
                                    {
                                        // En ny loop utan punkter vore en loop ingen hör:
                                        // sätt ett hörbart spann (sista fjärdedelen) första
                                        // gången ett läge slås på.
                                        if mode != crate::audio::LoopMode::Off
                                            && ch.sample_loop_end <= ch.sample_loop_start + 0.01
                                        {
                                            ch.sample_loop_start = 0.75;
                                            ch.sample_loop_end = 1.0;
                                        }
                                    }
                                }
                                let looping = ch.loop_mode != crate::audio::LoopMode::Off;
                                ui.add_enabled_ui(looping, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(crate::i18n::t("Loopstart:"));
                                        ui.add(egui::Slider::new(&mut ch.sample_loop_start, 0.0..=1.0));
                                    });
                                    ui.horizontal(|ui| {
                                        ui.label(crate::i18n::t("Loopslut:"));
                                        ui.add(egui::Slider::new(&mut ch.sample_loop_end, 0.0..=1.0));
                                    });
                                    // Ett bakvänt par är ingen loop — säg det i stället för
                                    // att tiga, för motorn spelar då som en en-skottsprovare.
                                    if looping && ch.sample_loop_end <= ch.sample_loop_start {
                                        ui.label(
                                            egui::RichText::new(crate::i18n::t("⚠ Loopslut måste ligga efter loopstart — nu spelas ljudet som en skott."))
                                                .color(crate::ui::theme::palette().yellow())
                                                .size(Theme::FONT_S),
                                        );
                                    }
                                    ui.checkbox(&mut ch.ping_pong, crate::i18n::t("↔ Ping-pong (vänd i ändarna)"));
                                });
                            });

                            // **Keymap: multi-samples** (Fas 8.4/7). Ordningen i listan **är**
                            // prioriteten (den sista zonen som matchar noten vinner), så den står
                            // här i stället för i en hjälptext någon annanstans.
                            ui.separator();
                            ui.label(egui::RichText::new(crate::i18n::t("Keymap (multi-samples):")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            if ui
                                .button(crate::i18n::t("➕ Lägg till zon av kanalens sampel"))
                                .on_hover_text(crate::i18n::t("Zonen får kanalens sampel och grundton och täcker alla toner och anslag — snäva in intervallen sedan. Den sista zonen i listan som matchar noten är den som hörs."))
                                .clicked()
                            {
                                let mut z = crate::audio::keymap::SampleZone::full_range(ch.sample_base_note);
                                z.sample_path = ch.sample_path.clone();
                                z.pcm = ch.pcm_audio.clone();
                                ch.zones.push(z);
                            }
                            if ui
                                .button(crate::i18n::t("📥 Importera SoundFont (.sf2)"))
                                .on_hover_text(crate::i18n::t("Läser instrumentets zoner ur filen och lägger dem i keymappen (FL:s Soundfont Player). Preset- och modulationslagret läses inte, men samplets loop-punkter följer med zonen."))
                                .clicked()
                                && let Some(p) = rfd::FileDialog::new()
                                    .add_filter("SoundFont", &["sf2"])
                                    .pick_file()
                            {
                                // Nothing is imported here: the preset is picked in the
                                // panel below, so the file's presets are not ignored.
                                let read = std::fs::read(&p)
                                    .map_err(|e| e.to_string())
                                    .and_then(|b| crate::audio::sf2::parse(&b));
                                match read {
                                    Ok(sf) => {
                                        sf2_open = Some(Sf2Pick {
                                            channel: chop_ch,
                                            path: p.display().to_string(),
                                            selected: 0,
                                            sf,
                                        });
                                    }
                                    Err(e) => {
                                        self.status_message = crate::tstatus!(
                                            "⚠️ Kunde inte läsa '{}': {}",
                                            p.display(),
                                            e
                                        );
                                    }
                                }
                            }
                            if ui
                                .button(crate::i18n::t("📥 Importera SFZ"))
                                .on_hover_text(crate::i18n::t("Läser zoner ur en SFZ-fil (region/group/global). Samplevägarna ligger relativt .sfz-filen; ett saknat sampel hoppas över och de andra zonerna laddas. Anslags- och filteropkoderna läses inte."))
                                .clicked()
                                && let Some(p) = rfd::FileDialog::new()
                                    .add_filter("SFZ", &["sfz"])
                                    .pick_file()
                            {
                                match crate::audio::sfz::load(&p.to_string_lossy()) {
                                    Ok(inst) => {
                                        let count = inst.zones.len();
                                        let name = inst.name;
                                        ch.zones.extend(inst.zones);
                                        self.status_message = crate::tstatus!(
                                            "🎹 '{}': {} zoner in i keymappen",
                                            name,
                                            count
                                        );
                                    }
                                    Err(e) => {
                                        self.status_message = crate::tstatus!(
                                            "⚠️ Kunde inte läsa '{}': {}",
                                            p.display(),
                                            e
                                        );
                                    }
                                }
                            }
                            // **Instrumentbiblioteket** (`~/.local/share/sonix/instruments/`):
                            // ett ställe för allt spelbart — inbyggt, JSFX, SF2 och SFZ.
                            // Plugins listas men laddas från plugin-fliken: de kräver en
                            // instans per spår, vilket `ch` inte kan göra härifrån.
                            ui.collapsing(crate::i18n::t("📚 Instrumentbibliotek"), |ui| {
                                let entries = crate::audio::instrument_library::scan();
                                for entry in entries {
                                    use crate::audio::instrument_library::InstrumentKind;
                                    let label = format!("{} ({})", entry.name, entry.format);
                                    let resp = ui.button(label).on_hover_text(&entry.path);
                                    if !resp.clicked() {
                                        continue;
                                    }
                                    match entry.kind {
                                        InstrumentKind::Builtin | InstrumentKind::JsFx => {
                                            // Built on this thread; the audio thread swaps it.
                                            match crate::audio::command::build_instrument(
                                                Some(&entry.path),
                                                self.engine.sample_rate as f32,
                                            ) {
                                                Ok(source) => {
                                                    let _ = self.engine.send_command(
                                                        crate::audio::command::AudioCommand::SetInstrument {
                                                            source,
                                                        },
                                                    );
                                                    self.status_message =
                                                        crate::tstatus!("🎹 Källa: {}", entry.name);
                                                }
                                                Err(err) => self.status_message = err,
                                            }
                                        }
                                        InstrumentKind::Sfz => match crate::audio::sfz::load(&entry.path) {
                                            Ok(inst) => {
                                                let count = inst.zones.len();
                                                ch.zones.extend(inst.zones);
                                                self.status_message = crate::tstatus!(
                                                    "🎹 '{}': {} zoner in i keymappen",
                                                    inst.name,
                                                    count
                                                );
                                            }
                                            Err(e) => {
                                                self.status_message =
                                                    crate::tstatus!("⚠️ {}: {}", entry.name, e);
                                            }
                                        },
                                        InstrumentKind::SoundFont => {
                                            let parsed = std::fs::read(&entry.path)
                                                .map_err(|e| e.to_string())
                                                .and_then(|bytes| crate::audio::sf2::parse(&bytes));
                                            match parsed {
                                                Ok(sf) => {
                                                    // Presetvalet sker i SF2-knappens panel; här
                                                    // importeras första instrumentet, som förut.
                                                    match sf.instruments.iter().find(|i| !i.zones.is_empty()) {
                                                        Some(inst) => {
                                                            let count = inst.zones.len();
                                                            let name = inst.name.clone();
                                                            ch.zones.extend(inst.zones.iter().cloned());
                                                            self.status_message = crate::tstatus!(
                                                                "🎹 '{}': {} zoner in i keymappen ({} presets i filen — välj i SF2-knappen)",
                                                                name,
                                                                count,
                                                                sf.presets.len()
                                                            );
                                                        }
                                                        None => {
                                                            self.status_message = crate::tstatus!(
                                                                "⚠️ '{}' har inga zoner med ljud",
                                                                entry.name
                                                            );
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    self.status_message =
                                                        crate::tstatus!("⚠️ {}: {}", entry.name, e);
                                                }
                                            }
                                        }
                                        InstrumentKind::Plugin => {
                                            self.status_message = crate::tstatus!(
                                                "🔌 '{}' laddas i plugin-fliken (kräver ett spår)",
                                                entry.name
                                            );
                                        }
                                    }
                                }
                                ui.label(
                                    egui::RichText::new(
                                        crate::audio::instrument_library::default_dir().display().to_string(),
                                    )
                                    .size(Theme::FONT_XS)
                                    .color(crate::ui::theme::palette().text_muted()),
                                );
                            });
                            // **Släppljud** (FL Keys): dämparens ljud, spelas när tonen släpps.
                            ui.horizontal(|ui| {
                                let label = match &ch.release_path {
                                    Some(p) => format!(
                                        "🎧 {}",
                                        std::path::Path::new(p)
                                            .file_name()
                                            .map(|n| n.to_string_lossy().to_string())
                                            .unwrap_or_else(|| p.clone())
                                    ),
                                    None => crate::i18n::t("🎧 Släppljud…").to_string(),
                                };
                                if ui.button(egui::RichText::new(label).size(Theme::FONT_S))
                                    .on_hover_text(crate::i18n::t("Ljudet som hörs när tonen släpps (dämparen), i stället för en rak nedtoning"))
                                    .clicked()
                                    && let Some(p) = rfd::FileDialog::new()
                                        .add_filter("WAV", &["wav"])
                                        .pick_file()
                                {
                                    match crate::audio::wav_reader::load_wav_pcm(&p.to_string_lossy()) {
                                        Ok((l, _, rate)) => {
                                            ch.release_path = Some(p.to_string_lossy().to_string());
                                            ch.release_pcm = Some((std::sync::Arc::new(l), rate));
                                        }
                                        Err(e) => {
                                            self.status_message = crate::tstatus!(
                                                "⚠️ Kunde inte läsa släppljudet: {}",
                                                e
                                            );
                                        }
                                    }
                                }
                                if ch.release_path.is_some() && ui.small_button("✕").clicked() {
                                    ch.release_path = None;
                                    ch.release_pcm = None;
                                }
                            });
                            if ch.zones.is_empty() {
                                ui.label(egui::RichText::new(crate::i18n::t("Inga zoner — kanalen spelar sitt eget sampel.")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            }
                            let mut ta_bort: Option<usize> = None;
                            for (zi, z) in ch.zones.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(format!("{}", zi + 1)).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                                    ui.add(egui::DragValue::new(&mut z.root).range(0..=127).speed(0.2).prefix(crate::i18n::t("grund ")))
                                        .on_hover_text(crate::i18n::t("Sampelns grundton: transponeringen räknas härifrån, så zonen kan ligga i sitt eget register."));
                                    ui.add(egui::DragValue::new(&mut z.key_low).range(0..=127).speed(0.2).prefix(crate::i18n::t("ton ")));
                                    ui.label("–");
                                    ui.add(egui::DragValue::new(&mut z.key_high).range(0..=127).speed(0.2));
                                    ui.add(egui::Slider::new(&mut z.vel_low, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
                                    ui.label("–");
                                    ui.add(egui::Slider::new(&mut z.vel_high, 0.0..=1.0).custom_formatter(|v, _| format!("{:.0} %", v * 100.0)));
                                    if !z.is_playable() {
                                        ui.label(egui::RichText::new(crate::i18n::t("⚠ filen kunde inte läsas — kanalens eget sampel spelar")).color(crate::ui::theme::palette().yellow()).size(Theme::FONT_S));
                                    }
                                    if ui.button("🗑").on_hover_text(crate::i18n::t("Ta bort zonen")).clicked() {
                                        ta_bort = Some(zi);
                                    }
                                });
                            }
                            if let Some(i) = ta_bort {
                                ch.zones.remove(i);
                            }
                            // Ett **bakvänt** intervall täcker ingenting (se `keymap::covers`), så
                            // det vänds här i stället för att bli en tyst zon som aldrig hörs.
                            for z in ch.zones.iter_mut() {
                                if z.key_low > z.key_high {
                                    std::mem::swap(&mut z.key_low, &mut z.key_high);
                                }
                                if z.vel_low > z.vel_high {
                                    std::mem::swap(&mut z.vel_low, &mut z.vel_high);
                                }
                                z.vel_low = z.vel_low.clamp(0.0, 1.0);
                                z.vel_high = z.vel_high.clamp(0.0, 1.0);
                            }

                            ui.separator();
                            ui.label(egui::RichText::new(crate::i18n::t("Envelope (ADSR):")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Attack:"));
                                ui.add(egui::Slider::new(&mut ch.amp_env.attack, 0.0..=2.0).suffix(" s").fixed_decimals(3));
                            });
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Decay:"));
                                ui.add(egui::Slider::new(&mut ch.amp_env.decay, 0.0..=2.0).suffix(" s").fixed_decimals(3));
                            });
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Sustain:"));
                                ui.add(egui::Slider::new(&mut ch.amp_env.sustain, 0.0..=1.0));
                            });
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Release:"));
                                ui.add(egui::Slider::new(&mut ch.amp_env.release, 0.0..=4.0).suffix(" s").fixed_decimals(3));
                            });
                            // **Anslagets känslighet** (Fas 8.4/7). Den hör ihop med envelopen:
                            // velocityn skalar dess nivå, inte dess tider — en svagare not är
                            // svagare, inte kortare. Standardvärdet 1,0 är den linjära faktor
                            // kanalen alltid har haft, så ett projekt från före ratten låter
                            // likadant.
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Velocitet:"));
                                ui.add(
                                    egui::Slider::new(&mut ch.velocity_sensitivity, 0.0..=1.0)
                                        .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
                                )
                                .on_hover_text(crate::i18n::t("Hur mycket notens anslag får påverka nivån. 100 % är den faktor som alltid har funnits, 0 % stänger av den helt — då låter varje anslag lika starkt. Gäller kanalens sampel; inte den inbyggda synten."));
                            });
                            // **Kurvan** (Fas 8.4/7): hur anslaget översätts till nivå innan
                            // känsligheten blandas in. Listan läses ur `VelocityCurve::ALL`, så
                            // en ny kurva dyker upp här utan en rad ny UI-kod.
                            ui.horizontal(|ui| {
                                ui.label(crate::i18n::t("Kurva:"));
                                egui::ComboBox::from_id_salt("velocity-curve")
                                    .selected_text(crate::i18n::t(ch.velocity_curve.label()))
                                    .show_ui(ui, |ui| {
                                        for c in crate::audio::envelope::VelocityCurve::ALL {
                                            ui.selectable_value(
                                                &mut ch.velocity_curve,
                                                c,
                                                crate::i18n::t(c.label()),
                                            );
                                        }
                                    })
                                    .response
                                    .on_hover_text(crate::i18n::t("Rak: halvt anslag ger halv nivå (kanalen har alltid haft den). Kvadratisk (\"curve 2\", som FL:s sampler och Abletons Simpler): halvt anslag ger en fjärdedels nivå — −12 dB i stället för −6 dB. Skillnaden hörs mest på svaga slag."));
                            });
                            // **Filterenvelopen** (Fas 8.4/7): ett lågpassfilter per röst med sin
                            // **egen** ADSR. Den är filtrets, inte amplitudens — en "pluck" är
                            // filtret som stänger, medan nivån står still.
                            ui.separator();
                            ui.label(egui::RichText::new(crate::i18n::t("Filter (lågpass):")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            ui.checkbox(&mut ch.filter.on, crate::i18n::t("Filter på"))
                                .on_hover_text(crate::i18n::t("Ett lågpassfilter per röst. Avstängt är filtret inte ett filter som står öppet — det rör samplarna inte alls, så ett projekt från före filtret låter exakt som förut."));
                            ui.add_enabled_ui(ch.filter.on, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("Cutoff:"));
                                    ui.add(egui::Slider::new(&mut ch.filter.cutoff_hz, 20.0..=20_000.0).logarithmic(true).suffix(" Hz").fixed_decimals(0));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("Resonans:"));
                                    ui.add(egui::Slider::new(&mut ch.filter.resonance, 0.1..=10.0).fixed_decimals(2));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("Envelop:"));
                                    ui.add(egui::Slider::new(&mut ch.filter.env_amount_octaves, -4.0..=6.0).suffix(" oktaver").fixed_decimals(2))
                                        .on_hover_text(crate::i18n::t("Hur långt envelopen flyttar cutoffen, i oktaver — samma avstånd i klang oavsett var cutoffen står (0 = ingen förflyttning alls). Negativt stänger filtret i stället."));
                                });
                                // Envelopens egna tider.
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("F-attack:"));
                                    ui.add(egui::Slider::new(&mut ch.filter.env.attack, 0.0..=2.0).suffix(" s").fixed_decimals(3));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("F-decay:"));
                                    ui.add(egui::Slider::new(&mut ch.filter.env.decay, 0.0..=2.0).suffix(" s").fixed_decimals(3));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("F-sustain:"));
                                    ui.add(egui::Slider::new(&mut ch.filter.env.sustain, 0.0..=1.0));
                                });
                                ui.horizontal(|ui| {
                                    ui.label(crate::i18n::t("F-släpp:"));
                                    ui.add(egui::Slider::new(&mut ch.filter.env.release, 0.0..=4.0).suffix(" s").fixed_decimals(3));
                                });
                            });

                            // **Ingen envelop** — knappen hör till samplern (en syster till
                            // envelop- och filterraderna), inte till velocitetsraden. Fram till
                            // 2026-09-15 saknade velocitets-`ui.horizontal` sin egen stängning
                            // och lånade den här, vilket lade **hela filterblocket inuti en
                            // radlayout** — alltså i samma rad som reglaget.
                            ui.horizontal(|ui| {
                                if ui
                                    .button(crate::i18n::t("Ingen envelop"))
                                    .on_hover_text(crate::i18n::t("Identiteten: full nivå från första samplet, inget släpp. Det är så en kanal utan envelop låter — och så ett projekt från före samplern låter."))
                                    .clicked()
                                {
                                    ch.amp_env = crate::audio::envelope::AdsrParams::identity();
                                }
                                if ch.amp_env.is_identity() {
                                    ui.label(egui::RichText::new(crate::i18n::t("(ingen envelop vald)")).color(crate::ui::theme::palette().text_muted()).size(Theme::FONT_S));
                                }
                            });
                        });
                    });
                });
            }

        if sf2_open.is_some() {
            self.sf2_pick = sf2_open.take();
        }
        // **SF2 preset picker**: the file's presets are a choice, not noise. Option 0 is the
        // old behaviour (all zones of the first instrument), so nothing that worked stops.
        if let Some(mut pick) = self.sf2_pick.take() {
            ui.add_space(6.0);
            let mut import = false;
            let mut cancel = false;
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("🎹 {}", pick.path))
                            .size(Theme::FONT_S)
                            .color(crate::ui::theme::palette().text_muted()),
                    );
                    egui::ComboBox::from_id_salt("sf2-preset")
                        .selected_text(match pick.selected {
                            0 => "(alla zoner i första instrumentet)".to_string(),
                            i => pick.sf.presets.get(i - 1).cloned().unwrap_or_default(),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut pick.selected,
                                0,
                                "(alla zoner i första instrumentet)",
                            );
                            for (i, name) in pick.sf.presets.iter().enumerate() {
                                ui.selectable_value(&mut pick.selected, i + 1, name.as_str());
                            }
                        });
                    import = ui
                        .button(crate::i18n::t("📥 Importera"))
                        .on_hover_text(crate::i18n::t(
                            "Lägger det valda presetets zoner i kanalens keymap.",
                        ))
                        .clicked();
                    cancel = ui.small_button("✕").clicked();
                });
            });
            if import {
                let inst = if pick.selected == 0 {
                    pick.sf.instruments.iter().find(|i| !i.zones.is_empty())
                } else {
                    pick.sf
                        .preset_instruments
                        .get(pick.selected - 1)
                        .copied()
                        .flatten()
                        .and_then(|i| pick.sf.instruments.get(i))
                        .filter(|i| !i.zones.is_empty())
                };
                match inst {
                    Some(i) => {
                        let (name, count) = (i.name.clone(), i.zones.len());
                        let zones = i.zones.clone();
                        if let Some(ch) = self.channels.get_mut(pick.channel) {
                            ch.zones.extend(zones);
                        }
                        self.status_message = crate::tstatus!(
                            "🎹 '{}': {} zoner in i keymappen ({} presets i filen)",
                            name,
                            count,
                            pick.sf.presets.len()
                        );
                    }
                    None => {
                        self.status_message = crate::tstatus!(
                            "⚠️ Preset '{}' har inga zoner med ljud",
                            pick.sf
                                .presets
                                .get(pick.selected.saturating_sub(1))
                                .cloned()
                                .unwrap_or_default()
                        );
                    }
                }
            } else if !cancel {
                self.sf2_pick = Some(pick);
            }
        }

        self.sync_active_pattern_from_ui();
    });
}
}

impl SonixApp {
pub fn apply_alchemy_morph(&mut self) {
    let px = self.alchemy_puck[0];
    let py = self.alchemy_puck[1];
    use std::f32::consts::PI;

    let snapshots = [
        (-0.5 * PI, 0.005, 0.35, 4200.0, 1.8, Waveform::Saw),
        (-0.25 * PI, 0.35, 0.6, 2800.0, 1.2, Waveform::Saw),
        (0.0 * PI, 0.005, 0.15, 1200.0, 4.5, Waveform::Saw),
        (0.25 * PI, 0.01, 0.25, 6000.0, 2.0, Waveform::Square),
        (0.5 * PI, 0.5, 0.8, 1500.0, 0.8, Waveform::Triangle),
        (0.75 * PI, 0.08, 0.4, 3500.0, 2.5, Waveform::Triangle),
        (1.0 * PI, 0.002, 0.6, 8000.0, 3.0, Waveform::Sine),
        (1.25 * PI, 0.001, 0.08, 12000.0, 0.7, Waveform::Square),
    ];

    let mut total_weight = 0.0;
    let mut morph_attack = 0.0;
    let mut morph_decay = 0.0;
    let mut morph_cutoff = 0.0;
    let mut morph_res = 0.0;
    let mut closest_wf = Waveform::Saw;
    let mut min_dist = f32::MAX;

    for (angle, att, dec, cut, res, wf) in snapshots {
        let sx = angle.cos();
        let sy = angle.sin();
        let dist = ((px - sx).powi(2) + (py - sy).powi(2)).sqrt().max(0.05);
        let w = 1.0 / (dist.powf(2.5));
        total_weight += w;
        morph_attack += att * w;
        morph_decay += dec * w;
        morph_cutoff += cut * w;
        morph_res += res * w;

        if dist < min_dist {
            min_dist = dist;
            closest_wf = wf;
        }
    }

    if total_weight > 0.0001 {
        self.adsr.attack = morph_attack / total_weight;
        self.adsr.decay = morph_decay / total_weight;
        self.filter.cutoff = (morph_cutoff / total_weight).clamp(200.0, 18000.0);
        self.filter.resonance = (morph_res / total_weight).clamp(0.5, 9.0);
        self.waveform = closest_wf;

        let _ = self.engine.send_command(AudioCommand::SetAdsr(self.adsr));
        let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
        let _ = self.engine.send_command(AudioCommand::SetWaveform(self.waveform));
    }
}
}

impl SonixApp {
pub(crate) fn render_alchemy_morph_pad(&mut self, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("✨ SONIX ALCHEMY SYNTHESIZER (Transform Pad)")).strong().size(Theme::FONT_L).color(crate::ui::theme::palette().cyan()));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("8-punkters realtidsmorfning av vågform, ADSR och filter (subtraktiv syntes)")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            // Center Morphing Pad
            ui.group(|ui| {
                ui.set_width(380.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("TRANSFORM PAD")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                    ui.label(egui::RichText::new(crate::i18n::t("Dra den lysande markören för att sömlöst smälta samman filter, vågformer och ADSR")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                    ui.add_space(6.0);

                    if alchemy_transform_matrix(ui, &mut self.alchemy_puck, Vec2::new(360.0, 300.0)) {
                        self.apply_alchemy_morph();
                    }
                });
            });

            ui.separator();

            // Live Morphed Parameters
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("AKTIVA SYNTPARAMETRAR (MORPHED)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                    ui.add_space(6.0);

                    ui.label(crate::tstatus!("Vågform: {}", self.waveform.name()));
                    ui.label(format!("Filter Cutoff: {:.0} Hz", self.filter.cutoff));
                    ui.label(crate::tstatus!("Resonans (Q): {:.2}", self.filter.resonance));
                    ui.label(format!("Attack: {:.3}s  •  Decay: {:.2}s", self.adsr.attack, self.adsr.decay));

                    ui.add_space(8.0);
                    ui.separator();

                    ui.label(egui::RichText::new(crate::i18n::t("SNABBVAL SNAPSHOTS")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                    let snaps = [
                        ("1. Pluck", 0.0_f32, -1.0_f32),
                        ("2. Lush String", 0.707, -0.707),
                        ("3. 303 Acid", 1.0, 0.0),
                        ("4. Synthwave", 0.707, 0.707),
                        ("5. Warm Pad", 0.0, 1.0),
                        ("6. Cosmic Brass", -0.707, 0.707),
                        ("7. Digital Bell", -1.0, 0.0),
                        ("8. Chiptune", -0.707, -0.707),
                    ];

                    ui.columns(2, |cols| {
                        for (i, (s_name, px, py)) in snaps.iter().enumerate() {
                            let col_i = i % 2;
                            if cols[col_i].button(*s_name).clicked() {
                                self.alchemy_puck = [*px, *py];
                                self.apply_alchemy_morph();
                            }
                        }
                    });

                    ui.add_space(8.0);
                    if ui.button(crate::i18n::t("🎹 Provspela Ton (C4)")).clicked() {
                        self.play_note(60);
                    }
                });
            });
        });
    });
}
}

impl SonixApp {
pub(crate) fn render_remix_fx_view(&mut self, ui: &mut egui::Ui) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎛 REMIX FX & GROSS BEAT (DJ Performance Touch)")).strong().size(Theme::FONT_L).color(crate::ui::theme::palette().green()));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("DJ-effekter, Tape Stop, Filter Sweeps och Stutter Glitch")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
        });

        ui.add_space(8.0);

        ui.horizontal(|ui| {
            // DJ XY Filter Touch Surface
            ui.group(|ui| {
                ui.set_width(340.0);
                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("DJ FILTER & CRUSH XY PAD")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                    let (rect, resp) = ui.allocate_exact_size(Vec2::new(320.0, 240.0), Sense::click_and_drag());
                    let painter = ui.painter();

                    if (resp.dragged() || resp.clicked())
                        && let Some(pos) = resp.interact_pointer_pos() {
                            self.remix_xy[0] = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                            self.remix_xy[1] = (1.0 - (pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);

                            self.filter.cutoff = 200.0 + self.remix_xy[0] * 12000.0;
                            self.filter.resonance = 0.5 + self.remix_xy[1] * 7.5;
                            let _ = self.engine.send_command(AudioCommand::SetFilter(self.filter));
                        }

                    painter.rect_filled(rect, Rounding::same(6.0), crate::ui::theme::palette().bg());
                    painter.rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.5_f32, crate::ui::theme::palette().green()));

                    let c = rect.center();
                    painter.line_segment([Pos2::new(rect.min.x, c.y), Pos2::new(rect.max.x, c.y)], Stroke::new(1.0_f32, crate::ui::theme::palette().header()));
                    painter.line_segment([Pos2::new(c.x, rect.min.y), Pos2::new(c.x, rect.max.y)], Stroke::new(1.0_f32, crate::ui::theme::palette().header()));

                    let puck_x = rect.min.x + self.remix_xy[0] * rect.width();
                    let puck_y = rect.max.y - self.remix_xy[1] * rect.height();
                    painter.circle_filled(Pos2::new(puck_x, puck_y), 12.0, crate::ui::theme::Theme::alpha(crate::ui::theme::palette().green(), 100));
                    painter.circle_filled(Pos2::new(puck_x, puck_y), 6.0, crate::ui::theme::palette().green());

                    ui.label(egui::RichText::new(format!("Cutoff: {:.0} Hz  •  Res: {:.1}", self.filter.cutoff, self.filter.resonance)).size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                });
            });

            ui.separator();

            // DJ Performance Buttons
            ui.group(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("DJ PERFORMANCE PADS")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                    ui.add_space(6.0);

                    ui.horizontal(|ui| {
                        // Tape Stop
                        let stop_col = if self.remix_tape_stop { crate::ui::theme::palette().orange() } else { crate::ui::theme::palette().panel_alt() };
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⏸ TAPE STOP")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(stop_col).min_size(Vec2::new(100.0, 40.0))).clicked() {
                            self.remix_tape_stop = !self.remix_tape_stop;
                            let _ = self.engine.send_command(AudioCommand::SetTapeStop { active: self.remix_tape_stop });
                        }

                        // Vinyl Scratch
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📻 VINYL SCRATCH")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().border()).min_size(Vec2::new(120.0, 40.0))).clicked() {
                            self.drive = 4.5;
                            let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
                        }
                    });

                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(crate::i18n::t("GROSS BEAT / STUTTER GLITCH")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                    ui.horizontal(|ui| {
                        if ui.selectable_label(self.remix_stutter == 1, "⚡ 1/4 Repeat").clicked() {
                            self.remix_stutter = if self.remix_stutter == 1 { 0 } else { 1 };
                            let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                            self.status_message = crate::i18n::t("⚡ 1/4 Beat Repeat aktiv").to_string();
                        }
                        if ui.selectable_label(self.remix_stutter == 2, "⚡ 1/8 Stutter").clicked() {
                            self.remix_stutter = if self.remix_stutter == 2 { 0 } else { 2 };
                            let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                            self.status_message = crate::i18n::t("⚡ 1/8 Stutter aktiv").to_string();
                        }
                        if ui.selectable_label(self.remix_stutter == 3, "⚡ 1/16 Roll").clicked() {
                            self.remix_stutter = if self.remix_stutter == 3 { 0 } else { 3 };
                            let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                            self.status_message = crate::i18n::t("⚡ 1/16 High-Speed Roll aktiv").to_string();
                        }
                        if ui.selectable_label(self.remix_stutter == 4, "🌀 Reverse").clicked() {
                            self.remix_stutter = if self.remix_stutter == 4 { 0 } else { 4 };
                            let _ = self.engine.send_command(AudioCommand::SetRemixFx { mode: self.remix_stutter as u8, bpm: self.bpm });
                            self.status_message = crate::i18n::t("🌀 Gross Beat Reverse aktiv").to_string();
                        }
                    });

                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(crate::i18n::t("GROSS BEAT / STEGGRIND (16 steg)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                    ui.horizontal(|ui| {
                        let label = if self.gate_on {
                            crate::i18n::t("🟩 STEGGRIND PÅ")
                        } else {
                            crate::i18n::t("⬛ STEGGRIND AV")
                        };
                        let fill = if self.gate_on { crate::ui::theme::palette().green() } else { crate::ui::theme::palette().panel_alt() };
                        let mut changed = false;
                        if ui
                            .add(
                                egui::Button::new(egui::RichText::new(label).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text()))
                                    .fill(fill)
                                    .min_size(Vec2::new(120.0, 26.0)),
                            )
                            .clicked()
                        {
                            self.gate_on = !self.gate_on;
                            changed = true;
                        }
                        for i in 0..16 {
                            let on = self.gate_gains[i] > 0.5;
                            let cell = if on { crate::ui::theme::palette().cyan() } else { crate::ui::theme::palette().panel() };
                            let text_col = if on { Color32::BLACK } else { crate::ui::theme::palette().text_muted() };
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(format!("{}", i + 1)).size(Theme::FONT_XS).color(text_col),
                                    )
                                    .fill(cell)
                                    .min_size(Vec2::new(21.0, 26.0)),
                                )
                                .clicked()
                            {
                                self.gate_gains[i] = if on { 0.0 } else { 1.0 };
                                changed = true;
                            }
                        }
                        if changed {
                            let _ = self.engine.send_command(AudioCommand::SetGatePattern(
                                crate::audio::master_fx::GatePatternParams {
                                    gains: self.gate_gains,
                                    bpm: self.bpm,
                                    smooth: self.gate_smooth,
                                    enabled: self.gate_on,
                                },
                            ));
                        }
                    });
                    ui.horizontal(|ui| {
                        if ui
                            .add(egui::Slider::new(&mut self.gate_smooth, 0.0..=1.0).text(crate::i18n::t("Mjuk kant")))
                            .changed()
                        {
                            let _ = self.engine.send_command(AudioCommand::SetGatePattern(
                                crate::audio::master_fx::GatePatternParams {
                                    gains: self.gate_gains,
                                    bpm: self.bpm,
                                    smooth: self.gate_smooth,
                                    enabled: self.gate_on,
                                },
                            ));
                        }
                    });

                    ui.add_space(10.0);
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Styr DJ-effekter under uppspelning för live-remixing av dina mönster!")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                    });
                });
            });
        });
    });
}
}

fn rand_simple(seed: usize) -> f32 {
    let mut x = (seed as u32).wrapping_mul(1103515245).wrapping_add(12345);
    x = (x >> 16) & 0x7FFF;
    x as f32 / 32767.0
}
