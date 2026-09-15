//! Kommandovägen: vad motorn gör när UI-tråden säger något — och ordningen spåren ska köras i.
//! Status: stabil — kommandovägen från UI-tråden in i motorn.
//! Rör inte: ordningen som `recompute_stem_order` räknar fram — mekanismen bakom sends mellan spår (8.3), och ett nej ska vara ett nej (slingan namnges).

use super::*;

impl SynthEngine {
    pub fn handle_command(&mut self, cmd: AudioCommand) {
        let significant = matches!(
            &cmd,

            AudioCommand::StopAll
                | AudioCommand::SetSongPlayback(_)
                | AudioCommand::SetMasterVolume(_)
                | AudioCommand::ClearAllStemTracks
                | AudioCommand::LoadStemTrack { .. }
                | AudioCommand::SetStemTrackState { .. }
                | AudioCommand::SetStemTrackRegions { .. }
                | AudioCommand::SetStemTrackRouting { .. }
                | AudioCommand::SetBusState { .. }
                | AudioCommand::SetVcaState { .. }
                | AudioCommand::SetTrackEq { .. }
                | AudioCommand::SetTrackMix { .. }
                | AudioCommand::SetRemixFx { .. }
                | AudioCommand::SetTapeStop { .. }
                | AudioCommand::SeekSongPosition(_)
        );
        if significant {
            // NOTE: never print `cmd` via {:?} here – LoadStemTrack contains
            // Arc<Vec<f32>> whose Debug dumps every PCM sample (multi-GB logs).
            self.dbg_state("ENGINE", &format!("<< {}", describe_cmd(&cmd)));
        }
        match cmd {
            AudioCommand::NoteOn { note, freq, velocity } => {
                let mut target_idx = None;
                for (i, voice) in self.voices.iter().enumerate() {
                    if voice.is_active() && voice.note == note {
                        target_idx = Some(i);
                        break;
                    }
                }
                if target_idx.is_none() {
                    for (i, voice) in self.voices.iter().enumerate() {
                        if !voice.is_active() {
                            target_idx = Some(i);
                            break;
                        }
                    }
                }
                let idx = target_idx.unwrap_or(0);
                self.voices[idx].trigger(note, freq, velocity, self.sample_rate);
            }
            AudioCommand::NoteOnDelayed { note, freq, velocity, delay_samples } => {
                // Samma kö som StrumChord. Fördröjning 0 betyder "nästa sample",
                // vilket i praktiken är direkt — så en not utan mikro-tajming
                // låter som förut.
                self.scheduled_notes.push(ScheduledNote {
                    samples_until: delay_samples,
                    note,
                    freq,
                    velocity,
                });
                if self.scheduled_notes.len() > 256 {
                    let overflow = self.scheduled_notes.len() - 256;
                    self.scheduled_notes.drain(0..overflow);
                }
            }
            AudioCommand::NoteOff { note } => {
                for voice in &mut self.voices {
                    if voice.is_active() && voice.note == note {
                        voice.release();
                    }
                }
            }
            AudioCommand::StrumChord { notes, velocity, start_samples, spread_samples, mode } => {
                let mut ordered = notes;
                match mode {
                    2 => ordered.reverse(),
                    3 => {
                        let n = ordered.len();
                        if n > 1 {
                            let mut seed = (self.song_time_samples as u32) ^ 0x9E37_79B9;
                            for i in (1..n).rev() {
                                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                                let j = (seed as usize) % (i + 1);
                                ordered.swap(i, j);
                            }
                        }
                    }
                    _ => {}
                }
                let step = if mode == 0 { 0 } else { spread_samples };
                for (i, note) in ordered.into_iter().enumerate() {
                    let freq = 440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0);
                    let delay = start_samples.saturating_add(step.saturating_mul(i as u32));
                    self.scheduled_notes.push(ScheduledNote {
                        samples_until: delay,
                        note,
                        freq,
                        velocity,
                    });
                }
                // Hard cap so a stuck stream of commands can never grow forever.
                if self.scheduled_notes.len() > 256 {
                    let overflow = self.scheduled_notes.len() - 256;
                    self.scheduled_notes.drain(0..overflow);
                }
            }
            AudioCommand::TriggerDrum(drum_type) => {
                let slot = match drum_type {
                    DrumType::Kick => 0,
                    DrumType::Snare => 1,
                    DrumType::Clap => 2,
                    DrumType::HiHatClosed => 3,
                    DrumType::HiHatOpen => 4,
                    DrumType::Crash => 5,
                    DrumType::TomLow => 6,
                    DrumType::TomHigh => 7,
                    DrumType::MetronomeHigh => 8,
                    DrumType::MetronomeLow => 9,
                };
                self.drums[slot].trigger(drum_type);
            }
            AudioCommand::SetWaveform(wf) => {
                self.waveform = wf;
            }
            AudioCommand::SetAdsr(adsr) => {
                self.adsr = adsr;
            }
            AudioCommand::SetFilter(flt) => {
                self.filter_params = flt;
            }
            AudioCommand::SetFilterEnv { amount, adsr } => {
                self.filter_env_amount = amount.clamp(-6.0, 6.0);
                self.filter_env = adsr;
            }
            AudioCommand::SetDelay(dly) => {
                self.delay_params = dly;
            }
            AudioCommand::SetReverb(rvb) => {
                self.reverb_params = rvb;
            }
            AudioCommand::SetDrive(drv) => {
                self.drive = drv.clamp(1.0, 10.0);
            }
            AudioCommand::SetMasterFx(params) => {
                self.master_fx.set_params(params);
            }
            AudioCommand::SetTrackEq { track_index, settings } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.eq = settings;
                    track.eq_proc.set_settings(settings);
                }
            }
            AudioCommand::SetTrackMix { track_index, comp_threshold_db, comp_ratio, reverb_send, delay_send, pitch_semitones } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.comp_threshold_db = comp_threshold_db;
                    track.comp_ratio = comp_ratio;
                    track.reverb_send = reverb_send.clamp(0.0, 1.0);
                    track.delay_send = delay_send.clamp(0.0, 1.0);
                    track.pitch_semitones = pitch_semitones.clamp(-24.0, 24.0);
                    track.pitch_shifter.set_ratio(2.0_f32.powf(track.pitch_semitones / 12.0));
                    track.pitch_active = track.pitch_semitones.abs() > 0.005;
                    track.pitch_shifter.set_active(track.pitch_active);
                }
            }
            AudioCommand::SetRemixFx { mode, bpm } => {
                self.remix_fx.set(mode, bpm);
            }
            AudioCommand::SetTapeStop { active } => {
                self.tape_stop.set_active(active);
            }
            AudioCommand::SetPatcherGraph(spec) => {
                self.patcher_spec = Some(spec.clone());
                self.patcher = Some(PatchProcessor::new(&spec, self.sample_rate));
            }
            AudioCommand::SetPatcherEnabled(enabled) => {
                self.patcher_enabled = enabled;
                if enabled && self.patcher.is_none() {
                    if let Some(spec) = &self.patcher_spec {
                        self.patcher = Some(PatchProcessor::new(spec, self.sample_rate));
                    }
                }
            }
            AudioCommand::PatcherNoteOn { freq, velocity } => {
                if let Some(p) = &mut self.patcher {
                    p.note_on(freq, velocity);
                }
            }
            AudioCommand::PatcherNoteOff => {
                if let Some(p) = &mut self.patcher {
                    p.note_off();
                }
            }
            AudioCommand::LoadPreset(preset) => {
                let (wf, adsr, flt) = preset.settings();
                self.waveform = wf;
                self.adsr = adsr;
                self.filter_params = flt;
                for voice in &mut self.voices {
                    voice.filter.reset();
                }
            }
            AudioCommand::SetMasterVolume(vol) => {
                self.master_volume = vol.clamp(0.0, 1.0);
            }
            AudioCommand::StopAll => {
                for voice in &mut self.voices {
                    voice.reset();
                }
                for drum in &mut self.drums {
                    drum.reset();
                }
                for sv in &mut self.sample_voices {
                    sv.active = false;
                }
                if let Some(ref mut aud) = self.audition {
                    aud.is_playing = false;
                }
                self.song_playing = false;
            }
            AudioCommand::SetTrackPlugin { track_index, insert } => {
                if insert.is_some() {
                    while self.stem_tracks.len() <= track_index {
                        self.stem_tracks.push(StemVoiceTrack::new(
                            Arc::new(Vec::new()),
                            Arc::new(Vec::new()),
                            self.sample_rate,
                            1.0,
                            0.0,
                            0.0,
                            self.sample_rate,
                        ));
                    }
                }
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.plugin = insert;
                    track.pdc.set_delay(0);
                }
            }
            AudioCommand::SetPluginParameter { track_index, param_id, value } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index)
                    && let Some(plugin) = &mut track.plugin
                {
                    plugin.set_parameter(param_id, value);
                }
            }
            // **Manuellt latens-offset** (Fas 8.6). Offsetet lagras på spåret och räknas in i
            // PDC:n i `process_stereo` — det ska alltså **inte** röra delay-linjen här:
            // nästa bildruta sätter den till (max − spårets latens), och en nollställning
            // här skulle bara kasta bort kön mitt i ett block.
            AudioCommand::SetPluginLatencyOffset { track_index, frames } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.manual_latency_frames = frames;
                }
            }
            // Smart disable (Fas 8.6). Ingen plugin på spåret = inget att slå på, och det är
            // inte ett fel: kommandot kommer från en vy som kan ha ritats en bildruta före
            // spårets insert.
            AudioCommand::SetPluginSmartDisable { track_index, enabled } => {
                if let Some(plugin) = self
                    .stem_tracks
                    .get_mut(track_index)
                    .and_then(|t| t.plugin.as_mut())
                {
                    plugin.set_smart_disable(enabled);
                }
            }
            AudioCommand::LoadStemTrack { track_index, left, right, sample_rate, volume, pan, start_time_secs } => {
                let track = StemVoiceTrack::new(left, right, sample_rate, volume, pan, start_time_secs, self.sample_rate);
                if track_index < self.stem_tracks.len() {
                    let eq = self.stem_tracks[track_index].eq;
                    let old = &mut self.stem_tracks[track_index];
                    let old_offset = old.manual_latency_frames;
                    let (ct, cr, rs, ds, ps, pa) = (old.comp_threshold_db, old.comp_ratio, old.reverb_send, old.delay_send, old.pitch_semitones, old.pitch_active);
                    // Preserve a live plugin insert (and its PDC line) across reloads.
                    let plugin = old.plugin.take();
                    let pdc = std::mem::replace(&mut old.pdc, PdcDelay::new());
                    let mut new_track = track;
                    new_track.eq = eq;
                    new_track.eq_proc.set_settings(eq);
                    new_track.comp_threshold_db = ct;
                    new_track.comp_ratio = cr;
                    new_track.reverb_send = rs;
                    new_track.delay_send = ds;
                    new_track.pitch_semitones = ps;
                    new_track.pitch_shifter.set_ratio(2.0_f32.powf(ps / 12.0));
                    new_track.pitch_shifter.set_active(pa);
                    new_track.pitch_active = pa;
                    new_track.plugin = plugin;
                    new_track.pdc = pdc;
                    // **Offsetet överlever omladdningen, av samma skäl som plugin och PDC.**
                    // `LoadStemTrack` skickas vid varje uppspelningsstart, så ett offset som
                    // inte följde med skulle nollas tyst varje gång man tryckte play — och
                    // bara höras som att spåret gled igen (Fas 8.6).
                    new_track.manual_latency_frames = old_offset;
                    // **Regionerna överlever en omladdning** (Fas 8.10c). De är state som
                    // eq, kompressor och plugin — och att tappa dem tyst var precis vad
                    // som gjorde en tempoändring ohörbar: appen skickade `LoadStemTrack`
                    // vid varje uppspelningsstart och klippen föll tillbaka till
                    // originalet medan vyn fortsatte rita den sträckta filen.
                    new_track.regions = std::mem::take(&mut old.regions);
                    self.stem_tracks[track_index] = new_track;
                } else {
                    while self.stem_tracks.len() < track_index {
                        self.stem_tracks.push(StemVoiceTrack::new(
                            Arc::new(Vec::new()),
                            Arc::new(Vec::new()),
                            self.sample_rate,
                            1.0,
                            0.0,
                            0.0,
                            self.sample_rate,
                        ));
                    }
                    self.stem_tracks.push(track);
                }
                self.has_stem_solo = self.stem_tracks.iter().any(|t| t.solo);
                self.recount_stretched_tracks();
                self.recompute_stem_order();
            }
            AudioCommand::ClearAllStemTracks => {
                self.stem_tracks.clear();
                self.has_stem_solo = false;
                self.recount_stretched_tracks();
                self.recompute_stem_order();
            }
            AudioCommand::SetStemTrackState { track_index, volume, pan, muted, solo } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.volume = volume;
                    track.set_pan(pan);
                    track.muted = muted;
                    track.solo = solo;
                    self.has_stem_solo = self.stem_tracks.iter().any(|t| t.solo);
                }
            }
            AudioCommand::SetStemTrackRegions { track_index, regions } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.regions = regions;
                }
                self.recount_stretched_tracks();
            }
            AudioCommand::SetStemTrackRouting { track_index, bus, vca } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.bus = bus.min(NUM_BUSES - 1);
                    track.vca = vca.filter(|&v| v < NUM_VCAS);
                }
            }
            AudioCommand::SetPluginExtraOutputs { track_index, targets } => {
                let n = self.stem_tracks.len();
                let mut dropped: Vec<String> = Vec::new();
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    track.extra_out_targets = targets
                        .into_iter()
                        .map(|t| match t {
                            // Ett mål som pekar på spåret självt vore en slinga, och ett mål
                            // utanför listan finns inte. Båda **släpps**, och sägs högt: en
                            // koppling som inte gick att göra ska inte se ut som en gjord.
                            Some(to) if to == track_index => {
                                dropped.push(format!(
                                    "spår {}: utbuss till spåret självt — släppt",
                                    to + 1
                                ));
                                None
                            }
                            Some(to) if to >= n => {
                                dropped.push(format!(
                                    "spår {}: utbuss till spår {} som inte finns — släppt",
                                    track_index + 1,
                                    to + 1
                                ));
                                None
                            }
                            other => other,
                        })
                        .collect();
                }
                for line in dropped {
                    dbg_log("plugin", &line);
                }
                // Ordningen och ingångskanterna byggs i samma pass, som för sends: en buss
                // som kommer en sample sent är samma fel som en send som kommer sent.
                self.recompute_stem_order();
            }
            AudioCommand::SetStemTrackSends { track_index, sends } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    // Kläms här, en gång: målet till en buss som finns, nivån till
                    // 0..2 (en send ska kunna vara starkare än spåret, men inte
                    // oändligt), och en send utan nivå är ingen send.
                    track.sends = sends
                        .into_iter()
                        .filter(|s| s.level.is_finite() && s.level.abs() > 1e-6)
                        .map(|s| StemSend {
                            // Bussmål klampas (samma regel som förut); ett spårmål behålls
                            // orört — spåret kan laddas senare, och att klampa till "sista
                            // spåret" vore att routa till fel spår i stället för inget.
                            target: match s.target {
                                SendTarget::Bus { target_bus } => {
                                    SendTarget::Bus { target_bus: target_bus.min(NUM_BUSES - 1) }
                                }
                                other => other,
                            },
                            level: s.level.clamp(0.0, 2.0),
                        })
                        .collect();
                    // Ordningen kan ha ändrats av den här senden (Fas 8.3).
                    self.recompute_stem_order();
                }
            }
            AudioCommand::SetStemTrackSidechain { track_index, from, amount_db, threshold_db } => {
                if let Some(track) = self.stem_tracks.get_mut(track_index) {
                    // Ett `from` som pekar på spåret självt är ingen sidokedja —
                    // det är en slinga, och den släpps inte in här. Ett `from`
                    // utanför listan kan bli giltigt senare (spår läggs till), så
                    // det sparas och filtreras vid användning.
                    track.sidechain_from = from.filter(|&k| k != track_index);
                    track.sidechain_amount_db = amount_db.clamp(0.0, 60.0);
                    track.sidechain_threshold_db = threshold_db.clamp(-80.0, 0.0);
                    track.sidechain_ducker.reset();
                }
            }
            AudioCommand::SetBusState { bus, volume, muted, solo } => {
                let b = bus.min(NUM_BUSES - 1);
                self.bus_volume[b] = volume.max(0.0);
                self.bus_muted[b] = muted;
                self.bus_solo[b] = solo;
            }
            AudioCommand::SetVcaState { vca, volume, muted, solo } => {
                let v = vca.min(NUM_VCAS - 1);
                self.vca_volume[v] = volume.max(0.0);
                self.vca_muted[v] = muted;
                self.vca_solo[v] = solo;
            }
            AudioCommand::SeekSongPosition(secs) => {
                self.song_time_samples = (secs.max(0.0) * self.sample_rate) as usize;
                for track in &mut self.stem_tracks {
                    track.pitch_shifter.reset();
                }
            }
            AudioCommand::SetSongPlayback(playing) => {
                self.song_playing = playing;
                for track in &mut self.stem_tracks {
                    track.pitch_shifter.reset();
                }
            }
            AudioCommand::PlayAudition {
                left,
                right,
                sample_rate,
                volume,
                pitch_ratio,
                time_stretch_ratio,
                is_reverse,
                loop_playback,
            } => {
                let start_pos = if is_reverse {
                    (left.len() as f32 - 1.0).max(0.0)
                } else {
                    0.0
                };
                self.audition = Some(AuditionVoice {
                    left,
                    right,
                    sample_rate,
                    volume: volume.max(0.0),
                    pitch_ratio: pitch_ratio.max(0.05),
                    time_stretch_ratio: time_stretch_ratio.max(0.05),
                    is_reverse,
                    loop_playback,
                    play_pos_samples: start_pos,
                    is_playing: true,
                    wsola: Wsola::new(sample_rate),
                });
            }
            AudioCommand::TriggerSampleVoice {
                left,
                right,
                sample_rate,
                base_note,
                note,
                pitch_semitones,
                pitch_cents,
                velocity,
                velocity_sensitivity,
                volume,
                reverse,
                start01,
                end01,
                channel,
                loop_mode,
                loop_start01,
                loop_end01,
                ping_pong,
                amp_env,
                filter,
                hold_secs,
            } => {
                if left.is_empty() {
                    return;
                }
                let len_f = left.len() as f32;
                let start_f = (start01.clamp(0.0, 1.0) * len_f).floor();
                let end_f = ((end01.clamp(0.0, 1.0) * len_f).floor()).max(start_f + 1.0);
                let semis = note as f32 - base_note as f32 + pitch_semitones as f32 + pitch_cents / 100.0;
                let ratio = (semis / 12.0).exp2().clamp(0.1, 8.0);
                let step = (sample_rate as f32 / self.sample_rate) * ratio;

                // Pick the oldest/last used voice slot so retriggers never block.
                let idx = self.sample_voice_cursor % MAX_SAMPLE_VOICES;
                self.sample_voice_cursor = (self.sample_voice_cursor + 1) % MAX_SAMPLE_VOICES;
                let v = &mut self.sample_voices[idx];
                v.left = left;
                v.right = right;
                v.src_sample_rate = sample_rate as f32;
                v.step = step.max(0.0001);
                v.reverse = reverse;
                v.start_frame = start_f;
                v.end_frame = end_f.min(len_f);
                if reverse {
                    v.pos = v.end_frame;
                } else {
                    v.pos = v.start_frame;
                }
                // Samplern (Fas 8.4): loopen kläms och ordnas av `loop_frames` (ett
                // bakvänt par är ingen loop — då spelar rösten som en en-skottsprovare),
                // notens längd räknas i **utramar** (det `frames_done` räknar), och
                // envelopen är avstängd när parametrarna är identiteten.
                v.channel = channel;
                v.loop_mode = loop_mode;
                v.loop_span = loop_frames(v.left.len(), loop_start01, loop_end01);
                v.ping_pong = ping_pong;
                v.dir = if reverse { -1.0 } else { 1.0 };
                v.released = false;
                v.hold_frames = if hold_secs.is_finite() && hold_secs > 0.0 {
                    Some((hold_secs * self.sample_rate).round() as u32)
                } else {
                    None
                };
                // **Filtret** (Fas 8.4/7): klämt **en gång** här, så samplevägen slipper göra
                // det per sample. Filtrets och envelopens tillstånd nollställs per röst — en
                // återanvänd röst får inte bära förra notens svans i filtret.
                v.filter = crate::audio::filter::SamplerFilter {
                    on: filter.on,
                    cutoff_hz: filter.cutoff_hz.clamp(20.0, 20_000.0),
                    resonance: filter.resonance.clamp(0.1, 10.0),
                    env_amount_octaves: filter.env_amount_octaves,
                    env: filter.env,
                };
                v.filter_state = crate::audio::filter::StateVariableFilter::new(self.sample_rate);
                v.filter_env = AdsrVoice::new(self.sample_rate);
                if filter.on {
                    v.filter_env.gate_on();
                }
                v.env_params = amp_env;
                v.env_on = !amp_env.is_identity();
                v.amp_env = AdsrVoice::new(self.sample_rate);
                if v.env_on {
                    v.amp_env.gate_on();
                }
                // **Anslaget flyttar ut ur volymen** (Fas 8.4/7): det blev en egen faktor på
                // rösten i stället för en rå multiplikation här. Med standardkänsligheten 1,0
                // är talet detsamma som förut — men nu går det att stänga av per kanal, och
                // envelopen (som ligger efter i samplevägen) skalas av anslaget.
                v.velocity_gain =
                    crate::audio::envelope::velocity_gain(velocity, velocity_sensitivity);
                v.volume = volume.clamp(0.0, 1.5);
                let p: f32 = 0.0; // pan handled on the channel strip in future
                v.pan_l = ((1.0 - p) * 0.5).sqrt();
                v.pan_r = ((1.0 + p) * 0.5).sqrt();
                // 2 ms, inte 1,5: samma ramp används nu vid **båda** kanterna, och 2 ms
                // ligger inom det intervall (1–5 ms) Reapers fade pad och Abeltons
                // per-slice-fade använder.
                v.fade_frames = ((self.sample_rate * SLICE_FADE_SECS) as u32).max(1);
                v.frames_done = 0;
                v.active = true;
            }

            AudioCommand::ReleaseSampleVoices { channel } => {
                // **Not-av** (Fas 8.4): bara rösterna på den kanalen. En röst som redan
                // släppts rörs inte (annars startade släppet om varje gång).
                for v in &mut self.sample_voices {
                    if v.channel == channel && !v.released {
                        v.released = true;
                        if v.env_on {
                            v.amp_env.gate_off();
                        }
                        if v.filter.on {
                            v.filter_env.gate_off();
                        }
                    }
                }
            }
            AudioCommand::StopAudition => {
                if let Some(ref mut aud) = self.audition {
                    aud.is_playing = false;
                }
            }
            AudioCommand::SetMonitorRing { ring } => {
                self.monitor_ring = Some(ring);
                self.monitor_buf.clear();
                self.monitor_idx = 0;
            }
            AudioCommand::SetMonitorLevel(level) => {
                self.monitor_level = level.clamp(0.0, 1.0);
            }
            AudioCommand::SetAuditionParams {
                volume,
                pitch_ratio,
                time_stretch_ratio,
            } => {
                if let Some(ref mut aud) = self.audition {
                    aud.volume = volume.max(0.0);
                    aud.pitch_ratio = pitch_ratio.max(0.05);
                    aud.time_stretch_ratio = time_stretch_ratio.max(0.05);
                }
            }
        }
        if significant {
            self.dbg_state("ENGINE", ">> handled");
        }
    }
    }

    impl SynthEngine {
    #[inline(always)]
    /// Räknar om [`Self::stretched_region_tracks`] ur spåren (Fas 8.10c).
    ///
    /// Ett tal som svarar på **vad motorn har**, inte på vad appen skickade. Anropas
    /// där regioner eller spår byts — några spår, några regioner, ingen kostnad i
    /// sample-loopen.
    /// Räknar om **spårordningen** (Fas 8.3). Anropas när sends eller spårlistan ändras —
    /// aldrig per sample.
    ///
    /// En slinga kan inte bli en ordning. Då behålls den förra (och är den ogiltig för den
    /// nya spårlistan, den naturliga ordningen), och det skrivs i loggen: motorn **gissar
    /// aldrig** på en routering — en tyst omsortering vore en gissning. Ett spår vars sändare
    /// räknas senare får sin send **en sample senare** i stället, vilket är den gamla
    /// ordningens kända beteende sedan tidigare (sidokedjan läser också förra samplet).
    fn recompute_stem_order(&mut self) {
        let n = self.stem_tracks.len();
        let graph: Vec<Vec<usize>> = self
            .stem_tracks
            .iter()
            .map(|t| {
                // Spår-sends (8.3) och pluginens egna utbussar (8.6) är samma sorts kant:
                // mottagaren måste räknas **efter** källan, annars kommer ljudet en sample
                // sent. En graf, ett ställe — då kan de inte driva isär.
                let mut edges: Vec<usize> = t.sends.iter().filter_map(|s| s.target.as_track()).collect();
                edges.extend(t.extra_out_targets.iter().flatten().copied());
                edges
            })
            .collect();
        match crate::audio::command::plan_track_order(n, &graph) {
            Ok(order) => self.stem_order = order,
            Err((a, b)) => {
                dbg_log(
                    "sends",
                    &format!(
                        "slinga mellan spår {} och {} — behåller förra ordningen",
                        a + 1,
                        b + 1
                    ),
                );
                let valid = self.stem_order.len() == n
                    && {
                        let mut seen = self.stem_order.clone();
                        seen.sort_unstable();
                        seen == (0..n).collect::<Vec<_>>()
                    };
                if !valid {
                    self.stem_order = (0..n).collect();
                }
            }
        }
        let mut incoming: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
        for (from, track) in self.stem_tracks.iter().enumerate() {
            for send in &track.sends {
                if let Some(to) = send.target.as_track() {
                    if to != from && to < n {
                        incoming[to].push((from, send.level));
                    }
                }
            }
        }
        self.stem_incoming = incoming;
        self.track_out_l.resize(n, 0.0);
        self.track_out_r.resize(n, 0.0);
        self.plugin_bus_in.resize(n, [0.0; 2]);
    }
    }

    impl SynthEngine {
    fn recount_stretched_tracks(&mut self) {
        self.stretched_region_tracks = self
            .stem_tracks
            .iter()
            .filter(|t| t.regions.iter().any(|r| r.source_audio.is_some()))
            .count() as u32;
    }
}
