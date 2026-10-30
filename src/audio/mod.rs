/// Stereoljud: vänster, höger och samplingsfrekvens. Aliaset finns för att en tuple av tre
/// element som upprepas i tio signaturer inte säger något om vad den är.
pub type StereoPcm = (std::sync::Arc<Vec<f32>>, std::sync::Arc<Vec<f32>>, u32);

pub mod ai_client;
pub mod builtin_fx;
pub mod ai_features;
pub mod ai_generator;
pub mod ai_tools;
pub mod command;
pub mod project_script;
pub mod resample;
pub mod script;
pub mod control_room;
pub mod ddp;
pub mod dither;
pub mod drum;
pub mod edison;
pub mod effects;
pub mod engine;
pub mod envelope;
pub mod exporter;
pub mod factory_samples;
pub mod filter;
pub mod filter_topologies;
pub mod flex;
pub mod fsc;
pub mod hardware_control;
pub mod keymap;
pub mod key_mapper;
pub mod loop_station;
pub mod launcher;
pub mod loudness;
pub mod macro_chain;
pub mod master_fx;
pub mod metadata;
pub mod phrase_preview;
pub mod asd;
pub mod jsfx;
pub mod spectral_edit;
pub mod stft;
pub mod tuning;
pub mod video;
pub mod warp;
pub mod modulation;
pub mod input_profile;
pub mod midi_input;
pub mod midi_output;
// Helpers here are exercised by the optional `neural` backend and by unit
// tests; in the default (non-neural) build several are intentionally unused.
#[allow(dead_code)]
pub mod neural_separator;
pub mod newtime;
pub mod patcher;
pub mod pedal_resonance;
pub mod plugin_gui;
pub mod plugin_host;
#[cfg(feature = "plugin-host")]
pub mod plugin_sandbox;
#[cfg(feature = "plugin-host")]
pub mod sandbox_audio;
pub mod sf2;
// Minimal in-process VST3 host (Fas 4.6). Feature-gated like the CLAP ABI.
#[cfg(feature = "plugin-host")]
pub mod plugin_vst3;
// Minimal in-process VST2 host (Fas 4.6c). Feature-gated like the CLAP/VST3 ABIs.
#[cfg(feature = "plugin-host")]
pub mod plugin_vst2;
// Minimal in-process LV2 host (dlopen + `lv2_descriptor`). Feature-gated like the
// CLAP/VST2/VST3 ABIs.
#[cfg(feature = "plugin-host")]
pub mod lv2;
// The live CLAP host's ABI code is feature-gated; the shared data types and
// `inspect()` are always compiled so the UI can show an honest status.
#[allow(dead_code)]
pub mod plugin_host_live;
/// Realtidsmätningen (Fas 7.2) används av testerna — den är ett mätverktyg, inte
/// en del av appen, så den byggs bara i testläget.
#[cfg(test)]
pub mod realtime_bench;
pub mod onset;
pub mod recorder;
pub mod repair;
pub mod smf;
pub mod tempo;
pub mod stem_separator;
pub mod stretch;
// Optional Rubber Band backend (dlopen) — WSOLA in `stretch` stays the default.
pub mod rubber_band;
pub mod spectral_image;
pub mod timbre;
pub mod spectrum;
pub mod synth;
pub mod vocal_harmonizer;
pub mod wav_reader;
pub mod wav_writer;
pub mod scale;
pub mod waveform;

#[allow(unused_imports)]
pub use command::{
    AudioCommand, LoopMode, Preset, SendTarget, StemRegionPlayback, StemSend, Waveform,
    loop_frames,
};
#[allow(unused_imports)]
pub use drum::DrumType;
#[allow(unused_imports)]
pub use effects::{DelayParams, ReverbParams};
#[allow(unused_imports)]
pub use engine::{AudioEngine, AudioSettings};
#[allow(unused_imports)]
pub use envelope::AdsrParams;
#[allow(unused_imports)]
pub use exporter::{
    build_offline_engine, load_timeline_into_engine, render_project_offline, sanitize_filename,
    write_export, write_export_with, write_stem_wav, DitherSettings, ExportFormat, ExportMeta, FxState, PatternSnap, RackChannel, RenderSpec,
    SynthVoiceState, TrackAudioSnap, TrackRole, TrackSnap, VoiceSpec,
};
#[allow(unused_imports)]
pub use filter::FilterParams;
#[allow(unused_imports)]
pub use hardware_control::{
    control_channel, osc_to_event, parse_osc_message, ControlEvent, McuInput, OscArg, OscServer,
};
#[allow(unused_imports)]
pub use midi_input::{note_to_roll_offset, MidiKeyboardInput};
#[allow(unused_imports)]
pub use neural_separator::{is_available as neural_available, model_status as neural_model_status};
#[allow(unused_imports)]
pub use plugin_host_live::{
    inspect as inspect_plugin, is_available as plugin_host_available, PluginInfo,
    PluginInspection, PluginParameter,
};
#[allow(unused_imports)]
pub use loudness::{
    integrated_lufs, normalize_loudness, normalize_to_preset, true_peak_db, LoudnessPreset,
};
#[allow(unused_imports)]
pub use master_fx::{
    BitCrushParams, BitCrusher, ControlRoomParams, FreqShiftParams, FrequencyShifter,
    DelayBank, DelayBankParams, Distructor, DistructorParams, Effector, EffectorParams, Hardcore,
    HardcoreParams, Granular, GranularParams, MultibandDelay, MultibandDelayParams,
    CompressorParams, ConvolverParams, DeEsserParams, DoublerParams, GateParams, LimiterParams, MasterEqBand,
    MasterFilterParams, LovePhilterParams, LuxeverbParams, EmphasisParams, MasterFxChain, MasterFxParams, ModFxParams, HyperChorusParams, TimeCurveParams, TimeShape, MultibandParams, SaturatorParams,
    StereoEq, StereoParams, Shelves, ShelvesParams,
    IR_BANK, synthesize_ir,
    TrackEqSettings, TransientParams,
};
#[allow(unused_imports)]
pub use synth::SynthEngine;
#[allow(unused_imports)]
pub use wav_reader::{load_audio_pcm, load_wav_pcm, read_wav_envelope};
pub use wav_writer::write_pcm_f32_to_wav;
