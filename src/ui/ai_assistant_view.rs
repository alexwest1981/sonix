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
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("🤖 AI MUSIC PROMPT ENGINE & ASSISTANT (Suno & Local AI)").strong().size(14.0).color(Theme::FL_CYAN));
            ui.separator();
            ui.label(egui::RichText::new("Generera MIDI-slingor, basgångar, ackordföljder och trumspår direkt via naturligt språk").size(11.0).color(Theme::TEXT_MUTED));
        });

        ui.add_space(8.0);

        // Prompt Input & Generation Box
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("✍ SKRIV MUSIKPROMPT (SVENSKA ELLER ENGELSKA):").strong().size(11.0).color(Theme::FL_ORANGE));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    ui.text_edit_singleline(&mut ai_assistant.prompt_input);
                    if ui.add(egui::Button::new(egui::RichText::new("✨ Generera AI-Musik").strong().size(12.0).color(Color32::BLACK)).fill(Theme::FL_YELLOW)).clicked() {
                        ai_assistant.generate_from_prompt();
                        *status_msg = ai_assistant.last_status.clone();
                    }
                });

                ui.add_space(6.0);

                // Quick Prompt Inspiration Chips
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("Inspiration:").size(10.5).color(Theme::TEXT_MUTED));
                    let prompts = [
                        "80s Synthwave bassline i A-moll",
                        "Neo-Soul varma 9th ackord",
                        "303 Acid resonant slide",
                        "Mörk Cyberpunk 808 basgång",
                        "Funky disco basgång med syncopation",
                    ];
                    for p in prompts {
                        if ui.button(p).clicked() {
                            ai_assistant.prompt_input = p.to_string();
                            ai_assistant.generate_from_prompt();
                            *status_msg = ai_assistant.last_status.clone();
                        }
                    }
                });
            });
        });

        ui.add_space(8.0);

        // Generated AI Clips List
        ui.group(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("🎵 GENERERADE AI-KLIPP & PATTERNS").strong().size(12.0).color(Theme::FL_GREEN));
                ui.separator();
                ui.label(egui::RichText::new("Applicera direkt på synt, bas eller trumspår med ett klick").size(10.5).color(Theme::TEXT_MUTED));
            });

            ui.add_space(4.0);

            for clip in &ai_assistant.generated_history {
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new(&clip.title).strong().size(12.0).color(Color32::WHITE));
                            ui.label(egui::RichText::new(format!("Genre: {}  •  Tonart: {}  •  BPM: {:.0}", clip.genre, clip.key_signature, clip.bpm)).size(10.0).color(Theme::TEXT_MUTED));
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Apply to Bass (Channel 7)
                            if ui.add(egui::Button::new(egui::RichText::new("📥 Applicera på 🎸 Sub Bass").strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_ORANGE)).clicked()
                                && channels.len() > 7 {
                                    channels[7].steps = clip.channel_steps;
                                    channels[7].notes = clip.notes;
                                    *status_msg = format!("✔ Applicerade \"{}\" på Sub Bass (Kanal 8)!", clip.title);
                                }

                            // Apply to Lead (Channel 6)
                            if ui.add(egui::Button::new(egui::RichText::new("📥 Applicera på 🎹 303 Lead").strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_CYAN)).clicked()
                                && channels.len() > 6 {
                                    channels[6].steps = clip.channel_steps;
                                    channels[6].notes = clip.notes;
                                    *status_msg = format!("✔ Applicerade \"{}\" på 303 Lead (Kanal 7)!", clip.title);
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
