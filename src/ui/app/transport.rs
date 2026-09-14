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
        let mut mic_track = PlaylistTrack::new(crate::i18n::t("🎤 Mic (Voice & Sång)").to_string(), "🎤", TrackKind::VocalAudio, Color32::WHITE);
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
        let mut mic_track = PlaylistTrack::new(crate::i18n::t("🎤 Mic (Voice & Sång)").to_string(), "🎤", TrackKind::VocalAudio, Color32::WHITE);
        mic_track.volume = 1.0;
        mic_track.is_rec_armed = true;
        self.playlist_tracks.push(mic_track);
    }

    // 2. Re-classify and ensure colors are applied to every track and its regions
    let last_idx = self.playlist_tracks.len() - 1;
    for (idx, track) in self.playlist_tracks.iter_mut().enumerate() {
        let is_mic = idx == last_idx && (track.name.to_lowercase().contains("mic") || track.name.to_lowercase().contains("mik") || track.name.to_lowercase().contains("voice"));
        if is_mic {
            track.color = Color32::WHITE;
            track.icon = "🎤";
            track.kind = TrackKind::VocalAudio;
            for r in &mut track.regions {
                r.color = Color32::WHITE;
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
            self.current_step = (self.current_step + 1) % 16;
            self.trigger_step(self.current_step);
        } else {
            self.song_step_in_bar += 1;
            if self.song_step_in_bar >= 16 {
                self.song_step_in_bar = 0;
                self.song_bar += 1;
                let max_bars = self.get_max_project_bars();
                if self.song_bar >= self.loop_end_bar || self.song_bar >= max_bars {
                    self.song_bar = self.loop_start_bar;
                    // Slingan ska gälla LJUDET också. Utan sökningen fortsätter
                    // motor-klockan framåt medan takträknaren hoppar tillbaka, och
                    // då går spelhuvud och ljud isär vid slingpunkten.
                    let back_to = crate::audio::tempo::TempoMap::single(self.bpm.max(40.0))
                        .secs_at_bar(self.loop_start_bar as f64) as f32;
                    let _ = self.engine.send_command(AudioCommand::SeekSongPosition(back_to));
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
                        let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: ch.volume * vel });
                    }
                }
                7 => {
                    let freq = midi_to_freq(note);
                    let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: ch.volume * vel });
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
            crate::midi_take::plan_for_step(&pat.take, step, self.step_samples(step), &|key, slot| {
                (48..72).contains(&key) && grid[(key - 48) as usize][slot]
            })
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
                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: vel });
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

        if let Some(pat_idx) = track.clips[bar]
            && let Some(pat) = self.patterns.get(pat_idx) {
                match track.kind {
                    TrackKind::Drums => {
                        for ch_idx in 0..=5 {
                            if ch_idx < pat.channel_steps.len() && pat.channel_steps[ch_idx][step_in_bar] {
                                let note = pat.channel_notes.get(ch_idx).map(|n| n[step_in_bar]).unwrap_or(36);
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
                        let take_plan = crate::midi_take::plan_for_step(
                            &pat.take,
                            step_in_bar,
                            self.step_samples(step_in_bar),
                            &|key, slot| {
                                (48..72).contains(&key) && pat.piano_roll_grid[(key - 48) as usize][slot]
                            },
                        );
                        let grid_active = track.kind == TrackKind::SynthLead
                            && ((0..24).any(|r| pat.piano_roll_grid[r][step_in_bar])
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
                                if pat.piano_roll_grid[row][step_in_bar] {
                                    let note = 48 + row as u8;
                                    if take_plan.skip.contains(&note) {
                                        continue;
                                    }
                                    let freq = midi_to_freq(note);
                                    let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: track.volume * vel });
                                }
                            }
                        } else if pat.channel_steps.len() > ch_idx && pat.channel_steps[ch_idx][step_in_bar] {
                            let note = pat.channel_notes[ch_idx][step_in_bar];
                            let hold = self.step_duration(step_in_bar).as_secs_f32();
                            let sample_cmd = self.channels.get(ch_idx)
                                .and_then(|ch| channel_sample_trigger_command(ch, ch_idx, note, track.volume * vel, hold));
                            if let Some(cmd) = sample_cmd {
                                let _ = self.engine.send_command(cmd);
                            } else {
                                let freq = midi_to_freq(note);
                                let _ = self.engine.send_command(AudioCommand::NoteOn { note, freq, velocity: track.volume * vel });
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
    if !self.active_keys.contains(&note) {
        self.active_keys.insert(note);
        let freq = midi_to_freq(note);
        let _ = self.engine.send_command(AudioCommand::NoteOn {
            note,
            freq,
            velocity: velocity.clamp(0.05, 1.0),
        });
    }
}
}

impl SonixApp {
pub(crate) fn release_note(&mut self, note: u8) {
    self.active_keys.remove(&note);
    let _ = self.engine.send_command(AudioCommand::NoteOff { note });
}
}

impl SonixApp {
/// Handles a real MIDI keyboard note event: plays it, and (when armed and
/// the transport is running) records it into the active Piano Roll pattern.
fn handle_midi_note(&mut self, note: u8, velocity: u8, on: bool) {
    let is_on = on && velocity > 0;
    if is_on {
        if self.midi_held_notes.insert(note) {
            self.play_note_velocity(note, velocity as f32 / 127.0);
            if self.midi_record_armed && self.is_playing {
                self.record_midi_note_at_step(note);
                // Tagningen (Fas 6.4): noten skrivs till rutnätet *och* sparas
                // med sin faktiska tid, så att den går att kvantisera eller
                // humanisera i efterhand. Rutnätet ensamt kastade tiden.
                let pos = self.current_take_pos();
                if let Some(pat) = self.patterns.get_mut(self.selected_pattern) {
                    pat.take.push(pos, note, velocity as f32 / 127.0);
                }
            }
        }
    } else if self.midi_held_notes.remove(&note) {
        self.release_note(note);
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
        pat.take = crate::midi_take::Take::new();
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
    if pat.take.is_empty() {
        self.status_message = crate::i18n::t(
            "⚠ Ingen tagning att kvantisera — armera ⏺ MIDI-REC och spela in först",
        )
        .to_string();
        return;
    }
    let before = pat.take.tightness();
    let notes = pat.take.len();
    pat.take.quantize(strength, swing, grid);
    let after = pat.take.tightness();
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

impl SonixApp {
/// Lägger medveten mänsklig variation på tagningen (Fas 6.4). Slumptalet
/// räknas upp varje gång, så två tryck ger inte exakt samma tagning.
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
    if pat.take.is_empty() {
        self.status_message = crate::i18n::t(
            "⚠ Ingen tagning att humanisera — armera ⏺ MIDI-REC och spela in först",
        )
        .to_string();
        return;
    }
    let before = pat.take.tightness();
    let notes = pat.take.len();
    pat.take.humanize(timing_steps, velocity_amount, seed);
    let after = pat.take.tightness();
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
/// Writes a held MIDI note into the Piano Roll grid at the current step.
fn record_midi_note_at_step(&mut self, note: u8) {
    const BASE_MIDI: u8 = 48;
    const ROWS: usize = 24;
    if let Some(off) = crate::audio::note_to_roll_offset(note, BASE_MIDI, ROWS) {
        let step = self.current_step.min(15);
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
        ControlEvent::MidiNote { note, velocity, on } => {
            self.handle_midi_note(note, velocity, on);
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

