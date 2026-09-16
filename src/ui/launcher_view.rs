//! **Rutnätet på skärmen — den mjukvarustyrda launch-ytan** (Sprint 1, punkt 5, steg 2).
//!
//! Alex: *"Jag har ingen launchpad, tänkte om vi kunde bygga en mjukvarustyrd?"* Här är ytan.
//! Reglerna — när en start får ske, vad en kö betyder, vad som händer med det som redan spelar —
//! bor i `audio::launcher`, och dit hör de. Den här filen **ritar och klickar** och gör ingenting
//! annat: den skriver tillstånd i modellen och berättar i statusraden vad som hände. Att bestämma
//! *när* något ska hända är inte ytans sak, och det är därför en framtida Launchpad kan använda
//! exakt samma modell utan att någonting här behöver ändras.
//!
//! Status: byggs (Sprint 1, punkt 5, steg 2) — rutnätet, kvantiseringsvalet och scenknapparna.
//! Kvar: tangentbordsstyrning (spela rutnätet med en hand) och MIDI in/ut (punkt 4).
//! Rör inte: klick skriver **bara** tillstånd i modellen. Ingen väg härifrån får sätta slingan
//! eller starta transporten direkt — då skulle takten bli beroende av vilken yta som klickade.

use eframe::egui;

use crate::audio::launcher::{LaunchQuantize, Launcher, SlotContent, SlotState};
use crate::ui::song_structure_modal::SongSectionItem;
use crate::ui::theme::Theme;

/// Kvantiseringens etikett. Presentation, därför här och inte i modellen.
pub fn quantize_label(q: LaunchQuantize) -> &'static str {
    match q {
        LaunchQuantize::Bar => "1 takt",
        LaunchQuantize::HalfBar => "1/2 takt",
        LaunchQuantize::Beat => "1 slag",
        LaunchQuantize::Immediate => "direkt",
    }
}

/// **Sektionerna blir slots.** Början räknas fram rad för rad — den gissas aldrig, och en
/// sektion utan längd får en takt (hellre en takt än noll).
pub fn slots_from_sections(sections: &[SongSectionItem]) -> Vec<SlotContent> {
    let mut start = 0usize;
    let mut out = Vec::with_capacity(sections.len());
    for section in sections {
        let length = section.length_bars.max(1);
        out.push(SlotContent::Section {
            name: section.name.clone(),
            start_bar: start,
            length_bars: length,
        });
        start += length;
    }
    out
}

/// Färgen en slot ritas i. Spelläge, kö och väntande stopp ska gå att se på håll.
fn slot_color(state: SlotState, queued_stop: bool) -> egui::Color32 {
    if queued_stop {
        return Theme::FL_ORANGE;
    }
    match state {
        SlotState::Playing => egui::Color32::from_rgb(46, 160, 67),
        SlotState::Queued => egui::Color32::from_rgb(196, 160, 40),
        SlotState::Stopped => egui::Color32::from_rgb(48, 54, 64),
        SlotState::Empty => egui::Color32::from_rgb(28, 30, 36),
    }
}

fn slot_label(state: SlotState, queued_stop: bool, name: Option<&str>) -> String {
    match (state, queued_stop) {
        (SlotState::Playing, true) => format!("⏹ {}", name.unwrap_or("")),
        (SlotState::Playing, false) => format!("▶ {}", name.unwrap_or("")),
        (SlotState::Queued, _) => format!("⏳ {}", name.unwrap_or("")),
        (SlotState::Stopped, _) => format!("○ {}", name.unwrap_or("")),
        (SlotState::Empty, _) => "—".to_string(),
    }
}

/// Ritar rutnätet. `open` styrs av menyn; `status_msg` är appens statusrad.
pub fn render_launcher_window(
    ctx: &egui::Context,
    open: &mut bool,
    launcher: &mut Launcher,
    quantize: &mut LaunchQuantize,
    transport_playing: bool,
    status_msg: &mut String,
) {
    if !*open {
        return;
    }

    egui::Window::new(crate::i18n::t("🎛 Scenrutnät (mjuk Launchpad)"))
        .open(open)
        .collapsible(false)
        .resizable(true)
        .default_width(420.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(crate::i18n::t("Kvantisering:"));
                for q in [
                    LaunchQuantize::Bar,
                    LaunchQuantize::HalfBar,
                    LaunchQuantize::Beat,
                    LaunchQuantize::Immediate,
                ] {
                    if ui
                        .selectable_label(*quantize == q, crate::i18n::t(quantize_label(q)))
                        .clicked()
                    {
                        *quantize = q;
                    }
                }
            });
            ui.separator();

            let tracks = launcher.tracks();
            let scenes = launcher.scenes();
            for scene in 0..scenes {
                ui.horizontal(|ui| {
                    // Scenknappen: startar hela raden.
                    let scene_name = (0..tracks)
                        .filter_map(|track| {
                            let slot = launcher.slot(track, scene);
                            match (slot.state, slot.content.name()) {
                                (SlotState::Playing, Some(name)) => Some(name.to_string()),
                                _ => None,
                            }
                        })
                        .next()
                        .unwrap_or_else(|| format!("{} {}", crate::i18n::t("Scen"), scene + 1));
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(format!("▶ {scene_name}")).strong(),
                            )
                            .min_size(egui::vec2(120.0, 26.0)),
                        )
                        .on_hover_text(crate::i18n::t("Startar alla slots i raden"))
                        .clicked()
                    {
                        let started = launcher.request_scene(scene);
                        *status_msg = if started > 0 {
                            crate::tstatus!("🎛 scen {} — {} start(er) köade", scene + 1, started)
                        } else {
                            crate::i18n::t("🎛 scenen är tom").to_string()
                        };
                    }

                    for track in 0..tracks {
                        let slot = launcher.slot(track, scene);
                        let state = slot.state;
                        let queued_stop = slot.queued_stop;
                        let name = slot.content.name().map(|s| s.to_string());
                        let text = slot_label(state, queued_stop, name.as_deref());
                        let mut button = egui::Button::new(egui::RichText::new(text).size(11.5))
                            .fill(slot_color(state, queued_stop))
                            .min_size(egui::vec2(150.0, 26.0));
                        if matches!(state, SlotState::Playing | SlotState::Queued) {
                            button = button.fill(slot_color(state, queued_stop));
                        }
                        let clicked = ui.add(button).clicked();
                        if clicked {
                            match state {
                                SlotState::Empty => {
                                    *status_msg =
                                        crate::i18n::t("🎛 slotten är tom — ingenting att starta")
                                            .to_string();
                                }
                                SlotState::Playing | SlotState::Queued => {
                                    launcher.request_stop(track, scene);
                                    *status_msg = crate::i18n::t(
                                        "🎛 stopp köat — slotten spelar till nästa punkt",
                                    )
                                    .to_string();
                                }
                                SlotState::Stopped => {
                                    match launcher.request_launch(track, scene) {
                                        Ok(()) => {
                                            *status_msg = crate::tstatus!(
                                                "🎛 start köad — sker vid nästa {}",
                                                crate::i18n::t(quantize_label(*quantize))
                                            )
                                        }
                                        Err(reason) => {
                                            *status_msg = crate::tstatus!(
                                                "🎛 kunde inte starta: {}",
                                                reason
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .button(crate::i18n::t("⏹ Stoppa allt"))
                    .clicked()
                {
                    launcher.stop_all();
                    *status_msg =
                        crate::i18n::t("🎛 allt stoppat till nästa punkt").to_string();
                }
                ui.label(
                    egui::RichText::new(if transport_playing {
                        crate::i18n::t("transporten går")
                    } else {
                        crate::i18n::t("transporten står still")
                    })
                    .size(10.5)
                    .color(Theme::TEXT_MUTED),
                );
            });
            ui.label(
                egui::RichText::new(crate::i18n::t(
                    "Ett klick startar eller stoppar vid nästa kvantiseringspunkt — kön syns i färgen.",
                ))
                .size(10.5)
                .color(Theme::TEXT_MUTED),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::song_structure_modal::SectionType;

    fn section(name: &str, bars: usize) -> SongSectionItem {
        SongSectionItem {
            section_type: SectionType::Verse,
            name: name.to_string(),
            length_bars: bars,
        }
    }

    /// **Början räknas fram, den gissas inte.** Sektionerna läggs efter varandra, och en
    /// sektion som saknar längd får en takt i stället för noll.
    #[test]
    fn section_start_bars_are_counted_not_guessed() {
        let sections = vec![
            section("Intro", 4),
            section("Vers", 8),
            section("Refräng", 0),
        ];
        let slots = slots_from_sections(&sections);
        assert_eq!(slots.len(), 3);
        match &slots[0] {
            SlotContent::Section {
                start_bar,
                length_bars,
                ..
            } => {
                assert_eq!(*start_bar, 0);
                assert_eq!(*length_bars, 4);
            }
            _ => panic!("fel innehåll"),
        }
        match &slots[1] {
            SlotContent::Section { start_bar, .. } => assert_eq!(*start_bar, 4),
            _ => panic!("fel innehåll"),
        }
        match &slots[2] {
            SlotContent::Section {
                start_bar,
                length_bars,
                ..
            } => {
                assert_eq!(*start_bar, 12, "efter Intro 4 + Vers 8");
                assert_eq!(*length_bars, 1, "noll takter blir en takt, inte ingenting");
            }
            _ => panic!("fel innehåll"),
        }
    }

    #[test]
    fn no_sections_is_no_slots() {
        assert!(slots_from_sections(&[]).is_empty());
    }

    /// Etiketterna är de fyra valen, och de går att översätta (de är i18n-nycklar).
    #[test]
    fn every_quantize_choice_has_a_label() {
        for q in [
            LaunchQuantize::Bar,
            LaunchQuantize::HalfBar,
            LaunchQuantize::Beat,
            LaunchQuantize::Immediate,
        ] {
            assert!(!quantize_label(q).is_empty());
        }
    }
}
