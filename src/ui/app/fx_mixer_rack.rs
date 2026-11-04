//! Effektmixern: remsor, EQ-block och masterkanal.
//!
//! Utdelad ur `mixer.rs` 2026-11-02.

use super::*;
use crate::ui::theme::Theme;

/// Kanalremsans minsta och största bredd. Bredden **räknas** ur ytan mellan dem:
/// ett bord som slutar halvvägs in i fönstret ser halvfärdigt ut, hur fina
/// rattarna än är.
const STRIP_W_MIN: f32 = 88.0;
const STRIP_W_MAX: f32 = 118.0;
/// Alla remsor är lika höga, så rubrikband och fadergropar ligger på samma y.
const STRIP_H: f32 = 300.0;
/// Fadern har samma höjd i varje remsa — det är den linjen ögat följer.
const STRIP_FADER_H: f32 = 104.0;
const STRIP_GAP: f32 = 4.0;

/// En kanalremsa: fast bredd och höjd, namnet i ett rubrikband överst, och
/// **allt annat på samma y i varje remsa**. Master, kontrollrum, spår, bussar och
/// VCA hade var sin geometri förut (78/96/82/76 px och olika höjd), så raden såg
/// ut som sju paneler i stället för ett bord. Ett klick i bandet väljer remsan.
fn mixer_strip(
    ui: &mut egui::Ui,
    width: f32,
    title: &str,
    accent: Color32,
    selected: bool,
    selectable: bool,
    body: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let pal = crate::ui::theme::palette();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, STRIP_H), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, Rounding::same(4.0), if selected { pal.panel() } else { pal.panel_alt() });
    painter.rect_stroke(
        rect,
        Rounding::same(4.0),
        Stroke::new(if selected { 1.5_f32 } else { 1.0_f32 }, if selected { accent } else { pal.grid() }),
    );

    // Rubrikbandet bär namnet. Det satt förut i en knapp inuti remsan som växte
    // med texten, så allt under den började olika långt ner i varje remsa.
    let band = Rect::from_min_size(rect.min, Vec2::new(width, 20.0));
    painter.rect_filled(band, Rounding { nw: 4.0, ne: 4.0, sw: 0.0, se: 0.0 }, pal.header());
    ui.put(
        band.shrink2(Vec2::new(4.0, 1.0)),
        egui::Label::new(egui::RichText::new(title).strong().size(Theme::FONT_XS).color(accent))
            .wrap_mode(egui::TextWrapMode::Truncate),
    );

    let mut child = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_max(
                Pos2::new(rect.min.x + 2.0, rect.min.y + 22.0),
                Pos2::new(rect.max.x - 2.0, rect.max.y - 2.0),
            ))
            .layout(egui::Layout::top_down(egui::Align::Center)),
    );
    child.spacing_mut().item_spacing = Vec2::new(2.0, 2.0);
    body(&mut child);

    let id = ui.id().with(("mixer_strip", title));
    ui.interact(band, id, if selectable { Sense::click() } else { Sense::hover() })
}

/// Reglaget ligger alltid längst ner i remsan. Låg det där innehållet råkade
/// sluta gick fadergroparna inte att läsa av på längden.
fn strip_fader(
    ui: &mut egui::Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    peak: f32,
    accent: Color32,
    readout: impl Fn(f32) -> egui::RichText,
) -> bool {
    let mut changed = false;
    ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
        ui.label(readout(*value));
        changed = vertical_fader(ui, value, min, max, peak, accent, STRIP_FADER_H);
    });
    changed
}

/// Content widths for the four blocks of the track EQ row: knobs, curve, presets, dynamics.
/// Each block gets a share of what the row actually has. The old fixed floors
/// (160/220/150/200 px) needed ~790 px together and stacked one block per line in a
/// 470 px dock. The 2 px slack keeps float rounding from wrapping the fourth block.
fn eq_row_blocks(row_w: f32, block_pad: f32, block_gap: f32) -> [f32; 4] {
    let usable = (row_w - block_gap * 3.0 - block_pad * 4.0 - 2.0).max(0.0);
    [usable * 0.24, usable * 0.32, usable * 0.20, usable * 0.24]
}

/// En rattkolumn med fast bredd. Ett naket `vertical_centered` tar hela den
/// återstående raden, så tre kolumner blev bredare än rutan de stod i — och då
/// växte hela panelen utanför fönstret.
fn knob_column(ui: &mut egui::Ui, width: f32, body: impl FnOnce(&mut egui::Ui)) {
    ui.scope(|ui| {
        ui.set_width(width);
        ui.vertical_centered(|ui| body(ui));
    });
}

/// Skiljelinje mellan spåren, bussarna, VCA-grupperna och mastern i samma rad.
fn strip_divider(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(7.0, STRIP_H), Sense::hover());
    ui.painter().rect_filled(
        Rect::from_center_size(rect.center(), Vec2::new(1.0, STRIP_H - 16.0)),
        Rounding::same(0.0),
        crate::ui::theme::palette().grid(),
    );
}

impl SonixApp {
pub(crate) fn render_effects_mixer_rack(&mut self, ui: &mut egui::Ui) {
    // Ensure selected timeline track is within bounds
    if !self.playlist_tracks.is_empty() && self.selected_timeline_track >= self.playlist_tracks.len() {
        self.selected_timeline_track = 0;
    }

    let pal = crate::ui::theme::palette();
    let num_tracks = self.playlist_tracks.len();
    let num_buses = crate::audio::synth::NUM_BUSES;
    let num_vcas = crate::audio::synth::NUM_VCAS;
    let strip_count = num_tracks + num_buses + num_vcas + 2; // + master + kontrollrum
    let mut group_changed = false;
    let mut toggle_talkback: Option<bool> = None;

    // **En ram, en rad.** Sektionerna skiljs åt av en linje, inte av ännu en
    // panel: det var ramarna i ramarna som gjorde ytan rörig, och varje sektion
    // hade sin egen vänster- och högerkant.
    // **Bredden mäts mot den synliga kanten, inte mot `clip_rect().width()`.**
    // Vyns Ui börjar innanför klipprutan (förälderns marginal), så clip-bredden
    // gav panelen ~13 px mer än fönstret hade — och texten längst till höger
    // klipptes av skärmkanten i stället för att sluta vid panelens ram.
    ui.set_max_width((ui.clip_rect().right() - ui.cursor().min.x).max(320.0));
    ui.group(|ui| {
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(crate::i18n::t("🎛 SONIX PRO MULTI-TRACK MIXER & DEDIKERAD EQ-STUDIO")).strong().size(Theme::FONT_M).color(pal.cyan()));
            ui.separator();
            ui.label(egui::RichText::new(crate::tstatus!("Låt: '{}' • {} låtspår med 3-bands parametrisk EQ, dynamik & effekter", self.project_name, self.playlist_tracks.len())).size(Theme::FONT_S).color(pal.text_muted()));
        });
        ui.add_space(4.0);

        // Bredden läses **före** ScrollArea: inuti den är `available_width` den
        // ytan rullningen fått, inte fönstrets.
        let strip_w = ((ui.available_width() - (strip_count as f32 + 4.0) * STRIP_GAP) / strip_count.max(1) as f32)
            .clamp(STRIP_W_MIN, STRIP_W_MAX);

        // ============================================================
        // BORDET: spår → bussar → VCA → master → kontrollrum
        // ============================================================
        egui::ScrollArea::horizontal().id_salt("mixer_strip_bank").auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(STRIP_GAP, 0.0);

                for t_idx in 0..num_tracks {
                    let is_selected = self.selected_timeline_track == t_idx;
                    let (track_name, track_color, track_icon) = {
                        let t = &self.playlist_tracks[t_idx];
                        (t.name.clone(), t.color, t.icon)
                    };
                    let lower = track_name.to_lowercase();
                    let is_mic = (t_idx + 1 == num_tracks
                        && (lower.contains("mic")
                            || lower.contains("mik")
                            || self.playlist_tracks[t_idx].kind == TrackKind::VocalAudio))
                        || lower.contains("mic (")
                        || lower.contains("mikrofon");
                    let accent = if is_mic { pal.cyan() } else { track_color };

                    let resp = mixer_strip(
                        ui,
                        strip_w,
                        &format!("{} {}", track_icon, track_name),
                        accent,
                        is_selected,
                        true,
                        |ui| {
                            // EQ-av/på + inspelningsarm
                            ui.horizontal(|ui| {
                                let eq_on = self.playlist_tracks[t_idx].eq.enabled;
                                let eq_pill_bg = if eq_on { pal.panel_alt() } else { pal.header() };
                                let eq_pill_col = if eq_on { pal.green() } else { pal.text_muted() };
                                let eq_txt = if eq_on { "EQ ON" } else { "EQ OFF" };
                                if ui.add(egui::Button::new(egui::RichText::new(eq_txt).size(Theme::FONT_XS).color(eq_pill_col)).fill(eq_pill_bg).min_size(Vec2::new(38.0, 14.0))).clicked() {
                                    self.playlist_tracks[t_idx].eq.enabled = !self.playlist_tracks[t_idx].eq.enabled;
                                    self.sync_track_audio_state(t_idx);
                                }

                                let is_armed = self.playlist_tracks[t_idx].is_rec_armed;
                                let rec_bg = if is_armed { pal.red() } else { pal.panel() };
                                let rec_col = if is_armed { pal.text() } else { pal.text_muted() };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("REC")).size(Theme::FONT_XS).color(rec_col)).fill(rec_bg).min_size(Vec2::new(28.0, 14.0))).clicked() {
                                    self.playlist_tracks[t_idx].is_rec_armed = !self.playlist_tracks[t_idx].is_rec_armed;
                                }
                            });

                            // Kurvminiatyr — samma bredd som resten av remsan
                            let (thumb_rect, _) = ui.allocate_exact_size(Vec2::new((strip_w - 12.0).max(48.0), 22.0), Sense::hover());
                            let eq_snapshot = self.playlist_tracks[t_idx].eq.clone();
                            mini_track_eq_curve(
                                ui.painter(),
                                thumb_rect,
                                eq_snapshot.low_gain_db,
                                eq_snapshot.mid_gain_db,
                                eq_snapshot.high_gain_db,
                                accent,
                                eq_snapshot.enabled,
                            );

                            // Panorering
                            let mut pan_val = self.playlist_tracks[t_idx].pan;
                            if rotary_knob(ui, &mut pan_val, -1.0, 1.0, "PAN", pal.text_muted(), 15.0) {
                                self.playlist_tracks[t_idx].pan = pan_val;
                                self.sync_track_audio_state(t_idx);
                            }

                            // Mute / Solo
                            ui.horizontal(|ui| {
                                let muted = self.playlist_tracks[t_idx].muted;
                                let m_col = if muted { pal.red() } else { pal.grid() };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("M")).size(Theme::FONT_XS).color(if muted { pal.text() } else { pal.text_muted() })).fill(m_col).min_size(Vec2::new(18.0, 15.0))).clicked() {
                                    self.playlist_tracks[t_idx].muted = !muted;
                                    self.sync_track_audio_state(t_idx);
                                }

                                let solo = self.playlist_tracks[t_idx].solo;
                                let s_col = if solo { pal.orange() } else { pal.grid() };
                                if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("S")).size(Theme::FONT_XS).color(if solo { Color32::BLACK } else { pal.text_muted() })).fill(s_col).min_size(Vec2::new(18.0, 15.0))).clicked() {
                                    self.playlist_tracks[t_idx].solo = !solo;
                                    self.sync_track_audio_state(t_idx);
                                }
                            });

                            // Reglaget längst ner
                            let meter_level = if self.is_playing && !self.playlist_tracks[t_idx].muted {
                                if is_mic { self.vocal_studio.mic_vu_level } else { self.engine.get_peak_level() }
                            } else {
                                0.0
                            };
                            let mut vol_val = self.playlist_tracks[t_idx].volume;
                            if strip_fader(ui, &mut vol_val, 0.0, 1.25, meter_level, accent, |v| {
                                let gain_db = if v <= 0.001 { -60.0 } else { 20.0 * v.log10() };
                                egui::RichText::new(format!("{:.0}%\n{:+.1}dB", v * 100.0, gain_db)).size(Theme::FONT_XS).color(pal.text_muted())
                            }) {
                                self.playlist_tracks[t_idx].volume = vol_val;
                                self.sync_track_audio_state(t_idx);
                            }
                        },
                    );
                    let resp = resp.on_hover_text(crate::tstatus!("Spår {}: {}\nKlicka i namnet för att öppna dedikerad EQ & detaljer", t_idx + 1, track_name));
                    if resp.clicked() {
                        self.selected_timeline_track = t_idx;
                        self.status_message = crate::tstatus!("Valde '{}' för EQ & mixjustering", track_name);
                    }
                }

                if num_tracks > 0 {
                    strip_divider(ui);
                }

                // Bussarna summerar spåren; de står efter spåren, före mastern.
                for b in 0..num_buses {
                    let bus_name = crate::i18n::t(crate::audio::synth::BUS_NAMES[b]);
                    let resp = mixer_strip(ui, strip_w, &format!("BUS {bus_name}"), pal.cyan(), false, false, |ui| {
                        ui.horizontal(|ui| {
                            let muted = self.bus_muted[b];
                            let m_col = if muted { pal.red() } else { pal.text_muted() };
                            if ui.add(egui::Button::new(egui::RichText::new("M").size(Theme::FONT_S).color(m_col)).min_size(egui::vec2(22.0, 16.0))).on_hover_text(crate::i18n::t("Tysta bussen (M)")).clicked() {
                                self.bus_muted[b] = !muted;
                                group_changed = true;
                            }
                            let solo = self.bus_solo[b];
                            let s_col = if solo { pal.yellow() } else { pal.text_muted() };
                            if ui.add(egui::Button::new(egui::RichText::new("S").size(Theme::FONT_S).color(s_col)).min_size(egui::vec2(22.0, 16.0))).on_hover_text(crate::i18n::t("Solo för bussen (S)")).clicked() {
                                self.bus_solo[b] = !solo;
                                group_changed = true;
                            }
                        });
                        // **Bussens egna kurvor** (Fas 8.8): här skapas de — vid
                        // reglaget de hör till. Motorn lyder dem redan; knapparna
                        // skriver bara ner det läge handen ställt in. Ingen gissning
                        // om vad kurvan "borde" vara: den blir vad fadern står på.
                        ui.horizontal(|ui| {
                            let ix = self.bus_automation.iter().position(|l| l.bus == b);
                            let på = ix.is_some_and(|i| self.bus_automation[i].enabled);
                            let punkter = ix.map_or(0, |i| self.bus_automation[i].points.len());
                            let färg = if på { pal.cyan() } else { pal.text_muted() };
                            if ui
                                .add(egui::Button::new(egui::RichText::new("🔗").size(Theme::FONT_S).color(färg)).min_size(egui::vec2(22.0, 16.0)))
                                .on_hover_text(crate::tstatus!(
                                    "Bussens kurva — {} ({} punkter). Bussen äger den själv; flera spår kan skicka hit.",
                                    if på { crate::i18n::t("på") } else { crate::i18n::t("av") },
                                    punkter
                                ))
                                .clicked()
                            {
                                match ix {
                                    Some(i) => self.bus_automation[i].enabled = !på,
                                    None => self.bus_automation.push(BusAutomationLane {
                                        bus: b,
                                        enabled: true,
                                        points: Vec::new(),
                                    }),
                                }
                            }
                            if ui
                                .add(egui::Button::new(egui::RichText::new("+").size(Theme::FONT_S).color(pal.text_muted())).min_size(egui::vec2(22.0, 16.0)))
                                .on_hover_text(crate::i18n::t("Lägg en punkt här, på faderns nuvarande nivå"))
                                .clicked()
                            {
                                let bar = self.tempo_map().bar_at_secs(self.song_time as f64) as f32;
                                let value = self.bus_volume[b];
                                let i = match ix {
                                    Some(i) => {
                                        self.bus_automation[i].enabled = true;
                                        i
                                    }
                                    None => {
                                        self.bus_automation.push(BusAutomationLane {
                                            bus: b,
                                            enabled: true,
                                            points: Vec::new(),
                                        });
                                        self.bus_automation.len() - 1
                                    }
                                };
                                let lane = &mut self.bus_automation[i];
                                // En punkt per takt: två på samma plats ger en lodrät
                                // kurva, och en sådan kan ingen mena.
                                lane.points.retain(|p| (p.time_bars - bar).abs() > 1e-3);
                                lane.points.push(AutomationPoint { time_bars: bar, value });
                                lane.points.sort_by(|x, y| {
                                    x.time_bars
                                        .partial_cmp(&y.time_bars)
                                        .unwrap_or(std::cmp::Ordering::Equal)
                                });
                                self.status_message = crate::tstatus!(
                                    "🔗 Buss {}: punkt vid takt {:.2} ({:.0} %)",
                                    bus_name,
                                    bar,
                                    value * 100.0
                                );
                            }
                        });
                        let mut vol = self.bus_volume[b];
                        if strip_fader(ui, &mut vol, 0.0, 1.25, 0.0, pal.cyan(), |v| {
                            let gain_db = if v <= 0.001 { -60.0 } else { 20.0 * v.log10() };
                            egui::RichText::new(format!("{:.0}% {:+.1}dB", v * 100.0, gain_db)).size(Theme::FONT_XS).color(pal.text_muted())
                        }) {
                            self.bus_volume[b] = vol;
                            group_changed = true;
                        }
                    });
                    resp.on_hover_text(crate::tstatus!("Buss {} — summerar alla spår som skickas hit", bus_name));
                }

                strip_divider(ui);

                // VCA styr grupper utan att routa ljudet.
                for v in 0..num_vcas {
                    mixer_strip(ui, strip_w, &format!("VCA {}", v + 1), pal.purple(), false, false, |ui| {
                        ui.horizontal(|ui| {
                            let muted = self.vca_muted[v];
                            let m_col = if muted { pal.red() } else { pal.text_muted() };
                            if ui.add(egui::Button::new(egui::RichText::new("M").size(Theme::FONT_S).color(m_col)).min_size(egui::vec2(22.0, 16.0))).on_hover_text(crate::i18n::t("Tysta gruppen (M)")).clicked() {
                                self.vca_muted[v] = !muted;
                                group_changed = true;
                            }
                            let solo = self.vca_solos[v];
                            let s_col = if solo { pal.yellow() } else { pal.text_muted() };
                            if ui.add(egui::Button::new(egui::RichText::new("S").size(Theme::FONT_S).color(s_col)).min_size(egui::vec2(22.0, 16.0))).on_hover_text(crate::i18n::t("Solo för gruppen (S)")).clicked() {
                                self.vca_solos[v] = !solo;
                                group_changed = true;
                            }
                        });
                        let mut vol = self.vca_faders[v];
                        if strip_fader(ui, &mut vol, 0.0, 1.25, 0.0, pal.purple(), |val| {
                            let gain_db = if val <= 0.001 { -60.0 } else { 20.0 * val.log10() };
                            egui::RichText::new(format!("{:.0}% {:+.1}dB", val * 100.0, gain_db)).size(Theme::FONT_XS).color(pal.text_muted())
                        }) {
                            self.vca_faders[v] = vol;
                            group_changed = true;
                        }
                    });
                }

                strip_divider(ui);

                // MASTER — bordets sista remsa, som på en konsol.
                mixer_strip(ui, strip_w, crate::i18n::t("MASTER"), pal.orange(), false, false, |ui| {
                    // PAN och BREDD gick förut bara till projektfilen — ingen ljudkod läste
                    // dem. Nu skickas de som ett kommando, och provet mäter matematiken.
                    let mut master_ch = false;
                    master_ch |= crate::ui::widgets::rotary_knob_reset(ui, &mut self.master_pan, -1.0, 1.0, "PAN", pal.text(), 16.0, 0.0);
                    master_ch |= crate::ui::widgets::rotary_knob_reset(ui, &mut self.stereo_width, 0.0, 2.0, "BREDD", pal.cyan(), 16.0, 1.0);
                    if master_ch {
                        let _ = self.engine.send_command(AudioCommand::SetMasterPanWidth {
                            pan: self.master_pan,
                            width: self.stereo_width,
                        });
                    }
                    // **A/B** (Abletons Compare): A och B fångar mixen — volym, pan, mute
                    // per spår plus masternivån — och ↔ växlar och lägger den valda sidan.
                    ui.horizontal(|ui| {
                        for side in [("A", false), ("B", true)] {
                            if ui.small_button(crate::i18n::t(side.0))
                                .on_hover_text(crate::tstatus!("Fånga mixen som {}", crate::i18n::t(side.0)))
                                .clicked()
                            {
                                let snap = self.mix_snapshot(side.0);
                                if side.1 { self.compare.capture_b(snap) } else { self.compare.capture_a(snap) }
                                self.status_message = crate::tstatus!("🎚 Mixen fångad som {}", side.0);
                            }
                        }
                        if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("↔")).size(Theme::FONT_S)).selected(self.compare.active))
                            .on_hover_text(crate::i18n::t("Växla mellan A och B"))
                            .clicked()
                        {
                            match self.compare.switch().cloned() {
                                None => self.status_message = crate::i18n::t("Växlar först när både A och B är fångade").to_string(),
                                Some(snap) => {
                                    for (idx, volume, pan, muted) in self.compare.apply_to(&snap) {
                                        if let Some(t) = self.playlist_tracks.get_mut(idx) {
                                            t.volume = volume;
                                            t.pan = pan;
                                            t.muted = muted;
                                        }
                                        self.sync_track_stem_to_engine(idx);
                                    }
                                    self.master_volume = snap.master;
                                    let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                                    self.status_message = crate::tstatus!("↔ {} hörs", snap.name);
                                }
                            }
                        }
                    });

                    let peak = self.engine.get_peak_level();
                    if strip_fader(ui, &mut self.master_volume, 0.0, 1.25, peak, pal.orange(), |v| {
                        let gain_db = if v <= 0.001 { -60.0 } else { 20.0 * v.log10() };
                        egui::RichText::new(format!("{:.0}%\n{:+.1}dB", v * 100.0, gain_db)).strong().size(Theme::FONT_XS).color(pal.orange())
                    }) {
                        let _ = self.engine.send_command(AudioCommand::SetMasterVolume(self.master_volume));
                    }
                });

                // KONTROLLRUM — det man lyssnar genom. Sitter efter masterns
                // FX-kedja och rör aldrig exporten.
                mixer_strip(ui, strip_w, crate::i18n::t("CONTROL ROOM"), pal.cyan(), false, false, |ui| {
                    let cr_state = self.control_room;
                    let mut changed = false;
                    ui.horizontal(|ui| {
                        changed |= ui
                            .add(egui::Button::new(egui::RichText::new("DIM").size(Theme::FONT_XS)).selected(cr_state.dim))
                            .on_hover_text(crate::i18n::t("Dimma monitorerna −20 dB (rör inte exporten)"))
                            .clicked()
                            && { self.control_room.dim = !cr_state.dim; true };
                        changed |= ui
                            .add(egui::Button::new(egui::RichText::new("MONO").size(Theme::FONT_XS)).selected(cr_state.mono))
                            .on_hover_text(crate::i18n::t("Lyssna i mono — kontrollera att mixen håller utan bredd"))
                            .clicked()
                            && { self.control_room.mono = !cr_state.mono; true };
                    });
                    changed |= ui
                        .add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔇 TYST")).size(Theme::FONT_XS)).selected(cr_state.mute))
                        .on_hover_text(crate::i18n::t("Tysta monitorerna — spelet fortsätter"))
                        .clicked()
                        && { self.control_room.mute = !cr_state.mute; true };
                    // **Cue-lyssning:** vad monitorerna hör, aldrig vad som exporteras.
                    ui.horizontal(|ui| {
                        for (target, label) in [
                            (crate::audio::control_room::CueListen::Main, "MIX"),
                            (crate::audio::control_room::CueListen::A, "A"),
                            (crate::audio::control_room::CueListen::B, "B"),
                        ] {
                            let on = self.cue_listen == target;
                            if ui
                                .add(egui::Button::new(egui::RichText::new(label).size(Theme::FONT_XS)).selected(on))
                                .on_hover_text(crate::i18n::t("Cue hör bara spår med en send till bussen — exporten påverkas inte"))
                                .clicked()
                                && !on
                            {
                                self.cue_listen = target;
                                let _ = self.engine.send_command(AudioCommand::SetCueListen(target));
                            }
                        }
                    });
                    if ui
                        .add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 TALKBACK")).size(Theme::FONT_XS)).selected(self.talkback))
                        .on_hover_text(crate::i18n::t("Micken till monitorerna i stället för in i mastern — hörs även i DIM"))
                        .clicked()
                    {
                        toggle_talkback = Some(!self.talkback);
                    }
                    // **Vart pratet går:** MIX = monitorerna, A/B = sångarens cue.
                    ui.horizontal(|ui| {
                        for (route, label) in [(0u8, "MIX"), (1, "A"), (2, "B")] {
                            if ui
                                .add(egui::Button::new(egui::RichText::new(label).size(Theme::FONT_XS)).selected(self.talkback_cue == route))
                                .on_hover_text(crate::i18n::t("Talkback till monitorerna eller till en cue-buss"))
                                .clicked()
                            {
                                self.talkback_cue = route;
                                let _ = self.engine.send_command(crate::audio::AudioCommand::SetTalkbackCue(route));
                            }
                        }
                    });
                    // Bärvågen: synten blir bärvåg och stammarna modulator.
                    ui.spacing_mut().slider_width = strip_w - 28.0;
                    let mut vocoder = self.vocoder_amount;
                    if ui
                        .add(egui::Slider::new(&mut vocoder, 0.0..=1.0).text(crate::i18n::t("BÄRVÅG")).show_value(false))
                        .on_hover_text(crate::i18n::t("Synten blir bärvåg och stammarna modulator — 0 = av"))
                        .changed()
                    {
                        self.vocoder_amount = vocoder;
                        let _ = self.engine.send_command(AudioCommand::SetMasterVocoder { amount: vocoder });
                    }
                    if changed {
                        let state = self.control_room;
                        let _ = self.engine.send_command(AudioCommand::SetControlRoom(
                            crate::audio::command::ControlRoomState {
                                level: state.level,
                                dim: state.dim,
                                mono: state.mono,
                                mute: state.mute,
                            },
                        ));
                    }
                    let state = self.control_room;
                    if strip_fader(ui, &mut self.control_room.level, 0.0, 2.0, 0.0, pal.cyan(), |v| {
                        let db = if v <= 0.001 { -60.0 } else { 20.0 * v.log10() };
                        egui::RichText::new(format!("{db:+.1} dB")).size(Theme::FONT_XS).color(pal.cyan())
                    }) {
                        let level = self.control_room.level;
                        let _ = self.engine.send_command(AudioCommand::SetControlRoom(
                            crate::audio::command::ControlRoomState {
                                level,
                                dim: state.dim,
                                mono: state.mono,
                                mute: state.mute,
                            },
                        ));
                    }
                });
            });
        });

        if let Some(on) = toggle_talkback {
            self.set_talkback(on);
        }
        if group_changed {
            self.sync_group_state();
        }

        ui.add_space(2.0);
        ui.label(egui::RichText::new(crate::i18n::t("Bussar summerar spår; VCA styr grupper utan att routa ljudet.")).size(Theme::FONT_XS).color(pal.text_muted()));

        ui.separator();

        // ============================================================
        // VALD KANAL: dedikerad EQ, kurva, presets och dynamik
        // ============================================================
        if !self.playlist_tracks.is_empty() {
            let sel_idx = self.selected_timeline_track.min(self.playlist_tracks.len() - 1);
            let sel_t_name = self.playlist_tracks[sel_idx].name.clone();
            let sel_t_col = self.playlist_tracks[sel_idx].color;
            let sel_t_icon = self.playlist_tracks[sel_idx].icon;
            let mut track_dirty = false;

            // **Två rader, inte en som radbryts huller om buller:** rad 1 är
            // kanalens identitet och EQ-av/på, rad 2 är routingen.
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(format!("🎛 DEDIKERAD PARAMETRISK EQ — {} {}", sel_t_icon, sel_t_name)).strong().size(Theme::FONT_M).color(sel_t_col));
                ui.separator();
                let mut eq_enabled = self.playlist_tracks[sel_idx].eq.enabled;
                let eq_txt = if eq_enabled { "EQ Aktiv" } else { "Bypass (Av)" };
                let eq_col = if eq_enabled { pal.green() } else { pal.text_muted() };
                if ui.checkbox(&mut eq_enabled, egui::RichText::new(eq_txt).size(Theme::FONT_S).color(eq_col)).changed() {
                    self.playlist_tracks[sel_idx].eq.enabled = eq_enabled;
                    track_dirty = true;
                }
                ui.separator();
                // Räknaren står **efter** reglagen, inte högerställd: en högerställd
                // etikett klipptes av panelkanten i smala fönster.
                ui.label(egui::RichText::new(crate::tstatus!("Spår {} av {}", sel_idx + 1, self.playlist_tracks.len())).size(Theme::FONT_S).color(pal.text_muted()));
            });

            ui.add_space(4.0);

            // Routingen: buss, VCA, sidokedja och sends.
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(6.0, 6.0);
                ui.label(egui::RichText::new(crate::i18n::t("Buss:")).size(Theme::FONT_S).color(pal.text_muted()));
                let cur_bus = self.playlist_tracks[sel_idx].bus.min(crate::audio::synth::NUM_BUSES - 1);
                egui::ComboBox::from_id_salt("sel_track_bus")
                    .selected_text(crate::i18n::t(crate::audio::synth::BUS_NAMES[cur_bus]))
                    .width(84.0)
                    .show_ui(ui, |ui| {
                        for b in 0..crate::audio::synth::NUM_BUSES {
                            if ui.selectable_label(cur_bus == b, crate::i18n::t(crate::audio::synth::BUS_NAMES[b])).clicked() && cur_bus != b {
                                self.playlist_tracks[sel_idx].bus = b;
                                track_dirty = true;
                            }
                        }
                    });

                ui.label(egui::RichText::new("VCA:").size(Theme::FONT_S).color(pal.text_muted()));
                let cur_vca = self.playlist_tracks[sel_idx].vca;
                egui::ComboBox::from_id_salt("sel_track_vca")
                    .selected_text(match cur_vca {
                        Some(v) => format!("VCA {}", v + 1),
                        None => crate::i18n::t("Ingen").to_string(),
                    })
                    .width(84.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(cur_vca.is_none(), crate::i18n::t("Ingen")).clicked() {
                            self.playlist_tracks[sel_idx].vca = None;
                            track_dirty = true;
                        }
                        for v in 0..crate::audio::synth::NUM_VCAS {
                            if ui.selectable_label(cur_vca == Some(v), format!("VCA {}", v + 1)).clicked() {
                                self.playlist_tracks[sel_idx].vca = Some(v);
                                track_dirty = true;
                            }
                        }
                    });

                // Sidokedja (Fas 8.3): spåret duckas av ett annat spårs ljud.
                ui.label(egui::RichText::new(crate::i18n::t("Sidokedja:")).size(Theme::FONT_S).color(pal.text_muted()));
                if self.playlist_tracks.len() < 2 {
                    // En sidokedja behöver ett annat spår att lyssna på.
                    ui.label(egui::RichText::new(crate::i18n::t("(behöver två spår)")).size(Theme::FONT_S).color(pal.text_muted()));
                } else {
                    let cur_key = self.playlist_tracks[sel_idx].sidechain_from;
                    egui::ComboBox::from_id_salt("sel_track_sidechain")
                        .selected_text(match cur_key {
                            Some(k) => format!("{} {}", crate::i18n::t("Spår"), k + 1),
                            None => crate::i18n::t("Ingen").to_string(),
                        })
                        .width(84.0)
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(cur_key.is_none(), crate::i18n::t("Ingen")).clicked() && cur_key.is_some() {
                                self.playlist_tracks[sel_idx].sidechain_from = None;
                                track_dirty = true;
                            }
                            for k in 0..self.playlist_tracks.len() {
                                // Ett spår kan inte ducka sig självt — det är en slinga.
                                if k == sel_idx {
                                    continue;
                                }
                                if ui.selectable_label(cur_key == Some(k), format!("{} {}", crate::i18n::t("Spår"), k + 1)).clicked() && cur_key != Some(k) {
                                    self.playlist_tracks[sel_idx].sidechain_from = Some(k);
                                    track_dirty = true;
                                }
                            }
                        });
                    if cur_key.is_some() {
                        let sc_track = &mut self.playlist_tracks[sel_idx];
                        track_dirty |= ui
                            .add(egui::Slider::new(&mut sc_track.sidechain_amount_db, 0.0..=24.0).text(crate::i18n::t("Duckning (dB)")))
                            .changed();
                        track_dirty |= ui
                            .add(egui::Slider::new(&mut sc_track.sidechain_threshold_db, -60.0..=0.0).text(crate::i18n::t("Tröskel (dB)")))
                            .changed();
                    }
                }

                // Sends (Fas 8.13 bussar, Fas 8.3 spår): en del av spårets signal
                // till en annan **buss** eller in i ett annat **spårs kedja**.
                // Post-fader i båda fallen — samma signal som går till spårets
                // egen buss, efter volym, EQ, kompressor och sidokedja.
                //
                // En spår-send kan bli en slinga (A → B → A), och en slinga går
                // inte att beräkna: den skulle mata sig själv. Därför prövas varje
                // ändring mot `plan_track_order` **innan** den får skrivas, och
                // ett nej säger vilka två spår det gällde. Motorn har samma regel
                // och behåller sin förra ordning om en fil ändå skulle bära en
                // slinga — den gissar aldrig.
                ui.label(egui::RichText::new(crate::i18n::t("Skicka till:")).size(Theme::FONT_S).color(pal.text_muted()));
                {
                    // Kandidaterna och grafen samlas in **före** det mutabla lånet
                    // av spårets send-lista: att läsa ett annat spår inifrån den
                    // låningen går inte.
                    let track_choices: Vec<(usize, String)> = self
                        .playlist_tracks
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != sel_idx)
                        .map(|(i, t)| (i, crate::tstatus!("{} — {}", i + 1, t.name)))
                        .collect();
                    let track_count = self.playlist_tracks.len();
                    let base_graph: Vec<Vec<usize>> = self
                        .playlist_tracks
                        .iter()
                        .map(|t| {
                            t.sends
                                .iter()
                                .map(|s| s.target.as_track().unwrap_or(usize::MAX))
                                .collect()
                        })
                        .collect();

                    let sends = &mut self.playlist_tracks[sel_idx].sends;
                    let mut remove_send: Option<usize> = None;
                    for (s_idx, send) in sends.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            let selected = match send.target {
                                crate::audio::SendTarget::Bus { target_bus } => {
                                    let b = target_bus.min(crate::audio::synth::NUM_BUSES - 1);
                                    crate::audio::synth::BUS_NAMES[b].to_string()
                                }
                                crate::audio::SendTarget::Track { target_track } => {
                                    format!("Spår {}", target_track + 1)
                                }
                            };
                            egui::ComboBox::from_id_salt(format!("sel_track_send_{s_idx}"))
                                .selected_text(selected)
                                .width(120.0)
                                .show_ui(ui, |ui| {
                                    ui.label(egui::RichText::new(crate::i18n::t("Bussar")).size(Theme::FONT_S).color(pal.text_muted()));
                                    for (bi, name) in crate::audio::synth::BUS_NAMES.iter().enumerate() {
                                        if ui
                                            .selectable_label(send.target.as_bus() == Some(bi), *name)
                                            .clicked()
                                        {
                                            send.target = crate::audio::SendTarget::bus(bi);
                                            track_dirty = true;
                                        }
                                    }
                                    ui.separator();
                                    ui.label(egui::RichText::new(crate::i18n::t("Spår (in i dess kedja)")).size(Theme::FONT_S).color(pal.text_muted()));
                                    for (ti, name) in &track_choices {
                                        if ui
                                            .selectable_label(send.target.as_track() == Some(*ti), name)
                                            .clicked()
                                        {
                                            // Slingkontrollen: samma graf, men med
                                            // den här raden ändrad.
                                            let mut graph = base_graph.clone();
                                            if let Some(row) = graph.get_mut(sel_idx).and_then(|r| r.get_mut(s_idx)) {
                                                *row = *ti;
                                            }
                                            match crate::audio::command::plan_track_order(track_count, &graph) {
                                                Ok(_) => {
                                                    send.target = crate::audio::SendTarget::track(*ti);
                                                    track_dirty = true;
                                                }
                                                Err((a, b)) => {
                                                    self.status_message = crate::tstatus!(
                                                        "⚠ Det skulle bli en slinga (spår {} → spår {}) — senden ändrades inte.",
                                                        a + 1, b + 1
                                                    );
                                                }
                                            }
                                        }
                                    }
                                });
                            track_dirty |= ui
                                .add(egui::Slider::new(&mut send.level, 0.0..=2.0).show_value(true))
                                .on_hover_text(crate::i18n::t("Hur mycket av spåret som går till målet (1,0 = lika starkt som spårets egen utgång)"))
                                .changed();
                            if ui.button("🗑").on_hover_text(crate::i18n::t("Ta bort senden")).clicked() {
                                remove_send = Some(s_idx);
                            }
                        });
                    }
                    if let Some(i) = remove_send {
                        sends.remove(i);
                        track_dirty = true;
                    }
                    if sends.len() < 4
                        && ui
                            .button(crate::i18n::t("➕ Ny send"))
                            .on_hover_text(crate::i18n::t("Skicka en del av spåret till en buss eller in i ett annat spårs kedja — t.ex. FX-bussen"))
                            .clicked()
                    {
                        // FX-bussen är standardmål: det är den vanligaste
                        // parallella vägen, och den kan ändras direkt.
                        sends.push(crate::audio::StemSend {
                            target: crate::audio::SendTarget::bus(crate::audio::synth::NUM_BUSES - 1),
                            level: 0.5,
                        });
                        track_dirty = true;
                    }
                }
            });

            ui.add_space(4.0);

            // ================================================================
            // Fyra rutor som fyller raden: rattar, kurva, presets, dynamik.
            // Bredden delas ur ytan så rutan längst till höger slutar där
            // panelen slutar — förut slutade varje sektion på sitt eget x.
            // ================================================================
            let mut status_msg_update: Option<String> = None;
            let track_mut = &mut self.playlist_tracks[sel_idx];
            // `ui.set_width` sätter **innehållet**, och `ui.group` lägger sin egen
            // ram runt det. Utan avdraget blev fyra rutor ~50 px bredare än raden
            // och panelens högerkant hamnade utanför fönstret.
            let block_pad = egui::Frame::group(ui.style()).inner_margin.sum().x;
            let row_w = ui.available_width();
            let block_gap = 8.0;
            let [knobs_w, curve_w, presets_w, dyn_w] = eq_row_blocks(row_w, block_pad, block_gap);

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(block_gap, 8.0);

                // Block A: 3-bands parametrisk EQ
                ui.group(|ui| {
                    // **Fast ruta, inte `set_width` + `vertical`:** en grupp som bara får en
// minsta bredd växer med innehållet, och då slutade rutorna på olika y.
                    ui.allocate_ui_with_layout(
                        Vec2::new(knobs_w, 250.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("📊 3-BANDS FREKVENSJUSTERING")).strong().size(Theme::FONT_S).color(pal.orange()));
                        ui.separator();

                        let col_w = (knobs_w - 24.0) / 3.0;
                        ui.horizontal(|ui| {
                            // Low Band
                            knob_column(ui, col_w, |ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("LOW SHELF")).size(Theme::FONT_XS).color(pal.orange()));
                                track_dirty |= crate::ui::widgets::rotary_knob_reset(ui, &mut track_mut.eq.low_gain_db, -12.0, 12.0, "GAIN", pal.orange(), 20.0, 0.0);
                                track_dirty |= rotary_knob(ui, &mut track_mut.eq.low_freq, 30.0, 400.0, "FREQ", pal.orange(), 16.0);
                                ui.label(egui::RichText::new(format!("{:.0}Hz\n{:+.1}dB", track_mut.eq.low_freq, track_mut.eq.low_gain_db)).size(Theme::FONT_XS).color(pal.text_muted()));
                            });

                            // Mid Band
                            knob_column(ui, col_w, |ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("MID BELL")).size(Theme::FONT_XS).color(pal.yellow()));
                                track_dirty |= crate::ui::widgets::rotary_knob_reset(ui, &mut track_mut.eq.mid_gain_db, -12.0, 12.0, "GAIN", pal.yellow(), 20.0, 0.0);
                                track_dirty |= rotary_knob(ui, &mut track_mut.eq.mid_freq, 200.0, 8000.0, "FREQ", pal.yellow(), 16.0);
                                track_dirty |= crate::ui::widgets::rotary_knob_reset(ui, &mut track_mut.eq.mid_q, 0.4, 4.0, "Q", pal.yellow(), 16.0, 1.0);
                                ui.label(egui::RichText::new(format!("{:.0}Hz\n{:+.1}dB", track_mut.eq.mid_freq, track_mut.eq.mid_gain_db)).size(Theme::FONT_XS).color(pal.text_muted()));
                            });

                            // High Band
                            knob_column(ui, col_w, |ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("HIGH SHELF")).size(Theme::FONT_XS).color(pal.cyan()));
                                track_dirty |= crate::ui::widgets::rotary_knob_reset(ui, &mut track_mut.eq.high_gain_db, -12.0, 12.0, "GAIN", pal.cyan(), 20.0, 0.0);
                                track_dirty |= rotary_knob(ui, &mut track_mut.eq.high_freq, 2500.0, 16000.0, "FREQ", pal.cyan(), 16.0);
                                ui.label(egui::RichText::new(format!("{:.0}Hz\n{:+.1}dB", track_mut.eq.high_freq, track_mut.eq.high_gain_db)).size(Theme::FONT_XS).color(pal.text_muted()));
                            });
                        });
                    });
                });

                // Block B: interaktiv frekvenskurva
                ui.group(|ui| {
                    // **Fast ruta, inte `set_width` + `vertical`:** en grupp som bara får en
// minsta bredd växer med innehållet, och då slutade rutorna på olika y.
                    ui.allocate_ui_with_layout(
                        Vec2::new(curve_w, 250.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("📈 INTERAKTIV FREKVENSKURVA")).strong().size(Theme::FONT_S).color(pal.cyan()));
                        ui.separator();
                        track_dirty |= eq_curve_visualizer(
                            ui,
                            &mut track_mut.eq.low_gain_db,
                            &mut track_mut.eq.mid_gain_db,
                            &mut track_mut.eq.high_gain_db,
                            Vec2::new((curve_w - 24.0).max(24.0), 84.0),
                        );
                    });
                });

                // Block C: snabbpresets
                ui.group(|ui| {
                    // **Fast ruta, inte `set_width` + `vertical`:** en grupp som bara får en
// minsta bredd växer med innehållet, och då slutade rutorna på olika y.
                    ui.allocate_ui_with_layout(
                        Vec2::new(presets_w, 250.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("⚡ SNABBPRESETS")).strong().size(Theme::FONT_S).color(pal.green()));
                        ui.separator();

                        ui.horizontal_wrapped(|ui| {
                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎙 Sång (Vocal Air)")).size(Theme::FONT_XS).color(pal.text())).fill(pal.border())).clicked() {
                                track_mut.eq.low_gain_db = -2.5;
                                track_mut.eq.low_freq = 120.0;
                                track_mut.eq.mid_gain_db = 2.0;
                                track_mut.eq.mid_freq = 3200.0;
                                track_mut.eq.high_gain_db = 4.0;
                                track_mut.eq.high_freq = 11000.0;
                                track_mut.eq.enabled = true;
                                track_dirty = true;
                                status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Sång (Vocal Air)' på {}", sel_t_name));
                            }

                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🗣 Kör / Harmoni")).size(Theme::FONT_XS).color(pal.text())).fill(pal.border())).clicked() {
                                track_mut.eq.low_gain_db = -4.0;
                                track_mut.eq.low_freq = 160.0;
                                track_mut.eq.mid_gain_db = -1.5;
                                track_mut.eq.mid_freq = 1200.0;
                                track_mut.eq.high_gain_db = 3.0;
                                track_mut.eq.high_freq = 9000.0;
                                track_mut.eq.enabled = true;
                                track_dirty = true;
                                status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Kör / Harmoni' på {}", sel_t_name));
                            }

                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🥁 Trum-Punch")).size(Theme::FONT_XS).color(pal.text())).fill(pal.panel_alt())).clicked() {
                                track_mut.eq.low_gain_db = 4.5;
                                track_mut.eq.low_freq = 80.0;
                                track_mut.eq.mid_gain_db = -2.5;
                                track_mut.eq.mid_freq = 450.0;
                                track_mut.eq.high_gain_db = 3.5;
                                track_mut.eq.high_freq = 7000.0;
                                track_mut.eq.enabled = true;
                                track_dirty = true;
                                status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Trum-Punch' på {}", sel_t_name));
                            }

                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎸 Bas / Sub Power")).size(Theme::FONT_XS).color(pal.text())).fill(pal.panel_alt())).clicked() {
                                track_mut.eq.low_gain_db = 5.0;
                                track_mut.eq.low_freq = 65.0;
                                track_mut.eq.mid_gain_db = -2.0;
                                track_mut.eq.mid_freq = 600.0;
                                track_mut.eq.high_gain_db = -4.0;
                                track_mut.eq.high_freq = 4000.0;
                                track_mut.eq.enabled = true;
                                track_dirty = true;
                                status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Bas / Sub Power' på {}", sel_t_name));
                            }

                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🎸 Gitarr / Presence")).size(Theme::FONT_XS).color(pal.text())).fill(pal.panel_alt())).clicked() {
                                track_mut.eq.low_gain_db = -2.0;
                                track_mut.eq.low_freq = 100.0;
                                track_mut.eq.mid_gain_db = 3.0;
                                track_mut.eq.mid_freq = 2400.0;
                                track_mut.eq.high_gain_db = 2.0;
                                track_mut.eq.high_freq = 6000.0;
                                track_mut.eq.enabled = true;
                                track_dirty = true;
                                status_msg_update = Some(crate::tstatus!("⚡ Applicerade preset 'Gitarr / Presence' på {}", sel_t_name));
                            }

                            if ui.add(egui::Button::new(egui::RichText::new(crate::i18n::t("🔄 Nollställ EQ")).size(Theme::FONT_XS).color(pal.text())).fill(pal.border())).clicked() {
                                track_mut.eq = TrackEq::default();
                                track_dirty = true;
                                status_msg_update = Some(crate::tstatus!("🔄 Nollställde EQ för {}", sel_t_name));
                            }
                        });
                    });
                });

                // Block D har sin andel ur samma delning som de tre andra, så de fyra
                // rutorna slutar vid panelens högerkant.
                // Block D: dynamik och effekt-sändning
                ui.group(|ui| {
                    // **Fast ruta, inte `set_width` + `vertical`:** en grupp som bara får en
// minsta bredd växer med innehållet, och då slutade rutorna på olika y.
                    ui.allocate_ui_with_layout(
                        Vec2::new(dyn_w, 250.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                        ui.label(egui::RichText::new(crate::i18n::t("🎚 DYNAMIK & EFFEKT-SÄNDNING")).strong().size(Theme::FONT_S).color(pal.purple()));
                        ui.separator();

                        let col_w = (dyn_w - 24.0) / 3.0;
                        ui.horizontal(|ui| {
                            knob_column(ui, col_w, |ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("KOMPRESSOR")).size(Theme::FONT_XS).color(pal.green()));
                                track_dirty |= rotary_knob(ui, &mut track_mut.comp_threshold_db, -36.0, 0.0, "TRSH", pal.green(), 17.0);
                                track_dirty |= rotary_knob(ui, &mut track_mut.comp_ratio, 1.0, 10.0, "RATIO", pal.green(), 17.0);
                            });

                            knob_column(ui, col_w, |ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("SÄNDNING")).size(Theme::FONT_XS).color(pal.purple()));
                                track_dirty |= rotary_knob(ui, &mut track_mut.reverb_send, 0.0, 1.0, "REV", pal.purple(), 17.0);
                                track_dirty |= rotary_knob(ui, &mut track_mut.delay_send, 0.0, 1.0, "DLY", pal.cyan(), 17.0);
                                ui.horizontal(|ui| {
                                    track_dirty |= rotary_knob(ui, &mut track_mut.cue_a, 0.0, 1.0, "CUE A", pal.orange(), 13.0);
                                    track_dirty |= rotary_knob(ui, &mut track_mut.cue_b, 0.0, 1.0, "CUE B", pal.orange(), 13.0);
                                });
                            });

                            knob_column(ui, col_w, |ui| {
                                ui.label(egui::RichText::new(crate::i18n::t("TONHÖJD")).size(Theme::FONT_XS).color(pal.yellow()));
                                track_dirty |= pitch_knob(ui, &mut track_mut.pitch_semitones, -12.0, 12.0, "PITCH", pal.yellow(), 17.0);
                                ui.label(egui::RichText::new(format!("{:+.2} st", track_mut.pitch_semitones)).size(Theme::FONT_XS).color(pal.text_muted()));
                            });
                        });
                    });
                });
            });

            if let Some(msg) = status_msg_update {
                self.status_message = msg;
            }

            if track_dirty {
                self.sync_track_audio_state(sel_idx);
            }
        }

        ui.separator();

        // ============================================================
        // MASTER-FX: tre lika breda rutor, samma kant som raden ovanför
        // ============================================================
        let fx_gap = 8.0;
        let fx_pad = egui::Frame::group(ui.style()).inner_margin.sum().x;
        let fx_w = ((ui.available_width() - fx_gap * 2.0) / 3.0 - fx_pad).max(200.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(fx_gap, 6.0);

            ui.group(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(fx_w, 96.0),
                    // Mittenställt: tre rattar i en 520 px ruta såg ut att ha hamnat
                    // i vänsterkanten när allt annat i raden är centrerat.
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🌊 MASTER STEREO DELAY")).strong().size(Theme::FONT_S).color(pal.cyan()));
                    ui.separator();
                    ui.horizontal(|ui| {
                        let mut ch = false;
                        ch |= rotary_knob(ui, &mut self.delay.time_ms, 30.0, 800.0, "TID (ms)", pal.cyan(), 19.0);
                        ch |= rotary_knob(ui, &mut self.delay.feedback, 0.0, 0.88, "FEEDBACK", pal.cyan(), 19.0);
                        ch |= rotary_knob(ui, &mut self.delay.mix, 0.0, 1.0, "MIX", pal.orange(), 19.0);
                        if ch { let _ = self.engine.send_command(AudioCommand::SetDelay(self.delay)); }
                    });
                });
            });

            ui.group(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(fx_w, 96.0),
                    // Mittenställt: tre rattar i en 520 px ruta såg ut att ha hamnat
                    // i vänsterkanten när allt annat i raden är centrerat.
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("✨ MASTER SPACE REVERB")).strong().size(Theme::FONT_S).color(pal.purple()));
                    ui.separator();
                    ui.horizontal(|ui| {
                        let mut ch = false;
                        ch |= rotary_knob(ui, &mut self.reverb.room_size, 0.1, 0.95, "RUM", pal.purple(), 19.0);
                        ch |= rotary_knob(ui, &mut self.reverb.damping, 0.05, 0.9, "DÄMP", pal.purple(), 19.0);
                        ch |= rotary_knob(ui, &mut self.reverb.mix, 0.0, 1.0, "MIX", pal.orange(), 19.0);
                        if ch { let _ = self.engine.send_command(AudioCommand::SetReverb(self.reverb)); }
                    });
                });
            });

            ui.group(|ui| {
                ui.allocate_ui_with_layout(
                    Vec2::new(fx_w, 96.0),
                    // Mittenställt: tre rattar i en 520 px ruta såg ut att ha hamnat
                    // i vänsterkanten när allt annat i raden är centrerat.
                    egui::Layout::top_down(egui::Align::Center),
                    |ui| {
                    ui.label(egui::RichText::new(crate::i18n::t("🔥 MASTER ANALOG DRIVE & LIMITER")).strong().size(Theme::FONT_S).color(pal.yellow()));
                    ui.separator();
                    ui.horizontal(|ui| {
                        let mut ch = false;
                        ch |= rotary_knob(ui, &mut self.stompbox_drive, 1.0, 8.0, "DRIVE", pal.orange(), 19.0);
                        // TON läste ingen förut. "4x12 Cab" är borta: en högtalarsimulering är
                        // en impuls och ett filter, inte en kryssruta som inte gör något.
                        ch |= rotary_knob(ui, &mut self.stompbox_tone, 1000.0, 10000.0, "TON", pal.yellow(), 19.0);
                        if ch {
                            self.drive = self.stompbox_drive;
                            let _ = self.engine.send_command(AudioCommand::SetDrive(self.drive));
                            let _ = self.engine.send_command(AudioCommand::SetDriveTone(self.stompbox_tone));
                        }
                    });
                });
            });
        });
    });
}
}

#[cfg(test)]
mod eq_row_tests {
    use super::eq_row_blocks;

    /// Four blocks used to need ~790 px because of their fixed floors, so a 470 px dock
    /// stacked them one per line and the last one claimed the whole row. Now each block
    /// takes a share of the row and no block is wider than the row itself.
    #[test]
    fn eq_row_blocks_never_exceed_the_width_they_are_given() {
        let block_pad = eframe::egui::Frame::group(&eframe::egui::Style::default()).inner_margin.sum().x;
        let block_gap = 8.0;
        for row_w in [320.0, 470.0, 900.0, 1600.0] {
            let blocks = eq_row_blocks(row_w, block_pad, block_gap);
            let total = blocks.iter().sum::<f32>() + block_gap * 3.0 + block_pad * 4.0;
            println!(
                "row {row_w:.0} px -> blocks {:?}, total {total:.1} px (pad {block_pad:.0}, gap {block_gap:.0})",
                blocks.map(|w| (w * 10.0).round() / 10.0)
            );
            for (i, w) in blocks.iter().enumerate() {
                assert!(*w <= row_w, "block {i} is {w} px in a {row_w} px row");
                assert!(*w > 0.0, "block {i} collapsed to {w} px");
            }
            assert!(total <= row_w, "blocks need {total} px in a {row_w} px row");
        }
    }
}
