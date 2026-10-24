//! Uppspelningen — stegklockan, sequencern, tangentbordet, tagningarna och hårdvaran.
//!
//! Stegklockan räknas från **förra deadline** (`steps_elapsed`), inte från nu: att sätta
//! ankaret till nu varje steg summerar felet (0,66 s efter 12,6 s). Spelhuvudet läser
//! motorns egen position — två klockor för samma sak driver isär, och en av dem hörs.
//!
//! Status: byggs — uppspelningen.
//! Rör inte: `step_duration` och motorns position är samma klocka; lägg inte en tredje.

use super::*;

/// Hur många hela steg klockan har hunnit förbi sedan ankaret (Fas 8.13b).
///
/// **Ren funktion**, så att regeln går att pröva utan fönster. Lärdomen den bär är
/// mätt i Alex' projekt 2026-09-13: spelhuvudet låg **0,66 s efter ljudet** 12,64 s
/// in i Rock and Hard Place, alltså 118 sextondelar × drygt halva bildrutetiden.
/// Felet kom av att ankaret flyttades till NU i stället för till nästa deadline —
/// varje steg blev upp till en bildruta för sent, och felet summerades. Att i stället
/// räkna *hur många steg tiden hunnit förbi* gör felet avgränsat till ett steg, hur
/// länge det än spelas: samma sak som DAW:ar gör när de låter spelhuvudet följa
/// ljudklockan.
///
/// Ett steg som ännu inte är inne räknas inte (golv, inte avrundning): spelhuvudet ska
/// aldrig ligga före ljudet.
pub(crate) fn steps_elapsed(anchor: Instant, step: std::time::Duration, now: Instant) -> usize {
    let step = step.as_nanos();
    if step == 0 {
        return 0;
    }
    let elapsed = now.duration_since(anchor).as_nanos();
    (elapsed / step) as usize
}

impl SonixApp {
/// Ensures that a dedicated Mic track exists at the very end of the playlist,
/// and that all tracks have their categorized colors, icons, and region colors refreshed.
pub fn ensure_mic_track_exists(&mut self) {
    if self.playlist_tracks.is_empty() {
        let mut mic_track = PlaylistTrack::new(crate::i18n::t("🎤 Mic (Voice & Sång)").to_string(), "🎤", TrackKind::VocalAudio, crate::ui::theme::palette().text());
        mic_track.volume = 1.0;
        mic_track.is_rec_armed = true;
        self.playlist_tracks.push(mic_track);
        return;
    }

    // 1. Check if any track is a mic track
    let mic_pos = self.playlist_tracks.iter().position(|t| {
        let n = t.name.to_lowercase();
        n.contains("mic") || n.contains("mikrofon") || n.contains("microphone") || n.contains("mik")
    });

    if let Some(pos) = mic_pos {
        // If it's not already the last track, move it to the end
        if pos != self.playlist_tracks.len() - 1 {
            let mic_t = self.playlist_tracks.remove(pos);
            self.playlist_tracks.push(mic_t);
        }
    } else {
        // Create dedicated Mic track at the end
        let mut mic_track = PlaylistTrack::new(crate::i18n::t("🎤 Mic (Voice & Sång)").to_string(), "🎤", TrackKind::VocalAudio, crate::ui::theme::palette().text());
        mic_track.volume = 1.0;
        mic_track.is_rec_armed = true;
        self.playlist_tracks.push(mic_track);
    }

    // 2. Re-classify and ensure colors are applied to every track and its regions
    let last_idx = self.playlist_tracks.len() - 1;
    for (idx, track) in self.playlist_tracks.iter_mut().enumerate() {
        let is_mic = idx == last_idx && (track.name.to_lowercase().contains("mic") || track.name.to_lowercase().contains("mik") || track.name.to_lowercase().contains("voice"));
        if is_mic {
            track.color = crate::ui::theme::palette().text();
            track.icon = "🎤";
            track.kind = TrackKind::VocalAudio;
            for r in &mut track.regions {
                r.color = crate::ui::theme::palette().text();
            }
        } else {
            let (kind, icon, color) = classify_track_style(&track.name);
            track.kind = kind;
            track.icon = icon;
            track.color = color;
            for r in &mut track.regions {
                r.color = color;
            }
        }
    }
}
}

impl SonixApp {
pub fn toggle_timeline_recording(&mut self) {
    if self.is_recording_timeline {
        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        let take_result = self.vocal_studio.stop_recording();
        self.is_recording_timeline = false;
        self.is_playing = false;
        let _ = self.engine.send_command(AudioCommand::SetSongPlayback(false));

        if let Err(e) = &take_result {
            self.status_message = format!("❌ {}", e);
        }
        if let Ok(take_idx) = take_result {
            if let Some(take) = self.vocal_studio.takes.get(take_idx).cloned() {
                let armed_idx = self.playlist_tracks.iter().position(|t| t.is_rec_armed).unwrap_or(0);
                let start_bar = self.timeline_rec_start_bar;
                let length_bars =
            (tempo.bars_for_secs_at(start_bar as f64, take.duration_secs as f64) as f32)
                .max(0.25);
                let mic_sr = take.sample_rate;

                let pcm_arc = std::sync::Arc::new(take.pcm_samples);
                self.playlist_tracks[armed_idx].pcm_audio = Some((pcm_arc.clone(), pcm_arc, mic_sr));

                let region = AudioRegion {
                    source_bpm: self.bpm, // tagningen gjordes i projektets tempo (8.10)
                    tape: false,
                    id: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as usize,
                    name: format!("🎤 {}", take.name),
                    start_bar,
                    length_bars,
                    sample_offset_sec: 0.0,
                    source_path: None,
                    waveform_peaks: take.waveform_data,
                    volume: 1.0,
                    fade_in_bars: 0.0,
                    fade_out_bars: 0.0,
                    muted: false,
                    is_reverse: false,
                    color: self.playlist_tracks[armed_idx].color,
                    loop_length_bars: 0.0,
                    warp_mode: Default::default(),
};
                self.playlist_tracks[armed_idx].regions.push(region);
                self.sync_track_stem_to_engine(armed_idx);
                self.status_message = crate::tstatus!("✔ Spelade in '{}' direkt i spår {} ({}) vid takt {:.1} ({:.2}s)!", take.name, armed_idx + 1, self.playlist_tracks[armed_idx].name, start_bar + 1.0, take.duration_secs);
            }
        }
    } else {
        if !self.playlist_tracks.iter().any(|t| t.is_rec_armed) && !self.playlist_tracks.is_empty() {
            self.playlist_tracks[0].is_rec_armed = true;
        }
        let armed_idx = self.playlist_tracks.iter().position(|t| t.is_rec_armed).unwrap_or(0);
        let tempo = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0));
        self.timeline_rec_start_bar = tempo.bar_at_secs(self.song_time as f64) as f32;
        self.is_recording_timeline = true;
        if let Err(e) = self.vocal_studio.start_recording() {
            self.status_message = format!("❌ {}", e);
            self.is_recording_timeline = false;
            return;
        }
        self.is_playing = true;
        let _ = self.engine.send_command(AudioCommand::SetSongPlayback(true));
        self.status_message = crate::tstatus!("🔴 Spelar in direkt i spår '{}' från takt {:.1}...", self.playlist_tracks[armed_idx].name, self.timeline_rec_start_bar + 1.0);
    }
}
}

impl SonixApp {
pub fn toggle_playback(&mut self) {
    if self.is_recording_timeline {
        self.toggle_timeline_recording();
        return;
    }
    self.is_playing = !self.is_playing;
    if self.is_playing {
        ui_dbg(&format!("toggle_playback -> PLAY song_bar={} step={} pattern_mode={}", self.song_bar, self.song_step_in_bar, self.pattern_mode));
        for t in &mut self.playlist_tracks {
            t.automation_last = [f32::NAN; AutomationParam::COUNT];
        }
        self.sync_all_stems_to_engine();
        let song_secs = if self.pattern_mode {
            0.0
        } else {
            crate::audio::tempo::TempoMap::single(self.bpm.max(40.0)).secs_at_bar(
            self.song_bar as f64 + self.song_step_in_bar as f64 / 16.0,
        ) as f32
        };
        self.song_time = song_secs;
        // Stegklockan ankras i NU: en paus är inte tid som ska spelas in.
        self.last_step_time = Instant::now();
        let _ = self.engine.send_command(AudioCommand::SeekSongPosition(song_secs));
        let _ = self.engine.send_command(AudioCommand::SetSongPlayback(true));
    } else {
        ui_dbg("toggle_playback -> STOP (pause)");
        let _ = self.engine.send_command(AudioCommand::SetSongPlayback(false));
        let _ = self.engine.send_command(AudioCommand::StopAll);
        self.active_keys.clear();
        self.active_mouse_note = None;
    }
}
}

impl SonixApp {
pub fn stop_playback(&mut self) {
    if self.is_recording_timeline {
        self.toggle_timeline_recording();
        return;
    }
    ui_dbg("stop_playback");
    self.is_playing = false;
    self.current_step = 0;
    self.song_bar = 0;
    self.song_step_in_bar = 0;
    self.song_time = 0.0;
    self.last_step_time = Instant::now();
    let _ = self.engine.send_command(AudioCommand::SetSongPlayback(false));
    let _ = self.engine.send_command(AudioCommand::SeekSongPosition(0.0));
    let _ = self.engine.send_command(AudioCommand::StopAll);
    // Hjulet går tillbaka till mitten: en bend som stod mitt i en kurva ska inte färga
    // nästa not när man trycker på stopp.
    for channel in 0..=16u8 {
        let _ = self.engine.send_command(AudioCommand::ChannelPitchBend {
            channel,
            semitones: 0.0,
        });
    }
    self.active_keys.clear();
    self.active_mouse_note = None;
}
}

impl SonixApp {
#[allow(dead_code)]
pub fn seek_song_bar(&mut self, bar: usize) {
    self.song_bar = bar;
    self.song_step_in_bar = 0;
    self.song_time =
        crate::audio::tempo::TempoMap::single(self.bpm.max(40.0)).secs_at_bar(bar as f64)
            as f32;
    self.last_step_time = Instant::now();
    let _ = self.engine.send_command(AudioCommand::SeekSongPosition(self.song_time));
    self.status_message = crate::tstatus!("Flyttade markör till Takt {}", bar + 1);
}
}

impl SonixApp {
pub fn seek_song_time(&mut self, time_secs: f32) {
    let clamped = time_secs.max(0.0);
    self.song_time = clamped;
    let bar_float = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0))
        .bar_at_secs(clamped as f64) as f32;
    self.song_bar = bar_float.floor() as usize;
    let rem_bar = (bar_float - self.song_bar as f32).max(0.0);
    self.song_step_in_bar = ((rem_bar * 16.0).floor() as usize).min(15);
    let _ = self.engine.send_command(AudioCommand::SeekSongPosition(clamped));
    self.status_message = crate::tstatus!("Flyttade markör till {}", format_time_hundredths(clamped));
}
}

impl SonixApp {
pub fn sync_track_audio_state(&mut self, track_idx: usize) {
    if track_idx < self.playlist_tracks.len() {
        let t = &self.playlist_tracks[track_idx];
        let _ = self.engine.send_command(AudioCommand::SetStemTrackState {
            track_index: track_idx,
            volume: t.volume,
            pan: t.pan,
            muted: t.muted,
            solo: t.solo,
        });
        let _ = self.engine.send_command(AudioCommand::SetStemTrackRouting {
            track_index: track_idx,
            bus: t.bus,
            vca: t.vca,
        });
        let _ = self.engine.send_command(AudioCommand::SetStemTrackSidechain {
            track_index: track_idx,
            from: t.sidechain_from,
            amount_db: t.sidechain_amount_db,
            threshold_db: t.sidechain_threshold_db,
        });
        // Sends (Fas 8.13): samma ställe som sidokedjan, av samma skäl —
        // en omsynk efter ett spårbyte eller en ångring ska höras.
        let _ = self.engine.send_command(AudioCommand::SetStemTrackSends {
            track_index: track_idx,
            sends: t.sends.clone(),
        });
        let _ = self.engine.send_command(AudioCommand::SetTrackEq {
            track_index: track_idx,
            settings: t.eq.to_settings(),
        });
        let _ = self.engine.send_command(AudioCommand::SetCueSend {
            track_index: track_idx,
            a: t.cue_a,
            b: t.cue_b,
        });
        let _ = self.engine.send_command(AudioCommand::SetTrackMix {
            track_index: track_idx,
            comp_threshold_db: t.comp_threshold_db,
            comp_ratio: t.comp_ratio,
            reverb_send: t.reverb_send,
            delay_send: t.delay_send,
            pitch_semitones: t.pitch_semitones,
        });
    }
}
}

impl SonixApp {
/// Pushes the sub-mix bus and VCA group state (gain/mute/solo) to the audio
/// engine (Fas 5.2). Cheap; safe to call every UI frame or on any change.
pub fn sync_group_state(&mut self) {
    for bus in 0..self.bus_volume.len() {
        let _ = self.engine.send_command(AudioCommand::SetBusState {
            bus,
            volume: self.bus_volume[bus],
            muted: self.bus_muted[bus],
            solo: self.bus_solo[bus],
        });
    }
    for vca in 0..self.vca_faders.len() {
        let _ = self.engine.send_command(AudioCommand::SetVcaState {
            vca,
            volume: self.vca_faders[vca],
            muted: self.vca_muted[vca],
            solo: self.vca_solos[vca],
        });
    }
}
}

impl SonixApp {
fn step_duration(&self, step: usize) -> std::time::Duration {
    // Stegets längd kommer från tempokartan (Fas 8.2, steg 1). Med ett enda
    // tempo är talet exakt detsamma som förut — det är bevisat i
    // `audio::tempo`-testerna — så klockan är oförändrad tills kartan får
    // fler punkter.
    let map = crate::audio::tempo::TempoMap::single(self.bpm);
    let base_seconds = map.secs_per_step_at(self.song_bar as f64) as f32;
    let swing_factor = if step % 2 == 1 {
        1.0 + self.swing * 0.35
    } else {
        1.0 - self.swing * 0.35
    };
    std::time::Duration::from_secs_f32(base_seconds * swing_factor)
}
}

impl SonixApp {
pub fn get_max_project_bars(&self) -> usize {
    let max_region_end = self.playlist_tracks.iter()
        .flat_map(|t| t.regions.iter())
        .map(|r| r.start_bar + r.length_bars)
        .fold(0.0_f32, |m, b| m.max(b));
    (max_region_end.ceil() as usize).max(self.loop_end_bar).max(32)
}
}

impl SonixApp {
pub(crate) fn advance_sequencer(&mut self) {
    if !self.is_playing {
        return;
    }

    // Stegklockan räknas från den förra DEADLINE, ett steg i taget (Fas 8.13b).
    //
    // **Mätt 2026-09-13 (Alex: "markören står där ljudet börjar, men vågformen
    // visar 0,7 s kvar"):** på 12,64 s in i Rock and Hard Place låg spelhuvudet
    // **0,66 s efter** ljudet — 118 sextondelar × drygt halva bildrutetiden.
    // Orsaken stod här: `last_step_time = Instant::now()` flyttade ankaret till NU,
    // alltså upp till en bildruta för SENT varje gång, och felet summerades. Se
    // `steps_elapsed` — regeln den bär prövas i `mod tests` längst ned.
    let mut steps_taken = 0usize;
    loop {
        let cur = if self.pattern_mode { self.current_step } else { self.song_step_in_bar };
        let dur = self.step_duration(cur);
        if steps_elapsed(self.last_step_time, dur, Instant::now()) == 0 {
            break;
        }
        self.last_step_time += dur;
        steps_taken += 1;
        self.song_time += dur.as_secs_f32();

        if self.pattern_mode {
            self.current_step = crate::ui::app::state::next_pattern_step(self.current_step, self.pattern_len());
            // **Follow action:** en takt är 16 steg, så varvet vid 0 är taktgränsen. Åtgärden
            // prövas en gång per takt — inte per steg — och bara när det finns en.
            if self.current_step == 0 {
                self.follow_bars += 1.0;
                if let Some(action) = self.follow {
                    if action.due(self.follow_bars) {
                        // Slumpen dras ur klockan, inte ur ett sparat frö: en follow action ska
                        // kunna gå olika vägar mellan två spelningar.
                        // Slumpen dras ur klockan, inte ur ett sparat frö: en follow action ska
                        // kunna gå olika vägar mellan två spelningar.
                        let roll = crate::rng::Rng::new(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.subsec_nanos() as u64 | 1)
                                .unwrap_or(1),
                        )
                        .next_f32();
                        match action.apply(self.selected_pattern, self.patterns.len(), roll) {
                            crate::follow::FollowOutcome::Switch(next) => {
                                self.selected_pattern = next;
                                self.follow_bars = 0.0;
                                self.status_message = format!("Följd: slinga {}", next + 1);
                            }
                            crate::follow::FollowOutcome::Stop => {
                                self.is_playing = false;
                                self.follow_bars = 0.0;
                                let _ = self.engine.send_command(AudioCommand::SetSongPlayback(false));
                            }
                            crate::follow::FollowOutcome::Stay => {
                                self.follow_bars = 0.0;
                            }
                        }
                    }
                }
            }
            self.trigger_step(self.current_step);
        } else {
            self.song_step_in_bar += 1;
            if self.song_step_in_bar >= 16 {
                self.song_step_in_bar = 0;
                self.song_bar += 1;
                let max_bars = self.get_max_project_bars();
                // Sektionskedjan går före slingan: en arranger track är ett eget val av
                // ordning, och då ska markörerna styra, inte loop-punkterna.
                let chain_target = if self.arranger_chain_active && !self.arranger_chain.is_empty() {
                    let sections: Vec<(usize, usize)> = self
                        .song_markers
                        .iter()
                        .map(|m| (m.start_bar, m.length_bars))
                        .collect();
                    crate::ui::app::state::next_chain_bar(self.song_bar, &sections, &self.arranger_chain)
                } else {
                    None
                };
                if let Some(target) = chain_target {
                    self.song_bar = target;
                    let seek = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0))
                        .secs_at_bar(target as f64) as f32;
                    let _ = self.engine.send_command(AudioCommand::SeekSongPosition(seek));
                } else {
                    match crate::ui::app::state::after_bar_end(
                        self.song_bar,
                        self.loop_enabled,
                        self.loop_start_bar,
                        self.loop_end_bar,
                        max_bars,
                    ) {
                        crate::ui::app::state::BarAdvance::Continue => {}
                        crate::ui::app::state::BarAdvance::Wrap(target) => {
                            self.song_bar = target;
                            // Slingan ska gälla LJUDET också. Utan sökningen fortsätter
                            // motor-klockan framåt medan takträknaren hoppar tillbaka, och
                            // då går spelhuvud och ljud isär vid slingpunkten.
                            let back_to = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0))
                                .secs_at_bar(target as f64) as f32;
                            let _ = self.engine.send_command(AudioCommand::SeekSongPosition(back_to));
                        }
                        crate::ui::app::state::BarAdvance::Stop => {
                            // Låten är slut: stanna på plats, utan att nollställa.
                            self.song_bar = max_bars;
                            self.is_playing = false;
                            let _ = self.engine.send_command(AudioCommand::SetSongPlayback(false));
                            self.status_message =
                                crate::tstatus!("⏹ Låten slut (takt {})", max_bars);
                        }
                    }
                }
            }
            self.current_step = self.song_step_in_bar;
            self.trigger_song_step(self.song_bar, self.song_step_in_bar);
        }

        // Sustain any held MIDI notes across the new step while recording.
        if self.midi_record_armed && !self.midi_held_notes.is_empty() {
            let held: Vec<u8> = self.midi_held_notes.iter().copied().collect();
            for note in held {
                self.record_midi_note_at_step(note);
            }
        }

        if steps_taken >= Self::MAX_STEPS_PER_FRAME {
            // En lång paus (fönstret har stått still) ska inte avfyra femtio noter
            // på en bildruta. Hellre tappa steg än spränga låten — och ankaret
            // flyttas fram, så att vi inte hamnar i evig eftersläpning.
            self.last_step_time = Instant::now();
            break;
        }
    }

    // Spelhuvudet följer LJUDET (Fas 8.13b). Motorn räknar sina egna bildrutor,
    // och det är den klockan örat hör. I mönsterläget styr stegklockan själv.
    let audio_pos = self.engine.song_position_secs();
    if audio_pos > 0.0 && !self.pattern_mode {
        self.song_time = audio_pos;
    }
}
}

impl SonixApp {
fn trigger_step(&mut self, step: usize) {
    // Inspelade bend i det här steget, samplenoggrant fördröjda.
    let due = self
        .patterns
        .get(self.selected_pattern)
        .map(|p| crate::ui::app::state::bends_for_step(&p.bends, step, self.step_samples(step)))
        .unwrap_or_default();
    for (channel, semitones, delay_samples) in due {
        let _ = self.engine.send_command(AudioCommand::BendDelayed {
            channel,
            semitones,
            delay_samples,
        });
    }

    if self.metronome_enabled && step % 4 == 0 {
        if step == 0 {
            let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::MetronomeHigh));
        } else {
            let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::MetronomeLow));
        }
    }

    let has_solo = self.channels.iter().any(|c| c.solo);
    let vel = self.step_velocities[step];
    let grid_active = (0..24).any(|r| self.piano_roll_grid[r][step]);

    for (idx, ch) in self.channels.iter().enumerate() {
        let is_audible = if has_solo { ch.solo } else { !ch.muted };
        if ch.steps[step] && is_audible {
            let note = ch.notes[step];
            let hold = self.step_duration(step).as_secs_f32();
            if let Some(cmd) = channel_sample_trigger_command(ch, idx, note, vel, hold) {
                let _ = self.engine.send_command(cmd);
                continue;
            }
            match idx {
                0 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick)); }
                1 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Snare)); }
                2 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Clap)); }
                3 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed)); }
                4 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen)); }
                5 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Crash)); }
                6 => {
                    if !grid_active {
                        let freq = midi_to_freq(note);
                        let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: ch.volume * vel, channel: 0 });
                    }
                }
                7 => {
                    let freq = midi_to_freq(note);
                    let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: ch.volume * vel, channel: 0 });
                }
                _ => {}
            }
        }
    }

    if self.drummer_tom_steps[step] {
        let tom = if self.drummer_tom_high { DrumType::TomHigh } else { DrumType::TomLow };
        let _ = self.engine.send_command(AudioCommand::TriggerDrum(tom));
    }

    let chan6_audible = self.channels.get(6)
        .map(|c| if has_solo { c.solo } else { !c.muted })
        .unwrap_or(false);
    // Tagningen (Fas 6.4 steg 2) läses **före** grinden: en not som spelades
    // sent i ett steg har sin ruta på nästa steg (det är `floor(pos)` som är
    // steget den klingar från), och med grinden först skulle en sådan not
    // aldrig spelas — varken härifrån eller från sin egen ruta.
    let take_plan = match self.patterns.get(self.selected_pattern) {
        Some(pat) => {
            let grid = &self.piano_roll_grid;
            // Slumpen kommer ur klockan: en not med sannolikhet ska kunna falla olika mellan
            // två varv, annars är sannolikheten bara ett långsammare "alltid".
            let mut rng = crate::rng::Rng::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos() as u64 | 1)
                    .unwrap_or(1),
            );
            let take = pat.comped_take();
            crate::midi_take::plan_for_step_rolled(
                &take,
                step,
                self.step_samples(step),
                &|key, slot| (48..72).contains(&key) && grid[(key - 48) as usize][slot],
                &mut || rng.next_f32(),
            )
        }
        None => crate::midi_take::TakePlan::default(),
    };
    let grid_plays_here = grid_active || !take_plan.play.is_empty();
    if grid_plays_here && chan6_audible {
        for (note, delay, take_vel) in &take_plan.play {
            let freq = midi_to_freq(*note);
            let _ = self.engine.send_command(AudioCommand::NoteOnDelayed {
                note: *note,
                freq,
                velocity: vel * take_vel,
                delay_samples: *delay,
            });
        }
        for row in 0..24 {
            if self.piano_roll_grid[row][step] {
                let note = 48 + row as u8;
                if take_plan.skip.contains(&note) {
                    continue;
                }
                let freq = midi_to_freq(note);
                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: vel, channel: 0 });
            }
        }
    }
}
}

impl SonixApp {
fn trigger_song_step(&mut self, bar: usize, step_in_bar: usize) {
    if bar >= 32 || step_in_bar >= 16 {
        return;
    }

    // Bendkurvan hör till patternet, inte till spåret: skicka den **en** gång per steg,
    // även om fyra spår spelar samma block.
    if let Some((pat_idx, step_off)) = self
        .playlist_tracks
        .iter()
        .find_map(|t| crate::ui::app::state::clip_at_bar(t, bar))
        && let Some(pat) = self.patterns.get(pat_idx)
    {
        let at = step_off + step_in_bar;
        let due = crate::ui::app::state::bends_for_step(&pat.bends, at, self.step_samples(at));
        for (channel, semitones, delay_samples) in due {
            let _ = self.engine.send_command(AudioCommand::BendDelayed {
                channel,
                semitones,
                delay_samples,
            });
        }
    }

    if self.metronome_enabled && step_in_bar % 4 == 0 {
        if step_in_bar == 0 {
            let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::MetronomeHigh));
        } else {
            let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::MetronomeLow));
        }
    }
    let has_track_solo = self.playlist_tracks.iter().any(|t| t.solo);
    let vel = self.step_velocities[step_in_bar];

    for (_t_idx, track) in self.playlist_tracks.iter().enumerate() {
        let is_audible = if has_track_solo { track.solo } else { !track.muted };
        if !is_audible {
            continue;
        }
        // Ett fruset spår ligger färdigrenderat: ljudet strömmas av motorn
        // som ett stem-spår, och pattern-triggningen ska vara tyst — annars
        // hörs spåret två gånger. (Pattern-LÄGET rör kanalracket, inte
        // spåret, och påverkas därför inte.)
        if track.is_frozen() {
            continue;
        }

        if let Some((pat_idx, step_off)) = crate::ui::app::state::clip_at_bar(track, bar)
            && let Some(pat) = self.patterns.get(pat_idx) {
            // Blocket kan börja tidigare än den här takten: då spelar vi dess senare del.
            let sip = step_off + step_in_bar;
                match track.kind {
                    TrackKind::Drums => {
                        for ch_idx in 0..=5 {
                            if ch_idx < pat.channel_steps.len() && pat.channel_steps[ch_idx][sip] {
                                let note = pat.channel_notes.get(ch_idx).map(|n| n[sip]).unwrap_or(36);
                                let hold = self.step_duration(step_in_bar).as_secs_f32();
                                let sample_cmd = self.channels.get(ch_idx)
                                    .and_then(|ch| channel_sample_trigger_command(ch, ch_idx, note, track.volume * vel, hold));
                                if let Some(cmd) = sample_cmd {
                                    let _ = self.engine.send_command(cmd);
                                } else {
                                    match ch_idx {
                                        0 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Kick)); }
                                        1 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Snare)); }
                                        2 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Clap)); }
                                        3 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatClosed)); }
                                        4 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::HiHatOpen)); }
                                        5 => { let _ = self.engine.send_command(AudioCommand::TriggerDrum(DrumType::Crash)); }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        if self.drummer_tom_steps[step_in_bar] {
                            let tom = if self.drummer_tom_high { DrumType::TomHigh } else { DrumType::TomLow };
                            let _ = self.engine.send_command(AudioCommand::TriggerDrum(tom));
                        }
                    }
                    TrackKind::SynthLead | TrackKind::Bassline => {
                        let ch_idx = if track.kind == TrackKind::SynthLead { 6 } else { 7 };
                        // Tagningens mikro-tajming (Fas 6.4 steg 2), samma väg
                        // som i pattern-läget — och läses före grinden, annars
                        // tystnar en not som spelades sent i föregående steg.
                        let mut rng = crate::rng::Rng::new(
                            std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.subsec_nanos() as u64 | 1)
                                .unwrap_or(1),
                        );
                        // Kompningen kan gälla en viss takt i blocket: `sip / 16` är takten
                        // inne i patternet, `step_in_bar` är alltid 0–15 (tagningen är en takt).
                        let take = pat.comped_take_at(Some(sip / 16));
                        let take_plan = crate::midi_take::plan_for_step_rolled(
                            &take,
                            step_in_bar,
                            self.step_samples(step_in_bar),
                            &|key, slot| {
                                (48..72).contains(&key) && pat.piano_roll_grid[(key - 48) as usize][slot]
                            },
                            &mut || rng.next_f32(),
                        );
                        let grid_active = track.kind == TrackKind::SynthLead
                            && ((0..24).any(|r| pat.piano_roll_grid[r][sip])
                                || !take_plan.play.is_empty());
                        if grid_active {
                            for (note, delay, take_vel) in &take_plan.play {
                                let freq = midi_to_freq(*note);
                                let _ = self.engine.send_command(AudioCommand::NoteOnDelayed {
                                    note: *note,
                                    freq,
                                    velocity: track.volume * vel * take_vel,
                                    delay_samples: *delay,
                                });
                            }
                            for row in 0..24 {
                                if pat.piano_roll_grid[row][sip] {
                                    let note = 48 + row as u8;
                                    if take_plan.skip.contains(&note) {
                                        continue;
                                    }
                                    let freq = midi_to_freq(note);
                                    let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: track.volume * vel, channel: 0 });
                                }
                            }
                        } else if pat.channel_steps.len() > ch_idx && pat.channel_steps[ch_idx][sip] {
                            let note = pat.channel_notes[ch_idx][sip];
                            let hold = self.step_duration(step_in_bar).as_secs_f32();
                            let sample_cmd = self.channels.get(ch_idx)
                                .and_then(|ch| channel_sample_trigger_command(ch, ch_idx, note, track.volume * vel, hold));
                            if let Some(cmd) = sample_cmd {
                                let _ = self.engine.send_command(cmd);
                            } else {
                                let freq = midi_to_freq(note);
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: track.volume * vel, channel: 0 });
                            }
                        }
                    }
                    TrackKind::VocalAudio | TrackKind::CustomAudio | TrackKind::Fx => {}
                }
            }
    }
}
}

impl SonixApp {
pub(crate) fn play_note(&mut self, note: u8) {
    self.play_note_velocity(note, 0.85);
}
}

impl SonixApp {
fn play_note_velocity(&mut self, note: u8, velocity: f32) {
    self.play_note_velocity_on_channel(note, velocity, 0);
}
}

impl SonixApp {
/// `channel` 0 = tangentbord/UI, 1–16 = MIDI-kanal + 1 (MPE).
fn play_note_velocity_on_channel(&mut self, note: u8, velocity: f32, channel: u8) {
    // **Capture MIDI**: varje spelad not hamnar i bufferten, med tid sedan den första. Det är
    // samma väg som allt levande spel går genom, så ingen tangent kan smyga sig förbi.
    let elapsed = match self.capture_clock {
        Some(start) => start.elapsed().as_secs_f32(),
        None => {
            self.capture_clock = Some(std::time::Instant::now());
            0.0
        }
    };
    self.note_capture.push(elapsed, note, velocity);
    // Score-loggen har sin egen klocka: capture-bufferten töms vid sin dump, loggen ska
    // fortsätta räknas från sin egen första not.
    let logged_at = match self.score_clock {
        Some(start) => start.elapsed().as_secs_f32(),
        None => {
            self.score_clock = Some(std::time::Instant::now());
            0.0
        }
    };
    self.score_log.push(logged_at, note, velocity);
    if !self.active_keys.contains(&note) {
        self.active_keys.insert(note);
        let freq = midi_to_freq(note);
        let _ = self.engine.send_command(AudioCommand::NoteOn {
            note,
            freq,
            velocity: velocity.clamp(0.05, 1.0),
            channel,
        });
    }
}
}

impl SonixApp {
pub(crate) fn release_note(&mut self, note: u8) {
    self.release_note_on_channel(note, 0);
}
}

impl SonixApp {
pub(crate) fn release_note_on_channel(&mut self, note: u8, channel: u8) {
    self.active_keys.remove(&note);
    let _ = self.engine.send_command(AudioCommand::NoteOff { note, channel });
}
}

impl SonixApp {
/// Handles a real MIDI keyboard note event: plays it, and (when armed and
/// the transport is running) records it into the active Piano Roll pattern.
fn handle_midi_note(&mut self, note: u8, velocity: u8, on: bool, channel: u8) {
    // Kanal 0 i motorn betyder "inte från MIDI"; MIDI:s kanal 0 (mastern) blir 1.
    // Kartan läggs på **före** kanalen räknas om, och på både anslag och släpp — annars
    // släpps den transponerade noten aldrig, för note-off bär originaltangenten.
    let (mapped_note, mapped_velocity, mapped_channel) =
        self.midi_map.apply(note, velocity, channel + 1, self.song_key_root);
    let is_on = on && velocity > 0;
    if is_on {
        if self.midi_held_notes.insert(note) {
            self.play_note_velocity_on_channel(
                mapped_note,
                mapped_velocity as f32 / 127.0,
                mapped_channel,
            );
            if self.midi_record_armed && self.is_playing {
                self.record_midi_note_at_step(mapped_note);
                // Tagningen (Fas 6.4): noten skrivs till rutnätet *och* sparas
                // med sin faktiska tid, så att den går att kvantisera eller
                // humanisera i efterhand. Rutnätet ensamt kastade tiden.
                let pos = self.current_take_pos();
                if let Some(pat) = self.patterns.get_mut(self.selected_pattern) {
                    pat.active_take_mut()
                        .push(pos, mapped_note, mapped_velocity as f32 / 127.0);
                }
            }
        }
    } else if self.midi_held_notes.remove(&note) {
        self.release_note_on_channel(mapped_note, mapped_channel);
    }
}

/// MPE: mastern (kanal 0) böjer allt, medlemskanalerna bara sina egna noter.
///
/// Spelar man in sparas böjningen som en punkt i patternets kurva, så den hörs igen vid
/// uppspelning — och går att se och rätta i pianorullen.
fn handle_midi_pitch_bend(&mut self, channel: u8, semitones: f32) {
    let engine_channel = if channel == 0 { 0 } else { channel + 1 };
    let _ = self.engine.send_command(AudioCommand::ChannelPitchBend {
        channel: engine_channel,
        semitones,
    });
    if self.midi_record_armed && self.is_playing {
        let pos = self.current_pattern_pos();
        if let Some(pat) = self.patterns.get_mut(self.selected_pattern) {
            crate::ui::app::state::push_bend(
                &mut pat.bends,
                crate::ui::app::state::BendPoint {
                    pos,
                    channel: engine_channel,
                    semitones,
                },
            );
        }
    }
}
}

impl SonixApp {
/// Hur många samples ett steg är (Fas 6.4 steg 2). Tagningens mikro-tajming
/// räknas om till samples med motorns egen frekvens, eftersom kön i motorn
/// räknar ned per sample.
fn step_samples(&self, step: usize) -> u32 {
    let secs = self.step_duration(step).as_secs_f32();
    (secs * self.engine.sample_rate as f32).max(1.0) as u32
}
}

impl SonixApp {
/// Positionen i takten (i steg) för en not som spelas just nu (Fas 6.4).
///
/// Stegfasen mäts mot sekvenserns stegklocka, som går i UI-tråden:
/// upplösningen är därför en bildruta (≈16 ms vid 60 Hz), inte samplen.
fn current_pattern_pos(&self) -> f32 {
    let step_secs = self.step_duration(self.current_step).as_secs_f32();
    let phase = if step_secs > 0.0 {
        self.last_step_time.elapsed().as_secs_f32() / step_secs
    } else {
        0.0
    };
    (self.current_step as f32 + phase.clamp(0.0, 0.999)).clamp(0.0, 63.99)
}

fn current_take_pos(&self) -> f32 {
    let step_secs = self.step_duration(self.current_step).as_secs_f32();
    let phase = if step_secs > 0.0 {
        self.last_step_time.elapsed().as_secs_f32() / step_secs
    } else {
        0.0
    };
    crate::midi_take::take_pos_from(self.current_step, phase)
}
}

impl SonixApp {
/// Börjar en ny tagning i det valda patternet (Fas 6.4). Utan detta skulle
/// flera inspelningsförsök växa ihop till en enda tagning.
pub(crate) fn begin_new_take(&mut self) {
    let name = self
        .patterns
        .get(self.selected_pattern)
        .map(|p| p.name.clone())
        .unwrap_or_default();
    if let Some(pat) = self.patterns.get_mut(self.selected_pattern) {
        // **Take lanes**: varje nytt inspelningsförsök blir en egen lane i stället för att
        // skriva över det förra. Inspelningen går alltid till lane 1, och den gamla flyttas
        // till listan — då kan man lyssna på båda och välja.
        if !pat.take.is_empty() {
            pat.alt_takes.push(pat.take.clone());
        }
        pat.take = crate::midi_take::Take::new();
        pat.active_take = 0;
    }
    self.status_message = crate::tstatus!("🔴 Inspelning armar — ny tagning i '{}'", name);
}
}

impl SonixApp {
/// Kvantiserar tagningen i det valda patternet och *mäter* skillnaden
/// (Fas 6.4): "otajthet" är medelavståndet från noterna till rutnätet i steg,
/// så både före och efter går att skriva ut i siffror i stället för att
/// påstås.
pub(crate) fn quantize_take(&mut self, strength: f32, grid: crate::midi_take::TakeGrid) {
    // Ångringspunkten tas *före* ändringen (Fas 6.4), som för allt annat som
    // ändrar projektet.
    self.push_undo("🎯 Kvantisering av tagningen");
    let swing = self.swing;
    let Some(pat) = self.patterns.get_mut(self.selected_pattern) else {
        return;
    };
    if pat.active_take().is_empty() {
        self.status_message = crate::i18n::t(
            "⚠ Ingen tagning att kvantisera — armera ⏺ MIDI-REC och spela in först",
        )
        .to_string();
        return;
    }
    let before = pat.active_take().tightness();
    let notes = pat.active_take().len();
    pat.active_take_mut().quantize(strength, swing, grid);
    let after = pat.active_take().tightness();
    self.status_message = crate::tstatus!(
        "🎯 Kvantiserade {} noter ({}, styrka {:.0} %, sväng {:.0} %): {:.2} → {:.2} steg otajt",
        notes,
        grid.label(),
        strength * 100.0,
        swing * 100.0,
        before,
        after
    );
}
}

/// Fraserna i en mapp: `(namn utan ändelse, sökväg)`, sorterade skiftlägesokänsligt.
///
/// Ren funktion så att filtret kan prövas utan en filväljare: bara `.mid`/`.midi`,
/// skiftlägesokänsligt — samma regel som MIDI-importen.
pub(crate) fn phrase_files(dir: &std::path::Path) -> Vec<(String, std::path::PathBuf)> {
    let mut phrases = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return phrases;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let is_midi = path.extension().is_some_and(|e| {
            e.eq_ignore_ascii_case("mid")
                || e.eq_ignore_ascii_case("midi")
                || e.eq_ignore_ascii_case("fsc")
        });
        if !is_midi {
            continue;
        }
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        phrases.push((name, path));
    }
    phrases.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    phrases
}

impl SonixApp {
/// Lägger medveten mänsklig variation på tagningen (Fas 6.4). Slumptalet
/// räknas upp varje gång, så två tryck ger inte exakt samma tagning.
/// Läser en Scala-stämning (`.ascl`) och gör den aktiv.
///
/// Stämningen påverkar **alla** not-till-frekvens-vägar (uppspelning, export,
/// AI-generatorn), eftersom de går genom `audio::tuning`.
pub(crate) fn load_tuning_from_file(&mut self) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Stämning (.ascl)", &["ascl"])
        .set_title("Välj stämning")
        .pick_file()
    else {
        return;
    };
    let data = match std::fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            self.status_message = crate::tstatus!("⚠️ Kunde inte läsa '{}': {}", path.display(), e);
            return;
        }
    };
    match crate::audio::tuning::parse_ascl(&String::from_utf8_lossy(&data)) {
        Some(tuning) => {
            let steps = tuning.cents.len();
            let period = tuning.period_cents;
            let name = tuning.name.clone();
            crate::audio::tuning::set_tuning(Some(tuning));
            self.status_message = crate::tstatus!(
                "🎼 Stämning '{}': {} steg, period {:.2} cent",
                name,
                steps,
                period
            );
        }
        None => {
            self.status_message = crate::tstatus!(
                "⚠️ '{}' är ingen Scala-stämning vi kan läsa",
                path.display()
            );
        }
    }
}

/// Tillbaka till liksvävande tolvton.
pub(crate) fn reset_tuning(&mut self) {
    crate::audio::tuning::set_tuning(None);
    self.status_message = crate::i18n::t("🎼 Tillbaka till 12-TET").to_string();
}

/// Transponerar hela tagningen. Cubase har 31 Logical Editor-förval som gör detta;
/// vi hade ingen bulk-operation alls på en tagning.
pub(crate) fn transpose_take(&mut self, semitones: i32) {
    let Some(pat) = self.patterns.get(self.selected_pattern) else {
        return;
    };
    if pat.active_take().is_empty() {
        self.status_message = crate::i18n::t("⚠️ Ingen tagning att transponera").to_string();
        return;
    }
    self.push_undo(&format!("🎹 Transponera {semitones:+}"));
    let Some(pat) = self.patterns.get_mut(self.selected_pattern) else {
        return;
    };
    let moved = pat.active_take_mut().transpose(semitones);
    self.status_message = crate::tstatus!("🎹 {} not(er) {} halvtoner", moved, semitones);
}

/// Skalar anslaget på hela tagningen.
pub(crate) fn scale_take_velocity(&mut self, factor: f32) {
    let Some(pat) = self.patterns.get(self.selected_pattern) else {
        return;
    };
    if pat.active_take().is_empty() {
        self.status_message = crate::i18n::t("⚠️ Ingen tagning att ändra anslaget på").to_string();
        return;
    }
    self.push_undo(&format!("🔉 Anslag ×{factor:.2}"));
    let Some(pat) = self.patterns.get_mut(self.selected_pattern) else {
        return;
    };
    let changed = pat.take.scale_velocity(factor);
    self.status_message = crate::tstatus!("🔉 Anslaget ändrat på {} not(er)", changed);
}

/// Lägger en groovemall (Abletons `.agr`) på tagningen.
///
/// Tagningen är det lager både uppspelningen och exporten läser, så svänget hörs och hamnar i
/// filen på samma sätt — det finns ingen andra väg att hålla i takt.
/// **Extrahera en groove ur spårets ljud** (FL:s "Extrahera groove").
///
/// Anslagen i spåret jämförs med det raka rutnätet vid projektets tempo; skillnaden blir en
/// mall i poolen, precis som en läst `.agr`. Ingen ny detektor: `detect_onsets` är samma
/// funktion som slicing och transientläget använder, så extraktionen hör vad de hör.
pub(crate) fn extract_groove_from_track(&mut self) {
    let idx = self.selected_timeline_track.min(self.playlist_tracks.len().saturating_sub(1));
    let Some(track) = self.playlist_tracks.get(idx) else {
        self.status_message = crate::i18n::t("⚠️ Välj ett spår först").to_string();
        return;
    };
    // Fruset ljud ligger i `frozen_pcm`; annars är det spårets egen tagning.
    let audio = track
        .frozen_pcm
        .clone()
        .or_else(|| track.pcm_audio.clone());
    let Some((left, _right, sample_rate)) = audio else {
        self.status_message = crate::tstatus!(
            "🎵 '{}' har inget ljud att lyssna på — extrahera ur en tagning i stället",
            track.name
        );
        return;
    };
    let onsets = crate::audio::onset::detect_onsets(
        &left,
        sample_rate as f32,
        &crate::audio::onset::OnsetParams::default(),
    );
    let secs: Vec<f32> = onsets
        .iter()
        .map(|&o| o as f32 / sample_rate as f32)
        .collect();
    match crate::groove::extract_groove(&format!("Ur '{}'", track.name), &secs, self.bpm) {
        Some(groove) => {
            let depth = groove.depth();
            let count = onsets.len();
            self.groove_pool
                .push(crate::groove::GrooveEntry::new(groove, 1.0));
            self.status_message = crate::tstatus!(
                "🎵 Extraherade en groove ur '{}': {} anslag, djup {:.1} % av ett steg",
                track.name,
                count,
                depth * 100.0
            );
        }
        None => {
            self.status_message = crate::tstatus!(
                "🎵 '{}' svänger inte — anslagen ligger på rutnätet, så det finns ingen groove att ta",
                track.name
            );
        }
    }
}

pub(crate) fn apply_groove_from_file(&mut self) {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Groove (.agr)", &["agr"])
        .set_title("Välj groovemall")
        .pick_file()
    else {
        return;
    };
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            self.status_message = crate::tstatus!("⚠️ Kunde inte läsa '{}': {}", path.display(), e);
            return;
        }
    };
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    let Some(groove) = crate::groove::parse_agr_bytes(&bytes, &name) else {
        self.status_message = if crate::groove::agr_looks_binary(&bytes) {
            // Binära mallar läses sedan 2026-10-22; faller den här är det filen, inte formatet.
            crate::tstatus!("⚠️ '{}' är binär men innehåller ingen notlista att läsa", name)
        } else {
            crate::tstatus!("⚠️ '{}' innehåller inga noter — ingen groove att läsa", name)
        };
        return;
    };
    let amount = self.take_groove_amount.clamp(0.0, 1.0);
    // Mallen läggs i **poolen** i stället för att läggas på direkt: en pool är flera
    // mallar, och då måste inläsning och påläggning vara två steg.
    let depth = groove.depth();
    self.groove_pool.push(crate::groove::GrooveEntry::new(groove, amount));
    self.status_message = crate::tstatus!(
        "🎵 '{}' i poolen ({} mallar, djup {:.1} %) — tryck ▶ för att lägga på",
        name,
        self.groove_pool.len(),
        depth * 100.0
    );
}

/// Läser alla `.agr` i en mapp till biblioteket — en gång per mapp, inte per mall.
/// Poolen fylls sedan genom att välja ur biblioteket; 220 rader i verktygsraden vore
/// ingen meny.
pub(crate) fn load_groove_library(&mut self) {
    let Some(dir) = rfd::FileDialog::new()
        .set_title(crate::i18n::t("Välj mapp med .agr-mallar"))
        .pick_folder()
    else {
        return;
    };
    self.groove_folder = Some(dir.clone());
    self.read_groove_folder(&dir);
}

/// Läser om **samma** mapp — så att en mall som lagts dit medan pianorullen var öppen syns
/// utan att man behöver välja mappen igen.
pub(crate) fn rescan_groove_library(&mut self) {
    match self.groove_folder.clone() {
        Some(dir) => self.read_groove_folder(&dir),
        None => self.load_groove_library(),
    }
}

fn read_groove_folder(&mut self, dir: &std::path::Path) {
    let library = crate::groove::GrooveLibrary::load_from_dir(dir);
    let found = library.grooves.len();
    self.groove_library = library;
    self.status_message = if found == 0 {
        crate::tstatus!(
            "⚠️ Inga .agr-mallar i '{}'",
            dir.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| dir.display().to_string())
        )
    } else {
        crate::tstatus!("📚 {} mallar inlästa", found)
    };
}

/// Frasbanken (FL): läser en mapp med `.mid`-fraser till listan. Filtreringen är
/// densamma som importen använder — bara .mid/.midi, skiftlägesokänsligt.
pub(crate) fn load_phrase_bank(&mut self) {
    let Some(dir) = rfd::FileDialog::new()
        .set_title(crate::i18n::t("Välj mapp med .mid- eller .fsc-fraser"))
        .pick_folder()
    else {
        return;
    };
    let phrases = phrase_files(&dir);
    let found = phrases.len();
    self.phrases = phrases;
    self.phrase_folder = Some(dir);
    self.show_phrase_bank = true;
    self.status_message = if found == 0 {
        crate::i18n::t("⚠ Inga .mid- eller .fsc-fraser i mappen").to_string()
    } else {
        crate::tstatus!("🎼 {} fraser", found)
    };
}

/// Läser en fras (`.fsc` eller `.mid`) till noter för lyssning, normaliserade till 480 PPQ.
pub(crate) fn preview_notes_from_file(
    path: &std::path::Path,
) -> Result<Vec<crate::audio::phrase_preview::PreviewNote>, String> {
    use crate::audio::phrase_preview::PreviewNote;
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("fsc"))
    {
        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let score = crate::audio::fsc::parse_fsc(&bytes)?;
        let notes = crate::audio::fsc::to_midi_notes(&score, crate::audio::smf::PPQ);
        return Ok(notes
            .into_iter()
            .map(|n| PreviewNote {
                start: n.start,
                length: n.length,
                key: n.key,
                velocity: n.velocity as f32 / 127.0,
            })
            .collect());
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let parsed = crate::audio::smf::parse_midi(&bytes)?;
    // Filens egen PPQ kan vara 96 eller 960; lyssningen räknar i 480 som resten av appen.
    let scale = |tick: u32| (tick as u64 * crate::audio::smf::PPQ as u64 / parsed.ppq.max(1) as u64) as u32;
    Ok(parsed
        .notes_with_track()
        .into_iter()
        .map(|(_, n)| PreviewNote {
            start: scale(n.start),
            length: scale(n.length).max(1),
            key: n.key,
            velocity: n.velocity as f32 / 127.0,
        })
        .collect())
}

/// Spelar frasen en gång — infogar ingenting.
    pub(crate) fn preview_phrase(&mut self, path: &std::path::Path) {
        self.stop_phrase_preview();
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        match Self::preview_notes_from_file(path) {
            Ok(notes) if !notes.is_empty() => {
                let count = notes.len();
                self.phrase_preview = Some(crate::audio::phrase_preview::PhrasePreview::new(notes));
                self.status_message = crate::tstatus!("▶ '{}' ({} noter)", name, count);
            }
            Ok(_) => {
                self.status_message = crate::tstatus!("⚠ '{}' innehöll inga noter", name);
            }
            Err(e) => {
                self.status_message = crate::tstatus!("⚠ Kunde inte läsa '{}': {}", name, e);
            }
        }
    }

    /// Spelar en demorytm med mallen i poolen — så att man hör den innan den läggs på.
    pub(crate) fn preview_groove(&mut self, index: usize) {
        let Some(entry) = self.groove_pool.get(index) else {
            return;
        };
        let name = entry.name.clone();
        let notes: Vec<crate::audio::phrase_preview::PreviewNote> = entry
            .groove
            .demo_notes(entry.amount)
            .into_iter()
            .map(|(start, length, key, velocity)| {
                crate::audio::phrase_preview::PreviewNote {
                    start,
                    length,
                    key,
                    velocity,
                }
            })
            .collect();
        self.stop_phrase_preview();
        let count = notes.len();
        self.phrase_preview = Some(crate::audio::phrase_preview::PhrasePreview::new(notes));
        self.status_message = crate::tstatus!("▶ mall '{}' ({} noter)", name, count);
    }

/// Tystar lyssningen och släpper allt som klingar.
    pub(crate) fn stop_phrase_preview(&mut self) {
        if let Some(preview) = self.phrase_preview.take() {
            for note in preview.sounding() {
                self.release_note(note);
            }
        }
    }

    /// En bildruta av lyssningen: noter som ska tändas eller släckas just nu.
    ///
    /// Tiden räknas från `Instant` och omvandlas till frasens tick med projektets tempo —
    /// samma tick som filen skrevs i, alltså hörs frasen i det tempo den infogas i.
    pub(crate) fn advance_phrase_preview(&mut self) {
        let bpm = self.bpm;
        let Some(preview) = self.phrase_preview.as_mut() else {
            return;
        };
        let tick =
            (preview.elapsed_ms() * bpm * crate::audio::smf::PPQ as f32 / 60_000.0) as u32;
        let step = preview.step(tick);
        let finished = preview.finished(tick);
        for note in step.stop {
            self.release_note(note);
        }
        for (note, velocity) in step.start {
            self.play_note_velocity(note, velocity);
        }
        if finished {
            self.phrase_preview = None;
        }
    }

/// Infogar en fras i det valda patternet. En `.fsc` (FL:s egen frasbank) läses av
/// `audio::fsc` och går samma väg som MIDI-noterna; allt annat läses som `.mid`.
pub(crate) fn insert_phrase(&mut self, path: &std::path::Path) {
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| path.display().to_string());
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("fsc"))
    {
        match std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| crate::audio::fsc::parse_fsc(&bytes))
        {
            Ok(score) => {
                let ppq = crate::audio::smf::PPQ;
                let notes = crate::audio::fsc::to_midi_notes(&score, ppq);
                let report = self.place_notes_into_patterns(&notes, ppq);
                self.status_message = crate::tstatus!(
                    "🎼 '{}' infogad ({} noter)",
                    name,
                    report.notes_placed
                );
            }
            Err(e) => {
                self.status_message =
                    crate::tstatus!("⚠ Kunde inte läsa frasen '{}': {}", name, e);
            }
        }
        return;
    }
    let path = path.to_string_lossy().to_string();
    match self.import_midi_into_selected_pattern(&path) {
        Ok(report) => {
            self.status_message =
                crate::tstatus!("🎼 '{}' infogad ({} noter)", name, report.notes_placed);
        }
        Err(e) => {
            self.status_message =
                crate::tstatus!("⚠ Kunde inte läsa frasen '{}': {}", name, e);
        }
    }
}

/// Lägger hela poolen på tagningen — alla aktiva mallar, i listans ordning.
pub(crate) fn apply_groove_pool_to_take(&mut self) {
    let active = self.groove_pool.iter().filter(|e| e.enabled).count();
    if active == 0 {
        self.status_message = crate::i18n::t("🎵 Poolen är tom").to_string();
        return;
    }
    if self.patterns.get(self.selected_pattern).is_none_or(|p| p.take.is_empty()) {
        self.status_message =
            crate::i18n::t("⚠️ Ingen tagning att lägga grooven på — spela in först").to_string();
        return;
    }
    self.push_undo("🎵 Groove-pool");
    let pool = self.groove_pool.clone();
    let Some(pat) = self.patterns.get_mut(self.selected_pattern) else {
        return;
    };
    let moved = pat.take.apply_groove_pool(&pool);
    self.status_message =
        crate::tstatus!("🎵 {} mall(ar) lagda på {} not(er)", active, moved);
}

pub(crate) fn humanize_take(&mut self, amount: f32) {
    self.push_undo("🌀 Humanisering av tagningen");
    let amount = amount.clamp(0.0, 1.0);
    let timing_steps = 0.15 * amount;
    let velocity_amount = 0.3 * amount;
    self.take_seed_counter = self.take_seed_counter.wrapping_add(1);
    let seed = self
        .take_seed_counter
        .wrapping_mul(0x9E37_79B9_7F4A_7C15)
        .wrapping_add(0x1234_5678);
    let Some(pat) = self.patterns.get_mut(self.selected_pattern) else {
        return;
    };
    if pat.active_take().is_empty() {
        self.status_message = crate::i18n::t(
            "⚠ Ingen tagning att humanisera — armera ⏺ MIDI-REC och spela in först",
        )
        .to_string();
        return;
    }
    let before = pat.take.tightness();
    let notes = pat.take.len();
    pat.take.humanize(timing_steps, velocity_amount, seed);
    let after = pat.active_take().tightness();
    self.status_message = crate::tstatus!(
        "🌀 Humaniserade {} noter (mängd {:.0} %: tid ±{:.2} steg, anslag ±{:.0} %): {:.2} → {:.2} steg otajt",
        notes,
        amount * 100.0,
        timing_steps,
        velocity_amount * 100.0,
        before,
        after
    );
}
}

impl SonixApp {
/// Kör pianorullsskriptet i textfältet mot vald tagning.
pub(crate) fn run_piano_script(&mut self) {
    let text = self.script_text.clone();
    if text.trim().is_empty() {
        self.status_message =
            crate::i18n::t("Skriv ett skript först — t.ex. 'transpose 12'").to_string();
        return;
    }
    self.push_undo("📜 Pianorullsskript");
    self.take_seed_counter = self.take_seed_counter.wrapping_add(1);
    let seed = self.take_seed_counter;
    let Some(pat) = self.patterns.get_mut(self.selected_pattern) else {
        return;
    };
    match crate::piano_script::run_script(&mut pat.take, &text, seed) {
        Ok(reports) => {
            let changed: usize = reports.iter().map(|r| r.changed).sum();
            let lines = reports.len();
            self.status_message = crate::tstatus!(
                "📜 Skriptet kördes: {} rader, {} noter ändrade",
                lines,
                changed
            );
        }
        Err(err) => {
            // Ett trasigt skript ska inte lämna tagningen halvändrad utan att säga det.
            self.status_message = crate::tstatus!("❌ {}", err);
        }
    }
}
}

impl SonixApp {
/// Strummar tagningens ackord: noter på samma slag sprids i tonhöjdsordning.
pub(crate) fn strum_take(&mut self, spread_steps: f32, down: bool) {
    self.push_undo("🪕 Strumning av tagningen");
    let Some(pat) = self.patterns.get_mut(self.selected_pattern) else {
        return;
    };
    if pat.active_take().is_empty() {
        self.status_message = crate::i18n::t(
            "⚠ Ingen tagning att strumma — armera ⏺ MIDI-REC och spela in först",
        )
        .to_string();
        return;
    }
    let moved = pat.take.strum(spread_steps, down);
    self.status_message = if moved == 0 {
        crate::i18n::t("Inget att strumma: inga noter ligger på samma slag").to_string()
    } else {
        crate::tstatus!(
            "🪕 Strummade {} noter med {:.2} steg mellan varje ({})",
            moved,
            spread_steps,
            if down { "ner" } else { "upp" }
        )
    };
}
}

impl SonixApp {
/// Längden på det spelande patternet i steg: 1–4 takter × 16.
fn pattern_len(&self) -> usize {
    crate::ui::app::state::pattern_steps(
        self.patterns.get(self.selected_pattern).map_or(1, |p| p.bars),
    )
}

/// Writes a held MIDI note into the Piano Roll grid at the current step.
fn record_midi_note_at_step(&mut self, note: u8) {
    const BASE_MIDI: u8 = 48;
    const ROWS: usize = 24;
    if let Some(off) = crate::audio::note_to_roll_offset(note, BASE_MIDI, ROWS) {
        let step = self.current_step.min(self.pattern_len() - 1);
        self.piano_roll_grid[off][step] = true;
        self.channels[6].steps[step] = true;
        self.channels[6].notes[step] = BASE_MIDI + off as u8;
        self.sync_active_pattern_from_ui();
    }
}
}

impl SonixApp {
/// Drains real MCU/OSC events and refreshes connection status.
pub(crate) fn poll_hardware_control(&mut self) {
    while let Ok(ev) = self.control_rx.try_recv() {
        if self.midi_learn_active {
            self.midi_learn_active = false;
            self.status_message = crate::tstatus!("🎯 MIDI Learn fångade kontroll: {}", format!("{:?}", ev));
        }
        self.apply_control_event(ev);
        // En hårdvarukontroll (MCU/OSC) ändrar mixern utan pekare. Den ska
        // ändå ge en ångringspunkt (Fas 6.2), annars vore en faderändring
        // från en kontrollyta omöjlig att ångra.
        self.mixer_control_event = true;
    }
    if let Some(server) = &self.osc_server {
        self.osc_rx_count = server.received();
    }
    if let Some(mcu) = &self.mcu_input {
        self.mcu_connected = true;
        let devices = mcu.device_list();
        self.mcu_device_name = if devices.is_empty() {
            crate::i18n::t("Sonix MCU In (väntar – anslut enhet med aconnect)").to_string()
        } else {
            devices.join(", ")
        };
    } else {
        self.mcu_connected = false;
    }
    if let Some(midi) = &self.midi_input {
        self.midi_keyboard_connected = true;
        self.midi_note_count = midi.received();
        let devices = midi.device_list();
        self.midi_device_name = if devices.is_empty() {
            crate::i18n::t("Ingen MIDI-klaviatur hittad (koppla in en)").to_string()
        } else {
            devices.join(", ")
        };
    } else {
        self.midi_keyboard_connected = false;
    }
}
}

impl SonixApp {
fn apply_control_event(&mut self, ev: ControlEvent) {
    match ev {
        ControlEvent::Play(v) => {
            if v != self.is_playing {
                self.toggle_playback();
            }
        }
        ControlEvent::Stop => {
            if self.is_playing {
                self.stop_playback();
            }
        }
        ControlEvent::Record(v) => {
            if v != self.is_recording_timeline {
                self.toggle_timeline_recording();
            }
        }
        ControlEvent::TrackVolume { track, value } => {
            if track < self.playlist_tracks.len() {
                self.playlist_tracks[track].volume = value.clamp(0.0, 1.25);
                self.sync_track_audio_state(track);
            }
        }
        ControlEvent::TrackMute { track, mute } => {
            if track < self.playlist_tracks.len() {
                self.playlist_tracks[track].muted = mute;
                self.sync_track_audio_state(track);
            }
        }
        ControlEvent::BusVolume { bus, value } => {
            if bus < self.bus_volume.len() {
                self.bus_volume[bus] = value.clamp(0.0, 1.25);
                self.sync_group_state();
            }
        }
        ControlEvent::VcaVolume { vca, value } => {
            if vca < self.vca_faders.len() {
                self.vca_faders[vca] = value.clamp(0.0, 1.25);
                self.sync_group_state();
            }
        }
        ControlEvent::MidiNote { note, velocity, on, channel } => {
            self.handle_midi_note(note, velocity, on, channel);
        }
        ControlEvent::PitchBend { channel, semitones } => {
            self.handle_midi_pitch_bend(channel, semitones);
        }
    }
}
}

impl SonixApp {
/// Push the modular patcher graph to the audio engine whenever it changes.
pub(crate) fn sync_patcher_graph(&mut self) {
    let spec = self.modular_graph.to_spec();
    if self.last_patcher_spec.as_ref() != Some(&spec) {
        self.last_patcher_spec = Some(spec.clone());
        let _ = self.engine.send_command(AudioCommand::SetPatcherGraph(spec));
    }
}
}

impl SonixApp {
/// Registers the microphone monitor ring with the engine (once, or after a
/// reconfigure) and pushes the live monitoring / auto-tune parameters into
/// the input callback. Cheap enough to call every UI frame.
pub(crate) fn sync_mic_monitoring(&mut self) {
    // Sprint 1, punkt 6: monitorns ring dräneras en sample per ut-sample, så den antar att
    // ingången och motorn går i samma tempo. Är de olika hålls monitorn av med besked i stället
    // för att höras i fel hastighet. (Resampling i monitorvägen är nästa steg, se SPRINT.md.)
    if let Some(input_rate) = self.vocal_studio.mic_capture.as_ref().map(|m| m.sample_rate)
        && let crate::audio::input_profile::RateVerdict::Mismatch { engine, input } =
            crate::audio::input_profile::rate_verdict(self.engine.sample_rate, input_rate)
    {
        self.vocal_studio.monitoring_on = false;
        if !self.rate_mismatch_reported {
            self.rate_mismatch_reported = true;
            self.status_message = crate::tstatus!(
                "⚠ ingången går i {} Hz och motorn i {} Hz — monitorn är av (annars hörs fel tempo)",
                input,
                engine
            );
        }
    } else {
        self.rate_mismatch_reported = false;
    }

    // Sprint 1, punkt 1: en inkopplad instrumentkabel ska användas utan att någon öppnar en
    // inställningsruta. En gång per appstart — därefter är det användarens val som gäller
    // ("🔄 Uppdatera enheter" i ljudmodalen tar nya tag).
    if !self.input_autodetect_done {
        self.input_autodetect_done = true;
        self.autodetect_instrument_input();
    }
    let ring = self
        .vocal_studio
        .mic_capture
        .as_ref()
        .map(|m| std::sync::Arc::clone(&m.monitor_ring));
    let needs_send = match (&ring, &self.monitor_ring_sent) {
        (Some(a), Some(b)) => !std::sync::Arc::ptr_eq(a, b),
        (Some(_), None) => true,
        _ => false,
    };
    if needs_send && let Some(r) = ring {
        let _ = self.engine.send_command(AudioCommand::SetMonitorRing { ring: r.clone() });
        self.monitor_ring_sent = Some(r);
    }
    // Egen nivå för direktlyssningen: med den kan rundgång brytas utan att
    // sänka mastervolymen (som tidigare var enda reglaget).
    let level = self.vocal_studio.monitor_level.clamp(0.0, 1.0);
    if self.monitor_level_sent != Some(level) {
        let _ = self
            .engine
            .send_command(AudioCommand::SetMonitorLevel(level));
        self.monitor_level_sent = Some(level);
    }
    self.vocal_studio.sync_live_effects(
        self.vocal_studio.realtime_autotune,
        self.vocal_harmonizer.autotune_speed,
        self.vocal_harmonizer.autotune_speed,
        self.vocal_harmonizer.root_note,
        self.vocal_harmonizer.target_scale,
    );
}
}

