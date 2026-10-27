//! **Vakthunden för ljudströmmen.**
//!
//! cpal 0.15 återhämtar sig inte från en ALSA-ström som svarat POLLERR: `alsa::poll()`
//! fortsätter svara fel, callbacken kallas aldrig mer — men strömmen ser fortfarande
//! "RUNNING" ut i `pactl`. Appen blir tyst utan att något säger till. Mätt 2026-10-27:
//! 40 000 fel/s, 1,5 GB logg och noll ljud; att bygga om strömmen gav tillbaka ljudet
//! direkt (ren ström, noll fel), så omstart är den väg som finns.
//!
//! Regeln bor i `engine::audio_stream_frozen` som ren funktion med eget prov. Här mäts
//! bara tiden, och omstarten använder samma `reconfigure` +
//! `resync_engine_after_reconfigure` som ljudmodalen — ingen ny väg att hålla i takt.

use crate::audio::engine::{audio_stream_frozen, AUDIO_REBUILD_COOLDOWN_SECS};
use super::*;

impl SonixApp {
    /// Körs en gång per bildruta. Bygger om strömmen när hjärtslaget stått stilla.
    pub(crate) fn audio_watchdog(&mut self, ctx: &egui::Context) {
        // Kapas som autosparandet: en återupptagen bildruta efter en paus ska inte
        // räknas som en sekund av tystnad.
        let dt = ctx.input(|i| i.stable_dt).min(0.5);
        let heartbeat = self.engine.heartbeat();
        if heartbeat != self.audio_heartbeat_seen {
            self.audio_heartbeat_seen = heartbeat;
            self.audio_frozen_secs = 0.0;
        } else {
            self.audio_frozen_secs += dt;
        }
        self.audio_rebuild_cooldown = (self.audio_rebuild_cooldown - dt).max(0.0);

        if !audio_stream_frozen(self.audio_frozen_secs, self.audio_rebuild_cooldown) {
            return;
        }
        self.audio_rebuild_cooldown = AUDIO_REBUILD_COOLDOWN_SECS;

        let rate = Some(self.engine.sample_rate);
        let buffer = self.engine.buffer_frames;
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
