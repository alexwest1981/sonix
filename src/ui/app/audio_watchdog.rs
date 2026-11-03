//! **Vakthunden för ljudströmmen.**
//!
//! cpal 0.15 återhämtar sig inte från en ALSA-ström som svarat POLLERR: `alsa::poll()`
//! fortsätter svara fel, callbacken kallas aldrig mer — men strömmen ser fortfarande
//! "RUNNING" ut i `pactl`. Appen blir tyst utan att något säger till. Mätt 2026-10-27:
//! 40 000 fel/s, 1,5 GB logg och noll ljud.
//!
//! **Varför strömmen dör** (mätt samma dag, samma cpal-ström som appen använder): en
//! callback som tar 20 ms — schemaläggaren i en lastad desktop, precis vad som händer
//! när stämmor läses in — får en 128- eller 256-framesström att fastna i POLLERR för
//! alltid, medan 512 och 1024 står emot. Därför bygger vakthunden om strömmen, och
//! klättrar buffertstegen uppåt i stället för att upprepa en inställning som inte
//! håller. Trappan bor i `engine::rebuild_step` som ren funktion med eget prov.
//!
//! Utan tak vore det här en klickloop: var tredje sekund revs strömmen ner och
//! byggdes upp igen (129 gånger på sex minuter, mätt), och det hörs som ett klickande
//! i takt. När bufferttrappan i `rebuild_step` är slut ger den upp och säger det i
//! statusraden i stället.

use crate::audio::engine::{audio_stream_frozen, rebuild_step, Rebuild, AUDIO_REBUILD_COOLDOWN_SECS};
use super::*;

/// Hur länge en ny ström ska leva innan den räknas som återhämtad (och trappan
/// nollställs). Kortare än så hinner en trasig inställning aldrig visa sig.
const AUDIO_RECOVERED_SECS: f32 = 10.0;

impl SonixApp {
    /// Körs en gång per bildruta. Bygger om strömmen när hjärtslaget stått stilla.
    pub(crate) fn audio_watchdog(&mut self, ctx: &egui::Context) {
        // Kapas som autosparandet: en återupptagen bildruta efter en paus ska inte
        // räknas som en sekund av tystnad.
        let dt = ctx.input(|i| i.stable_dt).min(0.5);
        // **Ingen omstart medan fönstret är i bakgrunden och inget spelar** (Alex 2026-11-03:
        // "not responding" så fort han bytte arbetsyta). En omstart öppnar ljudenheten, och det är
        // den enda här som kan blockera bildrutan i sekunder — på en enhet som svarar POLLERR kan
        // den ta längre tid än window-managerns tålamod. Villkoret är **båda**: står transporten
        // och spelar lyssnar han även när fönstret ligger bakom ett annat, och då måste ljudet
        // räddas; är inget på väg ut finns inget att rädda, och räknaren står still tills appen
        // syns igen.
        let focused = ctx.input(|i| i.viewport().focused.unwrap_or(true));
        if !focused && !self.is_playing {
            self.audio_frozen_secs = 0.0;
            self.audio_recovered_secs = 0.0;
            self.audio_rebuild_cooldown = (self.audio_rebuild_cooldown - dt).max(0.0);
            return;
        }
        let heartbeat = self.engine.heartbeat();
        if heartbeat != self.audio_heartbeat_seen {
            self.audio_heartbeat_seen = heartbeat;
            self.audio_frozen_secs = 0.0;
            // Strömmen lever och har levt ett tag: trappan börjar om från början.
            self.audio_recovered_secs += dt;
            if self.audio_recovered_secs >= AUDIO_RECOVERED_SECS {
                self.audio_rebuild_failures = 0;
                self.audio_gave_up = false;
            }
        } else {
            self.audio_frozen_secs += dt;
            self.audio_recovered_secs = 0.0;
        }
        self.audio_rebuild_cooldown = (self.audio_rebuild_cooldown - dt).max(0.0);

        if !audio_stream_frozen(self.audio_frozen_secs, self.audio_rebuild_cooldown) {
            return;
        }
        self.audio_rebuild_cooldown = AUDIO_REBUILD_COOLDOWN_SECS;

        let step = rebuild_step(self.engine.buffer_frames, self.audio_rebuild_failures);
        if step == Rebuild::GiveUp {
            // Säg det en gång, inte varannan sekund.
            // **Trappan vet bäst när den är slut.** En egen konstant kunde aldrig enas med
            // `rebuild_step`s fyra steg — beskedet skrevs därför aldrig alls (EGM runda 4).
            if !self.audio_gave_up {
                self.audio_gave_up = true;
                self.status_message = crate::tstatus!(
                    "⚠ ljudet kan inte hållas igång ens med {} frames — stäng av tunga program \
                     eller välj en annan enhet i ljudmodalen",
                    self.engine.buffer_frames.unwrap_or(0)
                );
            }
            return;
        }
        self.audio_rebuild_failures += 1;

        let rate = Some(self.engine.sample_rate);
        let buffer = match step {
            Rebuild::Same => self.engine.buffer_frames,
            Rebuild::Bigger(frames) => Some(frames),
            Rebuild::GiveUp => unreachable!("avbrutet ovan"),
        };
        let was_playing = self.is_playing;
        // Före omstarten: reconfigure nollställer räknaren, och det är den döda
        // strömmens fel som ska stå i beskedet.
        let failed_errors = self.engine.error_count();
        match self.engine.reconfigure(rate, buffer) {
            Ok(()) => {
                // Samma efterarbete som ljudmodalen: den nya strömmen har en ny synth,
                // och allt UI:t redan skickat måste skickas igen.
                self.resync_engine_after_reconfigure();
                // Resyncen lämnar transporten stoppad. Den som tryckte play när strömmen
                // dog ska inte behöva trycka igen: spela från samma position.
                if was_playing {
                    self.is_playing = true;
                    let _ = self.engine.send_command(AudioCommand::SeekSongPosition(self.song_time));
                    let _ = self.engine.send_command(AudioCommand::SetSongPlayback(true));
                }
                self.audio_heartbeat_seen = self.engine.heartbeat();
                self.audio_frozen_secs = 0.0;
                self.audio_recovered_secs = 0.0;
                let buf = match self.engine.buffer_frames {
                    Some(f) => format!("{f} frames"),
                    None => crate::i18n::t("enhetens standard").to_string(),
                };
                self.status_message = crate::tstatus!(
                    "⚠ ljudströmmen hade dött efter {} fel — startad om ({} Hz, {})",
                    failed_errors,
                    self.engine.sample_rate,
                    buf
                );
            }
            Err(e) => {
                self.status_message = crate::tstatus!(
                    "⚠ ljudströmmen är död och gick inte att starta om: {}",
                    e
                );
            }
        }
    }
}
