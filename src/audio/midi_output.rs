//! **MIDI ut — en väg, `midir`, på alla plattformar** (Sprint 1, punkt 4).
//!
//! In-vägen ([`crate::audio::midi_input`]) fanns; ut-vägen gjorde inte det, så varken en
//! extern synt, en LED på en kontroller eller en hårdvarusequencer kunde styras härifrån.
//! Modulen är medvetet liten: **hitta portar, öppna en, skicka noter**. Ingen egen
//! schemaläggning — appen skickar när den spelar, och tiden är då appens, inte ALSA:s.
//!
//! **Allt MIDI-ut går genom [`MidiOut`] och inget annat.** Samma läxa som in-vägen: två
//! implementationer betyder att bara en av dem prövas.
//!
//! Status: ny — ingen hårdvara finns på maskinen, så det som är *mätt* här är portlistan
//! och att sändningen inte ger fel. Att en mottagare hörde noten är **inte mätt**
//! (se `--selftest`, som säger det rakt ut).

use std::sync::Mutex;

/// En öppen utgång. Innehållet är mutexat eftersom `midir` kräver `&mut` för att skicka,
/// medan appen kan vilja skicka från mer än en plats i UI-tråden.
pub struct MidiOut {
    name: String,
    conn: Mutex<midir::MidiOutputConnection>,
}

impl MidiOut {
    /// Portnamnen som finns **nu** (listan hämtas färsk varje gång, som för in-vägen).
    pub fn ports() -> Vec<String> {
        match midir::MidiOutput::new("Sonix Out") {
            Ok(out) => out
                .ports()
                .iter()
                .map(|p| out.port_name(p).unwrap_or_else(|_| "?".to_string()))
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Öppnar porten med det givna indexet i [`MidiOut::ports`].
    pub fn open(index: usize) -> Result<Self, String> {
        let out = midir::MidiOutput::new("Sonix Out")
            .map_err(|e| format!("MIDI-ut kunde inte startas: {e}"))?;
        let ports = out.ports();
        let port = ports
            .get(index)
            .ok_or_else(|| format!("ingen MIDI-ut-port på plats {index}"))?;
        let name = out.port_name(port).unwrap_or_else(|_| "?".to_string());
        let conn = out
            .connect(port, "Sonix Out")
            .map_err(|e| format!("kunde inte öppna '{name}': {e}"))?;
        Ok(Self { name, conn: Mutex::new(conn) })
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Raw bytes — sysex, CCs, whatever a receiver expects. Control-surface feedback
    /// goes through here too, so there is still exactly one MIDI-out path.
    pub fn send(&self, bytes: &[u8]) -> Result<(), String> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| "MIDI-ut är låst av en trasig tråd".to_string())?;
        conn.send(bytes).map_err(|e| e.to_string())
    }

    pub fn note_on(&self, channel: u8, note: u8, velocity: u8) -> Result<(), String> {
        self.send(&[0x90 | (channel & 0x0F), note & 0x7F, velocity & 0x7F])
    }

    pub fn note_off(&self, channel: u8, note: u8, velocity: u8) -> Result<(), String> {
        self.send(&[0x80 | (channel & 0x0F), note & 0x7F, velocity & 0x7F])
    }

    /// Skickar en not på och av — används av `--selftest` och av "testnot"-knappen.
    pub fn blip(&self, channel: u8, note: u8, velocity: u8) -> Result<(), String> {
        self.note_on(channel, note, velocity)?;
        self.note_off(channel, note, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_port_list_is_always_answerable() {
        // Utan hårdvara är listan tom — men den ska gå att fråga utan att panika, och namnen
        // ska vara text (ingen `?` som ser ut som en port).
        let ports = MidiOut::ports();
        assert!(ports.iter().all(|p| !p.is_empty()));
    }

    #[test]
    fn opening_a_missing_port_is_an_error_not_a_panic() {
        // **Två miljöer, två rimliga fel.** På en maskin med ALSA-sequencer finns `Midi Through`
        // och felet är "ingen port på plats N"; på en runner *utan* MIDI-stack går själva
        // `MidiOutput::new` bet. Provet kräver därför att felet finns och har en text — inte
        // vilken text det är. (Samma läxa som i18n-provet: ett prov som letar en viss sträng
        // mäter översättningen, inte beteendet.)
        let too_high = MidiOut::ports().len() + 5;
        match MidiOut::open(too_high) {
            Ok(_) => panic!("plats {too_high} finns inte"),
            Err(msg) => assert!(!msg.is_empty(), "felet måste ha en text"),
        }
    }

    #[test]
    fn a_note_reaches_the_first_real_port() {
        // `Midi Through` finns på varje ALSA-system. Saknas den (annat OS, ingen port) är
        // provet "inte mätt" och hoppar över sig självt — det står i utdatan, inte som grönt.
        let ports = MidiOut::ports();
        if ports.is_empty() {
            eprintln!("MIDI-ut: inga portar — testnoten är inte mätt");
            return;
        }
        let out = MidiOut::open(0).expect("port 0 ska gå att öppna");
        out.blip(0, 60, 100).expect("noten ska gå att skicka");
        eprintln!("MIDI-ut: not skickad till '{}'", out.name());
    }
}
