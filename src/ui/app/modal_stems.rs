//! Stämdialoger: Suno-import, stämfokus och förlopp.
//!
//! Utdelad ur `modals.rs` 2026-11-02; filen var 4 486 rader och höll 34 dialoger.

use super::*;
use crate::ui::theme::Theme;

impl SonixApp {
pub(crate) fn render_suno_stem_import_modal(&mut self, ctx: &egui::Context) {
    if !self.show_suno_import_modal {
        return;
    }

    let mut close = false;
    let mut zip_to_import: Option<String> = None;
    let mut folder_to_import: Option<(String, String, f32)> = None;

    egui::Window::new(crate::i18n::t("📦 Import Stems / Multi-Track Importer"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(580.0);

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🎵")).size(Theme::FONT_XL));
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Multi-Track Stämmor & Ljudspår (Stems)")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().orange()));
                        ui.label(egui::RichText::new(crate::i18n::t("Importera nedladdade ZIP-paket eller mappar med stämmor (Suno, FL Studio, Ableton, Logic m.fl.). Sonix läser ut äkta 48kHz WAV-vågformer, detekterar tempo (BPM) och mappar spåren i tidslinjen.")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                    });
                });
            });

            ui.add_space(8.0);

            // Quick Rescan Bar
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("UPPTÄCKTA STEMPAKET:")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                if ui.button(crate::i18n::t("🔄 Skanna ~/Music & ~/Downloads")).clicked() {
                    self.scan_for_suno_stems();
                }
            });

            ui.add_space(4.0);

            // List of detected ZIPs
            ui.group(|ui| {
                if self.detected_suno_zips.is_empty() {
                    ui.label(egui::RichText::new(crate::i18n::t("Inga .zip-stempaket hittades i ~/Music eller ~/Downloads. Klicka på knappen ovan för att skanna, eller ange sökväg manuellt nedan.")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                } else {
                    egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                        for zip_p in &self.detected_suno_zips {
                            let (title, bpm) = Self::parse_suno_zip_info(zip_p);
                            let is_wav = zip_p.contains("(1).zip") || zip_p.to_lowercase().contains("wav");
                            let badge = if is_wav { "🔊 48kHz WAV Stems" } else { "🎧 MP3 Stems" };

                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(format!("🎵 {}", title)).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new(format!("Tempo: {:.1} BPM", bpm)).size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                                            ui.label(egui::RichText::new(badge).size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                                        });
                                    });

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⚡ Importera Alla Stämmor")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().green())).clicked() {
                                            zip_to_import = Some(zip_p.clone());
                                        }
                                    });
                                });
                            });
                            ui.add_space(2.0);
                        }
                    });
                }
            });

            ui.add_space(8.0);

            // Manual Path Input & File Browser
            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("VÄLJ ZIP-FIL ELLER MAPP")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📁 Välj ZIP-fil från datorn...")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().track_blue())).clicked() {
                        self.spawn_async_file_picker("Stempaket", &["zip"], crate::i18n::t("Välj ZIP Stempaket"));
                    }

                    ui.separator();
                    ui.label(crate::i18n::t("eller ange sökväg:"));
                    ui.text_edit_singleline(&mut self.custom_stem_path_input);
                    if ui.add(egui::Button::new(crate::i18n::t("Ladda")).fill(crate::ui::theme::palette().border())).clicked() {
                        let path = self.custom_stem_path_input.trim().to_string();
                        if path.to_lowercase().ends_with(".zip") {
                            zip_to_import = Some(path);
                        } else {
                            let (title, bpm) = Self::parse_suno_zip_info(&path);
                            folder_to_import = Some((path, title, bpm));
                        }
                    }
                });

                ui.add_space(3.0);
                ui.label(egui::RichText::new(crate::i18n::t("💡 Tips: Du kan även dra & släppa (Drag & Drop) .zip-filer direkt in i fönstret!")).size(Theme::FONT_XS).color(crate::ui::theme::palette().green()));
            });

            ui.add_space(8.0);

            // Stems Layout Preview Info
            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("SPÅRSTRUKTUR I SONIX:")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                ui.horizontal_wrapped(|ui| {
                    let stems_info = [
                        ("🎙 0 Lead Vocals", "Vocal Bus"),
                        ("🗣 1 Backing Vocals", "Vocal Bus"),
                        ("🥁 2 Drums", "Drum Bus"),
                        ("🎸 3 Bass", "Drum Bus / Sidechain"),
                        ("🎸 4 Guitar", "Synth Bus"),
                        ("🪘 5 Percussion", "Drum Bus"),
                        ("🎹 6 Synth", "Synth Bus"),
                        ("✨ 7 Other", "FX Send"),
                    ];
                    for (st, bus) in stems_info.iter() {
                        ui.label(egui::RichText::new(format!("• {} ➔ [{}]  ", st, bus)).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                    }
                });
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new(crate::i18n::t("Stäng")).size(Theme::FONT_S)).clicked() {
                    close = true;
                }
            });
        });

    if let Some(zip_p) = zip_to_import {
        self.start_suno_zip_import(&zip_p);
        close = true;
    }

    if let Some((folder, title, bpm)) = folder_to_import {
        self.import_suno_stems_from_folder(&folder, &title, bpm);
        close = true;
    }

    if close {
        self.show_suno_import_modal = false;
    }
}
}

impl SonixApp {
pub(crate) fn render_stem_import_progress_modal(&mut self, ctx: &egui::Context) {
    let mut completed_payload = None;
    let mut progress_info = None;

    if let Ok(mut p) = self.stem_import_progress.try_lock() {
        if p.is_importing {
            ctx.request_repaint();
            progress_info = Some((
                p.stage.clone(),
                p.current_file.clone(),
                p.current_idx,
                p.total_files,
                p.progress_ratio,
            ));
        }
        if let Some(res) = p.completed_payload.take() {
            completed_payload = Some(res);
        }
        if let Some(err) = p.error_message.take() {
            self.status_message = crate::tstatus!("❌ Fel vid stämimport: {}", err);
        }
    }

    if let Some((stage, file, idx, total, ratio)) = progress_info {
        egui::Window::new(crate::i18n::t("⏳ Importerar stämspår..."))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(460.0);
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Läser in och avkodar stämmor (Stems)")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().orange()));
                            ui.label(egui::RichText::new(&stage).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                        });
                    });
                });

                ui.add_space(8.0);
                ui.add(egui::ProgressBar::new(ratio).show_percentage().animate(true));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if !file.is_empty() {
                        ui.label(egui::RichText::new(crate::tstatus!("Spår {} av {}: {}", idx, total, file)).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("{:.0}%", ratio * 100.0)).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                    });
                });
                ui.add_space(4.0);
            });
    }

    if let Some(res) = completed_payload {
        self.apply_imported_stems(res);
    }
}
}

impl SonixApp {
pub fn open_stem_focus(&mut self, track_idx: usize) {
    if track_idx < self.playlist_tracks.len() {
        self.focused_stem_track = Some(track_idx);
        self.selected_timeline_track = track_idx;
        self.show_stem_focus_modal = true;
        self.status_message = crate::tstatus!("🔍 Öppnade stämeditor för '{}'", self.playlist_tracks[track_idx].name);
    }
}
}

impl SonixApp {
pub(crate) fn render_stem_focus_modal(&mut self, ctx: &egui::Context) {
    if !self.show_stem_focus_modal {
        return;
    }

    let num_tracks = self.playlist_tracks.len();
    if num_tracks == 0 {
        self.show_stem_focus_modal = false;
        return;
    }

    let t_idx = self.focused_stem_track.unwrap_or(0).min(num_tracks.saturating_sub(1));
    let mut close_modal = false;
    let mut open_window = self.show_stem_focus_modal;
    let mut trigger_audio_sync = false;
    let mut trigger_region_sync = false;
    let mut navigate_idx: Option<usize> = None;
    let mut seek_to_sec: Option<f32> = None;
    // Frysningen görs efter stängningen: anropet behöver &mut self, och
    // inne i panelen lånas spåret.
    let mut freeze_request: Option<usize> = None;
    let mut bounce_request: Option<usize> = None;
    let mut group_bounce_request: Option<usize> = None;
    let mut declick_request: Option<usize> = None;
    let mut spectral_request: Option<usize> = None;
    let mut swap_request: Option<usize> = None;
    let mut group_new_request: Option<usize> = None;
    let mut group_toggle_request: Option<usize> = None;
    // `Some(true)` = fäll ihop alla, `Some(false)` = fäll upp alla.
    let mut group_collapse_all_request: Option<bool> = None;
    let mut unfreeze_request: Option<usize> = None;
    let mut flatten_request: Option<usize> = None;
    let mut consolidate_request: Option<usize> = None;
    let frozen_stale = self.frozen_is_stale(t_idx);

    // Samma karta som tidlinjen (Fas 8.2 steg 3): panelen visar tider, och en tid efter
    // ett tempobyte är bara rätt om den kommer från kartan.
    let tempo = self.tempo_map();
    // Spelhuvudets takt, för korsningen som bara gäller skarven man står på.
    let playhead_bar = tempo.bar_at_secs(self.song_time as f64) as f32;

    // **De fyra frågorna `sec_per_bar` svarade på — men med fel svar efter ett
    // tempobyte** (Fas 8.2 steg 3). En *plats* i tiden, en *längd*, en *omvändning*
    // och en *lokal* sekunder-per-takt är olika frågor; en enda skalär kunde bara
    // svara rätt på dem så länge tempot var konstant. Med ett tempo ger de samma tal
    // som förut, bit för bit.
    let secs_at = |bar: f32| tempo.secs_at_bar(bar as f64) as f32;
    let secs_len = |from_bar: f32, bars: f32| tempo.secs_for_bars_at(from_bar as f64, bars as f64) as f32;
    let bars_len = |from_bar: f32, secs: f32| tempo.bars_for_secs_at(from_bar as f64, secs as f64) as f32;

    egui::Window::new(crate::i18n::t("🎛 Stämeditor & Ljudfokus"))
        .open(&mut open_window)
        .collapsible(true)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(760.0, 560.0))
        .show(ctx, |ui| {
            let track = &mut self.playlist_tracks[t_idx];

            // ============================================================
            // 1. TOP HEADER & STEM NAVIGATION & SOLO ISOLATION
            // ============================================================
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    // Prev / Next Stem Navigator
                    if ui.button(crate::i18n::t("◀ Föregående")).on_hover_text(crate::i18n::t("Växla till föregående stämma")).clicked() {
                        if t_idx > 0 {
                            navigate_idx = Some(t_idx - 1);
                        } else {
                            navigate_idx = Some(num_tracks - 1);
                        }
                    }
                    if ui.button(crate::i18n::t("Nästa ▶")).on_hover_text(crate::i18n::t("Växla till nästa stämma")).clicked() {
                        if t_idx + 1 < num_tracks {
                            navigate_idx = Some(t_idx + 1);
                        } else {
                            navigate_idx = Some(0);
                        }
                    }

                    ui.separator();

                    // Stem Icon & Name TextEdit
                    ui.label(egui::RichText::new(track.icon).size(Theme::FONT_XL));
                    ui.add(egui::TextEdit::singleline(&mut track.name).desired_width(180.0).font(egui::FontId::proportional(Theme::FONT_M)));

                    ui.separator();

                    // SOLO / ISOLERA STÄMMA BUTTON (Prominent & Glowing)
                    let solo_bg = if track.solo { crate::ui::theme::palette().orange() } else { crate::ui::theme::palette().header() };
                    let solo_fg = if track.solo { Color32::BLACK } else { crate::ui::theme::palette().orange() };
                    if ui.add(egui::Button::new(egui::RichText::new(if track.solo { "🎧 ISOLERAD (SOLO ON)" } else { crate::i18n::t("🎧 ISOLERA STÄMMA") }).strong().color(solo_fg)).fill(solo_bg)).on_hover_text(crate::i18n::t("Isolera och lyssna enbart på denna stämma i realtid")).clicked() {
                        track.solo = !track.solo;
                        trigger_audio_sync = true;
                    }

                    // MUTE TOGGLE
                    let mute_bg = if track.muted { crate::ui::theme::palette().red() } else { crate::ui::theme::palette().panel_alt() };
                    let mute_fg = if track.muted { crate::ui::theme::palette().text() } else { crate::ui::theme::palette().text_muted() };
                    if ui.add(egui::Button::new(egui::RichText::new(if track.muted { "🔇 MUTAD" } else { "🔊 AKTIV" }).strong().color(mute_fg)).fill(mute_bg)).clicked() {
                        track.muted = !track.muted;
                        trigger_audio_sync = true;
                    }

                    // FRYS / TINA (Tier 2). Ett fruset spår ligger som
                    // färdigt ljud: uppspelningen slipper köra syntesen, och
                    // patterns finns kvar i projektet så att en upptining
                    // återställer exakt samma musik.
                    ui.separator();
                    if track.is_frozen() {
                        let label = if frozen_stale {
                            crate::i18n::t("🔥 Tina (inaktuell)")
                        } else {
                            crate::i18n::t("🔥 Tina")
                        };
                        let bg = if frozen_stale {
                            crate::ui::theme::palette().track_amber()
                        } else {
                            crate::ui::theme::palette().border_soft()
                        };
                        if ui.add(egui::Button::new(egui::RichText::new(label).strong().color(crate::ui::theme::palette().text())).fill(bg))
                            .on_hover_text(crate::i18n::t("Spåret spelas som färdigt ljud. Tina upp det för att köra patterns igen — musiken är oförändrad."))
                            .clicked()
                        {
                            unfreeze_request = Some(t_idx);
                        }
                        // **Konsolidera**: spåret blir ett klipp i stället för flera.
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🧩 Konsolidera spår")).strong().color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().border()))
                            .on_hover_text(crate::i18n::t("Renderar spåret och ersätter alla dess klipp med ett enda. Volym, pan, EQ och sends är kvar."))
                            .clicked()
                        {
                            consolidate_request = Some(t_idx);
                        }
                        // **Platta till**: den frysta bufferten blir en region, så spåret är
                        // ljud på riktigt och går att klippa i som vilket ljudspår som helst.
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎵 Platta till")).strong().color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().panel_alt()))
                            .on_hover_text(crate::i18n::t("Gör den frysta bufferten till en region på samma spår — sedan beter sig spåret som inspelat ljud."))
                            .clicked()
                        {
                            flatten_request = Some(t_idx);
                        }
                        if frozen_stale {
                            ui.label(egui::RichText::new(crate::i18n::t("⚠ ändrat sedan frysningen")).size(Theme::FONT_S).color(crate::ui::theme::palette().orange()))
                                .on_hover_text(crate::i18n::t("Mönstret eller ljudet har ändrats efter frysningen, så ljudet är inte längre det du hör av patterns. Tina och frys igen."));
                        }
                    } else {
                        // Frysningen byter ut spårets uppspelning och gäller därför bara
                        // pattern-spår; bakningen lägger ljudet på ett nytt spår och gäller
                        // alla som inte redan är frusna.
                        if track.can_freeze() && ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("❄ Frys spår")).strong().color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().border()))
                            .on_hover_text(crate::i18n::t("Renderar spåret till ljud och spelar det i stället för patterns — sparar CPU i stora projekt. Patterns finns kvar, och Ctrl+Z tar tillbaka frysningen."))
                            .clicked()
                        {
                            freeze_request = Some(t_idx);
                        }
                        // Baka till ett **nytt** spår: samma render, men källan ligger kvar
                        // (tystad) så att de två går att jämföra.
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎧 Baka till nytt spår")).strong().color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().panel_alt()))
                            .on_hover_text(crate::i18n::t("Renderar spåret till ett nytt ljudspår och tystar källan. Inget tas bort, och Ctrl+Z tar tillbaka hela bakningen."))
                            .clicked()
                        {
                            bounce_request = Some(t_idx);
                        }
                        // Baka **hela gruppen** — bara när spåret faktiskt ligger i en grupp.
                        if let Some(g) = track.group
                            && ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎧 Baka gruppen till nytt spår")).strong().color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().panel_alt()))
                                .on_hover_text(crate::i18n::t("Renderar gruppens spår tillsammans till ett nytt ljudspår och tystar dem. Ctrl+Z tar tillbaka hela bakningen."))
                                .clicked()
                            {
                                group_bounce_request = Some(g);
                            }
                        // **Klicka bort skarvar:** lagar hopp i det inspelade materialet.
                        // Bara spår med ljud kan ha klickar att laga.
                        // **Spågrupp** (FL): lägg spåret i en grupp och fäll ihop den. Det
                        // första spåret i gruppen syns alltid, så att raden man fäller upp
                        // gruppen med inte försvinner.
                        match track.group {
                            None => {
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📁 Ny grupp")).size(Theme::FONT_S)))
                                    .on_hover_text(crate::i18n::t("Lägger spåret i en ny grupp som kan fällas ihop"))
                                    .clicked()
                                {
                                    group_new_request = Some(t_idx);
                                }
                            }
                            Some(g) => {
                                let collapsed = self.track_groups.get(g).map(|gr| gr.collapsed).unwrap_or(false);
                                let label = if collapsed { crate::i18n::t("📁 Fäll upp gruppen") } else { crate::i18n::t("📁 Fäll ihop gruppen") };
                                if ui.add(egui::Button::new(egui::RichText::new(label).size(Theme::FONT_S))).clicked() {
                                    group_toggle_request = Some(g);
                                }
                            }
                        }
                        // **Alla grupper på en gång** (FL: `Collapse all grouped tracks`).
                        if !self.track_groups.is_empty() {
                            ui.horizontal(|ui| {
                                if ui.small_button("📁 Fäll ihop alla").clicked() {
                                    group_collapse_all_request = Some(true);
                                }
                                if ui.small_button("📁 Fäll upp alla").clicked() {
                                    group_collapse_all_request = Some(false);
                                }
                            });
                        }
                        // **Byt källa:** spåret behåller mix och inställningar, bara ljudet byts.
                        if track.pcm_audio.is_some()
                            && ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔄 Byt källa")).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().border()))
                                .on_hover_text(crate::i18n::t("Låter spåret få ett annat ljud men behålla volym, panorering, EQ, sends och automation."))
                                .clicked()
                        {
                            swap_request = Some(t_idx);
                        }
                        if track.pcm_audio.is_some()
                            && ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🧹 Ta bort klick")).color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().border()))
                                .on_hover_text(crate::i18n::t("Hittar tvära hopp i spårets ljud och interpolerar över dem. Musiken runt omkring lämnas orörd."))
                                .clicked()
                        {
                            declick_request = Some(t_idx);
                        }
                        if track.pcm_audio.is_some()
                            && ui.add(egui::Button::new(egui::RichText::new("🎨 Spektral målning").color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().border()))
                                .on_hover_text("Måla dämpning direkt på spårets spektrogram — för att plocka bort ett ljud utan att röra resten.")
                                .clicked()
                        {
                            spectral_request = Some(t_idx);
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(crate::i18n::t("❌ Stäng")).clicked() {
                            close_modal = true;
                        }
                    });
                });
            });

            ui.add_space(4.0);

            // ============================================================
            // 2. TAB SELECTOR BAR
            // ============================================================
            ui.horizontal(|ui| {
                let tabs = [
                    (0, "🎚 Volym, Pan & Dynamik"),
                    (1, "📈 3-Bands Parametrisk EQ"),
                    (2, "⏱ Fading & Tidsredigering (0.01s)"),
                    (3, crate::i18n::t("🎵 Klipp & Vågform")),
                ];
                for (tab_idx, label) in tabs {
                    let is_sel = self.stem_focus_active_tab == tab_idx;
                    let bg = if is_sel { crate::ui::theme::palette().cyan() } else { crate::ui::theme::palette().panel() };
                    let fg = if is_sel { Color32::BLACK } else { crate::ui::theme::palette().text() };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(label)).strong().size(Theme::FONT_S).color(fg)).fill(bg)).clicked() {
                        self.stem_focus_active_tab = tab_idx;
                    }
                }
            });

            ui.add_space(4.0);

            // ============================================================
            // 3. TAB CONTENT
            // ============================================================
            match self.stem_focus_active_tab {
                0 => {
                    // TAB 1: VOLYM, PAN & DYNAMIK
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Nivåer & Stereobild för stämman")).strong().color(crate::ui::theme::palette().orange()));
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            // Volume Section
                            ui.group(|ui| {
                                ui.set_width(340.0);
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("🎚 Exakt Ljudvolym & Gain")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                                    ui.add_space(4.0);

                                    let cur_vol = track.volume;
                                    let gain_db = if cur_vol <= 0.001 { -60.0 } else { 20.0 * cur_vol.log10() };
                                    ui.label(egui::RichText::new(crate::tstatus!("Gain: {:+.2} dB  (Nivå: {:.1}%)", gain_db, cur_vol * 100.0)).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().text()));

                                    ui.add_space(4.0);
                                    let mut vol_slider = track.volume;
                                    if ui.add(egui::Slider::new(&mut vol_slider, 0.0..=2.0).custom_formatter(|v, _| {
                                        let db = if v <= 0.001 { -60.0 } else { 20.0 * v.log10() };
                                        format!("{:+.1} dB ({:.0}%)", db, v * 100.0)
                                    })).changed() {
                                        track.volume = vol_slider;
                                        trigger_audio_sync = true;
                                    }

                                    ui.add_space(6.0);
                                    ui.label(crate::i18n::t("Snabbjustering (0.1 dB precision):"));
                                    ui.horizontal(|ui| {
                                        if ui.button(crate::i18n::t("-0.5 dB")).clicked() {
                                            track.volume = (track.volume * 10.0_f32.powf(-0.5 / 20.0)).max(0.0);
                                            trigger_audio_sync = true;
                                        }
                                        if ui.button(crate::i18n::t("-0.1 dB")).clicked() {
                                            track.volume = (track.volume * 10.0_f32.powf(-0.1 / 20.0)).max(0.0);
                                            trigger_audio_sync = true;
                                        }
                                        if ui.button(crate::i18n::t("0.0 dB (100%)")).clicked() {
                                            track.volume = 1.0;
                                            trigger_audio_sync = true;
                                        }
                                        if ui.button(crate::i18n::t("+0.1 dB")).clicked() {
                                            track.volume = (track.volume * 10.0_f32.powf(0.1 / 20.0)).min(2.0);
                                            trigger_audio_sync = true;
                                        }
                                        if ui.button(crate::i18n::t("+0.5 dB")).clicked() {
                                            track.volume = (track.volume * 10.0_f32.powf(0.5 / 20.0)).min(2.0);
                                            trigger_audio_sync = true;
                                        }
                                    });

                                    ui.horizontal(|ui| {
                                        if ui.button(crate::i18n::t("📉 -3.0 dB")).clicked() {
                                            track.volume = (track.volume * 10.0_f32.powf(-3.0 / 20.0)).max(0.0);
                                            trigger_audio_sync = true;
                                        }
                                        if ui.button(crate::i18n::t("📉 -6.0 dB")).clicked() {
                                            track.volume = (track.volume * 10.0_f32.powf(-6.0 / 20.0)).max(0.0);
                                            trigger_audio_sync = true;
                                        }
                                        if ui.button(crate::i18n::t("🔇 Muta")).clicked() {
                                            track.volume = 0.0;
                                            trigger_audio_sync = true;
                                        }
                                    });
                                });
                            });

                            ui.separator();

                            // Pan & Dynamics Section
                            ui.group(|ui| {
                                ui.set_width(340.0);
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("↔ Stereopanorering")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                                    let pan_text = if track.pan < -0.01 {
                                        crate::tstatus!("Vänster {:.0}%", track.pan.abs() * 100.0)
                                    } else if track.pan > 0.01 {
                                        crate::tstatus!("Höger {:.0}%", track.pan * 100.0)
                                    } else {
                                        crate::i18n::t("Center (Mitt)").to_string()
                                    };
                                    ui.label(egui::RichText::new(format!("Pan: {}", pan_text)).color(crate::ui::theme::palette().text()));

                                    let mut pan_val = track.pan;
                                    if ui.add(egui::Slider::new(&mut pan_val, -1.0..=1.0).text("L / R")).changed() {
                                        track.pan = pan_val;
                                        trigger_audio_sync = true;
                                    }

                                    ui.horizontal(|ui| {
                                        if ui.button(crate::i18n::t("100% V")).clicked() { track.pan = -1.0; trigger_audio_sync = true; }
                                        if ui.button(crate::i18n::t("50% V")).clicked() { track.pan = -0.5; trigger_audio_sync = true; }
                                        if ui.button(crate::i18n::t("Center")).clicked() { track.pan = 0.0; trigger_audio_sync = true; }
                                        if ui.button(crate::i18n::t("50% H")).clicked() { track.pan = 0.5; trigger_audio_sync = true; }
                                        if ui.button(crate::i18n::t("100% H")).clicked() { track.pan = 1.0; trigger_audio_sync = true; }
                                    });

                                    ui.add_space(8.0);
                                    ui.label(egui::RichText::new(crate::i18n::t("🎛 Dynamik & Kompressor")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                                    ui.add(egui::Slider::new(&mut track.comp_threshold_db, -30.0..=0.0).text("Threshold (dB)"));
                                    ui.add(egui::Slider::new(&mut track.comp_ratio, 1.0..=8.0).text("Ratio"));
                                });
                            });
                        });
                    });
                }

                1 => {
                    // TAB 2: 3-BANDS PARAMETRISK EQ MED GRAFISK KURVA
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("📈 3-Bands Parametrisk Stäm-EQ")).strong().color(crate::ui::theme::palette().orange()));
                            ui.checkbox(&mut track.eq.enabled, "Aktivera EQ");
                        });

                        ui.add_space(4.0);

                        // Graphical EQ Curve Canvas
                        let (eq_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(10.0), 130.0), Sense::hover());
                        let p = ui.painter();
                        p.rect_filled(eq_rect, Rounding::same(4.0), crate::ui::theme::palette().bg_deep());
                        p.rect_stroke(eq_rect, Rounding::same(4.0), Stroke::new(1.0_f32, crate::ui::theme::palette().grid()));

                        // Draw Grid Lines (100 Hz, 1 kHz, 10 kHz and 0 dB, +6 dB, -6 dB)
                        let mid_y = eq_rect.center().y;
                        p.line_segment([Pos2::new(eq_rect.min.x, mid_y), Pos2::new(eq_rect.max.x, mid_y)], Stroke::new(1.0_f32, crate::ui::theme::palette().border()));
                        p.line_segment([Pos2::new(eq_rect.min.x, mid_y - 30.0), Pos2::new(eq_rect.max.x, mid_y - 30.0)], Stroke::new(0.5_f32, crate::ui::theme::palette().grid()));
                        p.line_segment([Pos2::new(eq_rect.min.x, mid_y + 30.0), Pos2::new(eq_rect.max.x, mid_y + 30.0)], Stroke::new(0.5_f32, crate::ui::theme::palette().grid()));

                        p.text(Pos2::new(eq_rect.min.x + 6.0, mid_y - 30.0), egui::Align2::LEFT_CENTER, "+6 dB", egui::FontId::proportional(Theme::FONT_XS), crate::ui::theme::palette().track_steel());
                        p.text(Pos2::new(eq_rect.min.x + 6.0, mid_y), egui::Align2::LEFT_CENTER, "0 dB", egui::FontId::proportional(Theme::FONT_XS), crate::ui::theme::palette().text_muted());
                        p.text(Pos2::new(eq_rect.min.x + 6.0, mid_y + 30.0), egui::Align2::LEFT_CENTER, "-6 dB", egui::FontId::proportional(Theme::FONT_XS), crate::ui::theme::palette().track_steel());

                        // Frequency Grid Markers
                        let freq_x = |f: f32| -> f32 {
                            let min_log = 20.0_f32.log10();
                            let max_log = 20000.0_f32.log10();
                            let norm = (f.log10() - min_log) / (max_log - min_log);
                            eq_rect.min.x + norm * eq_rect.width()
                        };

                        for &(freq, lbl) in &[(100.0, "100Hz"), (1000.0, "1kHz"), (10000.0, "10kHz")] {
                            let fx = freq_x(freq);
                            p.line_segment([Pos2::new(fx, eq_rect.min.y), Pos2::new(fx, eq_rect.max.y)], Stroke::new(0.5_f32, crate::ui::theme::palette().grid()));
                            p.text(Pos2::new(fx, eq_rect.max.y - 8.0), egui::Align2::CENTER_CENTER, lbl, egui::FontId::proportional(Theme::FONT_XS), crate::ui::theme::palette().track_steel());
                        }

                        // Plot Smooth EQ Curve
                        if track.eq.enabled {
                            let num_points = 80;
                            let mut curve_pts = Vec::with_capacity(num_points);
                            for i in 0..num_points {
                                let norm = i as f32 / (num_points - 1) as f32;
                                let freq = 20.0 * 1000.0_f32.powf(norm); // 20 Hz to 20 kHz

                                // Low shelf calculation
                                let low_resp = track.eq.low_gain_db / (1.0 + (freq / track.eq.low_freq).powi(2));

                                // Mid bell calculation
                                let mid_ratio = (freq / track.eq.mid_freq).ln();
                                let mid_resp = track.eq.mid_gain_db * (-0.5 * (mid_ratio * track.eq.mid_q).powi(2)).exp();

                                // High shelf calculation
                                let high_resp = track.eq.high_gain_db * (freq / track.eq.high_freq).powi(2) / (1.0 + (freq / track.eq.high_freq).powi(2));

                                let total_db = low_resp + mid_resp + high_resp;
                                let px = eq_rect.min.x + norm * eq_rect.width();
                                let py = mid_y - (total_db * 5.0).clamp(-55.0, 55.0);
                                curve_pts.push(Pos2::new(px, py));
                            }

                            for pair in curve_pts.windows(2) {
                                p.line_segment([pair[0], pair[1]], Stroke::new(2.0_f32, crate::ui::theme::palette().cyan()));
                            }

                            // Draw Control Points for Low, Mid, High
                            let low_pt = Pos2::new(freq_x(track.eq.low_freq), mid_y - track.eq.low_gain_db * 5.0);
                            let mid_pt = Pos2::new(freq_x(track.eq.mid_freq), mid_y - track.eq.mid_gain_db * 5.0);
                            let high_pt = Pos2::new(freq_x(track.eq.high_freq), mid_y - track.eq.high_gain_db * 5.0);

                            p.circle_filled(low_pt, 4.5, crate::ui::theme::palette().orange());
                            p.circle_filled(mid_pt, 4.5, crate::ui::theme::palette().green());
                            p.circle_filled(high_pt, 4.5, crate::ui::theme::palette().purple());
                        }

                        ui.add_space(6.0);

                        // EQ Knobs & Sliders Row
                        ui.horizontal(|ui| {
                            // Low Band
                            ui.group(|ui| {
                                ui.set_width(220.0);
                                ui.label(egui::RichText::new(crate::i18n::t("🔊 Bas (Low Shelf)")).strong().color(crate::ui::theme::palette().orange()));
                                ui.add(egui::Slider::new(&mut track.eq.low_gain_db, -12.0..=12.0).text("Gain (dB)").suffix(" dB"));
                                ui.add(egui::Slider::new(&mut track.eq.low_freq, 40.0..=400.0).text("Frekvens").suffix(" Hz"));
                                ui.horizontal(|ui| {
                                    if ui.button(crate::i18n::t("-1dB")).clicked() { track.eq.low_gain_db = (track.eq.low_gain_db - 1.0).max(-12.0); }
                                    if ui.button(crate::i18n::t("0dB")).clicked() { track.eq.low_gain_db = 0.0; }
                                    if ui.button(crate::i18n::t("+1dB")).clicked() { track.eq.low_gain_db = (track.eq.low_gain_db + 1.0).min(12.0); }
                                });
                            });

                            // Mid Band
                            ui.group(|ui| {
                                ui.set_width(220.0);
                                ui.label(egui::RichText::new(crate::i18n::t("🎙 Mellanregister (Mid Peak)")).strong().color(crate::ui::theme::palette().green()));
                                ui.add(egui::Slider::new(&mut track.eq.mid_gain_db, -12.0..=12.0).text("Gain (dB)").suffix(" dB"));
                                ui.add(egui::Slider::new(&mut track.eq.mid_freq, 200.0..=6000.0).text("Frekvens").suffix(" Hz"));
                                ui.add(egui::Slider::new(&mut track.eq.mid_q, 0.5..=3.0).text("Q (Bredd)"));
                            });

                            // High Band
                            ui.group(|ui| {
                                ui.set_width(220.0);
                                ui.label(egui::RichText::new(crate::i18n::t("✨ Diskant (High Shelf)")).strong().color(crate::ui::theme::palette().purple()));
                                ui.add(egui::Slider::new(&mut track.eq.high_gain_db, -12.0..=12.0).text("Gain (dB)").suffix(" dB"));
                                ui.add(egui::Slider::new(&mut track.eq.high_freq, 3000.0..=16000.0).text("Frekvens").suffix(" Hz"));
                                ui.horizontal(|ui| {
                                    if ui.button(crate::i18n::t("-1dB")).clicked() { track.eq.high_gain_db = (track.eq.high_gain_db - 1.0).max(-12.0); }
                                    if ui.button(crate::i18n::t("0dB")).clicked() { track.eq.high_gain_db = 0.0; }
                                    if ui.button(crate::i18n::t("+1dB")).clicked() { track.eq.high_gain_db = (track.eq.high_gain_db + 1.0).min(12.0); }
                                });
                            });
                        });

                        ui.add_space(4.0);

                        // EQ Presets
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Förinställningar:")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            if ui.button(crate::i18n::t("🎙 Sång: Luft & Värme")).clicked() {
                                track.eq.low_gain_db = -2.0; track.eq.low_freq = 100.0;
                                track.eq.mid_gain_db = 1.5; track.eq.mid_freq = 3000.0; track.eq.mid_q = 1.2;
                                track.eq.high_gain_db = 3.5; track.eq.high_freq = 9000.0;
                            }
                            if ui.button(crate::i18n::t("🥁 Trummor: Punch & Snap")).clicked() {
                                track.eq.low_gain_db = 3.0; track.eq.low_freq = 75.0;
                                track.eq.mid_gain_db = -2.5; track.eq.mid_freq = 450.0; track.eq.mid_q = 1.0;
                                track.eq.high_gain_db = 2.0; track.eq.high_freq = 7500.0;
                            }
                            if ui.button(crate::i18n::t("🎸 Gitarr: Presence")).clicked() {
                                track.eq.low_gain_db = -3.0; track.eq.low_freq = 120.0;
                                track.eq.mid_gain_db = 2.0; track.eq.mid_freq = 2400.0; track.eq.mid_q = 1.4;
                                track.eq.high_gain_db = 1.0; track.eq.high_freq = 6000.0;
                            }
                            if ui.button(crate::i18n::t("🔊 Bas: Deep Sub")).clicked() {
                                track.eq.low_gain_db = 4.0; track.eq.low_freq = 65.0;
                                track.eq.mid_gain_db = -3.0; track.eq.mid_freq = 1000.0; track.eq.mid_q = 1.0;
                                track.eq.high_gain_db = -4.0; track.eq.high_freq = 5000.0;
                            }
                            if ui.button(crate::i18n::t("🔄 Nollställ (Flat)")).clicked() {
                                track.eq = TrackEq::default();
                            }
                        });
                    });

                    trigger_audio_sync = true;
                }

                2 => {
                    // TAB 3: FADING & TIDSREDIGERING (0.01s Precision)
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("⏱ Fading & Fade-kurvor (Exakt hundradels precision)")).strong().color(crate::ui::theme::palette().orange()));
                        ui.add_space(4.0);

                        // If there are regions on this track, allow editing the first / selected region or batch-apply
                        let num_regions = track.regions.len();
                        let mut apply_all_fades = false;
                        let mut global_fade_in_sec = 0.05_f32;
                        let mut global_fade_out_sec = 0.05_f32;

                        if num_regions > 0 {
                            global_fade_in_sec = secs_len(track.regions[0].start_bar, track.regions[0].fade_in_bars);
                            global_fade_out_sec = secs_len(track.regions[0].start_bar, track.regions[0].fade_out_bars);
                        }

                        ui.horizontal(|ui| {
                            // Fade In Box
                            ui.group(|ui| {
                                ui.set_width(340.0);
                                ui.label(egui::RichText::new(crate::i18n::t("📈 Fade In (In-toning)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                                let cs_in = (global_fade_in_sec * 100.0).round() as i32;
                                ui.label(egui::RichText::new(crate::tstatus!("Längd: {:.2} sekunder  ({} hundradelar)", global_fade_in_sec, cs_in)).strong().color(crate::ui::theme::palette().text()));

                                ui.add_space(4.0);
                                if ui.add(egui::Slider::new(&mut global_fade_in_sec, 0.0..=5.0).text("Sekunder")).changed() {
                                    apply_all_fades = true;
                                }

                                ui.add_space(4.0);
                                ui.label(crate::i18n::t("Finjustera med hundradelar:"));
                                ui.horizontal(|ui| {
                                    if ui.button(crate::i18n::t("-0.10s")).clicked() { global_fade_in_sec = (global_fade_in_sec - 0.10).max(0.0); apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("-0.01s")).clicked() { global_fade_in_sec = (global_fade_in_sec - 0.01).max(0.0); apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("0.00s")).clicked() { global_fade_in_sec = 0.0; apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("+0.01s")).clicked() { global_fade_in_sec = (global_fade_in_sec + 0.01).min(10.0); apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("+0.10s")).clicked() { global_fade_in_sec = (global_fade_in_sec + 0.10).min(10.0); apply_all_fades = true; }
                                });

                                ui.horizontal(|ui| {
                                    if ui.button(crate::i18n::t("20 ms")).clicked() { global_fade_in_sec = 0.02; apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("50 ms")).clicked() { global_fade_in_sec = 0.05; apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("100 ms")).clicked() { global_fade_in_sec = 0.10; apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("500 ms")).clicked() { global_fade_in_sec = 0.50; apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("1.00 s")).clicked() { global_fade_in_sec = 1.00; apply_all_fades = true; }
                                });
                            });

                            ui.separator();

                            // Fade Out Box
                            ui.group(|ui| {
                                ui.set_width(340.0);
                                ui.label(egui::RichText::new(crate::i18n::t("📉 Fade Out (Ut-toning)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                                let cs_out = (global_fade_out_sec * 100.0).round() as i32;
                                ui.label(egui::RichText::new(crate::tstatus!("Längd: {:.2} sekunder  ({} hundradelar)", global_fade_out_sec, cs_out)).strong().color(crate::ui::theme::palette().text()));

                                ui.add_space(4.0);
                                if ui.add(egui::Slider::new(&mut global_fade_out_sec, 0.0..=5.0).text("Sekunder")).changed() {
                                    apply_all_fades = true;
                                }

                                ui.add_space(4.0);
                                ui.label(crate::i18n::t("Finjustera med hundradelar:"));
                                ui.horizontal(|ui| {
                                    if ui.button(crate::i18n::t("-0.10s")).clicked() { global_fade_out_sec = (global_fade_out_sec - 0.10).max(0.0); apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("-0.01s")).clicked() { global_fade_out_sec = (global_fade_out_sec - 0.01).max(0.0); apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("0.00s")).clicked() { global_fade_out_sec = 0.0; apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("+0.01s")).clicked() { global_fade_out_sec = (global_fade_out_sec + 0.01).min(10.0); apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("+0.10s")).clicked() { global_fade_out_sec = (global_fade_out_sec + 0.10).min(10.0); apply_all_fades = true; }
                                });

                                ui.horizontal(|ui| {
                                    if ui.button(crate::i18n::t("20 ms")).clicked() { global_fade_out_sec = 0.02; apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("50 ms")).clicked() { global_fade_out_sec = 0.05; apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("100 ms")).clicked() { global_fade_out_sec = 0.10; apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("500 ms")).clicked() { global_fade_out_sec = 0.50; apply_all_fades = true; }
                                    if ui.button(crate::i18n::t("1.00 s")).clicked() { global_fade_out_sec = 1.00; apply_all_fades = true; }
                                });
                            });
                        });

                        if apply_all_fades {
                            for r in &mut track.regions {
                                r.fade_in_bars = bars_len(r.start_bar, global_fade_in_sec);
                                r.fade_out_bars = bars_len(r.start_bar, global_fade_out_sec);
                            }
                            trigger_region_sync = true;
                        }

                        // Kanalanteckningar (FL:s Fruity NoteBook). Inget som hörs, så ingen
                        // synk behövs mot motorn — bara fältet som följer med projektfilen.
                        ui.add_space(6.0);
                        ui.collapsing(crate::i18n::t("📓 Kanalanteckningar"), |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut track.notes)
                                    .desired_rows(3)
                                    .desired_width(320.0),
                            );
                        });

                        // Korsningen lägger bara fade-värden där klipp överlappar — regeln
                        // (och dess halvering vid stora överlapp) bor i `create_crossfades`.
                        ui.add_space(6.0);
                        if ui.button(crate::i18n::t("✚ Korsa överlappande klipp")).clicked()
                            && track.create_crossfades() > 0
                        {
                            trigger_region_sync = true;
                        }
                        if ui
                            .button("✚ Korsa vid spelhuvudet")
                            .on_hover_text("Korsar bara skarven som ligger närmast spelhuvudet, inte hela spåret")
                            .clicked()
                            && track.create_crossfade_at(playhead_bar) > 0
                        {
                            trigger_region_sync = true;
                        }

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Tonhöjd (Pitch Shift):")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                            ui.add(
                                egui::Slider::new(&mut track.pitch_semitones, -12.0..=12.0)
                                    .step_by(0.01)
                                    .fixed_decimals(2)
                                    .suffix(" st"),
                            );
                            if ui.button(crate::i18n::t("Nollställ Pitch")).clicked() {
                                track.pitch_semitones = 0.0;
                            }
                        });
                    });
                }

                3 => {
                    // TAB 4: KLIPP & VÅGFORM
                    ui.group(|ui| {
                        ui.label(egui::RichText::new(format!("🎵 Klipp & Regioner i {}", track.name)).strong().color(crate::ui::theme::palette().orange()));
                        ui.add_space(4.0);

                        // Waveform Overview
                        let (wave_rect, wave_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(10.0), 90.0), Sense::click_and_drag());
                        let p = ui.painter();
                        p.rect_filled(wave_rect, Rounding::same(4.0), crate::ui::theme::palette().bg_deep());
                        p.rect_stroke(wave_rect, Rounding::same(4.0), Stroke::new(1.0_f32, crate::ui::theme::palette().grid()));

                        let mid_y = wave_rect.center().y;

                        // Draw Regions & Waves
                        let track_color = track.color;
                        for r in &track.regions {
                            let total_max_bars = 140.0_f32;
                            let rx_start = wave_rect.min.x + (r.start_bar / total_max_bars) * wave_rect.width();
                            let rx_end = rx_start + (r.length_bars / total_max_bars) * wave_rect.width();
                            let r_rect = Rect::from_min_max(Pos2::new(rx_start, wave_rect.min.y + 2.0), Pos2::new(rx_end, wave_rect.max.y - 2.0));

                            if r_rect.width() > 2.0 {
                                p.rect_filled(r_rect, Rounding::same(2.0), Color32::from_rgba_unmultiplied(track_color.r(), track_color.g(), track_color.b(), 60));
                                p.rect_stroke(r_rect, Rounding::same(2.0), Stroke::new(1.0_f32, track_color));

                                // Draw Wave peaks
                                let pts = r.waveform_peaks.len().max(1);
                                let w_step = r_rect.width() / pts as f32;
                                for (wi, &amp) in r.waveform_peaks.iter().enumerate() {
                                    let wx = r_rect.min.x + wi as f32 * w_step;
                                    let wh = amp * (r_rect.height() * 0.40);
                                    p.line_segment([Pos2::new(wx, mid_y - wh), Pos2::new(wx, mid_y + wh)], Stroke::new(1.0_f32, crate::ui::theme::palette().text()));
                                }
                            }
                        }

                        // Interactive Scrubbing on Waveform
                        if (wave_resp.clicked() || wave_resp.dragged())
                            && let Some(m_pos) = wave_resp.hover_pos() {
                                let norm = ((m_pos.x - wave_rect.min.x) / wave_rect.width()).clamp(0.0, 1.0);
                                let target_sec = norm * secs_len(0.0, 140.0);
                                seek_to_sec = Some(target_sec);
                            }

                        ui.add_space(6.0);

                        // Region Table
                        egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                            let mut del_region_idx: Option<usize> = None;
                            for (ri, r) in track.regions.iter_mut().enumerate() {
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new(format!("#{}: {}", ri + 1, r.name)).strong().color(crate::ui::theme::palette().text()));
                                    ui.separator();

                                    let r_start_sec = secs_at(r.start_bar);
                                    let r_len_sec = secs_len(r.start_bar, r.length_bars);
                                    ui.label(crate::tstatus!("Start: {}  |  Längd: {}", format_time_hundredths(r_start_sec), format_time_hundredths(r_len_sec)));

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button(crate::i18n::t("🗑 Ta bort")).clicked() {
                                            del_region_idx = Some(ri);
                                        }
                                        let m_txt = if r.muted { crate::i18n::t("🔇 Mutad") } else { crate::i18n::t("🔊 På") };
                                        if ui.button(m_txt).clicked() {
                                            r.muted = !r.muted;
                                            trigger_region_sync = true;
                                        }
                                    });
                                });
                                ui.separator();
                            }

                            if let Some(del_i) = del_region_idx
                                && del_i < track.regions.len() {
                                    track.regions.remove(del_i);
                                    trigger_region_sync = true;
                                }
                        });
                    });
                }

                _ => {}
            }
        });

    if let Some(target) = freeze_request {
        self.freeze_track(target);
    }
    if let Some(target) = flatten_request {
        self.flatten_track(target);
    }
    if let Some(target) = consolidate_request {
        self.consolidate_track(target);
    }
    if let Some(target) = bounce_request {
        self.bounce_track_to_new(target);
    }
    if let Some(group) = group_bounce_request {
        self.bounce_group_to_new(group);
    }
    if let Some(target) = declick_request {
        self.declick_track(target);
    }
    if let Some(target) = spectral_request {
        self.open_spectral_editor(target);
    }
    if let Some(t_idx) = group_new_request {
        let name = crate::tstatus!("Grupp {}", self.track_groups.len() + 1);
        self.track_groups.push(crate::ui::app::state::TrackGroup { name, collapsed: false });
        let g = self.track_groups.len() - 1;
        if let Some(track) = self.playlist_tracks.get_mut(t_idx) {
            track.group = Some(g);
        }
    }
    if let Some(g) = group_toggle_request
        && let Some(group) = self.track_groups.get_mut(g)
    {
        group.collapsed = !group.collapsed;
    }
    if let Some(collapsed) = group_collapse_all_request {
        crate::ui::app::state::set_all_groups_collapsed(&mut self.track_groups, collapsed);
    }
    if let Some(target) = swap_request {
        // Filväljaren går i egen tråd; svaret tas emot i `frame.rs` och kopplas till spåret.
        self.pending_swap_source = Some(target);
        self.spawn_async_file_picker("Ljud", &["wav", "mp3", "flac", "ogg", "m4a"], "Byt källa");
    }
    if let Some(target) = unfreeze_request {
        self.unfreeze_track(target);
    }

    if let Some(target_idx) = navigate_idx {
        self.focused_stem_track = Some(target_idx);
        self.selected_timeline_track = target_idx;
    }

    if let Some(target_sec) = seek_to_sec {
        self.seek_song_time(target_sec);
    }

    if trigger_audio_sync {
        self.sync_track_audio_state(t_idx);
    }
    if trigger_region_sync {
        self.sync_track_regions(t_idx);
    }

    if close_modal || !open_window {
        self.show_stem_focus_modal = false;
    }
}
}
