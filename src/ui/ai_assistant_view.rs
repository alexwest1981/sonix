use eframe::egui::{self, Color32, Ui};
use crate::audio::ai_generator::AiMusicAssistant;
use crate::ui::app::ChannelStrip;
use crate::ui::theme::Theme;

pub fn render_ai_assistant_view(
    ui: &mut Ui,
    ai_assistant: &mut AiMusicAssistant,
    channels: &mut [ChannelStrip],
    status_msg: &mut String,
) -> bool {
    let mut asked = false;
    if let Some(msg) = ai_assistant.poll_remote_generation() {
        *status_msg = msg;
    }
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🤖 AI MUSIC PROMPT ENGINE & ASSISTANT (Suno & Local AI)")).strong().size(Theme::FONT_L).color(crate::ui::theme::palette().cyan()));
            ui.separator();
            ui.label(egui::RichText::new(crate::i18n::t("Generera MIDI-slingor, basgångar, ackordföljder och trumspår direkt via naturligt språk")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
        });

        ui.add_space(8.0);

        // Prompt Input & Generation Box
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("✍ SKRIV MUSIKPROMPT (SVENSKA ELLER ENGELSKA):")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut ai_assistant.prompt_input);
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✨ Generera AI-Musik")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().yellow())).clicked() {
                        ai_assistant.generate();
                        *status_msg = ai_assistant.last_status.clone();
                    }
                    // **Assistenten** går via verktygslagret (ai_tools): den kan sätta tempo, skriva
                    // ett trumkomp och föreslå sektioner. Vyn äger inget av det — den flaggar bara.
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🤖 Kör assistenten")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().cyan())).clicked() {
                        asked = true;
                    }
                    if ai_assistant.is_generating {
                        ui.spinner();
                        ui.label(egui::RichText::new(crate::i18n::t("⏳ Genererar via AI-API...")).size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
                    }
                });

                ui.add_space(6.0);

                // Quick Prompt Inspiration Chips
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("Inspiration:")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
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

        if !ai_assistant.last_reply.trim().is_empty() {
            ui.label(
                egui::RichText::new(&ai_assistant.last_reply)
                    .size(Theme::FONT_S)
                    .color(crate::ui::theme::palette().cyan()),
            );
        }

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
                            .size(Theme::FONT_S)
                            .color(crate::ui::theme::palette().text_muted()),
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
                ui.label(egui::RichText::new(crate::i18n::t("🎧 AI-ljudgenerering (sparas i samma fil)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
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
                    .size(Theme::FONT_S)
                    .color(if audio_ready { crate::ui::theme::palette().green() } else { crate::ui::theme::palette().text_muted() }),
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
                        .size(Theme::FONT_S)
                        .color(if ready { crate::ui::theme::palette().green() } else { crate::ui::theme::palette().text_muted() }),
                    );
                });
                if let Some(err) = &ai_assistant.last_error {
                    ui.label(egui::RichText::new(format!("⚠ {err}")).size(Theme::FONT_S).color(crate::ui::theme::palette().red()));
                }
            });

        ui.add_space(8.0);

        // Generated AI Clips List
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🎵 GENERERADE AI-KLIPP & PATTERNS")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::t("Applicera direkt på synt, bas eller trumspår med ett klick")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
            });

            ui.add_space(4.0);

            for clip in &ai_assistant.generated_history {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&clip.title).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                            ui.label(egui::RichText::new(format!("{} {}  •  {} {}  •  BPM: {:.0}", crate::i18n::t("Genre:"), clip.genre, crate::i18n::t("Tonart:"), clip.key_signature, clip.bpm)).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Apply to Bass (Channel 7)
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📥 Applicera på 🎸 Sub Bass")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().orange())).clicked()
                                && channels.len() > 7 {
                                    channels[7].steps[..16].copy_from_slice(&clip.channel_steps);
                                    channels[7].notes[..16].copy_from_slice(&clip.notes);
                                    *status_msg = crate::tstatus!("✔ Applicerade \"{}\" på Sub Bass (Kanal 8)!", clip.title);
                                }

                            // Apply to Lead (Channel 6)
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📥 Applicera på 🎹 303 Lead")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().cyan())).clicked()
                                && channels.len() > 6 {
                                    channels[6].steps[..16].copy_from_slice(&clip.channel_steps);
                                    channels[6].notes[..16].copy_from_slice(&clip.notes);
                                    *status_msg = crate::tstatus!("✔ Applicerade \"{}\" på 303 Lead (Kanal 7)!", clip.title);
                                }

                            // Step count badge
                            let active_steps = clip.channel_steps.iter().filter(|&&s| s).count();
                            ui.label(egui::RichText::new(format!("{} steg", active_steps)).size(Theme::FONT_S).color(crate::ui::theme::palette().yellow()));
                        });
                    });
                });
                ui.add_space(2.0);
            }
        });
    });
    asked
}
