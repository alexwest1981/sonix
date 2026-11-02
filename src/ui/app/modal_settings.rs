//! Inställningsdialoger: AI, ljud och mikrofon.
//!
//! Utdelad ur `modals.rs` 2026-11-02; filen var 4 486 rader och höll 34 dialoger.

use super::*;
use crate::ui::theme::Theme;

impl SonixApp {
pub(crate) fn render_ai_settings_modal(&mut self, ctx: &egui::Context) {
    if !self.show_ai_settings_modal {
        return;
    }

    let mut close = false;
    let mut open = self.show_ai_settings_modal;
    egui::Window::new(crate::i18n::t("🤖 AI-inställningar"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(560.0, 480.0))
        .show(ctx, |ui| {
            ui.label(crate::i18n::t("Samma konfiguration som i AI Music Studio – sparas till ~/.config/sonix/ai.json."));
            ui.add_space(8.0);

            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("💬 Textgenerering (AI Co-Producer)")).strong().color(crate::ui::theme::palette().green()));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Provider:"));
                    egui::ComboBox::from_id_salt("legacy_ai_provider_combo")
                        .selected_text(self.ai_assistant.config.provider.label())
                        .show_ui(ui, |ui| {
                            for p in crate::audio::ai_client::AiProvider::ALL {
                                ui.selectable_value(&mut self.ai_assistant.config.provider, p, p.label());
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Bas-URL:"));
                    ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.base_url).desired_width(340.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Modell:"));
                    ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.model).desired_width(340.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("API-nyckel:"));
                    ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.api_key).password(true).desired_width(340.0));
                });
            });

            ui.add_space(4.0);
            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("🎧 AI-ljudgenerering (Stable Audio m.fl.)")).strong().color(crate::ui::theme::palette().cyan()));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Ljud-provider:"));
                    egui::ComboBox::from_id_salt("legacy_ai_audio_provider_combo")
                        .selected_text(self.ai_assistant.config.audio_provider.label())
                        .show_ui(ui, |ui| {
                            for p in crate::audio::ai_client::AudioProvider::ALL {
                                ui.selectable_value(&mut self.ai_assistant.config.audio_provider, p, p.label());
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Bas-URL:"));
                    ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.audio_base_url).desired_width(340.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Modell:"));
                    ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.audio_model).desired_width(340.0).hint_text(crate::i18n::t("(tomt = providers standard)")));
                });
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("API-nyckel:"));
                    ui.add(egui::TextEdit::singleline(&mut self.ai_assistant.config.audio_api_key).password(true).desired_width(340.0));
                });
            });

            ui.add_space(6.0);
            let ready = self.ai_assistant.config.is_ready();
            ui.label(
                egui::RichText::new(if ready {
                    crate::i18n::t("Redo: använder AI-API")
                } else {
                    crate::i18n::t("Offline: använder lokal regelbaserad motor")
                })
                .size(Theme::FONT_S)
                .color(if ready { crate::ui::theme::palette().green() } else { crate::ui::theme::palette().text_muted() }),
            );

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("💾 Spara inställningar")).strong().color(Color32::BLACK)).fill(crate::ui::theme::palette().green())).clicked() {
                    match self.ai_assistant.config.save() {
                        Ok(path) => self.status_message = crate::tstatus!("✔ Sparade AI-inställningar till {}", path.display()),
                        Err(e) => self.status_message = crate::tstatus!("⚠ Kunde inte spara AI-inställningar: {}", e),
                    }
                    close = true;
                }
                if ui.button(crate::i18n::t("Avbryt")).clicked() {
                    close = true;
                }
            });
        });

    if close || !open {
        self.show_ai_settings_modal = false;
    }
}
}

impl SonixApp {
pub(crate) fn render_audio_settings_modal(&mut self, ctx: &egui::Context) {
    if !self.show_audio_settings_modal {
        return;
    }

    let mut close = false;
    let mut open = self.show_audio_settings_modal;
    egui::Window::new(crate::i18n::t("🎛 Ljud- & MIDI-inställningar"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(500.0, 380.0))
        .show(ctx, |ui| {
            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("Ljudmotor & Drivrutiner")).strong().color(crate::ui::theme::palette().cyan()));
                ui.label(crate::tstatus!("Värd: {} · Enhet: {}", self.engine.host_name, self.engine.device_name));
                let buffer_txt = self.engine.buffer_frames
                    .map(|f| format!("{f} frames"))
                    .unwrap_or_else(|| crate::i18n::t("enhetens standard").to_string());
                ui.label(egui::RichText::new(crate::tstatus!(
                    "Aktiv ström: {} Hz · {} · {} kanaler",
                    self.engine.sample_rate,
                    buffer_txt,
                    self.engine.channels
                )).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label(self.tr("Samplingsfrekvens:"));
                    let rates = ["44.1 kHz", "48.0 kHz", "96.0 kHz"];
                    for (i, rate) in rates.iter().enumerate() {
                        if ui.selectable_label(self.audio_sample_rate_idx == i, *rate).clicked() {
                            self.audio_sample_rate_idx = i;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Bufferstorlek:"));
                    let buffers = ["128 (2.9ms)", "256 (5.8ms)", "512 (11.6ms)", "1024 (23.2ms)"];
                    for (i, buf) in buffers.iter().enumerate() {
                        if ui.selectable_label(self.audio_buffer_size_idx == i, *buf).clicked() {
                            self.audio_buffer_size_idx = i;
                        }
                    }
                });

                ui.add_space(6.0);
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁 Tillämpa på ljudströmmen")).strong().color(Color32::BLACK)).fill(crate::ui::theme::palette().green())).clicked() {
                    let rates = [44100_u32, 48000, 96000];
                    let buffers = [128_u32, 256, 512, 1024];
                    let requested_rate = rates[self.audio_sample_rate_idx.min(rates.len() - 1)];
                    let requested_buffer = buffers[self.audio_buffer_size_idx.min(buffers.len() - 1)];
                    match self.engine.reconfigure(Some(requested_rate), Some(requested_buffer)) {
                        Ok(()) => {
                            let actual_rate = self.engine.sample_rate;
                            let actual_buffer = self.engine.buffer_frames;
                            let settings = AudioSettings {
                                sample_rate: Some(actual_rate),
                                buffer_frames: actual_buffer,
                                output_buses: self.audio_output_buses.clone(),
                            };
                            let save_note = match settings.save() {
                                Ok(_) => String::new(),
                                Err(e) => crate::tstatus!(" (kunde inte spara: {})", e),
                            };
                            self.audio_sample_rate_idx = match actual_rate { 44100 => 0, 96000 => 2, _ => 1 };
                            self.audio_buffer_size_idx = match actual_buffer { Some(128) => 0, Some(512) => 2, Some(1024) => 3, _ => 1 };
                            self.resync_engine_after_reconfigure();
                            let buf_txt = actual_buffer.map(|f| format!("{f} frames")).unwrap_or_else(|| crate::i18n::t("enhetens standard").to_string());
                            self.status_message = crate::tstatus!(
                                "🔁 Ljudströmmen omstartad: {} Hz · {}{}",
                                actual_rate, buf_txt, save_note
                            );
                            close = true;
                        }
                        Err(e) => {
                            self.status_message = crate::tstatus!("⚠ Kunde inte byta ljudkonfiguration: {}", e);
                        }
                    }
                }
            });

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(crate::tstatus!(
                        "Stämning: {}",
                        crate::audio::tuning::current_name()
                    ))
                    .size(Theme::FONT_S)
                    .color(crate::ui::theme::palette().text_muted()),
                );
                if ui.button(crate::i18n::t("🎼 Byt stämning (.ascl)...")).clicked() {
                    self.load_tuning_from_file();
                }
                if ui.button(crate::i18n::t("🎼 12-TET")).clicked() {
                    self.reset_tuning();
                }
            });
            ui.add_space(8.0);
            ui.group(|ui| {
                let channels = self.engine.channels.max(2) as usize;
                ui.label(
                    egui::RichText::new(crate::tstatus!("Utgångsbussar ({} kanaler på enheten)", channels))
                        .strong()
                        .color(crate::ui::theme::palette().cyan()),
                );
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Varje buss är ett kanalpar. Bussar utanför enhetens kanalantal används inte.",
                    ))
                    .size(Theme::FONT_S)
                    .color(crate::ui::theme::palette().text_muted()),
                );
                let mut remove: Option<usize> = None;
                for (i, bus) in self.audio_output_buses.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut bus.enabled, "");
                        ui.add(egui::TextEdit::singleline(&mut bus.name).desired_width(110.0));
                        ui.label(crate::i18n::t("vänster"));
                        ui.add(egui::DragValue::new(&mut bus.left).range(0..=(channels.saturating_sub(1))));
                        ui.label(crate::i18n::t("höger"));
                        ui.add(egui::DragValue::new(&mut bus.right).range(0..=(channels.saturating_sub(1))));
                        if ui.small_button("✖").clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    self.audio_output_buses.remove(i);
                }
                if ui.button(crate::i18n::t("➕ Lägg till buss")).clicked() {
                    let n = self.audio_output_buses.len();
                    let left = (n * 2).min(channels.saturating_sub(2));
                    self.audio_output_buses.push(crate::audio::engine::OutputBus::new(
                        "Buss",
                        left,
                        (left + 1).min(channels.saturating_sub(1)),
                    ));
                }
            });

            ui.add_space(10.0);
            if ui.button(crate::i18n::t("Stäng")).clicked() {
                close = true;
            }
        });

    if close || !open {
        self.show_audio_settings_modal = false;
    }
}
}

impl SonixApp {
pub(crate) fn render_mic_settings_modal(&mut self, ctx: &egui::Context) {
    if !self.show_mic_settings_modal {
        return;
    }

    let mut close = false;
    let mut open = self.show_mic_settings_modal;
    egui::Window::new(crate::i18n::t("🎙 Mikrofonjustering & Enhetsinställningar (Microphone Panel)"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(560.0, 480.0))
        .show(ctx, |ui| {
            ui.vertical(|ui| {
                // Group 1: Device Selection & Host Audio Stream
                ui.group(|ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Ljudenhet & Mikrofonkälla")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().cyan()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button(crate::i18n::t("🔄 Uppdatera enheter")).on_hover_text(crate::i18n::t("Sök efter anslutna USB- och hårdvarumikrofoner")).clicked() {
                                let devs = LiveMicrophoneCapture::list_devices();
                                self.vocal_studio.mic_settings.available_devices = devs;
                            }
                        });
                    });
                    ui.add_space(4.0);

                    let dev_list = self.vocal_studio.mic_settings.available_devices.clone();
                    let mut current_idx = self.vocal_studio.mic_settings.selected_device_idx;

                    // Ingångarna klassas (Sprint 1, punkt 1): en gitarrkabel ska heta gitarrkabel.
                    // Klassningen läser USB-id ur sysfs — billigt nog att göra när rutan är öppen.
                    let cards = crate::audio::input_profile::system_cards();
                    let inputs = crate::audio::input_profile::resolve_inputs(&dev_list, &cards);

                    egui::ComboBox::from_label(crate::i18n::t("Välj Mikrofon"))
                        .selected_text(dev_list.get(current_idx).cloned().unwrap_or_else(|| crate::i18n::t("Standardmikrofon").to_string()))
                        .width(360.0)
                        .show_ui(ui, |ui| {
                            for (idx, dev_name) in dev_list.iter().enumerate() {
                                let label = match inputs.get(idx) {
                                    Some(input) if input.kind != crate::audio::input_profile::InputKind::Unknown => {
                                        format!("{} — {}", crate::i18n::t(input.kind.badge()), input.label)
                                    }
                                    _ => format!("🎙 {}", dev_name),
                                };
                                if ui.selectable_value(&mut current_idx, idx, label).clicked() {
                                    let switched = self.vocal_studio.select_microphone(idx);
                                    if switched {
                                        self.status_message = crate::tstatus!("✔ Mikrofon ändrad till: {}", dev_name);
                                    }
                                }
                            }
                        });

                    // En känd instrumentingång får en egen rad: ett klick gör den redo —
                    // samma väg som automatiken vid start använder.
                    if let Some(instrument) = crate::audio::input_profile::first_instrument(&inputs) {
                        let device = instrument.device_name.clone();
                        let label = instrument.label;
                        let note = instrument.note;
                        let kind = instrument.kind;
                        let mut apply = false;
                        ui.horizontal(|ui| {
                            apply = ui
                                .add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎸 Instrumentläge")).strong()).fill(crate::ui::theme::palette().orange()))
                                .on_hover_text(crate::tstatus!("{} — {}", device, note))
                                .clicked();
                            ui.label(egui::RichText::new(crate::tstatus!("{} {} hittad", crate::i18n::t(kind.badge()), label)).size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                        });
                        if apply {
                            let text = self.apply_instrument_plan(instrument);
                            self.status_message = text;
                        }
                    }

                    let active_dev = self.vocal_studio.mic_capture.as_ref().map(|m| m.device_name.as_str()).unwrap_or("Standard");
                    let sr = self.vocal_studio.mic_capture.as_ref().map(|m| m.sample_rate).unwrap_or(44100);
                    ui.label(egui::RichText::new(crate::tstatus!("🟢 Aktiv ström: {} • {} Hz 32-bit Float", active_dev, sr)).size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                });

                ui.add_space(6.0);

                // Group 2: Real-time VU & Level Meter with Gain
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("Ingångsnivå & Förförstärkare (Gain & VU)")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().orange()));
                    ui.add_space(4.0);

                    let vu = self.vocal_studio.mic_vu_level;
                    let vu_db = if vu > 0.0001 {
                        (20.0 * vu.log10()).max(-48.0)
                    } else {
                        -48.0
                    };

                    ui.horizontal(|ui| {
                        let text_col = if vu > 0.85 {
                            crate::ui::theme::palette().red()
                        } else if vu > 0.50 {
                            crate::ui::theme::palette().orange()
                        } else {
                            crate::ui::theme::palette().green()
                        };
                        ui.label(egui::RichText::new(crate::tstatus!("Ingångssignal: {:.1} dB", vu_db)).monospace().strong().color(text_col));

                        if vu > 0.95 {
                            ui.label(egui::RichText::new(crate::i18n::t("⚠ CLIP")).strong().color(crate::ui::theme::palette().red()));
                        }
                    });

                    // Visual LED Bar Meter
                    let (m_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                    ui.painter().rect_filled(m_rect, Rounding::same(3.0), crate::ui::theme::palette().bg());
                    ui.painter().rect_stroke(m_rect, Rounding::same(3.0), Stroke::new(1.0_f32, crate::ui::theme::palette().border()));

                    let fill_w = (m_rect.width() * vu.clamp(0.0, 1.0)).max(2.0);
                    let bar_color = if vu > 0.85 {
                        crate::ui::theme::palette().red()
                    } else if vu > 0.50 {
                        crate::ui::theme::palette().orange()
                    } else {
                        crate::ui::theme::palette().green()
                    };
                    ui.painter().rect_filled(Rect::from_min_size(m_rect.min, Vec2::new(fill_w, m_rect.height())), Rounding::same(3.0), bar_color);

                    ui.add_space(4.0);
                    let mut gain = self.vocal_studio.mic_settings.input_gain;
                    let gain_db = 20.0 * gain.log10();
                    let gain_str = crate::tstatus!("Mikrofonförstärkning: {:.2}x ({:+.1} dB)", gain, gain_db);
                    ui.horizontal(|ui| {
                        if ui.add(egui::Slider::new(&mut gain, 0.0..=4.0).text(gain_str)).changed() {
                            self.vocal_studio.set_input_gain(gain);
                        }
                    });
                });

                ui.add_space(6.0);

                // Group 3: Noise Gate & Feedback Suppression
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("Brusreducering & Anti-rundgång")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().yellow()));
                    ui.add_space(4.0);

                    let mut gate = self.vocal_studio.mic_settings.noise_gate_thresh;
                    let gate_db = if gate > 0.0001 { 20.0 * gate.log10() } else { -60.0 };
                    let gate_open = self.vocal_studio.mic_vu_level >= gate;
                    let gate_str = crate::tstatus!("Bruströskel (Gate): {:.3} ({:.1} dB)", gate, gate_db);

                    ui.horizontal(|ui| {
                        if ui.add(egui::Slider::new(&mut gate, 0.000..=0.100).text(gate_str)).changed() {
                            self.vocal_studio.set_noise_gate(gate);
                        }
                        let led_color = if gate_open { crate::ui::theme::palette().green() } else { crate::ui::theme::palette().red() };
                        let led_text = if gate_open { crate::i18n::t("🟢 ÖPPEN") } else { crate::i18n::t("🔴 STÄNGD") };
                        ui.label(egui::RichText::new(led_text).strong().color(led_color).size(Theme::FONT_S));
                    });

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.vocal_studio.mic_settings.feedback_reduction, crate::i18n::t("🔇 Feedback Suppression / Anti-rundgång"));
                        let mut low_cut = self.vocal_studio.mic_settings.low_cut_80hz;
                        if ui
                            .checkbox(&mut low_cut, "📉 80Hz Low-Cut (Tar bort muller & bordsvibrationer)")
                            .on_hover_text(crate::i18n::t("Högpass som tar bort rummets lågfrekvens innan den når mastern. Utan den kan rummet börja vibrera (baston) när direktlyssningen är på."))
                            .changed()
                        {
                            self.vocal_studio.set_low_cut(low_cut);
                        }
                    });
                });

                ui.add_space(6.0);

                // Group 4: Live DSP Effects (Reverb, De-Esser, Compressor, Direct Monitoring)
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("Vokaleffekter i realtid (Live DSP)")).strong().size(Theme::FONT_M).color(crate::ui::theme::palette().purple()));
                    ui.add_space(4.0);

                    let rev_str = format!("Rumsklang: {:.0}%", self.vocal_studio.mic_settings.vocal_reverb * 100.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::Slider::new(&mut self.vocal_studio.mic_settings.vocal_reverb, 0.0..=1.0).text(rev_str));
                    });

                    let ess_str = crate::tstatus!("De-Esser (S-dämpning): {:.0}%", self.vocal_studio.mic_settings.de_esser_amount * 100.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::Slider::new(&mut self.vocal_studio.mic_settings.de_esser_amount, 0.0..=1.0).text(ess_str));
                    });

                    let comp_str = format!("Vokal Kompressor: {:.0}%", self.vocal_studio.mic_settings.compressor_amount * 100.0);
                    ui.horizontal(|ui| {
                        ui.add(egui::Slider::new(&mut self.vocal_studio.mic_settings.compressor_amount, 0.0..=1.0).text(comp_str));
                    });

                    ui.checkbox(&mut self.vocal_studio.mic_settings.direct_monitoring, crate::i18n::t("🎧 Direktlyssning i hörlurar (Zero-Latency Direct Monitoring)"));
                });

                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(crate::i18n::t("OK / Stäng")).clicked() {
                        close = true;
                    }

                    let is_rec = self.vocal_studio.is_recording;
                    let btn_label = if is_rec { "⏹ Avsluta Provpratning" } else { "🎙 Provprata & Testa Inspelning" };
                    let btn_col = if is_rec { crate::ui::theme::palette().red() } else { crate::ui::theme::palette().green() };
                    if ui.add(egui::Button::new(egui::RichText::new(btn_label).strong().color(crate::ui::theme::palette().text())).fill(btn_col)).clicked() {
                        if is_rec {
                            match self.vocal_studio.stop_recording() {
                                Ok(_) => {
                                    self.sync_track_stem_to_engine(4);
                                    self.status_message = crate::i18n::t("✔ Testinspelning klar och sparad i Sångstudion!").to_string();
                                }
                                Err(e) => self.status_message = format!("❌ {}", e),
                            }
                        } else {
                            match self.vocal_studio.start_recording() {
                                Ok(()) => self.status_message = crate::i18n::t("🔴 Provpratar... Säg några ord i mikrofonen!").to_string(),
                                Err(e) => self.status_message = format!("❌ {}", e),
                            }
                        }
                    }
                });
            });
        });

    if close || !open {
        self.show_mic_settings_modal = false;
    }
}
}
