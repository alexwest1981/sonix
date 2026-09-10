use eframe::egui::{self, Color32, Ui};
use crate::audio::ai_generator::AiMusicAssistant;
use crate::ui::app::ChannelStrip;
use crate::ui::theme::Theme;

pub fn render_ai_assistant_view(
    ui: &mut Ui,
    ai_assistant: &mut AiMusicAssistant,
    channels: &mut [ChannelStrip],
    status_msg: &mut String,
) {
    if let Some(msg) = ai_assistant.poll_remote_generation() {
        *status_msg = msg;
    }
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🤖 AI MUSIC PROMPT ENGINE & ASSISTANT (Suno & Local AI)")).strong().size(14.0).color(Theme::FL_CYAN));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Generera MIDI-slingor, basgångar, ackordföljder och trumspår direkt via naturligt språk")).size(11.0).color(Theme::TEXT_MUTED));
        });

        ui.add_space(8.0);

        // Prompt Input & Generation Box
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("✍ SKRIV MUSIKPROMPT (SVENSKA ELLER ENGELSKA):")).strong().size(11.0).color(Theme::FL_ORANGE));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut ai_assistant.prompt_input);
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✨ Generera AI-Musik")).strong().size(12.0).color(Color32::BLACK)).fill(Theme::FL_YELLOW)).clicked() {
                        ai_assistant.generate();
                        *status_msg = ai_assistant.last_status.clone();
                    }
                    if ai_assistant.is_generating {
                        ui.spinner();
                        ui.label(egui::RichText::new(crate::i18n::t("⏳ Genererar via AI-API...")).size(10.5).color(Theme::FL_CYAN));
                    }
                });

                ui.add_space(6.0);

                // Quick Prompt Inspiration Chips
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("Inspiration:")).size(10.5).color(Theme::TEXT_MUTED));
                    let prompts = [
                        crate::i18n::t("80s Synthwave bassline i A-moll"),
                        crate::i18n::t("Neo-Soul varma 9th ackord"),
                        crate::i18n::t("303 Acid resonant slide"),
                        crate::i18n::t("Mörk Cyberpunk 808 basgång"),
                        crate::i18n::t("Funky disco basgång med syncopation"),
                    ];
                    for p in prompts {
                        if ui.button(p).clicked() {
                            ai_assistant.prompt_input = p.to_string();
                            ai_assistant.generate();
                            *status_msg = ai_assistant.last_status.clone();
                        }
                    }
                });
            });
        });

        ui.add_space(4.0);

        // Project context (Etapp D): grounds generation in key/tempo/selection.
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut ai_assistant.use_context, crate::i18n::t("🎯 Använd projektkontext"));
            if ai_assistant.use_context {
                let c = &ai_assistant.context;
                let mut bits: Vec<String> = Vec::new();
                if !c.project_name.trim().is_empty() {
                    bits.push(c.project_name.clone());
                }
                if c.bpm > 0.0 {
                    bits.push(format!("{:.0} BPM", c.bpm));
                }
                if !c.key_label.trim().is_empty() {
                    bits.push(c.key_label.clone());
                }
                if !c.selected_track.trim().is_empty() {
                    bits.push(c.selected_track.clone());
                }
                if let Some(region) = &c.selected_region {
                    bits.push(region.clone());
                }
                if !bits.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("🎯 {}", bits.join(" · ")))
                            .size(10.0)
                            .color(Theme::TEXT_MUTED),
                    );
                }
            }
        });

        ui.add_space(8.0);

        // Optional remote AI-API configuration. Without it the local
        // rule-based engine is used (offline-first).
        egui::CollapsingHeader::new(crate::i18n::t("⚙ AI-API-inställningar (valfritt – annars lokal motor)"))
            .default_open(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Provider:"));
                    egui::ComboBox::from_id_salt("ai_provider_combo")
                        .selected_text(ai_assistant.config.provider.label())
                        .show_ui(ui, |ui| {
                            for p in crate::audio::ai_client::AiProvider::ALL {
                                ui.selectable_value(&mut ai_assistant.config.provider, p, p.label());
                            }
                        });
                    if ui.button(crate::i18n::t("Återställ standard")).clicked() {
                        ai_assistant.config.base_url.clear();
                        ai_assistant.config.model.clear();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Bas-URL:"));
                    ui.add(egui::TextEdit::singleline(&mut ai_assistant.config.base_url).desired_width(320.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Modell:"));
                    let models = ai_assistant.config.provider.known_models();
                    if !models.is_empty() {
                        let selected = if ai_assistant.config.model.trim().is_empty() {
                            crate::i18n::t("(standard)").to_string()
                        } else {
                            ai_assistant.config.model.clone()
                        };
                        egui::ComboBox::from_id_salt("ai_model_combo")
                            .selected_text(selected)
                            .show_ui(ui, |ui| {
                                for m in models {
                                    if ui
                                        .selectable_label(ai_assistant.config.model == *m, *m)
                                        .clicked()
                                    {
                                        ai_assistant.config.model = (*m).to_string();
                                    }
                                }
                            });
                    }
                    ui.add(egui::TextEdit::singleline(&mut ai_assistant.config.model).desired_width(220.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("API-nyckel:"));
                    ui.add(egui::TextEdit::singleline(&mut ai_assistant.config.api_key).password(true).desired_width(320.0));
                });
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::t("🎧 AI-ljudgenerering (sparas i samma fil)")).strong().size(11.0).color(Theme::FL_CYAN));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Ljud-provider:"));
                    egui::ComboBox::from_id_salt("ai_audio_provider_combo")
                        .selected_text(ai_assistant.config.audio_provider.label())
                        .show_ui(ui, |ui| {
                            for p in crate::audio::ai_client::AudioProvider::ALL {
                                ui.selectable_value(&mut ai_assistant.config.audio_provider, p, p.label());
                            }
                        });
                    if ui.button(crate::i18n::t("Återställ standard")).clicked() {
                        ai_assistant.config.audio_base_url.clear();
                        ai_assistant.config.audio_model.clear();
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Bas-URL:"));
                    ui.add(egui::TextEdit::singleline(&mut ai_assistant.config.audio_base_url).desired_width(320.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Modell:"));
                    ui.add(egui::TextEdit::singleline(&mut ai_assistant.config.audio_model).desired_width(320.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("API-nyckel:"));
                    ui.add(egui::TextEdit::singleline(&mut ai_assistant.config.audio_api_key).password(true).desired_width(320.0));
                });
                let audio_ready = ai_assistant.config.audio_is_ready();
                ui.label(
                    egui::RichText::new(if audio_ready {
                        crate::i18n::t("Redo: använder AI-ljud-API")
                    } else {
                        crate::i18n::t("Offline: använder lokal DSP")
                    })
                    .size(10.5)
                    .color(if audio_ready { Theme::FL_GREEN } else { Theme::TEXT_MUTED }),
                );
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::t("💾 Spara AI-konfiguration")).clicked() {
                        match ai_assistant.config.save() {
                            Ok(path) => *status_msg = crate::tstatus!("✔ Sparade AI-konfiguration till {}", path.display()),
                            Err(e) => *status_msg = crate::tstatus!("⚠ Kunde inte spara AI-konfiguration: {}", e),
                        }
                    }
                    let ready = ai_assistant.config.is_ready();
                    ui.label(
                        egui::RichText::new(if ready {
                            crate::i18n::t("Redo: använder AI-API")
                        } else {
                            crate::i18n::t("Offline: använder lokal regelbaserad motor")
                        })
                        .size(10.5)
                        .color(if ready { Theme::FL_GREEN } else { Theme::TEXT_MUTED }),
                    );
                });
                if let Some(err) = &ai_assistant.last_error {
                    ui.label(egui::RichText::new(format!("⚠ {err}")).size(10.5).color(Theme::FL_RED));
                }
            });

        ui.add_space(8.0);

        // Generated AI Clips List
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🎵 GENERERADE AI-KLIPP & PATTERNS")).strong().size(12.0).color(Theme::FL_GREEN));
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::t("Applicera direkt på synt, bas eller trumspår med ett klick")).size(10.5).color(Theme::TEXT_MUTED));
            });

            ui.add_space(4.0);

            for clip in &ai_assistant.generated_history {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&clip.title).strong().size(12.0).color(Color32::WHITE));
                            ui.label(egui::RichText::new(format!("{} {}  •  {} {}  •  BPM: {:.0}", crate::i18n::t("Genre:"), clip.genre, crate::i18n::t("Tonart:"), clip.key_signature, clip.bpm)).size(10.0).color(Theme::TEXT_MUTED));
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Apply to Bass (Channel 7)
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📥 Applicera på 🎸 Sub Bass")).strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_ORANGE)).clicked()
                                && channels.len() > 7 {
                                    channels[7].steps = clip.channel_steps;
                                    channels[7].notes = clip.notes;
                                    *status_msg = crate::tstatus!("✔ Applicerade \"{}\" på Sub Bass (Kanal 8)!", clip.title);
                                }

                            // Apply to Lead (Channel 6)
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📥 Applicera på 🎹 303 Lead")).strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_CYAN)).clicked()
                                && channels.len() > 6 {
                                    channels[6].steps = clip.channel_steps;
                                    channels[6].notes = clip.notes;
                                    *status_msg = crate::tstatus!("✔ Applicerade \"{}\" på 303 Lead (Kanal 7)!", clip.title);
                                }

                            // Step count badge
                            let active_steps = clip.channel_steps.iter().filter(|&&s| s).count();
                            ui.label(egui::RichText::new(format!("{} steg", active_steps)).size(10.5).color(Theme::FL_YELLOW));
                        });
                    });
                });
                ui.add_space(2.0);
            }
        });
    });
}
