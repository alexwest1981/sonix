//! Offline full-project rendering and audio export for Sonix Studio.
//!
//! The renderer replays the exact same trigger logic as live playback
//! (real Channel Rack WAV samples, drum voices, synth voices and timeline
//! stem regions through the same [`SynthEngine`] DSP), but fully offline
//! into a float32 stereo buffer that can be encoded to any format.
//!
//! Encoders:
//! - WAV (16/24-bit PCM, 32-bit float) written in-process with clean INFO tags.
//! - FLAC (24-bit lossless) encoded in-process with `flacenc` and tagged with
//!   a VORBIS_COMMENT metadata block (no AI/provider data ever added).
//! - MP3/OGG/AAC via `ffmpeg` when installed (hybrid mode).

use std::io::Write;
use std::process::Command;
use std::sync::Arc;

use super::command::{AudioCommand, StemRegionPlayback, StemSend};
use super::drum::DrumType;
use super::effects::{DelayParams, ReverbParams};
use super::envelope::AdsrParams;
use super::filter::FilterParams;
use super::master_fx::{MasterFxParams, TrackEqSettings};
use super::synth::SynthEngine;

// ---------------------------------------------------------------------------
// Playback snapshot types (pure data, no egui dependency)
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct VoiceSpec {
    pub left: Arc<Vec<f32>>,
    pub right: Arc<Vec<f32>>,
    pub sample_rate: u32,
    pub base_note: u8,
    pub semitones: i8,
    pub cents: f32,
    pub volume: f32,
    pub reverse: bool,
    pub start: f32,
    pub end: f32,
}

#[derive(Clone)]
pub struct RackChannel {
    pub voice: Option<VoiceSpec>,
    /// Slicekartan ur kanalens ljud (Fas 8.7). Noten `base_note + i` spelar
    /// slice `i` — samma regel i uppspelningen och i exporten, annars låter
    /// filen inte som det du hörde.
    pub slices: Vec<(f32, f32)>,
    /// Fallback gain used for the built-in synth/bass voices when a channel
    /// has no loaded sample (mirrors `ChannelStrip.volume`).
    pub fallback_volume: f32,
    pub steps: [bool; 16],
    pub notes: [u8; 16],
}

#[derive(Clone)]
pub struct PatternSnap {
    /// Per Channel-Rack channel step grid.
    pub steps: Vec<[bool; 16]>,
    /// Per Channel-Rack channel note grid.
    pub notes: Vec<[u8; 16]>,
    /// Piano-rollen, 24 rader × 16 steg (MIDI 48–71).
    ///
    /// Den behövs för att exporten ska spela samma noter som uppspelningen: den
    /// vanliga kanal 6-rutan bär bara **en** not per steg (den flattenade
    /// spegeln), så ett polyfont piano-roll-steg blev en enda not i filen.
    pub piano_roll: [[bool; 16]; 24],
    /// Den inspelade tagningen med sin mikro-tajming (Fas 6.4/6.5-uppföljning).
    ///
    /// Utan den renderade exporten noterna på rutnätet medan uppspelningen
    /// spelade dem där de faktiskt spelades — alltså ännu en skillnad mellan
    /// filen och det du hör.
    pub take: crate::midi_take::Take,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrackRole {
    Drums,
    Synth,
    Bass,
    Audio,
}

#[derive(Clone)]
pub struct TrackSnap {
    pub role: TrackRole,
    pub clips: [Option<usize>; 32],
    pub volume: f32,
    pub muted: bool,
    pub solo: bool,
}

/// Timeline audio (stems/mic/FX) for one playlist track index.
#[derive(Clone)]
pub struct TrackAudioSnap {
    pub track_index: usize,
    pub left: Arc<Vec<f32>>,
    pub right: Arc<Vec<f32>>,
    pub sample_rate: u32,
    pub volume: f32,
    pub pan: f32,
    pub muted: bool,
    pub regions: Vec<StemRegionPlayback>,
    pub eq: TrackEqSettings,
    /// Channel-strip dynamics / send values, so offline render matches live.
    pub comp_threshold_db: f32,
    pub comp_ratio: f32,
    pub reverb_send: f32,
    pub delay_send: f32,
    /// Formant-preserving transposition in semitones.
    pub pitch_semitones: f32,
    /// Sub-mix bus assignment (Fas 5.2).
    pub bus: usize,
    /// Optional VCA group assignment (Fas 5.2).
    pub vca: Option<usize>,
    /// Sidokedja (Fas 8.3): spåret duckas av det här spårets ljud.
    pub sidechain_from: Option<usize>,
    pub sidechain_amount_db: f32,
    pub sidechain_threshold_db: f32,
    /// Sends (Fas 8.13): parallella vägar till andra bussar. Utan dem i
    /// specen skulle en exporterad fil tappa dem — och då låter filen inte som
    /// högtalarna (samma regel som sidokedjan fick).
    pub sends: Vec<StemSend>,
}

/// Master FX / synth settings snapshot.
#[derive(Clone, Copy)]
pub struct FxState {
    pub waveform: super::command::Waveform,
    pub adsr: AdsrParams,
    pub filter: FilterParams,
    pub delay: DelayParams,
    pub reverb: ReverbParams,
    pub drive: f32,
    pub master_volume: f32,
    pub master_fx: MasterFxParams,
}

#[derive(Clone)]
pub struct RenderSpec {
    pub sample_rate: u32,
    pub bpm: f32,
    pub swing: f32,
    pub num_bars: usize,
    pub pattern_mode: bool,
    pub rack: Vec<RackChannel>,
    pub patterns: Vec<PatternSnap>,
    pub tracks: Vec<TrackSnap>,
    /// Audio stems parallel to `tracks` (empty = no audio on that index).
    pub timeline: Vec<TrackAudioSnap>,
    pub velocities: [f32; 16],
    /// When exporting a single stem, only this track index triggers patterns
    /// and only its audio stem is audible. `None` = full mix (live mute/solo
    /// semantics are respected).
    pub solo_track: Option<usize>,
    /// Extra silence after the last bar so reverb/delay tails decay.
    pub tail_secs: f32,
    /// Sub-mix bus group state applied to the full mix (Fas 5.2).
    pub bus_volume: [f32; super::synth::NUM_BUSES],
    pub bus_muted: [bool; super::synth::NUM_BUSES],
    pub bus_solo: [bool; super::synth::NUM_BUSES],
    /// VCA group state applied to the full mix (Fas 5.2).
    pub vca_volume: [f32; super::synth::NUM_VCAS],
    pub vca_muted: [bool; super::synth::NUM_VCAS],
    pub vca_solo: [bool; super::synth::NUM_VCAS],
}

pub fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

fn sample_trigger_command(ch: &RackChannel, note: u8, velocity: f32) -> Option<AudioCommand> {
    let v = ch.voice.as_ref()?;
    if v.left.is_empty() {
        return None;
    }
    let (start01, end01) =
        crate::audio::onset::window_for_note(&ch.slices, v.base_note, note, (v.start, v.end));
    Some(AudioCommand::TriggerSampleVoice {
        left: v.left.clone(),
        right: v.right.clone(),
        sample_rate: v.sample_rate,
        base_note: v.base_note,
        note,
        pitch_semitones: v.semitones,
        pitch_cents: v.cents,
        velocity,
        volume: v.volume,
        reverse: v.reverse,
        start01,
        end01,
    })
}

fn drum_cmd(ch_idx: usize) -> AudioCommand {
    match ch_idx {
        0 => AudioCommand::TriggerDrum(DrumType::Kick),
        1 => AudioCommand::TriggerDrum(DrumType::Snare),
        2 => AudioCommand::TriggerDrum(DrumType::Clap),
        3 => AudioCommand::TriggerDrum(DrumType::HiHatClosed),
        4 => AudioCommand::TriggerDrum(DrumType::HiHatOpen),
        5 => AudioCommand::TriggerDrum(DrumType::Crash),
        _ => AudioCommand::TriggerDrum(DrumType::Kick),
    }
}

/// Build the trigger commands for one 16th-note step, mirroring the live UI
/// (`trigger_step` in pattern mode, `trigger_song_step` in song mode).
/// Noterna ett steg ger upphov till. Delas med realtidsmätningen (Fas 7.2), så att
/// mätningen kör samma trigger-väg som exporten.
pub(crate) fn triggers_for_step(spec: &RenderSpec, bar: usize, sib: usize) -> Vec<AudioCommand> {
    let mut cmds = Vec::new();
    if spec.pattern_mode {
        let vel = spec.velocities[sib];
        for (idx, ch) in spec.rack.iter().enumerate() {
            if !ch.steps[sib] {
                continue;
            }
            let note = ch.notes[sib];
            if let Some(cmd) = sample_trigger_command(ch, note, vel) {
                cmds.push(cmd);
                continue;
            }
            match idx {
                0..=5 => cmds.push(drum_cmd(idx)),
                6 | 7 => {
                    let freq = midi_to_freq(note);
                    cmds.push(AudioCommand::NoteOn { note, freq, velocity: ch.fallback_volume * vel });
                }
                _ => {}
            }
        }
        return cmds;
    }

    let has_track_solo = spec.tracks.iter().any(|t| t.solo);
    for (t_idx, track) in spec.tracks.iter().enumerate() {
        // In per-track stem mode the chosen track is always audible (even when
        // muted in the mix); otherwise live mute/solo rules apply.
        let audible = match spec.solo_track {
            Some(s) => t_idx == s,
            None => {
                if has_track_solo {
                    track.solo
                } else {
                    !track.muted
                }
            }
        };
        if !audible {
            continue;
        }
        if track.role == TrackRole::Audio {
            continue; // handled by the timeline stem engine
        }
        let Some(pat_idx) = track.clips[bar] else { continue };
        let Some(pat) = spec.patterns.get(pat_idx) else { continue };
        let vel = spec.velocities[sib];
        match track.role {
            TrackRole::Drums => {
                for ch_idx in 0..6 {
                    let Some(ch_steps) = pat.steps.get(ch_idx) else { continue };
                    if !ch_steps[sib] {
                        continue;
                    }
                    let note = pat.notes.get(ch_idx).map(|n| n[sib]).unwrap_or(36);
                    if let Some(rack_ch) = spec.rack.get(ch_idx) {
                        if let Some(cmd) = sample_trigger_command(rack_ch, note, track.volume * vel) {
                            cmds.push(cmd);
                            continue;
                        }
                    }
                    cmds.push(drum_cmd(ch_idx));
                }
            }
            TrackRole::Synth | TrackRole::Bass => {
                // Piano-rollen först, precis som i `trigger_song_step`: är någon
                // rad tänd på detta steg spelar rutnätet, och då ska exporten
                // spela **alla** tända rader — inte den flattenade kanal
                // 6-spegeln, som bara bär en not per steg.
                // Samma plan som uppspelningen använder (Fas 6.4): noter vars
                // floor(pos) är detta steg spelas med sin fördröjning och sitt
                // anslag, och rutnätets rutor för dem hoppas över. Planen läses före
                // grinden, annars tystnar en not som spelades sent i förra steget.
                let step_samples = (60.0 / spec.bpm.max(1.0) / 4.0 * spec.sample_rate as f32) as u32;
                let plan = crate::midi_take::plan_for_step(
                    &pat.take,
                    sib,
                    step_samples,
                    &|key, slot| (48..72).contains(&key) && pat.piano_roll[(key - 48) as usize][slot],
                );
                if track.role == TrackRole::Synth
                    && ((0..24).any(|r| pat.piano_roll[r][sib]) || !plan.play.is_empty())
                {
                    for (note, delay, take_vel) in &plan.play {
                        let freq = midi_to_freq(*note);
                        cmds.push(AudioCommand::NoteOnDelayed {
                            note: *note,
                            freq,
                            velocity: track.volume * vel * take_vel,
                            delay_samples: *delay,
                        });
                    }
                    for row in 0..24 {
                        if pat.piano_roll[row][sib] {
                            let note = 48 + row as u8;
                            if plan.skip.contains(&note) {
                                continue;
                            }
                            let freq = midi_to_freq(note);
                            cmds.push(AudioCommand::NoteOn { note, freq, velocity: track.volume * vel });
                        }
                    }
                    continue;
                }
                let ch_idx = if track.role == TrackRole::Synth { 6 } else { 7 };
                let Some(ch_steps) = pat.steps.get(ch_idx) else { continue };
                if !ch_steps[sib] {
                    continue;
                }
                let note = pat.notes.get(ch_idx).map(|n| n[sib]).unwrap_or(60);
                if let Some(rack_ch) = spec.rack.get(ch_idx) {
                    if let Some(cmd) = sample_trigger_command(rack_ch, note, track.volume * vel) {
                        cmds.push(cmd);
                        continue;
                    }
                }
                let freq = midi_to_freq(note);
                cmds.push(AudioCommand::NoteOn { note, freq, velocity: track.volume * vel });
            }
            TrackRole::Audio => {}
        }
    }
    cmds
}

/// Loads timeline stems into the engine exactly as live playback does
/// (index == playlist track index, regions in absolute seconds).
pub fn load_timeline_into_engine(engine: &mut SynthEngine, timeline: &[TrackAudioSnap], solo_track: Option<usize>) {
    for t in timeline {
        if t.left.is_empty() {
            continue;
        }
        let forced_solo = solo_track.map(|s| s == t.track_index).unwrap_or(false);
        let other_in_solo = solo_track.is_some() && !forced_solo;
        engine.handle_command(AudioCommand::LoadStemTrack {
            track_index: t.track_index,
            left: t.left.clone(),
            right: t.right.clone(),
            sample_rate: t.sample_rate as f32,
            volume: t.volume,
            pan: t.pan,
            start_time_secs: 0.0,
        });
        engine.handle_command(AudioCommand::SetStemTrackRegions {
            track_index: t.track_index,
            regions: t.regions.clone(),
        });
        engine.handle_command(AudioCommand::SetStemTrackState {
            track_index: t.track_index,
            volume: t.volume,
            pan: t.pan,
            muted: other_in_solo || (!forced_solo && t.muted),
            solo: false,
        });
        engine.handle_command(AudioCommand::SetStemTrackRouting {
            track_index: t.track_index,
            bus: t.bus,
            vca: t.vca,
        });
        engine.handle_command(AudioCommand::SetStemTrackSidechain {
            track_index: t.track_index,
            from: t.sidechain_from,
            amount_db: t.sidechain_amount_db,
            threshold_db: t.sidechain_threshold_db,
        });
        engine.handle_command(AudioCommand::SetStemTrackSends {
            track_index: t.track_index,
            sends: t.sends.clone(),
        });
        engine.handle_command(AudioCommand::SetTrackEq {
            track_index: t.track_index,
            settings: t.eq,
        });
        engine.handle_command(AudioCommand::SetTrackMix {
            track_index: t.track_index,
            comp_threshold_db: t.comp_threshold_db,
            comp_ratio: t.comp_ratio,
            reverb_send: t.reverb_send,
            delay_send: t.delay_send,
            pitch_semitones: t.pitch_semitones,
        });
    }
}

/// Renders the project offline to interleaved float32 stereo samples.
/// A ready engine (FX + stems) must be passed in; `synth_config` on the
/// engine is used verbatim, giving bit-identical master processing.
pub fn render_project_offline(engine: &mut SynthEngine, spec: &RenderSpec) -> Vec<f32> {
    let rate = engine.sample_rate;
    let steps_per_bar = 16usize;
    let total_steps = steps_per_bar * spec.num_bars;

    let mut out: Vec<f32> = Vec::with_capacity(total_steps * 128 + 1024);
    let step_frames = |sib: usize| -> u64 {
        let base = (rate as f64) * 60.0 / (spec.bpm.max(20.0) as f64) / 4.0;
        let f = if sib % 2 == 1 {
            1.0 + (spec.swing * 0.35) as f64
        } else {
            1.0 - (spec.swing * 0.35) as f64
        };
        (base * f).round().max(1.0) as u64
    };

    let mut song_playing = false;
    if !spec.pattern_mode {
        engine.song_time_samples = 0;
        song_playing = true;
    }

    for gs in 0..total_steps {
        let bar = gs / 16;
        let sib = gs % 16;
        for cmd in triggers_for_step(spec, bar, sib) {
            engine.handle_command(cmd);
        }
        let dur = step_frames(sib);
        engine.song_playing = song_playing;
        for _ in 0..dur {
            let (l, r) = engine.process_stereo();
            out.push(l);
            out.push(r);
        }
    }

    // Tail: keep the engine running a little after the final bar so that
    // delay/reverb tails and drum decays do not get cut off abruptly.
    engine.song_playing = song_playing;
    let tail = ((rate * spec.tail_secs.max(0.0)) as u64).min(rate as u64 * 8);
    for _ in 0..tail {
        let (l, r) = engine.process_stereo();
        out.push(l);
        out.push(r);
    }
    out
}

/// Builds a fully configured offline engine for `spec` (master FX copied from
/// `fx`, timeline stems loaded).
pub fn build_offline_engine(spec: &RenderSpec, fx: &FxState) -> SynthEngine {
    let mut engine = SynthEngine::new(spec.sample_rate as f32);
    engine.waveform = fx.waveform;
    engine.adsr = fx.adsr;
    engine.filter_params = fx.filter;
    engine.delay_params = fx.delay;
    engine.delay = super::effects::StereoDelay::new(engine.sample_rate);
    engine.reverb_params = fx.reverb;
    engine.drive = fx.drive.clamp(1.0, 10.0);
    engine.master_fx.set_params(fx.master_fx);
    engine.master_volume = fx.master_volume.clamp(0.0, 1.0);
    // Apply the project's sub-mix bus / VCA group state to the full mix. For a
    // single-stem export (`solo_track`), group gains/solos are left neutral so
    // only the explicitly isolated track is audible (Fas 5.2).
    if spec.solo_track.is_none() {
        for bus in 0..super::synth::NUM_BUSES {
            engine.handle_command(AudioCommand::SetBusState {
                bus,
                volume: spec.bus_volume[bus],
                muted: spec.bus_muted[bus],
                solo: spec.bus_solo[bus],
            });
        }
        for vca in 0..super::synth::NUM_VCAS {
            engine.handle_command(AudioCommand::SetVcaState {
                vca,
                volume: spec.vca_volume[vca],
                muted: spec.vca_muted[vca],
                solo: spec.vca_solo[vca],
            });
        }
    }
    load_timeline_into_engine(&mut engine, &spec.timeline, spec.solo_track);
    engine
}

// ---------------------------------------------------------------------------
// File encoding
// ---------------------------------------------------------------------------

/// Clean metadata written to exported files. Software marker is always Sonix
/// Studio; no AI/provider tags are ever injected.
#[derive(Clone, Debug, Default)]
pub struct ExportMeta {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: String,
    pub comment: String,
}

impl ExportMeta {
    pub const SOFTWARE: &'static str = "Sonix Studio";
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportFormat {
    Wav16,
    Wav24,
    Wav32,
    Flac,
    Mp3,
    Ogg,
    Aac,
}

impl ExportFormat {
    pub fn label(self) -> &'static str {
        match self {
            ExportFormat::Wav16 => "WAV (16-bit PCM)",
            ExportFormat::Wav24 => "WAV (24-bit PCM)",
            ExportFormat::Wav32 => "WAV (32-bit float)",
            ExportFormat::Flac => "FLAC (24-bit lossless)",
            ExportFormat::Mp3 => "MP3 (320 kbps CBR)",
            ExportFormat::Ogg => "OGG Vorbis (hög kvalitet)",
            ExportFormat::Aac => "AAC / M4A (320 kbps)",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            ExportFormat::Wav16 | ExportFormat::Wav24 | ExportFormat::Wav32 => "wav",
            ExportFormat::Flac => "flac",
            ExportFormat::Mp3 => "mp3",
            ExportFormat::Ogg => "ogg",
            ExportFormat::Aac => "m4a",
        }
    }
}

/// Sanitize a name for use in a file name.
pub fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == ' ' || c == '.' {
                c
            } else {
                ' '
            }
        })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join("_")
}

/// Dither-inställningar för export (Fas 6.5).
#[derive(Clone, Copy, Debug)]
pub struct DitherSettings {
    /// Dither vid kvantisering till 16 bitar. På som standard: det är
    /// standardpraxis och kostar ingenting utom en kvantnivås brus.
    pub enabled: bool,
    /// Noise shaping: flyttar bruset uppåt i frekvens. Av som standard, eftersom
    /// det lägger mer energi i diskanten — en smaksak.
    pub noise_shaping: bool,
    pub seed: u64,
}

impl Default for DitherSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            noise_shaping: false,
            seed: crate::audio::dither::DEFAULT_SEED,
        }
    }
}

/// Som [`write_export`], men med valfria dither-inställningar.
pub fn write_export_with(
    path: &str,
    fmt: ExportFormat,
    samples_lr: &[f32],
    sample_rate: u32,
    meta: &ExportMeta,
    dither: DitherSettings,
) -> Result<(), String> {
    match fmt {
        ExportFormat::Wav16 => write_wav(path, samples_lr, sample_rate, 16, false, meta, dither),
        ExportFormat::Wav24 => write_wav(path, samples_lr, sample_rate, 24, false, meta, dither),
        ExportFormat::Wav32 => write_wav(path, samples_lr, sample_rate, 32, true, meta, dither),
        _ => write_export(path, fmt, samples_lr, sample_rate, meta),
    }
}

pub fn write_export(
    path: &str,
    format: ExportFormat,
    samples_lr: &[f32],
    sample_rate: u32,
    meta: &ExportMeta,
) -> Result<(), String> {
    match format {
        ExportFormat::Wav16 => {
            write_wav(path, samples_lr, sample_rate, 16, false, meta, DitherSettings::default())
        }
        ExportFormat::Wav24 => {
            write_wav(path, samples_lr, sample_rate, 24, false, meta, DitherSettings::default())
        }
        ExportFormat::Wav32 => {
            write_wav(path, samples_lr, sample_rate, 32, true, meta, DitherSettings::default())
        }
        ExportFormat::Flac => write_flac(path, samples_lr, sample_rate, 24, meta),
        ExportFormat::Mp3 | ExportFormat::Ogg | ExportFormat::Aac => {
            write_with_ffmpeg(path, format, samples_lr, sample_rate, meta)
        }
    }
}

fn clamp01(v: f32) -> f32 {
    v.clamp(-1.0, 1.0)
}

fn info_tag(out: &mut Vec<u8>, fourcc: &[u8; 4], value: &str) {
    if value.is_empty() {
        return;
    }
    let bytes = value.as_bytes();
    out.extend_from_slice(fourcc);
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(bytes);
    if bytes.len() % 2 == 1 {
        out.push(0);
    }
}

/// Skriver en wav med givna sampel och metadata.
///
/// `pub(crate)` för att `write_stem_wav` (och testerna i `metadata.rs`, som prövar
/// städningen mot en **riktig** fil från den här skrivaren) ska kunna nå den.
pub(crate) fn write_wav(
    path: &str,
    samples: &[f32],
    sample_rate: u32,
    bits: u16,
    is_float: bool,
    meta: &ExportMeta,
    dither: DitherSettings,
) -> Result<(), String> {
    let channels: u16 = 2;
    let bytes_per_sample = bits / 8;
    let data_bytes = samples.len() as u32 * bytes_per_sample as u32;

    let mut tags: Vec<u8> = Vec::new();
    info_tag(&mut tags, b"INAM", &meta.title);
    info_tag(&mut tags, b"IART", &meta.artist);
    info_tag(&mut tags, b"IPRD", &meta.album);
    info_tag(&mut tags, b"IGNR", &meta.genre);
    info_tag(&mut tags, b"ICRD", &meta.year);
    info_tag(&mut tags, b"ICMT", &meta.comment);
    info_tag(&mut tags, b"ISFT", ExportMeta::SOFTWARE);

    let mut file: Vec<u8> = Vec::with_capacity(44 + data_bytes as usize + tags.len() + 64);
    file.extend_from_slice(b"RIFF");
    file.extend_from_slice(&[0; 4]); // size placeholder, fixed below
    file.extend_from_slice(b"WAVE");

    // fmt chunk
    file.extend_from_slice(b"fmt ");
    file.extend_from_slice(&16_u32.to_le_bytes());
    let audio_format: u16 = if is_float { 3 } else { 1 };
    file.extend_from_slice(&audio_format.to_le_bytes());
    file.extend_from_slice(&channels.to_le_bytes());
    file.extend_from_slice(&sample_rate.to_le_bytes());
    let byte_rate = sample_rate * channels as u32 * bytes_per_sample as u32;
    file.extend_from_slice(&byte_rate.to_le_bytes());
    let block_align = channels * bytes_per_sample;
    file.extend_from_slice(&block_align.to_le_bytes());
    file.extend_from_slice(&bits.to_le_bytes());

    // data chunk
    file.extend_from_slice(b"data");
    file.extend_from_slice(&data_bytes.to_le_bytes());
    if is_float {
        for &s in samples {
            file.extend_from_slice(&clamp01(s).to_le_bytes());
        }
    } else if bits == 24 {
        for &s in samples {
            let v = (clamp01(s) * 8388607.0) as i32;
            file.push((v & 0xff) as u8);
            file.push(((v >> 8) & 0xff) as u8);
            file.push(((v >> 16) & 0xff) as u8);
        }
    } else {
        // 16 bitar: dither (Fas 6.5). Utan det blir kvantiseringsfelet korrelerat
        // med materialet och hörs som distorsion på svaga partier; med TPDF blir
        // det ett jämnt brusgolv. Kanalerna håller egna shaping-tillstånd.
        if dither.enabled {
            let mut d = crate::audio::dither::TpdfDither::new(dither.seed)
                .with_noise_shaping(dither.noise_shaping);
            for (i, &s) in samples.iter().enumerate() {
                let v = d.quantize_i16(s, i % 2);
                file.extend_from_slice(&v.to_le_bytes());
            }
        } else {
            for &s in samples {
                let v = (clamp01(s) * 32767.0) as i16;
                file.extend_from_slice(&v.to_le_bytes());
            }
        }
    }

    // LIST INFO chunk (trailing, keeps PCM parsers happy)
    if !tags.is_empty() {
        let mut list_body: Vec<u8> = Vec::new();
        list_body.extend_from_slice(b"INFO");
        list_body.extend_from_slice(&tags);
        if list_body.len() % 2 == 1 {
            list_body.push(0);
        }
        file.extend_from_slice(b"LIST");
        file.extend_from_slice(&(list_body.len() as u32).to_le_bytes());
        file.extend_from_slice(&list_body);
    }

    let riff_size = file.len() as u32 - 8;
    file[4..8].copy_from_slice(&riff_size.to_le_bytes());
    std::fs::write(path, file).map_err(|e| e.to_string())
}

fn vorbis_comment_bytes(meta: &ExportMeta) -> Vec<u8> {
    fn push_field(out: &mut Vec<u8>, comment: &str) {
        if comment.is_empty() {
            return;
        }
        out.extend_from_slice(&(comment.len() as u32).to_le_bytes());
        out.extend_from_slice(comment.as_bytes());
    }
    let vendor = ExportMeta::SOFTWARE;
    let mut fields: Vec<String> = Vec::new();
    if !meta.title.is_empty() { fields.push(format!("TITLE={}", meta.title)); }
    if !meta.artist.is_empty() { fields.push(format!("ARTIST={}", meta.artist)); }
    if !meta.album.is_empty() { fields.push(format!("ALBUM={}", meta.album)); }
    if !meta.genre.is_empty() { fields.push(format!("GENRE={}", meta.genre)); }
    if !meta.year.is_empty() { fields.push(format!("DATE={}", meta.year)); }
    if !meta.comment.is_empty() { fields.push(format!("COMMENT={}", meta.comment)); }
    fields.push(format!("ENCODER={}", ExportMeta::SOFTWARE));

    let mut out: Vec<u8> = Vec::new();
    out.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    out.extend_from_slice(vendor.as_bytes());
    out.extend_from_slice(&(fields.len() as u32).to_le_bytes());
    for f in &fields {
        push_field(&mut out, f);
    }
    out
}

fn write_flac(
    path: &str,
    samples: &[f32],
    sample_rate: u32,
    bits: u16,
    meta: &ExportMeta,
) -> Result<(), String> {
    use flacenc::bitsink::ByteSink;
    use flacenc::component::{BitRepr, MetadataBlockData};
    use flacenc::config::Encoder;
    use flacenc::error::Verify;
    use flacenc::source::MemSource;

    let scale = ((1i64 << (bits - 1)) - 1) as f64; // e.g. 8388607 for 24-bit
    let int_samples: Vec<i32> = samples
        .iter()
        .map(|&s| (clamp01(s) as f64 * scale).round() as i32)
        .collect();

    let source = MemSource::from_samples(&int_samples, 2, bits as usize, sample_rate as usize);
    let config = Encoder::default()
        .into_verified()
        .map_err(|(_cfg, e)| format!("{:?}", e))?;
    let mut stream = flacenc::encode_with_fixed_block_size(&config, source, 4096)
        .map_err(|e| e.to_string())?;
    // Append a clean VORBIS_COMMENT metadata block (type 4).
    let comment = vorbis_comment_bytes(meta);
    let block = MetadataBlockData::new_unknown(4, &comment).map_err(|e| e.to_string())?;
    stream.add_metadata_block(block);

    let mut sink = ByteSink::new();
    stream.write(&mut sink).map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    file.write_all(sink.as_slice()).map_err(|e| e.to_string())
}

fn ffmpeg_available() -> bool {
    Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn write_with_ffmpeg(
    path: &str,
    format: ExportFormat,
    samples: &[f32],
    sample_rate: u32,
    meta: &ExportMeta,
) -> Result<(), String> {
    if !ffmpeg_available() {
        return Err(crate::tstatus!("ffmpeg är inte installerat – kan inte koda {}. Installera ffmpeg (t.ex. 'sudo pacman -S ffmpeg') och försök igen. WAV/FLAC fungerar alltid utan ffmpeg.",
            format.label()
        ));
    }

    let dir = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp_path = dir.join(format!(".sonix_export_tmp_{}.wav", std::process::id()));
    write_wav(
        &tmp_path.to_string_lossy(),
        samples,
        sample_rate,
        24,
        false,
        meta,
        DitherSettings::default(),
    )?;

    let (codec_args, _label): (Vec<String>, &str) = match format {
        ExportFormat::Mp3 => (
            vec!["-codec:a".into(), "libmp3lame".into(), "-b:a".into(), "320k".into()],
            "MP3",
        ),
        ExportFormat::Ogg => (
            vec!["-codec:a".into(), "libvorbis".into(), "-q:a".into(), "6".into()],
            "OGG Vorbis",
        ),
        ExportFormat::Aac => (
            vec!["-codec:a".into(), "aac".into(), "-b:a".into(), "320k".into()],
            "AAC",
        ),
        _ => return Err("ffmpeg får bara användas för MP3/OGG/AAC".into()),
    };

    let mut cmd = Command::new("ffmpeg");
    cmd.arg("-hide_banner").arg("-y").arg("-i").arg(&tmp_path);
    if !meta.title.is_empty() { cmd.arg("-metadata").arg(format!("title={}", meta.title)); }
    if !meta.artist.is_empty() { cmd.arg("-metadata").arg(format!("artist={}", meta.artist)); }
    if !meta.album.is_empty() { cmd.arg("-metadata").arg(format!("album={}", meta.album)); }
    if !meta.genre.is_empty() { cmd.arg("-metadata").arg(format!("genre={}", meta.genre)); }
    if !meta.year.is_empty() { cmd.arg("-metadata").arg(format!("date={}", meta.year)); }
    if !meta.comment.is_empty() { cmd.arg("-metadata").arg(format!("comment={}", meta.comment)); }
    cmd.arg("-metadata").arg(format!("encoder={}", ExportMeta::SOFTWARE));
    cmd.args(&codec_args);
    cmd.arg(path);

    let out = cmd
        .output()
        .map_err(|e| format!("kunde inte starta ffmpeg: {}", e))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        let _ = std::fs::remove_file(&tmp_path);
        return Err(crate::tstatus!("ffmpeg misslyckades för {}: {}",
            format.label(),
            stderr.lines().last().unwrap_or(&stderr)
        ));
    }
    let _ = std::fs::remove_file(&tmp_path);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render_smoke_spec() -> RenderSpec {
        let mut sine = Vec::with_capacity(4410);
        for i in 0..4410 {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
            sine.push(s);
        }
        let l = Arc::new(sine);
        let r = Arc::new(l.clone().to_vec());
        let mut rack = Vec::new();
        for i in 0..8 {
            let voice = if i == 0 {
                Some(VoiceSpec {
                    left: l.clone(),
                    right: r.clone(),
                    sample_rate: 44100,
                    base_note: 36,
                    semitones: 0,
                    cents: 0.0,
                    volume: 0.9,
                    reverse: false,
                    start: 0.0,
                    end: 1.0,
                })
            } else {
                None
            };
            let mut steps = [false; 16];
            steps[0] = true;
            steps[8] = true;
            rack.push(RackChannel {
                voice,
                slices: Vec::new(),
                fallback_volume: 0.85,
                steps,
                notes: [36; 16],
            });
        }
        RenderSpec {
            sample_rate: 44100,
            bpm: 120.0,
            swing: 0.0,
            num_bars: 2,
            pattern_mode: true,
            rack,
            patterns: vec![],
            tracks: vec![],
            timeline: vec![],
            velocities: [0.85; 16],
            solo_track: None,
            tail_secs: 0.1,
            bus_volume: [1.0; crate::audio::synth::NUM_BUSES],
            bus_muted: [false; crate::audio::synth::NUM_BUSES],
            bus_solo: [false; crate::audio::synth::NUM_BUSES],
            vca_volume: [1.0; crate::audio::synth::NUM_VCAS],
            vca_muted: [false; crate::audio::synth::NUM_VCAS],
            vca_solo: [false; crate::audio::synth::NUM_VCAS],
        }
    }

    /// En spec med ett synthspår som spelar pattern 0 i takt 0.
    fn synth_spec_with(piano_roll: [[bool; 16]; 24], channel6_steps: [bool; 16], channel6_note: u8) -> RenderSpec {
        synth_spec_with_take(piano_roll, channel6_steps, channel6_note, crate::midi_take::Take::new())
    }

    fn synth_spec_with_take(
        piano_roll: [[bool; 16]; 24],
        channel6_steps: [bool; 16],
        channel6_note: u8,
        take: crate::midi_take::Take,
    ) -> RenderSpec {
        // Åtta kanalrader, som appens specc: index 6 är synthkanalen.
        let mut steps = vec![[false; 16]; 8];
        steps[6] = channel6_steps;
        let mut notes = vec![[60u8; 16]; 8];
        notes[6] = [channel6_note; 16];
        let mut patterns = Vec::new();
        patterns.push(PatternSnap {
            steps,
            notes,
            piano_roll,
            take,
        });
        RenderSpec {
            sample_rate: 44100,
            bpm: 120.0,
            swing: 0.0,
            num_bars: 1,
            pattern_mode: false,
            rack: Vec::new(),
            patterns,
            tracks: vec![TrackSnap {
                role: TrackRole::Synth,
                clips: {
                    let mut c = [None; 32];
                    c[0] = Some(0);
                    c
                },
                volume: 1.0,
                muted: false,
                solo: false,
            }],
            timeline: vec![],
            velocities: [1.0; 16],
            solo_track: None,
            tail_secs: 0.05,
            bus_volume: [1.0; crate::audio::synth::NUM_BUSES],
            bus_muted: [false; crate::audio::synth::NUM_BUSES],
            bus_solo: [false; crate::audio::synth::NUM_BUSES],
            vca_volume: [1.0; crate::audio::synth::NUM_VCAS],
            vca_muted: [false; crate::audio::synth::NUM_VCAS],
            vca_solo: [false; crate::audio::synth::NUM_VCAS],
        }
    }

    /// (notnummer, fördröjning i samples) för de fördröjda noterna.
    fn delayed_notes(cmds: &[AudioCommand]) -> Vec<(u8, u32)> {
        cmds.iter()
            .filter_map(|c| match c {
                AudioCommand::NoteOnDelayed { note, delay_samples, .. } => Some((*note, *delay_samples)),
                _ => None,
            })
            .collect()
    }

    fn note_keys(cmds: &[AudioCommand]) -> Vec<u8> {
        cmds.iter()
            .filter_map(|c| match c {
                AudioCommand::NoteOn { note, .. } => Some(*note),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_polyphonic_piano_roll_step_exports_every_note() {
        // Ett ackord i piano-rollen (rad 0, 4 och 7 = MIDI 48, 52, 55) ska bli
        // tre noter i exporten — uppspelningen spelar alla rader, så filen måste
        // göra detsamma. Kanal 6-rutan bär bara en not per steg.
        let mut grid = [[false; 16]; 24];
        grid[0][0] = true;
        grid[4][0] = true;
        grid[7][0] = true;
        let mut ch_steps = [false; 16];
        ch_steps[0] = true;
        let spec = synth_spec_with(grid, ch_steps, 55);

        let cmds = triggers_for_step(&spec, 0, 0);
        let mut keys = note_keys(&cmds);
        keys.sort();
        assert_eq!(keys, vec![48, 52, 55], "hela ackordet ska med i exporten");
    }

    #[test]
    fn a_step_without_piano_roll_notes_still_uses_the_channel_grid() {
        // Är rutnätet tomt ska kanal 6-rutan spela som förut (en not).
        let grid = [[false; 16]; 24];
        let mut ch_steps = [false; 16];
        ch_steps[3] = true;
        let spec = synth_spec_with(grid, ch_steps, 62);

        let cmds = triggers_for_step(&spec, 0, 3);
        assert_eq!(note_keys(&cmds), vec![62]);
        assert!(note_keys(&triggers_for_step(&spec, 0, 0)).is_empty());
    }

    #[test]
    fn the_piano_roll_wins_over_the_channel_grid_for_the_same_step() {
        // Precis som i uppspelningen: är rutnätet tänt på steget spelar det, och
        // kanalrutans not ska inte läggas ovanpå (annars blev det en dubbelnot).
        let mut grid = [[false; 16]; 24];
        grid[12][5] = true; // MIDI 60
        let mut ch_steps = [false; 16];
        ch_steps[5] = true;
        let spec = synth_spec_with(grid, ch_steps, 40);

        let cmds = triggers_for_step(&spec, 0, 5);
        assert_eq!(note_keys(&cmds), vec![60], "bara rutnätets not på det steget");
    }

    /// Energin vid en frekvens (Goertzel), för att mäta vilka toner som hörs.
    fn tone_energy(buf: &[f32], freq: f32, sr: f32) -> f32 {
        let k = 2.0 * (2.0 * std::f32::consts::PI * freq / sr).cos();
        let (mut s1, mut s2) = (0.0f32, 0.0f32);
        for &x in buf {
            let s0 = x + k * s1 - s2;
            s2 = s1;
            s1 = s0;
        }
        ((s1 * s1 + s2 * s2 - k * s1 * s2).max(0.0)).sqrt() / buf.len() as f32
    }

    #[test]
    fn the_take_moves_a_note_off_the_grid_in_the_export() {
        // En not inspelad 0.25 steg sent ska exporteras med sin fördröjning, inte
        // på rutnätet — annars matchar filen inte det du hör.
        let mut grid = [[false; 16]; 24];
        grid[12][2] = true; // MIDI 60 på steg 2
        let mut take = crate::midi_take::Take::new();
        take.push(2.25, 60, 0.8);
        let spec = synth_spec_with_take(grid, [false; 16], 60, take);

        // Steg 2: en fördröjning på en fjärdedels steg (0.25 × 5512 samples).
        let cmds = triggers_for_step(&spec, 0, 2);
        let delayed = delayed_notes(&cmds);
        assert_eq!(delayed.len(), 1, "en fördröjd not: {delayed:?}");
        let (note, delay) = delayed[0];
        assert_eq!(note, 60);
        assert!((delay as i64 - 1378).abs() <= 2, "fördröjningen var {delay}");
        // Och ingen dubbelnot: rutan hoppas över när tagningen sköter den.
        assert!(note_keys(&cmds).is_empty(), "ingen odelajad not också");

        // Steg 3 ska vara tyst (noten hör till steg 2).
        assert!(triggers_for_step(&spec, 0, 3).is_empty());
    }

    #[test]
    fn a_note_played_late_in_a_step_still_sounds_from_that_step() {
        // Noten spelades 0.7 steg in i steg 2, alltså *efter* mitten: dess ruta
        // hamnar på steg 3 (närmaste rutnätslinje) medan den klingar från steg 2.
        // Läses tagningen efter grinden tystnar noten helt — det var en riktig
        // bugg i 6.4 tills den här grinden flyttades.
        let mut grid = [[false; 16]; 24];
        grid[12][3] = true; // rutan hamnade på steg 3
        let mut take = crate::midi_take::Take::new();
        take.push(2.7, 60, 0.8);
        let spec = synth_spec_with_take(grid, [false; 16], 60, take);

        // Steg 2 har ingen ruta tänd alls — ändå ska noten spelas därifrån.
        assert!(!(0..24).any(|r| spec.patterns[0].piano_roll[r][2]));
        let delayed = delayed_notes(&triggers_for_step(&spec, 0, 2));
        assert_eq!(delayed.len(), 1, "noten ska klinga från steg 2: {delayed:?}");
        assert!((delayed[0].1 as i64 - 3858).abs() <= 2, "fördröjningen var {}", delayed[0].1);

        // Och steg 3 ska inte spela den igen.
        assert!(triggers_for_step(&spec, 0, 3).is_empty(), "ingen dubbelnot på rutan");
    }

    #[test]
    fn a_take_note_whose_cell_was_cleared_is_not_exported() {
        // Rutnätet är fortfarande sanningen om vad som är på.
        let grid = [[false; 16]; 24];
        let mut take = crate::midi_take::Take::new();
        take.push(2.25, 60, 0.8);
        let spec = synth_spec_with_take(grid, [false; 16], 60, take);
        assert!(triggers_for_step(&spec, 0, 2).is_empty());
    }

    #[test]
    fn every_note_of_a_polyphonic_step_is_audible_in_the_render() {
        // Kommandolistan räcker inte som bevis: tonerna ska höras i ljudet också.
        // Ackordet C-dur (MIDI 48, 52, 55 = 130.81, 164.81, 196.00 Hz).
        let mut grid = [[false; 16]; 24];
        grid[0][0] = true;
        grid[4][0] = true;
        grid[7][0] = true;
        let mut ch_steps = [false; 16];
        ch_steps[0] = true;
        let spec = synth_spec_with(grid, ch_steps, 60);

        let fx = FxState {
            waveform: super::super::command::Waveform::Sine,
            adsr: AdsrParams { attack: 0.005, decay: 0.2, sustain: 0.8, release: 0.2 },
            filter: FilterParams::default(),
            delay: DelayParams::default(),
            reverb: ReverbParams::default(),
            drive: 1.0,
            master_volume: 1.0,
            master_fx: MasterFxParams::default(),
        };
        let mut engine = build_offline_engine(&spec, &fx);
        let buf = render_project_offline(&mut engine, &spec);
        assert!(!buf.is_empty(), "renderingen gav inget ljud");

        let left: Vec<f32> = buf.iter().step_by(2).copied().collect();
        let energies: Vec<f32> = [130.81, 164.81, 196.00]
            .iter()
            .map(|f| tone_energy(&left, *f, spec.sample_rate as f32))
            .collect();
        // En kontrollton som inte spelas, för att ha något att jämföra med.
        let control = tone_energy(&left, 155.56, spec.sample_rate as f32);

        for (f, e) in [130.81, 164.81, 196.00].iter().zip(energies.iter()) {
            assert!(
                *e > control * 4.0,
                "tonen {f} Hz hörs inte: {e} mot kontrollen {control}"
            );
        }
    }

    #[test]
    fn offline_render_produces_audio() {
        let spec = render_smoke_spec();
        let fx = FxState {
            waveform: super::super::command::Waveform::Square,
            adsr: AdsrParams { attack: 0.005, decay: 0.2, sustain: 0.1, release: 0.1 },
            filter: FilterParams::default(),
            delay: DelayParams::default(),
            reverb: ReverbParams::default(),
            drive: 1.0,
            master_volume: 0.9,
            master_fx: MasterFxParams::default(),
        };
        let mut engine = build_offline_engine(&spec, &fx);
        let buf = render_project_offline(&mut engine, &spec);
        assert!(!buf.is_empty(), "rendering produced no output");
        let peak = buf.iter().fold(0.0f32, |m, &s| m.max(s.abs()));
        assert!(peak > 0.01, "rendering silent, peak={}", peak);
        let frames = buf.len() / 2;
        let step_frames = (44100.0 * 60.0 / 120.0 / 4.0) as usize; // 5512/5513
        let min_frames = 2 * 16 * (step_frames - 1) + 4410;
        let max_frames = 2 * 16 * (step_frames + 1) + 4410 + 64;
        assert!(frames >= min_frames && frames <= max_frames, "unexpected frame count {}", frames);
    }

    /// Fas 8.13 i exporten: en **send** följer med offline-renderingen.
    ///
    /// Spår 1 (880 Hz) skickas till buss 3, som är neddragen till 0,5 — energin vid
    /// 880 Hz i den färdiga mixen ska då öka. Att målets buss är neddragen är själva
    /// poängen: en send till en buss med samma nivå är ohörbar (summan är linjär),
    /// så utan det hade testet mätt ingenting. Regeln "filen ska låta som
    /// högtalarna" har kostat oss två gånger redan.
    #[test]
    fn a_send_follows_the_offline_render() {
        fn spec_with_send(send: bool) -> RenderSpec {
            let mut spec = render_smoke_spec();
            // Tyst kanalrack: bara de två tidslinjespåren ska höras.
            for ch in spec.rack.iter_mut() {
                ch.steps = [false; 16];
            }
            spec.pattern_mode = false;
            spec.bus_volume[3] = 0.5;
            let tone = |freq: f32| -> Arc<Vec<f32>> {
                let n = 44_100 * 3;
                Arc::new(
                    (0..n)
                        .map(|i| {
                            (2.0 * std::f32::consts::PI * freq * i as f32 / 44_100.0).sin() * 0.5
                        })
                        .collect(),
                )
            };
            let snap = |track_index: usize, freq: f32, sends: Vec<StemSend>| TrackAudioSnap {
                track_index,
                left: tone(freq),
                right: tone(freq),
                sample_rate: 44_100,
                volume: 1.0,
                pan: 0.0,
                muted: false,
                regions: vec![StemRegionPlayback {
                    start_time_secs: 0.0,
                    length_secs: 3.0,
                    sample_offset_sec: 0.0,
                    gain: 1.0,
                    fade_in_sec: 0.0,
                    fade_out_sec: 0.0,
                    muted: false,
                    is_reverse: false,
                    loop_length_secs: 0.0,
                    stretch_ratio: 1.0,
                    source_audio: None,
                }],
                eq: TrackEqSettings::default(),
                comp_threshold_db: 0.0,
                comp_ratio: 1.0,
                reverb_send: 0.0,
                delay_send: 0.0,
                pitch_semitones: 0.0,
                bus: 1,
                vca: None,
                sidechain_from: None,
                sidechain_amount_db: 0.0,
                sidechain_threshold_db: -30.0,
                sends,
            };
            spec.timeline = vec![
                snap(0, 220.0, Vec::new()),
                snap(
                    1,
                    880.0,
                    if send {
                        vec![StemSend { target_bus: 3, level: 1.0 }]
                    } else {
                        Vec::new()
                    },
                ),
            ];
            spec
        }

        let energy_880 = |send: bool| -> f32 {
            let spec = spec_with_send(send);
            let fx = FxState {
                waveform: super::super::command::Waveform::Square,
                adsr: AdsrParams { attack: 0.005, decay: 0.2, sustain: 0.1, release: 0.1 },
                filter: FilterParams::default(),
                delay: DelayParams::default(),
                reverb: ReverbParams::default(),
                drive: 1.0,
                master_volume: 0.9,
                master_fx: MasterFxParams::default(),
            };
            let mut engine = build_offline_engine(&spec, &fx);
            let buf = render_project_offline(&mut engine, &spec);
            let left: Vec<f32> = buf.iter().step_by(2).copied().collect();
            tone_energy(&left[44_100..], 880.0, 44_100.0)
        };

        let without = energy_880(false);
        let with = energy_880(true);
        assert!(without > 0.0, "spår 1 ska höras i exporten");
        assert!(
            with > without * 1.05,
            "senden ska höras i exporten: {without} → {with}"
        );
    }

    /// Fas 8.3 i exporten: en sidokedja följer med offline-renderingen. Spår 1
    /// (880 Hz) duckas av spår 0 (220 Hz) — energin vid 880 Hz mäts i den färdiga
    /// mixen, alltså samma sak som hörs vid uppspelning. Utan det här testet hade
    /// exporten kunnat tappa en duckning som uppspelningen har.
    #[test]
    fn a_sidechain_follows_the_offline_render() {
        fn spec_with_sidechain(sidechain: bool) -> RenderSpec {
            let mut spec = render_smoke_spec();
            // Tyst kanalrack: bara de två tidslinjespåren ska höras.
            for ch in spec.rack.iter_mut() {
                ch.steps = [false; 16];
            }
            // Tidslinjespåren hörs bara när sången spelas (inte pattern-läget).
            spec.pattern_mode = false;
            let tone = |freq: f32| -> Arc<Vec<f32>> {
                let n = 44_100 * 3;
                Arc::new(
                    (0..n)
                        .map(|i| {
                            (2.0 * std::f32::consts::PI * freq * i as f32 / 44_100.0).sin() * 0.5
                        })
                        .collect(),
                )
            };
            let snap = |track_index: usize, freq: f32, from: Option<usize>| TrackAudioSnap {
                track_index,
                left: tone(freq),
                right: tone(freq),
                sample_rate: 44_100,
                volume: 1.0,
                pan: 0.0,
                muted: false,
                regions: vec![StemRegionPlayback {
                    start_time_secs: 0.0,
                    length_secs: 3.0,
                    sample_offset_sec: 0.0,
                    gain: 1.0,
                    fade_in_sec: 0.0,
                    fade_out_sec: 0.0,
                    muted: false,
                    is_reverse: false,
                    loop_length_secs: 0.0,
                    stretch_ratio: 1.0,
                    source_audio: None,
                }],
                eq: TrackEqSettings::default(),
                comp_threshold_db: 0.0,
                comp_ratio: 1.0,
                reverb_send: 0.0,
                delay_send: 0.0,
                pitch_semitones: 0.0,
                bus: 0,
                vca: None,
                sidechain_from: from,
                sidechain_amount_db: 18.0,
                sidechain_threshold_db: -30.0,
                sends: Vec::new(),
            };
            spec.timeline = vec![
                snap(0, 220.0, None),
                snap(1, 880.0, if sidechain { Some(0) } else { None }),
            ];
            spec
        }

        let energy_880 = |sidechain: bool| -> f32 {
            let spec = spec_with_sidechain(sidechain);
            let fx = FxState {
                waveform: super::super::command::Waveform::Square,
                adsr: AdsrParams { attack: 0.005, decay: 0.2, sustain: 0.1, release: 0.1 },
                filter: FilterParams::default(),
                delay: DelayParams::default(),
                reverb: ReverbParams::default(),
                drive: 1.0,
                master_volume: 0.9,
                master_fx: MasterFxParams::default(),
            };
            let mut engine = build_offline_engine(&spec, &fx);
            let buf = render_project_offline(&mut engine, &spec);
            let left: Vec<f32> = buf.iter().step_by(2).copied().collect();
            tone_energy(&left[44_100..], 880.0, 44_100.0)
        };

        let open = energy_880(false);
        let ducked = energy_880(true);
        assert!(open > 0.0, "målspåret ska höras i exporten");
        assert!(
            ducked < open * 0.6,
            "exporten ska ha duckningen: {open} → {ducked}"
        );
    }

    fn sine_buffer(rate: u32, secs: f32) -> Vec<f32> {
        let n = (rate as f32 * secs) as usize;
        let mut v = Vec::with_capacity(n * 2);
        for i in 0..n {
            let t = i as f32 / rate as f32;
            let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.4;
            v.push(s);
            v.push(s * 0.9);
        }
        v
    }

    fn meta() -> ExportMeta {
        ExportMeta {
            title: "Testlåt".into(),
            artist: "Sonix".into(),
            album: "Album".into(),
            genre: "Test".into(),
            year: "2026".into(),
            comment: "Ren export utan AI-info".into(),
        }
    }

    fn ffmpeg_probe(path: &str) -> String {
        let out = Command::new("ffmpeg")
            .args(["-hide_banner", "-i", path])
            .output()
            .expect("ffmpeg probe");
        format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
    }

    #[test]
    fn wav_formats_write_valid_headers() {
        let dir = std::env::temp_dir().join("sonix_export_test_wav");
        std::fs::create_dir_all(&dir).unwrap();
        let buf = sine_buffer(44100, 0.2);
        for (fmt, bits_f) in [
            (ExportFormat::Wav16, (1u16, 16u16)),
            (ExportFormat::Wav24, (1, 24)),
            (ExportFormat::Wav32, (3, 32)),
        ] {
            let p = dir.join(format!("t_{}.wav", fmt.label()));
            write_export(&p.to_string_lossy(), fmt, &buf, 44100, &meta()).unwrap();
            let bytes = std::fs::read(&p).unwrap();
            assert!(bytes.len() > 44, "file too small");
            assert_eq!(&bytes[0..4], b"RIFF");
            assert_eq!(&bytes[8..12], b"WAVE");
            let audio_fmt = u16::from_le_bytes([bytes[20], bytes[21]]);
            assert_eq!(audio_fmt, bits_f.0);
            let bits = u16::from_le_bytes([bytes[34], bytes[35]]);
            assert_eq!(bits, bits_f.1);
            assert!(String::from_utf8_lossy(&bytes).contains("ISFT"));
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Läser ut 16-bitars-PCM ur en WAV-fil som `write_export` skrev.
    fn read_wav_i16(path: &std::path::Path) -> Vec<i16> {
        let bytes = std::fs::read(path).unwrap();
        // Hitta "data"-chunken och läs den som i16.
        let mut i = 12;
        while i + 8 <= bytes.len() {
            let id = &bytes[i..i + 4];
            let size = u32::from_le_bytes([bytes[i + 4], bytes[i + 5], bytes[i + 6], bytes[i + 7]]) as usize;
            if id == b"data" {
                let start = i + 8;
                let end = (start + size).min(bytes.len());
                return bytes[start..end]
                    .chunks_exact(2)
                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                    .collect();
            }
            i += 8 + size + (size % 2);
        }
        panic!("ingen data-chunk i {}", path.display());
    }

    #[test]
    fn dither_reaches_the_16_bit_file_and_removes_the_bias() {
        // Fas 6.5 "klart när", mätt på **filens bytes** — inte bara i modulen.
        let dir = std::env::temp_dir().join("sonix_export_test_dither");
        std::fs::create_dir_all(&dir).unwrap();

        // En svag ton (≈ −80 dBFS) är värsta fallet för kvantisering.
        let n = 20_000;
        let amp = 0.6 / 32768.0;
        let buf: Vec<f32> = (0..n)
            .map(|i| amp * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44_100.0).sin())
            .collect();

        let with_path = dir.join("med_dither.wav");
        let without_path = dir.join("utan_dither.wav");
        write_export_with(
            &with_path.to_string_lossy(),
            ExportFormat::Wav16,
            &buf,
            44_100,
            &meta(),
            DitherSettings { enabled: true, noise_shaping: false, seed: 4242 },
        )
        .unwrap();
        write_export_with(
            &without_path.to_string_lossy(),
            ExportFormat::Wav16,
            &buf,
            44_100,
            &meta(),
            DitherSettings { enabled: false, noise_shaping: false, seed: 4242 },
        )
        .unwrap();

        let with = read_wav_i16(&with_path);
        let without = read_wav_i16(&without_path);
        assert_eq!(with.len(), n, "lika många samples i båda filerna");
        assert_ne!(with, without, "dithern ska höras i bytesen");

        // Måttet som gäller för en ton är hur felet hänger ihop med signalen —
        // inte medelfelet. Trunkering är ensidig i *belopp*, så felet byter tecken
        // mellan halvperioderna och tar ut sig i medel; det är korrelationen som
        // hörs som distorsion.
        let corr = |q: &[i16]| {
            let mut num = 0.0f64;
            let mut de = 0.0f64;
            let mut ds = 0.0f64;
            for (i, &v) in q.iter().enumerate() {
                let s = buf[i] as f64;
                let e = v as f64 / 32768.0 - s;
                num += e * s;
                de += e * e;
                ds += s * s;
            }
            (num / (de.sqrt() * ds.sqrt() + 1e-30)).abs()
        };
        let corr_with = corr(&with);
        let corr_without = corr(&without);

        assert!(
            corr_without > 0.7,
            "en odithrad svag ton ska ha felet klistrat vid signalen, var {corr_without}"
        );
        assert!(
            corr_with < 0.2,
            "dithrad fil ska ha okorrelerat fel, var {corr_with}"
        );

        // Och dithern får inte skada materialet: på normal nivå ska varje sample
        // ligga inom ett kvantsteg från originalet.
        let mut loud = Vec::with_capacity(2000);
        for i in 0..2000 {
            loud.push(0.2 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 44_100.0).sin());
        }
        let loud_path = dir.join("normal_niva.wav");
        write_export_with(
            &loud_path.to_string_lossy(),
            ExportFormat::Wav16,
            &loud,
            44_100,
            &meta(),
            DitherSettings::default(),
        )
        .unwrap();
        let read_back = read_wav_i16(&loud_path);
        for (i, &v) in read_back.iter().enumerate() {
            let diff = (v as f64 / 32768.0 - loud[i] as f64).abs();
            assert!(
                diff <= 1.5 / 32768.0,
                "sample {i} avvek {diff} (mer än ett kvantsteg)"
            );
        }

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Skriver två 16-bitarsfiler till /tmp — en dithrad och en odithrad svag ton
    /// — för extern kontroll med `ffprobe`/`ffmpeg` och spektrummätning.
    /// Ignoreras av vanliga testkörningar:
    /// `cargo test --locked --bin sonix -- --ignored dump_dither`.
    #[test]
    #[ignore]
    fn dump_dither_16bit_sample_for_external_check() {
        // −60 dBFS: svagt nog att kvantiseringsfelet syns i spektrumet.
        // Bufferten är **interleaved stereo**, precis som `render_buffer` ger —
        // en monobuffer skulle bli halva samplingsfrekvensen i filen.
        let n = 44_100;
        let amp = 0.001;
        let mut buf: Vec<f32> = Vec::with_capacity(n * 2);
        for i in 0..n {
            let s = amp * (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44_100.0).sin();
            buf.push(s);
            buf.push(s);
        }
        write_export_with(
            "/tmp/sonix_dither_med.wav",
            ExportFormat::Wav16,
            &buf,
            44_100,
            &meta(),
            DitherSettings::default(),
        )
        .unwrap();
        write_export_with(
            "/tmp/sonix_dither_utan.wav",
            ExportFormat::Wav16,
            &buf,
            44_100,
            &meta(),
            DitherSettings { enabled: false, noise_shaping: false, seed: 1 },
        )
        .unwrap();
        write_export_with(
            "/tmp/sonix_dither_shaping.wav",
            ExportFormat::Wav16,
            &buf,
            44_100,
            &meta(),
            DitherSettings { enabled: true, noise_shaping: true, seed: 1 },
        )
        .unwrap();
        println!("skrev tre filer i /tmp ({} samples, {:.0} dBFS)", n, 20.0 * amp.log10());
    }

    #[test]
    fn flac_roundtrip_with_clean_tags() {
        if !ffmpeg_available() {
            eprintln!("ffmpeg saknas, hoppar flac-test");
            return;
        }
        let dir = std::env::temp_dir().join("sonix_export_test_flac");
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.flac");
        let buf = sine_buffer(44100, 0.3);
        write_export(&p.to_string_lossy(), ExportFormat::Flac, &buf, 44100, &meta()).unwrap();
        assert!(p.metadata().unwrap().len() > 100);
        let probe = ffmpeg_probe(&p.to_string_lossy());
        assert!(probe.contains("flac"), "inte flac: {}", probe);
        // Clean tags present; software marker only, no AI/provider names.
        for tag in ["TITLE", "Testlåt", "ARTIST", "Sonix", "ALBUM", "Album", "GENRE", "Test", "DATE", "2026", "ENCODER"] {
            assert!(probe.contains(tag), "saknar tag {}: {}", tag, probe);
        }
        assert!(probe.contains("Sonix Studio"), "saknar Software-markör: {}", probe);
        for banned in ["suno", "chatgpt", "claude", "openai", "anthropic", "gemini", "copilot"] {
            assert!(!probe.to_lowercase().contains(banned), "hittade {} i metadata", banned);
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn compressed_formats_via_ffmpeg() {
        if !ffmpeg_available() {
            eprintln!("ffmpeg saknas, hoppar komprimerings-test");
            return;
        }
        let dir = std::env::temp_dir().join("sonix_export_test_cmp");
        std::fs::create_dir_all(&dir).unwrap();
        let buf = sine_buffer(44100, 0.3);
        for fmt in [ExportFormat::Mp3, ExportFormat::Ogg, ExportFormat::Aac] {
            let p = dir.join(format!("t.{}", fmt.ext()));
            write_export(&p.to_string_lossy(), fmt, &buf, 44100, &meta()).unwrap();
            assert!(p.metadata().unwrap().len() > 100);
            let probe = ffmpeg_probe(&p.to_string_lossy());
            let lower = probe.to_lowercase();
            for banned in ["suno", "chatgpt", "claude", "openai", "anthropic", "gemini", "copilot"] {
                assert!(!lower.contains(banned), "hittade {} i metadata: {}", banned, probe);
            }
            assert!(probe.contains("Testlåt") || lower.contains("testlåt"), "saknar titel");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// Skriver en stämma (separat stereopar) som en 32-bitars flyttals-wav (8.5a).
///
/// `write_wav` tar **interleaved** stereo — den räknar `channels = 2` själv — så
/// `left`/`right` måste flätas `L0 R0 L1 R1 …` innan de lämnas vidare. 32-bitars
/// float är samma väg som exporterarens egen `Wav32`, alltså rätt format för en
/// arbetsfil: ingen dither, ingen kvantisering, inget att förlora.
///
/// Sökvägen kommer färdig från anroparen: **bara `paths.rs` får bygga sökvägar**
/// (6.0), så den här funktionen äger bara flätningen och skrivningen.
///
/// Varför den finns: stämseparatorn lämnade tidigare inga filer alls — bara en
/// grov översikt och en `source_path` till originalet. Var originalet en mp3,
/// som appen inte kan avkoda, blev stämman ett tyst klipp med trovärdig vågform.
pub fn write_stem_wav(
    path: &str,
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
) -> Result<(), String> {
    let frames = left.len().min(right.len());
    if frames == 0 {
        return Err("stämman är tom".to_string());
    }
    let mut interleaved = Vec::with_capacity(frames * 2);
    for i in 0..frames {
        interleaved.push(left[i]);
        interleaved.push(right[i]);
    }
    let meta = ExportMeta {
        title: std::path::Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default(),
        artist: String::new(),
        album: String::new(),
        genre: String::new(),
        year: String::new(),
        comment: String::new(),
    };
    write_wav(
        path,
        &interleaved,
        sample_rate,
        32,
        true,
        &meta,
        DitherSettings::default(),
    )
}

#[cfg(test)]
mod stem_wav_tests {
    use super::*;

    /// En stämma som skrivs till disk ska gå att läsa tillbaka med appens egen
    /// avkodare — samma längd, och samplen på plats. Det är hela poängen: en
    /// stämma som finns som fil kan aldrig bli ett tyst klipp.
    #[test]
    fn a_stem_written_to_disk_reads_back_with_the_same_audio() {
        let dir = std::env::temp_dir().join("sonix_stem_wav_test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("Stämma (Test).wav");
        let path_str = path.to_string_lossy().to_string();

        let n = 1000;
        let left: Vec<f32> = (0..n).map(|i| (i as f32 / n as f32) * 0.5).collect();
        let right: Vec<f32> = (0..n).map(|i| -((i as f32 / n as f32) * 0.5)).collect();

        write_stem_wav(&path_str, &left, &right, 44100).expect("skrivningen ska lyckas");
        assert!(path.exists(), "filen ska finnas på disk");

        let (l, r, sr) = crate::audio::load_wav_pcm(&path_str).expect("filen ska gå att läsa");
        assert_eq!(sr, 44100);
        assert_eq!(l.len(), n, "längden ska bevaras");
        assert_eq!(r.len(), n);
        // En 32-bitars float-wav är förlustfri: samplen ska komma tillbaka som de var.
        for i in [0, 1, n / 2, n - 1] {
            assert!(
                (l[i] - left[i]).abs() < 1e-6,
                "vänsterkanalen vid {i}: {} mot {}",
                l[i],
                left[i]
            );
            assert!(
                (r[i] - right[i]).abs() < 1e-6,
                "högerkanalen vid {i}: {} mot {}",
                r[i],
                right[i]
            );
        }
        // Osymmetri ska bevaras — en stämma är inte ett symmetriskt hölje.
        assert!(l[500] > 0.0 && r[500] < 0.0, "kanalerna ska inte blandas ihop");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
