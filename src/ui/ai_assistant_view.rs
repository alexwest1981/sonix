use eframe::egui::{self, Color32, Ui};
use crate::audio::ai_generator::AiMusicAssistant;
use crate::ui::app::ChannelStrip;
use crate::ui::theme::Theme;

/// Vad vyn vill att appen ska göra. Vyn äger inget av det själv — den flaggar bara.
#[derive(Default)]
pub struct AiViewAction {
    pub ask: bool,
    pub apply_plan: bool,
    /// Generera **ljud** (Suno/TTS) — flyttad hit från arrangörens kapsel, som togs bort
    /// tillsammans med den överflödiga kromen.
    pub generate_audio: bool,
    /// Open the Suno panel (sign in, prompt, create — stems land on their own channels).
    pub open_suno: bool,
}

pub fn render_ai_assistant_view(
    ui: &mut Ui,
    ai_assistant: &mut AiMusicAssistant,
    channels: &mut [ChannelStrip],
    status_msg: &mut String,
    plan_pending: bool,
) -> AiViewAction {
    let mut action = AiViewAction::default();
    if let Some(msg) = ai_assistant.poll_remote_generation() {
        *status_msg = msg;
    }
    // **Allt ska gå att nå.** Vyn var en lång stapel utan scroll: vid ett normalt fönster
    // försvann nederdelen och inget svarade på hjulet. ScrollArea gör höjden tillgänglig i
    // stället för att klippa den.
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
    ui.group(|ui| {
        // Panelen fyller höjden: annars blir nedre halvan en tom rand under listan.
        ui.set_min_height(ui.available_height());
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

                // Fältet äger hela bredden och knapparna ligger under det: förut klämdes fem
                // knappar in till höger om ett halvtomt fält.
                ui.add(
                    egui::TextEdit::singleline(&mut ai_assistant.prompt_input)
                        .desired_width(ui.available_width() - 8.0)
                        .hint_text(crate::i18n::t("Beskriv låten, slingan eller ljudet")),
                );
                ui.horizontal_wrapped(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✨ Generera AI-Musik")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().yellow())).clicked() {
                        ai_assistant.generate();
                        *status_msg = ai_assistant.last_status.clone();
                    }
                    // **Assistenten** går via verktygslagret (ai_tools): den kan sätta tempo, skriva
                    // ett trumkomp och föreslå sektioner. Vyn äger inget av det — den flaggar bara.
                    if plan_pending && ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✅ Skapa sektionerna")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().green())).clicked() {
                        action.apply_plan = true;
                    }
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🤖 Kör assistenten")).strong().size(Theme::FONT_S).color(Color32::BLACK)).fill(crate::ui::theme::palette().cyan())).clicked() {
                        action.ask = true;
                    }
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎧 Generera ljud")).strong().size(Theme::FONT_S)).fill(crate::ui::theme::palette().purple())).clicked() {
                        action.generate_audio = true;
                    }
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("☀ Suno: sign in & create stems")).strong().size(Theme::FONT_S)).fill(crate::ui::theme::palette().orange())).clicked() {
                        action.open_suno = true;
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

            for (index, clip) in ai_assistant.generated_history.iter().enumerate() {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&clip.title).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text()));
                            ui.label(egui::RichText::new(format!("{} {}  •  {} {}  •  BPM: {:.0}", crate::i18n::t("Genre:"), clip.genre, crate::i18n::t("Tonart:"), clip.key_signature, clip.bpm)).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // **Målkanalen väljs, den gissas inte.** Förut fanns två fasta
                            // knappar (Lead/Sub Bass) för varje klipp, även för en trap-808:a
                            // eller en ackordmelodi. Nu finns en rullgardin med kanalernas egna
                            // namn, och standardvalet följer klippets genre.
                            let default = target_channel(&clip.genre, channels);
                            let id = egui::Id::new(("ai_clip_target", index));
                            let mut target: usize =
                                ui.data_mut(|d| *d.get_temp_mut_or(id, default));
                            if target >= channels.len() {
                                target = default;
                            }
                            let label = channels
                                .get(target)
                                .map(|c| c.name.clone())
                                .unwrap_or_else(|| crate::i18n::t("Ingen kanal").to_string());
                            egui::ComboBox::from_id_salt(id.with("combo"))
                                .selected_text(label)
                                .show_ui(ui, |ui| {
                                    for (ci, channel) in channels.iter().enumerate() {
                                        ui.selectable_value(&mut target, ci, channel.name.clone());
                                    }
                                });
                            ui.data_mut(|d| d.insert_temp(id, target));
                            if ui
                                .add(egui::Button::new(
                                    egui::RichText::new(crate::i18n::t("📥 Applicera"))
                                        .strong()
                                        .size(Theme::FONT_S)
                                        .color(Color32::BLACK),
                                )
                                .fill(crate::ui::theme::palette().orange()))
                                .clicked()
                                && let Some(channel) = channels.get_mut(target)
                            {
                                channel.steps[..16].copy_from_slice(&clip.channel_steps);
                                channel.notes[..16].copy_from_slice(&clip.notes);
                                *status_msg = crate::tstatus!(
                                    "✔ \"{}\" → {}",
                                    clip.title,
                                    channel.name
                                );
                            }
                            // Tempot i klippet kan skilja sig från projektets. Då står det här i
                            // stället för att se ut som om det stämmer.
                            let project_bpm = ai_assistant.context.bpm;
                            if project_bpm > 0.0 && (clip.bpm - project_bpm).abs() > 0.5 {
                                ui.label(
                                    egui::RichText::new(format!(
                                        "⚠ {:.0} BPM ≠ {:.0}",
                                        clip.bpm, project_bpm
                                    ))
                                    .size(Theme::FONT_S)
                                    .color(crate::ui::theme::palette().yellow()),
                                );
                            }
                            let active_steps = clip.channel_steps.iter().filter(|&&s| s).count();
                            ui.label(
                                egui::RichText::new(format!("{active_steps}"))
                                    .size(Theme::FONT_S)
                                    .color(crate::ui::theme::palette().yellow()),
                            );                        });
                    });
                });
                ui.add_space(2.0);
            }
        });
    });
    });
    action
}

/// **Klippets egen genre pekar ut kanalen.** En 808-bas hör inte hemma på en lead-kanal, så
/// standardvalet följer genren och användaren kan sedan välja fritt i rullgardinen.
fn target_channel(genre: &str, channels: &[ChannelStrip]) -> usize {
    let genre = genre.to_lowercase();
    let wanted: &[&str] = if genre.contains("bass") || genre.contains("808") || genre.contains("bas") {
        &["bass", "bas", "sub"]
    } else if genre.contains("drum") || genre.contains("trap") || genre.contains("drill") || genre.contains("groove") {
        &["drum", "trum", "kick", "perc"]
    } else if genre.contains("chord") || genre.contains("pad") || genre.contains("keys") {
        &["keys", "piano", "pad", "chord"]
    } else {
        &["lead", "synth", "melodi"]
    };
    for (index, channel) in channels.iter().enumerate() {
        let name = channel.name.to_lowercase();
        if wanted.iter().any(|w| name.contains(w)) {
            return index;
        }
    }
    0
}
