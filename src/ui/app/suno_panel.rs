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

/// **Dagrubriken över en lista med låtar.** Dagens datum sägs som "Idag" — jämförelsen görs mot
/// systemklockan i UTC, samma zon som serverns tidsstämplar, så rubriken aldrig ljuger.
fn day_label(day: &str) -> String {
    let today = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0);
    let stamp = |y: i64, m: i64, d: i64| -> i64 {
        // Civil-from-days (Howard Hinnant): dagar sedan 1970-01-01 för ett datum i UTC.
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    };
    let parsed = day
        .split('-')
        .filter_map(|part| part.parse::<i64>().ok())
        .collect::<Vec<_>>();
    if parsed.len() == 3 && stamp(parsed[0], parsed[1], parsed[2]) == today as i64 {
        return crate::i18n::t("Idag").to_string();
    }
    day.to_string()
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
    /// **Tre kolumner, som Sunos egen sida:** menyn till vänster, skapandet i mitten och
    /// arbetsytan till höger — så att låtarna syns medan man skriver nästa.
    pub(crate) fn render_suno_view(&mut self, ui: &mut egui::Ui) {
        self.suno_poll_covers(ui.ctx());
        self.suno_header(ui);
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
                    RichText::new(crate::i18n::t(
                        "Öppna den här adressen om webbläsaren inte öppnades:",
                    ))
                    .size(Theme::FONT_S)
                    .color(palette().text_muted()),
                );
                let mut shown = login_url.clone();
                ui.add(
                    egui::TextEdit::singleline(&mut shown)
                        .desired_width(420.0)
                        .font(egui::TextStyle::Monospace),
                );
                if ui.button(crate::i18n::t("🌐 Öppna igen")).clicked() {
                    let _ = crate::platform::open_url(&login_url);
                }
            });
        }
        ui.separator();

        let height = ui.available_height();
        let rail = 128.0;
        let workspace = 372.0;
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                Vec2::new(rail, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.suno_nav_rail(ui),
            );
            ui.separator();
            let centre = (ui.available_width() - workspace - 16.0).max(300.0);
            ui.allocate_ui_with_layout(
                Vec2::new(centre, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.suno_centre(ui),
            );
            ui.separator();
            ui.allocate_ui_with_layout(
                Vec2::new(workspace, height),
                egui::Layout::top_down(egui::Align::Min),
                |ui| self.suno_workspace_column(ui),
            );
        });
        self.suno_player_bar(ui);
    }

    /// **Spelaren längst ned, i full bredd** — som Sunos egen rad: titel, tid, vågform med
    /// läget utritat, och stopp. Ljudet går genom appens egen motor, inte genom VLC.
    fn suno_player_bar(&mut self, ui: &mut egui::Ui) {
        let Some(player) = self.suno_player.clone() else {
            return;
        };
        // Låten är slut när motorn inte spelar längre — då försvinner raden av sig själv.
        let playing = self.engine.is_audition_playing();
        if !playing && player.started.is_some_and(|at| at.elapsed().as_secs_f32() > 0.3) {
            self.suno_player = None;
            return;
        }
        ui.separator();
        ui.horizontal(|ui| {
            let button = if playing { "■" } else { "▶" };
            if ui
                .add(egui::Button::new(RichText::new(button).size(Theme::FONT_M)))
                .on_hover_text(crate::i18n::t("Spela / stoppa"))
                .clicked()
            {
                if playing {
                    let _ = self.engine.send_command(crate::audio::AudioCommand::StopAudition);
                } else if let Some(start) = player.started.as_ref() {
                    let _ = start;
                    // Startar om från början: filen är redan avkodad en gång, så det är
                    // samma väg som första gången.
                    if let Some(file) = self
                        .suno_clips
                        .iter()
                        .find(|clip| clip.title == player.title)
                        .and_then(|clip| clip.file.clone())
                    {
                        let title = player.title.clone();
                        self.suno_play_local(&file, &title);
                    }
                }
            }
            ui.label(
                RichText::new(&player.title)
                    .strong()
                    .size(Theme::FONT_S)
                    .color(palette().text()),
            );
            let elapsed = player.elapsed();
            ui.label(
                RichText::new(format!(
                    "{}:{:02} / {}:{:02}",
                    (elapsed as u64) / 60,
                    (elapsed as u64) % 60,
                    (player.seconds as u64) / 60,
                    (player.seconds as u64) % 60
                ))
                .size(Theme::FONT_XS)
                .color(palette().text_muted()),
            );
            // Vågformen: en stapel per peak, den spelade delen i orange.
            let width = (ui.available_width() - 40.0).max(80.0);
            let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 26.0), egui::Sense::hover());
            let painter = ui.painter();
            let played = if player.seconds > 0.0 {
                (elapsed / player.seconds).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let count = player.peaks.len().max(1);
            let step = rect.width() / count as f32;
            for (index, peak) in player.peaks.iter().enumerate() {
                let height = (peak.clamp(0.0, 1.0) * rect.height() * 0.9).max(1.5);
                let x = rect.left() + index as f32 * step;
                let colour = if (index as f32 / count as f32) <= played {
                    palette().orange()
                } else {
                    palette().text_muted()
                };
                painter.rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(x, rect.center().y - height / 2.0),
                        Vec2::new((step - 1.0).max(1.0), height),
                    ),
                    1.0,
                    colour,
                );
            }
            if ui.button("✕").on_hover_text(crate::i18n::t("Stäng")).clicked() {
                let _ = self.engine.send_command(crate::audio::AudioCommand::StopAudition);
                self.suno_player = None;
            }
        });
    }

    /// Rubriken: namn, läge, krediter och den senaste repliken från servern.
    fn suno_header(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("☀ SUNO")
                    .strong()
                    .size(Theme::FONT_L)
                    .color(palette().orange()),
            );
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
            // **Krediterna står i huvudet, inte som en överraskning i knappen.** Två pooler:
            // nedladdningar (som en hämtning tar ur) och genereringar.
            if let Some(credits) = self.suno_account.as_ref().and_then(|a| a.credits) {
                ui.label(
                    RichText::new(crate::tstatus!("⬇ {} nedladdningar", credits as i64))
                        .size(Theme::FONT_XS)
                        .color(palette().text_muted()),
                );
            }
            if !self.suno_status.trim().is_empty() {
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
    }

    /// Vänstermenyn: en rad per ruta, den aktiva med fylld bakgrund.
    fn suno_nav_rail(&mut self, ui: &mut egui::Ui) {
        for (index, label) in TABS.iter().enumerate() {
            let active = self.suno_tab == index;
            let text = RichText::new(crate::i18n::t(label)).size(Theme::FONT_S);
            let text = if active {
                text.strong().color(palette().text())
            } else {
                text.color(palette().text_muted())
            };
            let fill = if active {
                palette().panel()
            } else {
                egui::Color32::TRANSPARENT
            };
            if ui
                .add(
                    egui::Button::new(text)
                        .fill(fill)
                        .min_size(Vec2::new(112.0, 26.0)),
                )
                .clicked()
            {
                self.suno_tab = index;
            }
        }
    }

    /// Mittkolumnen: rutan som är vald, och Skapa-knappen längst ned i full bredd.
    fn suno_centre(&mut self, ui: &mut egui::Ui) {
        // **Tre slag av skapande, som Suno:** låtar, tal och korta ljud. Servern har egna
        // verktyg för varje (`audio-create`, `speech-create`, `sounds-create`).
        ui.horizontal(|ui| {
            for (mode, label) in [(0u8, "🎵 Låtar"), (1, "🗣 Tal"), (2, "🔔 Korta ljud")] {
                let active = self.suno_mode == mode;
                let text = RichText::new(crate::i18n::t(label)).size(Theme::FONT_S);
                let text = if active {
                    text.strong().color(palette().orange())
                } else {
                    text.color(palette().text_muted())
                };
                if ui.add(egui::Button::new(text).fill(egui::Color32::TRANSPARENT)).clicked() {
                    self.suno_mode = mode;
                }
            }
        });
        ui.separator();
        let bottom = 44.0;
        let height = (ui.available_height() - bottom).max(120.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.suno_mode {
                        1 => self.suno_tab_alt(ui, 1),
                        2 => self.suno_tab_alt(ui, 2),
                        _ => match self.suno_tab {
                            0 => self.suno_tab_create(ui),
                            1 => self.suno_tab_voices(ui),
                            2 => self.suno_tab_library(ui),
                            3 => self.suno_tab_meta(ui, crate::ui::app::SunoMeta::Playlists),
                            4 => self.suno_tab_meta(ui, crate::ui::app::SunoMeta::Workspaces),
                            _ => self.suno_tab_meta(ui, crate::ui::app::SunoMeta::Account),
                        },
                    });
            },
        );
        // **Skapa-knappen sitter längst ned, i full bredd** — det är den handling hela rutan
        // finns för, och hos Suno är det den enda knappen som lyser.
        let width = ui.available_width();
        let enabled = self.suno_pending.is_none() && (self.suno_mode > 0 || self.suno_tab == 0);
        if ui
            .add_enabled(
                enabled,
                egui::Button::new(
                    RichText::new(crate::i18n::t("♫ Skapa"))
                        .strong()
                        .size(Theme::FONT_M)
                        .color(egui::Color32::BLACK),
                )
                .fill(palette().orange())
                .min_size(Vec2::new(width, 32.0)),
            )
            .clicked()
        {
            match self.suno_mode {
                0 => self.suno_create(),
                mode => self.suno_create_alt(mode),
            }
        }
    }

    /// **Tal eller korta ljud.** Fälten kommer ur serverns schema för verktyget; etiketterna
    /// är fältets eget namn, för servern har inga svenska etiketter att hämta.
    fn suno_tab_alt(&mut self, ui: &mut egui::Ui, mode: u8) {
        let tool = if mode == 1 { "speech-create" } else { "sounds-create" };
        let empty = if mode == 1 {
            self.suno_speech_form.is_empty()
        } else {
            self.suno_sounds_form.is_empty()
        };
        if empty {
            let schema = self
                .suno_alt_schemas
                .iter()
                .find(|(name, _)| name == tool)
                .map(|(_, schema)| schema.clone());
            let fields = schema
                .map(|schema| crate::ui::app::suno_form::fields_from_schema(&schema))
                .unwrap_or_default();
            if fields.is_empty() {
                ui.label(
                    RichText::new(crate::i18n::t(
                        "Anslut för att hämta serverns fält — de gissas inte fram.",
                    ))
                    .size(Theme::FONT_S)
                    .color(palette().text_muted()),
                );
                return;
            }
            if mode == 1 {
                self.suno_speech_form = fields;
            } else {
                self.suno_sounds_form = fields;
            }
        }
        let form = if mode == 1 {
            &mut self.suno_speech_form
        } else {
            &mut self.suno_sounds_form
        };
        ui.add_space(4.0);
        for (name, field) in form.iter_mut() {
            let label = {
                let mut chars = name.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => name.clone(),
                }
            };
            match field {
                SunoField::Text { value, multiline } => {
                    ui.label(RichText::new(&label).size(Theme::FONT_S));
                    if *multiline {
                        ui.add(
                            egui::TextEdit::multiline(value)
                                .desired_rows(3)
                                .desired_width(f32::INFINITY),
                        );
                    } else {
                        ui.add(
                            egui::TextEdit::singleline(value).desired_width(f32::INFINITY),
                        );
                    }
                }
                SunoField::Bool(value) => {
                    ui.checkbox(value, &label);
                }
                SunoField::Number { value, min, max } => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&label).size(Theme::FONT_S));
                        ui.add(egui::DragValue::new(value).range(*min..=*max));
                    });
                }
                SunoField::Choice { options, selected } => {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&label).size(Theme::FONT_S));
                        let shown = options
                            .get(*selected)
                            .cloned()
                            .unwrap_or_else(|| crate::i18n::t("(inget)").to_string());
                        egui::ComboBox::from_id_salt(format!("suno-alt-{tool}-{name}"))
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for (index, option) in options.iter().enumerate() {
                                    if ui.selectable_label(*selected == index, option).clicked() {
                                        *selected = index;
                                    }
                                }
                            });
                    });
                }
            }
        }
    }

    /// **Omslagen kommer från servern.** De hämtas en gång per adress i en egen tråd och
    /// läggs som textur — en rad som ritas om 60 gånger i sekunden får aldrig hämta en bild.
    fn suno_poll_covers(&mut self, ctx: &egui::Context) {
        let finished: Vec<(String, egui::ColorImage)> = self
            .suno_cover_rx
            .lock()
            .map(|mut queue| queue.drain(..).collect())
            .unwrap_or_default();
        for (url, image) in finished {
            let texture = ctx.load_texture(&url, image, egui::TextureOptions::LINEAR);
            self.suno_covers.insert(url, texture);
        }
    }

    /// Texturen för en bildadress, eller `None` medan den hämtas (då ritas en platshållare).
    fn suno_cover_texture(&mut self, url: &str) -> Option<egui::TextureHandle> {
        if let Some(texture) = self.suno_covers.get(url) {
            return Some(texture.clone());
        }
        self.suno_spawn_cover(url);
        None
    }

    /// **Omslagen beställs när listan kommer**, inte först när panelen ritar den — då hinner
    /// bilderna bli färdiga innan ytan öppnas.
    pub(crate) fn suno_prefetch_covers(&mut self) {
        let urls: Vec<String> = self
            .suno_library
            .iter()
            .filter_map(|song| song.cover_url.clone())
            .take(60)
            .collect();
        for url in urls {
            if !self.suno_covers.contains_key(&url) {
                self.suno_spawn_cover(&url);
            }
        }
    }

    /// Startar en hämtning av en bild, en gång per adress.
    fn suno_spawn_cover(&mut self, url: &str) {
        if self.suno_cover_tries.insert(url.to_string()) {
            let url = url.to_string();
            let sink = self.suno_cover_rx.clone();
            std::thread::spawn(move || {
                let agent = ureq::AgentBuilder::new()
                    .timeout(std::time::Duration::from_secs(20))
                    .build();
                let response = match agent.get(&url).call() {
                    Ok(response) => response,
                    Err(error) => {
                        println!("suno: omslaget föll på hämtningen ({error})");
                        return;
                    }
                };
                let mut bytes = Vec::new();
                {
                    use std::io::Read;
                    if response
                        .into_reader()
                        .take(4_000_000)
                        .read_to_end(&mut bytes)
                        .is_err()
                    {
                        return;
                    }
                }
                let decoded = match image::load_from_memory(&bytes) {
                    Ok(decoded) => decoded,
                    Err(error) => {
                        println!("suno: omslaget gick inte att avkoda ({error})");
                        return;
                    }
                };
                let rgba = decoded.to_rgba8();
                let size = [rgba.width() as usize, rgba.height() as usize];
                let image = egui::ColorImage::from_rgba_unmultiplied(size, &rgba.into_raw());
                if let Ok(mut queue) = sink.lock() {
                    queue.push((url, image));
                }
            });
        }
    }

    /// **Högerkolumnen: arbetsytan.** Sökfält, dagrubriker och en rad per låt med omslag,
    /// titel, stil och de två handlingarna — som Sunos egen lista.
    fn suno_workspace_column(&mut self, ui: &mut egui::Ui) {
        let pending = self.suno_pending.is_some();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(crate::i18n::t("Min arbetsyta"))
                    .strong()
                    .size(Theme::FONT_M),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_enabled(!pending, egui::Button::new(crate::i18n::t("🔄")))
                    .on_hover_text(crate::i18n::t("Hämta biblioteket igen"))
                    .clicked()
                {
                    self.suno_load_library();
                }
                ui.label(
                    RichText::new(format!("{}", self.suno_library.len()))
                        .size(Theme::FONT_XS)
                        .color(palette().text_muted()),
                );
            });
        });
        ui.add(
            egui::TextEdit::singleline(&mut self.suno_library_query)
                .desired_width(f32::INFINITY)
                .hint_text(crate::i18n::t("Sök")),
        );
        // **Filtren, som hos Suno:** pillren silar listan lokalt. Ingen serverfråga, ingen kredit.
        ui.horizontal_wrapped(|ui| {
            for (value, label) in [
                (0u8, "Alla"),
                (1, "♥ Gillade"),
                (2, "Publika"),
                (3, "Uppladdningar"),
            ] {
                let active = self.suno_filter == value;
                let text = RichText::new(crate::i18n::t(label)).size(Theme::FONT_XS);
                let text = if active {
                    text.color(palette().bg())
                } else {
                    text.color(palette().text_muted())
                };
                let fill = if active { palette().orange() } else { palette().panel() };
                if ui.add(egui::Button::new(text).fill(fill)).clicked() {
                    self.suno_filter = value;
                }
            }
        });
        ui.add_space(6.0);

        struct Row {
            id: String,
            title: String,
            style: String,
            description: String,
            duration: String,
            cover: Option<String>,
            has_file: bool,
            day: String,
            liked: bool,
        }
        let query = self.suno_library_query.trim().to_lowercase();
        let rows: Vec<Row> = self
            .suno_library
            .iter()
            .filter(|song| match self.suno_filter {
                1 => song.liked,
                2 => song.visibility.as_deref() == Some("public"),
                3 => song.kind.as_deref() == Some("upload"),
                _ => true,
            })
            .filter(|song| {
                query.is_empty()
                    || song.title.to_lowercase().contains(&query)
                    || song
                        .style
                        .as_deref()
                        .is_some_and(|s| s.to_lowercase().contains(&query))
                    || song
                        .description
                        .as_deref()
                        .is_some_and(|s| s.to_lowercase().contains(&query))
            })
            .map(|song| Row {
                id: song.id.clone(),
                title: song.title.clone(),
                style: song.style.clone().unwrap_or_default(),
                description: song.description.clone().unwrap_or_default(),
                duration: song
                    .duration_secs
                    .map(|d| format!("{}:{:02}", (d as u64) / 60, (d as u64) % 60))
                    .unwrap_or_default(),
                cover: song.cover_url.clone(),
                has_file: self.suno_clips.iter().any(|clip| {
                    clip.id == song.id && clip.file.as_ref().is_some_and(|f| f.exists())
                }),
                day: song
                    .created
                    .as_deref()
                    .map(|c| c.chars().take(10).collect())
                    .unwrap_or_default(),
                liked: song.liked,
            })
            .collect();
        if rows.is_empty() {
            ui.label(
                RichText::new(crate::i18n::t("Inga låtar här än — hämta biblioteket med 🔄."))
                    .size(Theme::FONT_S)
                    .color(palette().text_muted()),
            );
            return;
        }
        let mut play: Option<usize> = None;
        let mut import: Option<usize> = None;
        let mut like: Option<usize> = None;
        let mut share: Option<usize> = None;
        let mut remix: Option<usize> = None;
        let mut last_day = String::new();
        for (index, row) in rows.iter().enumerate() {
            if row.day != last_day {
                ui.add_space(4.0);
                ui.label(
                    RichText::new(if row.day.is_empty() {
                        crate::i18n::t("Tidigare").to_string()
                    } else {
                        day_label(&row.day)
                    })
                    .size(Theme::FONT_XS)
                    .color(palette().text_muted()),
                );
                last_day = row.day.clone();
            }
            egui::Frame::none()
                .fill(palette().panel())
                .rounding(6.0)
                .inner_margin(6.0)
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        // Omslaget, eller en platshållare med titelns första bokstav.
                        match row
                            .cover
                            .as_ref()
                            .and_then(|url| self.suno_cover_texture(url))
                        {
                            Some(texture) => {
                                ui.add(
                                    egui::Image::new((texture.id(), Vec2::splat(48.0)))
                                        .rounding(4.0),
                                );
                            }
                            None => {
                                let (rect, _) = ui.allocate_exact_size(
                                    Vec2::splat(48.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(rect, 4.0, palette().bg());
                                let initial = row
                                    .title
                                    .chars()
                                    .next()
                                    .map(|c| c.to_uppercase().to_string())
                                    .unwrap_or_default();
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    initial,
                                    egui::FontId::proportional(20.0),
                                    palette().text_muted(),
                                );
                            }
                        }
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&row.title)
                                        .strong()
                                        .size(Theme::FONT_S)
                                        .color(palette().text()),
                                );
                                if !row.duration.is_empty() {
                                    ui.label(
                                        RichText::new(&row.duration)
                                            .size(Theme::FONT_XS)
                                            .color(palette().text_muted()),
                                    );
                                }
                                // **Gilla, dela och remixa** sitter på titelraden, högerställda:
                                // fem knappar på en rad blev en klippt rad i en smal kolumn.
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let heart = if row.liked { "♥" } else { "♡" };
                                        if ui
                                            .add_enabled(!pending, egui::Button::new(heart))
                                            .on_hover_text(crate::i18n::t("Gilla"))
                                            .clicked()
                                        {
                                            like = Some(index);
                                        }
                                        if ui
                                            .add(egui::Button::new("🔗"))
                                            .on_hover_text(crate::i18n::t(
                                                "Dela — öppnar låtens sida",
                                            ))
                                            .clicked()
                                        {
                                            share = Some(index);
                                        }
                                        if ui
                                            .add(egui::Button::new("✎"))
                                            .on_hover_text(crate::i18n::t(
                                                "Remixa — stilen och beskrivningen in i formuläret",
                                            ))
                                            .clicked()
                                        {
                                            remix = Some(index);
                                        }
                                    },
                                );
                            });
                            if !row.style.is_empty() {
                                // **Stilen är en hel prompt** (mätt: sex rader), inte en tagg —
                                // en rad i orange, hela texten på hover.
                                let short = if row.style.chars().count() > 52 {
                                    format!("{}…", row.style.chars().take(52).collect::<String>())
                                } else {
                                    row.style.clone()
                                };
                                let _ = ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new(short)
                                                .size(Theme::FONT_XS)
                                                .color(palette().orange()),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(&row.style);
                            }
                            if !row.description.is_empty() {
                                // **Två rader, inte en vägg av text.** Beskrivningen är en
                                // hel prompt; hela texten står i hover-texten.
                                let short = if row.description.chars().count() > 96 {
                                    format!("{}…", row.description.chars().take(96).collect::<String>())
                                } else {
                                    row.description.clone()
                                };
                                // **En rad, som hos Suno** — beskrivningen är en hel prompt och
                                // skulle annars äta hela kolumnen. Hela texten på hover.
                                let _ = ui
                                    .add(
                                        egui::Label::new(
                                            RichText::new(short)
                                                .size(Theme::FONT_XS)
                                                .color(palette().text_muted()),
                                        )
                                        .truncate(),
                                    )
                                    .on_hover_text(&row.description);
                            }
                            ui.horizontal(|ui| {
                                // **En hämtning kostar en kredit — en uppspelning av en fil
                                // som redan ligger på disk kostar ingenting.**
                                if row.has_file {
                                    if ui
                                        .add_enabled(
                                            !pending,
                                            egui::Button::new(crate::i18n::t("▶ Spela")),
                                        )
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
                                    &row.id,
                                    crate::i18n::t("▶ Lyssna — 1 kredit"),
                                ) {
                                    play = Some(index);
                                }
                                if self.suno_spend_button(
                                    ui,
                                    !pending,
                                    SunoSpend::Import,
                                    &row.id,
                                    crate::i18n::t("📥 Importera"),
                                ) {
                                    import = Some(index);
                                }
                            });
                        });
                    });
                });
            ui.add_space(4.0);
        }
        if let Some(index) = play {
            self.suno_play_library(index);
        }
        if let Some(index) = import {
            let id = rows[index].id.clone();
            self.suno_import_confirmed(&id);
        }
        if let Some(index) = like {
            // Raden i biblioteket, inte i den filtrerade listan: index pekar på `rows`.
            let id = rows[index].id.clone();
            if let Some(pos) = self.suno_library.iter().position(|song| song.id == id) {
                self.suno_toggle_like(pos);
            }
        }
        if let Some(index) = share {
            let id = rows[index].id.clone();
            if let Some(pos) = self.suno_library.iter().position(|song| song.id == id) {
                self.suno_share(pos);
            }
        }
        if let Some(index) = remix {
            let id = rows[index].id.clone();
            if let Some(pos) = self.suno_library.iter().position(|song| song.id == id) {
                self.suno_remix(pos);
            }
        }
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
        // **Samma växel som Sunos:** Enkel visar det man skriver, Avancerat fäller ut
        // "Fler val". Standard är Avancerat — ingenting göms för den som redan vant sig.
        ui.horizontal(|ui| {
            for (advanced, label) in [(false, "Enkel"), (true, "Avancerat")] {
                let active = self.suno_advanced == advanced;
                let text = RichText::new(crate::i18n::t(label)).size(Theme::FONT_XS);
                // Aktiv = orange botten med mörk text; panelgrått på panelgrått gick inte att
                // läsa (mätt på skärmdump: bara den inaktiva syntes).
                let text = if active {
                    text.strong().color(palette().bg())
                } else {
                    text.color(palette().text_muted())
                };
                let fill = if active {
                    palette().orange()
                } else {
                    egui::Color32::TRANSPARENT
                };
                if ui.add(egui::Button::new(text).fill(fill)).clicked() {
                    self.suno_advanced = advanced;
                }
            }
        });
        ui.add_space(4.0);
        ui.horizontal_top(|ui| {
            let form_width = if self.suno_advanced { width * 0.55 - 8.0 } else { width - 8.0 };
            ui.allocate_ui(Vec2::new(form_width, 0.0), |ui| {
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
                    if !self.suno_advanced {
                        return;
                    }
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
