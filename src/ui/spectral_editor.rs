//! **Spektral målning i gränssnittet.** Spektrogrammet ritas som en textur, penseln skriver
//! i masken och `Apply` gör masken till ljud igen. Ingen ny DSP här — den bor i
//! `audio::spectral_edit` och är prövad där.

use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Sense, TextureHandle, TextureOptions, Vec2};

use crate::audio::spectral_edit::{Mask, Spectrogram};

pub struct SpectralEditor {
    pub track: usize,
    pub track_name: String,
    pub spec: Spectrogram,
    pub mask: Mask,
    base: Option<TextureHandle>,
    overlay: Option<TextureHandle>,
    /// Målningen som RGBA ovanpå spektrogrammet, i vyns upplösning.
    overlay_pixels: Vec<u8>,
    overlay_dirty: bool,
    pub brush: usize,
    pub gain_db: f32,
}

pub enum SpectralAction {
    None,
    Apply,
    Close,
}

impl SpectralEditor {
    pub fn new(track: usize, track_name: String, left: &[f32], right: &[f32], sample_rate: f32) -> Self {
        let spec = Spectrogram::analyze(left, right, sample_rate);
        let mask = Mask::new(spec.frames, spec.bins);
        let (w, h) = spec.view_size();
        Self {
            track,
            track_name,
            spec,
            mask,
            base: None,
            overlay: None,
            overlay_pixels: vec![0; w * h * 4],
            overlay_dirty: true,
            brush: 3,
            gain_db: -60.0,
        }
    }

    /// Målar i masken från en punkt i vyn. `vx`/`vy` är rutor i vyn, `vy = 0` är högst upp.
    pub fn paint_at_view(&mut self, vx: isize, vy: isize) {
        let (w, h) = self.spec.view_size();
        let r = self.brush as isize;
        let x0 = (vx - r).max(0) as usize;
        let x1 = ((vx + r + 1).max(1) as usize).min(w);
        let y0 = (vy - r).max(0) as usize;
        let y1 = ((vy + r + 1).max(1) as usize).min(h);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        // Vyns rader är vända: rad 0 är högsta frekvensen.
        let row0 = h - y1;
        let row1 = h - y0;
        let frame_lo = x0 * self.spec.frames / w;
        let frame_hi = (x1 * self.spec.frames).div_ceil(w);
        let bin_lo = row0 * self.spec.bins / h;
        let bin_hi = (row1 * self.spec.bins).div_ceil(h);
        self.mask.paint(frame_lo, frame_hi, bin_lo, bin_hi, self.gain_db);

        let alpha = 150u8;
        for y in y0..y1 {
            for x in x0..x1 {
                let o = (y * w + x) * 4;
                self.overlay_pixels[o] = 0;
                self.overlay_pixels[o + 1] = 0;
                self.overlay_pixels[o + 2] = 0;
                self.overlay_pixels[o + 3] = alpha;
            }
        }
        self.overlay_dirty = true;
    }

    pub fn clear_paint(&mut self) {
        self.mask = Mask::new(self.spec.frames, self.spec.bins);
        self.overlay_pixels.iter_mut().for_each(|v| *v = 0);
        self.overlay_dirty = true;
    }

    fn textures(&mut self, ctx: &egui::Context) {
        let (w, h) = self.spec.view_size();
        if self.base.is_none() {
            let (bw, bh, rgb) = self.spec.view_rgb();
            let pixels = rgb
                .chunks_exact(3)
                .map(|c| Color32::from_rgb(c[0], c[1], c[2]))
                .collect();
            let image = ColorImage { size: [bw, bh], pixels };
            self.base = Some(ctx.load_texture("spectrogram", image, TextureOptions::NEAREST));
        }
        if self.overlay.is_none() || self.overlay_dirty {
            let pixels = self
                .overlay_pixels
                .chunks_exact(4)
                .map(|c| Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3]))
                .collect();
            let image = ColorImage { size: [w, h], pixels };
            match &mut self.overlay {
                Some(handle) => handle.set(image, TextureOptions::NEAREST),
                None => self.overlay = Some(ctx.load_texture("spectral_paint", image, TextureOptions::NEAREST)),
            }
            self.overlay_dirty = false;
        }
    }
}

pub fn render_spectral_editor(ctx: &egui::Context, editor: &mut SpectralEditor) -> SpectralAction {
    editor.textures(ctx);
    let mut open = true;
    let mut action = SpectralAction::None;
    let palette = crate::ui::theme::palette();

    egui::Window::new(format!("🎨 Spektral målning – {}", editor.track_name))
        .id(egui::Id::new("spectral_editor"))
        .open(&mut open)
        .default_size(Vec2::new(960.0, 600.0))
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Måla på spektrogrammet: det målade området dämpas när du trycker Verkställ.")
                        .size(crate::ui::theme::Theme::FONT_S)
                        .color(palette.text_muted()),
                );
            });
            ui.horizontal(|ui| {
                ui.add(egui::Slider::new(&mut editor.brush, 1..=12).text("Pensel"));
                ui.add(egui::Slider::new(&mut editor.gain_db, -60.0..=0.0).text("Dämpning dB"));
                ui.label(format!("{} rutor målade", editor.mask.painted_cells()));
            });

            let (w, h) = editor.spec.view_size();
            let aspect = h as f32 / w.max(1) as f32;
            let width = ui.available_width().max(200.0);
            let height = (width * aspect).clamp(120.0, ui.available_height() - 48.0);
            let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 0.0, palette.bg());
            if let Some(base) = &editor.base {
                painter.image(base.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }
            if let Some(overlay) = &editor.overlay {
                painter.image(overlay.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
            }

            if (response.dragged() || response.clicked())
                && let Some(pos) = response.interact_pointer_pos()
            {
                let fx = ((pos.x - rect.left()) / rect.width()).clamp(0.0, 0.999);
                let fy = ((pos.y - rect.top()) / rect.height()).clamp(0.0, 0.999);
                editor.paint_at_view((fx * w as f32) as isize, (fy * h as f32) as isize);
            }

            ui.horizontal(|ui| {
                if ui.button("✅ Verkställ").clicked() {
                    action = SpectralAction::Apply;
                }
                if ui.button("🧽 Rensa målning").clicked() {
                    editor.clear_paint();
                }
                ui.label(
                    egui::RichText::new(format!(
                        "{:.1} s · {} bildrutor · {} bin",
                        editor.spec.seconds, editor.spec.frames, editor.spec.bins
                    ))
                    .size(crate::ui::theme::Theme::FONT_XS)
                    .color(palette.text_muted()),
                );
            });
        });

    if !open || matches!(action, SpectralAction::Apply) {
        return match action {
            SpectralAction::Apply => SpectralAction::Apply,
            _ => SpectralAction::Close,
        };
    }
    action
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::spectral_edit::BINS;

    fn editor() -> SpectralEditor {
        let sr = 48_000.0;
        let n = 24_000;
        let l: Vec<f32> = (0..n).map(|i| 0.3 * (i as f32 * 0.05).sin()).collect();
        SpectralEditor::new(0, "test".to_string(), &l, &l, sr)
    }

    /// Penseln i vyn måste träffa rätt rutor i masken: överst i bild = högsta frekvensen.
    #[test]
    fn painting_the_top_row_attenuates_the_highest_bins() {
        let mut ed = editor();
        ed.brush = 0;
        ed.gain_db = -60.0;
        ed.paint_at_view(0, 0);
        let bins = ed.spec.bins;
        assert_eq!(bins, BINS);
        assert!(ed.mask.gains[bins - 1] < 0.01, "högsta binet målades inte");
        assert_eq!(ed.mask.gains[0], 1.0, "lägsta binet ska vara orört");
        // En ruta i vyn täcker flera bin när analysen är finare än bilden — men bara
        // bildrutorna under den första vynsrutan.
        assert!(ed.mask.painted_cells() >= 1);
        let bins = ed.spec.bins;
        for (i, g) in ed.mask.gains.iter().enumerate() {
            if *g < 0.999 {
                assert_eq!(i / bins, 0, "målat utanför första bildrutan");
            }
        }
    }

    /// Målning i nedre kanten träffar basen, och rensa tar bort allt igen.
    #[test]
    fn the_bottom_row_is_the_bass_and_clear_removes_the_paint() {
        let mut ed = editor();
        ed.brush = 0;
        let (_, h) = ed.spec.view_size();
        ed.paint_at_view(0, (h - 1) as isize);
        assert!(ed.mask.gains[0] < 0.01, "lägsta binet målades inte");
        assert!(ed.mask.painted_cells() > 0);
        ed.clear_paint();
        assert_eq!(ed.mask.painted_cells(), 0);
        assert!(ed.overlay_pixels.iter().all(|v| *v == 0));
    }

    /// Fönstret ritas utan backend: egui-kontexten kör en bildruta och ska lämna former.
    /// Det fångar panik i layouten och texturerna — inte bara i matematiken.
    #[test]
    fn the_window_renders_in_a_headless_frame() {
        let mut ed = editor();
        let ctx = egui::Context::default();
        let mut raw = egui::RawInput::default();
        raw.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)));
        let output = ctx.run(raw, |ctx| {
            let _ = render_spectral_editor(ctx, &mut ed);
        });
        assert!(!output.shapes.is_empty(), "ingen form ritades");
        assert!(ed.base.is_some() && ed.overlay.is_some(), "texturerna skapades inte");
    }

    /// En pensel nära kanten får inte gå utanför rutnätet.
    #[test]
    fn the_brush_clamps_at_the_edges() {
        let mut ed = editor();
        ed.brush = 12;
        ed.paint_at_view(-50, -50);
        ed.paint_at_view(100_000, 100_000);
        assert!(ed.mask.gains.iter().all(|g| *g > 0.0));
        assert!(ed.mask.painted_cells() > 0);
    }
}
