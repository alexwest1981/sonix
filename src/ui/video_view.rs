//! **Videospår** i gränssnittet: en referensfilm synkad till tidslinjen, så musik kan
//! skrivas mot bild. Filmen ligger inte i mixen och går inte genom mastern.
//!
//! While the transport rolls the picture comes from `Playback` — one ffmpeg writing a
//! continuous stream, read at the clip's own frame rate. Paused scrubbing still decodes one
//! frame per step with `frame_at`, at most ten times a second, because a scrub that moves
//! faster than ffmpeg can seek is better served by the frame that is already there.

use eframe::egui;
use crate::audio::video_engine::Playback;
use crate::ui::theme::Theme;

/// Allt videovyn minns mellan bildrutor.
#[derive(Default)]
pub struct VideoState {
    pub path: Option<String>,
    pub info: Option<crate::audio::video::VideoInfo>,
    /// The frame source: it keeps the last frames decoded, so scrubbing over the same
    /// neighbourhood does not start ffmpeg again for every step.
    source: Option<crate::audio::video_engine::VideoSource>,
    /// The running decoder, only while the transport plays. Dropping it stops the child.
    playback: Option<Playback>,
    /// Where playback started in the file and when, so a jump of the transport can be told
    /// apart from normal running.
    play_from: f64,
    play_started: f64,
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
        match crate::audio::video_engine::VideoSource::open(path) {
            Ok(source) => {
                self.path = Some(path.to_string());
                self.info = Some(source.info);
                self.source = Some(source);
                self.playback = None;
                self.texture = None;
                self.shown_frame = None;
                self.last_error = None;
            }
            Err(err) => {
                self.last_error = Some(err);
                self.path = None;
                self.info = None;
                self.source = None;
            }
        }
    }

    /// Hämtar rutan för tidslinjepositionen om det behövs. `now` är en monoton klocka i
    /// sekunder (används för att begränsa hur ofta ffmpeg startas).
    fn refresh(&mut self, ctx: &egui::Context, position_secs: f64, is_playing: bool, now: f64) {
        let Some(info) = self.info else {
            return;
        };
        let Some(index) = crate::audio::video::frame_index(position_secs, &info, self.offset_secs)
        else {
            // Före filmens början eller efter slutet: tomt spår, inte fryst bild.
            self.playback = None;
            self.texture = None;
            self.shown_frame = None;
            return;
        };

        if is_playing {
            let local = index as f64 / info.fps;
            if self.playback.is_none() {
                let Some(path) = self.path.clone() else {
                    return;
                };
                let mut play = Playback::new(&path, info);
                if let Err(err) = play.start(local) {
                    self.last_error = Some(err);
                    return;
                }
                self.playback = Some(play);
                self.play_from = local;
                self.play_started = now;
            }
            // A jump of more than a quarter second is a scrub during playback, not drift.
            if (local - (self.play_from + now - self.play_started)).abs() > 0.25 {
                if let Some(play) = self.playback.as_mut()
                    && let Err(err) = play.seek(local)
                {
                    self.last_error = Some(err);
                    return;
                }
                self.play_from = local;
                self.play_started = now;
            }
            if let Some(frame) = self.playback.as_mut().and_then(|play| play.latest()) {
                self.show(ctx, &frame, index, now);
            }
            return;
        }

        // Paused: the one-shot path decodes the scrubbed frame and reports what it finds.
        self.playback = None;
        if self.shown_frame == Some(index) || now - self.last_update < 0.1 {
            return;
        }
        let Some(source) = self.source.as_mut() else {
            return;
        };
        match source.frame_at(index as f64 / info.fps) {
            Ok(frame) => self.show(ctx, &frame, index, now),
            Err(err) => {
                self.last_error = Some(err);
                self.last_update = now;
            }
        }
    }

    /// Puts a frame on screen. The texture is reused: at 25 fps a new one per frame would
    /// throw a quarter of a megabyte of GPU memory after every picture.
    fn show(&mut self, ctx: &egui::Context, frame: &crate::audio::video_engine::Frame, index: u64, now: f64) {
        let image = egui::ColorImage::from_rgb(
            [frame.width as usize, frame.height as usize],
            &frame.rgb,
        );
        match &mut self.texture {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                self.texture =
                    Some(ctx.load_texture("sonix_video", image, egui::TextureOptions::LINEAR))
            }
        }
        self.shown_frame = Some(index);
        self.last_update = now;
        self.last_error = None;
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

        state.refresh(ui.ctx(), song_time, is_playing, ui.input(|i| i.time));

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
