//! **MusicXML-export**: tagningen som noter i en fil andra program kan öppna.
//!
//! Tagningen är en takt i 4/4, så filen blir en takt med en stämma. `divisions` = 4
//! gör en sextondel till en division, alltså går stegen i tagningen rakt in som
//! `<duration>` utan omräkning. Positionen skrivs som tyst tid mellan noterna
//! (`<forward>`) i stället för att varje not får en egen takt — det är samma sak
//! för läsaren och mindre kod för oss.

use crate::midi_take::{Take, TakeNote};
use std::fmt::Write;

/// Divisioner per fjärdedel. En sextondel (ett steg) = 1 division.
const DIVISIONS: u32 = 4;

/// Tonhöjd som (stam, alter, oktav). Kors och inga b-förtecken: samma val som
/// rutnätet gör, så en svart tangent heter C# och inte Db.
fn pitch(key: u8) -> (&'static str, u8, i32) {
    const STEPS: [&str; 12] = ["C", "C", "D", "D", "E", "F", "F", "G", "G", "A", "A", "B"];
    const ALTERS: [u8; 12] = [0, 1, 0, 1, 0, 0, 1, 0, 1, 0, 1, 0];
    let pc = key as usize % 12;
    (STEPS[pc], ALTERS[pc], key as i32 / 12 - 1)
}

/// XML-text som tål & < > i titeln.
fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Notens längd i divisioner. Okänd längd (0,0) blir en sextondel — en not utan
/// längd ska höras, inte försvinna.
fn duration(n: &TakeNote) -> u32 {
    if n.length_steps > 0.0 {
        n.length_steps.round().max(1.0) as u32
    } else {
        1
    }
}

/// Längden som notnamn: 4 = fjärdedel, 2 = åttondel, 1 = sextondel.
// shortcut: längre noter än en fjärdedel skrivs som quarter; ta halv- och helnoter när längderna i tagningen används.
fn note_type(d: u32) -> &'static str {
    match d {
        0 | 1 => "16th",
        2 | 3 => "eighth",
        _ => "quarter",
    }
}

/// Tagningen som ett MusicXML-dokument (score-partwise 3.1, en del `P1`).
pub fn take_to_musicxml(take: &Take, title: &str, bpm: f32) -> String {
    let mut notes: Vec<&TakeNote> = take.notes.iter().collect();
    notes.sort_by(|a, b| a.pos.total_cmp(&b.pos));

    let mut x = String::new();
    x.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    x.push_str("<!DOCTYPE score-partwise PUBLIC \"-//Recordare//DTD MusicXML 3.1 Partwise//EN\" \"http://www.musicxml.org/dtds/partwise.dtd\">\n");
    x.push_str("<score-partwise version=\"3.1\">\n");
    x.push_str("  <part-list>\n");
    let _ = writeln!(
        x,
        "    <score-part id=\"P1\"><part-name>{}</part-name></score-part>",
        escape(title)
    );
    x.push_str("  </part-list>\n");
    x.push_str("  <part id=\"P1\">\n");
    x.push_str("    <measure number=\"1\">\n");
    let _ = writeln!(
        x,
        "      <attributes><divisions>{DIVISIONS}</divisions>\
         <key><fifths>0</fifths></key>\
         <time><beats>4</beats><beat-type>4</beat-type></time>\
         <clef><sign>G</sign><line>2</line></clef></attributes>"
    );
    let _ = writeln!(x, "      <sound tempo=\"{}\"/>", bpm.round() as i64);

    // Tidshjulen: var i takten läsaren står när noterna skrivs i ordning.
    let mut cursor: i64 = 0;
    let mut i = 0;
    while i < notes.len() {
        let pos = (notes[i].pos.round() as i64).clamp(0, 15);
        let mut j = i;
        while j < notes.len() && (notes[j].pos.round() as i64).clamp(0, 15) == pos {
            j += 1;
        }

        if pos > cursor {
            let _ = writeln!(x, "      <forward><duration>{}</duration></forward>", pos - cursor);
        } else if pos < cursor {
            // Noten börjar före den förra notens slut: backa läsaren dit.
            let _ = writeln!(x, "      <backup><duration>{}</duration></backup>", cursor - pos);
        }
        cursor = pos;

        for (k, n) in notes[i..j].iter().enumerate() {
            let (step, alt, octave) = pitch(n.key);
            let d = duration(n);
            x.push_str("      <note>\n");
            if k > 0 {
                x.push_str("        <chord/>\n");
            }
            let alter = if alt > 0 {
                format!("<alter>{alt}</alter>")
            } else {
                String::new()
            };
            let _ = writeln!(
                x,
                "        <pitch><step>{step}</step>{alter}<octave>{octave}</octave></pitch>"
            );
            let _ = writeln!(x, "        <duration>{d}</duration>");
            let _ = writeln!(x, "        <type>{}</type>", note_type(d));
            // Anslaget som dynamik i procent: 0,8 → 80.
            let _ = writeln!(
                x,
                "        <sound dynamics=\"{}\"/>",
                (n.velocity.clamp(0.0, 1.0) * 100.0).round() as i64
            );
            x.push_str("      </note>\n");
            if k == 0 {
                cursor += d as i64;
            }
        }
        i = j;
    }

    x.push_str("    </measure>\n");
    x.push_str("  </part>\n");
    x.push_str("</score-partwise>\n");
    x
}

/// Skriver tagningen som MusicXML till `path`.
#[allow(dead_code)] // exportmenyn i UI:t kallar den inte än
pub fn export_musicxml(take: &Take, path: &str, title: &str, bpm: f32) -> std::io::Result<()> {
    std::fs::write(path, take_to_musicxml(take, title, bpm))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn take_of(notes: &[(f32, u8)]) -> Take {
        let mut t = Take::new();
        for (pos, key) in notes {
            t.push(*pos, *key, 0.8);
        }
        t
    }

    /// Tre noter blir tre `<note>`-element med rätt stam och oktav.
    #[test]
    fn a_c_major_arpeggio_exports_its_pitches() {
        let xml = take_to_musicxml(&take_of(&[(0.0, 60), (4.0, 64), (8.0, 67)]), "Arp", 120.0);
        assert!(xml.contains("<step>C</step><octave>4</octave>"), "C4 ska finnas");
        assert!(xml.contains("<step>E</step>"), "E4 ska finnas");
        assert!(xml.contains("<step>G</step>"), "G4 ska finnas");
        assert_eq!(xml.matches("<note>").count(), 3, "en <note> per not");
    }

    /// En svart tangent får `alter` = 1.
    #[test]
    fn a_black_key_gets_a_sharp() {
        let xml = take_to_musicxml(&take_of(&[(0.0, 61)]), "C#", 120.0);
        assert!(xml.contains("<step>C</step>"));
        assert!(xml.contains("<alter>1</alter>"), "C#4 ska ha alter 1");
    }

    /// Två noter på samma position blir ett ackord: en `<chord/>`, inte två.
    #[test]
    fn notes_on_the_same_position_become_one_chord() {
        let xml = take_to_musicxml(&take_of(&[(2.0, 60), (2.0, 64)]), "Ackord", 120.0);
        assert_eq!(xml.matches("<chord/>").count(), 1);
        assert_eq!(xml.matches("<note>").count(), 2);
    }

    /// Fyra steg blir en fjärdedel, och en fjärdedel tar fyra divisioner.
    #[test]
    fn a_four_step_note_is_a_quarter() {
        let mut t = take_of(&[(0.0, 60)]);
        t.notes[0].length_steps = 4.0;
        let xml = take_to_musicxml(&t, "Längd", 120.0);
        assert!(xml.contains("<type>quarter</type>"));
        assert!(xml.contains("<duration>4</duration>"));
    }

    /// Tempot står i första takten, som en `<sound tempo>`.
    #[test]
    fn the_tempo_is_written_in_the_first_measure() {
        let xml = take_to_musicxml(&take_of(&[(0.0, 60)]), "Tempo", 132.0);
        assert!(xml.contains("<sound tempo=\"132\"/>"));
    }

    /// Titeln escapas, så en & i namnet inte gör filen oläsbar.
    #[test]
    fn the_title_is_escaped() {
        let xml = take_to_musicxml(&take_of(&[(0.0, 60)]), "Rock & Roll", 120.0);
        assert!(xml.contains("<part-name>Rock &amp; Roll</part-name>"));
    }
}
