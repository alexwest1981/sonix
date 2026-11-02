//! Paneldialoger: lägg till spår, hårdvarukontroller och små fönster.
//!
//! Utdelad ur `modals.rs` 2026-11-02; filen var 4 486 rader och höll 34 dialoger.

use super::*;
use crate::ui::theme::Theme;

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
            ui.label(egui::RichText::new(crate::i18n::t("Direkt hårdvaruintegration med Mackie Control Universal (MCU) och Open Sound Control (OSC)")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
            ui.add_space(8.0);

            // Section 1: Mackie MCU
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🎚 MACKIE CONTROL UNIVERSAL (MCU)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (status_txt, status_col) = if self.mcu_input.is_some() {
                            (crate::i18n::t("● ALSA SEQUENCER-PORT ÖPPEN"), crate::ui::theme::palette().green())
                        } else {
                            (crate::i18n::t("○ FRÅNKOPPLAD"), crate::ui::theme::palette().text_muted())
                        };
                        ui.label(egui::RichText::new(status_txt).strong().size(Theme::FONT_S).color(status_col));
                    });
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Enhet:"));
                    ui.label(egui::RichText::new(&self.mcu_device_name).strong().color(crate::ui::theme::palette().cyan()));
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
                        ui.label(egui::RichText::new(format!("{} events", self.mcu_input.as_ref().map(|m| m.received()).unwrap_or(0))).size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                    }
                });

                // BCF2000 / FaderPort: raw MIDI bytes through the pure parser, sent on the
                // same control channel the MCU thread writes to.
                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Kontrollyta:"));
                    for kind in [
                        SurfaceKind::Mcu,
                        SurfaceKind::Bcf2000,
                        SurfaceKind::FaderPort,
                        SurfaceKind::BabyHui,
                    ] {
                        if ui.selectable_label(self.surface_kind == kind, kind.label()).clicked() {
                            self.surface_kind = kind;
                            self.surface_input = None;
                            self.surface_feedback_sent = None;
                            if kind != SurfaceKind::Mcu {
                                match SurfaceInput::connect(kind, self.control_tx.clone()) {
                                    Ok(s) => {
                                        self.surface_input = Some(s);
                                        self.status_message = crate::tstatus!(
                                            "✔ {} ansluten — fadrar och transport styr mixern.",
                                            kind.label()
                                        );
                                    }
                                    Err(e) => {
                                        self.status_message = crate::tstatus!("⚠ {}", e);
                                    }
                                }
                            }
                        }
                    }
                    if let Some(s) = &self.surface_input {
                        ui.label(egui::RichText::new(crate::tstatus!("{} händelser", s.received())).size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
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
                ui.label(egui::RichText::new(crate::i18n::t("Motorfaders feedback status:")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
                ui.horizontal(|ui| {
                    for i in 0..8 {
                        let ch_vol = if i < self.playlist_tracks.len() { self.playlist_tracks[i].volume } else if i < self.channels.len() { self.channels[i].volume } else { 0.8 };
                        ui.vertical_centered(|ui| {
                            ui.label(egui::RichText::new(format!("CH{}", i + 1)).size(Theme::FONT_XS));
                            let (rect, _) = ui.allocate_exact_size(Vec2::new(12.0, 36.0), Sense::hover());
                            ui.painter().rect_filled(rect, Rounding::same(2.0), crate::ui::theme::palette().bg());
                            let h = (ch_vol * 34.0).clamp(0.0, 34.0);
                            let fill_rect = Rect::from_min_max(Pos2::new(rect.min.x + 1.0, rect.max.y - h), Pos2::new(rect.max.x - 1.0, rect.max.y - 1.0));
                            ui.painter().rect_filled(fill_rect, Rounding::same(1.0), crate::ui::theme::palette().orange());
                        });
                    }
                });

                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    let learn_txt = if self.midi_learn_active { "🔴 MIDI LEARN: LYSSNAR..." } else { "🎯 Aktivera MIDI Learn" };
                    let learn_bg = if self.midi_learn_active { Color32::RED } else { crate::ui::theme::palette().panel() };
                    if ui.add(egui::Button::new(egui::RichText::new(learn_txt).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(learn_bg)).clicked() {
                        self.midi_learn_active = !self.midi_learn_active;
                    }
                });
            });

            ui.add_space(8.0);

            // Section 1b: MIDI Keyboard (Piano Roll input) — Fas 5.3
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🎹 MIDI-KLAVIATUR (Piano Roll)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let (status_txt, status_col) = if self.midi_keyboard_connected {
                            (crate::i18n::t("● MIDI-IN-PORT ÖPPEN"), crate::ui::theme::palette().green())
                        } else {
                            (crate::i18n::t("○ FRÅNKOPPLAD"), crate::ui::theme::palette().text_muted())
                        };
                        ui.label(egui::RichText::new(status_txt).strong().size(Theme::FONT_S).color(status_col));
                    });
                });
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label(crate::i18n::t("Enhet:"));
                    ui.label(egui::RichText::new(&self.midi_device_name).strong().color(crate::ui::theme::palette().cyan()));
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
                        ui.label(egui::RichText::new(crate::tstatus!("{} nothändelser", self.midi_note_count)).size(Theme::FONT_S).color(crate::ui::theme::palette().green()));
                    }
                });

                ui.separator();
                // **MIDI-kartan** (FL:s Keyboard Splitter / Key Mapper / Level Scaler):
                // fyra zoner, första träffen vinner. Kanal 0 = behåll notens kanal.
                egui::CollapsingHeader::new(
                    egui::RichText::new(crate::i18n::t("🎛 MIDI-KARTA (splitter)"))
                        .strong()
                        .size(Theme::FONT_S)
                        .color(crate::ui::theme::palette().green()),
                )
                .default_open(self.midi_map.enabled)
                .show(ui, |ui| {
                    ui.checkbox(&mut self.midi_map.enabled, crate::i18n::t("Aktivera zoner"));
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Ackord"));
                        let current =
                            crate::audio::key_mapper::CHORD_MODES[(self.midi_map.chord as usize)
                                .min(crate::audio::key_mapper::CHORD_MODES.len() - 1)]
                            .0;
                        egui::ComboBox::from_id_salt("midi_chord")
                            .selected_text(crate::i18n::t(current))
                            .show_ui(ui, |ui| {
                                for (idx, (name, _)) in
                                    crate::audio::key_mapper::CHORD_MODES.iter().enumerate()
                                {
                                    if ui
                                        .selectable_label(
                                            self.midi_map.chord as usize == idx,
                                            crate::i18n::t(name),
                                        )
                                        .clicked()
                                    {
                                        self.midi_map.chord = idx as u8;
                                    }
                                }
                            });
                    });
                    // Notlängd, eko och slumpat anslag — samma transformblock som zonerna.
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Notlängd"));
                        ui.add(
                            egui::DragValue::new(&mut self.midi_map.length_ms)
                                .range(0..=4000)
                                .speed(1.0)
                                .suffix(" ms"),
                        );
                        ui.label(crate::i18n::t("Eko"));
                        ui.add(
                            egui::DragValue::new(&mut self.midi_map.delay_ms)
                                .range(0..=1000)
                                .speed(1.0)
                                .suffix(" ms"),
                        );
                        ui.label("×");
                        ui.add(
                            egui::DragValue::new(&mut self.midi_map.echo_repeats)
                                .range(0..=crate::audio::key_mapper::MAX_ECHO_REPEATS)
                                .speed(0.1),
                        );
                        ui.label(crate::i18n::t("Slump"));
                        ui.add(
                            egui::DragValue::new(&mut self.midi_map.random_velocity)
                                .range(0.0..=1.0)
                                .speed(0.01)
                                .fixed_decimals(2),
                        );
                    });
                    for (i, zone) in self.midi_map.zones.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut zone.active, crate::tstatus!("Zon {}", i + 1));
                            ui.add(
                                egui::DragValue::new(&mut zone.low)
                                    .range(0..=127)
                                    .speed(1.0),
                            );
                            ui.label("–");
                            ui.add(
                                egui::DragValue::new(&mut zone.high)
                                    .range(0..=127)
                                    .speed(1.0),
                            );
                            ui.label(crate::i18n::t("Transp."));
                            ui.add(
                                egui::DragValue::new(&mut zone.transpose)
                                    .range(-48..=48)
                                    .speed(1.0),
                            );
                            ui.label(crate::i18n::t("Kanal"));
                            let mut channel = zone.channel.unwrap_or(0);
                            if ui
                                .add(
                                    egui::DragValue::new(&mut channel)
                                        .range(0..=16)
                                        .speed(1.0),
                                )
                                .changed()
                            {
                                zone.channel = if channel == 0 { None } else { Some(channel) };
                            }
                            ui.label(crate::i18n::t("Anslag"));
                            ui.add(
                                egui::DragValue::new(&mut zone.velocity_scale)
                                    .range(0.0..=2.0)
                                    .speed(0.01)
                                    .fixed_decimals(2),
                            );
                            ui.label(crate::i18n::t("Kurva"));
                            ui.add(
                                egui::DragValue::new(&mut zone.curve)
                                    .range(0.2..=5.0)
                                    .speed(0.01)
                                    .fixed_decimals(2),
                            );
                            // **Not → not**: kvantisera zonen till en skala.
                            let current = zone
                                .snap_scale
                                .map(|s| crate::audio::scale::SCALES[s as usize % crate::audio::scale::SCALES.len()].name)
                                .unwrap_or("–");
                            egui::ComboBox::from_id_salt(("zone_scale", i))
                                .selected_text(current)
                                .width(90.0)
                                .show_ui(ui, |ui| {
                                    if ui.selectable_label(zone.snap_scale.is_none(), "–").clicked() {
                                        zone.snap_scale = None;
                                    }
                                    for (si, scale) in crate::audio::scale::SCALES.iter().enumerate() {
                                        if ui
                                            .selectable_label(
                                                zone.snap_scale == Some(si as u8),
                                                scale.name,
                                            )
                                            .clicked()
                                        {
                                            zone.snap_scale = Some(si as u8);
                                        }
                                    }
                                });
                        });
                    }
                    ui.label(
                        egui::RichText::new(crate::i18n::t(
                            "Första zonen som täcker noten vinner. Kanal 0 = behåll notens kanal. Skalan kvantiserar noten (not → not).",
                        ))
                        .size(Theme::FONT_XS)
                        .color(crate::ui::theme::palette().text_muted()),
                    );
                });

                ui.separator();
                ui.label(egui::RichText::new(crate::i18n::t("🎹 MIDI-UT (extern synt)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().orange()));
                // Utgången tas ut ur fältet medan den används: `blip` lånar den bara som
                // `&`, men "koppla från" skriver fältet — och då kan båda inte låna `self`.
                let mut midi_out = self.midi_out.take();
                let mut disconnect = false;
                if let Some(out) = &midi_out {
                    let name = out.name().to_string();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(crate::tstatus!("● {}", name)).color(crate::ui::theme::palette().green()));
                        if ui.button(crate::i18n::t("🎵 Testnot")).clicked() {
                            match out.blip(0, 60, 100) {
                                Ok(()) => {
                                    self.status_message =
                                        crate::tstatus!("🎵 Testnot (C4) skickad till '{}'", name);
                                }
                                Err(e) => {
                                    self.status_message =
                                        crate::tstatus!("⚠ Kunde inte skicka MIDI: {}", e);
                                }
                            }
                        }
                        if ui.button(crate::i18n::t("🔌 Koppla från MIDI-ut")).clicked() {
                            disconnect = true;
                        }
                    });
                } else {
                    let ports = crate::audio::midi_output::MidiOut::ports();
                    if ports.is_empty() {
                        ui.label(
                            egui::RichText::new(crate::i18n::t("Inga MIDI-ut-portar hittades."))
                                .size(Theme::FONT_S)
                                .color(crate::ui::theme::palette().text_muted()),
                        );
                    }
                    for (i, name) in ports.iter().enumerate() {
                        if ui.button(crate::tstatus!("🔌 {}", name)).clicked() {
                            match crate::audio::midi_output::MidiOut::open(i) {
                                Ok(out) => {
                                    self.status_message =
                                        crate::tstatus!("✔ MIDI-ut öppnad: '{}'", name);
                                    midi_out = Some(out);
                                }
                                Err(e) => {
                                    self.status_message =
                                        crate::tstatus!("⚠ Kunde inte öppna MIDI-ut: {}", e);
                                }
                            }
                        }
                    }
                }
                if disconnect {
                    midi_out = None;
                    self.status_message = crate::i18n::t("MIDI-ut frånkopplad.").to_string();
                }
                self.midi_out = midi_out;

                // **Patch names from a `.ins`** (Cakewalk instrument definition): one button,
                // one file dialog. Without it the program change below can only say a number.
                ui.horizontal(|ui| {
                    if ui
                        .button(crate::i18n::t("🎛 Load .ins…"))
                        .on_hover_text(crate::i18n::t(
                            "Instrument definition: gives the external synth's programs their names.",
                        ))
                        .clicked()
                        && let Some(path) =
                            rfd::FileDialog::new().add_filter(".ins", &["ins"]).pick_file()
                    {
                        match crate::audio::ins::load(&path.to_string_lossy()) {
                            Ok(ins) => {
                                let patches: usize =
                                    ins.banks.iter().map(|b| b.patches.len()).sum();
                                let named = crate::audio::ins::program_name(
                                    &ins,
                                    self.midi_bank,
                                    self.midi_program,
                                )
                                .map(|n| format!(" — {n}"))
                                .unwrap_or_default();
                                self.status_message = crate::tstatus!(
                                    "✔ {}: {} banks, {} patches{}",
                                    ins.name,
                                    ins.banks.len(),
                                    patches,
                                    named
                                );
                                self.ins = Some(ins);
                            }
                            Err(e) => {
                                self.status_message =
                                    crate::tstatus!("⚠ Could not read .ins: {}", e)
                            }
                        }
                    }
                    if let Some(ins) = &self.ins {
                        ui.label(
                            egui::RichText::new(&ins.name)
                                .size(Theme::FONT_S)
                                .color(crate::ui::theme::palette().cyan()),
                        );
                    }
                });
                if self.ins.is_some() {
                    let mut patch_changed = false;
                    ui.horizontal(|ui| {
                        ui.label(crate::i18n::t("Bank"));
                        patch_changed |= ui
                            .add(egui::DragValue::new(&mut self.midi_bank).range(0..=15))
                            .changed();
                        ui.label(crate::i18n::t("Program"));
                        patch_changed |= ui
                            .add(egui::DragValue::new(&mut self.midi_program).range(0..=127))
                            .changed();
                        let name = self
                            .ins
                            .as_ref()
                            .and_then(|i| {
                                crate::audio::ins::program_name(
                                    i,
                                    self.midi_bank,
                                    self.midi_program,
                                )
                            })
                            .map(str::to_string)
                            .unwrap_or_else(|| "?".to_string());
                        ui.label(
                            egui::RichText::new(name)
                                .size(Theme::FONT_S)
                                .color(crate::ui::theme::palette().text()),
                        );
                    });
                    if patch_changed {
                        // shortcut: the bank is a name lookup only, no bank-select CC0/CC32 is
                        // sent — add it when a synth's own bank numbers are known.
                        let sent = self
                            .midi_out
                            .as_ref()
                            .map(|out| out.program_change(0, self.midi_program));
                        let name = self
                            .ins
                            .as_ref()
                            .and_then(|i| {
                                crate::audio::ins::program_name(
                                    i,
                                    self.midi_bank,
                                    self.midi_program,
                                )
                            })
                            .map(str::to_string)
                            .unwrap_or_else(|| "?".to_string());
                        self.status_message = match sent {
                            Some(Err(e)) => {
                                crate::tstatus!("⚠ Program change failed: {}", e)
                            }
                            _ => crate::tstatus!(
                                "🎹 Bank {} / program {}: {}",
                                self.midi_bank,
                                self.midi_program,
                                name
                            ),
                        };
                    }
                }

                ui.add_space(4.0);
                let rec_bg = if self.midi_record_armed { crate::ui::theme::palette().red() } else { crate::ui::theme::palette().panel() };
                let rec_txt = if self.midi_record_armed {
                    crate::i18n::t("🔴 MIDI-REC: ARMED (spelar in i Piano Roll)")
                } else {
                    crate::i18n::t("⏺ Armera MIDI-inspelning till Piano Roll")
                };
                if ui.add(egui::Button::new(egui::RichText::new(rec_txt).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().text())).fill(rec_bg)).on_hover_text(crate::i18n::t("När armerad skrivs hållna MIDI-klaviaturtoner in i det aktiva mönstrets rutnät under uppspelning.")).clicked() {
                    self.midi_record_armed = !self.midi_record_armed;
                if self.midi_record_armed {
                    self.begin_new_take();
                }
                }
                ui.label(egui::RichText::new(crate::i18n::t("Tips: klaviaturen ansluts automatiskt. Hittas den inte: koppla in den och anslut igen.")).size(Theme::FONT_XS).color(crate::ui::theme::palette().text_muted()));
            });

            ui.add_space(8.0);

            // Section 2: Open Sound Control (OSC)
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("📡 OPEN SOUND CONTROL (OSC / UDP)")).strong().size(Theme::FONT_S).color(crate::ui::theme::palette().cyan()));
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
                    let status_col = if self.osc_server.is_some() { crate::ui::theme::palette().green() } else { crate::ui::theme::palette().text_muted() };
                    let status_txt = match &self.osc_server {
                        Some(s) => format!("Lyssnar på UDP-port {} • mottagna OSC-paket: {}", s.bound_port, self.osc_rx_count),
                        None => format!("Mottagna OSC-paket: {}", self.osc_rx_count),
                    };
                    ui.label(egui::RichText::new(status_txt).size(Theme::FONT_S).color(status_col));
                    if let Some(err) = self.osc_server.as_ref().and_then(|s| s.last_error.lock().ok().and_then(|g| g.clone())) {
                        ui.label(egui::RichText::new(crate::tstatus!("⚠ {}", err)).size(Theme::FONT_S).color(crate::ui::theme::palette().yellow()));
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
                    ui.label(egui::RichText::new(crate::i18n::t("• /sonix/transport/play (1/0)\n• /sonix/track/{1..8}/volume (0.0 .. 1.25)\n• /sonix/track/{1..8}/mute (1/0)\n• /sonix/bus/{vocal|drum|synth|fx}/volume (0.0 .. 1.25)\n• /sonix/vca/{1..4}/volume (0.0 .. 1.25)")).size(Theme::FONT_S).color(crate::ui::theme::palette().text_muted()));
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
                    .size(Theme::FONT_M)
                    .color(crate::ui::theme::palette().text()),
                );
                ui.label(
                    egui::RichText::new(crate::i18n::t(
                        "Sonix kan inte spela mp3 — den konverteras först, vilket är ett extra varv när wav-filen redan finns. En stämma som bara finns som mp3 tas alltid med.",
                    ))
                    .size(Theme::FONT_S)
                    .color(crate::ui::theme::palette().text_muted()),
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
                    .size(Theme::FONT_M)
                    .color(crate::ui::theme::palette().text()),
                );
                ui.label(
                    egui::RichText::new(crate::tstatus!("mp3:  {}", chosen))
                        .size(Theme::FONT_S)
                        .color(crate::ui::theme::palette().text_muted()),
                );
                ui.label(
                    egui::RichText::new(crate::tstatus!("wav:  {}", sibling))
                        .size(Theme::FONT_S)
                        .color(crate::ui::theme::palette().text_muted()),
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
                self.start_stem_separation_with(&sibling, true);
            } else if separate_chosen {
                self.pending_wav_question = None;
                self.start_stem_separation_with(&chosen, true);
            }
        }
    }
}
}

impl SonixApp {
    /// **Anteckningar** (FL:s NoteBook): en ruta per projekt, sparad i projektfilen.
    pub(crate) fn render_notes_modal(&mut self, ctx: &egui::Context) {
        if !self.show_notes_modal {
            return;
        }
        let mut open = self.show_notes_modal;
        egui::Window::new(crate::i18n::t("📓 Anteckningar"))
            .open(&mut open)
            .default_width(440.0)
            .default_height(280.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.project_notes)
                        .desired_rows(12)
                        .desired_width(f32::INFINITY)
                        .hint_text(crate::i18n::t("Egna anteckningar om projektet — de följer med filen")),
                );
            });
        self.show_notes_modal = open;
    }
}
