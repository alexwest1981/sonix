//! **Edison** — klippets ljudeditor: välj ett urval i vågformen och redigera en kopia.
//!
//! Originalet rörs aldrig: redigeringen sker i minnet och resultatet blir ett **nytt
//! spår** när man trycker *Skapa spår*. Det gör ett misstag billigt — det finns ingen väg
//! från den här rutan som skriver över källfilen.

use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};

use crate::audio::edison as ops;
use crate::ui::theme::{palette, Theme};

/// Antal punkter i vågformsförhandsvisningen. Fler än så syns inte på en skärm och
/// kostar bara tid att räkna om efter varje redigering.
const PEAK_POINTS: usize = 1200;

#[derive(Clone, Copy, PartialEq)]
pub enum EdisonAction {
    None,
    Close,
    Create,
}

pub struct EdisonState {
    pub title: String,
    pub sample_rate: u32,
    /// Var klippet låg på tidslinjen, i takter — det nya spåret hamnar på samma plats.
    pub start_bar: f32,
    pub original: (Vec<f32>, Vec<f32>),
    pub work: (Vec<f32>, Vec<f32>),
    /// Urval i sampel, `[start, slut)`. `None` = hela klippet.
    pub selection: Option<(usize, usize)>,
    /// Vågformens topp per punkt, räknad ur `work`. Byggs om efter varje redigering —
    /// att räkna den varje bildruta hade kostat en genomsökning av hela klippet per frame.
    peaks: Vec<f32>,
    drag_anchor: Option<usize>,
    pub message: Option<String>,
    /// Spektrum över urvalet. `spectrum_key` är (start, slut, längd) det räknades för, så
    /// bilden inte räknas om varje ruta men ändå följer varje redigering.
    pub show_spectrum: bool,
    spectrum: Vec<f32>,
    spectrum_key: (usize, usize, usize),
}

impl EdisonState {
    pub fn new(title: String, left: Vec<f32>, right: Vec<f32>, sample_rate: u32, start_bar: f32) -> Self {
        let mut state = Self {
            title,
            sample_rate: sample_rate.max(1),
            start_bar,
            original: (left, right),
            work: (Vec::new(), Vec::new()),
            selection: None,
            peaks: Vec::new(),
            drag_anchor: None,
            message: None,
            show_spectrum: false,
            spectrum: Vec::new(),
            spectrum_key: (0, 0, 0),
        };
        state.reset();
        state
    }

    fn len(&self) -> usize {
        self.work.0.len().min(self.work.1.len())
    }

    pub fn duration_secs(&self) -> f32 {
        self.len() as f32 / self.sample_rate as f32
    }

    fn range(&self) -> (usize, usize) {
        self.selection.unwrap_or((0, self.len()))
    }

    pub fn reset(&mut self) {
        self.work = (self.original.0.clone(), self.original.1.clone());
        self.selection = None;
        self.rebuild_peaks();
        self.message = None;
    }

    fn rebuild_peaks(&mut self) {
        let len = self.len();
        self.peaks.clear();
        if len == 0 {
            return;
        }
        let bucket = (len as f32 / PEAK_POINTS as f32).max(1.0) as usize;
        for start in (0..len).step_by(bucket) {
            let end = (start + bucket).min(len);
            let mut top = 0.0_f32;
            for i in start..end {
                top = top.max(self.work.0[i].abs()).max(self.work.1[i].abs());
            }
            self.peaks.push(top);
        }
    }

    fn apply(&mut self, op: Op) {
        let (a, b) = self.range();
        let (l, r) = (&mut self.work.0, &mut self.work.1);
        match op {
            Op::Reverse => ops::reverse(l, r, a, b),
            Op::Normalize => ops::normalize(l, r, a, b, 0.891), // −1 dBFS
            Op::FadeIn => ops::fade(l, r, a, b, true),
            Op::FadeOut => ops::fade(l, r, a, b, false),
            Op::Silence => ops::silence(l, r, a, b),
            Op::Cut => {
                let (nl, nr) = ops::cut(l, r, a, b);
                self.work = (nl, nr);
                self.selection = None;
            }
            Op::Trim => {
                let (nl, nr) = ops::trim(l, r, a, b);
                self.work = (nl, nr);
                self.selection = None;
            }
        }
        self.rebuild_peaks();
        self.message = None;
    }
}

#[derive(Clone, Copy)]
enum Op {
    Reverse,
    Normalize,
    FadeIn,
    FadeOut,
    Silence,
    Cut,
    Trim,
}

pub fn render_edison_modal(ctx: &egui::Context, st: &mut EdisonState) -> EdisonAction {
    let mut action = EdisonAction::None;
    let mut pending: Option<Op> = None;
    let mut reset = false;

    egui::Window::new(crate::i18n::t("✂ EDISON — klippets ljud"))
        .collapsible(false)
        .resizable(false)
        .default_width(760.0)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "{} · {:.2} s · {} Hz",
                    st.title,
                    st.duration_secs(),
                    st.sample_rate
                ))
                .size(Theme::FONT_S)
                .color(palette().text_muted()),
            );
            ui.add_space(4.0);
            draw_waveform(ui, st);
            ui.add_space(4.0);

            let (a, b) = st.range();
            ui.label(
                egui::RichText::new(crate::tstatus!(
                    "Urval: {:.2}–{:.2} s ({} sampel)",
                    a as f32 / st.sample_rate as f32,
                    b as f32 / st.sample_rate as f32,
                    b.saturating_sub(a)
                ))
                .size(Theme::FONT_S),
            );
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                if ui.button(crate::i18n::t("🔄 Vänd")).clicked() {
                    pending = Some(Op::Reverse);
                }
                if ui.button(crate::i18n::t("📈 Normalisera (−1 dB)")).clicked() {
                    pending = Some(Op::Normalize);
                }
                if ui.button(crate::i18n::t("🌅 Tona in")).clicked() {
                    pending = Some(Op::FadeIn);
                }
                if ui.button(crate::i18n::t("🌇 Tona ut")).clicked() {
                    pending = Some(Op::FadeOut);
                }
                if ui.button(crate::i18n::t("🔇 Tysta")).clicked() {
                    pending = Some(Op::Silence);
                }
                if ui.button(crate::i18n::t("✂ Ta bort urvalet")).clicked() {
                    pending = Some(Op::Cut);
                }
                if ui.button(crate::i18n::t("⧉ Behåll bara urvalet")).clicked() {
                    pending = Some(Op::Trim);
                }
                if ui.button(crate::i18n::t("↺ Återställ")).clicked() {
                    reset = true;
                }
            });

            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui
                    .selectable_label(st.show_spectrum, crate::i18n::t("📊 Spektrum över urvalet"))
                    .on_hover_text(crate::i18n::t(
                        "Visar vilka frekvenser urvalet innehåller, 40 Hz–16 kHz på en logaritmisk axel. Räknas om när urvalet eller ljudet ändras.",
                    ))
                    .clicked()
                {
                    st.show_spectrum = !st.show_spectrum;
                }
            });
            if st.show_spectrum {
                draw_spectrum(ui, st);
            }

            if let Some(msg) = &st.message {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(msg).size(Theme::FONT_S).color(palette().red()));
            }

            ui.add_space(8.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .button(egui::RichText::new(crate::i18n::t("➕ Skapa spår av redigeringen")).strong())
                    .on_hover_text(crate::i18n::t(
                        "Skriver ett nytt wav i projektets mapp och lägger det som ett nytt spår. Källklippet och dess fil rörs inte.",
                    ))
                    .clicked()
                {
                    action = EdisonAction::Create;
                }
                if ui.button(crate::i18n::t("Stäng")).clicked() {
                    action = EdisonAction::Close;
                }
            });
        });

    if let Some(op) = pending {
        st.apply(op);
    }
    if reset {
        st.reset();
    }
    action
}


fn draw_spectrum(ui: &mut Ui, st: &mut EdisonState) {
    let (a, b) = st.selection.unwrap_or((0, st.len()));
    let key = (a, b, st.work.0.len());
    if st.spectrum_key != key {
        if b > a && st.work.1.len() >= b {
            let mono: Vec<f32> = (a..b).map(|i| 0.5 * (st.work.0[i] + st.work.1[i])).collect();
            st.spectrum = crate::audio::edison::spectrum_db(&mono, st.sample_rate as f32, 48);
        } else {
            st.spectrum.clear();
        }
        st.spectrum_key = key;
    }

    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 96.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, Rounding::same(4.0), palette().bg_deep());
    painter.rect_stroke(rect, Rounding::same(4.0), Stroke::new(1.0_f32, palette().border()));
    if st.spectrum.is_empty() {
        return;
    }
    let n = st.spectrum.len() as f32;
    let bw = rect.width() / n;
    for (i, level) in st.spectrum.iter().enumerate() {
        let h = (level * (rect.height() - 4.0)).max(0.5);
        let x = rect.min.x + bw * i as f32;
        let color = if *level > 0.75 {
            palette().orange()
        } else {
            palette().cyan()
        };
        painter.rect_filled(
            Rect::from_min_max(
                Pos2::new(x + 1.0, rect.max.y - h),
                Pos2::new(x + bw - 1.0, rect.max.y - 1.0),
            ),
            Rounding::ZERO,
            color,
        );
    }
}

fn draw_waveform(ui: &mut Ui, st: &mut EdisonState) {
    let width = ui.available_width().max(320.0);
    let height = 170.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(6.0), palette().bg_deep());
    painter.rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0_f32, palette().border()));

    let mid = rect.center().y;
    let half = rect.height() * 0.46;
    painter.line_segment(
        [Pos2::new(rect.min.x, mid), Pos2::new(rect.max.x, mid)],
        Stroke::new(1.0_f32, palette().border_soft()),
    );

    // Urvalet bakom vågformen, så markeringen syns även där ljudet är tyst.
    if let Some((a, b)) = st.selection {
        let len = st.len().max(1) as f32;
        let x0 = rect.min.x + rect.width() * (a as f32 / len);
        let x1 = rect.min.x + rect.width() * (b as f32 / len);
        painter.rect_filled(
            Rect::from_min_max(Pos2::new(x0, rect.min.y), Pos2::new(x1, rect.max.y)),
            Rounding::ZERO,
            Color32::from_rgba_unmultiplied(255, 165, 0, 36),
        );
    }

    let n = st.peaks.len().max(1) as f32;
    for (i, peak) in st.peaks.iter().enumerate() {
        let x = rect.min.x + rect.width() * (i as f32 + 0.5) / n;
        let h = (peak.clamp(0.0, 1.0) * half).max(0.5);
        painter.line_segment(
            [Pos2::new(x, mid - h), Pos2::new(x, mid + h)],
            Stroke::new(1.0_f32, palette().track_blue()),
        );
    }

    // Dra i vågformen för att välja. Ett klick utan dragning väljer allt igen.
    if response.drag_started()
        && let Some(pos) = response.interact_pointer_pos()
    {
        st.drag_anchor = Some(sample_at(rect, st.len(), pos.x));
    }
    if response.dragged()
        && let (Some(anchor), Some(pos)) = (st.drag_anchor, response.interact_pointer_pos())
    {
        let here = sample_at(rect, st.len(), pos.x);
        st.selection = Some(ops::selection(st.len(), anchor, here));
    }
    if response.drag_stopped() {
        st.drag_anchor = None;
    }
}

fn sample_at(rect: Rect, len: usize, x: f32) -> usize {
    if len == 0 {
        return 0;
    }
    let frac = ((x - rect.min.x) / rect.width().max(1.0)).clamp(0.0, 1.0);
    ((frac * len as f32) as usize).min(len)
}
