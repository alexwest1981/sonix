//! **Newtime** — klippets tidseditor: flytta var i tiden ljudet landar, med tonhöjden kvar.
//!
//! Vågformen ritas **som den kommer att låta**: varje segment mellan två markörer ritas
//! utsträckt eller ihopdraget till sin nya längd. Ett klick skapar en markör, ett drag
//! flyttar den, och högerkanten är filens nya slut. Renderingen sker på en kopia och blir
//! ett nytt spår — källklippet rörs aldrig.

use eframe::egui::{self, Color32, Pos2, Rounding, Sense, Stroke, Ui, Vec2};

use crate::audio::newtime::{Warp, WarpMarker};
use crate::ui::theme::{palette, Theme};

/// Vågformsförhandsvisningen: en topp per så många sampel. 512 räcker för att en pixel
/// ska få en ärlig höjd utan att hela klippet måste genomsökas varje bildruta.
const PREVIEW_BUCKET: usize = 512;

#[derive(Clone, Copy, PartialEq)]
pub enum NewtimeAction {
    None,
    Close,
    Create,
}

#[derive(Clone, Copy, PartialEq)]
enum Drag {
    Marker(usize),
    End,
}

pub struct NewtimeState {
    pub title: String,
    pub sample_rate: u32,
    pub start_bar: f32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub warp: Warp,
    preview: Vec<f32>,
    drag: Option<Drag>,
    pub selected: Option<usize>,
    pub message: Option<String>,
}

impl NewtimeState {
    pub fn new(title: String, left: Vec<f32>, right: Vec<f32>, sample_rate: u32, start_bar: f32) -> Self {
        let sr = sample_rate.max(1);
        let src_secs = left.len().min(right.len()) as f32 / sr as f32;
        let mut preview = Vec::with_capacity(left.len() / PREVIEW_BUCKET + 1);
        for chunk in 0..left.len().div_ceil(PREVIEW_BUCKET) {
            let a = chunk * PREVIEW_BUCKET;
            let b = (a + PREVIEW_BUCKET).min(left.len());
            let mut top = 0.0_f32;
            for i in a..b {
                top = top.max(left[i].abs()).max(right[i].abs());
            }
            preview.push(top);
        }
        Self {
            title,
            sample_rate: sr,
            start_bar,
            left,
            right,
            warp: Warp::new(src_secs),
            preview,
            drag: None,
            selected: None,
            message: None,
        }
    }

    pub fn source_secs(&self) -> f32 {
        self.warp.source_sec
    }

    pub fn out_secs(&self) -> f32 {
        self.warp.out_len()
    }

    /// Utgångens längd i förhållande till källan (1,0 = oförändrad).
    pub fn ratio(&self) -> f32 {
        if self.source_secs() > 0.0 { self.out_secs() / self.source_secs() } else { 1.0 }
    }

    pub fn render(&self) -> (Vec<f32>, Vec<f32>) {
        crate::audio::newtime::render(&self.left, &self.right, self.sample_rate as f32, &self.warp)
    }

    fn add_marker(&mut self, out_sec: f32) {
        let source = self.warp.source_at_out(out_sec);
        let marker = WarpMarker { source_sec: source, out_sec };
        let idx = self
            .warp
            .markers
            .iter()
            .position(|m| m.source_sec > source)
            .unwrap_or(self.warp.markers.len());
        self.warp.markers.insert(idx, marker);
        self.selected = Some(idx);
        self.message = None;
    }

    fn remove_selected(&mut self) {
        if let Some(i) = self.selected
            && i < self.warp.markers.len()
        {
            self.warp.markers.remove(i);
            self.selected = None;
        }
    }
}

pub fn render_newtime_modal(ctx: &egui::Context, st: &mut NewtimeState) -> NewtimeAction {
    let mut action = NewtimeAction::None;
    let mut remove = false;
    let mut reset = false;
    let mut drag_to: Option<f32> = None;

    egui::Window::new(crate::i18n::t("⏱ NEWTIME — klippets tid"))
        .collapsible(false)
        .resizable(false)
        .default_width(760.0)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(crate::tstatus!(
                    "{} · {:.2} s → {:.2} s ({:.2}×) · {} markörer",
                    st.title,
                    st.source_secs(),
                    st.out_secs(),
                    st.ratio(),
                    st.warp.markers.len()
                ))
                .size(Theme::FONT_S)
                .color(palette().text_muted()),
            );
            ui.add_space(4.0);
            draw_timeline(ui, st, &mut drag_to);
            ui.add_space(4.0);

            ui.label(
                egui::RichText::new(crate::i18n::t("Klicka i vågformen för att lägga en markör, dra den för att flytta ljudet, dra högerkanten för att ändra slutet"))
                    .size(Theme::FONT_S)
                    .color(palette().text_muted()),
            );
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                if ui.button(crate::i18n::t("🗑 Ta bort markör")).clicked() {
                    remove = true;
                }
                if ui.button(crate::i18n::t("↺ Nollställ tid")).clicked() {
                    reset = true;
                }
            });

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
                        "Sträcker om ljudet med bevarad tonhöjd och lägger det som ett nytt spår. Källklippet och dess fil rörs inte.",
                    ))
                    .clicked()
                {
                    action = NewtimeAction::Create;
                }
                if ui.button(crate::i18n::t("Stäng")).clicked() {
                    action = NewtimeAction::Close;
                }
            });
        });

    if let Some(t) = drag_to {
        st.apply_drag(t);
    }
    st.drag = None;
    if remove {
        st.remove_selected();
    }
    if reset {
        st.warp = Warp::new(st.source_secs());
        st.selected = None;
        st.message = None;
    }
    action
}

impl NewtimeState {
    /// Klämmer den dragna markören mellan sina grannar. Tiden får aldrig vända.
    fn apply_drag(&mut self, out_sec: f32) {
        match self.drag {
            Some(Drag::End) => {
                let last = self
                    .warp
                    .markers
                    .iter()
                    .map(|m| m.out_sec)
                    .fold(0.0_f32, f32::max);
                self.warp.end_out_sec = out_sec.max(last + 0.05).max(0.05);
            }
            Some(Drag::Marker(i)) if i < self.warp.markers.len() => {
                let prev = if i == 0 { 0.0 } else { self.warp.markers[i - 1].out_sec };
                let next = self
                    .warp
                    .markers
                    .get(i + 1)
                    .map(|m| m.out_sec)
                    .unwrap_or(self.out_secs());
                self.warp.markers[i].out_sec = out_sec.clamp(prev + 0.01, next - 0.01);
            }
            _ => {}
        }
    }
}

fn draw_timeline(ui: &mut Ui, st: &mut NewtimeState, drag_to: &mut Option<f32>) {
    let width = ui.available_width().max(320.0);
    let height = 190.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click_and_drag());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(6.0), palette().bg_deep());
    painter.rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0_f32, palette().border()));

    let total_out = st.out_secs().max(0.001);
    let sr = st.sample_rate as f32;
    let mid = rect.center().y;
    let half = rect.height() * 0.44;

    // Vågformen ritas i **utticen**: en pixel motsvarar en tid som pekar tillbaka på en
    // källsekund genom segmentets egen linjära karta.
    for px in 0..rect.width().max(1.0) as usize {
        let out_t = (px as f32 / rect.width()) * total_out;
        let src_t = st.warp.source_at_out(out_t).clamp(0.0, st.source_secs());
        let bucket = ((src_t * sr) as usize) / PREVIEW_BUCKET;
        let peak = st.preview.get(bucket).copied().unwrap_or(0.0).clamp(0.0, 1.0);
        let h = (peak * half).max(0.5);
        let x = rect.min.x + px as f32;
        painter.line_segment(
            [Pos2::new(x, mid - h), Pos2::new(x, mid + h)],
            Stroke::new(1.0_f32, palette().track_blue()),
        );
    }

    let x_of = |t: f32| rect.min.x + rect.width() * (t / total_out).clamp(0.0, 1.0);

    // Markörerna: tunna linjer med ett grepp om toppen.
    for (i, m) in st.warp.markers.iter().enumerate() {
        let x = x_of(m.out_sec);
        let selected = st.selected == Some(i);
        let color = if selected { Color32::WHITE } else { palette().orange() };
        painter.line_segment(
            [Pos2::new(x, rect.min.y + 4.0), Pos2::new(x, rect.max.y - 4.0)],
            Stroke::new(if selected { 2.0 } else { 1.0 } as f32, color),
        );
        painter.circle_filled(Pos2::new(x, rect.min.y + 6.0), 3.5, color);
    }
    // Slutet: dra för att ändra filens längd.
    let end_x = x_of(total_out);
    painter.line_segment(
        [Pos2::new(end_x, rect.min.y), Pos2::new(end_x, rect.max.y)],
        Stroke::new(2.0_f32, palette().text_muted()),
    );

    if response.drag_started()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let mut best: Option<(Drag, f32)> = None;
        for (i, m) in st.warp.markers.iter().enumerate() {
            let d = (x_of(m.out_sec) - pos.x).abs();
            if d < 8.0 {
                best = Some((Drag::Marker(i), d));
            }
        }
        if (end_x - pos.x).abs() < 8.0 {
            best = Some((Drag::End, 0.0));
        }
        st.drag = best.map(|(d, _)| d);
    }
    if response.dragged()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0) * total_out;
        *drag_to = Some(t);
    }
    if response.drag_stopped() {
        st.drag = None;
    }

    // Ett klick (utan dragning) lägger en markör där man pekade.
    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        let mut near = false;
        for m in &st.warp.markers {
            if (x_of(m.out_sec) - pos.x).abs() < 8.0 {
                near = true;
            }
        }
        if !near {
            let t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0) * total_out;
            st.add_marker(t);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> NewtimeState {
        // Två sekunder tystnad räcker: testerna rör markörernas tid, inte ljudet.
        NewtimeState::new("test".to_string(), vec![0.1; 96_000], vec![0.1; 96_000], 48_000, 0.0)
    }

    #[test]
    fn a_click_adds_a_marker_on_the_audio_under_the_pointer() {
        let mut st = state();
        st.add_marker(0.5);
        assert_eq!(st.warp.markers.len(), 1);
        assert!((st.warp.markers[0].out_sec - 0.5).abs() < 1e-4);
        // Utan stretch ligger källan på samma klocka som uttiden.
        assert!((st.warp.markers[0].source_sec - 0.5).abs() < 1e-3);
    }

    #[test]
    fn dragging_a_marker_stops_at_its_neighbours() {
        let mut st = state();
        st.add_marker(0.5);
        st.add_marker(1.5);
        st.drag = Some(Drag::Marker(0));
        st.apply_drag(5.0); // långt förbi nästa markör
        assert!(st.warp.markers[0].out_sec < st.warp.markers[1].out_sec, "tiden får inte vända");
        st.apply_drag(-3.0);
        assert!(st.warp.markers[0].out_sec > 0.0, "markören får inte hamna före början");
    }

    #[test]
    fn dragging_the_end_sets_the_new_length() {
        let mut st = state();
        st.drag = Some(Drag::End);
        st.apply_drag(1.5);
        assert!((st.out_secs() - 1.5).abs() < 1e-4);
        assert!((st.ratio() - 0.75).abs() < 1e-3);
    }
}
