pub mod ai_client;
pub mod ai_generator;
pub mod command;
pub mod drum;
pub mod effects;
pub mod engine;
pub mod envelope;
pub mod exporter;
pub mod factory_samples;
pub mod filter;
pub mod hardware_control;
pub mod loudness;
pub mod master_fx;
pub mod midi_input;
// Helpers here are exercised by the optional `neural` backend and by unit
// tests; in the default (non-neural) build several are intentionally unused.
#[allow(dead_code)]
pub mod neural_separator;
pub mod patcher;
pub mod plugin_gui;
pub mod plugin_host;
// The live CLAP host's ABI code is feature-gated; the shared data types and
// `inspect()` are always compiled so the UI can show an honest status.
#[allow(dead_code)]
pub mod plugin_host_live;
pub mod recorder;
pub mod stem_separator;
pub mod synth;
pub mod vocal_harmonizer;
pub mod wav_reader;
pub mod wav_writer;

#[allow(unused_imports)]
pub use command::{AudioCommand, Preset, StemRegionPlayback, Waveform};
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
    write_export, ExportFormat, ExportMeta, FxState, PatternSnap, RackChannel, RenderSpec,
    TrackAudioSnap, TrackRole, TrackSnap, VoiceSpec,
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
    CompressorParams, DeEsserParams, DoublerParams, GateParams, LimiterParams, MasterEqBand,
    MasterFilterParams, MasterFxChain, MasterFxParams, StereoEq, TrackEqSettings,
};
#[allow(unused_imports)]
pub use synth::SynthEngine;
#[allow(unused_imports)]
pub use wav_reader::{load_audio_pcm, load_wav_pcm, read_wav_envelope};
pub use wav_writer::write_pcm_f32_to_wav;
