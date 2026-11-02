//! Projektdialoger: import, inläsning, återställning och projekthanteraren.
//!
//! Utdelad ur `modals.rs` 2026-11-02; filen var 4 486 rader och höll 34 dialoger.

use super::*;
use crate::ui::theme::Theme;

impl SonixApp {
pub(crate) fn render_import_sample_modal(&mut self, ctx: &egui::Context) {
    if !self.show_import_modal {
        return;
    }

    let mut close = false;
    egui::Window::new(crate::i18n::t("📥 Importera & Skapa Nytt Sample Ljud"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(400.0);
            ui.label(egui::RichText::new(crate::i18n::t("Utöka ditt personliga ljudbibliotek med egna samples!")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Ljudets Namn:"));
                ui.text_edit_singleline(&mut self.custom_import_name_input);
            });

            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Kategori:"));
                egui::ComboBox::from_id_salt("import_cat_combo")
                    .selected_text(&self.custom_import_category)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut self.custom_import_category, "Egna Importerade".to_string(), "Egna Importerade");
                        ui.selectable_value(&mut self.custom_import_category, "Kicks".to_string(), "Kicks");
                        ui.selectable_value(&mut self.custom_import_category, "Snares".to_string(), "Snares");
                        ui.selectable_value(&mut self.custom_import_category, "Claps".to_string(), "Claps");
                        ui.selectable_value(&mut self.custom_import_category, "Hi-Hats".to_string(), "Hi-Hats");
                        ui.selectable_value(&mut self.custom_import_category, "Percussion".to_string(), "Percussion");
                        ui.selectable_value(&mut self.custom_import_category, "Vocal Chops".to_string(), "Vocal Chops");
                        ui.selectable_value(&mut self.custom_import_category, "808 & Bass".to_string(), "808 & Bass");
                    });
            });

            ui.add_space(8.0);
            ui.label(egui::RichText::new(crate::i18n::t("Sample Karaktär & Syntes:")).strong().size(Theme::FONT_S));

            let icons = ["📂", "💥", "🥁", "👏", "⚡", "🌊", "🗣", "🎸", "🎹", "✨", "🔔"];
            ui.horizontal_wrapped(|ui| {
                ui.label(crate::i18n::t("Ikon:"));
                for &ic in icons.iter() {
                    let is_sel = self.custom_import_icon == ic;
                    let fill = if is_sel { crate::ui::theme::palette().green() } else { crate::ui::theme::palette().panel() };
                    if ui.add(egui::Button::new(ic).fill(fill)).clicked() {
                        self.custom_import_icon = ic.to_string();
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Grundton (MIDI):"));
                ui.add(egui::Slider::new(&mut self.custom_import_note, 24..=84).text("Ton"));
            });

            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Frekvens & Tonhöjd:"));
                ui.add(egui::Slider::new(&mut self.custom_import_freq, 10.0..=90.0));
            });

            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Decay Tid:"));
                ui.add(egui::Slider::new(&mut self.custom_import_decay, 0.1..=1.5));
            });

            ui.add_space(8.0);

            // Waveform Preview
            let preview_wave: Vec<f32> = (0..32).map(|i| {
                let t = i as f32 / 32.0;
                let env = (-t * self.custom_import_decay * 4.0).exp();
                (t * self.custom_import_freq).sin() * env
            }).collect();

            let (wf_rect, _) = ui.allocate_exact_size(Vec2::new(380.0, 32.0), egui::Sense::hover());
            ui.painter().rect_filled(wf_rect, Rounding::same(3.0), crate::ui::theme::palette().bg());
            let mid_y = wf_rect.center().y;
            let dx = wf_rect.width() / 32.0;
            for (wi, &val) in preview_wave.iter().enumerate() {
                let sx = wf_rect.min.x + wi as f32 * dx + dx * 0.5;
                let h = (val.abs() * 13.0).max(1.0);
                ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.5_f32, crate::ui::theme::palette().green()));
            }

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new(crate::i18n::t("▶ Provspela")).size(Theme::FONT_S)).clicked() {
                    let freq = midi_to_freq(self.custom_import_note);
                    let _ = self.engine.send_command(AudioCommand::NoteOn {
                        note: self.custom_import_note,
                        freq,
                        velocity: 0.9, channel: 0 });
                }

                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("💾 Spara i Bibliotek")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().green())).clicked() {
                    self.import_custom_sample(
                        self.custom_import_name_input.clone(),
                        self.custom_import_category.clone(),
                        self.custom_import_icon.clone(),
                        self.custom_import_note,
                        crate::ui::theme::palette().amber(),
                        self.custom_import_freq,
                        self.custom_import_decay,
                    );
                    close = true;
                }

                if ui.button(egui::RichText::new(crate::i18n::t("Avbryt")).size(Theme::FONT_S)).clicked() {
                    close = true;
                }
            });
        });

    if close {
        self.show_import_modal = false;
    }
}
}

impl SonixApp {
pub(crate) fn render_project_load_progress_modal(&mut self, ctx: &egui::Context) {
    let mut completed_payload = None;
    let mut progress_info = None;

    if let Ok(mut p) = self.project_load_progress.try_lock() {
        if p.is_loading {
            ctx.request_repaint();
            progress_info = Some((
                p.project_name.clone(),
                p.stage.clone(),
                p.current_track.clone(),
                p.current_idx,
                p.total_tracks,
                p.progress_ratio,
            ));
        }
        if let Some(res) = p.completed_payload.take() {
            p.is_loading = false;
            completed_payload = Some(res);
        }
        if let Some(err) = p.error_message.take() {
            p.is_loading = false;
            self.status_message = crate::tstatus!("❌ Fel vid projektladdning: {}", err);
        }
    }

    if let Some((proj, stage, track, idx, total, ratio)) = progress_info {
        egui::Window::new(crate::i18n::t("⏳ Öppnar projekt..."))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
            .show(ctx, |ui| {
                ui.set_width(480.0);
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(crate::tstatus!("Öppnar: {}", proj)).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().cyan()));
                            ui.label(egui::RichText::new(&stage).size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                        });
                    });
                });

                ui.add_space(8.0);
                ui.add(egui::ProgressBar::new(ratio).show_percentage().animate(true));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if !track.is_empty() {
                        ui.label(egui::RichText::new(crate::tstatus!("Läser spår {}/{}: {}", idx, total, track)).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("{:.0}%", ratio * 100.0)).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                    });
                });
                ui.add_space(4.0);
            });
    }

    if let Some(res) = completed_payload {
        self.apply_loaded_project_payload(res);
    }
}
}

impl SonixApp {
/// Kraschåterställning (Fas 6.1): erbjuder de autosaves som är nyare än sin
/// manuella projektfil. Dialogen visas bara när det finns något att rädda.
pub(crate) fn render_recovery_modal(&mut self, ctx: &egui::Context) {
    if !self.show_recovery_modal {
        return;
    }

    // Snapshot av listan: fönster-closuren får inte låna self samtidigt som
    // knapptryckningar vill ändra self.
    let rows: Vec<(String, String, u64)> = self
        .recovery_candidates
        .iter()
        .map(|c| {
            (
                c.name.clone(),
                c.entry.path.to_string_lossy().to_string(),
                c.entry.stamp,
            )
        })
        .collect();
    let now = crate::autosave::now_stamp();

    let mut restore: Option<usize> = None;
    let mut dismiss = false;
    let mut open = self.show_recovery_modal;
    egui::Window::new(crate::i18n::t("🛟 Osparat arbete hittades"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(560.0, 280.0))
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(crate::i18n::t(
                    "Sonix avslutades innan projektet sparades. Dessa automatiska kopior är nyare än filen på disk:",
                ))
                .size(Theme::FONT_S)
                .color(crate::ui::theme::palette().text()),
            );
            ui.add_space(8.0);

            for (i, (name, path, stamp)) in rows.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "🛟 {} — {}",
                            name,
                            crate::autosave::relative_age(*stamp, now)
                        ))
                        .size(Theme::FONT_S)
                        .color(crate::ui::theme::palette().cyan()),
                    );
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(crate::i18n::t("  Återställ  "))
                                    .size(Theme::FONT_S),
                            )
                            .stroke(egui::Stroke::new(1.0_f32, crate::ui::theme::palette().green())),
                        )
                        .on_hover_text(path)
                        .clicked()
                    {
                        restore = Some(i);
                    }
                });
            }

            ui.add_space(10.0);
            ui.separator();
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(crate::i18n::t(
                    "Autosparningarna ligger i ~/.local/state/sonix/autosave/ (5 senaste per projekt). En återställd kopia pensioneras dit utan att raderas.",
                ))
                .size(Theme::FONT_S)
                .color(crate::ui::theme::palette().text_muted()),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .button(crate::i18n::t("  Fortsätt utan att återställa  "))
                    .clicked()
                {
                    // Stäng bara: autosparningarna ligger kvar och kan
                    // öppnas manuellt — inget raderas åt användaren.
                    dismiss = true;
                }
            });
        });

    if let Some(i) = restore {
        self.restore_autosave(i);
    } else if !open || dismiss {
        self.show_recovery_modal = false;
    }
}
}

impl SonixApp {
pub(crate) fn render_about_modal(&mut self, ctx: &egui::Context) {
    if !self.show_about_modal {
        return;
    }

    let mut close = false;
    let mut open = self.show_about_modal;
    egui::Window::new(crate::i18n::t("ℹ Om Sonix Studio"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(460.0, 420.0))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                // Samma logga, större — och samma fallback.
                match self.logo_texture_for(ctx) {
                    Some(tex) => {
                        ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::Vec2::splat(96.0)));
                    }
                    None => {
                        ui.label(egui::RichText::new(crate::i18n::t("SONIX")).size(Theme::FONT_XL).color(crate::ui::theme::palette().cyan()));
                    }
                }
                ui.heading(egui::RichText::new(crate::i18n::t("SONIX STUDIO")).strong().size(Theme::FONT_XL).color(crate::ui::theme::palette().orange()));
                ui.label(egui::RichText::new(crate::i18n::t("Professionell Digital Audio Workstation & AI Musikstudio för Linux")).size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                ui.label(egui::RichText::new(crate::tstatus!("Version {} (PipeWire / ALSA / JACK Audio Engine)", env!("CARGO_PKG_VERSION"))).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                ui.label(egui::RichText::new(crate::i18n::t("🚀 Huvudfunktioner i Sonix Studio:")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(crate::i18n::t("• Realtids Multi-Track Audio Streaming & Mixmotor i Rust")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                ui.label(egui::RichText::new(crate::i18n::t("• Fullt stöd för Suno AI Stems med linjär vågformsredigering")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                ui.label(egui::RichText::new(crate::i18n::t("• 16-Stegs Sonix Channel Rack, Piano Roll & Touch Piano")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                ui.label(egui::RichText::new(crate::i18n::t("• Klippverktyg (Slice ✂), Fading & Dynamisk Gain-justering")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                ui.label(egui::RichText::new(crate::i18n::t("• 64-bit SIMD Audio DSP med Zero-Latency Process Thread")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));

                ui.add_space(16.0);
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("  Stäng  ")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().orange())).clicked() {
                    close = true;
                }
                ui.add_space(8.0);
            });
        });

    if close || !open {
        self.show_about_modal = false;
    }
}
}

impl SonixApp {
pub(crate) fn render_project_manager_modal(&mut self, ctx: &egui::Context) {
    if !self.show_project_manager_modal {
        return;
    }

    let mut close = false;
    let mut create_new = false;
    let mut load_demo = false;
    let mut show_suno = false;
    let mut save_name: Option<String> = None;
    let mut load_file: Option<String> = None;
    let mut open = self.show_project_manager_modal;

    egui::Window::new(crate::i18n::t("📁 Filhanterare & Projekt"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(580.0, 440.0))
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                if ui.button(egui::RichText::new(crate::i18n::t("📄 Nytt tomt projekt")).strong().color(crate::ui::theme::palette().cyan())).clicked() {
                    create_new = true;
                    close = true;
                }
                if ui.button(egui::RichText::new(crate::i18n::t("⚡ Ladda Demo-projekt")).strong().color(crate::ui::theme::palette().green())).clicked() {
                    load_demo = true;
                    close = true;
                }
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📂 Välj .sonix från datorn...")).strong().color(crate::ui::theme::palette().text())).fill(crate::ui::theme::palette().track_blue())).clicked() {
                    self.spawn_async_file_picker(
                        "Sonix-projekt",
                        &["sonix"],
                        crate::i18n::t("Välj Sonix Projektfil (.sonix)"),
                    );
                }
                if ui.button(egui::RichText::new(crate::i18n::t("🎼 Importera Suno ZIP...")).strong().color(crate::ui::theme::palette().orange())).clicked() {
                    show_suno = true;
                    close = true;
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Öppna från sökväg
            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("📂 ÖPPNA PROJEKT FRÅN SÖKVÄG")).strong().color(crate::ui::theme::palette().cyan()));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Sökväg:"));
                    ui.text_edit_singleline(&mut self.custom_project_path_input);
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("Öppna")).strong().color(Color32::BLACK)).fill(crate::ui::theme::palette().green())).clicked() {
                        let p = self.custom_project_path_input.trim().to_string();
                        if !p.is_empty() {
                            load_file = Some(p);
                        }
                    }
                });
            });

            ui.add_space(6.0);

            // Spara som
            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("💾 SPARA PROJEKT")).strong().color(crate::ui::theme::palette().orange()));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Projektnamn:"));
                    ui.text_edit_singleline(&mut self.new_project_name_input);
                    if ui.button(crate::i18n::t("Spara till disk")).clicked() {
                        save_name = Some(self.new_project_name_input.clone());
                    }
                });
            });

            ui.add_space(8.0);
            let projects_dir = crate::paths::paths().projects_dir();
            ui.label(
                egui::RichText::new(format!(
                    "{} {}",
                    crate::i18n::t("📂 SPARADE PROJEKT:"),
                    projects_dir.display()
                ))
                .strong()
                .color(crate::ui::theme::palette().cyan()),
            );
            let mut saved_files = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&projects_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("sonix")
                        && let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            saved_files.push((stem.to_string(), path.to_string_lossy().to_string()));
                        }
                }
            }

            if saved_files.is_empty() {
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("Inga sparade .sonix-projekt hittades ännu. Spara ditt nuvarande projekt ovan eller ladda ett demo-projekt!")).italics().color(crate::ui::theme::palette().text_muted()));
                });
            } else {
                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                    for (stem, full_path) in saved_files {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("🎵 {}", stem)).strong().color(crate::ui::theme::palette().text()));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("Öppna Projekt")).strong().color(Color32::BLACK)).fill(crate::ui::theme::palette().green())).clicked() {
                                        load_file = Some(full_path.clone());
                                    }
                                });
                            });
                        });
                        ui.add_space(2.0);
                    }
                });
            }

            ui.add_space(10.0);
            if ui.button(self.tr("Stäng")).clicked() {
                close = true;
            }
        });

    if create_new {
        self.new_empty_project();
    }
    if load_demo {
        self.load_demo_project();
    }
    if show_suno {
        self.show_suno_import_modal = true;
    }
    if let Some(name) = save_name {
        self.save_project(&name);
    }
    if let Some(path) = load_file {
        self.load_project_file(&path);
        close = true;
    }

    if close || !open {
        self.show_project_manager_modal = false;
    }
}
}
