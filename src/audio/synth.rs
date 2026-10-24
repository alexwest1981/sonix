//! Status: stabil — motorn: mixer, bussar, sidokedjor (8.3) och sends
//! Rör inte: körordningen — `stem_order` och `stem_incoming` byggs i **samma** pass, så sändarens utgång finns när mottagarens kedja kör (8.3; provet `a_track_send_arrives_in_phase`).
//!
//! Här bor bara **roten**: `SynthEngine`-tillståndet, `new` och kommandots namn.
//! Resten är delat efter område (2026-09-14 — modulen var 3 517 rader): `process`
//! (renderingsloopen), `commands` (kommandovägen och körordningen), `voices` (rösterna,
//! spåren och samplingarna), `tests` (motorns prov).
use std::f32::consts::PI;
use std::sync::Mutex;
use super::command::{
    AudioCommand, FmParams, LoopMode, Preset, SendTarget, StemRegionPlayback, StemSend, Waveform,
    loop_frames,
};
use super::drum::{DrumType, DrumVoice};
use super::effects::{DelayParams, ReverbParams, SimpleReverb, StereoDelay};
use super::envelope::{AdsrParams, AdsrVoice};
use super::filter::{FilterParams, StateVariableFilter};
use super::master_fx::{Compressor, CompressorParams, GatePatternParams, MasterFxChain, RemixFx, StepGate, StereoEq, TapeStop, TrackEqSettings};
use super::patcher::{PatchProcessor, PatchSpec};
use super::plugin_host_live::{PdcDelay, PluginInsert};
use super::vocal_harmonizer::{FormantPitchShifter, Wsola};

mod commands;
mod process;
mod voices;

use voices::*;

/// Felsökningsloggning till `~/.local/state/sonix/logs/audio_debug.log`.
/// Påslagen när `SONIX_AUDIO_DEBUG` är satt, eller när markörfilen
/// `~/.local/state/sonix/debug_on` finns (så vanliga skrivbordsstarter kan
/// logga). Den äldre markören `~/Music/Sonix/debug_on` läses fortfarande.
fn dbg_enabled() -> bool {
    if std::env::var("SONIX_AUDIO_DEBUG").is_ok() {
        return true;
    }
    let paths = crate::paths::paths();
    paths.debug_marker_file().exists() || paths.legacy_library_file("debug_on").exists()
}

fn dbg_log(tag: &str, msg: &str) {
    if !dbg_enabled() {
        return;
    }
    let path = crate::paths::paths().log_file("audio_debug.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        use std::io::Write;
        let _ = writeln!(f, "[{}] {}: {}", std::process::id(), tag, msg);
    }
}

const MAX_VOICES: usize = 16;
const MAX_DRUMS: usize = 10;
const MAX_SAMPLE_VOICES: usize = 24;

/// Number of sub-mix buses (Vocal, Drum, Synth, FX) — Fas 5.2.
pub const NUM_BUSES: usize = 4;
/// Number of VCA control groups — Fas 5.2.
pub const NUM_VCAS: usize = 4;

/// Human-readable names for the four sub-mix buses, used by the mixer UI.
pub const BUS_NAMES: [&str; NUM_BUSES] = ["Vocal", "Trummor", "Synth", "FX"];

/// One-line description of a command, WITHOUT dumping Arc/PCM contents.
fn describe_cmd(cmd: &AudioCommand) -> String {
    match cmd {
        AudioCommand::StopAll => "StopAll".to_string(),
        AudioCommand::SetSongPlayback(playing) => format!("SetSongPlayback({})", playing),
        AudioCommand::SetMasterVolume(v) => format!("SetMasterVolume({})", v),
        AudioCommand::ClearAllStemTracks => "ClearAllStemTracks".to_string(),
        AudioCommand::LoadStemTrack { track_index, left, right, sample_rate, volume, pan, start_time_secs } => {
            format!(
                "LoadStemTrack(idx={} sr={:.0} vol={} pan={} start={:.2}s lenL={} lenR={})",
                track_index,
                sample_rate,
                volume,
                pan,
                start_time_secs,
                left.len(),
                right.len()
            )
        }
        AudioCommand::SetStemTrackState { track_index, volume, pan, muted, solo } => {
            format!("SetStemTrackState(idx={} vol={} pan={} muted={} solo={})", track_index, volume, pan, muted, solo)
        }
        AudioCommand::SetStemTrackRegions { track_index, regions } => {
            format!("SetStemTrackRegions(idx={} nregions={})", track_index, regions.len())
        }
        AudioCommand::SetStemTrackRouting { track_index, bus, vca } => {
            format!("SetStemTrackRouting(idx={} bus={} vca={:?})", track_index, bus, vca)
        }
        AudioCommand::SetStemTrackSends { track_index, sends } => {
            format!("SetStemTrackSends(idx={} antal={})", track_index, sends.len())
        }
        AudioCommand::SetStemTrackSidechain { track_index, from, amount_db, threshold_db } => {
            format!("SetStemTrackSidechain(idx={} from={:?} amount={} threshold={})", track_index, from, amount_db, threshold_db)
        }
        AudioCommand::SetBusState { bus, volume, muted, solo } => {
            format!("SetBusState(bus={} vol={} muted={} solo={})", bus, volume, muted, solo)
        }
        AudioCommand::SetVcaState { vca, volume, muted, solo } => {
            format!("SetVcaState(vca={} vol={} muted={} solo={})", vca, volume, muted, solo)
        }
        AudioCommand::SeekSongPosition(s) => format!("SeekSongPosition({:.2}s)", s),
        other => format!("{}", variant_name(other)),
    }
}

fn variant_name(cmd: &AudioCommand) -> &'static str {
    match cmd {
        AudioCommand::SetModulation { .. } => "SetModulation",
        AudioCommand::SetUnison { .. } => "SetUnison",
        AudioCommand::SetGlide { .. } => "SetGlide",
        AudioCommand::SetFilterDrive { .. } => "SetFilterDrive",
        AudioCommand::SetSub { .. } => "SetSub",
        AudioCommand::SetControlRoom(_) => "SetControlRoom",
        AudioCommand::SetHarmonics { .. } => "SetHarmonics",
        AudioCommand::SetPartialEnvelopes { .. } => "SetPartialEnvelopes",
        AudioCommand::SetSpectralFormant { .. } => "SetSpectralFormant",
        AudioCommand::NoteOn { .. } => "NoteOn",
        AudioCommand::NoteOnDelayed { .. } => "NoteOnDelayed",
        AudioCommand::NoteOff { .. } => "NoteOff",
        AudioCommand::ChannelPitchBend { .. } => "ChannelPitchBend",
        AudioCommand::BendDelayed { .. } => "BendDelayed",
        AudioCommand::StrumChord { .. } => "StrumChord",
        AudioCommand::TriggerDrum(_) => "TriggerDrum",
        AudioCommand::SetWaveform(_) => "SetWaveform",
        AudioCommand::SetFm(_) => "SetFm",
        AudioCommand::SetConvolverIr(_) => "SetConvolverIr",
        AudioCommand::SetAdsr(_) => "SetAdsr",
        AudioCommand::SetFilter(_) => "SetFilter",
        AudioCommand::SetFilterEnv { .. } => "SetFilterEnv",
        AudioCommand::SetDelay(_) => "SetDelay",
        AudioCommand::SetReverb(_) => "SetReverb",
        AudioCommand::SetDrive(_) => "SetDrive",
        AudioCommand::SetMasterFx(_) => "SetMasterFx",
        AudioCommand::SetTrackEq { .. } => "SetTrackEq",
        AudioCommand::SetTrackMix { .. } => "SetTrackMix",
        AudioCommand::SetRemixFx { .. } => "SetRemixFx",
        AudioCommand::SetGatePattern(_) => "SetGatePattern",
        AudioCommand::SetTapeStop { .. } => "SetTapeStop",
        AudioCommand::LoadPreset(_) => "LoadPreset",
        AudioCommand::SetMasterVolume(_) => "SetMasterVolume",
        AudioCommand::StopAll => "StopAll",
        AudioCommand::LoadStemTrack { .. } => "LoadStemTrack",
        AudioCommand::ClearAllStemTracks => "ClearAllStemTracks",
        AudioCommand::SetStemTrackSends { .. } => "SetStemTrackSends",
        AudioCommand::SetStemTrackState { .. } => "SetStemTrackState",
        AudioCommand::SetStemTrackRegions { .. } => "SetStemTrackRegions",
        AudioCommand::SetStemTrackRouting { .. } => "SetStemTrackRouting",
        AudioCommand::SetStemTrackSidechain { .. } => "SetStemTrackSidechain",
        AudioCommand::SetBusState { .. } => "SetBusState",
        AudioCommand::SetVcaState { .. } => "SetVcaState",
        AudioCommand::SeekSongPosition(_) => "SeekSongPosition",
        AudioCommand::SetSongPlayback(_) => "SetSongPlayback",
        AudioCommand::PlayAudition { .. } => "PlayAudition",
        AudioCommand::StopAudition => "StopAudition",
        AudioCommand::PlayScratch { .. } => "PlayScratch",
        AudioCommand::SetMonitorRing { .. } => "SetMonitorRing",
        AudioCommand::SetMonitorLevel(_) => "SetMonitorLevel",
        AudioCommand::SetAuditionParams { .. } => "SetAuditionParams",
        AudioCommand::TriggerSampleVoice { .. } => "TriggerSampleVoice",
        AudioCommand::SetPatcherGraph(_) => "SetPatcherGraph",
        AudioCommand::SetPatcherEnabled(_) => "SetPatcherEnabled",
        AudioCommand::PatcherNoteOn { .. } => "PatcherNoteOn",
        AudioCommand::PatcherNoteOff => "PatcherNoteOff",
        AudioCommand::SetTrackPlugin { .. } => "SetTrackPlugin",
        AudioCommand::SetPluginParameter { .. } => "SetPluginParameter",
        AudioCommand::SetPluginLatencyOffset { .. } => "SetPluginLatencyOffset",
        AudioCommand::SetPluginSmartDisable { .. } => "SetPluginSmartDisable",
        AudioCommand::SetPluginExtraOutputs { .. } => "SetPluginExtraOutputs",
    }
}

use std::sync::Arc;

/// Antal övertoner i den additiva oscillatorn. Sextiofyra partialer är Harmors bredd;
/// de ritas i dragstångspanelen och räknas i en tabell (se `AdditiveTable`).
pub const HARMONIC_COUNT: usize = 64;

/// Standardläget för dragstängerna: bara grundtonen utdragen.
pub const HARMONICS_DEFAULT: [f32; HARMONIC_COUNT] = {
    let mut h = [0.0f32; HARMONIC_COUNT];
    h[0] = 1.0;
    h
};

/// Längden på den additiva tabellen. 4 096 punkter ger den 64:e partialen 64 sampel per
/// period; interpolationsfelet ligger då kring en tusendel, långt under hörbarhet.
/// Måste vara en tvåpotens — `sine_at_cycle` maskar indexet i stället för att räkna modulo.
pub const ADDITIVE_TABLE_LEN: usize = 4096;

/// En partials amplitudkontur: `[attack s, decay s, sustain 0–1]`.
pub type PartialAdsr = [f32; 3];

/// Konturen som betyder "ingen egen envelop": ingen attack, ingen decay, full sustain.
/// Så länge alla partialer står här går rösten den snabba tabellvägen.
pub const PARTIAL_ENV_NEUTRAL: [PartialAdsr; HARMONIC_COUNT] = [[0.0, 0.0, 1.0]; HARMONIC_COUNT];

/// Sant när ingen partial har en egen kontur, alltså när tabellen räcker.
pub fn partial_envs_are_neutral(envs: &[PartialAdsr; HARMONIC_COUNT]) -> bool {
    envs.iter().all(|env| env[0] <= 0.0 && env[1] <= 0.0 && env[2] >= 1.0)
}

/// Nivån ur en partials kontur vid tiden `t` (sekunder): linjär attack, linjär decay mot
/// sustain. Release ligger på röstens egen envelop och rör inte den här kurvan.
pub fn partial_env_level(t: f32, env: PartialAdsr) -> f32 {
    let attack = env[0].max(0.0);
    let decay = env[1].max(0.0);
    let sustain = env[2].clamp(0.0, 1.0);
    if attack > 0.0 && t < attack {
        return (t / attack).clamp(0.0, 1.0);
    }
    if decay > 0.0 {
        let k = ((t - attack) / decay).clamp(0.0, 1.0);
        return 1.0 - (1.0 - sustain) * k;
    }
    sustain
}

/// **Spektralt filter** (steg 3 i `ADDITIVE-SYNTH.md`): en klangform som ligger över
/// partialindex, inte över Hz — därför flyttar den klangfärgen utan att röra tonhöjden.
/// `center` är partialnummer (1 = grundtonen), `width` hur många partialer bandet spänner
/// och `amount` hur mycket allt utanför bandet dämpas (0 = av, 1 = bara bandet kvar).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpectralFormant {
    pub center: f32,
    pub width: f32,
    pub amount: f32,
}

impl Default for SpectralFormant {
    fn default() -> Self {
        Self { center: 1.0, width: 4.0, amount: 0.0 }
    }
}

impl SpectralFormant {
    /// Gain för partial `partial` (1 = grundtonen).
    pub fn gain(&self, partial: usize) -> f32 {
        let amount = self.amount.clamp(0.0, 1.0);
        if amount <= 0.0 {
            return 1.0;
        }
        let x = (partial as f32 - self.center) / self.width.max(0.25);
        let band = (-0.5 * x * x).exp();
        1.0 - amount * (1.0 - band)
    }
}

/// Läser sinustabellen vid `cycle` varv (1,0 = ett varv), linjärt interpolerad.
/// Används av den långsamma additiva vägen, där varje partial har sin egen fas.
#[inline(always)]
pub fn sine_at_cycle(cycle: f32) -> f32 {
    let wrapped = cycle - cycle.floor();
    let x = wrapped * ADDITIVE_TABLE_LEN as f32;
    let i = (x as usize).min(ADDITIVE_TABLE_LEN - 1);
    let frac = x - i as f32;
    let s = sine_table();
    let next = (i + 1) & (ADDITIVE_TABLE_LEN - 1);
    s[i] + (s[next] - s[i]) * frac
}

/// Summan av alla partialer i en tabell, byggd när dragstängerna ändras och delad av alla
/// röster. En tabelläsning per sampel i stället för en `sin()` per partial gör att 64
/// partialer kostar lika mycket som 1 — mätt, se `ADDITIVE-SYNTH.md`.
///
/// shortcut: tabellen är gemensam och bandbegränsas inte, så en hög not kan vika partialer
/// ovanför Nyquist; bygg en tabell per oktav om det hörs.
pub struct AdditiveTable {
    samples: Vec<f32>,
}

impl AdditiveTable {
    pub fn new(levels: &[f32; HARMONIC_COUNT]) -> Self {
        let mut table = Self {
            samples: vec![0.0; ADDITIVE_TABLE_LEN + 1],
        };
        table.rebuild(levels);
        table
    }

    pub fn rebuild(&mut self, levels: &[f32; HARMONIC_COUNT]) {
        // Samma normalisering som den direkta summan: volymen ska inte bero på hur många
        // dragstänger som är utdragna.
        let weight: f32 = levels.iter().filter(|level| **level > 0.0).sum();
        let gain = if weight > 0.0 {
            1.0 / weight.max(1.0)
        } else {
            0.0
        };
        // Tabellen byggs ur en delad sinustabell. Med `sin()` per punkt tog en rattdragning
        // 4 096 × 64 anrop per bildruta, vilket hördes som hack.
        let sine = sine_table();
        for (j, sample) in self
            .samples
            .iter_mut()
            .take(ADDITIVE_TABLE_LEN)
            .enumerate()
        {
            let mut sum = 0.0;
            for (i, level) in levels.iter().enumerate() {
                if *level > 0.0 {
                    sum += *level * sine[(j * (i + 1)) % ADDITIVE_TABLE_LEN];
                }
            }
            *sample = sum * gain;
        }
        self.samples[ADDITIVE_TABLE_LEN] = self.samples[0];
    }

    /// Läser tabellen vid en fas i [0, 2π), linjärt interpolerad mellan punkterna.
    #[inline(always)]
    pub fn at_phase(&self, phase: f32) -> f32 {
        let x = (phase * (ADDITIVE_TABLE_LEN as f32 / std::f32::consts::TAU)).max(0.0);
        let i = (x as usize).min(ADDITIVE_TABLE_LEN - 1);
        let frac = x - i as f32;
        let a = self.samples[i];
        a + (self.samples[i + 1] - a) * frac
    }
}

fn sine_table() -> &'static [f32; ADDITIVE_TABLE_LEN] {
    static TABLE: std::sync::OnceLock<[f32; ADDITIVE_TABLE_LEN]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        std::array::from_fn(|j| {
            (std::f32::consts::TAU * j as f32 / ADDITIVE_TABLE_LEN as f32).sin()
        })
    })
}

pub struct SynthEngine {
    pub sample_rate: f32,
    /// **Control Room**: monitorläget. Neutralt tills den levande utgången säger annat —
    /// exportmotorn sätter det aldrig, så en dimmad monitor hamnar inte i en renderad fil.
    pub control_room: crate::audio::command::ControlRoomState,
    pub waveform: Waveform,
    pub adsr: AdsrParams,
    pub filter_params: FilterParams,
    /// Per-voice filter envelope depth in octaves and its shape.
    pub filter_env_amount: f32,
    pub filter_env: AdsrParams,
    pub delay_params: DelayParams,
    pub delay: StereoDelay,
    pub reverb_params: ReverbParams,
    pub reverb: SimpleReverb,
    pub drive: f32,
    pub master_fx: MasterFxChain,
    /// FM-inställningen för syntens röster (DX10/Sytrus-vägen).
    pub fm: FmParams,
    pub remix_fx: RemixFx,
    /// Steggrinden (Gross Beats volymkurva).
    pub step_gate: StepGate,
    pub gate_pattern: GatePatternParams,
    pub tape_stop: TapeStop,
    /// Modulationsmatrisen (FL:s Envelope/Peak/Keyboard Controller).
    pub modulation: crate::audio::modulation::ModMatrix,
    /// Nivåerna för den additiva oscillatorns övertoner. Endast `Waveform::Additive`
    /// läser dem; standard är bara grundtonen, så vågformen låter som en sinus tills
    /// användaren drar i dragstängerna.
    pub harmonics: [f32; HARMONIC_COUNT],
    /// Tabellen som rösterna läser: summan av `harmonics`, omräknad när de ändras.
    pub additive: AdditiveTable,
    /// Per-partial A/D/S för den additiva oscillatorn. Neutral = bara `harmonics` gäller.
    pub partial_envs: [PartialAdsr; HARMONIC_COUNT],
    /// Sant när minst en partial har en egen kontur; då går rösten den långsamma vägen.
    pub partial_envs_active: bool,
    /// Spektralt filter över partialindex. `amount = 0` lämnar ljudet orört.
    pub formant: SpectralFormant,
    /// Suboscillatorns nivå 0–1: en sinus en oktav under noten, efter filtret.
    pub sub_level: f32,
    /// Filterdrive 0–1: mjuk klippning efter röstens filter (0 = av, exakt som förut).
    pub filter_drive: f32,
    /// Key tracking 0–1: hur mycket filtrets brytfrekvens följer noten (0 = av).
    pub filter_key_track: f32,
    /// Portamento: tid i sekunder för fasökningen att glida till nästa not (0 = av).
    pub glide_time: f32,
    /// Frekvensen för senaste noten, startpunkt för gliden.
    pub last_note_freq: f32,
    /// Unison: hur många röster en not startar, och hur mycket de stäms isär i cent.
    /// Fler röster ger bredd och rörelse; en röst är exakt som förut.
    pub unison_voices: u8,
    pub unison_detune_cents: f32,
    /// Pitch bend per kanal, som multiplikator: index 0 är MPE-mastern, 1–16
    /// medlemskanalerna. En ny not startar med den böjning som gäller just nu —
    /// annars tappas en hållen pitch wheel vid nästa anslag.
    pub bend_ratio: [f32; 17],
    /// Modulationens tonhöjd i semitoner och filtrets brytfrekvens i oktaver, räknade
    /// en gång per sampel och lästa av rösterna. Noll när matrisen är tom.
    pub mod_pitch: f32,
    pub mod_cutoff: f32,
    pub master_volume: f32,
    pub voices: [Voice; MAX_VOICES],
    pub drums: [DrumVoice; MAX_DRUMS],
    // Multi-track audio stem streaming
    pub stem_tracks: Vec<StemVoiceTrack>,
    pub has_stem_solo: bool,
    /// Hur många spår som **faktiskt** har en sträckt fil i sin regionlista (Fas 8.10c).
    ///
    /// Räknas om när regioner eller spår byts — aldrig i sample-loopen. Appen läser
    /// talet och kan säga "motorn spelar 8 av 9" i stället för att anta att kommandot
    /// landade. **Mätt 2026-09-13:** `LoadStemTrack` byggde ett nytt spår och tömde
    /// regionerna tyst — Alex hörde "ingen märkbar skillnad på tempo" medan cachen var
    /// full av korrekta filer och vyn ritade som om de spelades.
    pub stretched_region_tracks: u32,
    pub song_playing: bool,
    pub song_time_samples: usize,
    // Isolated audition for Vocal Studio & Sound Browser
    pub audition: Option<AuditionVoice>,
    // Zero-latency microphone direct monitoring (post auto-tune mono samples)
    pub monitor_ring: Option<Arc<Mutex<Vec<f32>>>>,
    monitor_buf: Vec<f32>,
    monitor_idx: usize,
    /// Nivå för direktlyssningen (0.0–1.0), satt med `SetMonitorLevel`.
    monitor_level: f32,
    // Polyphonic WAV one-shot voices for the Channel Rack sample player
    pub sample_voices: [SampleVoice; MAX_SAMPLE_VOICES],
    pub sample_voice_cursor: usize,
    // Scheduled chord/strum notes (Chord Matrix audition)
    pub scheduled_notes: Vec<ScheduledNote>,
    /// Inspelade bend, samplenoggranna (samma kö-mönster som `scheduled_notes`).
    pub scheduled_bends: Vec<ScheduledBend>,
    // Modular Patcher DSP graph
    pub patcher: Option<PatchProcessor>,
    pub patcher_spec: Option<PatchSpec>,
    pub patcher_enabled: bool,
    /// PDC delay line for the synth/drum/sample bus (Fas 4.2).
    pub pdc_bus: PdcDelay,
    /// Sub-mix bus group gains / mute / solo (Fas 5.2).
    pub bus_volume: [f32; NUM_BUSES],
    pub bus_muted: [bool; NUM_BUSES],
    pub bus_solo: [bool; NUM_BUSES],
    /// VCA group gains / mute / solo (Fas 5.2).
    pub vca_volume: [f32; NUM_VCAS],
    pub vca_muted: [bool; NUM_VCAS],
    pub vca_solo: [bool; NUM_VCAS],
    /// **Ordningen stämmorna räknas i** (Fas 8.3): topologisk över spår-sends, så att en
    /// mottagare alltid räknas **efter** sina sändare. Räknas om när sends eller spår
    /// ändras — aldrig per sample.
    pub stem_order: Vec<usize>,
    /// Utgångarna för det **pågående samplet** (Fas 8.3). En spår-send läser sändarens
    /// utgång härifrån, så att den kommer fram exakt i fas i stället för en sample sent.
    /// Buffetarna återanvänds — ingen allokering per sample.
    pub track_out_l: Vec<f32>,
    pub track_out_r: Vec<f32>,
    /// **Vilka som skickar till ett spår** (Fas 8.3): för varje spår en lista av
    /// `(sändare, nivå)`. Byggs i samma pass som `stem_order` — samma graf, samma ställe —
    /// så att ordningen och vägarna inte kan driva isär. Sends ändras bara via
    /// `SetStemTrackSends`, som räknar om båda.
    pub stem_incoming: Vec<Vec<(usize, f32)>>,
    /// **Pluginens egna utbussar in i sina målspår** (Fas 8.6): för varje spår summan som
    /// källspårens pluginbussar lagt dit **för det här samplet**. Nollas i början av varje
    /// sample, så att en buss vars mål är tystat inte blir stående och läcker in i nästa.
    /// Bufferten återanvänds — ingen allokering per sample.
    pub plugin_bus_in: Vec<[f32; 2]>,
    // Debug heartbeat counters (only used when SONIX_AUDIO_DEBUG is set)
    pub dbg_frames: u64,
}

impl SynthEngine {
    /// Dragstängerna med det spektrala filtret pålagt — det är den här formen tabellen
    /// byggs av och som rösterna får. `formant.amount = 0` ger exakt `harmonics`.
    pub fn shaped_harmonics(&self) -> [f32; HARMONIC_COUNT] {
        let mut shaped = self.harmonics;
        for (k, level) in shaped.iter_mut().enumerate() {
            *level *= self.formant.gain(k + 1);
        }
        shaped
    }

    /// Bygger om tabellen och skickar den formade nivålistan till rösterna. Enda stället
    /// som rör dragstänger, formant eller förval — då kan tabellen och rösterna inte glida isär.
    pub fn rebuild_additive(&mut self) {
        let shaped = self.shaped_harmonics();
        self.additive.rebuild(&shaped);
        for voice in &mut self.voices {
            voice.harmonics = shaped;
            voice.partial_control = 0;
        }
    }

    pub fn new(sample_rate: f32) -> Self {
        let s = Preset::CleanPluck.settings();
        let (waveform, adsr, filter_params) = (s.waveform, s.adsr, s.filter);
        Self {
            sample_rate,
            control_room: crate::audio::command::ControlRoomState::default(),
            waveform,
            adsr,
            filter_params,
            filter_env_amount: 0.0,
            filter_env: AdsrParams::default(),
            delay_params: DelayParams::default(),
            delay: StereoDelay::new(sample_rate),
            reverb_params: ReverbParams::default(),
            reverb: SimpleReverb::new(sample_rate),
            drive: 1.0,
            master_fx: MasterFxChain::new(sample_rate),
            fm: FmParams::default(),
            remix_fx: RemixFx::new(sample_rate),
            step_gate: StepGate::new(sample_rate),
            gate_pattern: GatePatternParams::default(),
            tape_stop: TapeStop::new(sample_rate),
            master_volume: 0.8,
            modulation: crate::audio::modulation::ModMatrix::default(),
            harmonics: HARMONICS_DEFAULT,
            additive: AdditiveTable::new(&HARMONICS_DEFAULT),
            partial_envs: PARTIAL_ENV_NEUTRAL,
            partial_envs_active: false,
            formant: SpectralFormant::default(),
            sub_level: 0.0,
            filter_drive: 0.0,
            filter_key_track: 0.0,
            glide_time: 0.0,
            last_note_freq: 0.0,
            unison_voices: 1,
            unison_detune_cents: 12.0,
            bend_ratio: [1.0; 17],
            mod_pitch: 0.0,
            mod_cutoff: 0.0,
            voices: [Voice::new(sample_rate); MAX_VOICES],
            drums: [DrumVoice::new(sample_rate); MAX_DRUMS],
            stem_tracks: Vec::new(),
            has_stem_solo: false,
            stretched_region_tracks: 0,
            song_playing: false,
            song_time_samples: 0,
            audition: None,
            monitor_ring: None,
            monitor_buf: Vec::new(),
            monitor_idx: 0,
            monitor_level: 1.0,
            sample_voices: std::array::from_fn(|_| SampleVoice::new()),
            sample_voice_cursor: 0,
            scheduled_notes: Vec::new(),
            scheduled_bends: Vec::new(),
            patcher: None,
            patcher_spec: None,
            patcher_enabled: false,
            pdc_bus: PdcDelay::new(),
            bus_volume: [1.0; NUM_BUSES],
            bus_muted: [false; NUM_BUSES],
            bus_solo: [false; NUM_BUSES],
            vca_volume: [1.0; NUM_VCAS],
            vca_muted: [false; NUM_VCAS],
            vca_solo: [false; NUM_VCAS],
            stem_order: Vec::new(),
            track_out_l: Vec::new(),
            track_out_r: Vec::new(),
            stem_incoming: Vec::new(),
            plugin_bus_in: Vec::new(),
            dbg_frames: 0,
        }
    }

    fn dbg_state(&self, tag: &str, msg: &str) {
        let voices = self.voices.iter().filter(|v| v.is_active()).count();
        let drums = self.drums.iter().filter(|d| d.active).count();
        let samples = self.sample_voices.iter().filter(|v| v.is_active()).count();
        dbg_log(
            tag,
            &format!(
                "{} | song_playing={} t={:.2}s stems={} voices={} drums={} samples={} audition={} master={:.3}",
                msg,
                self.song_playing,
                self.song_time_samples as f32 / self.sample_rate,
                self.stem_tracks.len(),
                voices,
                drums,
                samples,
                self.audition.as_ref().map(|a| a.is_playing).unwrap_or(false),
                self.master_volume,
            ),
        );
    }

}

#[cfg(test)]
mod tests;
