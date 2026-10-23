//! **Fraslyssning**: spela en fras en gång, innan den infogas.
//!
//! Frasbanken kunde infoga men inte låta. Att starta en egen tråd med en klocka hade gett en
//! andra tidsbas vid sidan av spelet; här räknas i stället frasens tick mot den tid som gått,
//! och appens vanliga uppdatering frågar varje bildruta vilka noter som ska klinga. Samma
//! klocka som resten av appen, ingen tråd, och logiken är ren och går att pröva utan ljud.
//!
//! Status: stabil — tiderna är heltal i frasens tick och prövas i `tests`.

use std::time::Instant;

/// En not i frasen, i samma enhet som `smf::MidiNote` (tick vid filens PPQ).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreviewNote {
    pub start: u32,
    pub length: u32,
    pub key: u8,
    pub velocity: f32,
}

/// Vad som ska hända när klockan flyttas fram: noter som ska tändas, och noter som ska släckas.
#[derive(Debug, Default, PartialEq)]
pub struct PreviewStep {
    pub start: Vec<(u8, f32)>,
    pub stop: Vec<u8>,
}

pub struct PhrasePreview {
    notes: Vec<PreviewNote>,
    started: Instant,
    /// Index för nästa not som inte tänts än; noterna sorteras vid start.
    next: usize,
    /// Klingande noter med sin sluttick, så att de kan släckas i rätt ordning.
    sounding: Vec<(u8, u32)>,
    end: u32,
}

impl PhrasePreview {
    pub fn new(mut notes: Vec<PreviewNote>) -> Self {
        notes.sort_by_key(|n| n.start);
        let end = notes
            .iter()
            .map(|n| n.start.saturating_add(n.length.max(1)))
            .max()
            .unwrap_or(0);
        Self {
            notes,
            started: Instant::now(),
            next: 0,
            sounding: Vec::new(),
            end,
        }
    }

    /// Noterna som klingar just nu — används när lyssningen avbryts mitt i.
    pub fn sounding(&self) -> Vec<u8> {
        self.sounding.iter().map(|(key, _)| *key).collect()
    }

    /// Noterna som ska tändas respektive släckas vid `tick`.
    ///
    /// Släppet ligger **före** anslaget i listan: samma tangent kan ligga två gånger efter
    /// varandra i en fras, och då måste den gamla rösten bort innan den nya tänds.
    pub fn step(&mut self, tick: u32) -> PreviewStep {
        let mut out = PreviewStep::default();
        self.sounding.retain(|(key, end)| {
            if *end <= tick {
                out.stop.push(*key);
                false
            } else {
                true
            }
        });
        while let Some(note) = self.notes.get(self.next) {
            if note.start > tick {
                break;
            }
            out.start.push((note.key, note.velocity));
            self.sounding
                .push((note.key, note.start.saturating_add(note.length.max(1))));
            self.next += 1;
        }
        out
    }

    pub fn finished(&self, tick: u32) -> bool {
        self.next >= self.notes.len() && tick >= self.end
    }

    /// Millisekunder sedan start — appen räknar om det till tick med tempot.
    pub fn elapsed_ms(&self) -> f32 {
        self.started.elapsed().as_secs_f32() * 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(start: u32, length: u32, key: u8) -> PreviewNote {
        PreviewNote {
            start,
            length,
            key,
            velocity: 0.8,
        }
    }

    #[test]
    fn notes_fire_at_their_tick_and_stop_at_their_end() {
        let mut preview = PhrasePreview::new(vec![note(0, 120, 60), note(240, 120, 64)]);

        let first = preview.step(0);
        assert_eq!(first.start, vec![(60, 0.8)]);
        assert!(first.stop.is_empty());

        let middle = preview.step(119);
        assert!(middle.start.is_empty() && middle.stop.is_empty());

        // Vid 120 tar noten slut — men nästa börjar först vid 240.
        assert_eq!(preview.step(120).stop, vec![60]);

        let second = preview.step(240);
        assert_eq!(second.start, vec![(64, 0.8)]);
        assert_eq!(preview.step(360).stop, vec![64]);
        assert!(preview.finished(360));
        assert!(!preview.finished(359));
    }

    #[test]
    fn a_repeated_key_is_released_before_it_is_struck_again() {
        let mut preview = PhrasePreview::new(vec![note(0, 120, 60), note(120, 120, 60)]);
        preview.step(0);
        let again = preview.step(120);
        assert_eq!(again.stop, vec![60], "den gamla rösten släpps först");
        assert_eq!(again.start, vec![(60, 0.8)]);
    }

    #[test]
    fn notes_are_sorted_even_if_the_file_is_not() {
        let mut preview = PhrasePreview::new(vec![note(480, 60, 67), note(0, 60, 60)]);
        assert_eq!(preview.step(0).start, vec![(60, 0.8)]);
        assert_eq!(preview.step(480).start, vec![(67, 0.8)]);
    }

    #[test]
    fn a_zero_length_note_still_ends() {
        // Längd 0 betyder "ett steg" i `.fsc`-läsaren, men modulen ska inte hänga om den
        // kommer in ändå: noll längd blir en tick och släpps.
        let mut preview = PhrasePreview::new(vec![note(0, 0, 60)]);
        preview.step(0);
        assert!(preview.step(0).stop.is_empty(), "en tick lång, inte noll");
        assert_eq!(preview.step(1).stop, vec![60]);
    }
}
