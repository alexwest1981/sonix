//! **Suno-ytan.** Ett eget fönster i stället för en popup.
//!
//! Suno-panelen låg tidigare i en modal med allt på en gång: inloggning, formulär, verktygsnamn
//! som text, bibliotek och resultat. Det gick inte att läsa. Nu är ytan en egen flik med en egen
//! rad flikar — Skapa, Röster, Bibliotek, Spellistor, Arbetsytor, Konto — där varje del av
//! serverns verktygsyta får sin egen ruta i stället för att trängas.

use eframe::egui::{self, Vec2};
use egui::RichText;

use crate::ui::app::suno_form::{SunoField, SunoSlots};
use crate::ui::theme::{palette, Theme};

use super::SonixApp;

/// Flikarna inne i Suno-ytan.
const TABS: [&str; 6] = ["Skapa", "Röster", "Bibliotek", "Spellistor", "Arbetsytor", "Konto"];

/// What a confirmed download credit buys. The armed row lives in `suno_credit_song` as
/// `"<tag>:<id>"`, so a row is confirmed only by the button the user pressed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SunoSpend {
    /// Listen to one of the two takes.
    Clip,
    /// Listen to a library row.
    Play,
    /// Import a library row onto the timeline.
    Import,
}

impl SunoSpend {
    fn tag(self) -> &'static str {
        match self {
            Self::Clip => "clip",
            Self::Play => "play",
            Self::Import => "import",
        }
    }
}

/// A click on a spending button either arms its row or spends on the row already armed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SpendStep {
    Arm,
    Confirm,
}

fn credit_step(armed: Option<&str>, what: SunoSpend, id: &str) -> SpendStep {
    match armed.and_then(|value| value.split_once(':')) {
        Some((tag, armed_id)) if tag == what.tag() && armed_id == id => SpendStep::Confirm,
        _ => SpendStep::Arm,
    }
}

/// Applies the click: arming only records the row, and `Confirm` is the single step that lets the
/// caller download. Nothing is fetched before the second click.
fn step_after_click(armed: &mut Option<String>, what: SunoSpend, id: &str) -> SpendStep {
    let step = credit_step(armed.as_deref(), what, id);
    *armed = match step {
        SpendStep::Arm => Some(format!("{}:{id}", what.tag())),
        SpendStep::Confirm => None,
    };
    step
}

impl SonixApp {
    pub(crate) fn render_suno_view(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("☀ SUNO")
                    .strong()
                    .size(Theme::FONT_L)
                    .color(palette().orange()),
            );
            ui.separator();
            if self.suno_pending.is_some() {
                ui.spinner();
            }
            // **Inloggningen bor i Inställningar → AI.** Panelen visar bara läget och kan
            // kontrollera det igen — den öppnar aldrig en webbläsare av sig själv.
            let (state, colour) = if self.suno_signed_in {
                (crate::i18n::t("☀ inloggad"), palette().green())
            } else {
                (
                    crate::i18n::t("☀ inte inloggad — Inställningar → AI"),
                    palette().text_muted(),
                )
            };
            ui.label(RichText::new(state).size(Theme::FONT_S).color(colour));
            if ui
                .add_enabled(
                    self.suno_pending.is_none(),
                    egui::Button::new(crate::i18n::t("🔄 Kontrollera igen")),
                )
                .clicked()
            {
                self.suno_connect_silently();
            }
            // Skapa-knappen sitter i huvudraden: den får aldrig hamna under vikningen.
            if ui
                .add_enabled(
                    self.suno_pending.is_none(),
                    egui::Button::new(
                        RichText::new(crate::i18n::t("✨ Skapa"))
                            .strong()
                            .color(egui::Color32::BLACK),
                    )
                    .fill(palette().orange())
                    .min_size(Vec2::new(140.0, 26.0)),
                )
                .clicked()
            {
                self.suno_tab = 0;
                self.suno_create();
            }
            if !self.suno_status.trim().is_empty() {
                // Long statuses are cut off in the header row; the full line stays on hover.
                let _ = ui
                    .add(
                        egui::Label::new(
                            RichText::new(&self.suno_status)
                                .size(Theme::FONT_S)
                                .color(palette().cyan()),
                        )
                        .truncate(),
                    )
                    .on_hover_text(&self.suno_status);
            }
        });
        // Adressen syns medan inloggningen väntar — en GUI-app har ingen terminal att läsa den ur.
        let login_url = self
            .suno_login_url
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .unwrap_or_default();
        if self.suno_pending.is_some() && !login_url.is_empty() {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(crate::i18n::t("Öppna den här adressen om webbläsaren inte öppnades:"))
                        .size(Theme::FONT_S)
                        .color(palette().text_muted()),
                );
                let mut shown = login_url.clone();
                ui.add(
                    egui::TextEdit::singleline(&mut shown)
                        .desired_width(520.0)
                        .font(egui::TextStyle::Monospace),
                );
                if ui.button(crate::i18n::t("🌐 Öppna igen")).clicked() {
                    let _ = crate::platform::open_url(&login_url);
                }
            });
        }

        // Flikraden. Ett klick byter ruta; ingen modal ligger ovanpå något annat.
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            for (index, label) in TABS.iter().enumerate() {
                let active = self.suno_tab == index;
                let text = RichText::new(crate::i18n::t(label)).size(Theme::FONT_S);
                let text = if active {
                    text.strong().color(palette().bg())
                } else {
                    text.color(palette().text_muted())
                };
                let fill = if active {
                    palette().orange()
                } else {
                    palette().panel()
                };
                if ui.add(egui::Button::new(text).fill(fill)).clicked() {
                    self.suno_tab = index;
                }
            }
        });
        ui.separator();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| match self.suno_tab {
                0 => self.suno_tab_create(ui),
                1 => self.suno_tab_voices(ui),
                2 => self.suno_tab_library(ui),
                3 => self.suno_tab_meta(ui, crate::ui::app::SunoMeta::Playlists),
                4 => self.suno_tab_meta(ui, crate::ui::app::SunoMeta::Workspaces),
                _ => self.suno_tab_meta(ui, crate::ui::app::SunoMeta::Account),
            });
    }


    /// Listan som knapparna Audio/Voice/Inspo fäller ut, på plats i rutan.
    fn suno_pick_list(&mut self, ui: &mut egui::Ui) {
        use crate::ui::app::SunoPick;
        let Some(pick) = self.suno_pick else {
            return;
        };
        egui::Frame::none()
            .fill(palette().panel())
            .inner_margin(6.0)
            .show(ui, |ui| {
                if pick == SunoPick::Voice {
                    let rows: Vec<(String, String)> = self
                        .suno_voices
                        .iter()
                        .map(|(id, name)| (id.clone(), name.clone()))
                        .collect();
                    if rows.is_empty() {
                        ui.label(
                            RichText::new(
                                self.suno_voice_hint.clone().unwrap_or_else(|| {
                                    crate::i18n::t("Inga röster listade än.").to_string()
                                }),
                            )
                            .size(Theme::FONT_XS)
                            .color(palette().text_muted()),
                        );
                    }
                    for (index, (_, name)) in rows.iter().enumerate() {
                        if ui
                            .selectable_label(self.suno_voice_choice == index, name)
                            .clicked()
                        {
                            self.suno_voice_choice = index;
                            self.suno_pick = None;
                        }
                    }
                    return;
                }
                let rows: Vec<(String, String)> = self
                    .suno_library
                    .iter()
                    .map(|song| (song.id.clone(), song.title.clone()))
                    .collect();
                if rows.is_empty() {
                    ui.label(
                        RichText::new(
                            crate::i18n::t("Hämtar biblioteket…")
                                .to_string(),
                        )
                        .size(Theme::FONT_XS)
                        .color(palette().text_muted()),
                    );
                }
                for (id, title) in rows {
                    if ui.selectable_label(false, title.as_str()).clicked() {
                        match pick {
                            SunoPick::Audio => self.suno_source_song = Some((id, title)),
                            SunoPick::Inspo => self.suno_inspo_song = Some((id, title)),
                            SunoPick::Voice => {}
                        }
                        self.suno_pick = None;
                    }
                }
            });
    }

    /// The one spending button every credit action shares: it states the cost, arms its row on
    /// the first click and returns `true` only on the confirming click — the only moment a
    /// download may start.
    fn suno_spend_button(
        &mut self,
        ui: &mut egui::Ui,
        enabled: bool,
        what: SunoSpend,
        id: &str,
        label: &'static str,
    ) -> bool {
        let armed = credit_step(self.suno_credit_song.as_deref(), what, id) == SpendStep::Confirm;
        let text = if armed {
            crate::i18n::t("✅ Bekräfta — 1 kredit")
        } else {
            label
        };
        if !ui.add_enabled(enabled, egui::Button::new(text)).clicked() {
            return false;
        }
        step_after_click(&mut self.suno_credit_song, what, id) == SpendStep::Confirm
    }

    /// **Skapa** — Sunos eget flöde: lyrics, styles, bilagor till vänster, options till höger.
    fn suno_tab_create(&mut self, ui: &mut egui::Ui) {
        // Formuläret fylls ur reservschemat (mätt från den riktiga `audio-create`) redan innan
        // anslutningen svarat, så ytan går att skriva i direkt. Vid anslutning tar serverns eget
        // schema över.
        if self.suno_form.is_empty() {
            self.suno_form = crate::ui::app::suno_form::fallback_fields();
            ui.label(
                RichText::new(crate::i18n::t(
                    "Fälten är Sunos egna, mätta från audio-create. Anslut för att hämta serverns aktuella schema.",
                ))
                .size(Theme::FONT_XS)
                .color(palette().text_muted()),
            );
        }
        let slots: SunoSlots = crate::ui::app::suno_form::slots(&self.suno_form);
        let width = ui.available_width();
        ui.horizontal_top(|ui| {
            ui.allocate_ui(Vec2::new(width * 0.55 - 8.0, 0.0), |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(crate::i18n::t("✍ Skriv låten"))
                            .strong()
                            .size(Theme::FONT_S)
                            .color(palette().orange()),
                    );
                    // Simple-läget: en beskrivning, så skriver Suno både text och stil.
                    ui.label(
                        RichText::new(crate::i18n::t("Beskriv låten (Simple)"))
                            .size(Theme::FONT_S),
                    );
                    ui.add(
                        egui::TextEdit::multiline(&mut self.suno_prompt_input)
                            .desired_rows(3)
                            .desired_width(f32::INFINITY)
                            .hint_text(crate::i18n::t(
                                "t.ex. pagan metal med irländsk harpa, långsam och mörk",
                            )),
                    );
                    ui.label(
                        RichText::new(crate::i18n::t("eller Custom nedan:"))
                            .size(Theme::FONT_XS)
                            .color(palette().text_muted()),
                    );
                    self.suno_field_text(ui, slots.lyrics, "Lyrics", "Lämna tomt för instrumental", true);
                    self.suno_field_text(ui, slots.styles, "Styles", "pagan metal, irish harp, indie rap…", false);
                    ui.horizontal_wrapped(|ui| {
                        for chip in [
                            "pagan metal",
                            "irish harp",
                            "tremolo picking",
                            "melodic guitars",
                            "80s synthwave",
                            "neo-soul",
                        ] {
                            if ui.small_button(chip).clicked()
                                && let Some(index) = slots.styles
                                && let Some((_, SunoField::Text { value, .. })) =
                                    self.suno_form.get_mut(index)
                            {
                                if !value.trim().is_empty() {
                                    value.push_str(", ");
                                }
                                value.push_str(chip);
                            }
                        }
                    });
                    // Audio = en låt ur biblioteket som källa (cover), Voice = en röstmodell,
                    // Inspo = en låt till som blandas in (mashup). Varje knapp öppnar listan
                    // **här i rutan**, inte i en popup.
                    ui.horizontal_wrapped(|ui| {
                        for (pick, label, chosen) in [
                            (
                                crate::ui::app::SunoPick::Audio,
                                "🎵 Audio",
                                self.suno_source_song.as_ref().map(|(_, title)| title.clone()),
                            ),
                            (
                                crate::ui::app::SunoPick::Voice,
                                "🎤 Voice",
                                self.suno_voices
                                    .get(self.suno_voice_choice)
                                    .map(|(_, name)| name.clone()),
                            ),
                            (
                                crate::ui::app::SunoPick::Inspo,
                                "💡 Inspo",
                                self.suno_inspo_song.as_ref().map(|(_, title)| title.clone()),
                            ),
                        ] {
                            let text = match &chosen {
                                Some(name) => format!("{label}: {name}"),
                                None => label.to_string(),
                            };
                            let active = self.suno_pick == Some(pick);
                            let fill = if active || chosen.is_some() {
                                palette().cyan()
                            } else {
                                palette().panel()
                            };
                            let label_color = if active || chosen.is_some() {
                                palette().bg()
                            } else {
                                palette().text()
                            };
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(text).size(Theme::FONT_S).color(label_color),
                                    )
                                    .fill(fill),
                                )
                                .clicked()
                            {
                                self.suno_pick = if active { None } else { Some(pick) };
                                match pick {
                                    crate::ui::app::SunoPick::Audio
                                    | crate::ui::app::SunoPick::Inspo => {
                                        if self.suno_library.is_empty() {
                                            self.suno_load_library();
                                        }
                                    }
                                    crate::ui::app::SunoPick::Voice => {}
                                }
                            }
                        }
                        if ui.button("✖").on_hover_text("Rensa valen").clicked() {
                            self.suno_source_song = None;
                            self.suno_inspo_song = None;
                            self.suno_pick = None;
                        }
                    });
                });
            });
            ui.separator();
            ui.allocate_ui(Vec2::new(width * 0.42 - 8.0, 0.0), |ui| {
                ui.vertical(|ui| {
                    ui.label(
                        RichText::new(crate::i18n::t("⚙ Fler val"))
                            .strong()
                            .size(Theme::FONT_S)
                            .color(palette().cyan()),
                    );
                    for (slot, label) in [
                        (slots.weirdness, "Weirdness"),
                        (slots.style_influence, "Stilinfluens"),
                        (slots.variety, "Variation"),
                    ] {
                        self.suno_field_number(ui, slot, label);
                    }
                    for (slot, label) in [
                        (slots.vocal_gender, "Sångröst"),
                        (slots.duration, "Längd"),
                    ] {
                        self.suno_field_choice(ui, slot, label);
                    }
                    // **Modellen väljs ur kontots lista**, aldrig en tyst standardmodell.
                    if let Some(index) = slots.model {
                        let models: Vec<String> = self
                            .suno_account
                            .as_ref()
                            .map(|a| a.models.clone())
                            .unwrap_or_default();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(crate::i18n::t("Modell")).size(Theme::FONT_S));
                            if models.is_empty() {
                                ui.label(
                                    RichText::new(crate::i18n::t(
                                        "Hämta kontot under fliken Konto för modellistan",
                                    ))
                                    .size(Theme::FONT_XS)
                                    .color(palette().text_muted()),
                                );
                            } else if let Some((_, SunoField::Text { value, .. })) =
                                self.suno_form.get_mut(index)
                            {
                                let chosen = if value.trim().is_empty() {
                                    crate::i18n::t("(planens standard)").to_string()
                                } else {
                                    value.clone()
                                };
                                egui::ComboBox::from_id_salt("suno_model")
                                    .selected_text(chosen)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(value, String::new(), "(planens standard)");
                                        for model in &models {
                                            ui.selectable_value(value, model.clone(), model);
                                        }
                                    });
                            }
                        });
                    }
                    for (slot, label) in [
                        (slots.instrumental, "Instrumental (ingen sång)"),
                        (slots.max_mode, "Max-läge"),
                        (slots.personalize, "Personifiera"),
                    ] {
                        self.suno_field_bool(ui, slot, label);
                    }
                    self.suno_field_text(ui, slots.title, "Låtens titel", "(valfritt)", false);
                    // **Arbetsytan väljs, skrivs inte.** `workspace_id` tar ett id från
                    // `workspaces-list`; ett påhittat namn ger "workspace_id is not a workspace id".
                    if let Some(index) = slots.workspace {
                        let rows: Vec<(String, String)> = self
                            .suno_workspaces
                            .iter()
                            .map(|w| (w.id.clone(), w.name.clone()))
                            .collect();
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(crate::i18n::t("Spara i")).size(Theme::FONT_S),
                            );
                            if rows.is_empty() {
                                ui.label(
                                    RichText::new(crate::i18n::t(
                                        "Inga arbetsytor — hämta under fliken Arbetsytor",
                                    ))
                                    .size(Theme::FONT_XS)
                                    .color(palette().text_muted()),
                                );
                            } else if let Some((_, SunoField::Text { value, .. })) =
                                self.suno_form.get_mut(index)
                            {
                                let chosen = rows
                                    .iter()
                                    .find(|(id, _)| id == value)
                                    .map(|(_, name)| name.clone())
                                    .unwrap_or_else(|| crate::i18n::t("(ingen)").to_string());
                                egui::ComboBox::from_id_salt("suno_workspace")
                                    .selected_text(chosen)
                                    .show_ui(ui, |ui| {
                                        ui.selectable_value(value, String::new(), "(ingen)");
                                        for (id, name) in &rows {
                                            ui.selectable_value(value, id.clone(), name);
                                        }
                                    });
                            }
                        });
                    }
                });
            });
        });
        // Väljarlistan ligger under båda kolumnerna — inuti en kolumn med höjd 0 låste layouten.
        self.suno_pick_list(ui);
        // **Alternativen Suno gav.** Lyssna (kostar en hämtning), välj ett, separera med
        // förloppsindikator, exportera till tidslinjen — fyra tydliga steg i samma ruta.
        if !self.suno_clips.is_empty() {
            ui.add_space(6.0);
            ui.label(
                RichText::new(crate::tstatus!("🎵 {} alternativ", self.suno_clips.len()))
                    .strong()
                    .size(Theme::FONT_S)
                    .color(palette().orange()),
            );
            let rows: Vec<(usize, String, Option<f64>, bool, String)> = self
                .suno_clips
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let title = if c.title.trim().is_empty() {
                        c.id.clone()
                    } else {
                        c.title.clone()
                    };
                    (i, title, c.seconds, c.file.is_some(), c.id.clone())
                })
                .collect();
            let mut audition: Option<usize> = None;
            let mut separate: Option<usize> = None;
            let busy = self.suno_pending.is_some() || self.stem_separation_active;
            for (index, title, seconds, has_file, id) in rows {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).size(Theme::FONT_S));
                    let ready = seconds.is_some() || has_file;
                    match seconds {
                        Some(seconds) => {
                            ui.label(
                                RichText::new(format!("{seconds:.0}s"))
                                    .size(Theme::FONT_XS)
                                    .color(palette().text_muted()),
                            );
                        }
                        None if !has_file => {
                            // Är den inte klar än står det — i stället för att raden ser död ut.
                            ui.spinner();
                            ui.label(
                                RichText::new(crate::i18n::t("skapar…"))
                                    .size(Theme::FONT_XS)
                                    .color(palette().text_muted()),
                            );
                        }
                        None => {}
                    }
                    // A take already on disk plays for free; an unfetched one costs a credit and
                    // is armed first, so nothing is downloaded by the first click.
                    if has_file {
                        if ui
                            .add_enabled(!busy, egui::Button::new(crate::i18n::t("🎧 Spela upp")))
                            .clicked()
                        {
                            audition = Some(index);
                        }
                    } else if self.suno_spend_button(
                        ui,
                        !busy && ready,
                        SunoSpend::Clip,
                        &id,
                        crate::i18n::t("🎧 Lyssna — 1 kredit"),
                    ) {
                        audition = Some(index);
                    }
                    if ui
                        .add_enabled(
                            has_file && !busy,
                            egui::Button::new(crate::i18n::t("🎚 Separera")),
                        )
                        .on_disabled_hover_text(crate::i18n::t(
                            "Lyssna på alternativet först — filen hämtas då",
                        ))
                        .clicked()
                    {
                        separate = Some(index);
                    }
                });
            }
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::t("🔄 Uppdatera status")).clicked() {
                    self.suno_load_clip_titles();
                }
            });
            if let Some(index) = audition {
                self.suno_audition(index);
            }
            if let Some(index) = separate {
                self.suno_separate_clip(index);
            }
        }
        // Förloppet medan den separerar.
        if self.stem_separation_active {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    RichText::new(crate::i18n::t("Separerar stämmor…"))
                        .size(Theme::FONT_S)
                        .color(palette().cyan()),
                );
                ui.add(
                    egui::ProgressBar::new(self.stem_project.progress)
                        .desired_width(220.0)
                        .show_percentage(),
                );
            });
        } else if !self.stem_project.stem_files.is_empty() {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(crate::tstatus!(
                        "🎚 {} stämmor klara",
                        self.stem_project.stem_files.len()
                    ))
                    .size(Theme::FONT_S)
                    .color(palette().cyan()),
                );
                if ui
                    .button(crate::i18n::t("→ Exportera till tidslinjen"))
                    .clicked()
                {
                    self.suno_export_stems();
                }
            });
        }
    }

    /// **Röster** — `voices-list` som en lista, inte som ett namn i en verktygsrad.
    fn suno_tab_voices(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new(crate::i18n::t("Dina röstmodeller på Suno"))
                .strong()
                .size(Theme::FONT_S)
                .color(palette().orange()),
        );
        if self.suno_voices.is_empty() {
            ui.label(
                RichText::new(
                    self.suno_voice_hint
                        .clone()
                        .unwrap_or_else(|| crate::i18n::t("Inga röster listade än.").to_string()),
                )
                .size(Theme::FONT_S)
                .color(palette().text_muted()),
            );
            return;
        }
        let rows: Vec<(String, String)> = self
            .suno_voices
            .iter()
            .map(|(id, name)| (id.clone(), name.clone()))
            .collect();
        let mut pick: Option<usize> = None;
        for (index, (id, name)) in rows.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(name).size(Theme::FONT_S));
                ui.label(
                    RichText::new(id)
                        .monospace()
                        .size(Theme::FONT_XS)
                        .color(palette().text_muted()),
                );
                if ui.button(crate::i18n::t("Använd i Skapa")).clicked() {
                    pick = Some(index);
                }
            });
        }
        if let Some(index) = pick {
            self.suno_voice_choice = index;
            self.suno_tab = 0;
            self.suno_status = crate::i18n::t("Rösten är vald — skapa låten i Skapa.").to_string();
        }
    }

    /// **Bibliotek** — det som redan finns, med import per rad.
    fn suno_tab_library(&mut self, ui: &mut egui::Ui) {
        let pending = self.suno_pending.is_some();
        ui.horizontal(|ui| {
            ui.label(RichText::new(crate::i18n::t("Sök:")).size(Theme::FONT_S));
            ui.add(
                egui::TextEdit::singleline(&mut self.suno_library_query)
                    .desired_width(320.0)
                    .hint_text(crate::i18n::t("(tomt = allt)")),
            );
            if ui
                .add_enabled(!pending, egui::Button::new(crate::i18n::t("📚 Hämta")))
                .clicked()
            {
                self.suno_load_library();
            }
            ui.label(
                RichText::new(format!("{}", self.suno_library.len()))
                    .size(Theme::FONT_S)
                    .color(palette().text_muted()),
            );
        });
        ui.separator();
        // Raderna kopieras ut först: knappen muterar appen, och listan får inte lånas under tiden.
        let rows: Vec<(String, String, String, bool)> = self
            .suno_library
            .iter()
            .map(|song| {
                let stems = if song.stem_urls.is_empty() { "" } else { "🎚" };
                let length = song
                    .duration_secs
                    .map(|d| format!(" {d:.0}s"))
                    .unwrap_or_default();
                // **Ligger låten redan på disk kostar den inget att lyssna på.** Knappen ska
                // säga det, så ingen betalar för något de redan har.
                let has_file = self
                    .suno_clips
                    .iter()
                    .any(|clip| clip.id == song.id && clip.file.as_ref().is_some_and(|f| f.exists()));
                (
                    song.title.clone(),
                    format!("{stems}{length}"),
                    song.id.clone(),
                    has_file,
                )
            })
            .collect();
        let mut import: Option<usize> = None;
        let mut play: Option<usize> = None;
        for (index, (title, meta, index_id, has_file)) in rows.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(Theme::FONT_S));
                ui.label(
                    RichText::new(meta)
                        .size(Theme::FONT_XS)
                        .color(palette().text_muted()),
                );
                // **En hämtning kostar en kredit — en uppspelning av en fil som redan ligger
                // på disk kostar ingenting.** Låten hämtas aldrig två gånger.
                if *has_file {
                    if ui
                        .add_enabled(!pending, egui::Button::new(crate::i18n::t("🎧 Spela upp")))
                        .on_hover_text(crate::i18n::t(
                            "Filen ligger redan på disk — ingen kredit går åt",
                        ))
                        .clicked()
                    {
                        play = Some(index);
                    }
                } else if self.suno_spend_button(
                    ui,
                    !pending,
                    SunoSpend::Play,
                    index_id,
                    crate::i18n::t("🎧 Lyssna — 1 kredit"),
                ) {
                    play = Some(index);
                }
                if self.suno_spend_button(
                    ui,
                    !pending,
                    SunoSpend::Import,
                    index_id,
                    crate::i18n::t("📥 Importera — 1 kredit"),
                ) {
                    import = Some(index);
                }
            });
        }
        if let Some(index) = play {
            self.suno_play_library(index);
        }
        if let Some(index) = import {
            let id = rows[index].2.clone();
            self.suno_import_confirmed(&id);
        }
    }

    /// **Spellistor / arbetsytor / konto** — samma form: en lista, en knapp, en rad per post.
    fn suno_tab_meta(&mut self, ui: &mut egui::Ui, which: crate::ui::app::SunoMeta) {
        use crate::ui::app::SunoMeta;
        let pending = self.suno_pending.is_some();
        let (title, load) = match which {
            SunoMeta::Playlists => ("Spellistor", "📚 Hämta"),
            SunoMeta::Workspaces => ("Arbetsytor", "📚 Hämta"),
            SunoMeta::Account => ("Konto", "📚 Hämta"),
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(crate::i18n::t(title))
                    .strong()
                    .size(Theme::FONT_S)
                    .color(palette().orange()),
            );
            if ui
                .add_enabled(!pending, egui::Button::new(crate::i18n::t(load)))
                .clicked()
            {
                self.suno_load_meta(which);
            }
        });
        ui.separator();
        match which {
            SunoMeta::Playlists => {
                if self.suno_playlists.is_empty() {
                    ui.label(
                        RichText::new(crate::i18n::t("Inga spellistor listade än.").to_string())
                            .size(Theme::FONT_S)
                            .color(palette().text_muted()),
                    );
                }
                let rows: Vec<(String, String, u32)> = self
                    .suno_playlists
                    .iter()
                    .map(|p| (p.name.clone(), p.id.clone(), p.clip_count.unwrap_or(0)))
                    .collect();
                for (name, id, clips) in rows {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name).size(Theme::FONT_S));
                        ui.label(
                            RichText::new(format!("{clips}"))
                                .size(Theme::FONT_XS)
                                .color(palette().text_muted()),
                        );
                        ui.label(
                            RichText::new(id)
                                .monospace()
                                .size(Theme::FONT_XS)
                                .color(palette().text_muted()),
                        );
                    });
                }
            }
            SunoMeta::Workspaces => {
                if self.suno_workspaces.is_empty() {
                    ui.label(
                        RichText::new(crate::i18n::t("Inga arbetsytor listade än.").to_string())
                            .size(Theme::FONT_S)
                            .color(palette().text_muted()),
                    );
                }
                let rows: Vec<(String, String)> = self
                    .suno_workspaces
                    .iter()
                    .map(|w| (w.name.clone(), w.id.clone()))
                    .collect();
                for (name, id) in rows {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name).size(Theme::FONT_S));
                        ui.label(
                            RichText::new(id)
                                .monospace()
                                .size(Theme::FONT_XS)
                                .color(palette().text_muted()),
                        );
                    });
                }
            }
            SunoMeta::Account => match &self.suno_account {
                Some(account) => {
                    ui.label(
                        RichText::new(crate::tstatus!(
                            "Nedladdningskrediter: {}",
                            account
                                .credits
                                .map(|c| format!("{c:.0}"))
                                .unwrap_or_else(|| "—".to_string())
                        ))
                        .size(Theme::FONT_S),
                    );
                    if let Some(plan) = &account.plan {
                        ui.label(RichText::new(plan).size(Theme::FONT_S));
                    }
                }
                None => {
                    ui.label(
                        RichText::new(crate::i18n::t("Kontot är inte hämtat än.").to_string())
                            .size(Theme::FONT_S)
                            .color(palette().text_muted()),
                    );
                }
            },
        }
    }

    // ---- fälthelpers -------------------------------------------------

    fn suno_field_text(
        &mut self,
        ui: &mut egui::Ui,
        slot: Option<usize>,
        label: &'static str,
        hint: &'static str,
        multiline: bool,
    ) {
        let Some((_, SunoField::Text { value, .. })) = slot.and_then(|i| self.suno_form.get_mut(i))
        else {
            return;
        };
        let caption = crate::i18n::t(label).to_string();
        let placeholder = crate::i18n::t(hint).to_string();
        ui.label(RichText::new(caption).size(Theme::FONT_S));
        if multiline {
            ui.add(
                egui::TextEdit::multiline(value)
                    .desired_rows(4)
                    .desired_width(f32::INFINITY)
                    .hint_text(placeholder),
            );
        } else {
            ui.add(
                egui::TextEdit::singleline(value)
                    .desired_width(f32::INFINITY)
                    .hint_text(placeholder),
            );
        }
        ui.add_space(3.0);
    }

    fn suno_field_number(&mut self, ui: &mut egui::Ui, slot: Option<usize>, label: &'static str) {
        let Some((_, SunoField::Number { value, min, max })) =
            slot.and_then(|i| self.suno_form.get_mut(i))
        else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new(crate::i18n::t(label).to_string()).size(Theme::FONT_S));
            ui.add(
                egui::Slider::new(value, *min..=*max)
                    .custom_formatter(|v, _| format!("{:.0} %", v * 100.0)),
            );
        });
    }

    fn suno_field_choice(&mut self, ui: &mut egui::Ui, slot: Option<usize>, label: &'static str) {
        let Some((_, SunoField::Choice { options, selected })) =
            slot.and_then(|i| self.suno_form.get_mut(i))
        else {
            return;
        };
        let current = options.get(*selected).cloned().unwrap_or_default();
        ui.horizontal(|ui| {
            let caption = crate::i18n::t(label).to_string();
            ui.label(RichText::new(caption).size(Theme::FONT_S));
            egui::ComboBox::from_id_salt(label).selected_text(current).show_ui(ui, |ui| {
                for (index, option) in options.iter().enumerate() {
                    ui.selectable_value(selected, index, option);
                }
            });
        });
    }

    fn suno_field_bool(&mut self, ui: &mut egui::Ui, slot: Option<usize>, label: &'static str) {
        let Some((_, SunoField::Bool(on))) = slot.and_then(|i| self.suno_form.get_mut(i)) else {
            return;
        };
        let caption = crate::i18n::t(label).to_string();
        ui.checkbox(on, caption);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The credit gate: the first click on a spending button only arms its row, so no task is
    /// spawned; the second click on the same row confirms and is the only spending step.
    #[test]
    fn a_credit_click_arms_the_row_before_it_spends() {
        let mut armed: Option<String> = None;
        let mut spawned = 0;
        for click in 1..=2 {
            let step = step_after_click(&mut armed, SunoSpend::Clip, "clip-7");
            if step == SpendStep::Confirm {
                spawned += 1;
            }
            println!("click {click}: {step:?}, armed={armed:?}, spawned tasks={spawned}");
        }
        assert_eq!(spawned, 1, "a take was downloaded twice");
        // The confirm belongs to the button that was pressed: another action on the same id, or
        // another row, only arms itself.
        println!(
            "play on the same id: {:?}, clip on another id: {:?}",
            credit_step(Some("play:clip-7"), SunoSpend::Clip, "clip-7"),
            credit_step(Some("clip:clip-8"), SunoSpend::Clip, "clip-7"),
        );
        assert_eq!(
            credit_step(Some("play:clip-7"), SunoSpend::Clip, "clip-7"),
            SpendStep::Arm
        );
        assert_eq!(
            credit_step(Some("clip:clip-8"), SunoSpend::Clip, "clip-7"),
            SpendStep::Arm
        );
    }
}
