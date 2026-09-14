# SECTIONS.md — vad varje del av Sonix gör, och om den får röras

*Genererad av `tools/sections.py` — kör om den efter varje ändring: `python3 tools/sections.py`. Alla siffror är räknade ur koden; statusen ägs av modulens egen `//! Status:`-rad. Redigera inte det här dokumentet för hand.*

**63 moduler · 68606 rader kod · 19 med status · 44 utan.**

## Så läser du kartan

1. Leta upp modulen i tabellen. **Fryst/klart + många anropare = rör den inte** utan att läsa `Rör inte`-raden.
2. `Anropare` är hur många ställen utanför filen som nämner modulen — ett mått på hur dyrt ett misstag är, inte på hur viktig den är.
3. `Tester` är antalet `#[test]` i filen (`ign` = `#[ignore]`-körningar, som ofta är mätningar mot riktiga filer).
4. Statusen kommer från modulens egen dokumentation — **en ägare per faktum**. Saknas den står modulen i listan sist.

## Modulerna

| Modul | Rader | Status | Anropare | Tester | Vad den gör |
| :--- | ---: | :--- | ---: | :--- | :--- |
| `src/ui/app.rs` | 22832 | byggs — den stora ytan (UI + tillstånd). Kvar här: 8.7:s slice-UI, 8.10-vyns sträckt-märke, 8.4-samplern, 8.2:s fyra visningsställen | 5 | 85 (+1 ign) |  |
| `src/audio/synth.rs` | 3518 | stabil — motorn: mixer, bussar, sidokedjor (8.3) och sends | 97 | 49 |  |
| `src/i18n.rs` | 2530 | fryst — nycklar på engelska, texter på svenska | 1352 | 4 |  |
| `src/audio/plugin_host_live.rs` | 2424 | stabil (4.x) — CLAP-värden i egen process | 62 | 24 | In-process **CLAP** plugin host.  |
| `src/audio/exporter.rs` | 2125 | stabil — offline-rendering och export | 10 | 19 (+1 ign) | Offline full-project rendering and audio export for Sonix Studio.  |
| `src/audio/recorder.rs` | 1700 | stabil — inspelning | 7 | 16 |  |
| `src/audio/vocal_harmonizer.rs` | 1632 | — | 15 | 12 |  |
| `src/audio/stretch.rs` | 1627 | — | 17 | 24 (+8 ign) | Tempoföljning med bevarad tonhöjd (Fas 8.10, steg 2).  |
| `src/audio/factory_samples.rs` | 1575 | stabil — fabriksbiblioteket och `merge_library` | 9 | 10 |  |
| `src/audio/plugin_vst3.rs` | 1454 | stabil (4.6a) — VST3-värden, en utbuss | 4 | 9 | Minimal in-process **VST3** host (Fas 4.6a).  |
| `src/audio/metadata.rs` | 1451 | stabil (8.5b) — bara härkomst tas; musik, text och omslag lämnas | 4 | 18 | Tar bort AI-/Sunohärkomst ur ljudfiler — och ingenting annat (8.5b).  |
| `src/ui/plugins_view.rs` | 1059 | — | 1 | 0 |  |
| `src/audio/onset.rs` | 1040 | stabil — slagletning och slicekarta (8.7 steg 1 + 2) | 38 | 25 (+1 ign) | Onset-detektering och slicekarta (Fas 8.7).  |
| `src/ui/vocal_studio_view.rs` | 1027 | — | 1 | 0 |  |
| `src/audio/master_fx.rs` | 1016 | — | 6 | 9 | Real-time master bus FX chain and per-track equalizer DSP.  |
| `src/paths.rs` | 1008 | fryst — enda modulen som får bygga sökvägar | 46 | 14 | Kanoniska sökvägar för Sonix (Fas 6.0).  |
| `src/audio/plugin_sandbox.rs` | 945 | — | 9 | 8 | Out-of-process plugin sandbox (Fas 4.5a + 4.5b).  |
| `src/audio/plugin_vst2.rs` | 908 | — | 4 | 9 | Minimal in-process **VST2** host (Fas 4.6c).  |
| `src/audio/stem_separator.rs` | 885 | stabil (8.5a) — separatorn skriver stämmorna till disk och minns var de ligger | 6 | 11 |  |
| `src/audio/waveform.rs` | 851 | stabil — exakt hölje och flernivåcache (8.3) | 29 | 15 | Exakta vågformer: ett (min, max) per skärmpixel (Alex krav, Fas 8.3).  |
| `src/ui/fx_rack_modal.rs` | 838 | — | 1 | 0 |  |
| `src/audio/ai_generator.rs` | 790 | — | 6 | 7 |  |
| `src/audio/patcher.rs` | 782 | — | 4 | 6 |  |
| `src/audio/plugin_host.rs` | 757 | — | 3 | 7 |  |
| `src/ui/widgets.rs` | 734 | — | 4 | 0 |  |
| `src/audio/ai_client.rs` | 715 | — | 8 | 8 |  |
| `src/audio/smf.rs` | 706 | stabil — MIDI-export med tempobyten (8.2 steg 3) | 33 | 10 | Standard MIDI File (SMF) — skriv och läs `.mid` utan externa beroenden.  |
| `src/audio/command.rs` | 631 | byggs — kommando-protokollet (AI-vägen) | 22 | 7 |  |
| `src/audio/tempo.rs` | 631 | stabil — tempokartan och `region_source_secs` | 54 | 12 | Tempokarta (Fas 8.2, steg 1).  |
| `src/audio/sandbox_audio.rs` | 602 | — | 10 | 5 | Shared-memory audio transport for the out-of-process plugin sandbox (Fas 4.5b). |
| `src/midi_take.rs` | 564 | — | 29 | 27 | Inspelade nottagningar med sin faktiska tajming (Fas 6.4).  |
| `src/ui/chord_generator_modal.rs` | 516 | — | 1 | 0 |  |
| `src/audio/engine.rs` | 503 | — | 1 | 1 |  |
| `src/audio/realtime_bench.rs` | 501 | stabil — realtidsmätningen | 0 | 6 | Realtidsmätning av render-vägen (Fas 7.2).  |
| `src/audio/hardware_control.rs` | 493 | — | 3 | 5 |  |
| `src/ui/dice_generator_modal.rs` | 470 | — | 2 | 0 |  |
| `src/audio/neural_separator.rs` | 463 | — | 3 | 9 | Neural stem separation through an ONNX model (HTDemucs / Demucs family).  |
| `src/audio/plugin_gui.rs` | 462 | — | 2 | 3 | X11 window hosting for plugin GUIs (Fas 4.4b).  |
| `src/autosave.rs` | 430 | — | 42 | 10 | Autosave, versionsrotation och kraschåterställning (Fas 6.1).  |
| `src/ui/add_track_modal.rs` | 428 | — | 1 | 0 |  |
| `src/audio/loudness.rs` | 358 | — | 1 | 5 | Loudness measurement and normalization (ITU-R BS.1770 / EBU R128).  |
| `src/selftest.rs` | 353 | — | 1 | 4 | **Självtestet** (Fas 7.1) — den del av Windows-kriteriet som ingen CI-mätning kan svara på.  |
| `src/ui/tuner_modal.rs` | 351 | — | 1 | 0 |  |
| `src/audio/spectrum.rs` | 335 | ny (8.13) — toppradens nivå- och registermätare | 9 | 7 (+2 ign) | Registret i utgången — vilka frekvensband som bär energi — och mätarskalan (8.13).  |
| `src/audio/scale.rs` | 323 | stabil (8.11) — en tabell och ett index för tonarten | 18 | 8 | Tonarter: **en** tabell och **ett** index (Alex' kvittens 2026-09-12).  |
| `src/ui/patcher_view.rs` | 303 | — | 1 | 0 |  |
| `src/audio/dither.rs` | 296 | — | 5 | 8 | Dither vid kvantisering till fast punkt (Fas 6.5).  |
| `src/ui/song_structure_modal.rs` | 295 | — | 1 | 0 |  |
| `src/audio/wav_reader.rs` | 283 | — | 4 | 3 |  |
| `src/audio/midi_input.rs` | 275 | — | 2 | 10 | **MIDI-klaviatur in — en väg, `midir`, på alla plattformar** (Fas 7.1).  |
| `src/ui/ai_assistant_view.rs` | 260 | — | 1 | 0 |  |
| `src/main.rs` | 253 | — | 0 | 0 |  |
| `src/audio/drum.rs` | 252 | — | 5 | 3 |  |
| `src/audio/effects.rs` | 216 | — | 5 | 4 |  |
| `src/audio/envelope.rs` | 196 | — | 23 | 3 |  |
| `src/ui/stem_view.rs` | 185 | — | 1 | 0 |  |
| `src/audio/filter.rs` | 104 | — | 5 | 2 |  |
| `src/audio/mod.rs` | 104 | — | 0 | 0 |  |
| `src/rng.rs` | 82 | — | 2 | 4 | En liten deterministisk slumptalare (xorshift64\*).  |
| `src/platform.rs` | 63 | — | 2 | 1 | **Plattformens egna sätt** (Fas 7.1) — det som skiljer sig utan att vara sökvägar.  |
| `src/audio/wav_writer.rs` | 48 | — | 3 | 0 |  |
| `src/ui/theme.rs` | 30 | — | 13 | 0 |  |
| `src/ui/mod.rs` | 18 | — | 0 | 0 |  |

## Rör inte-regler och publika ingångar

### `src/audio/ai_client.rs`
- **Publika ingångar:** `AiProvider`, `ALL`, `label`, `requires_key`, `default_base`, `default_model`, `known_models`, `from_code`, `AudioProvider`, `ALL`, `label`, `requires_key` … (+16)

### `src/audio/ai_generator.rs`
- **Publika ingångar:** `AiGeneratedClip`, `GenContext`, `AiMusicAssistant`, `generate_preset_clips`, `generate_from_prompt`, `selected_genre_label`, `selected_key_label`, `selected_target_label`, `generate`, `request_remote_generation`, `poll_remote_generation`, `GenStyle` … (+1)

### `src/audio/command.rs`
- **Status:** byggs — kommando-protokollet (AI-vägen)
- **Rör inte:** nya kommandon ska gå genom `Command` så att agenten och UI:t gör samma sak
- **Publika ingångar:** `Waveform`, `name`, `Preset`, `settings`, `AudioCommand`, `bus`, `track`, `as_bus`, `as_track`, `plan_track_order`, `SendTarget`, `StemSend` … (+4)

### `src/audio/dither.rs`
- **Publika ingångar:** `DEFAULT_SEED`, `TpdfDither`, `new`, `with_noise_shaping`, `quantize_to_16`, `quantize_i16`

### `src/audio/drum.rs`
- **Publika ingångar:** `DrumType`, `DrumVoice`, `new`, `trigger`, `reset`, `next_sample`

### `src/audio/effects.rs`
- **Publika ingångar:** `DelayParams`, `StereoDelay`, `new`, `process`, `ReverbParams`, `SimpleReverb`, `new`, `process`

### `src/audio/engine.rs`
- **Publika ingångar:** `AudioEngine`, `AudioSettings`, `config_path`, `load`, `save`, `new`, `new_with`, `reconfigure`, `send_command`, `get_peak_level`, `song_position_secs`, `stretched_track_count` … (+4)

### `src/audio/envelope.rs`
- **Publika ingångar:** `EnvelopeStage`, `AdsrParams`, `identity`, `is_identity`, `AdsrVoice`, `new`, `gate_on`, `gate_off`, `reset`, `is_active`, `next_sample`

### `src/audio/exporter.rs`
- **Status:** stabil — offline-rendering och export
- **Rör inte:** exporten måste ge **samma ljud som högtalarna**; samma väg som uppspelningen, inte en parallell
- **Publika ingångar:** `VoiceSpec`, `RackChannel`, `PatternSnap`, `TrackRole`, `TrackSnap`, `TrackAudioSnap`, `FxState`, `RenderSpec`, `midi_to_freq`, `triggers_for_step`, `load_timeline_into_engine`, `render_project_offline` … (+12)

### `src/audio/factory_samples.rs`
- **Status:** stabil — fabriksbiblioteket och `merge_library`
- **Publika ingångar:** `GeneratedSample`, `new_stereo`, `compute_waveform_peaks`, `save_to_file`, `generate_all_factory_samples`, `ensure_factory_samples_directory`, `ScannedSampleItem`, `read_library_cache_items`, `ScanMsg`, `ScanPoll`, `LibraryScan`, `spawn` … (+3)

### `src/audio/filter.rs`
- **Publika ingångar:** `FilterParams`, `StateVariableFilter`, `new`, `reset`, `process_lowpass`

### `src/audio/hardware_control.rs`
- **Publika ingångar:** `ControlEvent`, `OscArg`, `parse_osc_message`, `osc_to_event`, `OscServer`, `start`, `received`, `McuInput`, `connect`, `received`, `device_list`, `connect` … (+3)

### `src/audio/loudness.rs`
- **Publika ingångar:** `LoudnessPreset`, `ALL`, `enabled`, `label`, `target_lufs`, `ceiling_dbtp`, `integrated_lufs`, `true_peak_db`, `normalize_loudness`, `normalize_to_preset`

### `src/audio/master_fx.rs`
- **Publika ingångar:** `TrackEqSettings`, `is_flat`, `StereoEq`, `new`, `set_settings`, `process`, `CompressorParams`, `Compressor`, `new`, `process`, `DoublerParams`, `Doubler` … (+35)

### `src/audio/metadata.rs`
- **Status:** stabil (8.5b) — bara härkomst tas; musik, text och omslag lämnas
- **Publika ingångar:** `id3v2_span`, `id3v1_offset`, `riff_metadata_spans`, `riff_has_audio`, `carries_provenance`, `CleanPlan`, `removed_bytes`, `removes_anything`, `plan_clean`, `TagReport`, `scan`, `strip_tags`

### `src/audio/midi_input.rs`
- **Publika ingångar:** `note_to_roll_offset`, `MidiKeyboardInput`, `control_events_from_midi`, `connect`, `received`, `device_list`

### `src/audio/neural_separator.rs`
- **Publika ingångar:** `DEMUCS_SAMPLE_RATE`, `DEFAULT_SEGMENT_SECONDS`, `DEFAULT_OVERLAP_SECONDS`, `NUM_SOURCES`, `NUM_CHANNELS`, `MODEL_ENV`, `MODEL_FILE_NAMES`, `model_dirs`, `find_model_in`, `find_model`, `model_status`, `is_available` … (+7)

### `src/audio/onset.rs`
- **Status:** stabil — slagletning och slicekarta (8.7 steg 1 + 2)
- **Rör inte:** mät på en jämn ton först: en detektor är en mätning, och 4 ms enpolsfilter + centrerad tröskel är den enda variant som ger noll falska slag
- **Publika ingångar:** `OnsetParams`, `Slice`, `onset_envelope`, `detect_onsets`, `music_start_source_secs`, `slices_from_onsets`, `nudge_slice_boundary`, `grid_capacity`, `detect_slice_map`, `spectral_flux_onsets`, `slices_to_steps`, `window_for_note`

### `src/audio/patcher.rs`
- **Publika ingångar:** `NodeType`, `PatcherNode`, `PatchCable`, `ModularGraph`, `load_default_preset`, `add_node`, `PatchNodeSpec`, `PatchSpec`, `to_spec`, `has_cycle`, `PatchProcessor`, `new` … (+4)

### `src/audio/plugin_gui.rs`
- **Publika ingångar:** `X11Window`, `open`, `window_id`, `resize`, `poll`, `is_alive`, `X11Window`, `open`, `window_id`, `resize`, `poll`, `is_alive` … (+7)

### `src/audio/plugin_host.rs`
- **Publika ingångar:** `PluginFormat`, `name`, `badge_color`, `PluginCategory`, `label`, `icon`, `PluginDescriptor`, `FlStudioPreset`, `ScanPath`, `PluginManager`, `detect_wine_version`, `detect_yabridge_installed` … (+7)

### `src/audio/plugin_host_live.rs`
- **Status:** stabil (4.x) — CLAP-värden i egen process
- **Rör inte:** en plugin i uppspelningsvägen får aldrig kunna tysta eller krascha motorn
- **Publika ingångar:** `PARAM_IS_STEPPED`, `PARAM_IS_PERIODIC`, `PARAM_IS_HIDDEN`, `PARAM_IS_READONLY`, `PARAM_IS_BYPASS`, `PARAM_IS_AUTOMATABLE`, `PARAM_IS_MODULATABLE`, `PluginParameter`, `is_stepped`, `is_periodic`, `is_hidden`, `is_readonly` … (+103)

### `src/audio/plugin_sandbox.rs`
- **Publika ingångar:** `WORKER_FLAG`, `SandboxRequest`, `SandboxResponse`, `SandboxInfo`, `SandboxParam`, `SandboxInspection`, `SandboxState`, `write_frame`, `read_frame`, `info_from_dto`, `param_from_dto`, `serve` … (+16)

### `src/audio/plugin_vst2.rs`
- **Publika ingångar:** `is_vst2_path`, `Vst2Instance`, `load`, `Vst2Processor`, `load_processor`, `inspect`

### `src/audio/plugin_vst3.rs`
- **Status:** stabil (4.6a) — VST3-värden, en utbuss
- **Publika ingångar:** `is_vst3_path`, `VstInstance`, `load`, `VstProcessor`, `load_processor`, `inspect`

### `src/audio/realtime_bench.rs`
- **Status:** stabil — realtidsmätningen
- **Rör inte:** release tillåter högst 2 missade block av 120; en spik är ett fel, inte brus
- **Publika ingångar:** `BUFFER_SIZES`, `BlockStats`, `from_durations`, `summary`, `reference_spec`, `reference_fx`, `frozen_spec`, `measure`

### `src/audio/recorder.rs`
- **Status:** stabil — inspelning
- **Publika ingångar:** `AudioTake`, `CustomSoundClip`, `RecordingMode`, `MicrophoneSettings`, `LiveMicrophoneCapture`, `HighPassFilter`, `new`, `set_cutoff_hz`, `set_cutoff`, `process`, `DC_BLOCKER_CUTOFF_HZ`, `LOW_CUT_CUTOFF_HZ` … (+40)

### `src/audio/sandbox_audio.rs`
- **Publika ingångar:** `DEFAULT_SLOTS`, `CHANNELS`, `BlockStatus`, `AudioBridge`, `create`, `attach`, `raw_fd`, `block_frames`, `slots`, `process_block`, `publish_input`, `take_output` … (+8)

### `src/audio/scale.rs`
- **Status:** stabil (8.11) — en tabell och ett index för tonarten
- **Rör inte:** en tabell, ett index, en test som räknar namnen — två listor för samma sak driver isär
- **Publika ingångar:** `ROOT_NAMES`, `Scale`, `SCALES`, `is_minor`, `scale_at`, `root_name`, `key_label`, `scale_notes`, `in_scale`, `nearest_in_scale`, `snap_row`

### `src/audio/smf.rs`
- **Status:** stabil — MIDI-export med tempobyten (8.2 steg 3)
- **Rör inte:** gyllene test: en orörd fil ska vara byte-identisk
- **Publika ingångar:** `PPQ`, `TICKS_PER_STEP_16TH`, `MidiNote`, `MidiTrack`, `write_midi`, `write_midi_with_tempo`, `ParsedTrack`, `ParsedMidi`, `notes_with_track`, `parse_midi`

### `src/audio/spectrum.rs`
- **Status:** ny (8.13) — toppradens nivå- och registermätare
- **Publika ingångar:** `REGISTERS`, `WINDOW`, `FLOOR_DB`, `CEIL_DB`, `db_to_unit`, `amp_to_unit`, `band_levels`, `smooth`

### `src/audio/stem_separator.rs`
- **Status:** stabil (8.5a) — separatorn skriver stämmorna till disk och minns var de ligger
- **Rör inte:** 8.5-regeln: en väg som skapar ett klipp eller en fil får aldrig hitta på ljud — säg fel och avbryt
- **Publika ingångar:** `StemType`, `name`, `color`, `StemChannel`, `StemAudio`, `StemProject`, `separate_stems`, `estimate_bpm`, `SeparationResult`, `run_separation`, `STEM_FILE_NAMES`, `write_stems_to_dir` … (+3)

### `src/audio/stretch.rs`
- **Publika ingångar:** `MIN_RATIO`, `MAX_RATIO`, `StretchPlan`, `changes_anything`, `plan`, `FollowMode`, `FollowDecision`, `decide`, `stretch_signalsmith`, `SIGNALSMITH_BLOCK_MS`, `SIGNALSMITH_INTERVAL_MS`, `stretch_signalsmith_with` … (+19)

### `src/audio/synth.rs`
- **Status:** stabil — motorn: mixer, bussar, sidokedjor (8.3) och sends
- **Rör inte:** sends mellan spår kräver att spårloopen i `process_stereo` delas i två faser + slingkontroll
- **Publika ingångar:** `NUM_BUSES`, `NUM_VCAS`, `BUS_NAMES`, `Voice`, `new`, `trigger`, `release`, `reset`, `is_active`, `next_sample`, `Ducker`, `new` … (+17)

### `src/audio/tempo.rs`
- **Status:** stabil — tempokartan och `region_source_secs`
- **Rör inte:** bit-exakt vid faktor 1,0; ändra bara med ett test som visar samma sak
- **Publika ingångar:** `STEPS_PER_BAR`, `TempoPoint`, `set_tempo_point`, `remove_tempo_point`, `TempoMap`, `single`, `from_points`, `points`, `is_single`, `bpm_at`, `secs_per_bar_at`, `secs_per_beat_at` … (+6)

### `src/audio/vocal_harmonizer.rs`
- **Publika ingångar:** `PitchBlob`, `HarmonyVoice`, `VocalHarmonizer`, `scale_mask`, `freq_to_midi`, `snap_midi_to_scale`, `pitch_shift_variable`, `pitch_shift`, `fft_radix2`, `formant_preserving_shift`, `Wsola`, `new` … (+32)

### `src/audio/wav_reader.rs`
- **Publika ingångar:** `read_wav_envelope`, `load_wav_pcm`, `load_audio_pcm`

### `src/audio/wav_writer.rs`
- **Publika ingångar:** `write_pcm_f32_to_wav`

### `src/audio/waveform.rs`
- **Status:** stabil — exakt hölje och flernivåcache (8.3)
- **Rör inte:** representationen ska bytas vid zoomtrösklar, aldrig göras finare kolumn för kolumn
- **Publika ingångar:** `envelope_per_pixel`, `FINEST_BUCKET`, `AmplitudeScale`, `DEFAULT_FLOOR_DB`, `amplitude_curve`, `ZoomRegime`, `DOTS_PIXELS_PER_SAMPLE`, `zoom_regime`, `PeakLevel`, `WaveformCache`, `build`, `level_count` … (+4)

### `src/autosave.rs`
- **Publika ingångar:** `KEEP_PER_PROJECT`, `INTERVAL_SECS`, `SUFFIX`, `RESTORED_SUFFIX`, `slug`, `file_name`, `parse_file_name`, `fingerprint`, `write_atomic`, `AutosaveEntry`, `worth_offering`, `save` … (+6)

### `src/i18n.rs`
- **Status:** fryst — nycklar på engelska, texter på svenska
- **Rör inte:** nya strängar går via `t()`/`tstatus!`, aldrig som literaler i UI:t
- **Publika ingångar:** `Language`, `all`, `code`, `native_name`, `from_code`, `detect_from_env`, `config_path`, `load_language`, `save_language`, `current`, `set_current`, `t` … (+6)

### `src/midi_take.rs`
- **Publika ingångar:** `STEPS_PER_BAR`, `SWING_MAX_FRACTION`, `TakeGrid`, `ALL`, `lines_per_bar`, `step_size`, `label`, `index`, `from_index`, `TakeNote`, `Take`, `new` … (+13)

### `src/paths.rs`
- **Status:** fryst — enda modulen som får bygga sökvägar
- **Rör inte:** nya sökvägar läggs här, aldrig hos anroparen
- **Publika ingångar:** `PlatformDefaults`, `APP_DIR`, `LIBRARY_DIR`, `ENV_PROJECTS_DIR`, `ENV_SAMPLES_DIR`, `ENV_CONFIG_DIR`, `ENV_DATA_DIR`, `ENV_STATE_DIR`, `ENV_CACHE_DIR`, `expand_tilde`, `parse_user_dir`, `Paths` … (+49)

### `src/platform.rs`
- **Publika ingångar:** `file_manager_command`, `open_dir`

### `src/rng.rs`
- **Publika ingångar:** `Rng`, `new`, `next_u64`, `next_f32`, `next_sym`

### `src/selftest.rs`
- **Publika ingångar:** `Check`, `ClockVerdict`, `clock_verdict`, `sound_was_heard`, `summary`, `run`

### `src/ui/add_track_modal.rs`
- **Publika ingångar:** `AddTrackCategory`, `TrackTemplate`, `get_track_templates`, `AddTrackModalState`, `render_add_track_modal`

### `src/ui/ai_assistant_view.rs`
- **Publika ingångar:** `render_ai_assistant_view`

### `src/ui/app.rs`
- **Status:** byggs — den stora ytan (UI + tillstånd). Kvar här: 8.7:s slice-UI, 8.10-vyns sträckt-märke, 8.4-samplern, 8.2:s fyra visningsställen
- **Rör inte:** två sessioner har krockat i den här filen; kolla `git status` och mtime före varje skrivning
- **Publika ingångar:** `ViewMode`, `ArrangerTool`, `RegionDragMode`, `RegionDragState`, `LibrarySampleItem`, `ChannelStrip`, `Pattern`, `TimeSnapMode`, `format_time_hundredths`, `format_bar_subdivisions`, `TrackKind`, `SongMarker` … (+183)

### `src/ui/chord_generator_modal.rs`
- **Publika ingångar:** `ScaleType`, `all`, `intervals`, `GeneratedChord`, `ChordGeneratorState`, `get_chord_pads`, `load_progression_preset`, `play_chord_sound`, `audition_progression`, `render_chord_generator_modal`

### `src/ui/dice_generator_modal.rs`
- **Publika ingångar:** `DiceCategory`, `GeneratedStepNote`, `DiceGeneratorState`, `roll_dice`, `render_dice_generator_modal`

### `src/ui/fx_rack_modal.rs`
- **Publika ingångar:** `VisualEqNode`, `FxPedal`, `FxRackState`, `load_preset`, `build_master_fx_params`, `sync_to_engine`, `render_fx_rack_modal`

### `src/ui/patcher_view.rs`
- **Publika ingångar:** `PatcherActions`, `render_patcher_view`

### `src/ui/plugins_view.rs`
- **Publika ingångar:** `PluginViewActions`, `render_plugins_view`

### `src/ui/song_structure_modal.rs`
- **Publika ingångar:** `SectionType`, `name_and_color`, `SongSectionItem`, `SongStructureState`, `load_preset`, `total_bars`, `render_song_structure_modal`

### `src/ui/stem_view.rs`
- **Publika ingångar:** `StemViewActions`, `render_stem_separator_view`

### `src/ui/theme.rs`
- **Publika ingångar:** `Theme`, `BG_DARK`, `HEADER_BG`, `PANEL_BG`, `CHANNEL_BG`, `LCD_BG`, `LCD_TEXT`, `LCD_ORANGE`, `FL_ORANGE`, `FL_GREEN`, `FL_RED`, `FL_CYAN` … (+5)

### `src/ui/tuner_modal.rs`
- **Publika ingångar:** `TuningPreset`, `all`, `strings`, `TunerState`, `render_tuner_modal`

### `src/ui/vocal_studio_view.rs`
- **Publika ingångar:** `render_vocal_studio_view`

### `src/ui/widgets.rs`
- **Publika ingångar:** `rotary_knob`, `rotary_knob_full`, `pitch_knob`, `fl_step_button`, `oscilloscope_display`, `vertical_fader`, `eq_curve_visualizer`, `mini_track_eq_curve`, `drummer_xy_matrix`, `alchemy_transform_matrix`, `stereo_meter`, `register_eq`

## Moduler utan status

De här är inte klassade än. Lägg till en `//! Status:`-rad i modulen (och `//! Rör inte:` om den har en sådan regel) — kör sedan om skriptet.

- `src/audio/ai_client.rs`
- `src/audio/ai_generator.rs`
- `src/audio/dither.rs`
- `src/audio/drum.rs`
- `src/audio/effects.rs`
- `src/audio/engine.rs`
- `src/audio/envelope.rs`
- `src/audio/filter.rs`
- `src/audio/hardware_control.rs`
- `src/audio/loudness.rs`
- `src/audio/master_fx.rs`
- `src/audio/midi_input.rs`
- `src/audio/mod.rs`
- `src/audio/neural_separator.rs`
- `src/audio/patcher.rs`
- `src/audio/plugin_gui.rs`
- `src/audio/plugin_host.rs`
- `src/audio/plugin_sandbox.rs`
- `src/audio/plugin_vst2.rs`
- `src/audio/sandbox_audio.rs`
- `src/audio/stretch.rs`
- `src/audio/vocal_harmonizer.rs`
- `src/audio/wav_reader.rs`
- `src/audio/wav_writer.rs`
- `src/autosave.rs`
- `src/main.rs`
- `src/midi_take.rs`
- `src/platform.rs`
- `src/rng.rs`
- `src/selftest.rs`
- `src/ui/add_track_modal.rs`
- `src/ui/ai_assistant_view.rs`
- `src/ui/chord_generator_modal.rs`
- `src/ui/dice_generator_modal.rs`
- `src/ui/fx_rack_modal.rs`
- `src/ui/mod.rs`
- `src/ui/patcher_view.rs`
- `src/ui/plugins_view.rs`
- `src/ui/song_structure_modal.rs`
- `src/ui/stem_view.rs`
- `src/ui/theme.rs`
- `src/ui/tuner_modal.rs`
- `src/ui/vocal_studio_view.rs`
- `src/ui/widgets.rs`
