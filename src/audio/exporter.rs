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

use super::command::{AudioCommand, StemRegionPlayback};
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
}

pub fn midi_to_freq(note: u8) -> f32 {
    440.0 * 2.0_f32.powf((note as f32 - 69.0) / 12.0)
}

fn sample_trigger_command(ch: &RackChannel, note: u8, velocity: f32) -> Option<AudioCommand> {
    let v = ch.voice.as_ref()?;
    if v.left.is_empty() {
        return None;
    }
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
        start01: v.start,
        end01: v.end,
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
fn triggers_for_step(spec: &RenderSpec, bar: usize, sib: usize) -> Vec<AudioCommand> {
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
        engine.handle_command(AudioCommand::SetTrackEq {
            track_index: t.track_index,
            settings: t.eq,
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
    engine.filter.reset();
    engine.delay_params = fx.delay;
    engine.delay = super::effects::StereoDelay::new(engine.sample_rate);
    engine.reverb_params = fx.reverb;
    engine.drive = fx.drive.clamp(1.0, 10.0);
    engine.master_fx.set_params(fx.master_fx);
    engine.master_volume = fx.master_volume.clamp(0.0, 1.0);
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

pub fn write_export(
    path: &str,
    format: ExportFormat,
    samples_lr: &[f32],
    sample_rate: u32,
    meta: &ExportMeta,
) -> Result<(), String> {
    match format {
        ExportFormat::Wav16 => write_wav(path, samples_lr, sample_rate, 16, false, meta),
        ExportFormat::Wav24 => write_wav(path, samples_lr, sample_rate, 24, false, meta),
        ExportFormat::Wav32 => write_wav(path, samples_lr, sample_rate, 32, true, meta),
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

fn write_wav(
    path: &str,
    samples: &[f32],
    sample_rate: u32,
    bits: u16,
    is_float: bool,
    meta: &ExportMeta,
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
        for &s in samples {
            let v = (clamp01(s) * 32767.0) as i16;
            file.extend_from_slice(&v.to_le_bytes());
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
    write_wav(&tmp_path.to_string_lossy(), samples, sample_rate, 24, false, meta)?;

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
