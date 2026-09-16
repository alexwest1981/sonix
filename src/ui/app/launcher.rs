//! **App-glue:t för scenrutnätet** — Sprint 1, punkt 5, steg 2.
//!
//! Delar samma uppgift som övriga `ui/app/*`: appen lånar modellens regler och gör det som bara
//! appen kan — starta transporten, sätta slingan, skriva i statusraden. Modellen (`audio::launcher`)
//! bestämmer **när**; den här filen lyder.
//!
//! `tick_launcher` går varje bildruta, även när rutan är stängd: en köad start ska inte stå still
//! för att man tittar bort.

use eframe::egui;

use crate::audio::launcher::{Launcher, LauncherCommand};
use crate::i18n::t;
use crate::ui::app::SonixApp;

impl SonixApp {
    /// Rutnätet byggs ur låtens sektioner: en slot per sektion, i taktordning. Färre än fyra
    /// sektioner ger fyra rader, så rutan aldrig är tom att titta på (men tomma slots nekas).
    pub(crate) fn rebuild_launcher_from_sections(&mut self) {
        let sections = self.song_structure_state.sections.clone();
        let contents = crate::ui::launcher_view::slots_from_sections(&sections);
        let scenes = contents.len().max(4);
        self.launcher = Launcher::new(1, scenes, 4);
        for (scene, content) in contents.into_iter().enumerate() {
            self.launcher.set_content(0, scene, content);
        }
        self.launcher_section_count = sections.len();
    }

    /// Modellen tickar mot transportens läge och kommandona verkställs här.
    pub(crate) fn tick_launcher(&mut self) {
        // Rutnätet följer sektionerna: ändras antalet byggs det om. (Längder och namn får
        // vänta tills ingenting spelar — att bygga om under spel skulle stoppa musiken.)
        if self.launcher_section_count != self.song_structure_state.sections.len() {
            self.rebuild_launcher_from_sections();
        }

        let beat = (self.song_step_in_bar / 4) % self.launcher.beats_per_bar;
        let commands =
            self.launcher
                .tick(self.song_bar, beat, self.launch_quantize, self.is_playing);
        for command in commands {
            match command {
                LauncherCommand::SetLoop { start_bar, end_bar } => {
                    self.loop_start_bar = start_bar;
                    self.loop_end_bar = end_bar.max(start_bar + 1);
                    self.status_message =
                        crate::tstatus!("🎛 slinga: takt {}–{}", start_bar + 1, end_bar);
                }
                LauncherCommand::Play => {
                    if !self.is_playing {
                        self.toggle_playback();
                    }
                }
                LauncherCommand::Stop => {
                    if self.is_playing {
                        self.stop_playback();
                        self.status_message = t("🎛 rutnätet: allt stoppat").to_string();
                    }
                }
            }
        }
    }

    /// Ritar rutan (och låter modellen ticka, stängd eller öppen).
    pub(crate) fn render_launcher_view(&mut self, ctx: &egui::Context) {
        self.tick_launcher();
        let transport_playing = self.is_playing;
        let mut status = self.status_message.clone();
        crate::ui::launcher_view::render_launcher_window(
            ctx,
            &mut self.show_launcher,
            &mut self.launcher,
            &mut self.launch_quantize,
            transport_playing,
            &mut status,
        );
        self.status_message = status;
    }
}
