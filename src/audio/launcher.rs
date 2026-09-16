//! **Launch-modellen: en rutnätsstyrd spelare** — Sprint 1, punkt 5 (första steget).
//!
//! Alex: *"Jag har ingen launchpad, tänkte om vi kunde bygga en mjukvarustyrd?"*
//!
//! Svaret är att bygga **modellen först och ytan sedan** — för då blir en riktig Launchpad
//! (punkt 4) inte en omskrivning utan bara en till anropare. Modulen här vet ingenting om
//! fönster, MIDI eller ljud: den håller koll på *vad som är på väg att hända och när*, och
//! lämnar tillbaka kommandon till den som äger transporten.
//!
//! **En modell, två ytor:**
//!
//! ```text
//!   rutnätet på skärmen ─┐
//!                        ├─→  Launcher  ──tick(bar, slag)──→  [SetLoop, Play, Stop]  ──→  transporten
//!   Launchpad via MIDI ──┘
//! ```
//!
//! **Reglerna är inte påhittade** — de tolv gemensamma reglerna i
//! `~/.sonix-research-looper-sv.md` (Ableton/Bitwig/Logic/FL) ligger bakom varje val här:
//!
//! 1. En start är **kvantiserad**: den sker vid nästa startpunkt (takt, halvtakt, slag eller
//!    direkt), aldrig mitt i ett ord.
//! 2. En köad slot **visar att den är köad** medan den väntar — annars ser gränssnittet dött ut.
//! 3. Ett klick på en spelande slot **könar ett stopp** till samma kvantiseringspunkt; den
//!    fortsätter spela till dess.
//! 4. **En slot per spår åt gången**: den nya tar över, den gamla stoppas vid samma punkt.
//! 5. En **scen** startar alla sina slots, och en tom slot är inte en start (den nekas, och
//!    ingenting ändras — hellre ett nekande än en tyst start av ingenting).
//! 6. Start **startar transporten** om den står still: rutnätets första klick är också "spela".
//! 7. När sista spelande slotten stoppas **stannar transporten**.
//! 8. Längden gissas aldrig: en slot bär sin egen längd i takter, och slingan sätts till
//!    innehållets gränser.
//!
//! Status: byggs (Sprint 1, punkt 5, steg 1) — modellen och dess regler är prövade; rutnätet
//! på skärmen är nästa steg, MIDI-ut (punkt 4) det därpå.
//! Rör inte: `tick` är den **enda** vägen från ett klick till ett kommando. Ytorna får skriva
//! tillstånd, men aldrig bestämma själva när något ska hända — då skulle skärmen och en
//! Launchpad kunna vara oense om samma takt.

/// När en start får ske. Ordningen är från mest tålamod till mest otålig.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LaunchQuantize {
    /// Nästa takt.
    Bar,
    /// Nästa halvtakt (taktens slag 0 eller hälften).
    HalfBar,
    /// Nästa slag.
    Beat,
    /// Så snart appen hinner — nästa `tick`, inte mitt i en bildruta.
    Immediate,
}

impl LaunchQuantize {
    /// Är (takt, slag) en startpunkt för den här kvantiseringen?
    ///
    /// `beats_per_bar` kommer från transporten — modulen antar inte 4/4, den frågar.
    pub fn is_point(self, beat: usize, beats_per_bar: usize) -> bool {
        let beats = beats_per_bar.max(1);
        match self {
            LaunchQuantize::Bar => beat == 0,
            LaunchQuantize::HalfBar => beat == 0 || beat == beats / 2,
            LaunchQuantize::Beat => true,
            LaunchQuantize::Immediate => true,
        }
    }
}

/// Vad som ligger i en slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SlotContent {
    Empty,
    /// En sektion av låten: den bär sin egen början och längd, så ingenting gissas.
    Section {
        name: String,
        start_bar: usize,
        length_bars: usize,
    },
}

impl SlotContent {
    pub fn is_empty(&self) -> bool {
        matches!(self, SlotContent::Empty)
    }

    pub fn name(&self) -> Option<&str> {
        match self {
            SlotContent::Section { name, .. } => Some(name),
            SlotContent::Empty => None,
        }
    }
}

/// Slotens tillstånd. `Queued` och `queued_stop` är **synliga** med flit: en kö är något
/// användaren ska se, inte något som händer i tysthet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotState {
    Empty,
    Stopped,
    Queued,
    Playing,
}

#[derive(Clone, Debug)]
pub struct Slot {
    pub content: SlotContent,
    pub state: SlotState,
    /// En spelande slot som väntar på att få stanna vid nästa punkt.
    pub queued_stop: bool,
}

/// Vad transporten ska göra. Modulen rör ingen transport själv — den säger vad som ska hända
/// och den som äger ljudet bestämmer hur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LauncherCommand {
    /// Sätt slingan till sektionen (takt `start_bar` till `end_bar`).
    SetLoop { start_bar: usize, end_bar: usize },
    /// Starta uppspelningen (om den inte redan går).
    Play,
    /// Stanna uppspelningen: ingenting spelar längre.
    Stop,
}

/// Rutnätet: `tracks × scenes` slots, radvis.
#[derive(Clone, Debug)]
pub struct Launcher {
    tracks: usize,
    scenes: usize,
    slots: Vec<Slot>,
    pub beats_per_bar: usize,
}

impl Launcher {
    pub fn new(tracks: usize, scenes: usize, beats_per_bar: usize) -> Self {
        let tracks = tracks.max(1);
        let scenes = scenes.max(1);
        let slots = vec![
            Slot {
                content: SlotContent::Empty,
                state: SlotState::Empty,
                queued_stop: false,
            };
            tracks * scenes
        ];
        Self {
            tracks,
            scenes,
            slots,
            beats_per_bar: beats_per_bar.max(1),
        }
    }

    pub fn tracks(&self) -> usize {
        self.tracks
    }

    pub fn scenes(&self) -> usize {
        self.scenes
    }

    fn index(&self, track: usize, scene: usize) -> usize {
        scene.min(self.scenes - 1) * self.tracks + track.min(self.tracks - 1)
    }

    pub fn slot(&self, track: usize, scene: usize) -> &Slot {
        &self.slots[self.index(track, scene)]
    }

    /// Lägger innehåll i en slot. Ett spelande innehåll byts **inte ut i tysthet** — det
    /// stoppas först (returnerar falskt), så att en yta kan säga till.
    pub fn set_content(&mut self, track: usize, scene: usize, content: SlotContent) -> bool {
        let i = self.index(track, scene);
        if self.slots[i].state == SlotState::Playing || self.slots[i].state == SlotState::Queued {
            return false;
        }
        let state = if content.is_empty() {
            SlotState::Empty
        } else {
            SlotState::Stopped
        };
        self.slots[i].content = content;
        self.slots[i].state = state;
        self.slots[i].queued_stop = false;
        true
    }

    /// **Start.** Nekas om slotten är tom — ingenting ändras då, och skälet går att visa.
    pub fn request_launch(&mut self, track: usize, scene: usize) -> Result<(), &'static str> {
        let i = self.index(track, scene);
        if self.slots[i].content.is_empty() {
            return Err("tom slot");
        }
        self.slots[i].state = SlotState::Queued;
        self.slots[i].queued_stop = false;
        Ok(())
    }

    /// **Stopp.** En slot som spelar slutar spela vid nästa punkt; en köad slot avbryts direkt
    /// (den har inte börjat, så det finns ingenting att vänta på).
    pub fn request_stop(&mut self, track: usize, scene: usize) {
        let i = self.index(track, scene);
        match self.slots[i].state {
            SlotState::Playing => self.slots[i].queued_stop = true,
            SlotState::Queued => self.slots[i].state = SlotState::Stopped,
            SlotState::Empty => {}
            SlotState::Stopped => {}
        }
    }

    /// Startar hela scenen (raden). Tomma slots nekas var för sig — resten startar.
    pub fn request_scene(&mut self, scene: usize) -> usize {
        let mut started = 0;
        for track in 0..self.tracks {
            if self.request_launch(track, scene).is_ok() {
                started += 1;
            }
        }
        started
    }

    /// Könar stopp för allt som spelar.
    pub fn stop_all(&mut self) {
        for slot in self.slots.iter_mut() {
            if slot.state == SlotState::Playing {
                slot.queued_stop = true;
            } else if slot.state == SlotState::Queued {
                slot.state = SlotState::Stopped;
            }
        }
    }

    pub fn is_playing(&self) -> bool {
        self.slots.iter().any(|s| s.state == SlotState::Playing)
    }

    /// **Hjärtat.** Anropas en gång per bildruta med transportens läge. Vid en kvantiseringspunkt
    /// verkställs kön och kommandona kommer tillbaka i den ordning de ska utföras.
    pub fn tick(
        &mut self,
        bar: usize,
        beat: usize,
        quantize: LaunchQuantize,
        transport_playing: bool,
    ) -> Vec<LauncherCommand> {
        if !quantize.is_point(beat, self.beats_per_bar) {
            return Vec::new();
        }

        let mut commands = Vec::new();
        let was_playing = self.is_playing();
        let mut started_anything = false;
        let mut stopped_anything = false;

        // 1. Stopp först: den gamla slotten ska lämna ifrån sig spåret innan den nya tar över.
        for i in 0..self.slots.len() {
            if self.slots[i].queued_stop {
                self.slots[i].queued_stop = false;
                if self.slots[i].state == SlotState::Playing {
                    self.slots[i].state = SlotState::Stopped;
                    stopped_anything = true;
                }
            }
        }

        // 2. Sedan starterna, spår för spår (regel 4: en slot per spår).
        for track in 0..self.tracks {
            let mut to_start: Option<usize> = None;
            for scene in 0..self.scenes {
                let i = self.index(track, scene);
                if self.slots[i].state == SlotState::Queued {
                    to_start = Some(i);
                }
            }
            let Some(i) = to_start else { continue };
            // Allt annat på samma spår viker för den nya.
            for scene in 0..self.scenes {
                let j = self.index(track, scene);
                if j != i && self.slots[j].state == SlotState::Playing {
                    self.slots[j].state = SlotState::Stopped;
                    stopped_anything = true;
                }
            }
            self.slots[i].state = SlotState::Playing;
            started_anything = true;
            if let SlotContent::Section {
                start_bar,
                length_bars,
                ..
            } = &self.slots[i].content
            {
                let start = *start_bar;
                let length = (*length_bars).max(1);
                commands.push(LauncherCommand::SetLoop {
                    start_bar: start,
                    end_bar: start + length,
                });
            }
        }

        let _ = (bar, was_playing);
        // 3. Transporten följer rutnätet: första starten spelar, sista stoppet stannar.
        if started_anything && !transport_playing {
            commands.push(LauncherCommand::Play);
        } else if stopped_anything && !self.is_playing() {
            commands.push(LauncherCommand::Stop);
        }
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn section(name: &str, start_bar: usize, length_bars: usize) -> SlotContent {
        SlotContent::Section {
            name: name.to_string(),
            start_bar,
            length_bars,
        }
    }

    /// Rutnätet med tre sektioner i samma spår (kolumn 0) — det läget Alex kan använda i dag.
    fn grid() -> Launcher {
        let mut l = Launcher::new(1, 3, 4);
        l.set_content(0, 0, section("Intro", 0, 4));
        l.set_content(0, 1, section("Vers", 4, 8));
        l.set_content(0, 2, section("Refräng", 12, 8));
        l
    }

    /// **Regel 1: ingen start mitt i en takt.** En start begärd vid slag 2 väntar till nästa takt.
    #[test]
    fn a_launch_waits_for_the_next_bar() {
        let mut l = grid();
        l.request_launch(0, 1).unwrap();
        assert_eq!(l.slot(0, 1).state, SlotState::Queued, "kön ska synas");

        // Slag 2 är ingen taktpunkt: ingenting händer.
        assert!(l.tick(4, 2, LaunchQuantize::Bar, false).is_empty());
        assert_eq!(l.slot(0, 1).state, SlotState::Queued);

        // Slag 0 i nästa takt: nu startar den, och transporten startar med den.
        let commands = l.tick(5, 0, LaunchQuantize::Bar, false);
        assert_eq!(l.slot(0, 1).state, SlotState::Playing);
        assert_eq!(
            commands,
            vec![
                LauncherCommand::SetLoop {
                    start_bar: 4,
                    end_bar: 12
                },
                LauncherCommand::Play,
            ]
        );
    }

    /// **Regel 3: ett klick på en spelande slot köar ett stopp** — den spelar till punkten.
    #[test]
    fn clicking_a_playing_slot_queues_a_stop() {
        let mut l = grid();
        l.request_launch(0, 0).unwrap();
        l.tick(0, 0, LaunchQuantize::Bar, false);
        assert!(l.is_playing());

        l.request_stop(0, 0);
        assert_eq!(l.slot(0, 0).state, SlotState::Playing, "den spelar vidare");
        assert!(l.slot(0, 0).queued_stop, "men kön ska synas");

        let commands = l.tick(4, 0, LaunchQuantize::Bar, true);
        assert_eq!(l.slot(0, 0).state, SlotState::Stopped);
        assert_eq!(
            commands,
            vec![LauncherCommand::Stop],
            "sista stoppet stannar transporten"
        );
    }

    /// **Regel 2 och 4: en slot per spår**, och den nya tar över vid samma punkt.
    #[test]
    fn a_new_slot_on_the_same_track_takes_over() {
        let mut l = grid();
        l.request_launch(0, 0).unwrap();
        l.tick(0, 0, LaunchQuantize::Bar, false);

        l.request_launch(0, 2).unwrap();
        assert_eq!(
            l.slot(0, 0).state,
            SlotState::Playing,
            "den gamla spelar till punkten"
        );
        let commands = l.tick(8, 0, LaunchQuantize::Bar, true);
        assert_eq!(l.slot(0, 0).state, SlotState::Stopped);
        assert_eq!(l.slot(0, 2).state, SlotState::Playing);
        assert_eq!(
            commands,
            vec![LauncherCommand::SetLoop {
                start_bar: 12,
                end_bar: 20
            }],
            "ingen Play när transporten redan går"
        );
    }

    /// **Regel 5: en tom slot nekas** — och ingenting ändras av ett nekande. Ett spår utan
    /// innehåll är tomt på riktigt, till skillnad från de fyllda raderna i `grid()`.
    #[test]
    fn an_empty_slot_is_refused_and_changes_nothing() {
        let mut l = Launcher::new(2, 2, 4);
        l.set_content(0, 0, section("Intro", 0, 4));

        assert_eq!(l.request_launch(1, 0), Err("tom slot"));
        assert_eq!(l.slot(1, 0).state, SlotState::Empty, "ett nekande ändrar ingenting");
        assert!(l.tick(1, 0, LaunchQuantize::Bar, false).is_empty());

        // En scen startar det som finns och hoppar tyst över det tomma.
        assert_eq!(l.request_scene(0), 1);
        let commands = l.tick(2, 0, LaunchQuantize::Bar, false);
        assert_eq!(l.slot(0, 0).state, SlotState::Playing);
        assert_eq!(l.slot(1, 0).state, SlotState::Empty);
        assert!(commands.contains(&LauncherCommand::Play));
    }

    /// En köad slot som stoppas avbryts **direkt** — den har inte börjat, så det finns
    /// ingenting att vänta på.
    #[test]
    fn a_queued_slot_is_cancelled_at_once() {
        let mut l = grid();
        l.request_launch(0, 1).unwrap();
        l.request_stop(0, 1);
        assert_eq!(l.slot(0, 1).state, SlotState::Stopped);
        assert!(l.tick(4, 0, LaunchQuantize::Bar, false).is_empty());
    }

    /// Kvantiseringen går att välja: slag, halvtakt, takt eller direkt.
    #[test]
    fn the_quantization_decides_when() {
        assert!(!LaunchQuantize::Bar.is_point(2, 4));
        assert!(LaunchQuantize::Bar.is_point(0, 4));
        assert!(LaunchQuantize::HalfBar.is_point(2, 4));
        assert!(!LaunchQuantize::HalfBar.is_point(3, 4));
        assert!(LaunchQuantize::Beat.is_point(3, 4));
        assert!(LaunchQuantize::Immediate.is_point(3, 4));
        // Modulen antar inte 4/4: en 3/4-takt ger halvtakten vid slag 1.
        assert!(LaunchQuantize::HalfBar.is_point(1, 3));
        assert!(!LaunchQuantize::HalfBar.is_point(2, 3));
    }

    /// **Direkt betyder nästa tick**, inte "mitt i bildrutan" — samma väg som allt annat.
    #[test]
    fn immediate_fires_on_the_next_tick() {
        let mut l = grid();
        l.request_launch(0, 0).unwrap();
        assert_eq!(l.slot(0, 0).state, SlotState::Queued);
        let commands = l.tick(0, 3, LaunchQuantize::Immediate, false);
        assert_eq!(l.slot(0, 0).state, SlotState::Playing);
        assert!(commands.contains(&LauncherCommand::Play));
    }

    /// **Regel 8: längden gissas aldrig.** Slingan blir innehållets egna gränser.
    #[test]
    fn the_loop_is_the_content_bounds() {
        let mut l = Launcher::new(1, 1, 4);
        l.set_content(0, 0, section("Brygga", 20, 6));
        l.request_launch(0, 0).unwrap();
        let commands = l.tick(0, 0, LaunchQuantize::Bar, false);
        assert_eq!(
            commands[0],
            LauncherCommand::SetLoop {
                start_bar: 20,
                end_bar: 26
            }
        );
    }

    /// Innehåll byts inte ut under spel — den som vill byta får stoppa först.
    #[test]
    fn playing_content_is_not_replaced_in_silence() {
        let mut l = grid();
        l.request_launch(0, 0).unwrap();
        l.tick(0, 0, LaunchQuantize::Bar, false);
        assert!(
            !l.set_content(0, 0, section("Annat", 0, 2)),
            "bytet ska nekas"
        );
        assert_eq!(l.slot(0, 0).content.name(), Some("Intro"));
    }

    /// Stoppa allt tömmer rutnätet på spelande slots och stannar transporten.
    #[test]
    fn stop_all_empties_the_playing_slots() {
        let mut l = Launcher::new(2, 2, 4);
        l.set_content(0, 0, section("A", 0, 4));
        l.set_content(1, 0, section("B", 4, 4));
        l.request_scene(0);
        l.tick(0, 0, LaunchQuantize::Bar, false);
        assert!(l.is_playing());

        l.stop_all();
        let commands = l.tick(4, 0, LaunchQuantize::Bar, true);
        assert!(!l.is_playing());
        assert_eq!(commands, vec![LauncherCommand::Stop]);
    }

}
