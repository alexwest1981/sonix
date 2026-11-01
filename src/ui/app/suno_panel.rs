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
            if ui
                .add_enabled(
                    self.suno_pending.is_none(),
                    egui::Button::new(crate::i18n::t("🔑 Logga in / anslut")),
                )
                .clicked()
            {
                self.suno_connect();
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
                ui.label(
                    RichText::new(&self.suno_status)
                        .size(Theme::FONT_S)
                        .color(palette().cyan()),
                );
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
                3 => self.suno_tab_tools(ui, "playlist"),
                4 => self.suno_tab_tools(ui, "workspace"),
                _ => self.suno_tab_tools(ui, "account"),
            });
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
                    ui.horizontal_wrapped(|ui| {
                        for (label, hint) in [
                            ("🎵 Audio", "kräver ett uppladdningsverktyg"),
                            ("🎤 Voice", "använd fliken Röster"),
                            ("💡 Inspo", "kräver ett Inspo-verktyg"),
                        ] {
                            ui.add_enabled(false, egui::Button::new(label))
                                .on_disabled_hover_text(hint);
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
                        (slots.model, "Modell"),
                    ] {
                        self.suno_field_choice(ui, slot, label);
                    }
                    for (slot, label) in [
                        (slots.instrumental, "Instrumental (ingen sång)"),
                        (slots.max_mode, "Max-läge"),
                        (slots.personalize, "Personifiera"),
                    ] {
                        self.suno_field_bool(ui, slot, label);
                    }
                    self.suno_field_text(ui, slots.title, "Låtens titel", "(valfritt)", false);
                    self.suno_field_text(ui, slots.workspace, "Spara i", "Min arbetsyta", false);
                });
            });
        });
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
        let rows: Vec<(String, String)> = self
            .suno_library
            .iter()
            .map(|song| {
                let stems = if song.stem_urls.is_empty() { "" } else { "🎚" };
                let length = song
                    .duration_secs
                    .map(|d| format!(" {d:.0}s"))
                    .unwrap_or_default();
                (song.title.clone(), format!("{stems}{length}"))
            })
            .collect();
        let mut import: Option<usize> = None;
        for (index, (title, meta)) in rows.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(title).size(Theme::FONT_S));
                ui.label(
                    RichText::new(meta)
                        .size(Theme::FONT_XS)
                        .color(palette().text_muted()),
                );
                if ui
                    .add_enabled(!pending, egui::Button::new(crate::i18n::t("📥 Importera")))
                    .clicked()
                {
                    import = Some(index);
                }
            });
        }
        if let Some(index) = import {
            self.suno_import_song(index);
        }
    }

    /// **Spellistor / arbetsytor / konto** — samma ruta tills vidare: rätt verktyg, rätt fråga.
    fn suno_tab_tools(&mut self, ui: &mut egui::Ui, kind: &str) {
        let (title, tool) = match kind {
            "playlist" => ("Spellistor", "playlists-list"),
            "workspace" => ("Arbetsytor", "workspaces-list"),
            _ => ("Konto", "account-usage"),
        };
        ui.label(
            RichText::new(title)
                .strong()
                .size(Theme::FONT_S)
                .color(palette().orange()),
        );
        ui.label(
            RichText::new(crate::tstatus!("☀ Verktyg: {}", tool))
                .size(Theme::FONT_XS)
                .color(palette().text_muted()),
        );
        // Verktygsytan finns redan i `suno_tools`; här listas den så att inget är dolt.
        let names: Vec<String> = self.suno_tools.clone();
        for name in names.iter().filter(|n| {
            let n = n.to_lowercase();
            match kind {
                "playlist" => n.contains("playlist"),
                "workspace" => n.contains("workspace"),
                _ => n.contains("account"),
            }
        }) {
            ui.label(RichText::new(name).monospace().size(Theme::FONT_XS));
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
