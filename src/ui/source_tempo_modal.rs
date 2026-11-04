//! **Källans tempo** — talet som avgör hur mycket klippets ljud sträcks (Fas 8.10).
//!
//! Rutan finns därför att tempomätningen kan hamna en oktav fel: ett komp med bakslag har
//! starkast periodicitet på *halva* takten, och då mäts 134,8 som 67,4. Utan ett ställe att
//! skriva rätt tal gick ett sådant klipp inte att räta upp — det blev fel tempo oavsett vad
//! man gjorde (Alex 2026-11-04). ÷2 och ×2 är därför de två viktigaste knapparna här.

use eframe::egui;

use crate::ui::theme::{Theme, palette};

pub struct SourceTempoState {
    pub track: usize,
    pub region: usize,
    pub name: String,
    /// Talet klippet bär nu (0 = okänt, då sträcks inget).
    pub old: f32,
    /// Talet som rutan redigerar.
    pub value: f32,
    pub project_bpm: f32,
    /// Hur många klipp i projektet som bär samma källtempo — stämmorna är den vanliga gruppen.
    pub shares: usize,
    pub apply_all: bool,
}

pub enum SourceTempoAction {
    None,
    Close,
    Apply,
    Measure,
}

/// Faktorn klippet spelas med vid ett givet källtempo — samma regel som `stretch_ratio_for`,
/// men utan golv och tak: rutan ska visa vad som *händer*, inte vad som är tillåtet.
fn ratio(source_bpm: f32, project_bpm: f32) -> f32 {
    if source_bpm <= 0.0 || project_bpm <= 0.0 {
        1.0
    } else {
        project_bpm / source_bpm
    }
}

pub fn render_source_tempo_modal(
    ctx: &egui::Context,
    st: &mut SourceTempoState,
) -> SourceTempoAction {
    let mut action = SourceTempoAction::None;
    egui::Window::new(crate::i18n::t("🎯 Källans tempo"))
        .collapsible(false)
        .resizable(false)
        .default_width(440.0)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(&st.name)
                    .size(Theme::FONT_S)
                    .color(palette().text_muted()),
            );
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut st.value)
                        .range(20.0..=300.0)
                        .speed(0.1)
                        .fixed_decimals(2)
                        .suffix(" BPM"),
                );
                if ui.button("÷2").clicked() {
                    st.value = (st.value / 2.0).max(20.0);
                }
                if ui.button("×2").clicked() {
                    st.value = (st.value * 2.0).min(300.0);
                }
                if ui.button(crate::i18n::t("🎵 Mät tempot i klippet")).clicked() {
                    action = SourceTempoAction::Measure;
                }
            });

            ui.add_space(2.0);
            let r = ratio(st.value, st.project_bpm);
            // **Ett tal som inte sträcker något ska synas som ett tal som inte sträcker
            // något.** Okänt tempo (0) är det vanligaste skälet till att ett klipp står
            // still när projektets tempo ändras.
            let (what, colour) = if st.value <= 0.0 {
                (
                    crate::i18n::t("källtempo okänt — inget sträcks").to_string(),
                    palette().orange(),
                )
            } else if (r - 1.0).abs() > 0.02 {
                (
                    crate::tstatus!("projekt {:.1} BPM → spelas {:.2}×", st.project_bpm, r),
                    palette().cyan(),
                )
            } else {
                (
                    crate::tstatus!("projekt {:.1} BPM → spelas som det är", st.project_bpm),
                    palette().text_muted(),
                )
            };
            ui.label(egui::RichText::new(what).size(Theme::FONT_S).color(colour));

            if st.shares > 1 {
                ui.add_space(4.0);
                ui.checkbox(
                    &mut st.apply_all,
                    crate::tstatus!("Gäller alla {} klipp med samma källtempo", st.shares),
                );
            }

            ui.add_space(4.0);
            if ui
                .button(crate::i18n::t("Använd projektets tempo"))
                .clicked()
            {
                st.value = st.project_bpm;
            }

            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.button(crate::i18n::t("Använd")).clicked() {
                    action = SourceTempoAction::Apply;
                }
                if ui.button(crate::i18n::t("Stäng")).clicked() {
                    action = SourceTempoAction::Close;
                }
            });
        });
    action
}
