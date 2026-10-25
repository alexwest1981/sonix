//! **Newtone** — klippets tonhöjdseditor.
//!
//! Analysen och omskrivningen är `vocal_harmonizer`s befintliga blob-motor
//! (`analyze_blobs` + `apply_blob_corrections`): varje detekterad ton blir en stapel som
//! kan flyttas i halvtoner. Precis som Edison arbetar rutan på en **kopia** och lämnar
//! resultatet som ett nytt spår — källklippet rörs aldrig.

use eframe::egui::{self, Color32, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2};

use crate::audio::vocal_harmonizer::VocalHarmonizer;
use crate::ui::theme::{palette, Theme};

/// Upplösningen i analysen: en "step" är en bit av klippet, inte en sextondel. 160 steg
/// ger korta ord på en tvåsekundersfras utan att bli fler staplar än att de går att klicka.
const STEPS: usize = 160;

#[derive(Clone, Copy, PartialEq)]
pub enum NewtoneAction {
    None,
    Close,
    Create,
}

pub struct NewtoneState {
    pub title: String,
    pub sample_rate: u32,
    pub start_bar: f32,
    pub left: Vec<f32>,
    pub right: Vec<f32>,
    pub harmonizer: VocalHarmonizer,
    pub message: Option<String>,
}

impl NewtoneState {
    pub fn new(title: String, left: Vec<f32>, right: Vec<f32>, sample_rate: u32, start_bar: f32) -> Self {
        let sr = sample_rate.max(1);
        // Analysen görs på mitten: en sång ligger oftast i båda kanalerna, och då räcker
        // ett mono-spår för att hitta tonerna. Omskrivningen sker sedan per kanal.
        let mono: Vec<f32> = left
            .iter()
            .zip(right.iter())
            .map(|(a, b)| (a + b) * 0.5)
            .collect();
        // **Kromatiskt, inte skalbundet:** editorn ska visa tonen som den sjöngs. Att
        // snappa till en skala redan i analysen gör en C# i A-moll till C eller D beroende
        // på centen, och då flyttar ±1 halvton från fel ton.
        let mut harmonizer = VocalHarmonizer { target_scale: 0, ..Default::default() };
        harmonizer.analyze_take(&mono, sr as f32, STEPS);
        // Ett steg är ~12 ms: kortare än så är en övergång mellan två toner, inte en ton.
        harmonizer.blobs.retain(|b| b.length_steps >= 2);
        let empty = harmonizer.blobs.is_empty();
        Self {
            title,
            sample_rate: sr,
            start_bar,
            left,
            right,
            harmonizer,
            message: if empty {
                Some(crate::i18n::t("Inga toner hittades i klippet").to_string())
            } else {
                None
            },
        }
    }

    pub fn duration_secs(&self) -> f32 {
        self.left.len() as f32 / self.sample_rate as f32
    }

    /// Tonhöjdscenen: alla toner med sina förskjutningar applicerade, per kanal.
    pub fn render_corrected(&self) -> (Vec<f32>, Vec<f32>) {
        let sr = self.sample_rate as f32;
        (
            self.harmonizer.apply_blob_corrections(&self.left, sr),
            self.harmonizer.apply_blob_corrections(&self.right, sr),
        )
    }

    fn shift_selected(&mut self, semitones: f32) {
        let selected = self.harmonizer.selected_blob;
        match selected {
            Some(i) if i < self.harmonizer.blobs.len() => {
                let blob = &mut self.harmonizer.blobs[i];
                blob.pitch_offset = (blob.pitch_offset + semitones).clamp(-12.0, 12.0);
            }
            _ => {
                // Ingen vald ton = hela frasen, samma regel som i en markeringsbaserad editor.
                for blob in &mut self.harmonizer.blobs {
                    blob.pitch_offset = (blob.pitch_offset + semitones).clamp(-12.0, 12.0);
                }
            }
        }
    }

    fn reset_offsets(&mut self) {
        for blob in &mut self.harmonizer.blobs {
            blob.pitch_offset = 0.0;
        }
    }
}

pub fn render_newtone_modal(ctx: &egui::Context, st: &mut NewtoneState) -> NewtoneAction {
    let mut action = NewtoneAction::None;
    let mut shift = 0.0_f32;
    let mut reset = false;
    let mut select: Option<usize> = None;

    egui::Window::new(crate::i18n::t("🎼 NEWTONE — klippets toner"))
        .collapsible(false)
        .resizable(false)
        .default_width(760.0)
        .show(ctx, |ui| {
            let notes = st.harmonizer.blobs.len();
            ui.label(
                egui::RichText::new(crate::tstatus!(
                    "{} · {:.2} s · {} toner",
                    st.title,
                    st.duration_secs(),
                    notes
                ))
                .size(Theme::FONT_S)
                .color(palette().text_muted()),
            );
            ui.add_space(4.0);
            draw_pitch_grid(ui, st, &mut select);
            ui.add_space(4.0);

            if let Some(i) = st.harmonizer.selected_blob
                && let Some(blob) = st.harmonizer.blobs.get(i)
            {
                ui.label(
                    egui::RichText::new(crate::tstatus!(
                        "Vald ton: {} · {:+.1} halvton",
                        note_name(blob.midi_note as i32),
                        blob.pitch_offset
                    ))
                    .size(Theme::FONT_S),
                );
            } else {
                ui.label(
                    egui::RichText::new(crate::i18n::t("Klicka en ton för att välja den — annars gäller knapparna hela frasen"))
                        .size(Theme::FONT_S)
                        .color(palette().text_muted()),
                );
            }
            ui.add_space(4.0);

            ui.horizontal_wrapped(|ui| {
                if ui.button(crate::i18n::t("−1 halvton")).clicked() {
                    shift = -1.0;
                }
                if ui.button(crate::i18n::t("+1 halvton")).clicked() {
                    shift = 1.0;
                }
                if ui.button(crate::i18n::t("↺ Nollställ toner")).clicked() {
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
                        "Skriver ett nytt wav i projektets mapp och lägger det som ett nytt spår. Källklippet och dess fil rörs inte.",
                    ))
                    .clicked()
                {
                    action = NewtoneAction::Create;
                }
                if ui.button(crate::i18n::t("Stäng")).clicked() {
                    action = NewtoneAction::Close;
                }
            });
        });

    if let Some(i) = select {
        st.harmonizer.selected_blob = Some(i);
    }
    if shift != 0.0 {
        st.shift_selected(shift);
    }
    if reset {
        st.reset_offsets();
    }
    action
}

fn draw_pitch_grid(ui: &mut Ui, st: &mut NewtoneState, select: &mut Option<usize>) {
    let width = ui.available_width().max(320.0);
    let height = 220.0;
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
    let painter = ui.painter();
    painter.rect_filled(rect, Rounding::same(6.0), palette().bg_deep());
    painter.rect_stroke(rect, Rounding::same(6.0), Stroke::new(1.0_f32, palette().border()));

    let blobs = &st.harmonizer.blobs;
    if blobs.is_empty() {
        return;
    }
    let (mut lo, mut hi) = (127_i32, 0_i32);
    for b in blobs {
        lo = lo.min(b.midi_note as i32);
        hi = hi.max(b.midi_note as i32);
    }
    lo -= 2;
    hi += 2;
    let span = (hi - lo).max(1) as f32;
    let y_of = |midi: i32| rect.max.y - ((midi - lo) as f32 / span) * rect.height() * 0.92 - rect.height() * 0.04;

    // Oktavlinjer: C-tonerna får en ljusare linje och sitt namn, så höjden går att läsa.
    for midi in lo..=hi {
        if midi % 12 == 0 {
            let y = y_of(midi);
            painter.line_segment(
                [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                Stroke::new(1.0_f32, palette().border()),
            );
            painter.text(
                Pos2::new(rect.min.x + 4.0, y),
                egui::Align2::LEFT_CENTER,
                note_name(midi),
                egui::FontId::proportional(Theme::FONT_XS),
                palette().text_muted(),
            );
        }
    }

    let steps = st.harmonizer.blob_steps.max(1) as f32;
    for (i, blob) in blobs.iter().enumerate() {
        let x0 = rect.min.x + rect.width() * blob.start_step as f32 / steps;
        let x1 = rect.min.x + rect.width() * (blob.start_step + blob.length_steps) as f32 / steps;
        // Tonhöjdsförskjutningen syns direkt: stapeln ritas där tonen *kommer att* ligga.
        let y = y_of(blob.midi_note as i32 + blob.pitch_offset.round() as i32);
        let bar = Rect::from_min_max(
            Pos2::new(x0 + 1.0, y - 5.0),
            Pos2::new((x1 - 1.0).max(x0 + 3.0), y + 5.0),
        );
        let selected = st.harmonizer.selected_blob == Some(i);
        painter.rect_filled(bar, Rounding::same(2.0), blob.color);
        if selected {
            painter.rect_stroke(bar, Rounding::same(2.0), Stroke::new(1.5_f32, Color32::WHITE));
        }
    }

    if response.clicked()
        && let Some(pos) = response.interact_pointer_pos()
    {
        // Närmaste stapel inom 10 px vinner — en ton är lättare att träffa än en rad.
        let mut best: Option<(usize, f32)> = None;
        for (i, blob) in blobs.iter().enumerate() {
            let x0 = rect.min.x + rect.width() * blob.start_step as f32 / steps;
            let x1 = rect.min.x + rect.width() * (blob.start_step + blob.length_steps) as f32 / steps;
            let y = y_of(blob.midi_note as i32 + blob.pitch_offset.round() as i32);
            let dx = if pos.x < x0 { x0 - pos.x } else if pos.x > x1 { pos.x - x1 } else { 0.0 };
            let dist = dx.hypot(pos.y - y);
            if dist < 14.0 && best.map(|(_, d)| dist < d).unwrap_or(true) {
                best = Some((i, dist));
            }
        }
        *select = best.map(|(i, _)| i);
    }
}

/// `60 = C4`, samma namn som pianorullen använder.
fn note_name(midi: i32) -> String {
    const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = midi / 12 - 1;
    format!("{}{}", NAMES[(midi.rem_euclid(12)) as usize], octave)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> NewtoneState {
        let mut st = NewtoneState::new("test".to_string(), vec![0.0; 4_800], vec![0.0; 4_800], 48_000, 0.0);
        st.harmonizer.blobs = vec![
            crate::audio::vocal_harmonizer::PitchBlob {
                start_step: 0,
                length_steps: 10,
                midi_note: 60,
                pitch_offset: 0.0,
                color: Color32::WHITE,
            },
            crate::audio::vocal_harmonizer::PitchBlob {
                start_step: 10,
                length_steps: 10,
                midi_note: 62,
                pitch_offset: 0.0,
                color: Color32::WHITE,
            },
        ];
        st.harmonizer.selected_blob = None;
        st
    }

    #[test]
    fn shifting_without_a_selection_moves_the_whole_phrase() {
        let mut st = state();
        st.shift_selected(1.0);
        assert!(st.harmonizer.blobs.iter().all(|b| (b.pitch_offset - 1.0).abs() < 1e-6));
    }

    #[test]
    fn shifting_a_selected_note_leaves_the_others_alone() {
        let mut st = state();
        st.harmonizer.selected_blob = Some(1);
        st.shift_selected(-2.0);
        assert_eq!(st.harmonizer.blobs[0].pitch_offset, 0.0);
        assert!((st.harmonizer.blobs[1].pitch_offset + 2.0).abs() < 1e-6);
        st.reset_offsets();
        assert!(st.harmonizer.blobs.iter().all(|b| b.pitch_offset == 0.0));
    }

    #[test]
    fn note_names_match_the_piano_roll() {
        assert_eq!(note_name(60), "C4");
        assert_eq!(note_name(69), "A4");
        assert_eq!(note_name(61), "C#4");
    }

    /// **Rattens väg ända till ljudet:** +2 halvton på en 220 Hz-ton ska ge ~247 Hz i den
    /// renderade kopian — annars är `pitch_offset` bara ett tal i gränssnittet.
    #[test]
    fn shifting_a_note_moves_the_rendered_pitch() {
        let sr = 48_000_u32;
        let n = sr as usize;
        let pcm: Vec<f32> = (0..n)
            .map(|i| (std::f32::consts::TAU * 220.0 * i as f32 / sr as f32).sin() * 0.5)
            .collect();
        let mut st = NewtoneState::new("test".to_string(), pcm.clone(), pcm, sr, 0.0);
        assert!(!st.harmonizer.blobs.is_empty(), "analysen ska hitta 220 Hz");
        st.harmonizer.selected_blob = None;
        st.shift_selected(2.0);
        let (left, _) = st.render_corrected();
        let f = crate::audio::recorder::detect_pitch_hz(&left[10_000..14_096], sr as f32)
            .expect("tonhöjden ska gå att mäta");
        assert!((f - 246.94).abs() < 8.0, "220 Hz + 2 halvton ska bli ~247 Hz, fick {f:.1}");
    }

}
