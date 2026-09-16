//! **Loop-pad: spela in ett lager i taget och låt det gå runt** — Sprint 1, punkt 7 (steg 1).
//!
//! Alex: *"Kan vi kika på loop pad med? Där man kan spela in, loopa, skapa nytt spår, spela in,
//! loopa osv, så man till slut kan spela in en hel låt med bara sin röst t ex, som Petebox gjorde."*
//!
//! Det är en **loopstation** (Boss RC-505, Ableton Looper, Loopy HD) — inte en launch-yta. Skillnaden
//! mot `launcher`: där startar man färdigt innehåll, här **blir inspelningen innehållet**. Ett tryck
//! spelar in exakt en loop-cykel, tar stopp av sig själv vid cykelns slut, och börjar genast spela —
//! utan att transporten stannar. Nästa tryck lägger ett lager ovanpå. Ångra tar bort det sista lagret.
//!
//! **Reglerna kommer ur samma research som startaren** (`~/.sonix-research-looper-sv.md`):
//!
//! - *"Inspelning börjar vid nästa kvantpunkt och transporten startar automatiskt om den stod
//!   stilla; övergången till loop sker utan att stanna"* (regel 6).
//! - `Rec-Length` = **loopens längd**, inte hur länge man råkade hålla knappen (Logic).
//! - *"vid stopp sätts längden till cellens längd eller förfluten tid avrundad uppåt till takt"*
//!   — det är regeln som gör att **första lagret får bestämma** när ingen slinga finns än.
//! - `At Rec-End`: spela upptagningen direkt i stället för att spela in fler pass.
//!
//! **Det som gör det här nära i Sonix** (mätt i koden 2026-09-16): spåren är redan PCM i minnet
//! (`StemVoiceTrack { left: Arc<Vec<f32>>, start_time_secs, .. }`), tagningen är redan PCM
//! (`AudioTake { pcm_samples, sample_rate }`), transporten har redan en taktslinga som rullar
//! tillbaka (`transport.rs`), och mikrofonen spelar redan in med monitor och autotune. Kvar är
//! kopplingen: **tagning → spår vid slingans start**, och att stoppa inspelningen på takten
//! i stället för på en knapp.
//!
//! Status: byggs (Sprint 1, punkt 7, steg 1) — modellen och dess regler är prövade. Glue:t
//! (tagning → `StemVoiceTrack` vid slingans start) är nästa steg, och därför är `dead_code`
//! tillåtet i filen så länge: modulen är byggd före sin väg, precis som `launcher` var i en dag.
//! Rör inte: `tick` är den enda vägen från ett tryck till ett kommando. Spelar en yta in själv
//! kommer två ytor att vara oense om var cykeln börjar — och då hamnar lagren i otakt.

#![allow(dead_code)]

/// Var loopstationen är i sin cykel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoopPhase {
    /// Ingenting pågår.
    Idle,
    /// Nedräkning innan första lagret (bara första gången, och bara om den är vald).
    CountingIn,
    /// Väntar på att cykeln ska börja. Ett tryck mitt i en takt ska inte klippa första slaget.
    Armed,
    /// Spelar in. `cycle` räknas från 1 och uppåt — flera pass är tillåtna (`At Rec-End`).
    Recording { cycle: usize },
    /// Allt spelar; nästa tryck lägger ett lager till.
    Playing,
}

/// Ett inspelat lager: ett varv av slingan.
#[derive(Clone, Debug, PartialEq)]
pub struct LoopLayer {
    pub name: String,
    pub start_bar: usize,
    pub length_bars: usize,
    pub muted: bool,
}

/// Vad appen ska göra. Modellen rör varken ljud eller transport själv.
#[derive(Clone, Debug, PartialEq)]
pub enum LoopCommand {
    /// Starta transporten (första lagret: något måste gå runt att sjunga mot). Appen kan
    /// strunta i kommandot om transporten redan går — modellen frågar inte efter tillståndet.
    StartTransport,
    /// Sätt slingan till det första lagrets längd (avrundat uppåt till takt).
    SetLoop { start_bar: usize, end_bar: usize },
    /// Börja spela in nu: tagningen ska tömmas och mikrofonen armeras.
    StartRecording { at_bar: usize },
    /// Stoppa inspelningen och **lägg tagningen som ett spår** vid `start_bar`,
    /// `length_bars` långt. Spåret ska spela direkt (ingen paus mellan in- och uppspelning).
    PlaceLayer {
        name: String,
        start_bar: usize,
        length_bars: usize,
    },
    /// Ta bort lagret (ångra).
    RemoveLayer { index: usize },
    /// Stoppa inspelningen utan att lägga ett lager (transporten stannade, eller avbrott).
    CancelRecording,
}

/// Loopstationen. `beats_per_bar` frågas, den antas inte.
#[derive(Clone, Debug)]
pub struct LoopStation {
    pub phase: LoopPhase,
    pub layers: Vec<LoopLayer>,
    /// Slingans gränser. `loop_end_bar == 0` betyder **ingen slinga satt än** — då bestämmer
    /// första lagret längden.
    pub loop_start_bar: usize,
    pub loop_end_bar: usize,
    /// Nedräkning i takter innan första lagret (0 = ingen).
    pub count_in_bars: usize,
    /// Hur många pass ett tryck spelar in innan det viker till uppspelning.
    pub cycles_per_press: usize,
    /// En inspelning som pågår just nu.
    recording_from_bar: Option<usize>,
    recording_cycle: usize,
    /// Har nedräkningen börjat räknas? (första slaget i nedräkningen)
    counting_from_bar: Option<usize>,
}

impl Default for LoopStation {
    fn default() -> Self {
        Self::new()
    }
}

impl LoopStation {
    pub fn new() -> Self {
        Self {
            phase: LoopPhase::Idle,
            layers: Vec::new(),
            loop_start_bar: 0,
            loop_end_bar: 0,
            count_in_bars: 0,
            cycles_per_press: 1,
            recording_from_bar: None,
            recording_cycle: 0,
            counting_from_bar: None,
        }
    }

    /// Finns en slinga att spela in mot?
    pub fn has_loop(&self) -> bool {
        self.loop_end_bar > self.loop_start_bar
    }

    pub fn loop_length_bars(&self) -> usize {
        self.loop_end_bar.saturating_sub(self.loop_start_bar)
    }

    pub fn is_recording(&self) -> bool {
        matches!(self.phase, LoopPhase::Recording { .. })
    }

    /// **Trycket.** Armerar inspelningen — den börjar vid nästa cykelstart, aldrig mitt i.
    /// Spelar något redan och ingen slinga finns, startar transporten först (regel 6).
    pub fn press(&mut self) -> Vec<LoopCommand> {
        match self.phase {
            LoopPhase::Recording { .. } => {
                // Ett tryck under inspelning avslutar den: lagret läggs och spelas upp direkt.
                self.finish_recording()
            }
            LoopPhase::Idle | LoopPhase::Playing => {
                // Första lagret behöver något att sjunga mot: transporten startar (appen struntar
                // i kommandot om den redan går). Nedräkningen, om den är vald, gäller bara då.
                if self.layers.is_empty() {
                    self.phase = if self.count_in_bars > 0 {
                        LoopPhase::CountingIn
                    } else {
                        LoopPhase::Armed
                    };
                    self.counting_from_bar = None;
                    vec![LoopCommand::StartTransport]
                } else {
                    self.phase = LoopPhase::Armed;
                    Vec::new()
                }
            }
            LoopPhase::Armed | LoopPhase::CountingIn => {
                // Ett tryck till tar tillbaka köandet — ingenting har börjat.
                self.phase = if self.layers.is_empty() {
                    LoopPhase::Idle
                } else {
                    LoopPhase::Playing
                };
                Vec::new()
            }
        }
    }

    /// Ångra: tar bort det sista lagret. Finns inget lager händer ingenting (inget påhittat).
    pub fn undo(&mut self) -> Vec<LoopCommand> {
        if self.is_recording() {
            return self.finish_recording();
        }
        let Some(index) = self.layers.len().checked_sub(1) else {
            return Vec::new();
        };
        self.layers.remove(index);
        self.phase = if self.layers.is_empty() {
            LoopPhase::Idle
        } else {
            LoopPhase::Playing
        };
        vec![LoopCommand::RemoveLayer { index }]
    }

    /// Avslutar en pågående inspelning och lägger lagret.
    fn finish_recording(&mut self) -> Vec<LoopCommand> {
        let Some(start_bar) = self.recording_from_bar.take() else {
            return Vec::new();
        };
        let length_bars = if self.has_loop() {
            self.loop_length_bars()
        } else {
            // Ingen slinga satt: längden är tiden som gick, avrundad uppåt till takt
            // (tidigast en takt — noll takter är ingen längd).
            let elapsed = self.elapsed_cycles().max(1);
            self.loop_start_bar = start_bar;
            self.loop_end_bar = start_bar + elapsed;
            self.recording_cycle = 0;
            self.layers.push(LoopLayer {
                name: format!("Lager {}", self.layers.len() + 1),
                start_bar,
                length_bars: elapsed,
                muted: false,
            });
            self.phase = LoopPhase::Playing;
            let layer = self.layers.last().cloned().expect("lagret lades nyss");
            return vec![
                LoopCommand::SetLoop {
                    start_bar,
                    end_bar: start_bar + elapsed,
                },
                LoopCommand::PlaceLayer {
                    name: layer.name,
                    start_bar,
                    length_bars: layer.length_bars,
                },
            ];
        };
        self.recording_cycle = 0;
        self.layers.push(LoopLayer {
            name: format!("Lager {}", self.layers.len() + 1),
            start_bar,
            length_bars,
            muted: false,
        });
        self.phase = LoopPhase::Playing;
        let layer = self.layers.last().cloned().expect("lagret lades nyss");
        vec![LoopCommand::PlaceLayer {
            name: layer.name,
            start_bar: layer.start_bar,
            length_bars: layer.length_bars,
        }]
    }

    /// Hur många cykler inspelningen har hållit på (minst 1 medan den pågår).
    fn elapsed_cycles(&self) -> usize {
        self.recording_cycle.max(1)
    }

    /// **Hjärtat.** Anropas varje bildruta med transportens läge.
    ///
    /// `bar` är aktuell takt och `at_cycle_start` är sant på cykelns första slag. Allt som har
    /// med *när* att göra avgörs här, så en knapp, en tangent och en framtida MIDI-pedal
    /// beter sig likadant.
    pub fn tick(
        &mut self,
        bar: usize,
        at_cycle_start: bool,
        transport_playing: bool,
    ) -> Vec<LoopCommand> {
        let mut commands = Vec::new();

        // Stannar transporten mitt i en inspelning avbryts den — en tagning utan takt är inget
        // lager, och den ska inte bli ett tyst spår som ligger och skräpar.
        if !transport_playing && self.is_recording() {
            self.recording_from_bar = None;
            self.recording_cycle = 0;
            self.phase = if self.layers.is_empty() {
                LoopPhase::Idle
            } else {
                LoopPhase::Playing
            };
            return vec![LoopCommand::CancelRecording];
        }

        match self.phase {
            LoopPhase::CountingIn => {
                if at_cycle_start {
                    let from = *self.counting_from_bar.get_or_insert(bar);
                    if bar.saturating_sub(from) >= self.count_in_bars {
                        self.counting_from_bar = None;
                        self.phase = LoopPhase::Armed;
                    }
                }
            }
            LoopPhase::Armed => {
                if at_cycle_start {
                    self.recording_from_bar = Some(bar);
                    self.recording_cycle = 1;
                    self.phase = LoopPhase::Recording { cycle: 1 };
                    commands.push(LoopCommand::StartRecording { at_bar: bar });
                }
            }
            LoopPhase::Recording { cycle } => {
                if at_cycle_start && bar != self.recording_from_bar.unwrap_or(bar) {
                    // Ett varv till, om flera pass är valda; annars är cykeln slut och lagret
                    // läggs **utan att transporten stannar**.
                    if cycle < self.cycles_per_press.max(1) {
                        self.recording_cycle = cycle + 1;
                        self.phase = LoopPhase::Recording { cycle: cycle + 1 };
                    } else {
                        commands.extend(self.finish_recording());
                    }
                }
            }
            LoopPhase::Idle | LoopPhase::Playing => {}
        }
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn armed_with_loop() -> LoopStation {
        let mut s = LoopStation::new();
        s.loop_start_bar = 0;
        s.loop_end_bar = 4;
        s.phase = LoopPhase::Armed;
        s
    }

    /// **Regel 6: ingen inspelning mitt i en takt.** Trycket armerar; tagningen börjar vid
    /// cykelns start, och transporten startar med den.
    #[test]
    fn arming_waits_for_the_cycle_start() {
        let mut s = LoopStation::new();
        let commands = s.press();
        assert_eq!(s.phase, LoopPhase::Armed);
        assert_eq!(commands, vec![LoopCommand::StartTransport]);

        // Mitt i takten händer ingenting.
        assert!(s.tick(2, false, true).is_empty());
        assert_eq!(s.phase, LoopPhase::Armed);

        // Vid cykelns start börjar tagningen.
        let commands = s.tick(4, true, true);
        assert_eq!(s.phase, LoopPhase::Recording { cycle: 1 });
        assert_eq!(commands, vec![LoopCommand::StartRecording { at_bar: 4 }]);
    }

    /// **Rec-Length = slingans längd.** Inspelningen tar slut av sig själv vid cykelns slut och
    /// lagret börjar spela **direkt** — ingen paus mellan in- och uppspelning.
    #[test]
    fn recording_stops_at_the_cycle_end_and_plays_at_once() {
        let mut s = armed_with_loop();
        s.tick(0, true, true);
        assert!(s.is_recording());

        // Ett varv senare: lagret läggs, ingenting stannar.
        let commands = s.tick(4, true, true);
        assert_eq!(s.phase, LoopPhase::Playing);
        assert_eq!(s.layers.len(), 1);
        assert_eq!(
            commands,
            vec![LoopCommand::PlaceLayer {
                name: "Lager 1".to_string(),
                start_bar: 0,
                length_bars: 4
            }]
        );
        assert!(
            !commands.contains(&LoopCommand::StartTransport),
            "transporten ska aldrig stanna eller startas om mellan varven"
        );
    }

    /// **Första lagret får bestämma när ingen slinga finns** — längden är tiden som gick,
    /// avrundad uppåt till takt, och slingan sätts till den.
    #[test]
    fn the_first_layer_defines_the_loop_when_none_is_set() {
        let mut s = LoopStation::new();
        s.press();
        s.tick(0, true, true);
        assert!(s.is_recording());

        // Ingen slinga: trycket igen avslutar tagningen.
        let commands = s.press();
        assert_eq!(s.phase, LoopPhase::Playing);
        assert!(s.has_loop());
        assert_eq!((s.loop_start_bar, s.loop_end_bar), (0, 1));
        assert!(
            commands.contains(&LoopCommand::SetLoop {
                start_bar: 0,
                end_bar: 1
            }),
            "slingan ska sättas till lagrets längd: {commands:?}"
        );
        assert_eq!(s.layers[0].length_bars, 1);
    }

    /// Lagren staplas och **ångra tar bort det sista** — inte allt, och inte ingenting.
    #[test]
    fn layers_stack_and_the_last_one_can_be_undone() {
        let mut s = LoopStation::new();
        s.loop_start_bar = 0;
        s.loop_end_bar = 4;
        for _ in 0..3 {
            s.press();
            s.tick(0, true, true);
            s.tick(4, true, true);
        }
        assert_eq!(s.layers.len(), 3);
        assert_eq!(s.layers[2].name, "Lager 3");

        let commands = s.undo();
        assert_eq!(commands, vec![LoopCommand::RemoveLayer { index: 2 }]);
        assert_eq!(s.layers.len(), 2);
        assert_eq!(s.phase, LoopPhase::Playing, "resten ska spela vidare");

        s.undo();
        s.undo();
        assert!(s.layers.is_empty());
        assert_eq!(s.phase, LoopPhase::Idle);
        // Ett tryck till på tom stapel hittar inte på ett lager.
        assert!(s.undo().is_empty());
        assert!(s.layers.is_empty());
    }

    /// Nedräkning fördröjer första lagret — men bara första gången.
    #[test]
    fn the_count_in_delays_only_the_first_layer() {
        let mut s = LoopStation::new();
        s.count_in_bars = 2;
        s.loop_start_bar = 0;
        s.loop_end_bar = 4;
        s.press();
        assert_eq!(s.phase, LoopPhase::CountingIn);

        s.tick(0, true, true); // nedräkningen börjar
        s.tick(1, true, true);
        assert_eq!(s.phase, LoopPhase::CountingIn, "två takter ska räknas");
        s.tick(2, true, true);
        assert_eq!(s.phase, LoopPhase::Armed, "efter nedräkningen armerad");

        s.tick(4, true, true);
        assert!(s.is_recording());
        s.tick(8, true, true);
        assert_eq!(s.layers.len(), 1);
    }

    /// Stannar transporten mitt i en inspelning **avbryts** den — ingen tyst tagning blir ett
    /// spår som ligger och skräpar, och inget lager påstås finnas.
    #[test]
    fn stopping_the_transport_cancels_the_recording() {
        let mut s = armed_with_loop();
        s.tick(0, true, true);
        assert!(s.is_recording());

        let commands = s.tick(2, false, false);
        assert_eq!(commands, vec![LoopCommand::CancelRecording]);
        assert!(!s.is_recording());
        assert!(s.layers.is_empty());
        assert_eq!(s.phase, LoopPhase::Idle);
    }

    /// Ett tryck mitt i en inspelning avslutar den och lägger lagret direkt.
    #[test]
    fn a_second_press_finishes_the_recording() {
        let mut s = armed_with_loop();
        s.tick(0, true, true);
        let commands = s.press();
        assert_eq!(s.phase, LoopPhase::Playing);
        assert_eq!(s.layers.len(), 1);
        assert!(matches!(commands[0], LoopCommand::PlaceLayer { .. }));
    }

    /// Flera pass är tillåtna (`At Rec-End`), men bara så många som valts.
    #[test]
    fn several_cycles_per_press_are_allowed() {
        let mut s = armed_with_loop();
        s.cycles_per_press = 2;
        s.tick(0, true, true);
        assert_eq!(s.phase, LoopPhase::Recording { cycle: 1 });
        s.tick(4, true, true);
        assert_eq!(s.phase, LoopPhase::Recording { cycle: 2 }, "andra passet");
        s.tick(8, true, true);
        assert_eq!(s.phase, LoopPhase::Playing);
        assert_eq!(s.layers.len(), 1);
    }

    /// Ett lager får **slingans** längd, inte tiden mellan två tryck. Det är hela skillnaden
    /// mellan en loopstation och en bandspelare.
    #[test]
    fn a_layer_gets_the_loop_length_not_the_press_length() {
        let mut s = LoopStation::new();
        s.loop_start_bar = 8;
        s.loop_end_bar = 16;
        s.phase = LoopPhase::Armed;
        s.tick(8, true, true);
        // Trycket kommer mitt i cykeln: lagret ska ändå bli åtta takter.
        let _ = s.press();
        assert_eq!(s.layers.len(), 1);
        assert_eq!(s.layers[0].length_bars, 8);
        assert_eq!(s.layers[0].start_bar, 8);
    }

    /// Ett tryck i köande läge tar tillbaka köandet: ingenting har börjat, alltså ingenting att ångra.
    #[test]
    fn a_second_press_before_the_start_cancels_the_queue() {
        let mut s = LoopStation::new();
        s.loop_start_bar = 0;
        s.loop_end_bar = 4;
        s.press();
        assert_eq!(s.phase, LoopPhase::Armed);
        let commands = s.press();
        assert!(commands.is_empty());
        assert_eq!(s.phase, LoopPhase::Idle);
        assert!(s.tick(0, true, true).is_empty());
    }
}
