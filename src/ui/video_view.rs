//! **Videospår** i gränssnittet: en referensfilm synkad till tidslinjen, så musik kan
//! skrivas mot bild. Filmen ligger inte i mixen och går inte genom mastern.
//!
//! Bildrutan hämtas med `ffmpeg` bara när **rutan byts**, och högst tio gånger i sekunden:
//! ett processanrop per bildruta hade kostat mer än uppspelningen. Under en transport som
//! rör sig snabbt visas därför den senast hämtade rutan tills nästa hinner fram — hellre en
//! ruta efter än en uppspelning som hackar.

use eframe::egui;
use crate::ui::theme::Theme;

/// Allt videovyn minns mellan bildrutor.
#[derive(Default)]
pub struct VideoState {
    pub path: Option<String>,
    pub info: Option<crate::audio::video::VideoInfo>,
    /// Var filmen börjar på tidslinjen, i sekunder.
    pub offset_secs: f64,
    texture: Option<egui::TextureHandle>,
    shown_frame: Option<u64>,
    last_update: f64,
    pub last_error: Option<String>,
}

impl VideoState {
    /// Laddar en fil och läser dess mått. Fel hamnar i `last_error` i stället för att panika.
    pub fn load(&mut self, path: &str) {
        match crate::audio::video::probe(path) {
            Ok(info) => {
                self.path = Some(path.to_string());
                self.info = Some(info);
                self.texture = None;
                self.shown_frame = None;
                self.last_error = None;
            }
            Err(err) => {
                self.last_error = Some(err);
                self.path = None;
                self.info = None;
            }
        }
    }

    /// Hämtar rutan för tidslinjepositionen om det behövs. `now` är en monoton klocka i
    /// sekunder (används för att begränsa hur ofta ffmpeg startas).
    fn refresh(&mut self, ctx: &egui::Context, position_secs: f64, now: f64) {
        let (Some(path), Some(info)) = (self.path.clone(), self.info) else {
            return;
        };
        let Some(index) = crate::audio::video::frame_index(position_secs, &info, self.offset_secs)
        else {
            // Före filmens början eller efter slutet: tomt spår, inte fryst bild.
            self.texture = None;
            self.shown_frame = None;
            return;
        };
        if self.shown_frame == Some(index) || now - self.last_update < 0.1 {
            return;
        }
        match crate::audio::video::frame_rgb(&path, index as f64 / info.fps, &info) {
            Ok(rgb) => {
                let image = egui::ColorImage::from_rgb(
                    [info.width as usize, info.height as usize],
                    &rgb,
                );
                self.texture =
                    Some(ctx.load_texture("sonix_video", image, egui::TextureOptions::LINEAR));
                self.shown_frame = Some(index);
                self.last_update = now;
                self.last_error = None;
            }
            Err(err) => {
                self.last_error = Some(err);
                self.last_update = now;
            }
        }
    }
}

/// Ritade videovyn. `song_time` är tidslinjens position i sekunder.
pub fn render_video_view(
    ui: &mut egui::Ui,
    state: &mut VideoState,
    song_time: f64,
    is_playing: bool,
    status_msg: &mut String,
) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(crate::i18n::t("🎬 VIDEO (referensbild)"))
                    .strong()
                    .size(Theme::FONT_M)
                    .color(crate::ui::theme::palette().cyan()),
            );
            ui.separator();
            if ui.button(crate::i18n::t("🎬 Ladda video…")).clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .add_filter(
                        crate::i18n::t("Video"),
                        &["mp4", "mov", "mkv", "webm", "avi", "m4v"],
                    )
                    .pick_file()
                {
                    state.load(path.to_string_lossy().as_ref());
                    *status_msg = match &state.info {
                        Some(info) => crate::tstatus!(
                            "🎬 {} — {}×{}, {:.2} fps, {:.1} s",
                            path.file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default(),
                            info.width,
                            info.height,
                            info.fps,
                            info.duration_secs
                        ),
                        None => crate::tstatus!(
                            "❌ {}",
                            state.last_error.clone().unwrap_or_default()
                        ),
                    };
                }
            ui.separator();
            ui.label(crate::i18n::t("Börjar på tidslinjen (s):"));
            ui.add(
                egui::DragValue::new(&mut state.offset_secs)
                    .range(-600.0..=3600.0)
                    .speed(0.1)
                    .fixed_decimals(2),
            )
            .on_hover_text(crate::i18n::t(
                "Var filmen börjar. Före det och efter filmens slut är spåret tomt.",
            ));
        });

        ui.add_space(4.0);

        state.refresh(ui.ctx(), song_time, ui.input(|i| i.time));

        if let Some(texture) = &state.texture {
            let available = ui.available_size();
            let size = texture.size_vec2();
            // Behåll bildförhållandet: filmen ska inte sträckas ut i panelen.
            let scale = (available.x / size.x).min((available.y - 8.0).max(1.0) / size.y).min(1.0);
            ui.image((texture.id(), size * scale.max(0.05)));
        } else if state.path.is_some() {
            ui.label(
                egui::RichText::new(crate::i18n::t(
                    "Ingen bild här: spåret är tomt före filmens början eller efter dess slut.",
                ))
                .color(crate::ui::theme::palette().text_muted()),
            );
        } else {
            ui.label(
                egui::RichText::new(crate::i18n::t(
                    "Ladda en film för att se den mot tidslinjen. Filmen påverkar inte ljudet.",
                ))
                .color(crate::ui::theme::palette().text_muted()),
            );
        }

        if let Some(err) = &state.last_error {
            ui.label(egui::RichText::new(format!("❌ {err}")).color(crate::ui::theme::palette().red()));
        }
        if is_playing && state.info.is_some() {
            ui.label(
                egui::RichText::new(crate::i18n::t("Spår: bildrutan följer uppspelningen"))
                    .size(Theme::FONT_S)
                    .color(crate::ui::theme::palette().text_muted()),
            );
        }
    });
}
