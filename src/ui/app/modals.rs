//! Dialogerna — alla små fönster på ett ställe.
//!
//! Varje funktion här ritar **ett** fönster och gör ingenting annat: reglerna den vilar på
//! bor i sina egna moduler (import, projekt, export), så att de går att pröva utan fönster.
//!
//! Status: byggs — dialogerna; en ny dialog läggs här och håller sin regel utanför.
//! Rör inte: knapptexten är en instruktion, inte kosmetika — ändra texten när vägen ändras.

use super::*;

impl SonixApp {
/// Logotypen som textur, laddad **första gången den behövs**. `None` när bilden inte gick
/// att avkoda — anroparen ritar då ingen bild, aldrig en tom ruta.
pub(crate) fn logo_texture_for(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
    if self.logo_texture.is_none()
        && let Some(img) = sonix_logo()
    {
        self.logo_texture = Some(ctx.load_texture(
            "sonix-logo",
            img.clone(),
            egui::TextureOptions::LINEAR,
        ));
    }
    self.logo_texture.clone()
}
}

impl SonixApp {
pub(crate) fn render_add_track_modal(&mut self, ctx: &egui::Context) {
    let import_req = render_add_track_modal(
        &mut self.show_add_track_modal,
        &mut self.add_track_state,
        &mut self.playlist_tracks,
        &mut self.status_message,
        ctx,
    );
    if let Some(tmpl) = import_req {
        self.pending_add_track_import = Some(tmpl);
        self.spawn_async_file_picker(
            "WAV",
            &["wav", "WAV"],
            crate::i18n::t("Välj ljudfil (WAV) att importera"),
        );
    }
}
}

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
            ui.label(egui::RichText::new(crate::i18n::t("Utöka ditt personliga ljudbibliotek med egna samples!")).size(11.0).color(Theme::TEXT_MUTED));
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
            ui.label(egui::RichText::new(crate::i18n::t("Sample Karaktär & Syntes:")).strong().size(11.0));

            let icons = ["📂", "💥", "🥁", "👏", "⚡", "🌊", "🗣", "🎸", "🎹", "✨", "🔔"];
            ui.horizontal_wrapped(|ui| {
                ui.label(crate::i18n::t("Ikon:"));
                for &ic in icons.iter() {
                    let is_sel = self.custom_import_icon == ic;
                    let fill = if is_sel { Theme::FL_GREEN } else { Theme::PANEL_BG };
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
            ui.painter().rect_filled(wf_rect, Rounding::same(3.0), Color32::from_rgb(18, 20, 24));
            let mid_y = wf_rect.center().y;
            let dx = wf_rect.width() / 32.0;
            for (wi, &val) in preview_wave.iter().enumerate() {
                let sx = wf_rect.min.x + wi as f32 * dx + dx * 0.5;
                let h = (val.abs() * 13.0).max(1.0);
                ui.painter().line_segment([Pos2::new(sx, mid_y - h), Pos2::new(sx, mid_y + h)], Stroke::new(1.5_f32, Theme::FL_GREEN));
            }

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new(crate::i18n::t("▶ Provspela")).size(11.0)).clicked() {
                    let freq = midi_to_freq(self.custom_import_note);
                    let _ = self.engine.send_command(AudioCommand::NoteOn {
                        note: self.custom_import_note,
                        freq,
                        velocity: 0.9,
                    });
                }

                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("💾 Spara i Bibliotek")).strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
                    self.import_custom_sample(
                        self.custom_import_name_input.clone(),
                        self.custom_import_category.clone(),
                        self.custom_import_icon.clone(),
                        self.custom_import_note,
                        Color32::from_rgb(255, 200, 80),
                        self.custom_import_freq,
                        self.custom_import_decay,
                    );
                    close = true;
                }

                if ui.button(egui::RichText::new(crate::i18n::t("Avbryt")).size(11.0)).clicked() {
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
pub(crate) fn render_hardware_controller_modal(&mut self, ctx: &egui::Context) {
    if !self.show_controller_modal {
        return;
    }

    let mut close = false;
    egui::Window::new(crate::i18n::t("🎛 Hårdvarukontroller & MCU / OSC Routing"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_width(520.0);
            ui.label(egui::RichText::new(crate::i18n::t("Direkt hårdvaruintegration med Mackie Control Universal (MCU) och Open Sound Control (OSC)")).size(11.0).color(Theme::TEXT_MUTED));
            ui.add_space(8.0);

            // Section 1: Mackie MCU
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🎚 MACKIE CONTROL UNIVERSAL (MCU)")).strong().size(12.0).color(Theme::FL_ORANGE));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (status_txt, status_col) = if self.mcu_input.is_some() {
                            (crate::i18n::t("● ALSA SEQUENCER-PORT ÖPPEN"), Theme::FL_GREEN)
                        } else {
                            (crate::i18n::t("○ FRÅNKOPPLAD"), Theme::TEXT_MUTED)
                        };
                        ui.label(egui::RichText::new(status_txt).strong().size(10.0).color(status_col));
                    });
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Enhet:"));
                    ui.label(egui::RichText::new(&self.mcu_device_name).strong().color(Theme::FL_CYAN));
                });

                ui.horizontal(|ui| {
                    if self.mcu_input.is_some() {
                        if ui.button(crate::i18n::t("🔌 Koppla från MCU")).clicked() {
                            self.mcu_input = None;
                            self.mcu_connected = false;
                            self.status_message = crate::i18n::t("MCU frånkopplad.").to_string();
                        }
                    } else if ui.button(crate::i18n::t("🔌 Anslut MCU (ALSA Seq)")).clicked() {
                        match McuInput::connect(self.control_tx.clone()) {
                            Ok(m) => {
                                self.mcu_input = Some(m);
                                self.status_message = crate::i18n::t("✔ MCU-port öppnad. Anslut enhet med 'aconnect'.").to_string();
                            }
                            Err(e) => {
                                self.status_message = crate::tstatus!("⚠ Kunde inte öppna MCU: {}", e);
                            }
                        }
                    }
                    if self.mcu_input.is_some() {
                        ui.label(egui::RichText::new(format!("{} events", self.mcu_input.as_ref().map(|m| m.received()).unwrap_or(0))).size(10.0).color(Theme::FL_GREEN));
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Fader Bank:"));
                    let banks = [crate::i18n::t("Bank 1 (Spår 1–8)"), crate::i18n::t("Bank 2 (Spår 9–16)"), crate::i18n::t("Bank 3 (Bussar & VCA)")];
                    for (b_i, name) in banks.iter().enumerate() {
                        let is_b = self.mcu_bank == b_i;
                        if ui.selectable_label(is_b, *name).clicked() {
                            self.mcu_bank = b_i;
                            self.status_message = crate::tstatus!("MCU växlade till {}", name);
                        }
                    }
                });

                ui.add_space(4.0);
                ui.label(egui::RichText::new(crate::i18n::t("Motorfaders feedback status:")).size(10.0).color(Theme::TEXT_MUTED));
                ui.horizontal(|ui| {
                    for i in 0..8 {
                        let ch_vol = if i < self.playlist_tracks.len() { self.playlist_tracks[i].volume } else if i < self.channels.len() { self.channels[i].volume } else { 0.8 };
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(format!("CH{}", i + 1)).size(8.0));
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 36.0), Sense::hover());
                            ui.painter().rect_filled(rect, Rounding::same(2.0), Color32::from_rgb(20, 24, 30));
                            let h = (ch_vol * 34.0).clamp(0.0, 34.0);
                            let fill_rect = Rect::from_min_max(Pos2::new(rect.min.x + 1.0, rect.max.y - h), Pos2::new(rect.max.x - 1.0, rect.max.y - 1.0));
                            ui.painter().rect_filled(fill_rect, Rounding::same(1.0), Theme::FL_ORANGE);
                        });
                    }
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let learn_txt = if self.midi_learn_active { "🔴 MIDI LEARN: LYSSNAR..." } else { "🎯 Aktivera MIDI Learn" };
                    let learn_bg = if self.midi_learn_active { Color32::RED } else { Theme::PANEL_BG };
                    if ui.add(egui::Button::new(egui::RichText::new(learn_txt).strong().size(10.5).color(Color32::WHITE)).fill(learn_bg)).clicked() {
                        self.midi_learn_active = !self.midi_learn_active;
                    }
                });
            });

            ui.add_space(8.0);

            // Section 1b: MIDI Keyboard (Piano Roll input) — Fas 5.3
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🎹 MIDI-KLAVIATUR (Piano Roll)")).strong().size(12.0).color(Theme::FL_GREEN));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (status_txt, status_col) = if self.midi_keyboard_connected {
                            (crate::i18n::t("● MIDI-IN-PORT ÖPPEN"), Theme::FL_GREEN)
                        } else {
                            (crate::i18n::t("○ FRÅNKOPPLAD"), Theme::TEXT_MUTED)
                        };
                        ui.label(egui::RichText::new(status_txt).strong().size(10.0).color(status_col));
                    });
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Enhet:"));
                    ui.label(egui::RichText::new(&self.midi_device_name).strong().color(Theme::FL_CYAN));
                });

                ui.horizontal(|ui| {
                    if self.midi_input.is_some() {
                        if ui.button(crate::i18n::t("🔌 Koppla från MIDI")).clicked() {
                            self.midi_input = None;
                            self.midi_keyboard_connected = false;
                            self.midi_held_notes.clear();
                            self.status_message = crate::i18n::t("MIDI-klaviatur frånkopplad.").to_string();
                        }
                    } else if ui.button(crate::i18n::t("🔌 Anslut MIDI")).clicked() {
                        match MidiKeyboardInput::connect(self.control_tx.clone()) {
                            Ok(m) => {
                                self.midi_input = Some(m);
                                self.status_message = crate::i18n::t("✔ MIDI-in öppnad — klaviaturen ansluts automatiskt.").to_string();
                            }
                            Err(e) => {
                                self.status_message = crate::tstatus!("⚠ Kunde inte öppna MIDI-in: {}", e);
                            }
                        }
                    }
                    if self.midi_input.is_some() {
                        ui.label(egui::RichText::new(crate::tstatus!("{} nothändelser", self.midi_note_count)).size(10.0).color(Theme::FL_GREEN));
                    }
                });

                ui.add_space(4.0);
                let rec_bg = if self.midi_record_armed { Color32::from_rgb(180, 40, 40) } else { Theme::PANEL_BG };
                let rec_txt = if self.midi_record_armed {
                    crate::i18n::t("🔴 MIDI-REC: ARMED (spelar in i Piano Roll)")
                } else {
                    crate::i18n::t("⏺ Armera MIDI-inspelning till Piano Roll")
                };
                if ui.add(egui::Button::new(egui::RichText::new(rec_txt).strong().size(10.5).color(Color32::WHITE)).fill(rec_bg)).on_hover_text(crate::i18n::t("När armerad skrivs hållna MIDI-klaviaturtoner in i det aktiva mönstrets rutnät under uppspelning.")).clicked() {
                    self.midi_record_armed = !self.midi_record_armed;
                if self.midi_record_armed {
                    self.begin_new_take();
                }
                }
                ui.label(egui::RichText::new(crate::i18n::t("Tips: klaviaturen ansluts automatiskt. Hittas den inte: koppla in den och anslut igen.")).size(9.5).color(Theme::TEXT_MUTED));
            });

            ui.add_space(8.0);

            // Section 2: Open Sound Control (OSC)
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("📡 OPEN SOUND CONTROL (OSC / UDP)")).strong().size(12.0).color(Theme::FL_CYAN));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.osc_server.is_some() {
                            if ui.button(crate::i18n::t("⏹ Stoppa server")).clicked() {
                                self.osc_server = None;
                                self.osc_enabled = false;
                                self.osc_rx_count = 0;
                                self.status_message = crate::i18n::t("OSC-server stoppad.").to_string();
                            }
                        } else if ui.button(crate::i18n::t("▶ Starta server")).clicked() {
                            match OscServer::start(self.osc_rx_port, self.control_tx.clone()) {
                                Ok(s) => {
                                    let bound = s.bound_port;
                                    self.osc_server = Some(s);
                                    self.osc_enabled = true;
                                    self.status_message = crate::tstatus!("📡 OSC-server lyssnar på UDP-port {}", bound);
                                }
                                Err(e) => {
                                    self.status_message = crate::tstatus!("⚠ Kunde inte starta OSC: {}", e);
                                }
                            }
                        }
                    });
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Lyssningsport (RX):"));
                    ui.add(egui::DragValue::new(&mut self.osc_rx_port).range(1024..=65535));
                    ui.separator();
                    ui.label(crate::i18n::t("Sändningsport (TX):"));
                    ui.add(egui::DragValue::new(&mut self.osc_tx_port).range(1024..=65535));
                });

                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    let status_col = if self.osc_server.is_some() { Theme::FL_GREEN } else { Theme::TEXT_MUTED };
                    let status_txt = match &self.osc_server {
                        Some(s) => format!("Lyssnar på UDP-port {} • mottagna OSC-paket: {}", s.bound_port, self.osc_rx_count),
                        None => format!("Mottagna OSC-paket: {}", self.osc_rx_count),
                    };
                    ui.label(egui::RichText::new(status_txt).size(10.5).color(status_col));
                    if let Some(err) = self.osc_server.as_ref().and_then(|s| s.last_error.lock().ok().and_then(|g| g.clone())) {
                        ui.label(egui::RichText::new(crate::tstatus!("⚠ {}", err)).size(10.0).color(Theme::FL_YELLOW));
                    }
                    if ui.button(crate::i18n::t("⚡ Skicka Test Ping (/sonix/ping)")).clicked() {
                        let addr = std::net::SocketAddr::from(([127, 0, 0, 1], self.osc_tx_port));
                        let mut pkt = Vec::new();
                        pkt.extend_from_slice(b"/sonix/ping\0\0\0");
                        pkt.push(b','); pkt.push(b'f'); pkt.push(0); pkt.push(0);
                        pkt.extend_from_slice(&1.0f32.to_be_bytes());
                        match std::net::UdpSocket::bind("0.0.0.0:0")
                            .and_then(|s| s.send_to(&pkt, addr))
                        {
                            Ok(n) => self.status_message = crate::tstatus!("📡 Sände {} byte OSC till {}", n, addr.to_string()),
                            Err(e) => self.status_message = crate::tstatus!("⚠ OSC-sändning misslyckades: {}", e),
                        }
                    }
                });

                ui.collapsing("📋 OSC Adress-mappningar", |ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("• /sonix/transport/play (1/0)\n• /sonix/track/{1..8}/volume (0.0 .. 1.25)\n• /sonix/track/{1..8}/mute (1/0)\n• /sonix/bus/{vocal|drum|synth|fx}/volume (0.0 .. 1.25)\n• /sonix/vca/{1..4}/volume (0.0 .. 1.25)")).size(10.0).color(Theme::TEXT_MUTED));
                });
            });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.button(self.tr("Stäng")).clicked() {
                    close = true;
                }
            });
        });

    if close {
        self.show_controller_modal = false;
    }
}
}

impl SonixApp {
/// Frågan om mp3 eller wav (Alex' förslag, Fas 8.5).
///
/// **Varför en fråga och inte en regel som bara kör.** Appen kan inte spela
/// mp3 — den konverteras först — så en mp3 vars stämma redan finns som wav är
/// ett extra varv utan vinst. Men den som importerar kan ha skäl att vilja ha
/// mp3-filerna med (de finns i projektet, de kan användas utanför Sonix), så
/// valet är användarens. Standardvalet är regeln: hoppa över dem.
pub(crate) fn render_wav_question_modal(&mut self, ctx: &egui::Context) {
    let Some(question) = self.pending_wav_question.clone() else {
        return;
    };
    let mut keep_mp3: Option<bool> = None;
    let mut separate_sibling = false;
    let mut separate_chosen = false;
    let mut cancel = false;
    egui::Window::new(crate::i18n::t("🎧 mp3 eller wav?"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .default_size(Vec2::new(620.0, 200.0))
        .show(ctx, |ui| match &question {
            WavQuestion::StemImport { duplicates, title, .. } => {
                ui.label(
                    egui::RichText::new(crate::tstatus!(
                        "{} mp3-filer i '{}' har redan sin stämma som wav-fil.",
                        duplicates,
                        title
                    ))
                    .size(12.5)
                    .color(Theme::TEXT_BRIGHT),
                );
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Sonix kan inte spela mp3 — den konverteras först, vilket är ett extra varv när wav-filen redan finns. En stämma som bara finns som mp3 tas alltid med.",
                    ))
                    .size(11.0)
                    .color(Theme::TEXT_MUTED),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(crate::i18n::t("Hoppa över mp3:erna (rekommenderas)"))
                        .clicked()
                    {
                        keep_mp3 = Some(false);
                    }
                    if ui.button(crate::i18n::t("Ta med mp3:erna också")).clicked() {
                        keep_mp3 = Some(true);
                    }
                    if ui.button(crate::i18n::t("Avbryt importen")).clicked() {
                        cancel = true;
                    }
                });
            }
            WavQuestion::Separate { chosen, sibling } => {
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Du valde en mp3 — och wav-filen finns bredvid.",
                    ))
                    .size(12.5)
                    .color(Theme::TEXT_BRIGHT),
                );
                ui.label(
                    egui::RichText::new(crate::tstatus!("mp3:  {}", chosen))
                        .size(10.5)
                        .color(Theme::TEXT_MUTED),
                );
                ui.label(
                    egui::RichText::new(crate::tstatus!("wav:  {}", sibling))
                        .size(10.5)
                        .color(Theme::TEXT_MUTED),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(crate::i18n::t("Separera wav-filen (rekommenderas)"))
                        .clicked()
                    {
                        separate_sibling = true;
                    }
                    if ui.button(crate::i18n::t("Separera mp3-filen")).clicked() {
                        separate_chosen = true;
                    }
                    if ui.button(crate::i18n::t("Avbryt")).clicked() {
                        cancel = true;
                    }
                });
            }
        });

    if cancel {
        self.pending_wav_question = None;
        self.status_message = crate::i18n::t("Avbrutet — inget lästes in").to_string();
        return;
    }
    match question {
        WavQuestion::StemImport { source, title, bpm, .. } => {
            let Some(keep) = keep_mp3 else {
                return;
            };
            self.pending_wav_question = None;
            match source {
                StemImportSource::Folder(dir) => {
                    self.start_suno_folder_import(&dir, &title, bpm, keep)
                }
                StemImportSource::Zip(zip) => self.start_suno_zip_import_choice(&zip, keep),
            }
        }
        WavQuestion::Separate { chosen, sibling } => {
            if separate_sibling {
                self.pending_wav_question = None;
                self.start_stem_separation_with(&sibling);
            } else if separate_chosen {
                self.pending_wav_question = None;
                self.start_stem_separation_with(&chosen);
            }
        }
    }
}
}

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
                    ui.label(egui::RichText::new(crate::i18n::t("🎵")).size(24.0));
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("Multi-Track Stämmor & Ljudspår (Stems)")).strong().size(13.0).color(Theme::FL_ORANGE));
                        ui.label(egui::RichText::new(crate::i18n::t("Importera nedladdade ZIP-paket eller mappar med stämmor (Suno, FL Studio, Ableton, Logic m.fl.). Sonix läser ut äkta 48kHz WAV-vågformer, detekterar tempo (BPM) och mappar spåren i tidslinjen.")).size(10.5).color(Theme::TEXT_MUTED));
                    });
                });
            });

            ui.add_space(8.0);

            // Quick Rescan Bar
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("UPPTÄCKTA STEMPAKET:")).strong().size(11.0).color(Theme::FL_CYAN));
                if ui.button(crate::i18n::t("🔄 Skanna ~/Music & ~/Downloads")).clicked() {
                    self.scan_for_suno_stems();
                }
            });

            ui.add_space(4.0);

            // List of detected ZIPs
            ui.group(|ui| {
                if self.detected_suno_zips.is_empty() {
                    ui.label(egui::RichText::new(crate::i18n::t("Inga .zip-stempaket hittades i ~/Music eller ~/Downloads. Klicka på knappen ovan för att skanna, eller ange sökväg manuellt nedan.")).size(10.5).color(Theme::TEXT_MUTED));
                } else {
                    egui::ScrollArea::vertical().max_height(140.0).show(ui, |ui| {
                        for zip_p in &self.detected_suno_zips {
                            let (title, bpm) = Self::parse_suno_zip_info(zip_p);
                            let is_wav = zip_p.contains("(1).zip") || zip_p.to_lowercase().contains("wav");
                            let badge = if is_wav { "🔊 48kHz WAV Stems" } else { "🎧 MP3 Stems" };

                            ui.group(|ui| {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new(format!("🎵 {}", title)).strong().size(11.5).color(Color32::WHITE));
                                        ui.horizontal(|ui| {
                                            ui.label(egui::RichText::new(format!("Tempo: {:.1} BPM", bpm)).size(10.0).color(Theme::FL_GREEN));
                                            ui.label(egui::RichText::new(badge).size(10.0).color(Theme::FL_CYAN));
                                        });
                                    });

                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("⚡ Importera Alla Stämmor")).strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
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
                ui.label(egui::RichText::new(crate::i18n::t("VÄLJ ZIP-FIL ELLER MAPP")).strong().size(11.0).color(Theme::FL_ORANGE));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📁 Välj ZIP-fil från datorn...")).strong().size(11.5).color(Color32::WHITE)).fill(Color32::from_rgb(60, 90, 150))).clicked() {
                        self.spawn_async_file_picker("Stempaket", &["zip"], crate::i18n::t("Välj ZIP Stempaket"));
                    }

                    ui.separator();
                    ui.label(crate::i18n::t("eller ange sökväg:"));
                    ui.text_edit_singleline(&mut self.custom_stem_path_input);
                    if ui.add(egui::Button::new(crate::i18n::t("Ladda")).fill(Color32::from_rgb(50, 60, 80))).clicked() {
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
                ui.label(egui::RichText::new(crate::i18n::t("💡 Tips: Du kan även dra & släppa (Drag & Drop) .zip-filer direkt in i fönstret!")).size(9.5).color(Theme::FL_GREEN));
            });

            ui.add_space(8.0);

            // Stems Layout Preview Info
            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("SPÅRSTRUKTUR I SONIX:")).strong().size(10.5).color(Theme::TEXT_MUTED));
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
                        ui.label(egui::RichText::new(format!("• {} ➔ [{}]  ", st, bus)).size(10.0).color(Theme::TEXT_BRIGHT));
                    }
                });
            });

            ui.add_space(10.0);

            ui.horizontal(|ui| {
                if ui.button(egui::RichText::new(crate::i18n::t("Stäng")).size(11.0)).clicked() {
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
                            ui.label(egui::RichText::new(crate::i18n::t("Läser in och avkodar stämmor (Stems)")).strong().size(13.0).color(Theme::FL_ORANGE));
                            ui.label(egui::RichText::new(&stage).size(11.0).color(Color32::WHITE));
                        });
                    });
                });

                ui.add_space(8.0);
                ui.add(egui::ProgressBar::new(ratio).show_percentage().animate(true));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if !file.is_empty() {
                        ui.label(egui::RichText::new(crate::tstatus!("Spår {} av {}: {}", idx, total, file)).size(10.0).color(Theme::TEXT_MUTED));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("{:.0}%", ratio * 100.0)).strong().size(11.0).color(Theme::FL_CYAN));
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
                            ui.label(egui::RichText::new(crate::tstatus!("Öppnar: {}", proj)).strong().size(13.0).color(Theme::FL_CYAN));
                            ui.label(egui::RichText::new(&stage).size(11.0).color(Color32::WHITE));
                        });
                    });
                });

                ui.add_space(8.0);
                ui.add(egui::ProgressBar::new(ratio).show_percentage().animate(true));
                ui.add_space(4.0);

                ui.horizontal(|ui| {
                    if !track.is_empty() {
                        ui.label(egui::RichText::new(crate::tstatus!("Läser spår {}/{}: {}", idx, total, track)).size(10.5).color(Theme::TEXT_MUTED));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("{:.0}%", ratio * 100.0)).strong().size(11.0).color(Theme::FL_ORANGE));
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
                .size(11.5)
                .color(Theme::TEXT_BRIGHT),
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
                        .size(11.5)
                        .color(Theme::FL_CYAN),
                    );
                    if ui
                        .button(crate::i18n::t("  Återställ  "))
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
                .size(10.0)
                .color(Theme::TEXT_MUTED),
            );
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(crate::i18n::t("  Fortsätt utan att återställa  "))
                                .size(11.5),
                        )
                        .fill(Theme::FL_ORANGE),
                    )
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
                        ui.label(egui::RichText::new(crate::i18n::t("SONIX")).size(28.0).color(Theme::FL_CYAN));
                    }
                }
                ui.heading(egui::RichText::new(crate::i18n::t("SONIX STUDIO")).strong().size(22.0).color(Theme::FL_ORANGE));
                ui.label(egui::RichText::new(crate::i18n::t("Professionell Digital Audio Workstation & AI Musikstudio för Linux")).size(12.0).color(Theme::FL_CYAN));
                ui.label(egui::RichText::new(crate::tstatus!("Version {} (PipeWire / ALSA / JACK Audio Engine)", env!("CARGO_PKG_VERSION"))).size(10.5).color(Theme::TEXT_MUTED));

                ui.add_space(12.0);
                ui.separator();
                ui.add_space(8.0);

                ui.label(egui::RichText::new(crate::i18n::t("🚀 Huvudfunktioner i Sonix Studio:")).strong().size(12.0).color(Theme::TEXT_BRIGHT));
                ui.add_space(4.0);
                ui.label(egui::RichText::new(crate::i18n::t("• Realtids Multi-Track Audio Streaming & Mixmotor i Rust")).size(11.0).color(Theme::TEXT_MUTED));
                ui.label(egui::RichText::new(crate::i18n::t("• Fullt stöd för Suno AI Stems med linjär vågformsredigering")).size(11.0).color(Theme::TEXT_MUTED));
                ui.label(egui::RichText::new(crate::i18n::t("• 16-Stegs Sonix Channel Rack, Piano Roll & Touch Piano")).size(11.0).color(Theme::TEXT_MUTED));
                ui.label(egui::RichText::new(crate::i18n::t("• Klippverktyg (Slice ✂), Fading & Dynamisk Gain-justering")).size(11.0).color(Theme::TEXT_MUTED));
                ui.label(egui::RichText::new(crate::i18n::t("• 64-bit SIMD Audio DSP med Zero-Latency Process Thread")).size(11.0).color(Theme::TEXT_MUTED));

                ui.add_space(16.0);
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("  Stäng  ")).strong().size(12.0).color(Color32::BLACK)).fill(Theme::FL_ORANGE)).clicked() {
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
                if ui.button(egui::RichText::new(crate::i18n::t("📄 Nytt tomt projekt")).strong().color(Theme::FL_CYAN)).clicked() {
                    create_new = true;
                    close = true;
                }
                if ui.button(egui::RichText::new(crate::i18n::t("⚡ Ladda Demo-projekt")).strong().color(Theme::FL_GREEN)).clicked() {
                    load_demo = true;
                    close = true;
                }
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("📂 Välj .sonix från datorn...")).strong().color(Color32::WHITE)).fill(Color32::from_rgb(60, 90, 150))).clicked() {
                    self.spawn_async_file_picker(
                        "Sonix-projekt",
                        &["sonix"],
                        crate::i18n::t("Välj Sonix Projektfil (.sonix)"),
                    );
                }
                if ui.button(egui::RichText::new(crate::i18n::t("🎼 Importera Suno ZIP...")).strong().color(Theme::FL_ORANGE)).clicked() {
                    show_suno = true;
                    close = true;
                }
            });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(8.0);

            // Öppna från sökväg
            ui.group(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("📂 ÖPPNA PROJEKT FRÅN SÖKVÄG")).strong().color(Theme::FL_CYAN));
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Sökväg:"));
                    ui.text_edit_singleline(&mut self.custom_project_path_input);
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("Öppna")).strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
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
                ui.label(egui::RichText::new(crate::i18n::t("💾 SPARA PROJEKT")).strong().color(Theme::FL_ORANGE));
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
                .color(Theme::FL_CYAN),
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
                    ui.label(egui::RichText::new(crate::i18n::t("Inga sparade .sonix-projekt hittades ännu. Spara ditt nuvarande projekt ovan eller ladda ett demo-projekt!")).italics().color(Theme::TEXT_MUTED));
                });
            } else {
                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                    for (stem, full_path) in saved_files {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(format!("🎵 {}", stem)).strong().color(Theme::TEXT_BRIGHT));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("Öppna Projekt")).strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
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
                ui.label(egui::RichText::new(crate::i18n::t("💬 Textgenerering (AI Co-Producer)")).strong().color(Theme::FL_GREEN));
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
                ui.label(egui::RichText::new(crate::i18n::t("🎧 AI-ljudgenerering (Stable Audio m.fl.)")).strong().color(Theme::FL_CYAN));
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
                .size(10.5)
                .color(if ready { Theme::FL_GREEN } else { Theme::TEXT_MUTED }),
            );

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("💾 Spara inställningar")).strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
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
                ui.label(egui::RichText::new(crate::i18n::t("Ljudmotor & Drivrutiner")).strong().color(Theme::FL_CYAN));
                ui.label(crate::tstatus!("Värd: {} · Enhet: {}", self.engine.host_name, self.engine.device_name));
                let buffer_txt = self.engine.buffer_frames
                    .map(|f| format!("{f} frames"))
                    .unwrap_or_else(|| crate::i18n::t("enhetens standard").to_string());
                ui.label(egui::RichText::new(crate::tstatus!(
                    "Aktiv ström: {} Hz · {} · {} kanaler",
                    self.engine.sample_rate,
                    buffer_txt,
                    self.engine.channels
                )).size(10.5).color(Theme::TEXT_MUTED));

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
                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔁 Tillämpa på ljudströmmen")).strong().color(Color32::BLACK)).fill(Theme::FL_GREEN)).clicked() {
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
                        ui.label(egui::RichText::new(crate::i18n::t("Ljudenhet & Mikrofonkälla")).strong().size(13.0).color(Theme::FL_CYAN));
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

                    egui::ComboBox::from_label(crate::i18n::t("Välj Mikrofon"))
                        .selected_text(dev_list.get(current_idx).cloned().unwrap_or_else(|| crate::i18n::t("Standardmikrofon").to_string()))
                        .width(360.0)
                        .show_ui(ui, |ui| {
                            for (idx, dev_name) in dev_list.iter().enumerate() {
                                let is_rec_pref = dev_name.to_lowercase().contains("samson") || dev_name.to_lowercase().contains("usb");
                                let label = if is_rec_pref {
                                    format!("🎤 {} (Rekommenderad)", dev_name)
                                } else {
                                    format!("🎙 {}", dev_name)
                                };
                                if ui.selectable_value(&mut current_idx, idx, label).clicked() {
                                    let switched = self.vocal_studio.select_microphone(idx);
                                    if switched {
                                        self.status_message = crate::tstatus!("✔ Mikrofon ändrad till: {}", dev_name);
                                    }
                                }
                            }
                        });

                    let active_dev = self.vocal_studio.mic_capture.as_ref().map(|m| m.device_name.as_str()).unwrap_or("Standard");
                    let sr = self.vocal_studio.mic_capture.as_ref().map(|m| m.sample_rate).unwrap_or(44100);
                    ui.label(egui::RichText::new(crate::tstatus!("🟢 Aktiv ström: {} • {} Hz 32-bit Float", active_dev, sr)).size(10.5).color(Theme::FL_GREEN));
                });

                ui.add_space(6.0);

                // Group 2: Real-time VU & Level Meter with Gain
                ui.group(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("Ingångsnivå & Förförstärkare (Gain & VU)")).strong().size(13.0).color(Theme::FL_ORANGE));
                    ui.add_space(4.0);

                    let vu = self.vocal_studio.mic_vu_level;
                    let vu_db = if vu > 0.0001 {
                        (20.0 * vu.log10()).max(-48.0)
                    } else {
                        -48.0
                    };

                    ui.horizontal(|ui| {
                        let text_col = if vu > 0.85 {
                            Color32::from_rgb(255, 60, 60)
                        } else if vu > 0.50 {
                            Theme::FL_ORANGE
                        } else {
                            Theme::FL_GREEN
                        };
                        ui.label(egui::RichText::new(crate::tstatus!("Ingångssignal: {:.1} dB", vu_db)).monospace().strong().color(text_col));

                        if vu > 0.95 {
                            ui.label(egui::RichText::new(crate::i18n::t("⚠ CLIP")).strong().color(Color32::from_rgb(255, 40, 40)));
                        }
                    });

                    // Visual LED Bar Meter
                    let (m_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
                    ui.painter().rect_filled(m_rect, Rounding::same(3.0), Color32::from_rgb(18, 22, 30));
                    ui.painter().rect_stroke(m_rect, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(40, 50, 68)));

                    let fill_w = (m_rect.width() * vu.clamp(0.0, 1.0)).max(2.0);
                    let bar_color = if vu > 0.85 {
                        Color32::from_rgb(255, 60, 60)
                    } else if vu > 0.50 {
                        Theme::FL_ORANGE
                    } else {
                        Theme::FL_GREEN
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
                    ui.label(egui::RichText::new(crate::i18n::t("Brusreducering & Anti-rundgång")).strong().size(13.0).color(Theme::FL_YELLOW));
                    ui.add_space(4.0);

                    let mut gate = self.vocal_studio.mic_settings.noise_gate_thresh;
                    let gate_db = if gate > 0.0001 { 20.0 * gate.log10() } else { -60.0 };
                    let gate_open = self.vocal_studio.mic_vu_level >= gate;
                    let gate_str = crate::tstatus!("Bruströskel (Gate): {:.3} ({:.1} dB)", gate, gate_db);

                    ui.horizontal(|ui| {
                        if ui.add(egui::Slider::new(&mut gate, 0.000..=0.100).text(gate_str)).changed() {
                            self.vocal_studio.set_noise_gate(gate);
                        }
                        let led_color = if gate_open { Theme::FL_GREEN } else { Color32::from_rgb(180, 40, 40) };
                        let led_text = if gate_open { crate::i18n::t("🟢 ÖPPEN") } else { crate::i18n::t("🔴 STÄNGD") };
                        ui.label(egui::RichText::new(led_text).strong().color(led_color).size(10.5));
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
                    ui.label(egui::RichText::new(crate::i18n::t("Vokaleffekter i realtid (Live DSP)")).strong().size(13.0).color(Theme::FL_PURPLE));
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
                    let btn_col = if is_rec { Color32::from_rgb(220, 40, 40) } else { Theme::FL_GREEN };
                    if ui.add(egui::Button::new(egui::RichText::new(btn_label).strong().color(Color32::WHITE)).fill(btn_col)).clicked() {
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

    if let Some(chords) = insert_chords {
        if p_idx < self.patterns.len() && c_idx < num_chans {
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
                self.channels[d].steps = dice.generated_drum_grid[d];
                if p_idx < self.patterns.len() && d < self.patterns[p_idx].channel_steps.len() {
                    self.patterns[p_idx].channel_steps[d] = dice.generated_drum_grid[d];
                }
            }
            self.status_message = crate::i18n::t("🎲 Klistrade in slumpat trumgroove på de 4 första trumspåren!").to_string();
        } else if p_idx < self.patterns.len() && c_idx < num_chans {
            self.channels[c_idx].steps = [false; 16];
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
    let mut unfreeze_request: Option<usize> = None;
    let frozen_stale = self.frozen_is_stale(t_idx);

    // Samma karta som tidlinjen (Fas 8.2 steg 3): panelen visar tider, och en tid efter
    // ett tempobyte är bara rätt om den kommer från kartan.
    let tempo = self.tempo_map();

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
                    ui.label(egui::RichText::new(track.icon).size(18.0));
                    ui.add(egui::TextEdit::singleline(&mut track.name).desired_width(180.0).font(egui::FontId::proportional(13.0)));

                    ui.separator();

                    // SOLO / ISOLERA STÄMMA BUTTON (Prominent & Glowing)
                    let solo_bg = if track.solo { Theme::FL_ORANGE } else { Color32::from_rgb(45, 40, 30) };
                    let solo_fg = if track.solo { Color32::BLACK } else { Theme::FL_ORANGE };
                    if ui.add(egui::Button::new(egui::RichText::new(if track.solo { "🎧 ISOLERAD (SOLO ON)" } else { crate::i18n::t("🎧 ISOLERA STÄMMA") }).strong().color(solo_fg)).fill(solo_bg)).on_hover_text(crate::i18n::t("Isolera och lyssna enbart på denna stämma i realtid")).clicked() {
                        track.solo = !track.solo;
                        trigger_audio_sync = true;
                    }

                    // MUTE TOGGLE
                    let mute_bg = if track.muted { Color32::from_rgb(180, 40, 40) } else { Color32::from_rgb(35, 40, 50) };
                    let mute_fg = if track.muted { Color32::WHITE } else { Theme::TEXT_MUTED };
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
                            Color32::from_rgb(150, 100, 30)
                        } else {
                            Color32::from_rgb(40, 70, 90)
                        };
                        if ui.add(egui::Button::new(egui::RichText::new(label).strong().color(Color32::WHITE)).fill(bg))
                            .on_hover_text(crate::i18n::t("Spåret spelas som färdigt ljud. Tina upp det för att köra patterns igen — musiken är oförändrad."))
                            .clicked()
                        {
                            unfreeze_request = Some(t_idx);
                        }
                        if frozen_stale {
                            ui.label(egui::RichText::new(crate::i18n::t("⚠ ändrat sedan frysningen")).size(10.0).color(Theme::FL_ORANGE))
                                .on_hover_text(crate::i18n::t("Mönstret eller ljudet har ändrats efter frysningen, så ljudet är inte längre det du hör av patterns. Tina och frys igen."));
                        }
                    } else if track.can_freeze() {
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("❄ Frys spår")).strong().color(Color32::WHITE)).fill(Color32::from_rgb(35, 60, 80)))
                            .on_hover_text(crate::i18n::t("Renderar spåret till ljud och spelar det i stället för patterns — sparar CPU i stora projekt. Patterns finns kvar, och Ctrl+Z tar tillbaka frysningen."))
                            .clicked()
                        {
                            freeze_request = Some(t_idx);
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
                    let bg = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(25, 30, 40) };
                    let fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t(label)).strong().size(11.0).color(fg)).fill(bg)).clicked() {
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
                        ui.label(egui::RichText::new(crate::i18n::t("Nivåer & Stereobild för stämman")).strong().color(Theme::FL_ORANGE));
                        ui.add_space(6.0);

                        ui.horizontal(|ui| {
                            // Volume Section
                            ui.group(|ui| {
                                ui.set_width(340.0);
                                ui.vertical(|ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("🎚 Exakt Ljudvolym & Gain")).strong().size(12.0).color(Theme::FL_CYAN));
                                    ui.add_space(4.0);

                                    let cur_vol = track.volume;
                                    let gain_db = if cur_vol <= 0.001 { -60.0 } else { 20.0 * cur_vol.log10() };
                                    ui.label(egui::RichText::new(crate::tstatus!("Gain: {:+.2} dB  (Nivå: {:.1}%)", gain_db, cur_vol * 100.0)).strong().size(13.0).color(Theme::TEXT_BRIGHT));

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
                                    ui.label(egui::RichText::new(crate::i18n::t("↔ Stereopanorering")).strong().size(12.0).color(Theme::FL_CYAN));
                                    let pan_text = if track.pan < -0.01 {
                                        crate::tstatus!("Vänster {:.0}%", track.pan.abs() * 100.0)
                                    } else if track.pan > 0.01 {
                                        crate::tstatus!("Höger {:.0}%", track.pan * 100.0)
                                    } else {
                                        crate::i18n::t("Center (Mitt)").to_string()
                                    };
                                    ui.label(egui::RichText::new(format!("Pan: {}", pan_text)).color(Theme::TEXT_BRIGHT));

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
                                    ui.label(egui::RichText::new(crate::i18n::t("🎛 Dynamik & Kompressor")).strong().size(12.0).color(Theme::FL_CYAN));
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
                            ui.label(egui::RichText::new(crate::i18n::t("📈 3-Bands Parametrisk Stäm-EQ")).strong().color(Theme::FL_ORANGE));
                            ui.checkbox(&mut track.eq.enabled, "Aktivera EQ");
                        });

                        ui.add_space(4.0);

                        // Graphical EQ Curve Canvas
                        let (eq_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(10.0), 130.0), Sense::hover());
                        let p = ui.painter();
                        p.rect_filled(eq_rect, Rounding::same(4.0), Color32::from_rgb(14, 18, 26));
                        p.rect_stroke(eq_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(32, 40, 56)));

                        // Draw Grid Lines (100 Hz, 1 kHz, 10 kHz and 0 dB, +6 dB, -6 dB)
                        let mid_y = eq_rect.center().y;
                        p.line_segment([Pos2::new(eq_rect.min.x, mid_y), Pos2::new(eq_rect.max.x, mid_y)], Stroke::new(1.0_f32, Color32::from_rgb(50, 60, 80)));
                        p.line_segment([Pos2::new(eq_rect.min.x, mid_y - 30.0), Pos2::new(eq_rect.max.x, mid_y - 30.0)], Stroke::new(0.5_f32, Color32::from_rgb(35, 42, 56)));
                        p.line_segment([Pos2::new(eq_rect.min.x, mid_y + 30.0), Pos2::new(eq_rect.max.x, mid_y + 30.0)], Stroke::new(0.5_f32, Color32::from_rgb(35, 42, 56)));

                        p.text(Pos2::new(eq_rect.min.x + 6.0, mid_y - 30.0), egui::Align2::LEFT_CENTER, "+6 dB", egui::FontId::proportional(9.0), Color32::from_rgb(100, 110, 130));
                        p.text(Pos2::new(eq_rect.min.x + 6.0, mid_y), egui::Align2::LEFT_CENTER, "0 dB", egui::FontId::proportional(9.0), Color32::from_rgb(120, 140, 160));
                        p.text(Pos2::new(eq_rect.min.x + 6.0, mid_y + 30.0), egui::Align2::LEFT_CENTER, "-6 dB", egui::FontId::proportional(9.0), Color32::from_rgb(100, 110, 130));

                        // Frequency Grid Markers
                        let freq_x = |f: f32| -> f32 {
                            let min_log = 20.0_f32.log10();
                            let max_log = 20000.0_f32.log10();
                            let norm = (f.log10() - min_log) / (max_log - min_log);
                            eq_rect.min.x + norm * eq_rect.width()
                        };

                        for &(freq, lbl) in &[(100.0, "100Hz"), (1000.0, "1kHz"), (10000.0, "10kHz")] {
                            let fx = freq_x(freq);
                            p.line_segment([Pos2::new(fx, eq_rect.min.y), Pos2::new(fx, eq_rect.max.y)], Stroke::new(0.5_f32, Color32::from_rgb(30, 36, 50)));
                            p.text(Pos2::new(fx, eq_rect.max.y - 8.0), egui::Align2::CENTER_CENTER, lbl, egui::FontId::proportional(9.0), Color32::from_rgb(90, 100, 120));
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
                                p.line_segment([pair[0], pair[1]], Stroke::new(2.0_f32, Theme::FL_CYAN));
                            }

                            // Draw Control Points for Low, Mid, High
                            let low_pt = Pos2::new(freq_x(track.eq.low_freq), mid_y - track.eq.low_gain_db * 5.0);
                            let mid_pt = Pos2::new(freq_x(track.eq.mid_freq), mid_y - track.eq.mid_gain_db * 5.0);
                            let high_pt = Pos2::new(freq_x(track.eq.high_freq), mid_y - track.eq.high_gain_db * 5.0);

                            p.circle_filled(low_pt, 4.5, Theme::FL_ORANGE);
                            p.circle_filled(mid_pt, 4.5, Theme::FL_GREEN);
                            p.circle_filled(high_pt, 4.5, Color32::from_rgb(180, 110, 255));
                        }

                        ui.add_space(6.0);

                        // EQ Knobs & Sliders Row
                        ui.horizontal(|ui| {
                            // Low Band
                            ui.group(|ui| {
                                ui.set_width(220.0);
                                ui.label(egui::RichText::new(crate::i18n::t("🔊 Bas (Low Shelf)")).strong().color(Theme::FL_ORANGE));
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
                                ui.label(egui::RichText::new(crate::i18n::t("🎙 Mellanregister (Mid Peak)")).strong().color(Theme::FL_GREEN));
                                ui.add(egui::Slider::new(&mut track.eq.mid_gain_db, -12.0..=12.0).text("Gain (dB)").suffix(" dB"));
                                ui.add(egui::Slider::new(&mut track.eq.mid_freq, 200.0..=6000.0).text("Frekvens").suffix(" Hz"));
                                ui.add(egui::Slider::new(&mut track.eq.mid_q, 0.5..=3.0).text("Q (Bredd)"));
                            });

                            // High Band
                            ui.group(|ui| {
                                ui.set_width(220.0);
                                ui.label(egui::RichText::new(crate::i18n::t("✨ Diskant (High Shelf)")).strong().color(Color32::from_rgb(180, 110, 255)));
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
                            ui.label(egui::RichText::new(crate::i18n::t("Förinställningar:")).size(10.5).color(Theme::TEXT_MUTED));
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
                        ui.label(egui::RichText::new(crate::i18n::t("⏱ Fading & Fade-kurvor (Exakt hundradels precision)")).strong().color(Theme::FL_ORANGE));
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
                                ui.label(egui::RichText::new(crate::i18n::t("📈 Fade In (In-toning)")).strong().size(12.0).color(Theme::FL_CYAN));
                                let cs_in = (global_fade_in_sec * 100.0).round() as i32;
                                ui.label(egui::RichText::new(crate::tstatus!("Längd: {:.2} sekunder  ({} hundradelar)", global_fade_in_sec, cs_in)).strong().color(Theme::TEXT_BRIGHT));

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
                                ui.label(egui::RichText::new(crate::i18n::t("📉 Fade Out (Ut-toning)")).strong().size(12.0).color(Theme::FL_ORANGE));
                                let cs_out = (global_fade_out_sec * 100.0).round() as i32;
                                ui.label(egui::RichText::new(crate::tstatus!("Längd: {:.2} sekunder  ({} hundradelar)", global_fade_out_sec, cs_out)).strong().color(Theme::TEXT_BRIGHT));

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

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("Tonhöjd (Pitch Shift):")).size(11.0).color(Theme::TEXT_MUTED));
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
                        ui.label(egui::RichText::new(format!("🎵 Klipp & Regioner i {}", track.name)).strong().color(Theme::FL_ORANGE));
                        ui.add_space(4.0);

                        // Waveform Overview
                        let (wave_rect, wave_resp) = ui.allocate_exact_size(Vec2::new(ui.available_width().max(10.0), 90.0), Sense::click_and_drag());
                        let p = ui.painter();
                        p.rect_filled(wave_rect, Rounding::same(4.0), Color32::from_rgb(12, 15, 22));
                        p.rect_stroke(wave_rect, Rounding::same(4.0), Stroke::new(1.0_f32, Color32::from_rgb(32, 40, 56)));

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
                                    p.line_segment([Pos2::new(wx, mid_y - wh), Pos2::new(wx, mid_y + wh)], Stroke::new(1.0_f32, Color32::WHITE));
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
                                    ui.label(egui::RichText::new(format!("#{}: {}", ri + 1, r.name)).strong().color(Theme::TEXT_BRIGHT));
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

impl SonixApp {
pub(crate) fn render_help_manual_modal(&mut self, ctx: &egui::Context) {
    if !self.show_help_guide {
        return;
    }

    let mut close = false;
    let mut open = self.show_help_guide;

    egui::Window::new(crate::i18n::t("📖 Sonix Studio – Komplett Bruksanvisning & Master Manual"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(920.0, 620.0))
        .min_size(Vec2::new(760.0, 480.0))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            // Top Header with quick search and close
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(crate::i18n::t("📖 SONIX STUDIO MASTER MANUAL")).strong().size(14.0).color(Theme::FL_ORANGE));
                ui.label(egui::RichText::new(crate::i18n::t("• Komplett Referensguide & Handbok")).size(11.0).color(Theme::FL_CYAN));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("✖ Stäng Manual")).strong().size(11.0).color(Color32::BLACK)).fill(Theme::FL_ORANGE)).clicked() {
                        close = true;
                    }
                });
            });

            ui.separator();

            // 2-Column Layout: Left Chapters Sidebar + Right Scrollable Documentation
            ui.horizontal(|ui| {
                // Left Chapter Navigation Sidebar
                ui.group(|ui| {
                    ui.set_width(230.0);
                    ui.set_height(ui.available_height());
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("KAPITEL & AVSNITT:")).strong().size(10.5).color(Theme::TEXT_MUTED));
                        ui.add_space(2.0);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(crate::i18n::t("🔍")).size(10.0));
                            ui.add(egui::TextEdit::singleline(&mut self.help_search_query).hint_text(crate::i18n::t("Sök i manualen...")).desired_width(170.0));
                        });
                        ui.add_space(4.0);

                        let chapters = [
                            (0, "🚀 1. Snabbstart & Översikt"),
                            (1, "⌨ 2. Tangentbord & Kommandon"),
                            (2, "📊 3. Tidslinje & 0.01s Snäpp"),
                            (3, "📦 4. Multi-Track Stämimport"),
                            (4, "🔍 5. Stämeditor & 3-Band EQ"),
                            (5, "🥁 6. Channel Rack & Beats"),
                            (6, "🎹 7. Piano Roll & Synth"),
                            (7, "🎤 8. Vocal Studio & Pitch"),
                            (8, "🎛 9. Mixer & WAV-Export"),
                            (9, "⚙ 10. PipeWire & Wayland"),
                        ];

                        egui::ScrollArea::vertical().id_salt("manual_nav_scroll").show(ui, |ui| {
                            for (ch_idx, title) in chapters {
                                let is_sel = self.help_manual_active_tab == ch_idx;
                                let bg = if is_sel { Theme::FL_CYAN } else { Color32::from_rgb(22, 26, 34) };
                                let fg = if is_sel { Color32::BLACK } else { Theme::TEXT_BRIGHT };

                                if ui.add_sized(Vec2::new(215.0, 26.0), egui::Button::new(egui::RichText::new(crate::i18n::t(title)).strong().size(10.5).color(fg)).fill(bg)).clicked() {
                                    self.help_manual_active_tab = ch_idx;
                                }
                                ui.add_space(2.0);
                            }
                        });
                    });
                });

                // Right Main Content Scroll Area
                ui.group(|ui| {
                    ui.set_width(ui.available_width());
                    ui.set_height(ui.available_height());

                    egui::ScrollArea::vertical()
                        .id_salt("manual_content_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            match self.help_manual_active_tab {
                                0 => {
                                    // 1. SNABBSTART & ÖVERSIKT
                                    ui.heading(egui::RichText::new(crate::i18n::t("🚀 1. Snabbstart & Översikt")).color(Theme::FL_ORANGE));
                                    ui.label(egui::RichText::new(crate::i18n::t("Välkommen till Sonix Studio – en blixtsnabb Digital Audio Workstation skapad för Linux med äkta realtidsprestanda.")).size(12.0).color(Theme::TEXT_BRIGHT));
                                    ui.add_space(8.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🎯 Typiskt produktionsarbetsflöde:")).strong().color(Theme::FL_CYAN));
                                        ui.add_space(4.0);
                                        ui.label(crate::i18n::t("1. Importera stämmor: Klicka på '📦 IMPORT STEMS' (Ctrl+I) för att läsa in ett ZIP-paket med sång, bas, trummor m.m."));
                                        ui.label(crate::i18n::t("2. Arrangera & Klipp: Använd saxverktyget (✂ Klipp) och zooma in djupt (Ctrl+Scroll) för att dela med 0.01s precision."));
                                        ui.label(crate::i18n::t("3. Fokusera & Förädla: Klicka på '🔍' på ett spår för att öppna Stämeditorn, isolera stämman med Solo och ratta 3-bands EQ."));
                                        ui.label(crate::i18n::t("4. Skapa trumkomp: Tryck F4 för att öppna Channel Rack och klicka in 16-stegs beats."));
                                        ui.label(crate::i18n::t("5. Spela in melodier: Tryck F5 för Piano Roll eller spela live med datortangentbordet."));
                                        ui.label(crate::i18n::t("6. Mixa & Exportera: Tryck F6 för Mixern och Ctrl+E för att exportera mastrad 48kHz WAV."));
                                    });

                                    ui.add_space(8.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("⚡ Systemarkitektur & Prestanda:")).strong().color(Theme::FL_GREEN));
                                        ui.label(crate::i18n::t("• 100% Rust Audio DSP – Inget hack, noll skräpsamling (Garbage Collection), noll latens."));
                                        ui.label(crate::i18n::t("• PipeWire & ALSA Native – Ansluter direkt till Linux moderna ljudserver."));
                                        ui.label(crate::i18n::t("• Asynkron bakgrundsavkodning – Gränssnittet fryser aldrig vid inläsning av tunga ljudfiler."));
                                    });
                                }

                                1 => {
                                    // 2. TANGENTBORD & KOMMANDON
                                    ui.heading(egui::RichText::new(crate::i18n::t("⌨ 2. Tangentbord & Komplett Kortkommandoreferens")).color(Theme::FL_ORANGE));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("NAVIGERING MELLAN VYER (Funktionstangenter):")).strong().color(Theme::FL_CYAN));
                                        ui.separator();
                                        let f_keys = [
                                            ("F1", "Bruksanvisning / Manual & Hjälpcenter"),
                                            ("F3", "Tidslinje / Multi-Track Arranger"),
                                            ("F4", "Sonix Channel Rack (16-stegs trummaskin)"),
                                            ("F5", "Piano Roll (Notinmatning & melodieditor)"),
                                            ("F6", "Mixer Console & Master Effektrack"),
                                            ("F7", "Analog Alchemy Synthesizer"),
                                            ("F8", "Vocal Studio (Melodyne & Harmonizer)"),
                                            ("F9", "AI Music Studio & Prompt Engine"),
                                            ("F10", "Modulär Synt & Patcher"),
                                        ];
                                        for (k, desc) in f_keys {
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new(format!("[ {:<3} ]", k)).monospace().strong().color(Theme::FL_ORANGE));
                                                ui.label(crate::i18n::t(desc));
                                            });
                                        }
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("PROJEKT & REDIGERINGSKOMMANDON:")).strong().color(Theme::FL_GREEN));
                                        ui.separator();
                                        let shortcuts = [
                                            ("Mellanslag (Space)", "Starta / Pausa uppspelning"),
                                            ("Ctrl + N", "Skapa nytt tomt projekt"),
                                            ("Ctrl + O / P", "Öppna Projektbläddrare"),
                                            ("Ctrl + S", "Spara projektfil"),
                                            ("Ctrl + I", "Importera Stämmor / Multi-Track Stems"),
                                            ("Ctrl + E", "Exportera projekt (WAV/FLAC/MP3/OGG/AAC)"),
                                            ("Del / Backspace", "Radera markerat ljudklipp"),
                                            ("Ctrl + Scroll", "Mjuk horisontell zoomning i tidslinjen"),
                                            ("A, W, S, E, D...", "Klaviatur – Spela synthen live med tangentbordet"),
                                        ];
                                        for (k, desc) in shortcuts {
                                            ui.horizontal(|ui| {
                                                ui.label(egui::RichText::new(format!("{:<18}", k)).monospace().strong().color(Theme::FL_CYAN));
                                                ui.label(crate::i18n::t(desc));
                                            });
                                        }
                                    });
                                }

                                2 => {
                                    // 3. TIDSLINJE & 0.01s PRECISION
                                    ui.heading(egui::RichText::new(crate::i18n::t("📊 3. Tidslinje, Snäpp & 0.01s Precision")).color(Theme::FL_ORANGE));
                                    ui.add_space(6.0);

                                    ui.label(egui::RichText::new(crate::i18n::t("Tidslinjen hanterar obegränsat med ljudspår och regioner med precision ned till 0.01 sekunder (hundradelar).")).size(11.5).color(Theme::TEXT_BRIGHT));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🔍 Dynamisk 4-Nivåers Tidslinjelinjal:")).strong().color(Theme::FL_CYAN));
                                        ui.label(crate::i18n::t("1. Takter (Bars): Stora vertikala streck med taktnummer (1, 2, 3...) och tidskod (MM:SS.cs)."));
                                        ui.label(crate::i18n::t("2. Beats: Fjärdedelstikar (.2, .3, .4) som syns vid normal zoom."));
                                        ui.label(crate::i18n::t("3. 1/16-delssteg: Tunt rutnät för exakt rytmisk klippning och placering."));
                                        ui.label(crate::i18n::t("4. Hundradelar (0.01s): Aktiveras vid djup inzoomning (200%–800%) för millimeterexakta snitt i sång och trummor."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("✂ Redigeringsverktyg & Snäpp:")).strong().color(Theme::FL_ORANGE));
                                        ui.label(crate::i18n::t("• ⇱ Välj: Klicka på en region för att markera och se dess egenskaper i inspektorn."));
                                        ui.label(crate::i18n::t("• ✎ Rita: Klicka i tidslinjen för att rita ut mönster och aktiva klipp."));
                                        ui.label(crate::i18n::t("• ✂ Klipp (0.01s): Saxverktyg. Klicka var som helst på ett ljudspår för att klyva klippet i två."));
                                        ui.label(crate::i18n::t("• 🔇 Muta / 🗑 Radera: Tysta eller radera regioner direkt med ett klick."));
                                        ui.separator();
                                        ui.label(crate::i18n::t("• Snäppläge '⚡ 0.01s (Fri)': Frikopplar från musikaliska takter och låter dig klippa med 10ms precision."));
                                        ui.label(crate::i18n::t("• Snäpplägen 1/16, Beat, Takt: Snäpper automatiskt till det musikaliska tempot (BPM)."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🏃 Följ Tidslinje (Auto-Scroll):")).strong().color(Theme::FL_GREEN));
                                        ui.label(crate::i18n::t("• '🏃 Följ tidslinje: PÅ': Tidslinjen rullar automatiskt och håller spelhuvudet centrerat på skärmen."));
                                        ui.label(crate::i18n::t("• '⏸ Följ tidslinje: AV': Tidslinjen står stilla så att du kan redigera i lugn och ro medan låten spelar."));
                                    });
                                }

                                3 => {
                                    // 4. MULTI-TRACK STÄMIMPORT
                                    ui.heading(egui::RichText::new(crate::i18n::t("📦 4. Multi-Track Stämimport (Stems)")).color(Theme::FL_ORANGE));
                                    ui.add_space(6.0);

                                    ui.label(egui::RichText::new(crate::i18n::t("Importera kompletta stämpaket (WAV, MP3, FLAC, OGG) från Suno AI, FL Studio, Ableton, Logic m.fl.")).size(11.5).color(Theme::TEXT_BRIGHT));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("📥 Hur du importerar:")).strong().color(Theme::FL_CYAN));
                                        ui.label(crate::i18n::t("1. Tryck '📦 IMPORT STEMS' i verktygsfältet eller tryck Ctrl+I."));
                                        ui.label(crate::i18n::t("2. Välj bland automatiskt upptäckta paket i ~/Music / ~/Downloads eller välj ZIP-fil/mapp manuellt."));
                                        ui.label(crate::i18n::t("3. Dra & Släpp (Drag & Drop): Dra en .zip-fil direkt in i Sonix-fönstret."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🎛 Automatisk Spåridentifiering & Routing:")).strong().color(Theme::FL_GREEN));
                                        ui.label(crate::i18n::t("• Lead Vocals ➔ 🎙 Sångbuss med lila färgkod."));
                                        ui.label(crate::i18n::t("• Backing Vocals ➔ 🗣 Körbuss."));
                                        ui.label(crate::i18n::t("• Drums / Kick / Snare ➔ 🥁 Trumbuss med cyan färgkod."));
                                        ui.label(crate::i18n::t("• Bass ➔ 🎸 Basbuss med gul färgkod."));
                                        ui.label(crate::i18n::t("• Guitar / Keys / Synth ➔ 🎹 Synthbuss med grön/orange färgkod."));
                                        ui.label(crate::i18n::t("• FX / Other ➔ ✨ Effektsändning."));
                                        ui.separator();
                                        ui.label(crate::i18n::t("Inläsningen sker i bakgrunden med en förloppsindikator utan att programmet hänger sig."));
                                    });
                                }

                                4 => {
                                    // 5. STÄMEDITOR & 3-BANDS EQ
                                    ui.heading(egui::RichText::new(crate::i18n::t("🔍 5. Dedikerad Stämeditor & Ljudfokus")).color(Theme::FL_ORANGE));
                                    ui.add_space(6.0);

                                    ui.label(egui::RichText::new(crate::i18n::t("Fokusera på en enda stämma med högprecisionskontroller, decibelnivåer, grafisk EQ och fading.")).size(11.5).color(Theme::TEXT_BRIGHT));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🎧 1. Isolera stämma (Solo On):")).strong().color(Theme::FL_ORANGE));
                                        ui.label(crate::i18n::t("Klicka på den stora knappen '🎧 ISOLERA STÄMMA (SOLO)' högst upp i editorn för att direkt tysta alla andra spår och lyssna enbart på den valda stämman."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("🎚 2. Volym, Pan & Dynamik (Flik 1):")).strong().color(Theme::FL_CYAN));
                                        ui.label(crate::i18n::t("• Volymreglage med exakt dB-visning och '±0.1 dB' finjusteringsknappar."));
                                        ui.label(crate::i18n::t("• Stereopanorering med snabbcentrering ('Center')."));
                                        ui.label(crate::i18n::t("• Kompressor: Justerbar Threshold (-30 dB till 0 dB) och Ratio (1:1 till 8:1)."));
                                        ui.label(crate::i18n::t("• Pitch Shifter: Transponera stämman ±12 halvtoner med bevarade formanter (1-centprecision, Shift = finjustering, dubbelklick = nollställ)."));
                                        ui.label(crate::i18n::t("• Reverb & Delay sends."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("📈 3. 3-Bands Grafisk Parametrisk EQ (Flik 2):")).strong().color(Theme::FL_GREEN));
                                        ui.label(crate::i18n::t("• Interaktiv frekvenskurva i realtid (20 Hz – 20 kHz) med dB-skala."));
                                        ui.label(crate::i18n::t("• Low Shelf (Bas): Gain ±12 dB, brytfrekvens 40–400 Hz."));
                                        ui.label(crate::i18n::t("• Mid Peak (Mellanregister): Gain ±12 dB, frekvens 200 Hz – 6 kHz, Q-faktor 0.5–3.0."));
                                        ui.label(crate::i18n::t("• High Shelf (Diskant): Gain ±12 dB, frekvens 3 kHz – 16 kHz."));
                                        ui.label(crate::i18n::t("• Snabbpresets för Sång, Trummor, Gitarr, Bas och Flat nollställning."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("⏱ 4. Fading & Envelope i hundradelar (Flik 3):")).strong().color(Color32::from_rgb(180, 110, 255)));
                                        ui.label(crate::i18n::t("• Fade In och Fade Out med 0.01s precision."));
                                        ui.label(crate::i18n::t("• Stegknappar för '±0.01s' och '±0.10s'. Snabbval för 20ms, 50ms, 100ms, 500ms, 1.00s."));
                                        ui.label(crate::i18n::t("• Slå på alla: Sätter samma fading på alla klipp i just den stämman."));
                                    });
                                }

                                5 => {
                                    // 6. CHANNEL RACK & BEATS
                                    ui.heading(egui::RichText::new(crate::i18n::t("🥁 6. Sonix Channel Rack & Stegsequencer (F4)")).color(Theme::FL_ORANGE));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("16-Stegs Trummaskin & Mönster:")).strong().color(Theme::FL_CYAN));
                                        ui.label(crate::i18n::t("• 8 Klassiska trumkanaler: Kick, Snare, Clap, Closed Hat, Open Hat, Crash, 303 Bass och Lead."));
                                        ui.label(crate::i18n::t("• Mönster P1–P4: Skapa variationer för vers, refräng och stick."));
                                        ui.label(crate::i18n::t("• Swing: Skjutreglage för att ge trummorna ett naturligt sväng."));
                                        ui.label(crate::i18n::t("• ⚡ Slumpa Beats: Genererar omedelbart nya inspirerande trumkomp."));
                                        ui.label(crate::i18n::t("• Velocity-redigering: Justera anslagskraften per steg i den nedre velocity-raden."));
                                    });
                                }

                                6 => {
                                    // 7. PIANO ROLL & SYNTH
                                    ui.heading(egui::RichText::new(crate::i18n::t("🎹 7. Piano Roll & Analog Synthesizer (F5 / F7)")).color(Theme::FL_ORANGE));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Piano Roll (F5):")).strong().color(Theme::FL_CYAN));
                                        ui.label(crate::i18n::t("• Grafiskt 3-oktavigt notinmatningsfönster (C3 till B5)."));
                                        ui.label(crate::i18n::t("• Klicka på klaviaturet till vänster för att provlyssna toner i realtid."));
                                        ui.label(crate::i18n::t("• ⚡ Slumpa Melodi: Skapar harmoniska melodislingor automatiskt."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Analog Alchemy Synthesizer (F7):")).strong().color(Theme::FL_GREEN));
                                        ui.label(crate::i18n::t("• 4 Vågformer: Sawtooth (sågtand), Square (fyrkant), Sine (sinus) och Noise (brus)."));
                                        ui.label(crate::i18n::t("• ADSR Envelope: Attack, Decay, Sustain och Release."));
                                        ui.label(crate::i18n::t("• Resonant SVF-filter (12 dB): Cutoff (20Hz–18kHz) och Resonans."));
                                        ui.label(crate::i18n::t("• Saturation & Drive: Analog rörvärme och distortion."));
                                    });
                                }

                                7 => {
                                    // 8. VOCAL STUDIO & PITCH
                                    ui.heading(egui::RichText::new(crate::i18n::t("🎤 8. Vocal Studio, Melodyne & Harmonizer (F8)")).color(Theme::FL_ORANGE));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Funktioner i Vocal Studio:")).strong().color(Theme::FL_CYAN));
                                        ui.label(crate::i18n::t("• Melodyne ARA2 Editor: Interaktiva tonhöjds-blobs för att justera sångens toner och timing."));
                                        ui.label(crate::i18n::t("• 4-Voice Harmonizer: Skapar fylliga sångarrangemang med kör, oktavdubbling eller vocoder."));
                                        ui.label(crate::i18n::t("• Comping & Tagningar: Spela in flera sångtagningar och klipp ihop den bästa versionen."));
                                        ui.label(crate::i18n::t("• Sampling & Recorder: Spela in egna ljud, klappar och instrument med din mikrofon."));
                                    });
                                }

                                8 => {
                                    // 9. MIXER & WAV-EXPORT
                                    ui.heading(egui::RichText::new(crate::i18n::t("🎛 9. Mixer Console, Effekter & WAV-Export (F6 / Ctrl+E)")).color(Theme::FL_ORANGE));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Mixerbord & Effektrack (F6):")).strong().color(Theme::FL_CYAN));
                                        ui.label(crate::i18n::t("• 8 Stereokanaler + Master Bus med analoga faders och VU peak meters."));
                                        ui.label(crate::i18n::t("• 3-Bands Parametrisk EQ per mixerkanal."));
                                        ui.label(crate::i18n::t("• Master Limiter & Maximizer för kommersiell ljudstyrka utan digital distorsion."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Export & Rendering (Ctrl+E):")).strong().color(Theme::FL_GREEN));
                                        ui.label(crate::i18n::t("• Export Song: Renderar hela tidslinjen till en sammanslagen masterfil."));
                                        ui.label(crate::i18n::t("• Export Pattern: Exporterar det aktiva Channel Rack-mönstret."));
                                        ui.label(crate::i18n::t("• 📤 BATCH EXPORT: Exporterar alla aktiva stämmor som separata WAV-filer till ./exports/."));
                                        ui.label(crate::i18n::t("• Format: 32-bit float / 24-bit PCM WAV i 44.1 kHz eller 48 kHz."));
                                    });
                                }

                                9 => {
                                    // 10. PIPEWIRE & WAYLAND SETUP
                                    ui.heading(egui::RichText::new(crate::i18n::t("⚙ 10. Ljudmotor, PipeWire & Hyprland / Wayland")).color(Theme::FL_ORANGE));
                                    ui.add_space(6.0);

                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Ljudmotor (PipeWire / ALSA / JACK):")).strong().color(Theme::FL_CYAN));
                                        ui.label(crate::i18n::t("• Sonix kommunicerar direkt med Linux professionella ljudserver i realtid."));
                                        ui.label(crate::i18n::t("• Buffertstorlek: 128/256 samples för noll latens vid live-spelning, 512/1024 samples för tunga projekt."));
                                    });

                                    ui.add_space(6.0);
                                    ui.group(|ui| {
                                        ui.label(egui::RichText::new(crate::i18n::t("Linux Wayland & Hyprland Säkerhet:")).strong().color(Theme::FL_GREEN));
                                        ui.label(crate::i18n::t("• Sonix är helt anpassat för Hyprland, GNOME Wayland och KDE."));
                                        ui.label(crate::i18n::t("• När fönstret täcks av ett annat fönster eller minimeras fortsätter ljudmotorn att spela utan avbrott samtidigt som GUI-uppritningen vilar för att förhindra krascher."));
                                        ui.label(crate::i18n::t("• Inbyggd Crash Logger sparar automatiskt eventuella problem till /tmp/sonix_crash.log."));
                                    });
                                }

                                _ => {}
                            }
                        });
                });
            });
        });

    if close || !open {
        self.show_help_guide = false;
    }
}
}

impl SonixApp {
    /// Alla dialoger, i den ordning de läggs ovanpå varandra.
    pub(crate) fn render_all_modals(&mut self, ctx: &egui::Context) {
        // 4. Modals & Dialogs
        self.render_import_sample_modal(ctx);
        self.render_hardware_controller_modal(ctx);
        self.render_batch_export_modal(ctx);
        self.render_suno_stem_import_modal(ctx);
        self.render_wav_question_modal(ctx);
        self.render_stem_import_progress_modal(ctx);
        self.render_project_load_progress_modal(ctx);
        self.render_about_modal(ctx);
        self.render_midi_import_modal(ctx);
        self.render_recovery_modal(ctx);
        self.render_project_manager_modal(ctx);
        self.render_ai_settings_modal(ctx);
        self.render_audio_settings_modal(ctx);
        self.render_mic_settings_modal(ctx);
        self.render_chord_generator_modal_view(ctx);
        self.render_tuner_modal_view(ctx);
        self.render_dice_generator_modal_view(ctx);
        self.render_fx_rack_modal_view(ctx);
        self.render_tempo_modal(ctx);
        self.render_song_structure_modal_view(ctx);
        self.render_stem_focus_modal(ctx);
        self.render_help_manual_modal(ctx);
        self.render_add_track_modal(ctx);
    }
}
