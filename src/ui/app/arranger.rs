//! Arrangören — tidslinjen som ritas.
//!
//! En enda stor ritfunktion (2 580 rader): spårrader, klipp, vågformer, markörer, slingor och
//! reglagen. Den läser \`tempo_map\`/\`bpm_at\` för placeringen, \`region_source_secs\` för
//! utsnittet och \`waveform\`-cachen för bilden — alltså ingenting eget.
//!
//! Status: byggs — arrangören.
//! Rör inte: enheten. Takt är inte sekund; vid ett tempobyte är bara takten rätt plats.

use super::*;

impl SonixApp {
pub(crate) fn render_playlist_arranger(&mut self, ui: &mut egui::Ui) {
    let needs_mic = self.playlist_tracks.is_empty()
        || !self.playlist_tracks.iter().any(|t| {
            let n = t.name.to_lowercase();
            n.contains("mic") || n.contains("mikrofon") || n.contains("microphone") || n.contains("mik")
        });
    if needs_mic {
        self.ensure_mic_track_exists();
    }

    ui.group(|ui| {
        self.draw_transport_bar(ui);


        self.draw_arranger_tools_bar(ui);


        ui.add_space(4.0);

        // ================================================================
        // 2. SUNO STUDIO ROLLING TIMELINE CANVAS & MULTI-TIER RULER
        // ================================================================
        // Handle Ctrl + Mouse Wheel Smooth Zoom
        if ui.input(|i| i.modifiers.ctrl && i.raw_scroll_delta.y != 0.0) {
            let delta = ui.input(|i| i.raw_scroll_delta.y);
            let zoom_mult = if delta > 0.0 { 1.15 } else { 0.87 };
            self.suno_zoom_level = (self.suno_zoom_level * zoom_mult).clamp(0.25, 20.0);
        }

        let header_w = 210.0;
        let bar_w = (60.0 * self.suno_zoom_level).clamp(16.0, 1500.0);
        let row_h = 46.0;
        let ruler_h = 32.0;
        // **Tidsaxeln går genom tempokartan** (Fas 8.2 steg 3). X är fortfarande takter
        // (`bar_w` per takt); varje gång en *sekund* behövs frågas kartan i stället för att
        // multiplicera med ett enda tempo. Med ett tempo ger det samma tal som förut, och
        // efter ett tempobyte är det här siffran är rätt — annars är varje tid efter bytet
        // tyst fel, och ett fel i visningen syns (en kloss på fel plats).
        let tempo = self.tempo_map();

        // **De fyra frågorna `sec_per_bar` svarade på — men med fel svar efter ett
        // tempobyte** (Fas 8.2 steg 3). En *plats* i tiden, en *längd*, en *omvändning*
        // och en *lokal* sekunder-per-takt är olika frågor; en enda skalär kunde bara
        // svara rätt på dem så länge tempot var konstant. Med ett tempo ger de samma tal
        // som förut, bit för bit.
        let secs_at = |bar: f32| tempo.secs_at_bar(bar as f64) as f32;
        let secs_len = |from_bar: f32, bars: f32| tempo.secs_for_bars_at(from_bar as f64, bars as f64) as f32;
        let bars_at = |secs: f32| tempo.bar_at_secs(secs as f64) as f32;
        let sec_per_bar_at = |bar: f32| tempo.secs_per_bar_at(bar as f64) as f32;
        let bars_len = |from_bar: f32, secs: f32| tempo.bars_for_secs_at(from_bar as f64, secs as f64) as f32;

        // Calculate max song bars based on audio regions
        let max_region_end = self.playlist_tracks.iter()
            .flat_map(|t| t.regions.iter())
            .map(|r| r.start_bar + r.length_bars)
            .fold(32.0_f32, |m, b| m.max(b));
        let total_bars = ((max_region_end + 8.0).ceil() as usize).clamp(32, 1024);
        let timeline_total_w = total_bars as f32 * bar_w;

        let mut deferred = DeferredActions::default();

        let num_tracks = self.playlist_tracks.len();
        let g = TimelineGeometry {
            header_w, bar_w, row_h, ruler_h, total_bars, timeline_total_w,
        };

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            // ================================================================
            self.draw_track_headers(ui, &g, &tempo, &mut deferred);

            // Vertical Divider Line between Left Frozen Column and Right Timeline ScrollArea
            ui.painter().line_segment(
                [Pos2::new(ui.cursor().min.x, ui.cursor().min.y), Pos2::new(ui.cursor().min.x, ui.cursor().min.y + (num_tracks as f32 * 50.0).max(200.0))],
                Stroke::new(1.0_f32, Color32::from_rgb(40, 48, 64)),
            );

            self.draw_timeline_lanes(
                ui, &g, &tempo, &secs_at, &secs_len, &bars_at, &sec_per_bar_at, &bars_len,
                &mut deferred,
            );
        });

        let DeferredActions {
            scrub_target_sec, drop_sample_action, track_delete_idx, track_duplicate_idx,
            track_paste_idx,
        } = deferred;

        // Floating drag badge tooltip that follows mouse cursor in foreground tooltip layer
        if let Some(ref drag_item) = self.sample_drag_item {
            let pointer_opt = ui.ctx().pointer_latest_pos()
                .or_else(|| ui.ctx().pointer_interact_pos())
                .or_else(|| ui.input(|i| i.pointer.hover_pos()));
            if let Some(pos) = pointer_opt {
                let tooltip_rect = Rect::from_min_size(Pos2::new(pos.x + 16.0, pos.y + 12.0), Vec2::new(200.0, 26.0));
                let painter = ui.ctx().layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("sample_drag_tooltip")));
                painter.rect_filled(tooltip_rect, Rounding::same(4.0), Color32::from_rgb(20, 26, 38));
                painter.rect_stroke(tooltip_rect, Rounding::same(4.0), Stroke::new(1.5_f32, Theme::FL_CYAN));
                painter.text(
                    Pos2::new(tooltip_rect.min.x + 8.0, tooltip_rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("📦 Dra: {} ({})", drag_item.name, drag_item.category),
                    egui::FontId::proportional(10.5),
                    Color32::WHITE,
                );
            }
        }

        // Process Sample Drop onto Playlist Timeline
        if let Some((target_t, item, drop_bar)) = drop_sample_action {
            self.add_sample_item_to_track_at_bar(target_t, &item, drop_bar);
            self.sample_drag_item = None;
        } else if self.sample_drag_item.is_some() && ui.input(|i| i.pointer.any_released() || i.pointer.primary_released()) {
            self.sample_drag_item = None;
        }

        // Execute deferred track actions safely AFTER UI rendering
        if let Some(del_idx) = track_delete_idx {
            self.delete_track(del_idx);
        }
        if let Some(dup_idx) = track_duplicate_idx {
            self.duplicate_track(dup_idx);
        }
        if let Some(paste_idx) = track_paste_idx {
            self.selected_timeline_track = paste_idx;
            self.paste_region();
        }

        // Apply scrub seeking if user interacted with ruler
        if let Some(target) = scrub_target_sec {
            self.seek_song_time(target);
        }

        self.draw_region_inspector(ui, &secs_at, &secs_len, &bars_at, &bars_len);

        ui.add_space(6.0);

        self.draw_arranger_ai_capsule(ui);

        self.draw_arranger_utility_bar(ui);

    });
}
}

impl SonixApp {
    /// Arrangörens första rad: projektet, stämimporten, transporten med klockan, tempot och
    /// tap-tempo, tonarten, taktarten och metronomen.
    pub(crate) fn draw_transport_bar(&mut self, ui: &mut egui::Ui) {
        // ================================================================
        // 1. ARRANGER TOP CONTROL & TRANSPORT BARS (2 Non-Overlapping Rows)
        // ================================================================
        // ROW 1: Project Pill, Stems Import, Capsule Transport & Clock, BPM (Responsive Wrapped)
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

            // Project title pill
            ui.group(|ui| {
                ui.set_height(26.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("🎵 {}", self.project_name)).strong().size(11.5).color(Theme::TEXT_BRIGHT));
                    if ui.button(crate::i18n::t("•••")).on_hover_text(crate::i18n::t("Filhanterare & Projektinställningar")).clicked() {
                        self.show_project_manager_modal = true;
                    }
                });
            });

            ui.separator();

            // Stems Importer Button
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📦 Stämmor (Stems)")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(110, 50, 160))).clicked() {
                self.scan_for_suno_stems();
                self.show_suno_import_modal = true;
            }

            ui.separator();

            // Centered Capsule Transport Bar (High Precision)
            ui.group(|ui| {
                ui.set_height(26.0);
                ui.horizontal(|ui| {
                    // Level LED peak meter
                    let peak = self.engine.get_peak_level();
                    let (m_rect, _) = ui.allocate_exact_size(Vec2::new(14.0, 16.0), Sense::hover());
                    let h = (peak * 14.0).clamp(2.0, 14.0);
                    let bar_rect = Rect::from_min_max(Pos2::new(m_rect.min.x + 3.0, m_rect.max.y - h), Pos2::new(m_rect.max.x - 3.0, m_rect.max.y));
                    ui.painter().rect_filled(bar_rect, Rounding::same(1.0), Theme::FL_GREEN);

                    // Transport Buttons
                    if ui.button(crate::i18n::t("⏮")).on_hover_text(crate::i18n::t("Gå till start (00:00.00)")).clicked() {
                        self.seek_song_time(0.0);
                    }

                    let is_rec = self.is_recording_timeline || self.vocal_studio.is_recording;
                    let rec_col = if is_rec { Color32::from_rgb(255, 40, 40) } else { Color32::from_rgb(180, 30, 30) };
                    let rec_label = if is_rec { "⏹ REC" } else { "⏺ REC" };
                    if ui.add(egui::Button::new(egui::RichText::new(rec_label).size(10.5).strong().color(Color32::WHITE)).fill(rec_col)).clicked() {
                        self.toggle_timeline_recording();
                    }

                    let play_txt = if self.is_playing { "⏸" } else { "▶" };
                    let play_bg = if self.is_playing { Theme::FL_GREEN } else { Color32::from_rgb(38, 45, 55) };
                    if ui.add(egui::Button::new(egui::RichText::new(play_txt).strong().size(11.0).color(Color32::WHITE)).fill(play_bg)).clicked() {
                        self.toggle_playback();
                    }

                    let loop_bg = Color32::from_rgb(38, 45, 55);
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁")).size(10.0).color(Theme::FL_CYAN)).fill(loop_bg)).on_hover_text(crate::i18n::t("Sätt loop")).clicked() {
                        self.status_message = crate::tstatus!("Loop-region satt till Takt {}-{}", self.loop_start_bar + 1, self.loop_end_bar);
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(crate::i18n::t("⏱ Tempo"))
                                    .size(10.5)
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(45, 40, 70)),
                        )
                        .on_hover_text(crate::i18n::t("Tempokarta — byten i låten"))
                        .clicked()
                    {
                        self.show_tempo_modal = true;
                    }

                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 Mik")).size(10.5).strong().color(Color32::WHITE)).fill(Color32::from_rgb(30, 80, 110))).on_hover_text(crate::i18n::t("Mikrofoninställningar")).clicked() {
                        self.show_mic_settings_modal = true;
                    }

                    ui.separator();

                    // High Precision LCD Time Display (Hundredths of a second)
                    let time_str = format_time_hundredths(self.song_time);
                    let bar_float =
                        self.tempo_map().bar_at_secs(self.song_time as f64) as f32;
                    let bar_text = format_bar_subdivisions(bar_float);
                    let full_time_str = format!("⏱ {}  {}", time_str, bar_text);
                    ui.label(egui::RichText::new(full_time_str).monospace().strong().size(11.5).color(Theme::FL_CYAN));
                });
            });

            ui.separator();

            // BPM & Tap-Tempo Pill
            ui.group(|ui| {
                ui.set_height(26.0);
                ui.horizontal(|ui| {
                    let (bpm_rect, _) =
                        ui.allocate_exact_size(Vec2::new(72.0, 20.0), Sense::hover());
                    ui.painter().rect_filled(
                        bpm_rect,
                        Rounding::same(2.0),
                        Color32::from_rgb(26, 32, 42),
                    );
                    // Skrivbart OCH dragbart — samma kontroll som tempodialogen
                    // använder. Att bara kunna dra tvingade fram skrubbande tills
                    // det blev "nästan" rätt; nu går det att skriva 48.2 exakt.
                    // DragValue visar en textmarkör vid klick och tar siffror,
                    // Enter, och Escape (avbryter) — beteendet kommer från egui,
                    // inte från en egen tolkning av tangenttryck.
                    // Hjälptexten säger vad som händer med LJUDET när tempot
                    // ändras (Fas 8.10) — annars är det en kontroll som ser ut
                    // att bara styra klockan. Antalet räknas, inte cachas.
                    let with_tempo = self.clips_with_source_tempo();
                    let tempo_hover = if with_tempo == 0 {
                        crate::i18n::t(
                            "Klippens tempo: inga klipp har ett känt inspelningstempo ännu, så ljudet rörs inte när du ändrar tempot.",
                        )
                        .to_string()
                    } else {
                        crate::tstatus!(
                            "Klippens tempo: {} klipp har ett känt inspelningstempo och följer med när du ändrar tempot — med bevarad tonhöjd (Fas 8.10 steg 2). Enstaka klipp kan i stället sättas i bandspelarläge i klippmenyn. Klipp med okänt tempo rörs inte.",
                            with_tempo
                        )
                    };
                    let tempo_field = ui.put(
                        bpm_rect.shrink2(Vec2::new(4.0, 2.0)),
                        egui::DragValue::new(&mut self.bpm)
                            .speed(0.5)
                            .range(40.0..=280.0)
                            .suffix(" BPM"),
                    );
                    tempo_field.on_hover_text(tempo_hover);

                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎯 TAP")).strong().size(10.0).color(Color32::WHITE)).fill(Color32::from_rgb(180, 70, 20))).on_hover_text(crate::i18n::t("Klicka i takt för att sätta tempo (Tap Tempo)")).clicked() {
                        self.tap_tempo();
                    }
                });
            });

            ui.separator();

            // Key Signature Picker
            ui.group(|ui| {
                ui.set_height(26.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🎼")).size(11.0));
                    // Tonarten (Fas 8.11): samma tabell och samma index som piano
                    // rollen. Förut hade de här menyerna egna listor, och "Dorian"
                    // här blev "Moll" där; fyra av tolv grundtoner visade dessutom
                    // fel namn (en tretton-namnlista för tolv tonhöjder).
                    let cur_root = crate::audio::scale::root_name(self.song_key_root);
                    egui::ComboBox::from_id_salt("arr_key_root")
                        .selected_text(cur_root)
                        .width(40.0)
                        .show_ui(ui, |ui| {
                            for (idx, &r_name) in crate::audio::scale::ROOT_NAMES.iter().enumerate() {
                                if ui.selectable_label(self.song_key_root as usize == idx, r_name).clicked() {
                                    self.song_key_root = idx as u8;
                                    self.status_message = crate::tstatus!(
                                        "🎼 Tonart: {}",
                                        crate::audio::scale::key_label(self.song_key_root, self.song_key_scale)
                                    );
                                }
                            }
                        });

                    let cur_sc = crate::i18n::t(crate::audio::scale::scale_at(self.song_key_scale).name);
                    egui::ComboBox::from_id_salt("arr_key_scale")
                        .selected_text(cur_sc)
                        .width(104.0)
                        .show_ui(ui, |ui| {
                            for (s_idx, sc) in crate::audio::scale::SCALES.iter().enumerate() {
                                if ui.selectable_label(self.song_key_scale == s_idx, crate::i18n::t(sc.name)).clicked() {
                                    self.song_key_scale = s_idx;
                                    self.status_message = crate::tstatus!(
                                        "🎼 Tonart: {}",
                                        crate::audio::scale::key_label(self.song_key_root, self.song_key_scale)
                                    );
                                }
                            }
                        });
                });
            });

            ui.separator();

            // Time Signature & Metronome
            ui.group(|ui| {
                ui.set_height(26.0);
                ui.horizontal(|ui| {
                    egui::ComboBox::from_id_salt("arr_time_sig")
                        .selected_text(format!("{}/{}", self.time_signature.0, self.time_signature.1))
                        .width(42.0)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(self.time_signature == (4, 4), "4/4").clicked() { self.time_signature = (4, 4); }
                            if ui.selectable_label(self.time_signature == (3, 4), "3/4").clicked() { self.time_signature = (3, 4); }
                            if ui.selectable_label(self.time_signature == (6, 8), "6/8").clicked() { self.time_signature = (6, 8); }
                        });

                    let metro_bg = if self.metronome_enabled { Theme::FL_ORANGE } else { Color32::from_rgb(32, 38, 48) };
                    let metro_fg = if self.metronome_enabled { Color32::BLACK } else { Theme::TEXT_MUTED };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔔 Metronom")).strong().size(10.0).color(metro_fg)).fill(metro_bg)).on_hover_text(crate::i18n::t("Hörbar klick-metronom vid uppspelning")).clicked() {
                        self.metronome_enabled = !self.metronome_enabled;
                        self.status_message = if self.metronome_enabled { crate::i18n::t("🔔 Metronom aktiverad").to_string() } else { crate::i18n::t("🔕 Metronom avstängd").to_string() };
                    }
                });
            });
        });
    }

    /// Arrangörens andra rad: verktygen (markera/pensla/klipp/tyst/stryk), snäppet,
    /// följ-spelhuvudet, automationskurvorna och zoomen.
    pub(crate) fn draw_arranger_tools_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(2.0);

        // ROW 2: Arranger Tools, Quick Slicing / Action Buttons, Snap Grid, Follow Playhead, and Zoom Controls (Responsive Wrapped)
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);

            // Arranger Tools Selector (Select, Paint, Slice ✂, Mute, Erase)
            ui.group(|ui| {
                ui.set_height(26.0);
                ui.horizontal(|ui| {
                    let tools = [
                        (ArrangerTool::Select, crate::i18n::t("⇱ Välj (1)")),
                        (ArrangerTool::Paint, crate::i18n::t("✎ Rita (2)")),
                        (ArrangerTool::Slice, crate::i18n::t("✂ Klipp (3)")),
                        (ArrangerTool::Mute, "🔇 Muta (4)"),
                        (ArrangerTool::Erase, "🗑 Radera (5)"),
                    ];
                    for (tool_val, tool_label) in tools {
                        let is_sel = std::mem::discriminant(&self.arranger_tool) == std::mem::discriminant(&tool_val);
                        let btn_bg = if is_sel {
                            if matches!(tool_val, ArrangerTool::Slice) { Color32::from_rgb(255, 120, 30) } else { Theme::FL_ORANGE }
                        } else {
                            Color32::from_rgb(32, 36, 44)
                        };
                        let btn_fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                        if ui.add(egui::Button::new(egui::RichText::new(tool_label).strong().size(10.5).color(btn_fg)).fill(btn_bg)).clicked() {
                            self.arranger_tool = tool_val;
                        }
                    }
                });
            });

            // Direct Quick-Cut Action Button (Always available!)
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✂ Klipp vid spelhuvud (Ctrl+B / S)")).strong().size(10.5).color(Color32::BLACK)).fill(Theme::FL_GREEN))
                .on_hover_text(crate::i18n::t("Dela vald region exakt vid den nuvarande tidsmarkören (Genväg: Ctrl+B eller S)"))
                .clicked() {
                    self.split_selected_region_at_playhead();
                }

            ui.separator();

            // Snap Selector (0.01s Precision, 1/16, Beat, Bar)
            ui.group(|ui| {
                ui.set_height(26.0);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("Snäpp:")).size(10.0).color(Theme::TEXT_MUTED));
                    let snaps = [
                        (TimeSnapMode::FreeHundredth, "⚡ 0.01s"),
                        (TimeSnapMode::Snap16th, "1/16"),
                        (TimeSnapMode::SnapBeat, "Beat"),
                        (TimeSnapMode::SnapBar, crate::i18n::t("Takt")),
                    ];
                    for (mode, label) in snaps {
                        let is_sel = self.timeline_snap_mode == mode;
                        let bg = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(28, 32, 40) };
                        let fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                        if ui.add(egui::Button::new(egui::RichText::new(label).strong().size(10.0).color(fg)).fill(bg)).clicked() {
                            self.timeline_snap_mode = mode;
                        }
                    }
                });
            });

            ui.separator();

            // Follow Playhead Toggle
            let auto_label = if self.timeline_auto_scroll { crate::i18n::t("🏃 Följ: PÅ") } else { crate::i18n::t("⏸ Följ: AV") };
            let auto_bg = if self.timeline_auto_scroll { Theme::FL_GREEN } else { Color32::from_rgb(36, 42, 54) };
            let auto_fg = if self.timeline_auto_scroll { Color32::BLACK } else { Theme::TEXT_MUTED };
            let auto_tip = if self.timeline_auto_scroll {
                crate::i18n::t("Tidslinjen rullar automatiskt med spelhuvudet under uppspelning. Klicka för att stänga av.")
            } else {
                crate::i18n::t("Tidslinjen står stilla så du kan redigera i lugn och ro. Klicka för att aktivera följning.")
            };
            if ui.add(egui::Button::new(egui::RichText::new(auto_label).strong().size(10.5).color(auto_fg)).fill(auto_bg)).on_hover_text(auto_tip).clicked() {
                self.timeline_auto_scroll = !self.timeline_auto_scroll;
                self.status_message = crate::tstatus!("Följ spelhuvud / Autoscroll: {}", if self.timeline_auto_scroll { "AKTIVERAD" } else { "INAKTIVERAD" });
            }

            ui.separator();

            // Automation Curves Toggle + Parameter Selector (Fas 5.4)
            ui.group(|ui| {
                ui.set_height(26.0);
                ui.horizontal(|ui| {
                    let auto_on = self.show_automation;
                    let auto_lbl = if auto_on { crate::i18n::t("📈 Automation: PÅ") } else { crate::i18n::t("📈 Automation: AV") };
                    let auto_bg = if auto_on { Theme::FL_CYAN } else { Color32::from_rgb(36, 42, 54) };
                    let auto_fg = if auto_on { Color32::BLACK } else { Theme::TEXT_MUTED };
                    if ui.add(egui::Button::new(egui::RichText::new(auto_lbl).strong().size(10.5).color(auto_fg)).fill(auto_bg))
                        .on_hover_text(crate::i18n::t("Visa/redigera automationskurvor för valt spår. Vänsterklicka i kurvan för att lägga till punkter, dra för att flytta, högerklicka för att ta bort."))
                        .clicked()
                    {
                        self.show_automation = !self.show_automation;
                        self.status_message = crate::tstatus!("Automationskurvor: {}", if self.show_automation { "AKTIVERADE" } else { "INAKTIVERADE" });
                    }
                    if auto_on {
                        for p in AutomationParam::ALL {
                            let mål = AutomationTarget::Track(p);
                            let sel = self.automation_target == mål;
                            let pbg = if sel { Theme::FL_ORANGE } else { Color32::from_rgb(28, 32, 40) };
                            let pfg = if sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                            if ui.add(egui::Button::new(egui::RichText::new(p.label()).strong().size(10.0).color(pfg)).fill(pbg)).clicked() {
                                self.automation_target = mål;
                            }
                        }
                        // **Plugin-målet kan inte vara en fast knapprad** (Fas 8.8): listan
                        // kommer från den laddade pluginen och finns bara när en instans
                        // svarar. Är ingen plugin laddad i spårets slot ritas ingen väljare —
                        // i stället för en tom meny som ser ut som ett fel.
                        let valt_spår = self
                            .selected_timeline_track
                            .min(self.playlist_tracks.len().saturating_sub(1));
                        let params: Vec<(u32, String)> = self
                            .plugin_parameters(valt_spår)
                            .iter()
                            .filter(|p| !p.is_hidden() && !p.is_readonly())
                            .map(|p| {
                                (
                                    p.id,
                                    if p.name.is_empty() { format!("{}", p.id) } else { p.name.clone() },
                                )
                            })
                            .collect();
                        if !params.is_empty() {
                            let nu = match self.automation_target {
                                AutomationTarget::Plugin { track, param_id } if track == valt_spår => {
                                    params.iter().find(|(id, _)| *id == param_id).map(|(_, n)| n.clone())
                                }
                                _ => None,
                            };
                            let spår_namn = self.playlist_tracks[valt_spår].name.clone();
                            egui::ComboBox::from_id_salt("automation_plugin_target")
                                .selected_text(nu.unwrap_or_else(|| {
                                    crate::tstatus!("🔌 {} — {}", crate::i18n::t("Plugin-parameter"), spår_namn)
                                }))
                                .width(190.0)
                                .show_ui(ui, |ui| {
                                    for (id, namn) in &params {
                                        let mål = AutomationTarget::Plugin { track: valt_spår, param_id: *id };
                                        if ui
                                            .selectable_label(self.automation_target == mål, namn)
                                            .clicked()
                                        {
                                            self.automation_target = mål;
                                        }
                                    }
                                });
                        }
                    }
                });
            });

            ui.separator();

            // Zoom Controls Group (Now part of wrapped row, no right_to_left collision!)
            ui.group(|ui| {
                ui.set_height(26.0);
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::t("🔍+")).on_hover_text(crate::i18n::t("Zooma in (Ctrl+Skrolla)")).clicked() {
                        self.suno_zoom_level = (self.suno_zoom_level * 1.25).min(20.0);
                    }
                    let zoom_presets = [(0.5, "50%"), (1.0, "100%"), (2.0, "200%"), (4.0, "400%"), (8.0, "800%")];
                    for (z_val, z_lbl) in zoom_presets {
                        if ui.selectable_label((self.suno_zoom_level - z_val).abs() < 0.1, z_lbl).clicked() {
                            self.suno_zoom_level = z_val;
                        }
                    }
                    if ui.button(crate::i18n::t("🔍-")).on_hover_text(crate::i18n::t("Zooma ut (Ctrl+Skrolla)")).clicked() {
                        self.suno_zoom_level = (self.suno_zoom_level * 0.8).max(0.25);
                    }
                    ui.label(egui::RichText::new(format!("{:.0}%", self.suno_zoom_level * 100.0)).monospace().size(10.0).color(Theme::TEXT_MUTED));
                });
            });
        });
    }

    /// Den flytande AI-capsulen i arrangören (prompt och generering).
    pub(crate) fn draw_arranger_ai_capsule(&mut self, ui: &mut egui::Ui) {
        // ================================================================
        // 3. SUNO STUDIO FLOATING AI PROMPT & GENERATION CAPSULE (BETA)
        // ================================================================
        ui.vertical_centered(|ui| {
            ui.group(|ui| {
                ui.set_max_width(680.0);
                ui.horizontal(|ui| {
                    // Beta Tag
                    ui.label(egui::RichText::new(crate::i18n::t("BETA")).strong().size(9.0).color(Theme::FL_ORANGE));
                    ui.separator();

                    // Add Context (+)
                    if ui.button(crate::i18n::t("➕")).on_hover_text(crate::i18n::t("Bifoga aktuellt spår/mix som AI-kontext")).clicked() {
                        self.suno_context_clip = Some(format!("{} - Master Mix", self.project_name));
                    }

                    // Context Chip
                    let mut clear_context = false;
                    if let Some(ref ctx_clip) = self.suno_context_clip {
                        let fill = Color32::from_rgb(60, 35, 100);
                        let clip_label = format!("🟣 {}", ctx_clip);
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(clip_label).size(10.0).color(Color32::WHITE));
                                if ui.add(egui::Button::new(crate::i18n::t("✕")).small().fill(fill)).clicked() {
                                    clear_context = true;
                                }
                            });
                        });
                    }
                    if clear_context {
                        self.suno_context_clip = None;
                    }

                    // Prompt Text Field
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut self.suno_prompt_input)
                            .hint_text("Select a spot on the timeline and ask me to add a part...")
                            .desired_width(320.0),
                    );

                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        self.trigger_suno_ai_generation();
                    }

                    // Model Selector Pill (Local & Open Source AI Engines)
                    egui::ComboBox::from_id_salt("suno_model_combo")
                        .selected_text(&self.suno_model_version)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut self.suno_model_version, "v5.5".to_string(), "v5.5");
                            ui.selectable_value(&mut self.suno_model_version, "ACE-Step (Lokal Suno)".to_string(), "ACE-Step (Lokal Suno)");
                            ui.selectable_value(&mut self.suno_model_version, "Stable Audio 3.0".to_string(), "Stable Audio 3.0");
                            ui.selectable_value(&mut self.suno_model_version, "Meta MusicGen".to_string(), "MusicGen (Lokal)");
                            ui.selectable_value(&mut self.suno_model_version, "Demucs UVR5".to_string(), "Demucs UVR5");
                        });

                    // Submit / Generate Button (↑)
                    let gen_bg = if self.suno_is_generating { Color32::RED } else { Theme::FL_GREEN };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(" ⬆ ")).strong().size(12.0).color(Color32::WHITE)).fill(gen_bg)).clicked() {
                        self.trigger_suno_ai_generation();
                    }
                });
            });

            ui.add_space(4.0);

            // Center Action: "+ Add Track Effects" Button (Suno Studio Style)
            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("➕ Add Track Effects")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(40, 48, 65))).clicked() {
                self.view_mode = ViewMode::EffectsMixer;
            }
        });
    }

    /// Arrangörens nedre rad: snabbväxlaren, Sound Browser-knappen och guiden.
    pub(crate) fn draw_arranger_utility_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(4.0);

        // ================================================================
        // 4. SUNO STUDIO BOTTOM UTILITY BAR (Quick Switcher, Browser, Guide)
        // ================================================================
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(6.0, 4.0);

            // Mode Switcher Icons
            if ui.selectable_label(self.view_mode == ViewMode::EffectsMixer, "🎛 Mixer").clicked() {
                self.view_mode = ViewMode::EffectsMixer;
            }
            if ui.selectable_label(self.view_mode == ViewMode::PianoRoll, "✏ Piano Roll").clicked() {
                self.view_mode = ViewMode::PianoRoll;
            }
            if ui.selectable_label(self.view_mode == ViewMode::ChannelRack, "☰ Channel Rack").clicked() {
                self.view_mode = ViewMode::ChannelRack;
            }
            if ui.selectable_label(self.view_mode == ViewMode::VocalStudio, "🎤 Vocal Studio").clicked() {
                self.view_mode = ViewMode::VocalStudio;
            }

            ui.separator();

            if ui.button(crate::i18n::t("💡 Snabbguide (F1)")).clicked() {
                self.show_help_guide = !self.show_help_guide;
            }

            if ui.button(crate::i18n::t("🗂 Projektfiler")).clicked() {
                self.show_browser = !self.show_browser;
            }
        });
    }
}

impl SonixApp {
    /// Spårhuvudena — den fasta vänsterkolumnen: namn, ikon, färg, mute/solo, volym,
    /// panning, EQ-kurva, VCA/utgång och de tre menyvalen (duplicera, klistra in, ta bort).
    ///
    /// `deferred` bär de valen vidare till anroparen, som utför dem **efter** att tidlinjen
    /// ritats — att mutera spårlistan medan den läses är hur en krasch uppstår.
    pub(crate) fn draw_track_headers(
        &mut self,
        ui: &mut egui::Ui,
        g: &TimelineGeometry,
        tempo: &crate::audio::tempo::TempoMap,
        deferred: &mut DeferredActions,
    ) {
        let TimelineGeometry { header_w, row_h, ruler_h, .. } = *g;
        let mut track_delete_idx = deferred.track_delete_idx.take();
        let mut track_duplicate_idx = deferred.track_duplicate_idx.take();
        let mut track_paste_idx = deferred.track_paste_idx.take();
        // 2.A FIXED LEFT TRACK HEADERS (Sticky / Frozen Column)
        // ================================================================
        ui.vertical(|ui| {
            ui.set_width(header_w);
            ui.spacing_mut().item_spacing = Vec2::ZERO;

            // Corner Track Header Spacer
            let (corner_rect, _) = ui.allocate_exact_size(Vec2::new(header_w, ruler_h), Sense::hover());
            ui.painter().rect_filled(corner_rect, Rounding::same(3.0), Color32::from_rgb(18, 20, 26));
            ui.painter().text(
                Pos2::new(corner_rect.min.x + 10.0, corner_rect.center().y),
                egui::Align2::LEFT_CENTER,
                crate::i18n::t("SPÅR / INSTRUMENT"),
                egui::FontId::proportional(11.0),
                Theme::TEXT_MUTED,
            );

            ui.add_space(2.0);

            // Track Header Cards (Spår 1 .. N)
            for t_idx in 0..self.playlist_tracks.len() {
                let is_sel_track = self.selected_timeline_track == t_idx;
                let track_color = self.playlist_tracks[t_idx].color;
                let track_muted = self.playlist_tracks[t_idx].muted;
                let track_solo = self.playlist_tracks[t_idx].solo;
                let track_vol = self.playlist_tracks[t_idx].volume;
                let track_name = self.playlist_tracks[t_idx].name.clone();
                let icon = self.playlist_tracks[t_idx].icon;
                let is_vocal_track = self.playlist_tracks[t_idx].kind == TrackKind::VocalAudio;
                let is_mic_track = track_name.to_lowercase().contains("mic")
                    || track_name.to_lowercase().contains("mikrofon")
                    || track_name.to_lowercase().contains("microphone")
                    || (t_idx == self.playlist_tracks.len() - 1 && track_name.to_lowercase().contains("mik"));
                let track_row_h = if is_mic_track { row_h * 2.0 } else { row_h };

                // Left Track Header Card (Sonix Style)
                let (h_rect, h_resp) = ui.allocate_exact_size(Vec2::new(header_w, track_row_h), Sense::click());
                let card_bg = if is_mic_track {
                    if is_sel_track { Color32::from_rgb(255, 255, 255) } else { Color32::from_rgb(248, 250, 254) }
                } else if is_sel_track {
                    Color32::from_rgb(28, 32, 42)
                } else {
                    Color32::from_rgb(18, 21, 28)
                };
                let card_stroke = if is_mic_track {
                    if is_sel_track { Color32::from_rgb(0, 180, 240) } else { Color32::from_rgb(205, 215, 228) }
                } else if is_sel_track {
                    track_color
                } else {
                    Color32::from_rgb(34, 40, 52)
                };
                ui.painter().rect_filled(h_rect, Rounding::same(3.0), card_bg);
                ui.painter().rect_stroke(h_rect, Rounding::same(3.0), Stroke::new(if is_sel_track { 1.5_f32 } else { 1.0_f32 }, card_stroke));

                // Left Accent Bar
                let accent_color = if is_mic_track { Color32::from_rgb(0, 180, 240) } else { track_color };
                let color_bar = Rect::from_min_size(h_rect.min, Vec2::new(4.0, h_rect.height()));
                ui.painter().rect_filled(color_bar, Rounding::same(1.5), accent_color);

                // Track Number & Name
                let display_name = if is_mic_track {
                    format!("{} 🎤 MIKROFON (Huvudinspelning)", t_idx + 1)
                } else if track_name.starts_with(icon) {
                    format!("{} {}", t_idx + 1, track_name)
                } else {
                    format!("{} {} {}", t_idx + 1, icon, track_name)
                };
                let title_col = if is_mic_track {
                    Color32::from_rgb(15, 20, 28)
                } else if is_sel_track {
                    Color32::WHITE
                } else {
                    track_color
                };
                ui.painter().text(
                    Pos2::new(h_rect.min.x + 10.0, h_rect.min.y + if is_mic_track { 16.0 } else { 12.0 }),
                    egui::Align2::LEFT_CENTER,
                    display_name,
                    egui::FontId::proportional(if is_mic_track { 11.5 } else { 11.0 }),
                    title_col,
                );

                let ctrl_y_offset = if is_mic_track { 22.0 } else { 0.0 };

                // Live Input Meter for Mic/Vocal tracks or armed tracks
                let is_armed = self.playlist_tracks[t_idx].is_rec_armed;
                if is_vocal_track || is_armed || is_mic_track {
                    let vu = self.vocal_studio.mic_vu_level;
                    let vu_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 116.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(20.0, if is_mic_track { 28.0 } else { 18.0 }));
                    ui.painter().rect_filled(vu_rect, Rounding::same(2.0), if is_mic_track { Color32::from_rgb(220, 226, 238) } else { Color32::from_rgb(22, 26, 34) });
                    let vu_h = (vu * (if is_mic_track { 24.0 } else { 14.0 })).clamp(2.0, if is_mic_track { 24.0 } else { 14.0 });
                    let vu_fill = Rect::from_min_max(
                        Pos2::new(vu_rect.min.x + 4.0, vu_rect.max.y - 2.0 - vu_h),
                        Pos2::new(vu_rect.max.x - 4.0, vu_rect.max.y - 2.0)
                    );
                    let vu_col = if vu > 0.85 { Color32::from_rgb(255, 60, 60) } else if vu > 0.5 { Color32::from_rgb(255, 200, 40) } else { Color32::from_rgb(0, 190, 240) };
                    ui.painter().rect_filled(vu_fill, Rounding::same(1.0), vu_col);
                }

                // Controls Row inside header (Vol Slider, Pan Slider, Mute, Solo, Record Arm R, Focus Editor 🔍)
                let vol_rect = Rect::from_min_size(Pos2::new(h_rect.min.x + 8.0, h_rect.min.y + 26.0 + ctrl_y_offset), Vec2::new(38.0, 12.0));
                ui.painter().rect_filled(vol_rect, Rounding::same(2.0), if is_mic_track { Color32::from_rgb(210, 218, 230) } else { Color32::from_rgb(30, 35, 45) });
                let vol_fill_w = vol_rect.width() * (track_vol / 1.25).clamp(0.0, 1.0);
                ui.painter().rect_filled(Rect::from_min_size(vol_rect.min, Vec2::new(vol_fill_w, vol_rect.height())), Rounding::same(2.0), accent_color);
                ui.painter().text(vol_rect.center(), egui::Align2::CENTER_CENTER, format!("{:.0}%", track_vol * 100.0), egui::FontId::proportional(8.0), Color32::WHITE);

                // Pan Slider
                let pan_rect = Rect::from_min_size(Pos2::new(h_rect.min.x + 50.0, h_rect.min.y + 26.0 + ctrl_y_offset), Vec2::new(36.0, 12.0));
                ui.painter().rect_filled(pan_rect, Rounding::same(2.0), if is_mic_track { Color32::from_rgb(210, 218, 230) } else { Color32::from_rgb(26, 30, 40) });
                let track_pan = self.playlist_tracks[t_idx].pan;
                let pan_norm = ((track_pan + 1.0) / 2.0).clamp(0.0, 1.0);
                let center_x = pan_rect.center().x;
                let thumb_x = pan_rect.min.x + pan_norm * pan_rect.width();
                let pan_fill_rect = if thumb_x >= center_x {
                    Rect::from_min_max(Pos2::new(center_x, pan_rect.min.y + 2.0), Pos2::new(thumb_x, pan_rect.max.y - 2.0))
                } else {
                    Rect::from_min_max(Pos2::new(thumb_x, pan_rect.min.y + 2.0), Pos2::new(center_x, pan_rect.max.y - 2.0))
                };
                ui.painter().rect_filled(pan_fill_rect, Rounding::same(1.0), Theme::FL_CYAN);
                let pan_str = if track_pan.abs() < 0.05 { "C".to_string() } else if track_pan < 0.0 { format!("L{:.0}", track_pan.abs() * 50.0) } else { format!("R{:.0}", track_pan * 50.0) };
                ui.painter().text(pan_rect.center(), egui::Align2::CENTER_CENTER, pan_str, egui::FontId::proportional(8.0), if is_mic_track { Color32::from_rgb(40, 50, 65) } else { Theme::TEXT_MUTED });

                // Mute icon (speaker)
                let m_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 90.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(18.0, 18.0));
                let m_bg = if track_muted { Color32::from_rgb(180, 40, 40) } else if is_mic_track { Color32::from_rgb(215, 222, 235) } else { Color32::from_rgb(30, 35, 45) };
                ui.painter().rect_filled(m_rect, Rounding::same(2.0), m_bg);
                ui.painter().text(m_rect.center(), egui::Align2::CENTER_CENTER, "🔊", egui::FontId::proportional(8.5), if track_muted { Color32::WHITE } else if is_mic_track { Color32::from_rgb(40, 50, 65) } else { Theme::TEXT_MUTED });

                // Solo (S)
                let s_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 68.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(18.0, 18.0));
                let s_bg = if track_solo { Theme::FL_ORANGE } else if is_mic_track { Color32::from_rgb(215, 222, 235) } else { Color32::from_rgb(30, 35, 45) };
                ui.painter().rect_filled(s_rect, Rounding::same(2.0), s_bg);
                ui.painter().text(s_rect.center(), egui::Align2::CENTER_CENTER, "S", egui::FontId::proportional(9.0), if track_solo { Color32::BLACK } else if is_mic_track { Color32::from_rgb(40, 50, 65) } else { Theme::TEXT_MUTED });

                // Record Arm (R)
                let r_arm_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 46.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(18.0, 18.0));
                let r_arm_bg = if is_armed { Color32::from_rgb(220, 30, 30) } else if is_mic_track { Color32::from_rgb(215, 222, 235) } else { Color32::from_rgb(30, 35, 45) };
                ui.painter().rect_filled(r_arm_rect, Rounding::same(2.0), r_arm_bg);
                ui.painter().text(r_arm_rect.center(), egui::Align2::CENTER_CENTER, "R", egui::FontId::proportional(9.0), if is_armed { Color32::WHITE } else if is_mic_track { Color32::from_rgb(40, 50, 65) } else { Theme::TEXT_MUTED });

                // Stem Focus / Detail Editor button (🔍)
                let f_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 24.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(18.0, 18.0));
                let is_focused = self.show_stem_focus_modal && self.focused_stem_track == Some(t_idx);
                let f_bg = if is_focused { Theme::FL_CYAN } else if is_mic_track { Color32::from_rgb(215, 222, 235) } else { Color32::from_rgb(30, 35, 45) };
                ui.painter().rect_filled(f_rect, Rounding::same(2.0), f_bg);
                ui.painter().text(f_rect.center(), egui::Align2::CENTER_CENTER, "🔍", egui::FontId::proportional(8.5), if is_focused { Color32::BLACK } else if is_mic_track { Color32::from_rgb(40, 50, 65) } else { Theme::TEXT_MUTED });

                if h_resp.clicked() || h_resp.double_clicked() {
                    ui.memory_mut(|m| m.stop_text_input());
                    self.selected_timeline_track = t_idx;
                    if !self.playlist_tracks[t_idx].regions.is_empty() {
                        self.selected_audio_region = Some((t_idx, 0));
                        let reg = &self.playlist_tracks[t_idx].regions[0];
                        self.status_message = crate::tstatus!("Markerade '{}' [Start: {} | Längd: {}]", reg.name, format_time_hundredths(tempo.secs_at_bar(reg.start_bar as f64) as f32), format_time_hundredths(tempo.secs_for_bars_at(reg.start_bar as f64, reg.length_bars as f64) as f32));
                    }
                    if let Some(mouse_pos) = h_resp.hover_pos() {
                        let vu_rect = Rect::from_min_size(Pos2::new(h_rect.max.x - 116.0, h_rect.min.y + 18.0 + ctrl_y_offset), Vec2::new(20.0, if is_mic_track { 28.0 } else { 18.0 }));
                        if (is_vocal_track || is_armed || is_mic_track) && vu_rect.contains(mouse_pos) {
                            self.show_mic_settings_modal = true;
                        } else if m_rect.contains(mouse_pos) {
                            self.playlist_tracks[t_idx].muted = !self.playlist_tracks[t_idx].muted;
                            self.sync_track_audio_state(t_idx);
                        } else if s_rect.contains(mouse_pos) {
                            self.playlist_tracks[t_idx].solo = !self.playlist_tracks[t_idx].solo;
                            self.sync_track_audio_state(t_idx);
                        } else if r_arm_rect.contains(mouse_pos) {
                            self.playlist_tracks[t_idx].is_rec_armed = !self.playlist_tracks[t_idx].is_rec_armed;
                            if self.playlist_tracks[t_idx].is_rec_armed {
                                self.status_message = crate::tstatus!("🔴 Spår {} ({}) är nu armerat för mikrofoninspelning!", t_idx + 1, self.playlist_tracks[t_idx].name);
                            }
                        } else if f_rect.contains(mouse_pos) || h_resp.double_clicked() {
                            self.open_stem_focus(t_idx);
                        } else if vol_rect.contains(mouse_pos) {
                            let ratio = (mouse_pos.x - vol_rect.min.x) / vol_rect.width();
                            self.playlist_tracks[t_idx].volume = (ratio * 1.25).clamp(0.0, 1.25);
                        } else if pan_rect.contains(mouse_pos) {
                            let ratio = (mouse_pos.x - pan_rect.min.x) / pan_rect.width();
                            self.playlist_tracks[t_idx].pan = (ratio * 2.0 - 1.0).clamp(-1.0, 1.0);
                            self.sync_track_audio_state(t_idx);
                        }
                    }
                }

                h_resp.context_menu(|ui| {
                    ui.set_min_width(230.0);
                    ui.label(egui::RichText::new(crate::tstatus!("Spår {}: {}", t_idx + 1, self.playlist_tracks[t_idx].name)).strong().color(Theme::FL_CYAN));
                    ui.separator();
                    if ui.button(crate::i18n::t("📋 Duplicera spår (under detta spår)")).clicked() {
                        track_duplicate_idx = Some(t_idx);
                        ui.close_menu();
                    }
                    if self.copied_region.is_some() {
                        if ui.button(crate::i18n::t("📋 Klistra in sample här (Ctrl+V)")).clicked() {
                            track_paste_idx = Some(t_idx);
                            ui.close_menu();
                        }
                    }
                    if ui.button(crate::i18n::t("➕ Lägg till nytt spår...")).clicked() {
                        self.show_add_track_modal = true;
                        ui.close_menu();
                    }
                    if ui.button(crate::i18n::t("🔴 Spela in på detta spår")).clicked() {
                        for t in &mut self.playlist_tracks { t.is_rec_armed = false; }
                        self.playlist_tracks[t_idx].is_rec_armed = true;
                        self.toggle_timeline_recording();
                        ui.close_menu();
                    }
                    if !is_mic_track && self.playlist_tracks.len() > 1 {
                        ui.separator();
                        if ui.button(egui::RichText::new(crate::i18n::t("🗑 Ta bort spår")).color(Color32::from_rgb(255, 90, 90))).clicked() {
                            track_delete_idx = Some(t_idx);
                            ui.close_menu();
                        }
                    }
                });

                ui.add_space(2.0);
            }

            // "+ Add Track" Action Row on Left
            let (add_rect, add_resp) = ui.allocate_exact_size(Vec2::new(header_w, 28.0), Sense::click());
            let is_h = add_resp.hovered();
            ui.painter().rect_filled(add_rect, Rounding::same(4.0), if is_h { Color32::from_rgb(32, 38, 50) } else { Color32::from_rgb(20, 24, 32) });
            ui.painter().rect_stroke(add_rect, Rounding::same(4.0), Stroke::new(1.0_f32, if is_h { Theme::FL_ORANGE } else { Color32::from_rgb(45, 52, 66) }));
            ui.painter().text(add_rect.center(), egui::Align2::CENTER_CENTER, crate::i18n::t("➕ Lägg till spår"), egui::FontId::proportional(11.0), if is_h { Theme::FL_ORANGE } else { Theme::TEXT_BRIGHT });
            if add_resp.clicked() {
                self.show_add_track_modal = true;
            }

            ui.add_space(4.0);

            // Master "Main" Header Card on Left
            let (m_header_rect, m_header_resp) = ui.allocate_exact_size(Vec2::new(header_w, row_h), Sense::click_and_drag());
            let is_master_muted = self.master_volume < 0.001;
            let card_bg = Color32::from_rgb(18, 22, 32);
            ui.painter().rect_filled(m_header_rect, Rounding::same(3.0), card_bg);
            ui.painter().rect_stroke(m_header_rect, Rounding::same(3.0), Stroke::new(1.0_f32, Theme::FL_ORANGE));

            // Left Accent Bar (Orange for Master)
            let color_bar = Rect::from_min_size(m_header_rect.min, Vec2::new(4.0, m_header_rect.height()));
            ui.painter().rect_filled(color_bar, Rounding::same(1.5), Theme::FL_ORANGE);

            // "Main" Track Label
            ui.painter().text(
                Pos2::new(m_header_rect.min.x + 10.0, m_header_rect.min.y + 12.0),
                egui::Align2::LEFT_CENTER,
                "🎛 Main Master",
                egui::FontId::proportional(11.0),
                Theme::FL_ORANGE,
            );

            // Controls Row: Master Volume Slider & Mute Speaker Icon
            let vol_rect = Rect::from_min_size(Pos2::new(m_header_rect.min.x + 10.0, m_header_rect.min.y + 26.0), Vec2::new(125.0, 12.0));
            ui.painter().rect_filled(vol_rect, Rounding::same(2.0), Color32::from_rgb(30, 35, 45));
            let vol_fill_w = vol_rect.width() * (self.master_volume / 1.25).clamp(0.0, 1.0);
            ui.painter().rect_filled(Rect::from_min_size(vol_rect.min, Vec2::new(vol_fill_w, vol_rect.height())), Rounding::same(2.0), Theme::FL_ORANGE);

            // Mute speaker button
            let m_rect = Rect::from_min_size(Pos2::new(m_header_rect.max.x - 26.0, m_header_rect.min.y + 18.0), Vec2::new(18.0, 18.0));
            let m_bg = if is_master_muted { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(30, 35, 45) };
            ui.painter().rect_filled(m_rect, Rounding::same(2.0), m_bg);
            ui.painter().text(m_rect.center(), egui::Align2::CENTER_CENTER, "🔊", egui::FontId::proportional(9.0), if is_master_muted { Color32::WHITE } else { Theme::TEXT_MUTED });

            if (m_header_resp.clicked() || m_header_resp.dragged())
                && let Some(mouse_pos) = m_header_resp.hover_pos() {
                    if m_rect.contains(mouse_pos) && m_header_resp.clicked() {
                        if is_master_muted {
                            self.master_volume = 0.85;
                        } else {
                            self.master_volume = 0.0;
                        }
                        let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                    } else if vol_rect.contains(mouse_pos) || (mouse_pos.x >= vol_rect.min.x && mouse_pos.x <= vol_rect.max.x && mouse_pos.y >= m_header_rect.min.y + 18.0) {
                        let ratio = ((mouse_pos.x - vol_rect.min.x) / vol_rect.width()).clamp(0.0, 1.0);
                        self.master_volume = (ratio * 1.25).clamp(0.0, 1.25);
                        let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                    }
                }
        });
        deferred.track_delete_idx = track_delete_idx;
        deferred.track_duplicate_idx = track_duplicate_idx;
        deferred.track_paste_idx = track_paste_idx;
    }

    /// Tidlinjens körfält — den vågrätt rullbara ytan: taktlinjalen, spårens körfält,
    /// klippen med sina vågformer, spelhuvudet, slingmarkeringen och automationskurvorna.
    ///
    /// De fyra tidsfrågorna kommer in som referenser: de definieras i anroparen (en ägare per
    /// fråga), och kroppen här är flyttad ordagrant — därför står `secs_at(bar)` kvar som det
    /// gjorde, inte `g.secs_at(bar)`.
    pub(crate) fn draw_timeline_lanes(
        &mut self,
        ui: &mut egui::Ui,
        g: &TimelineGeometry,
        tempo: &crate::audio::tempo::TempoMap,
        secs_at: &dyn Fn(f32) -> f32,
        secs_len: &dyn Fn(f32, f32) -> f32,
        bars_at: &dyn Fn(f32) -> f32,
        sec_per_bar_at: &dyn Fn(f32) -> f32,
        bars_len: &dyn Fn(f32, f32) -> f32,
        deferred: &mut DeferredActions,
    ) {
        let TimelineGeometry { bar_w, row_h, ruler_h, total_bars, timeline_total_w, .. } = *g;
        let mut scrub_target_sec = deferred.scrub_target_sec.take();
        let mut drop_sample_action = deferred.drop_sample_action.take();
        // Körfälten rör inte spårlistan — bara huvudena gör det — så den här är bara med
        // för att lämnas tillbaka orörd (spårhuvudet kan ha satt den före oss).
        let track_delete_idx = deferred.track_delete_idx.take();
        let mut track_duplicate_idx = deferred.track_duplicate_idx.take();
        let mut track_paste_idx = deferred.track_paste_idx.take();
        // ================================================================
        // 2.B HORIZONTALLY SCROLLABLE TIMELINE & WAVEFORM LANES
        // ================================================================
        egui::ScrollArea::horizontal()
            .id_salt("sonix_arranger_timeline_lanes_scroll")
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                    // 1. Timeline Ruler Strip
                    let (ruler_rect, ruler_resp) = ui.allocate_exact_size(Vec2::new(timeline_total_w, ruler_h), Sense::click_and_drag());
                    let timeline_left_x = ruler_rect.min.x;
                    // Högerklick på linjalen: sätt eller ta bort ett tempobyte
                    // (Fas 8.2). Taktnumret räknas ut HÄR, på klicket — inne i
                    // menyn står pekaren över menyn, inte över linjalen, och då
                    // hade bytet hamnat på fel takt.
                    if ruler_resp.secondary_clicked()
                        && let Some(pos) = ruler_resp.interact_pointer_pos()
                    {
                        let bar =
                            (((pos.x - ruler_rect.min.x) / bar_w).floor().max(0.0)) as u32;
                        self.tempo_menu_bar = Some(bar);
                    }
                    let mut tempo_set_here = false;
                    let mut tempo_remove_here = false;
                    ruler_resp.clone().context_menu(|ui| {
                        let bar = self.tempo_menu_bar.unwrap_or(0);
                        ui.label(crate::tstatus!("Takt {}", bar + 1));
                        if ui.button(crate::i18n::t("⏱ Sätt tempo här")).clicked() {
                            tempo_set_here = true;
                            ui.close_menu();
                        }
                        let has_point =
                            self.tempo_points.iter().any(|p| p.start_bar == bar);
                        if has_point
                            && ui
                                .button(crate::i18n::t("🗑 Ta bort tempobyte här"))
                                .clicked()
                        {
                            tempo_remove_here = true;
                            ui.close_menu();
                        }
                    });
                    if tempo_set_here {
                        let bar = self.tempo_menu_bar.unwrap_or(0);
                        let bpm = self.tempo_map().bpm_at(bar as f64);
                        self.set_tempo_point(bar, bpm);
                    } else if tempo_remove_here {
                        self.remove_tempo_point(self.tempo_menu_bar.unwrap_or(0));
                    }
                    ui.painter().rect_filled(ruler_rect, Rounding::ZERO, Color32::from_rgb(16, 19, 26));
                    ui.painter().line_segment([Pos2::new(ruler_rect.min.x, ruler_rect.max.y), Pos2::new(ruler_rect.max.x, ruler_rect.max.y)], Stroke::new(1.0_f32, Color32::from_rgb(45, 52, 68)));

                    // Highlight loop region on ruler
                    let loop_start_x = ruler_rect.min.x + self.loop_start_bar as f32 * bar_w;
                    let loop_end_x = ruler_rect.min.x + self.loop_end_bar as f32 * bar_w;
                    let loop_rect = Rect::from_min_max(Pos2::new(loop_start_x, ruler_rect.min.y), Pos2::new(loop_end_x, ruler_rect.max.y));
                    ui.painter().rect_filled(loop_rect, Rounding::ZERO, Color32::from_rgb(26, 36, 52));

                    // Song section markers (created by the Song Structure arranger)
                    for marker in &self.song_markers {
                        let mx = ruler_rect.min.x + marker.start_bar as f32 * bar_w;
                        let mw = (marker.length_bars as f32 * bar_w).max(2.0);
                        let band = Rect::from_min_size(
                            Pos2::new(mx, ruler_rect.max.y - 6.0),
                            Vec2::new(mw, 6.0),
                        );
                        ui.painter().rect_filled(band, Rounding::ZERO, marker.color);
                        ui.painter().line_segment(
                            [Pos2::new(mx, ruler_rect.min.y), Pos2::new(mx, ruler_rect.max.y)],
                            Stroke::new(1.5_f32, marker.color),
                        );
                        if mw > 28.0 {
                            ui.painter().text(
                                Pos2::new(mx + 4.0, ruler_rect.max.y - 8.0),
                                egui::Align2::LEFT_BOTTOM,
                                &marker.name,
                                egui::FontId::proportional(9.0),
                                marker.color,
                            );
                        }
                    }

                    // Draw dynamic tick marks & time codes
                    for bar_idx in 0..total_bars {
                        let bar_start_x = ruler_rect.min.x + bar_idx as f32 * bar_w;
                        let bar_time_sec = tempo.secs_at_bar(bar_idx as f64) as f32;

                        // Major Bar Tick Line
                        ui.painter().line_segment(
                            [Pos2::new(bar_start_x, ruler_rect.min.y), Pos2::new(bar_start_x, ruler_rect.max.y)],
                            Stroke::new(1.0_f32, Color32::from_rgb(65, 75, 95)),
                        );

                        // Major Bar Number Label
                        ui.painter().text(
                            Pos2::new(bar_start_x + 4.0, ruler_rect.min.y + 10.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{}", bar_idx + 1),
                            egui::FontId::proportional(11.0),
                            Theme::FL_CYAN,
                        );

                        // Major Time Code (MM:SS.cs)
                        ui.painter().text(
                            Pos2::new(bar_start_x + 4.0, ruler_rect.min.y + 22.0),
                            egui::Align2::LEFT_CENTER,
                            format_time_hundredths(bar_time_sec),
                            egui::FontId::proportional(9.0),
                            Theme::TEXT_MUTED,
                        );

                        // Beat Ticks (Rendered when bar_w >= 70.0)
                        if bar_w >= 70.0 {
                            for beat in 1..4 {
                                let bx = bar_start_x + beat as f32 * (bar_w / 4.0);
                                ui.painter().line_segment(
                                    [Pos2::new(bx, ruler_rect.min.y + 14.0), Pos2::new(bx, ruler_rect.max.y)],
                                    Stroke::new(0.8_f32, Color32::from_rgb(45, 52, 68)),
                                );
                                if bar_w >= 140.0 {
                                    ui.painter().text(
                                        Pos2::new(bx + 2.0, ruler_rect.min.y + 22.0),
                                        egui::Align2::LEFT_CENTER,
                                        format!(".{}", beat + 1),
                                        egui::FontId::proportional(8.5),
                                        Color32::from_rgb(110, 120, 140),
                                    );
                                }
                            }
                        }

                        // 16th-Step Ticks (Rendered when bar_w >= 220.0)
                        if bar_w >= 220.0 {
                            for step in 1..16 {
                                if step % 4 != 0 {
                                    let sx = bar_start_x + step as f32 * (bar_w / 16.0);
                                    ui.painter().line_segment(
                                        [Pos2::new(sx, ruler_rect.min.y + 22.0), Pos2::new(sx, ruler_rect.max.y)],
                                        Stroke::new(0.5_f32, Color32::from_rgb(38, 44, 56)),
                                    );
                                }
                            }
                        }

                        // Sub-second / Hundredth Precision Ticks (Rendered when bar_w >= 500.0)
                        if bar_w >= 500.0 {
                            // Inne i en takt är tempot konstant, så delstrecken räknas
                            // med **den taktens** sekunder per takt — inte projektets.
                            let bar_secs = tempo.secs_per_bar_at(bar_idx as f64) as f32;
                            let tenths = (bar_secs * 10.0) as usize;
                            for t in 1..tenths {
                                let tx = bar_start_x + (t as f32 * 0.10) * (bar_w / bar_secs);
                                if tx < bar_start_x + bar_w - 2.0 {
                                    ui.painter().line_segment(
                                        [Pos2::new(tx, ruler_rect.min.y + 25.0), Pos2::new(tx, ruler_rect.max.y)],
                                        Stroke::new(0.5_f32, Color32::from_rgb(70, 80, 100)),
                                    );
                                }
                            }
                        }
                    }

                    // Tempobyten i linjalen (Fas 8.2 — synliga sedan 8.12).
                    //
                    // Bytet fanns bara i ⏱ Tempokarta-listan; i linjalen syntes
                    // ingenting, så en låt med tre tempon såg ut som en med ett.
                    // Strecket gäller från sin takt och framåt, och BPM står
                    // strax till VÄNSTER om strecket — taktnumret står till höger
                    // om samma linje, så de två krockar inte.
                    //
                    // Ritas efter tick-varvet: en taktlinje ligger på samma x,
                    // och ritades bytet först skulle den grå linjen lägga sig
                    // över markeringen.
                    for point in &self.tempo_points {
                        let px = ruler_rect.min.x + point.start_bar as f32 * bar_w;
                        ui.painter().line_segment(
                            [
                                Pos2::new(px, ruler_rect.min.y),
                                Pos2::new(px, ruler_rect.max.y),
                            ],
                            Stroke::new(2.0_f32, Theme::FL_YELLOW),
                        );
                        ui.painter().text(
                            Pos2::new(px - 3.0, ruler_rect.min.y + 6.0),
                            egui::Align2::RIGHT_CENTER,
                            format!("⏱{:.0}", point.bpm),
                            egui::FontId::proportional(9.5),
                            Theme::FL_YELLOW,
                        );
                    }

                    // Ruler Interaction: Click / Drag / Scrub with high hundredth-second precision
                    if ruler_resp.clicked() || ruler_resp.dragged() {
                        ui.memory_mut(|m| m.stop_text_input());
                        if let Some(mouse_pos) = ruler_resp.hover_pos() {
                            let click_bar = (mouse_pos.x - ruler_rect.min.x) / bar_w;
                            // Snäppningen sker i TAKTER, inte i sekunder: ett
                            // 16-delssteg är en plats i takten, och över ett
                            // tempobyte finns inget enda "sekunder per takt" att
                            // räkna steg med. Sekunderna kommer ur kartan på ett
                            // ställe, efteråt.
                            let tempo = self.tempo_map();
                            let target_sec = match self.timeline_snap_mode {
                                TimeSnapMode::FreeHundredth => {
                                    // Hundradelar är en TID, inte en plats:
                                    // där är sekunden själv enheten.
                                    let raw_sec = tempo.secs_at_bar(click_bar as f64) as f32;
                                    (raw_sec * 100.0).round() / 100.0
                                }
                                TimeSnapMode::Snap16th => {
                                    let bar = (click_bar * 16.0).round() / 16.0;
                                    tempo.secs_at_bar(bar as f64) as f32
                                }
                                TimeSnapMode::SnapBeat => {
                                    let bar = (click_bar * 4.0).round() / 4.0;
                                    tempo.secs_at_bar(bar as f64) as f32
                                }
                                TimeSnapMode::SnapBar => {
                                    tempo.secs_at_bar(click_bar.round() as f64) as f32
                                }
                            };
                            scrub_target_sec = Some(target_sec.max(0.0));
                            self.pattern_mode = false;
                        }
                    }

                    // Ruler Hover Tooltip with exact hundredths
                    if let Some(hover_pos) = ruler_resp.hover_pos() {
                        let h_bar = (hover_pos.x - ruler_rect.min.x) / bar_w;
                        let h_sec = (self.tempo_map().secs_at_bar(h_bar as f64) as f32)
                            .max(0.0);
                        ruler_resp.clone().on_hover_text(format!("⏱ {} {}\n{}", crate::i18n::t("Tid:"), format_time_hundredths(h_sec), format_bar_subdivisions(h_bar)));
                    }

                    ui.add_space(2.0);

                    // 2. Track Lanes (Audio Waveforms, Clips, Grids)
                    for t_idx in 0..self.playlist_tracks.len() {
                        // Vågformscachen byggs här och inte på de tio ställen
                        // där pcm_audio sätts (Fas 8.3).
                        self.ensure_waveform_cache(t_idx);
                        let track_color = self.playlist_tracks[t_idx].color;
                        let track_name = self.playlist_tracks[t_idx].name.clone();
                        let is_mic_track = track_name.to_lowercase().contains("mic")
                            || track_name.to_lowercase().contains("mikrofon")
                            || track_name.to_lowercase().contains("microphone")
                            || (t_idx == self.playlist_tracks.len() - 1 && track_name.to_lowercase().contains("mik"));
                        let track_row_h = if is_mic_track { row_h * 2.0 } else { row_h };

                        // Full Timeline Track Lane (Scalable width with total_bars)
                        let (lane_rect, lane_resp) = ui.allocate_exact_size(Vec2::new(timeline_total_w, track_row_h), Sense::click_and_drag());

                        // Draw Background Bar Grid Lines
                        for bar_i in 0..total_bars {
                            let b_min_x = lane_rect.min.x + bar_i as f32 * bar_w;
                            let b_rect = Rect::from_min_size(Pos2::new(b_min_x, lane_rect.min.y), Vec2::new(bar_w, track_row_h));
                            let is_active_bar = !self.pattern_mode && self.is_playing && self.song_bar == bar_i;
                            let bg = if is_mic_track {
                                if is_active_bar {
                                    Color32::from_rgb(228, 240, 255)
                                } else if (bar_i / 4) % 2 == 0 {
                                    Color32::from_rgb(255, 255, 255)
                                } else {
                                    Color32::from_rgb(247, 249, 254)
                                }
                            } else if is_active_bar {
                                Color32::from_rgb(32, 44, 58)
                            } else if (bar_i / 4) % 2 == 0 {
                                Color32::from_rgb(16, 18, 24)
                            } else {
                                Color32::from_rgb(22, 25, 32)
                            };
                            ui.painter().rect_filled(b_rect, Rounding::ZERO, bg);
                            let grid_stroke_col = if is_mic_track { Color32::from_rgb(218, 225, 238) } else { Color32::from_rgb(36, 42, 54) };
                            ui.painter().line_segment([Pos2::new(b_min_x, lane_rect.min.y), Pos2::new(b_min_x, lane_rect.max.y)], Stroke::new(0.5_f32, grid_stroke_col));

                            // Beat Sub-Grid Lines when zoomed in
                            if bar_w >= 70.0 {
                                let beat_col = if is_mic_track { Color32::from_rgb(232, 236, 245) } else { Color32::from_rgb(26, 30, 40) };
                                for beat in 1..4 {
                                    let bx = b_min_x + beat as f32 * (bar_w / 4.0);
                                    ui.painter().line_segment([Pos2::new(bx, lane_rect.min.y), Pos2::new(bx, lane_rect.max.y)], Stroke::new(0.5_f32, beat_col));
                                }
                            }

                            // 16th-note sub-grid lines when zoomed in
                            if bar_w >= 220.0 {
                                let step_col = if is_mic_track { Color32::from_rgb(240, 243, 250) } else { Color32::from_rgb(20, 24, 32) };
                                for step in 1..16 {
                                    if step % 4 != 0 {
                                        let sx = b_min_x + step as f32 * (bar_w / 16.0);
                                        ui.painter().line_segment([Pos2::new(sx, lane_rect.min.y), Pos2::new(sx, lane_rect.max.y)], Stroke::new(0.3_f32, step_col));
                                    }
                                }
                            }
                        }

                        // Sound Browser Sample Drag & Drop Ghost Preview onto this Track Lane
                        if let Some(ref drag_item) = self.sample_drag_item {
                            let pointer_opt = ui.ctx().pointer_latest_pos()
                                .or_else(|| ui.ctx().pointer_interact_pos())
                                .or_else(|| ui.input(|i| i.pointer.hover_pos()))
                                .or_else(|| lane_resp.hover_pos());
                            if let Some(mouse_pos) = pointer_opt {
                                if lane_rect.contains(mouse_pos) {
                                    // **Snäppet sker i takter** (Fas 8.2): ett
                                    // sextondelssteg är en *plats i takten*, inte ett antal
                                    // sekunder — och sekunder-per-takt är inte ett tal över
                                    // ett tempobyte. Hundradelarna är undantaget: där *är*
                                    // sekunden enheten, och då får kartan svara.
                                    let raw_drop_bar = ((mouse_pos.x - lane_rect.min.x) / bar_w).max(0.0);
                                    let drop_bar = if self.timeline_snap_mode == TimeSnapMode::FreeHundredth {
                                        // Hundradelarna är undantaget: där är sekunden enheten.
                                        let raw_sec = tempo.secs_at_bar(raw_drop_bar as f64) as f32;
                                        let sec = (raw_sec * 100.0).round() / 100.0;
                                        tempo.bar_at_secs(sec as f64) as f32
                                    } else {
                                        snap_bar(self.timeline_snap_mode, raw_drop_bar)
                                    };
                                    let item_dur_sec: f32 = 2.0;
                                    let item_dur_bars =
                                        (item_dur_sec / tempo.secs_per_bar_at(drop_bar as f64) as f32).max(0.25);
                                    let ghost_x_start = lane_rect.min.x + drop_bar * bar_w;
                                    let ghost_x_end = (ghost_x_start + item_dur_bars * bar_w).min(lane_rect.max.x);
                                    let ghost_rect = Rect::from_min_max(
                                        Pos2::new(ghost_x_start, lane_rect.min.y + 2.0),
                                        Pos2::new(ghost_x_end, lane_rect.max.y - 2.0),
                                    );
                                    ui.painter().rect_filled(ghost_rect, Rounding::same(4.0), Color32::from_rgba_unmultiplied(255, 170, 0, 85));
                                    ui.painter().rect_stroke(ghost_rect, Rounding::same(4.0), Stroke::new(1.8_f32, Theme::FL_ORANGE));
                                    ui.painter().text(
                                        Pos2::new(ghost_rect.min.x + 6.0, ghost_rect.center().y),
                                        egui::Align2::LEFT_CENTER,
                                        crate::tstatus!("📥 Släpp här: {} (Takt {:.2})", drag_item.name, drop_bar + 1.0),
                                        egui::FontId::proportional(10.0),
                                        Color32::WHITE,
                                    );
                                    if ui.input(|i| i.pointer.any_released() || i.pointer.primary_released()) {
                                        drop_sample_action = Some((t_idx, drag_item.clone(), drop_bar));
                                    }
                                }
                            }
                        }

                        // Bottom line separator between tracks
                        ui.painter().line_segment([Pos2::new(lane_rect.min.x, lane_rect.max.y), Pos2::new(lane_rect.max.x, lane_rect.max.y)], Stroke::new(0.5_f32, if is_mic_track { Color32::from_rgb(200, 210, 225) } else { Color32::from_rgb(30, 36, 48) }));

                        // Render continuous Audio Regions on this track
                        let mut split_action: Option<(usize, f32)> = None;
                        let mut delete_action: Option<usize> = None;
                        let mut mute_action: Option<usize> = None;
                        let mut select_action: Option<usize> = None;
                        let mut clip_action: Option<(usize, Option<usize>)> = None;

                        {
                            let (has_regions, regions_snapshot) = if t_idx < self.playlist_tracks.len() {
                                (!self.playlist_tracks[t_idx].regions.is_empty(), self.playlist_tracks[t_idx].regions.clone())
                            } else {
                                (false, Vec::new())
                            };

                            if has_regions {
                                for (r_i, region) in regions_snapshot.iter().enumerate() {
                                    let is_region_selected = self.selected_audio_region == Some((t_idx, r_i));
                                    let rx_start = lane_rect.min.x + region.start_bar * bar_w;
                                    let rx_end = rx_start + region.length_bars * bar_w;
                                    let r_rect = Rect::from_min_max(Pos2::new(rx_start + 1.5, lane_rect.min.y + 2.0), Pos2::new(rx_end - 1.5, lane_rect.max.y - 2.0));

                                    if r_rect.width() > 1.0 {
                                        let col = if is_mic_track {
                                            Color32::WHITE
                                        } else if region.color == default_region_color() || region.color == Color32::from_rgb(100, 180, 255) {
                                            track_color
                                        } else {
                                            region.color
                                        };
                                        let fill = if is_mic_track {
                                            if region.muted {
                                                Color32::from_rgb(225, 228, 235)
                                            } else if is_region_selected {
                                                Color32::from_rgb(255, 255, 255)
                                            } else {
                                                Color32::from_rgb(252, 254, 255)
                                            }
                                        } else if region.muted {
                                            Color32::from_rgb(24, 26, 32)
                                        } else if is_region_selected {
                                            Color32::from_rgb(
                                                (col.r() as f32 * 0.45).min(255.0) as u8,
                                                (col.g() as f32 * 0.45).min(255.0) as u8,
                                                (col.b() as f32 * 0.45).min(255.0) as u8,
                                            )
                                        } else {
                                            Color32::from_rgb(
                                                (col.r() as f32 * 0.28) as u8,
                                                (col.g() as f32 * 0.28) as u8,
                                                (col.b() as f32 * 0.28) as u8,
                                            )
                                        };

                                        ui.painter().rect_filled(r_rect, Rounding::same(4.0), fill);

                                        // Top Header Banner strip for Region
                                        let header_h = 14.0_f32;
                                        let header_rect = Rect::from_min_max(r_rect.min, Pos2::new(r_rect.max.x, (r_rect.min.y + header_h).min(r_rect.max.y)));
                                        let header_bg = if is_mic_track {
                                            if region.muted {
                                                Color32::from_rgb(205, 210, 220)
                                            } else if is_region_selected {
                                                Color32::from_rgb(215, 230, 250)
                                            } else {
                                                Color32::from_rgb(235, 240, 248)
                                            }
                                        } else if region.muted {
                                            Color32::from_rgb(36, 40, 48)
                                        } else if is_region_selected {
                                            Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 190)
                                        } else {
                                            Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 120)
                                        };
                                        ui.painter().rect_filled(header_rect, Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 }, header_bg);

                                        let stroke_col = if is_region_selected {
                                            if is_mic_track { Color32::from_rgb(0, 180, 240) } else { Color32::WHITE }
                                        } else if region.muted {
                                            Color32::from_rgb(70, 75, 85)
                                        } else if is_mic_track {
                                            Color32::from_rgb(180, 190, 205)
                                        } else {
                                            col
                                        };
                                        ui.painter().rect_stroke(r_rect, Rounding::same(4.0), Stroke::new(if is_region_selected { 2.0_f32 } else { 1.2_f32 }, stroke_col));

                                        // Region Header Banner & Exact Time readout (Start & Length down to hundredths!)
                                        let gain_db = if region.volume <= 0.001 { -60.0 } else { 20.0 * region.volume.log10() };
                                        let r_start_str = format_time_hundredths(tempo.secs_at_bar(region.start_bar as f64) as f32);
                                        let r_len_str = format_time_hundredths(tempo.secs_for_bars_at(region.start_bar as f64, region.length_bars as f64) as f32);
                                        let rev_tag = if region.is_reverse { " 🔄[REV]" } else { "" };
                                        let loop_tag = if region.loop_length_bars > 0.001 && region.length_bars > region.loop_length_bars + 0.01 {
                                            let reps = (region.length_bars / region.loop_length_bars).ceil() as usize;
                                            format!(" 🔁[x{}]", reps)
                                        } else {
                                            "".to_string()
                                        };
                                        let title_text = format!("{}{rev_tag}{loop_tag} [⏱ {} | 📏 {} | {:+.1}dB]", region.name, r_start_str, r_len_str, gain_db);

                                        let header_text_col = if is_mic_track {
                                            Color32::from_rgb(18, 22, 30)
                                        } else if region.muted {
                                            Color32::from_rgb(140, 140, 150)
                                        } else {
                                            Color32::WHITE
                                        };

                                        ui.painter().text(
                                            Pos2::new(r_rect.min.x + 5.0, r_rect.min.y + 7.0),
                                            egui::Align2::LEFT_CENTER,
                                            title_text,
                                            egui::FontId::proportional(9.0),
                                            header_text_col,
                                        );

                                        // Draw Fade-in / Fade-out curve indicators
                                        if region.fade_in_bars > 0.01 {
                                            let fi_w = (region.fade_in_bars * bar_w).min(r_rect.width() * 0.5);
                                            let fi_pts = [
                                                Pos2::new(r_rect.min.x, r_rect.max.y),
                                                Pos2::new(r_rect.min.x + fi_w, r_rect.min.y),
                                            ];
                                            ui.painter().line_segment(fi_pts, Stroke::new(1.2_f32, Theme::FL_CYAN));
                                        }
                                        if region.fade_out_bars > 0.01 {
                                            let fo_w = (region.fade_out_bars * bar_w).min(r_rect.width() * 0.5);
                                            let fo_pts = [
                                                Pos2::new(r_rect.max.x - fo_w, r_rect.min.y),
                                                Pos2::new(r_rect.max.x, r_rect.max.y),
                                            ];
                                            ui.painter().line_segment(fo_pts, Stroke::new(1.2_f32, Theme::FL_ORANGE));
                                        }

                                        // Draw Continuous Soundtrap-Style Waveform Inside Region
                                        let mid_y = r_rect.center().y + 3.0;
                                        let wave_col = if is_mic_track {
                                            if region.muted {
                                                Color32::from_rgb(120, 125, 135)
                                            } else if is_region_selected {
                                                Color32::from_rgb(0, 0, 0)
                                            } else {
                                                Color32::from_rgb(15, 18, 24) // Jet Black Waveform!
                                            }
                                        } else if region.muted {
                                            Color32::from_rgb(90, 95, 105)
                                        } else if is_region_selected {
                                            Color32::WHITE
                                        } else {
                                            Color32::from_rgb(
                                                ((col.r() as f32 * 0.85) + 38.0).min(255.0) as u8,
                                                ((col.g() as f32 * 0.85) + 38.0).min(255.0) as u8,
                                                ((col.b() as f32 * 0.85) + 38.0).min(255.0) as u8,
                                            )
                                        };

                                        let is_looped = region.loop_length_bars > 0.001 && region.length_bars > region.loop_length_bars + 0.01;
                                        let loop_sec = if is_looped {
                                            secs_len(region.start_bar, region.loop_length_bars)
                                        } else {
                                            if region.length_bars > 0.001 { secs_len(region.start_bar, region.length_bars) } else { 1.0 }
                                        };

                                        // Fas 8.3: rita ur spårets cache när den finns —
                                        // ett äkta (min, max) per bildpunkt, i stället för
                                        // en fast array utsträckt över regionens bredd.
                                        //
                                        // En slingad region hoppas över: dess bildpunkter
                                        // följer upprepningen, inte en sammanhängande
                                        // sampelmängd, och det är en egen fråga.
                                        let mut drew_exact = false;
                                        if region.length_bars > 0.001
                                            && let Some(wave) = {
                                                let track = &self.playlist_tracks[t_idx];
                                                track.waveform_cache.as_ref().and_then(|(_, c)| {
                                                    track
                                                        .frozen_pcm
                                                        .as_ref()
                                                        .or(track.pcm_audio.as_ref())
                                                        .map(|(l, _, sr)| (c, l.as_slice(), *sr))
                                                })
                                            }
                                        {
                                            // Hela rektangeln, utan indrag: indraget
                                            // försköt hela mappningen, och det är
                                            // precis då startpunkten blir svår att pricka.
                                            let draw_start_x = r_rect.min.x;
                                            let draw_end_x = r_rect.max.x;
                                            let (cache, pcm, sr) = wave;
                                            let region_secs = self
                                                .tempo_map()
                                                .secs_for_bars_at(
                                                    region.start_bar as f64,
                                                    region.length_bars as f64,
                                                ) as f32;
                                            let start_sample =
                                                (region.sample_offset_sec.max(0.0) * sr as f32)
                                                    as usize;
                                            // Klippet följer tempot (Fas 8.10): källan en
                                            // region täcker är längre än dess tid på
                                            // tidslinjen när den spelades in i ett lägre
                                            // tempo. Utan faktorn här skulle vågformen visa
                                            // ett annat stycke än det som hörs — samma sorts
                                            // lögn som 8.3 stängde.
                                            // Tempot **där klippet ligger** — samma regel
                                            // som de två andra ställena (8.10 punkt 1), så
                                            // vyn och ljudet inte kan visa olika tempobyten.
                                            let region_rate = stretch_ratio_for(
                                                region.source_bpm,
                                                self.tempo_map()
                                                    .bpm_at(region.start_bar as f64),
                                            );
                                            let total_samples = region_source_span_samples(
                                                region_secs,
                                                region_rate,
                                                sr,
                                            );
                                            let cols =
                                                ((draw_end_x - draw_start_x).max(1.0)).ceil() as usize;
                                            // En slingad kloss upprepar ett kortare
                                            // stycke: då följer bildpunkterna
                                            // upprepningen, inte en sammanhängande
                                            // sampelmängd.
                                            let loop_samples = region_source_span_samples(
                                                loop_sec,
                                                region_rate,
                                                sr,
                                            );
                                            let env = if is_looped && loop_sec > 0.01 {
                                                cache.envelope_looped(
                                                    pcm,
                                                    start_sample,
                                                    loop_samples,
                                                    total_samples,
                                                    cols,
                                                )
                                            } else {
                                                cache.envelope_at(
                                                    pcm,
                                                    start_sample,
                                                    total_samples,
                                                    cols,
                                                )
                                            };
                                            let half = r_rect.height() * 0.42;
                                            let vol = region.volume.clamp(0.2, 1.8);
                                            // dB-skalan (Fas 8.4): utan den är
                                            // svaga partier i praktiken osynliga.
                                            // Audacity: örat behöver −18 dB för
                                            // halva styrkan, linjärt räcker −6.
                                            const WAVE_SCALE: crate::audio::waveform::AmplitudeScale =
                                                crate::audio::waveform::AmplitudeScale::Decibel;
                                            // På pixelcentra: en en-pixels linje som
                                            // hamnar mellan två pixlar flyter ut över
                                            // båda och ser mjuk ut. Halva pixeln är
                                            // centrum i egui.
                                            let start_x = draw_start_x.round() + 0.5;
                                            // Läget avgör vad som ritas (Fas 8.4): ett
                                            // hölje är en approximation, och den riktiga
                                            // vågformen finns bara där varje sampel får
                                            // sin egen bildpunkt. Att rita finare kolumner
                                            // gör den aldrig tydligare — representationen
                                            // måste bytas.
                                            match crate::audio::waveform::zoom_regime(
                                                total_samples as f64 / cols as f64,
                                            ) {
                                                crate::audio::waveform::ZoomRegime::Envelope => {
                                                    let mut x = start_x;
                                                    for (lo, hi) in &env {
                                                        // Toppen är max och botten är min: en
                                                        // osymmetrisk signal ska se osymmetrisk ut.
                                                        let top = mid_y
                                                            - (crate::audio::waveform::amplitude_curve(*hi, WAVE_SCALE)
                                                                * half
                                                                * vol)
                                                                .min(half);
                                                        let bot = mid_y
                                                            - (crate::audio::waveform::amplitude_curve(*lo, WAVE_SCALE)
                                                                * half
                                                                * vol)
                                                                .max(-half);
                                                        ui.painter().line_segment(
                                                            [
                                                                Pos2::new(x, top),
                                                                Pos2::new(
                                                                    x,
                                                                    bot.max(top + 0.5),
                                                                ),
                                                            ],
                                                            Stroke::new(1.0_f32, wave_col),
                                                        );
                                                        x += 1.0;
                                                    }
                                                }
                                                regime => {
                                                    // Ett sampel per bildpunkt: kurva genom
                                                    // samplarna, och punkter när de hunnit
                                                    // åtskilda (som Audacity).
                                                    let dots = regime
                                                        == crate::audio::waveform::ZoomRegime::SampleDots;
                                                    let mut prev: Option<Pos2> = None;
                                                    for i in 0..cols {
                                                        let k = if is_looped
                                                            && loop_samples > 0
                                                        {
                                                            start_sample
                                                                + (i * total_samples / cols)
                                                                    % loop_samples
                                                        } else {
                                                            start_sample
                                                                + i * total_samples / cols
                                                        };
                                                        let v = pcm.get(k).copied().unwrap_or(0.0);
                                                        let y = mid_y
                                                            - (crate::audio::waveform::amplitude_curve(v, WAVE_SCALE)
                                                                * half
                                                                * vol)
                                                                .clamp(-half, half);
                                                        let x = start_x + i as f32;
                                                        let p = Pos2::new(x, y);
                                                        if let Some(q) = prev {
                                                            ui.painter().line_segment(
                                                                [q, p],
                                                                Stroke::new(1.0_f32, wave_col),
                                                            );
                                                        }
                                                        if dots {
                                                            ui.painter().circle_filled(
                                                                p, 1.6, wave_col,
                                                            );
                                                        }
                                                        prev = Some(p);
                                                    }
                                                }
                                            }
                                            drew_exact = true;
                                        }

                                        // Den gamla vägen, för regioner utan PCM och för
                                        // slingade regioner. `drew_exact` gör att den inte
                                        // ritar ovanpå det exakta höljet.
                                        let num_peaks = if drew_exact {
                                            0
                                        } else {
                                            region.waveform_peaks.len()
                                        };
                                        if num_peaks > 0 {
                                            // Draw subtle center baseline
                                            ui.painter().line_segment(
                                                [Pos2::new(r_rect.min.x + 2.0, mid_y), Pos2::new(r_rect.max.x - 2.0, mid_y)],
                                                Stroke::new(0.8_f32, if is_mic_track { Color32::from_rgb(70, 75, 85) } else { Color32::from_rgba_unmultiplied(wave_col.r(), wave_col.g(), wave_col.b(), 60) })
                                            );

                                            // Draw vertical waveform bars across the entire region width
                                            let draw_start_x = r_rect.min.x + 2.0;
                                            let draw_end_x = r_rect.max.x - 2.0;
                                            let step_px = 3.0_f32;
                                            let mut cur_x = draw_start_x;
                                            while cur_x <= draw_end_x {
                                                let rel_x = cur_x - r_rect.min.x;
                                                let rel_time = secs_at(region.start_bar + rel_x / bar_w);

                                                let sample_time = if is_looped && loop_sec > 0.01 {
                                                    (region.sample_offset_sec + rel_time) % loop_sec
                                                } else {
                                                    region.sample_offset_sec + rel_time
                                                };

                                                let norm_pos = if loop_sec > 0.001 { (sample_time / loop_sec).clamp(0.0, 0.999) } else { 0.0 };
                                                let peak_idx = ((norm_pos * num_peaks as f32).floor() as usize).min(num_peaks - 1);
                                                let amp = region.waveform_peaks[peak_idx];

                                                let h = (amp * (r_rect.height() * 0.42) * region.volume.clamp(0.2, 1.8)).max(0.5);
                                                ui.painter().line_segment([Pos2::new(cur_x, mid_y - h), Pos2::new(cur_x, mid_y + h)], Stroke::new(2.2_f32, wave_col));

                                                cur_x += step_px;
                                            }

                                            // Draw loop repeat dividers (exact positions where loop cycles restart)
                                            if is_looped && loop_sec > 0.01 {
                                                let first_cycle_rem_sec = (loop_sec - (region.sample_offset_sec % loop_sec)) % loop_sec;
                                                let mut div_sec = if first_cycle_rem_sec > 0.02 { first_cycle_rem_sec } else { loop_sec };
                                                let reg_start_sec = tempo.secs_at_bar(region.start_bar as f64) as f32;
                                                let total_reg_sec =
                                                    tempo.secs_for_bars_at(region.start_bar as f64, region.length_bars as f64) as f32;
                                                while div_sec < total_reg_sec - 0.05 {
                                                    // Strecket ligger på en *tid* i regionen; x räknas
                                                    // genom kartan, samma väg som allt annat här.
                                                    let div_bar = tempo.bar_at_secs((reg_start_sec + div_sec) as f64) as f32
                                                        - region.start_bar as f32;
                                                    let div_x = rx_start + div_bar * bar_w;
                                                    if div_x > r_rect.min.x + 4.0 && div_x < r_rect.max.x - 4.0 {
                                                        ui.painter().line_segment(
                                                            [Pos2::new(div_x, r_rect.min.y), Pos2::new(div_x, r_rect.max.y)],
                                                            Stroke::new(1.0_f32, Theme::FL_CYAN),
                                                        );
                                                        ui.painter().text(
                                                            Pos2::new(div_x + 3.0, r_rect.min.y + 7.0),
                                                            egui::Align2::LEFT_CENTER,
                                                            "🔁",
                                                            egui::FontId::proportional(8.0),
                                                            Theme::FL_CYAN,
                                                        );
                                                    }
                                                    div_sec += loop_sec;
                                                }
                                            }
                                        }

                                        // Visual Edge Drag Handles for ALL regions (extra prominent when hovered/selected)
                                        let handle_w = 10.0_f32;
                                        let is_lane_h = lane_resp.hover_pos().map(|p| r_rect.contains(p)).unwrap_or(false)
                                            || ui.ctx().pointer_latest_pos().map(|p| r_rect.contains(p)).unwrap_or(false);

                                        // Left trim handle (Trim Start / Korta start från vänster kant)
                                        let left_h_rect = Rect::from_min_size(Pos2::new(r_rect.min.x, r_rect.min.y), Vec2::new(handle_w, r_rect.height()));
                                        let left_h_col = if is_region_selected {
                                            Theme::FL_CYAN
                                        } else if is_lane_h {
                                            Color32::from_rgb(180, 230, 255)
                                        } else {
                                            Color32::from_rgba_unmultiplied(255, 255, 255, 120)
                                        };
                                        ui.painter().rect_filled(left_h_rect, Rounding { nw: 3.0, ne: 0.0, sw: 3.0, se: 0.0 }, left_h_col);
                                        ui.painter().line_segment([Pos2::new(left_h_rect.min.x + 4.0, left_h_rect.min.y + 6.0), Pos2::new(left_h_rect.min.x + 4.0, left_h_rect.max.y - 6.0)], Stroke::new(1.2_f32, Color32::from_rgb(30, 40, 55)));

                                        // Right trim/loop handle (Trim End / Dra för att förlänga, korta eller loopa från höger kant)
                                        let right_h_rect = Rect::from_min_max(Pos2::new(r_rect.max.x - handle_w, r_rect.min.y), r_rect.max);
                                        let right_h_col = if is_region_selected {
                                            Theme::FL_ORANGE
                                        } else if is_lane_h {
                                            Color32::from_rgb(255, 200, 140)
                                        } else {
                                            Color32::from_rgba_unmultiplied(255, 255, 255, 120)
                                        };
                                        ui.painter().rect_filled(right_h_rect, Rounding { nw: 0.0, ne: 3.0, sw: 0.0, se: 3.0 }, right_h_col);
                                        ui.painter().line_segment([Pos2::new(right_h_rect.min.x + 4.0, right_h_rect.min.y + 6.0), Pos2::new(right_h_rect.min.x + 4.0, right_h_rect.max.y - 6.0)], Stroke::new(1.2_f32, Color32::from_rgb(55, 40, 30)));

                                        // Hover Cursor Feedback for Select and Draw/Paint tools (FL Studio style)
                                        if matches!(self.arranger_tool, ArrangerTool::Select | ArrangerTool::Paint) {
                                            let pointer_opt = lane_resp.hover_pos().or_else(|| ui.ctx().pointer_latest_pos());
                                            if let Some(mouse_pos) = pointer_opt {
                                                if r_rect.contains(mouse_pos) {
                                                    if (mouse_pos.x - rx_start).abs() <= 16.0 || mouse_pos.x <= rx_start + 16.0 {
                                                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                                                    } else if (mouse_pos.x - rx_end).abs() <= 16.0 || mouse_pos.x >= rx_end - 16.0 {
                                                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                                                    } else {
                                                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grab);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // Start Drag on Region (Edge Trim Start vs Edge Trim End / Loop vs Move)
                                let is_pointer_primary_down = ui.input(|i| i.pointer.primary_down());
                                let is_lane_dragging = lane_resp.drag_started()
                                    || lane_resp.dragged()
                                    || (lane_resp.hovered() && is_pointer_primary_down)
                                    || (is_pointer_primary_down && ui.input(|i| i.pointer.is_decidedly_dragging()));

                                if matches!(self.arranger_tool, ArrangerTool::Select | ArrangerTool::Paint) && is_lane_dragging && self.region_drag_state.is_none() {
                                    let mouse_pos_opt = ui.ctx().pointer_interact_pos()
                                        .or_else(|| ui.input(|i| i.pointer.press_origin()))
                                        .or_else(|| ui.ctx().pointer_latest_pos())
                                        .or_else(|| lane_resp.hover_pos());
                                    if let Some(mouse_pos) = mouse_pos_opt
                                        && let Some(r_i) = region_under_x(
                                            &regions_snapshot,
                                            mouse_pos.x,
                                            lane_rect.min.x,
                                            bar_w,
                                        )
                                    {
                                        let r = &regions_snapshot[r_i];
                                        let rx_start = lane_rect.min.x + r.start_bar * bar_w;
                                        let rx_end = rx_start + r.length_bars * bar_w;
                                        let init_loop_bars = if r.loop_length_bars > 0.001 {
                                            r.loop_length_bars
                                        } else {
                                            r.length_bars
                                        };
                                        let mode = if mouse_pos.x <= rx_start + 16.0
                                            || (mouse_pos.x - rx_start).abs() <= 10.0
                                        {
                                            RegionDragMode::TrimStart
                                        } else if mouse_pos.x >= rx_end - 16.0
                                            || (mouse_pos.x - rx_end).abs() <= 10.0
                                        {
                                            RegionDragMode::TrimEnd
                                        } else {
                                            RegionDragMode::Move
                                        };
                                        self.push_undo(match mode {
                                            RegionDragMode::TrimStart => crate::i18n::t("Trimma start (Vänster)"),
                                            RegionDragMode::TrimEnd => crate::i18n::t("Justera längd / loop (Höger)"),
                                            RegionDragMode::Move => crate::i18n::t("Flytta region"),
                                        });
                                        self.region_drag_state = Some(RegionDragState {
                                            track_idx: t_idx,
                                            region_idx: r_i,
                                            mode,
                                            initial_start_bar: r.start_bar,
                                            initial_length_bars: r.length_bars,
                                            initial_sample_offset_sec: r.sample_offset_sec,
                                            initial_loop_length_bars: init_loop_bars,
                                            drag_start_mouse_x: mouse_pos.x,
                                        });
                                        self.selected_audio_region = Some((t_idx, r_i));
                                        self.selected_timeline_track = t_idx;
                                    }
                                }

                                // Process active dragging
                                if let Some(ref drag) = self.region_drag_state {
                                    if drag.track_idx == t_idx {
                                        let pointer_pos_opt = ui.ctx().pointer_latest_pos()
                                            .or_else(|| ui.ctx().pointer_interact_pos())
                                            .or_else(|| lane_resp.hover_pos());
                                        if let Some(mouse_pos) = pointer_pos_opt {
                                            let is_alt_held = ui.input(|i| i.modifiers.alt);
                                            let raw_delta_bars = (mouse_pos.x - drag.drag_start_mouse_x) / bar_w;
                                            let grid_step_bars = match self.timeline_snap_mode {
                                                TimeSnapMode::FreeHundredth => (0.01 / sec_per_bar_at(drag.initial_start_bar)).max(0.001),
                                                TimeSnapMode::Snap16th => 1.0 / 16.0,
                                                TimeSnapMode::SnapBeat => 0.25,
                                                TimeSnapMode::SnapBar => 1.0,
                                            };

                                            if drag.region_idx < self.playlist_tracks[t_idx].regions.len() {
                                                let reg = &mut self.playlist_tracks[t_idx].regions[drag.region_idx];
                                                match drag.mode {
                                                    RegionDragMode::Move => {
                                                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::Grabbing);
                                                        let raw_start = drag.initial_start_bar + raw_delta_bars;
                                                        let new_start = if is_alt_held {
                                                            raw_start.max(0.0)
                                                        } else {
                                                            ((raw_start / grid_step_bars).round() * grid_step_bars).max(0.0)
                                                        };
                                                        reg.start_bar = new_start;
                                                        self.status_message = crate::tstatus!("↔ Flyttar '{}' till takt {:.2} (⏱ {})", reg.name, new_start + 1.0, format_time_hundredths(secs_at(new_start)));
                                                    }
                                                    RegionDragMode::TrimStart => {
                                                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                                                        let right_edge = drag.initial_start_bar + drag.initial_length_bars;
                                                        let max_expand_sec = drag.initial_sample_offset_sec;
                                                        let orig_sample_start = (drag.initial_start_bar - (max_expand_sec / sec_per_bar_at(drag.initial_start_bar))).max(0.0);
                                                        let raw_start = drag.initial_start_bar + raw_delta_bars;

                                                        let mut new_start = if is_alt_held {
                                                            raw_start
                                                        } else {
                                                            // Magnetic snap to sample start (0 offset)
                                                            let snap_px_bars = (16.0 / bar_w).max(0.04);
                                                            if (raw_start - orig_sample_start).abs() <= snap_px_bars {
                                                                orig_sample_start
                                                            } else {
                                                                (raw_start / grid_step_bars).round() * grid_step_bars
                                                            }
                                                        };
                                                        new_start = new_start.clamp(orig_sample_start, right_edge - 0.05);
                                                        let new_len = right_edge - new_start;
                                                        let new_offset = (drag.initial_sample_offset_sec + secs_len(drag.initial_start_bar, new_start - drag.initial_start_bar)).max(0.0);
                                                        reg.start_bar = new_start;
                                                        reg.length_bars = new_len;
                                                        reg.sample_offset_sec = new_offset;
                                                        self.status_message = crate::tstatus!("◀ Trimmar start på '{}': Start takt {:.2} | Bortklippt start: +{:.2}s | Längd: {:.2} takter", reg.name, new_start + 1.0, new_offset, new_len);
                                                    }
                                                    RegionDragMode::TrimEnd => {
                                                        ui.output_mut(|o| o.cursor_icon = egui::CursorIcon::ResizeHorizontal);
                                                        let base_loop_len = if drag.initial_loop_length_bars > 0.001 { drag.initial_loop_length_bars } else { drag.initial_length_bars };
                                                        let raw_len = (drag.initial_length_bars + raw_delta_bars).max(0.05);

                                                        let mut new_len = if is_alt_held {
                                                            raw_len
                                                        } else {
                                                            // 1. Magnetic snap to exact full loop cycles (1x, 2x, 3x, 4x, etc.)
                                                            let nearest_rep = (raw_len / base_loop_len).round().max(1.0);
                                                            let rep_snap_len = nearest_rep * base_loop_len;
                                                            let snap_px_bars = (18.0 / bar_w).max(0.05);
                                                            if (raw_len - rep_snap_len).abs() <= snap_px_bars {
                                                                rep_snap_len
                                                            } else {
                                                                // 2. Snap to active grid (1/16, beat, bar)
                                                                ((raw_len / grid_step_bars).round() * grid_step_bars).max(grid_step_bars.min(0.05))
                                                            }
                                                        };
                                                        new_len = new_len.max(0.05);
                                                        reg.loop_length_bars = base_loop_len;
                                                        reg.length_bars = new_len;

                                                        let reps = new_len / base_loop_len;
                                                        let is_exact_rep = (reps - reps.round()).abs() < 0.001;
                                                        if new_len > base_loop_len + 0.05 {
                                                            if is_exact_rep {
                                                                self.status_message = crate::tstatus!("🧲 Loop-snap: '{}' loopad exakt {:.0}x ({} takter, ⏱ {})", reg.name, reps.round(), new_len, format_time_hundredths(secs_len(reg.start_bar, new_len)));
                                                            } else {
                                                                self.status_message = crate::tstatus!("▶ Loopar '{}': {:.2} takter ({:.1}x repetitioner, ⏱ {})", reg.name, new_len, reps, format_time_hundredths(secs_len(reg.start_bar, new_len)));
                                                            }
                                                        } else {
                                                            self.status_message = crate::tstatus!("▶ Längd för '{}': {:.2} takter (⏱ {})", reg.name, new_len, format_time_hundredths(secs_len(reg.start_bar, new_len)));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                let is_drag_released = lane_resp.drag_stopped() || ui.input(|i| i.pointer.any_released() || i.pointer.primary_released() || !i.pointer.primary_down());
                                if is_drag_released && self.region_drag_state.is_some() {
                                    if let Some(ref drag) = self.region_drag_state {
                                        let t_idx_synced = drag.track_idx;
                                        self.sync_track_regions(t_idx_synced);
                                        self.status_message = crate::i18n::t("✅ Ändring sparad på tidslinjen!").to_string();
                                    }
                                    self.region_drag_state = None;
                                }

                                // Tool Click Interactions
                                if lane_resp.clicked() && self.region_drag_state.is_none() {
                                    ui.memory_mut(|m| m.stop_text_input());
                                    self.selected_timeline_track = t_idx;
                                    if let Some(mouse_pos) = lane_resp.hover_pos() {
                                        let click_bar = (mouse_pos.x - lane_rect.min.x) / bar_w;
                                        for (r_i, r) in regions_snapshot.iter().enumerate() {
                                            if click_bar >= r.start_bar && click_bar <= (r.start_bar + r.length_bars) {
                                                match self.arranger_tool {
                                                    ArrangerTool::Select => {
                                                        select_action = Some(r_i);
                                                    }
                                                    ArrangerTool::Slice => {
                                                        split_action = Some((r_i, click_bar));
                                                    }
                                                    ArrangerTool::Erase => {
                                                        delete_action = Some(r_i);
                                                    }
                                                    ArrangerTool::Mute => {
                                                        mute_action = Some(r_i);
                                                    }
                                                    _ => {
                                                        select_action = Some(r_i);
                                                    }
                                                }
                                                break;
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Fallback pattern clips
                                for bar_idx in 0..32 {
                                    if let Some(pat_idx) = self.playlist_tracks[t_idx].clips[bar_idx] {
                                        let c_min_x = lane_rect.min.x + bar_idx as f32 * bar_w;
                                        let c_rect = Rect::from_min_size(Pos2::new(c_min_x + 1.0, lane_rect.min.y + 2.0), Vec2::new(bar_w - 2.0, row_h - 4.0));
                                        ui.painter().rect_filled(c_rect, Rounding::same(2.0), track_color);
                                        ui.painter().text(c_rect.center(), egui::Align2::CENTER_CENTER, format!("P{}", pat_idx + 1), egui::FontId::proportional(10.0), Color32::BLACK);
                                    }
                                }

                                if (lane_resp.clicked() || lane_resp.secondary_clicked())
                                    && let Some(mouse_pos) = lane_resp.hover_pos() {
                                        let clicked_bar = ((mouse_pos.x - lane_rect.min.x) / bar_w) as usize;
                                        if clicked_bar < 32 {
                                            if lane_resp.secondary_clicked() || self.arranger_tool == ArrangerTool::Erase {
                                                clip_action = Some((clicked_bar, None));
                                            } else if self.arranger_tool == ArrangerTool::Paint {
                                                clip_action = Some((clicked_bar, Some(self.selected_pattern)));
                                            }
                                        }
                                    }
                            }
                        }

                        // **Högerklick väljer klippet under pekaren** (Fas 8.15).
                        //
                        // Menyn nedan gäller det *valda* klippet, och utan det här
                        // kunde "Radera region" träffa ett annat klipp än det man
                        // pekade på: valet var det man senast vänsterklickade. Samma
                        // regel som dragstarten (`region_under_x`), så de två dörrarna
                        // inte kan välja olika klipp.
                        //
                        // Klickar man utanför ett klipp lämnas valet orört — rubriken
                        // i menyn (`🎵 namn`) visar vilket klipp posterna gäller.
                        if lane_resp.secondary_clicked()
                            && let Some(mouse_pos) = lane_resp.hover_pos()
                        {
                            // Samma lista som menyn nedan läser — en källa.
                            let hit = region_under_x(
                                &self.playlist_tracks[t_idx].regions,
                                mouse_pos.x,
                                lane_rect.min.x,
                                bar_w,
                            );
                            if let Some(r_i) = hit {
                                self.selected_audio_region = Some((t_idx, r_i));
                                self.selected_timeline_track = t_idx;
                            }
                        }

                        // Context Menu for Region & Track Lane
                        lane_resp.context_menu(|ui| {
                            ui.set_min_width(240.0);
                            if let Some((sel_t, sel_r)) = self.selected_audio_region
                                && sel_t == t_idx
                                && sel_r < self.playlist_tracks[t_idx].regions.len() {
                                    let r_name = self.playlist_tracks[t_idx].regions[sel_r].name.clone();
                                    let is_muted = self.playlist_tracks[t_idx].regions[sel_r].muted;
                                    let is_rev = self.playlist_tracks[t_idx].regions[sel_r].is_reverse;

                                    ui.label(egui::RichText::new(format!("🎵 {}", r_name)).strong().color(Theme::FL_CYAN));
                                    ui.separator();

                                    if ui.button(crate::i18n::t("📋 Kopiera sample (Ctrl+C)")).clicked() {
                                        self.copy_selected_region();
                                        ui.close_menu();
                                    }
                                    if self.copied_region.is_some() {
                                        if ui.button(crate::i18n::t("📋 Klistra in sample vid spelhuvud (Ctrl+V)")).clicked() {
                                            self.paste_region();
                                            ui.close_menu();
                                        }
                                    }
                                    if ui.button(crate::i18n::t("📋 Duplicera region (Ctrl+D)")).clicked() {
                                        self.duplicate_selected_region();
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button(crate::i18n::t("🔁 Loopa/Repetera loop (x2 - Ctrl+L)")).clicked() {
                                        self.repeat_selected_region_loop(2.0);
                                        ui.close_menu();
                                    }
                                    if ui.button(crate::i18n::t("🔁 Loopa/Repetera loop (x4)")).clicked() {
                                        self.repeat_selected_region_loop(4.0);
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button(self.tr("✂ Klipp vid spelhuvud (Ctrl+B / S)")).clicked() {
                                        self.split_selected_region_at_playhead();
                                        ui.close_menu();
                                    }
                                    if ui
                                        .button(crate::i18n::t(
                                            "🔍 Hitta första slaget (mät i filen)",
                                        ))
                                        .on_hover_text(crate::i18n::t(
                                            "Mäter var ljudet börjar i klippets fil och sätter det som klippets början — samma regel som \"Sätt takt 1 här\", men talet kommer ur en mätning i stället för ur spelhuvudets position. **Hela stämgruppen följer med**: alla klipp som börjar på samma takt flyttas lika mycket i tid, så att stämmorna inte hamnar ur fas (Ctrl+Z tar tillbaka allt i ett steg). Ett tyst parti i början (eller en stämma som inte hörs inom 20 s) ger inget svar; då händer ingenting.",
                                        ))
                                        .clicked()
                                    {
                                        self.align_clip_to_first_beat();
                                        ui.close_menu();
                                    }
                                    if ui
                                        .button(crate::i18n::t(
                                            "🎯 Sätt takt 1 här (ljudet under spelhuvudet blir klippets början)",
                                        ))
                                        .on_hover_text(crate::i18n::t(
                                            "Kapar början av klippet så att slaget under spelhuvudet hamnar på rutnätet. Högerkanten står still, och **hela stämgruppen följer med** — alla klipp som börjar på samma takt flyttas lika mycket i tid, annars hamnar stämman ur fas med sina syskon (Ctrl+Z tar tillbaka allt i ett steg). Klippet flyttas inte.",
                                        ))
                                        .clicked()
                                    {
                                        self.set_beat_one_at_playhead();
                                        ui.close_menu();
                                    }
                                    if ui.button(crate::i18n::t("💾 Spara som sample i Sound Browser")).clicked() {
                                        self.save_region_to_sound_browser(sel_t, sel_r);
                                        ui.close_menu();
                                    }
                                    if ui.button(if is_rev { crate::i18n::t("🔄 Återställ riktning (Normal)") } else { crate::i18n::t("🔄 Vänd baklänges (Reverse - Ctrl+K)") }).clicked() {
                                        self.reverse_selected_region();
                                        ui.close_menu();
                                    }
                                    if ui.button(if is_muted { crate::i18n::t("🔊 Avmuta region (M)") } else { crate::i18n::t("🔇 Muta region (M)") }).clicked() {
                                        self.toggle_mute_selected_region();
                                        ui.close_menu();
                                    }
                                    if ui.button(crate::i18n::t("🎙 Öppna i Sångstudion (Isolerad provspelning & formning)")).clicked() {
                                        self.open_region_in_vocal_studio(sel_t, sel_r);
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    if ui.button(crate::i18n::t("📋 Duplicera hela detta spår")).clicked() {
                                        track_duplicate_idx = Some(t_idx);
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                    ui.menu_button(crate::i18n::t("🎨 Ändra färg"), |ui| {
                                        let colors = [
                                            ("Cyan", Theme::FL_CYAN),
                                            ("Orange", Theme::FL_ORANGE),
                                            ("Grön", Theme::FL_GREEN),
                                            ("Lila", Theme::FL_PURPLE),
                                            ("Blå", Color32::from_rgb(80, 150, 255)),
                                            ("Guld", Color32::from_rgb(255, 200, 50)),
                                        ];
                                        for (c_name, c_val) in colors {
                                            if ui.button(crate::i18n::t(c_name)).clicked() {
                                                self.playlist_tracks[t_idx].regions[sel_r].color = c_val;
                                                ui.close_menu();
                                            }
                                        }
                                    });
                                    ui.separator();
                                    if ui.button(egui::RichText::new(crate::i18n::t("🗑 Radera region (Delete)")).color(Color32::from_rgb(255, 90, 90))).clicked() {
                                        self.delete_selected_region();
                                        ui.close_menu();
                                    }
                            } else {
                                ui.label(egui::RichText::new(crate::tstatus!("Spår {}: {}", t_idx + 1, self.playlist_tracks[t_idx].name)).strong());
                                ui.separator();
                                if self.copied_region.is_some() {
                                    if ui.button(crate::i18n::t("📋 Klistra in sample vid spelhuvud (Ctrl+V)")).clicked() {
                                        track_paste_idx = Some(t_idx);
                                        ui.close_menu();
                                    }
                                    ui.separator();
                                }
                                if ui.button(crate::i18n::t("📋 Duplicera spår (under detta)")).clicked() {
                                    track_duplicate_idx = Some(t_idx);
                                    ui.close_menu();
                                }
                                if ui.button(crate::i18n::t("➕ Lägg till nytt spår...")).clicked() {
                                    self.show_add_track_modal = true;
                                    ui.close_menu();
                                }
                                if ui.button(crate::i18n::t("🔴 Spela in på detta spår")).clicked() {
                                    for t in &mut self.playlist_tracks { t.is_rec_armed = false; }
                                    self.playlist_tracks[t_idx].is_rec_armed = true;
                                    self.toggle_timeline_recording();
                                    ui.close_menu();
                                }
                            }
                        });

                        // Render live in-track recording region if this track is actively recording!
                        if self.is_recording_timeline && self.playlist_tracks[t_idx].is_rec_armed {
                            let live_s_bar = self.timeline_rec_start_bar;
                            let live_e_bar = bars_at(self.song_time).max(live_s_bar + 0.05);
                            let rx_start = lane_rect.min.x + live_s_bar * bar_w;
                            let rx_end = lane_rect.min.x + live_e_bar * bar_w;
                            let live_rect = Rect::from_min_max(Pos2::new(rx_start + 1.0, lane_rect.min.y + 2.0), Pos2::new(rx_end - 1.0, lane_rect.max.y - 2.0));

                            ui.painter().rect_filled(live_rect, Rounding::same(3.0), Color32::from_rgba_unmultiplied(220, 30, 30, 85));
                            ui.painter().rect_stroke(live_rect, Rounding::same(3.0), Stroke::new(1.5_f32, Color32::from_rgb(255, 60, 60)));

                            let live_peaks = &self.vocal_studio.live_recording_peaks;
                            if !live_peaks.is_empty() && live_rect.width() > 6.0 {
                                let step_x = (live_rect.width() - 4.0) / live_peaks.len().max(1) as f32;
                                let mid_y = live_rect.center().y;
                                for (pi, &p) in live_peaks.iter().enumerate() {
                                    let sx = live_rect.min.x + 2.0 + pi as f32 * step_x;
                                    let h = p * (live_rect.height() * 0.42);
                                    ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.3_f32, Color32::from_rgb(255, 190, 190)));
                                }
                            }

                            ui.painter().text(
                                Pos2::new(live_rect.min.x + 6.0, live_rect.min.y + 8.0),
                                egui::Align2::LEFT_CENTER,
                                format!("🔴 SPELAR IN... [⏱ {}]", format_time_hundredths(secs_len(live_s_bar, live_e_bar - live_s_bar))),
                                egui::FontId::proportional(9.5),
                                Color32::WHITE,
                            );
                        }

                        if let Some((c_bar, maybe_pat)) = clip_action {
                            self.playlist_tracks[t_idx].clips[c_bar] = maybe_pat;
                            if maybe_pat.is_some() {
                                self.status_message = crate::tstatus!("Placerade mönster {} vid takt {}", self.selected_pattern + 1, c_bar + 1);
                            } else {
                                self.status_message = crate::tstatus!("Raderade mönsterblock vid takt {}", c_bar + 1);
                            }
                        }

                        if let Some(s_idx) = select_action {
                            self.selected_audio_region = Some((t_idx, s_idx));
                            self.selected_timeline_track = t_idx;
                            if s_idx < self.playlist_tracks[t_idx].regions.len() {
                                let reg = &self.playlist_tracks[t_idx].regions[s_idx];
                                self.status_message = crate::tstatus!("Markerade '{}' [Start: {} | Längd: {}]", reg.name, format_time_hundredths(tempo.secs_at_bar(reg.start_bar as f64) as f32), format_time_hundredths(tempo.secs_for_bars_at(reg.start_bar as f64, reg.length_bars as f64) as f32));
                            }
                        }

                        // Execute Slice with Hundredth-Second Accuracy
                        if let Some((r_idx, click_bar)) = split_action {
                            let mut performed_split = false;
                            let mut cut_feedback_msg = String::new();

                            if r_idx < self.playlist_tracks[t_idx].regions.len() {
                                let orig = self.playlist_tracks[t_idx].regions[r_idx].clone();
                                let raw_cut_sec = secs_at(click_bar);

                                // **Snäppet sker i takter** (Fas 8.2), samma regel som
                                // i tidlinjen: klippet delas på en *plats i takten*, och
                                // sekunden hämtas ur kartan efteråt.
                                let cut_bar = if self.timeline_snap_mode == TimeSnapMode::FreeHundredth {
                                    let sec = (raw_cut_sec * 100.0).round() / 100.0;
                                    bars_at(sec)
                                } else {
                                    snap_bar(self.timeline_snap_mode, click_bar)
                                };
                                let cut_sec = secs_at(cut_bar);

                                let orig_start_sec = secs_at(orig.start_bar);
                                let orig_len_sec = secs_len(orig.start_bar, orig.length_bars);
                                let split_offset_sec = cut_sec - orig_start_sec;

                                if split_offset_sec > 0.05 && split_offset_sec < orig_len_sec - 0.05 {
                                    let split_offset_bar = bars_len(orig.start_bar, split_offset_sec);
                                    let split_points = ((split_offset_sec / orig_len_sec) * orig.waveform_peaks.len() as f32) as usize;

                                    let left_peaks = orig.waveform_peaks[..split_points.min(orig.waveform_peaks.len())].to_vec();
                                    let right_peaks = orig.waveform_peaks[split_points.min(orig.waveform_peaks.len())..].to_vec();

                                    let clean_title = orig.name
                                        .replace(" [Del 1]", "")
                                        .replace(" [Del 2]", "")
                                        .replace(" (Kopia)", "")
                                        .trim()
                                        .to_string();

                                    let new_color = match orig.color {
                                        c if c == Theme::FL_CYAN => Theme::FL_ORANGE,
                                        c if c == Theme::FL_ORANGE => Theme::FL_GREEN,
                                        c if c == Theme::FL_GREEN => Theme::FL_PURPLE,
                                        c if c == Theme::FL_PURPLE => Color32::from_rgb(80, 180, 255),
                                        _ => Theme::FL_CYAN,
                                    };

                                    let r_left = AudioRegion {
                                        source_bpm: orig.source_bpm, // halvan ärver klippets källa
                                        tape: false,
                                        id: orig.id,
                                        name: format!("{} [Del 1]", clean_title),
                                        start_bar: orig.start_bar,
                                        length_bars: split_offset_bar,
                                        sample_offset_sec: orig.sample_offset_sec,
                                        source_path: orig.source_path.clone(),
                                        waveform_peaks: left_peaks,
                                        volume: orig.volume,
                                        fade_in_bars: orig.fade_in_bars.min(split_offset_bar * 0.5),
                                        fade_out_bars: 0.0,
                                        muted: orig.muted,
                                        is_reverse: orig.is_reverse,
                                        color: orig.color,
                                        loop_length_bars: 0.0,
                                    };

                                    let r_right = AudioRegion {
                                        source_bpm: orig.source_bpm, // halvan ärver klippets källa
                                        tape: false,
                                        id: orig.id + 1000 + (self.playlist_tracks[t_idx].regions.len() * 10),
                                        name: format!("{} [Del 2]", clean_title),
                                        start_bar: orig.start_bar + split_offset_bar,
                                        length_bars: ((orig.length_bars - split_offset_bar) * 100.0).round() / 100.0,
                                        sample_offset_sec: ((orig.sample_offset_sec + split_offset_sec) * 100.0).round() / 100.0,
                                        source_path: orig.source_path,
                                        waveform_peaks: right_peaks,
                                        volume: orig.volume,
                                        fade_in_bars: 0.0,
                                        fade_out_bars: orig.fade_out_bars.min((orig.length_bars - split_offset_bar) * 0.5),
                                        muted: orig.muted,
                                        is_reverse: orig.is_reverse,
                                        color: new_color,
                                        loop_length_bars: 0.0,
                                    };

                                    self.push_undo(&format!("Klipp '{}' med saxverktyg", clean_title));
                                    self.playlist_tracks[t_idx].regions.remove(r_idx);
                                    self.playlist_tracks[t_idx].regions.insert(r_idx, r_right);
                                    self.playlist_tracks[t_idx].regions.insert(r_idx, r_left);
                                    performed_split = true;
                                    cut_feedback_msg = crate::tstatus!("✂ Klippte '{}' vid {} (Takt {})! [Ångra: Ctrl+Z]", clean_title, format_time_hundredths(cut_sec), format_bar_subdivisions(orig.start_bar + split_offset_bar));
                                }
                            }
                            if performed_split {
                                self.sync_track_regions(t_idx);
                                self.selected_audio_region = Some((t_idx, r_idx));
                                self.status_message = cut_feedback_msg;
                            }
                        }

                        if let Some(d_idx) = delete_action
                            && d_idx < self.playlist_tracks[t_idx].regions.len() {
                                let name = self.playlist_tracks[t_idx].regions[d_idx].name.clone();
                                self.push_undo(&format!("Radera '{}'", name));
                                self.playlist_tracks[t_idx].regions.remove(d_idx);
                                self.sync_track_regions(t_idx);
                                self.selected_audio_region = None;
                                self.status_message = crate::tstatus!("🗑 Raderade region '{}'. [Ångra: Ctrl+Z]", name);
                            }

                        if let Some(m_idx) = mute_action
                            && m_idx < self.playlist_tracks[t_idx].regions.len() {
                                self.push_undo("Muta/Avmuta region");
                                self.playlist_tracks[t_idx].regions[m_idx].muted = !self.playlist_tracks[t_idx].regions[m_idx].muted;
                                self.sync_track_regions(t_idx);
                                let name = self.playlist_tracks[t_idx].regions[m_idx].name.clone();
                                self.status_message = crate::tstatus!("🔇 Toggla mute för '{}'", name);
                            }

                        ui.add_space(2.0);
                    }

                    // "+ Add Track" Blank Lane Placeholder on Right
                    let (add_lane_rect, add_lane_resp) = ui.allocate_exact_size(Vec2::new(timeline_total_w, 28.0), Sense::click());
                    ui.painter().rect_filled(add_lane_rect, Rounding::same(4.0), Color32::from_rgb(15, 17, 22));
                    if add_lane_resp.clicked() {
                        self.show_add_track_modal = true;
                    }

                    if let Some(ref drag_item) = self.sample_drag_item {
                        let pointer_opt = ui.ctx().pointer_latest_pos()
                            .or_else(|| ui.ctx().pointer_interact_pos())
                            .or_else(|| ui.input(|i| i.pointer.hover_pos()))
                            .or_else(|| add_lane_resp.hover_pos());
                        if let Some(mouse_pos) = pointer_opt {
                            if add_lane_rect.contains(mouse_pos) {
                                let raw_drop_bar = ((mouse_pos.x - add_lane_rect.min.x) / bar_w).max(0.0);
                                let raw_drop_sec = secs_at(raw_drop_bar);
                                let drop_sec = match self.timeline_snap_mode {
                                    TimeSnapMode::FreeHundredth => (raw_drop_sec * 100.0).round() / 100.0,
                                    TimeSnapMode::Snap16th => {
                                        let step_sec = sec_per_bar_at(raw_drop_bar) / 16.0;
                                        (raw_drop_sec / step_sec).round() * step_sec
                                    }
                                    TimeSnapMode::SnapBeat => {
                                        let beat_sec = sec_per_bar_at(raw_drop_bar) / 4.0;
                                        (raw_drop_sec / beat_sec).round() * beat_sec
                                    }
                                    TimeSnapMode::SnapBar => (raw_drop_sec / sec_per_bar_at(raw_drop_bar)).round() * sec_per_bar_at(raw_drop_bar),
                                };
                                let drop_bar = bars_at(drop_sec).max(0.0);
                                let item_dur_sec: f32 = 2.0;
                                let item_dur_bars = (item_dur_sec / sec_per_bar_at(drop_bar)).max(0.25);
                                let ghost_x_start = add_lane_rect.min.x + drop_bar * bar_w;
                                let ghost_x_end = (ghost_x_start + item_dur_bars * bar_w).min(add_lane_rect.max.x);
                                let ghost_rect = Rect::from_min_max(
                                    Pos2::new(ghost_x_start, add_lane_rect.min.y + 2.0),
                                    Pos2::new(ghost_x_end, add_lane_rect.max.y - 2.0),
                                );
                                ui.painter().rect_filled(ghost_rect, Rounding::same(4.0), Color32::from_rgba_unmultiplied(0, 200, 255, 85));
                                ui.painter().rect_stroke(ghost_rect, Rounding::same(4.0), Stroke::new(1.8_f32, Theme::FL_CYAN));
                                ui.painter().text(
                                    Pos2::new(ghost_rect.min.x + 6.0, ghost_rect.center().y),
                                    egui::Align2::LEFT_CENTER,
                                    crate::tstatus!("➕ Nytt spår: Släpp {} (Takt {:.2})", drag_item.name, drop_bar + 1.0),
                                    egui::FontId::proportional(10.0),
                                    Color32::WHITE,
                                );
                                if ui.input(|i| i.pointer.any_released() || i.pointer.primary_released()) {
                                    drop_sample_action = Some((self.playlist_tracks.len(), drag_item.clone(), drop_bar));
                                }
                            }
                        }
                    }

                    ui.add_space(4.0);

                    // Master "Main" Track Lane (Suno Studio Style Master Strip)
                    let (m_lane_rect, _) = ui.allocate_exact_size(Vec2::new(timeline_total_w, row_h), Sense::hover());
                    ui.painter().rect_filled(m_lane_rect, Rounding::ZERO, Color32::from_rgb(14, 16, 22));
                    ui.painter().line_segment([Pos2::new(m_lane_rect.min.x, m_lane_rect.max.y), Pos2::new(m_lane_rect.max.x, m_lane_rect.max.y)], Stroke::new(0.5_f32, Color32::from_rgb(30, 36, 48)));

                    // Master peak meter visualization along the lane
                    let peak = self.engine.get_peak_level();
                    let meter_w = (peak * 350.0).min(timeline_total_w);
                    let meter_rect = Rect::from_min_size(Pos2::new(m_lane_rect.min.x, m_lane_rect.min.y + 12.0), Vec2::new(meter_w, row_h - 24.0));
                    ui.painter().rect_filled(meter_rect, Rounding::same(2.0), Color32::from_rgba_unmultiplied(255, 140, 0, 45));

                    ui.painter().text(
                        Pos2::new(m_lane_rect.min.x + 12.0, m_lane_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        crate::tstatus!("Stereo Master Utgång • Volym: {:.0}% • Peak: {:.2}", self.master_volume * 100.0, peak),
                        egui::FontId::proportional(10.0),
                        Color32::from_rgb(130, 140, 160),
                    );

                    // ====================================================
                    // AUTOMATION CURVE EDITOR (Fas 5.4)
                    // ====================================================
                    let mut auto_bottom = m_lane_rect.max.y;
                    if self.show_automation && !self.playlist_tracks.is_empty() {
                        let sel = self.selected_timeline_track.min(self.playlist_tracks.len() - 1);
                        let auto_h = 96.0;
                        let (auto_rect, auto_resp) = ui.allocate_exact_size(Vec2::new(timeline_total_w, auto_h), Sense::click_and_drag());
                        self.render_automation_lane(ui, auto_rect, &auto_resp, bar_w, sel);
                        auto_bottom = auto_rect.max.y;
                    }

                    // ====================================================
                    // 2.3 VERTICAL PLAYHEAD NEEDLE ACROSS ALL TRACKS
                    // ====================================================
                    let playhead_bar = bars_at(self.song_time);
                    let playhead_x = timeline_left_x + playhead_bar * bar_w;
                    let track_area_top = ruler_rect.min.y;
                    let track_area_bottom = auto_bottom;

                    if playhead_x >= timeline_left_x && playhead_x <= timeline_left_x + timeline_total_w {
                        // Auto-scroll timeline to follow playhead when enabled and playing
                        if self.timeline_auto_scroll && self.is_playing {
                            let follow_rect = Rect::from_min_max(
                                Pos2::new(playhead_x - 60.0, track_area_top),
                                Pos2::new(playhead_x + 60.0, track_area_top + 10.0),
                            );
                            ui.scroll_to_rect(follow_rect, Some(egui::Align::Center));
                        }

                        // Glowing line strictly bounded to track lanes
                        ui.painter().line_segment(
                            [Pos2::new(playhead_x, track_area_top), Pos2::new(playhead_x, track_area_bottom)],
                            Stroke::new(1.8_f32, Theme::FL_ORANGE),
                        );

                        // Triangle head badge on ruler
                        let tri = [
                            Pos2::new(playhead_x - 5.0, track_area_top),
                            Pos2::new(playhead_x + 5.0, track_area_top),
                            Pos2::new(playhead_x, track_area_top + 8.0),
                        ];
                        ui.painter().add(egui::Shape::convex_polygon(tri.to_vec(), Theme::FL_ORANGE, Stroke::NONE));
                    }
                });
            });
        deferred.scrub_target_sec = scrub_target_sec;
        deferred.drop_sample_action = drop_sample_action;
        deferred.track_delete_idx = track_delete_idx;
        deferred.track_duplicate_idx = track_duplicate_idx;
        deferred.track_paste_idx = track_paste_idx;
    }
}

/// Tidlinjens geometri — **namngiven**, inte positionsbunden.
///
/// `header_w` och `row_h` är båda `f32`, och det här repot har redan kostat på sig en
/// förväxling mellan två tal av samma typ. Med namngivna fält kan anroparen inte byta
/// plats på dem utan att skriva fel namn.
#[derive(Clone, Copy)]
pub(crate) struct TimelineGeometry {
    pub(crate) header_w: f32,
    pub(crate) bar_w: f32,
    pub(crate) row_h: f32,
    pub(crate) ruler_h: f32,
    pub(crate) total_bars: usize,
    pub(crate) timeline_total_w: f32,
}

/// Det användaren gjorde i tidlinjen, sparat till **efter** ritningen.
///
/// Att radera eller klistra in ett spår medan listan ritas är att mutera det man läser;
/// därför samlar ritningen ihop avsikterna och anroparen utför dem när allt är färdigritat.
#[derive(Default)]
pub(crate) struct DeferredActions {
    pub(crate) scrub_target_sec: Option<f32>,
    pub(crate) drop_sample_action: Option<(usize, LibrarySampleItem, f32)>,
    pub(crate) track_delete_idx: Option<usize>,
    pub(crate) track_duplicate_idx: Option<usize>,
    pub(crate) track_paste_idx: Option<usize>,
}

impl SonixApp {
    /// Den valda ljudregionens panel (2.5) — två våningar, radbryten, och regionens
    /// egna handlingar: dela, duplicera, backa, koppla ihop, ta bort.
    ///
    /// Kroppen är flyttad ordagrant; bara de fyra tidsfrågorna kommer in utifrån, som
    /// referenser — de definieras på **ett** ställe, i anroparen.
    pub(crate) fn draw_region_inspector(
        &mut self,
        ui: &mut egui::Ui,
        secs_at: &dyn Fn(f32) -> f32,
        secs_len: &dyn Fn(f32, f32) -> f32,
        bars_at: &dyn Fn(f32) -> f32,
        bars_len: &dyn Fn(f32, f32) -> f32,
    ) {
        // ================================================================
        // 2.5 SELECTED AUDIO REGION INSPECTOR (RESPONSIVE 2-TIER / WRAPPED)
        // ================================================================
        if let Some((t_idx, r_idx)) = self.selected_audio_region
            && t_idx < self.playlist_tracks.len() && r_idx < self.playlist_tracks[t_idx].regions.len() {
                let mut do_delete = false;
                let mut do_split = false;
                let mut do_copy = false;
                let mut do_paste = false;
                let mut do_loop_factor: Option<f32> = None;
                let mut do_duplicate = false;
                let mut do_reverse = false;
                let mut do_tape = false;
                let mut do_open_focus = false;
                let mut do_open_vocal_studio = false;
                let mut do_save_sample = false;
                let has_copied = self.copied_region.is_some();
                let copied_name = self.copied_region.as_ref().map(|c| c.name.clone()).unwrap_or_default();
                let r = &mut self.playlist_tracks[t_idx].regions[r_idx];
                let r_start_sec = secs_at(r.start_bar);
                let r_len_sec = secs_len(r.start_bar, r.length_bars);
                let r_name = r.name.clone();
                let is_rev = r.is_reverse;
                let is_tape = r.tape;

                ui.add_space(4.0);
                ui.group(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    ui.spacing_mut().slider_width = 75.0;

                    // Row 1: Title, Time offsets, Sliders (Responsive Wrapped)
                    ui.horizontal_wrapped(|ui| {
                        ui.label(egui::RichText::new(format!("🎵 {}", r_name)).strong().color(Theme::FL_CYAN));
                        ui.separator();

                        // Start time readout with coarse/fine nudging
                        ui.label(crate::i18n::t("⏱ Start:"));
                        ui.label(egui::RichText::new(format!("{:.2}t ({})", r.start_bar + 1.0, format_time_hundredths(r_start_sec))).monospace().strong().color(Theme::TEXT_BRIGHT));
                        if ui.button(crate::i18n::t("-1t")).on_hover_text(crate::i18n::t("Flytta 1 takt bakåt")).clicked() {
                            r.start_bar = (r.start_bar - 1.0).max(0.0);
                        }
                        if ui.button(crate::i18n::t("-0.1s")).on_hover_text(crate::i18n::t("Flytta 0.1s bakåt")).clicked() {
                            r.start_bar = bars_at((r_start_sec - 0.1).max(0.0));
                        }
                        if ui.button(crate::i18n::t("+0.1s")).on_hover_text(crate::i18n::t("Flytta 0.1s framåt")).clicked() {
                            r.start_bar = bars_at(r_start_sec + 0.1);
                        }
                        if ui.button(crate::i18n::t("+1t")).on_hover_text(crate::i18n::t("Flytta 1 takt framåt")).clicked() {
                            r.start_bar += 1.0;
                        }

                        ui.separator();

                        // Duration readout with coarse/fine nudging
                        ui.label(crate::i18n::t("📏 Längd:"));
                        ui.label(egui::RichText::new(format!("{:.2}t ({})", r.length_bars, format_time_hundredths(r_len_sec))).monospace().strong().color(Theme::TEXT_BRIGHT));
                        if ui.button(crate::i18n::t("-1t")).on_hover_text(crate::i18n::t("Korta 1 takt")).clicked() {
                            r.length_bars = (r.length_bars - 1.0).max(0.05);
                        }
                        if ui.button(crate::i18n::t("-0.1s")).on_hover_text(crate::i18n::t("Korta 0.1s")).clicked() {
                            r.length_bars = bars_len(r.start_bar, (r_len_sec - 0.1).max(0.05));
                        }
                        if ui.button(crate::i18n::t("+0.1s")).on_hover_text(crate::i18n::t("Förläng 0.1s")).clicked() {
                            r.length_bars = bars_len(r.start_bar, r_len_sec + 0.1);
                        }
                        if ui.button(crate::i18n::t("+1t")).on_hover_text(crate::i18n::t("Förläng 1 takt")).clicked() {
                            r.length_bars += 1.0;
                        }

                        ui.separator();

                        // Start-trim offset readout
                        ui.label(crate::i18n::t("✂ Start-trim:"));
                        ui.label(egui::RichText::new(format!("{:.2}s", r.sample_offset_sec)).monospace().strong().color(Theme::FL_CYAN));
                        if ui.button(crate::i18n::t("-0.1s")).on_hover_text(crate::i18n::t("Minska start-trim (visa mer av början)")).clicked() {
                            r.sample_offset_sec = (r.sample_offset_sec - 0.1).max(0.0);
                        }
                        if ui.button(crate::i18n::t("+0.1s")).on_hover_text(crate::i18n::t("Öka start-trim (klipp bort mer av början)")).clicked() {
                            r.sample_offset_sec += 0.1;
                        }

                        ui.separator();

                        // Gain Slider
                        ui.label(crate::i18n::t("🎚 Vol:"));
                        ui.add(egui::Slider::new(&mut r.volume, 0.0..=2.0).custom_formatter(|v, _| {
                            let db = if v <= 0.001 { -60.0 } else { 20.0 * v.log10() };
                            format!("{:.0}dB", db)
                        }));

                        ui.separator();

                        // Fade In Slider
                        ui.label(crate::i18n::t("📈 In:"));
                        ui.add(egui::Slider::new(&mut r.fade_in_bars, 0.0..=(r.length_bars * 0.5).max(0.05)).custom_formatter(|v, _| {
                            format!("{:.2}s", secs_at(v as f32))
                        }));

                        ui.separator();

                        // Fade Out Slider
                        ui.label(crate::i18n::t("📉 Ut:"));
                        ui.add(egui::Slider::new(&mut r.fade_out_bars, 0.0..=(r.length_bars * 0.5).max(0.05)).custom_formatter(|v, _| {
                            format!("{:.2}s", secs_at(v as f32))
                        }));
                    });

                    ui.separator();

                    // Row 2: Action Buttons (Always clearly visible & wrapped)
                    ui.horizontal_wrapped(|ui| {
                        let cur_song_time = self.song_time;
                        let can_cut = cur_song_time > r_start_sec + 0.02 && cur_song_time < r_start_sec + r_len_sec - 0.02;
                        let cut_label = if can_cut {
                            format!("✂ Dela vid spelhuvud ({})", format_time_hundredths(cur_song_time))
                        } else {
                            "✂ Dela vid spelhuvud (S / Ctrl+B)".to_string()
                        };
                        let cut_bg = if can_cut { Theme::FL_GREEN } else { Color32::from_rgb(45, 60, 50) };
                        let cut_fg = if can_cut { Color32::BLACK } else { Theme::TEXT_MUTED };

                        if ui.add(egui::Button::new(egui::RichText::new(cut_label).strong().size(10.5).color(cut_fg)).fill(cut_bg))
                            .on_hover_text(crate::i18n::t("Klipp/dela regionen på exakt denna tidpunkt"))
                            .clicked() {
                                do_split = true;
                            }

                        // Copy
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📋 Kopiera (Ctrl+C)")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(30, 60, 95)))
                            .on_hover_text(crate::i18n::t("Kopiera detta sample till urklipp"))
                            .clicked() {
                                do_copy = true;
                            }

                        // Paste (if clipboard has region)
                        if has_copied {
                            if ui.add(egui::Button::new(egui::RichText::new(format!("📋 Klistra in ({}) (Ctrl+V)", copied_name)).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(35, 85, 125)))
                                .on_hover_text(crate::i18n::t("Klistra in kopierat sample vid spelhuvudet på aktivt spår"))
                                .clicked() {
                                    do_paste = true;
                                }
                        }

                        // Duplicate
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📋 Duplicera (Ctrl+D)")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(35, 75, 115)))
                            .on_hover_text(crate::i18n::t("Duplicera samplen direkt efter den nuvarande"))
                            .clicked() {
                                do_duplicate = true;
                            }

                        // Loop x2 / x4
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁 Loop x2 (Ctrl+L)")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(20, 85, 100)))
                            .on_hover_text(crate::i18n::t("Repetera och fördubbla loop-längden"))
                            .clicked() {
                                do_loop_factor = Some(2.0);
                            }
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁 Loop x4")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(18, 75, 90)))
                            .on_hover_text(crate::i18n::t("Repetera samplen 4 gånger i följd"))
                            .clicked() {
                                do_loop_factor = Some(4.0);
                            }

                        // Reverse
                        let rev_label = if is_rev { crate::i18n::t("🔄 Normal (Framlänges)") } else { crate::i18n::t("🔄 Vänd baklänges (Reverse)") };
                        let rev_bg = if is_rev { Theme::FL_ORANGE } else { Color32::from_rgb(70, 45, 90) };
                        if ui.add(egui::Button::new(egui::RichText::new(rev_label).strong().size(10.5).color(Color32::WHITE)).fill(rev_bg))
                            .on_hover_text(crate::i18n::t("Spela upp ljudregionen baklänges i realtid (Ctrl+K / R)"))
                            .clicked() {
                                do_reverse = true;
                            }

                        // Klippets temoläge (Fas 8.10 steg 2): sträcks med bevarad
                        // tonhöjd (standard) eller bandspelaren (undantaget).
                        let tape_label = if is_tape {
                            crate::i18n::t("📼 Bandspelare")
                        } else {
                            crate::i18n::t("🎚 Sträcks (bevarad tonhöjd)")
                        };
                        let tape_bg = if is_tape { Theme::FL_ORANGE } else { Color32::from_rgb(30, 48, 44) };
                        if ui
                            .add(egui::Button::new(egui::RichText::new(tape_label).strong().size(10.5).color(Color32::WHITE)).fill(tape_bg))
                            .on_hover_text(crate::i18n::t(
                                "Standard: klippet sträcks med bevarad tonhöjd när tempot ändras (tonhöjden står still). Bandspelarläget låter tonhöjden följa med — det är en effekt, inte standarden.",
                            ))
                            .clicked()
                        {
                            do_tape = true;
                        }

                        // Mute toggle
                        let mute_bg = if r.muted { Theme::FL_ORANGE } else { Color32::from_rgb(32, 38, 48) };
                        if ui.add(egui::Button::new(egui::RichText::new(if r.muted { "🔇 Mutad" } else { "🔊 Aktiv" }).strong().size(10.5).color(Color32::WHITE)).fill(mute_bg)).clicked() {
                            r.muted = !r.muted;
                        }

                        // Save to Sound Browser
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("💾 Spara i Sound Browser")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(32, 95, 80)))
                            .on_hover_text(crate::i18n::t("Spara denna ljudregion som sample i Sound Browser & på disk"))
                            .clicked() {
                                do_save_sample = true;
                            }

                        // Open Stem Focus Editor
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔍 Öppna Stämeditor")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(140, 70, 20))).clicked() {
                            do_open_focus = true;
                        }

                        // Open in Vocal Studio
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 Öppna i Sångstudio")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(40, 90, 120)))
                            .on_hover_text(crate::i18n::t("Ladda in detta sample/region i Sångstudion för isolerad provspelning, pitch, time stretch & effekter"))
                            .clicked() {
                                do_open_vocal_studio = true;
                            }

                        // Delete
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🗑 Ta bort (Del)")).strong().size(10.5).color(Color32::WHITE)).fill(Color32::from_rgb(160, 40, 40))).clicked() {
                            do_delete = true;
                        }

                        // Close Inspector
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("❌ Stäng")).size(10.0))).clicked() {
                            self.selected_audio_region = None;
                        }
                    });
                });

                if do_open_focus {
                    self.open_stem_focus(t_idx);
                } else if do_open_vocal_studio {
                    self.open_region_in_vocal_studio(t_idx, r_idx);
                } else if do_copy {
                    self.copy_selected_region();
                } else if do_paste {
                    self.paste_region();
                } else if let Some(factor) = do_loop_factor {
                    self.repeat_selected_region_loop(factor);
                } else if do_save_sample {
                    self.save_region_to_sound_browser(t_idx, r_idx);
                } else if do_reverse {
                    self.reverse_selected_region();
                } else if do_tape {
                    self.toggle_region_tape(t_idx, r_idx);
                } else if do_delete {
                    self.delete_selected_region();
                } else if do_duplicate {
                    self.duplicate_selected_region();
                } else if do_split {
                    self.split_selected_region_at_playhead();
                } else {
                    self.sync_track_regions(t_idx);
                }
            }
    }
}
