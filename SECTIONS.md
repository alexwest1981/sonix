# SECTIONS.md — vad varje del av Sonix gör, och om den får röras

*Genererad av `tools/sections.py` — kör om den efter varje ändring: `python3 tools/sections.py`. Alla siffror är räknade ur koden; statusen ägs av modulens egen `//! Status:`-rad. Redigera inte det här dokumentet för hand.*

**94 moduler · 79970 rader kod · 48 med status · 46 utan.**

## Så läser du kartan

1. Leta upp modulen i tabellen. **Fryst/klart + många anropare = rör den inte** utan att läsa `Rör inte`-raden.
2. `Anropare` är hur många ställen utanför filen som nämner modulen — ett mått på hur dyrt ett misstag är, inte på hur viktig den är.
3. `Tester` är antalet `#[test]` i filen (`ign` = `#[ignore]`-körningar, som ofta är mätningar mot riktiga filer).
4. Statusen kommer från modulens egen dokumentation — **en ägare per faktum**. Saknas den står modulen i listan sist.

## Modulerna

| Modul | Rader | Status | Anropare | Tester | Vad den gör |
| :--- | ---: | :--- | ---: | :--- | :--- |
| `src/ui/app/tests.rs` | 3523 | byggs — proven hör till sina funktioner; flyttar du en funktion, flytta dess prov. | 0 | 99 (+1 ign) | Prov för app-ytan — flyttade ur `app.rs` 2026-09-14 (filen hade vuxit till 23 000 rader).  |
| `src/audio/plugin_host_live.rs` | 3332 | stabil (4.x) — CLAP-värden i egen process | 72 | 40 | In-process **CLAP** plugin host.  |
| `src/ui/app/mixer.rs` | 2949 | byggs — mixern och FX-racket. | 2 | 0 | Mixern — kanalracket, FX-racket och automationen.  |
| `src/i18n.rs` | 2858 | fryst — nycklar på engelska, texter på svenska | 1458 | 9 |  |
| `src/ui/app/modals.rs` | 2783 | byggs — dialogerna; en ny dialog läggs här och håller sin regel utanför. | 0 | 0 | Dialogerna — alla små fönster på ett ställe.  |
| `src/ui/app/arranger.rs` | 2776 | byggs — arrangören. | 0 | 0 | Arrangören — tidslinjen som ritas.  |
| `src/audio/exporter.rs` | 2233 | stabil — offline-rendering och export | 16 | 20 (+1 ign) | Offline full-project rendering and audio export for Sonix Studio.  |
| `src/ui/app/project.rs` | 2050 | byggs — projektformatet; migreringar flyttar men raderar aldrig. | 1 | 0 | Projektfilen — formen på disk, sparande, inläsning, autospar och återställning.  |
| `src/audio/synth/tests.rs` | 2032 | stabil — prov, inte kod; flyttar du en funktion, flytta dess prov. | 0 | 61 | Motorns prov: tidmappning, sträckning, regioner, kantrampen och klockan.  |
| `src/audio/plugin_vst3.rs` | 1982 | stabil (4.6a) — VST3-värden, en utbuss | 4 | 9 | Minimal in-process **VST3** host (Fas 4.6a).  |
| `src/audio/recorder.rs` | 1700 | stabil — inspelning | 8 | 16 |  |
| `src/audio/vocal_harmonizer.rs` | 1632 | — | 15 | 12 |  |
| `src/audio/stretch.rs` | 1627 | — | 18 | 24 (+8 ign) | Tempoföljning med bevarad tonhöjd (Fas 8.10, steg 2).  |
| `src/audio/factory_samples.rs` | 1575 | stabil — fabriksbiblioteket och `merge_library` | 9 | 10 |  |
| `src/audio/metadata.rs` | 1451 | stabil (8.5b) — bara härkomst tas; musik, text och omslag lämnas | 4 | 18 | Tar bort AI-/Sunohärkomst ur ljudfiler — och ingenting annat (8.5b).  |
| `src/ui/app/import.rs` | 1367 | byggs — importvägarna; nya format läggs här, inte i UI:t. | 1 | 0 | Import — stämmor från Suno, filer, separatorn och genererat ljud.  |
| `src/ui/app/timeline.rs` | 1302 | byggs — ångringen och klippoperationerna. | 1 | 0 | Tidslinjen — klippens geometri, ångringen och klippoperationerna.  |
| `src/ui/app/frame.rs` | 1242 | byggs — bildrutans faser. | 31 | 0 | Bildrutan — `update` uppdelad i sina faser (2026-09-14).  |
| `src/audio/macro_chain.rs` | 1207 | byggs — modellen, den rena körningen och batch-vägen (8.9 steg 1). | 21 | 14 | Makrokedjan (Fas 8.9): en sparad sekvens av kommandon som körs på filer.  |
| `src/ui/plugins_view.rs` | 1186 | — | 1 | 0 |  |
| `src/audio/plugin_host.rs` | 1153 | — | 4 | 14 |  |
| `src/ui/app/stretch.rs` | 1130 | byggs — sträckningen; cachenyckeln byts när motorn byts (motorns version i nyckeln). | 8 | 0 | Tempot och sträckningen — tempokartan, styckena, cachenycklarna och regionerna till motorn.  |
| `src/audio/onset.rs` | 1041 | stabil — slagletning, slicekarta, nudge, kantdämpning och dump (8.7 klart 2026-09-14) | 39 | 25 (+1 ign) | Onset-detektering och slicekarta (Fas 8.7).  |
| `src/ui/vocal_studio_view.rs` | 1027 | — | 1 | 0 |  |
| `src/audio/master_fx.rs` | 1016 | — | 6 | 9 | Real-time master bus FX chain and per-track equalizer DSP.  |
| `src/paths.rs` | 1014 | fryst — enda modulen som får bygga sökvägar | 50 | 14 | Kanoniska sökvägar för Sonix (Fas 6.0).  |
| `src/ui/app.rs` | 1010 | byggs — roten: tillståndet, starten och bildrutan. | 11 | 0 | **App-ytan — modulens rot.** Här bor `SonixApp` (tillståndet), `new` (starten) och `update` (bildrutan). Allt annat ligger i undermodulerna nedan, delade efter område |
| `src/ui/app/transport.rs` | 967 | byggs — uppspelningen. | 1 | 0 | Uppspelningen — stegklockan, sequencern, tangentbordet, tagningarna och hårdvaran.  |
| `src/audio/plugin_sandbox.rs` | 945 | — | 9 | 8 | Out-of-process plugin sandbox (Fas 4.5a + 4.5b).  |
| `src/ui/app/piano_roll.rs` | 926 | byggs — piano roll och trummisen. | 0 | 0 | Piano roll, trummisen och skalorna.  |
| `src/audio/plugin_vst2.rs` | 908 | — | 4 | 9 | Minimal in-process **VST2** host (Fas 4.6c).  |
| `src/ui/app/export.rs` | 908 | stabil — offline-renderingen och exporten. | 0 | 0 | Exporten och frysningen.  |
| `src/audio/stem_separator.rs` | 885 | stabil (8.5a) — separatorn skriver stämmorna till disk och minns var de ligger | 6 | 11 |  |
| `src/audio/waveform.rs` | 851 | stabil — exakt hölje och flernivåcache (8.3) | 30 | 15 | Exakta vågformer: ett (min, max) per skärmpixel (Alex krav, Fas 8.3).  |
| `src/ui/fx_rack_modal.rs` | 838 | — | 7 | 0 |  |
| `src/ui/app/state.rs` | 831 | byggs — datamodellen; nya fält hör hit och ska ha ett ärligt standardvärde. | 15 | 0 | Tillstånd och typer för app-ytan — utbrutet ur `app.rs` 2026-09-14.  |
| `src/audio/ai_generator.rs` | 790 | — | 6 | 7 |  |
| `src/audio/patcher.rs` | 782 | — | 4 | 6 |  |
| `src/audio/synth/process.rs` | 781 | stabil — motorns renderingsloop (en bildruta ljud i taget). | 53 | 0 | Renderingsloopen: en bildruta ljud i taget — röster, spår, bussar, sidokedjor och master. |
| `src/ui/macros_modal.rs` | 781 | byggs — steg 2. Klickas inte i CI (ingen skärm finns här), så logiken ligger i | 3 | 4 | Makron-modalen (Fas 8.9 steg 2): bygga, spara och köra en makrokedja inifrån appen.  |
| `src/audio/command.rs` | 738 | byggs — kommando-protokollet (AI-vägen) | 24 | 10 |  |
| `src/ui/app/plugins.rs` | 738 | byggs — plugin-värdarna. | 0 | 0 | Plugin-värdarna i UI:t — ladda, GUI-fönster, slots och sandlådan.  |
| `src/ui/widgets.rs` | 734 | — | 4 | 0 |  |
| `src/audio/ai_client.rs` | 715 | — | 8 | 8 |  |
| `src/audio/smf.rs` | 706 | stabil — MIDI-export med tempobyten (8.2 steg 3) | 33 | 10 | Standard MIDI File (SMF) — skriv och läs `.mid` utan externa beroenden.  |
| `src/audio/synth/commands.rs` | 705 | stabil — kommandovägen från UI-tråden in i motorn. | 0 | 0 | Kommandovägen: vad motorn gör när UI-tråden säger något — och ordningen spåren ska köras i. |
| `src/audio/input_profile.rs` | 662 | byggs (Sprint 1, punkt 1) — tabell, klassning och sysfs-läsning klara och prövade. | 11 | 12 | **Instrumentingångar som känns igen** (Sprint 1, punkt 1).  |
| `src/ui/app/browser.rs` | 633 | byggs — Sound Browser och biblioteksskanningen. | 4 | 0 | Sound Browser och biblioteket.  |
| `src/audio/tempo.rs` | 631 | stabil — tempokartan och `region_source_secs` | 57 | 12 | Tempokarta (Fas 8.2, steg 1).  |
| `src/audio/sandbox_audio.rs` | 602 | — | 10 | 5 | Shared-memory audio transport for the out-of-process plugin sandbox (Fas 4.5b). |
| `src/audio/engine.rs` | 579 | — | 2 | 3 |  |
| `src/midi_take.rs` | 564 | — | 29 | 27 | Inspelade nottagningar med sin faktiska tajming (Fas 6.4).  |
| `src/ui/app/midi.rs` | 522 | byggs — MIDI-vägarna. | 1 | 0 | MIDI — notschemat, importen, exporten och trumkanalerna.  |
| `src/ui/chord_generator_modal.rs` | 516 | — | 1 | 0 |  |
| `src/audio/loop_station.rs` | 507 | byggs (Sprint 1, punkt 7, steg 1) — modellen och dess regler är prövade. Glue:t | 0 | 10 | **Loop-pad: spela in ett lager i taget och låt det gå runt** — Sprint 1, punkt 7 (steg 1).  |
| `src/audio/launcher.rs` | 505 | byggs (Sprint 1, punkt 5, steg 1) — modellen och dess regler är prövade; rutnätet | 6 | 10 | **Launch-modellen: en rutnätsstyrd spelare** — Sprint 1, punkt 5 (första steget).  |
| `src/audio/realtime_bench.rs` | 505 | stabil — realtidsmätningen | 0 | 6 | Realtidsmätning av render-vägen (Fas 7.2).  |
| `src/audio/hardware_control.rs` | 493 | — | 3 | 5 |  |
| `src/audio/synth/voices.rs` | 471 | stabil — rösterna och spåren; en ny rösttyp hör hit och får eget prov. | 1 | 0 | Rösterna och spåren motorn spelar: synth, duckning, stämspår, audition, samplingar. |
| `src/ui/dice_generator_modal.rs` | 470 | — | 2 | 0 |  |
| `src/audio/plugin_gui.rs` | 468 | — | 2 | 3 | X11 window hosting for plugin GUIs (Fas 4.4b).  |
| `src/audio/neural_separator.rs` | 463 | — | 3 | 9 | Neural stem separation through an ONNX model (HTDemucs / Demucs family).  |
| `src/main.rs` | 463 | — | 0 | 0 |  |
| `src/autosave.rs` | 430 | — | 46 | 10 | Autosave, versionsrotation och kraschåterställning (Fas 6.1).  |
| `src/ui/add_track_modal.rs` | 428 | — | 1 | 0 |  |
| `src/audio/scale.rs` | 412 | stabil (8.11) — en tabell och ett index för tonarten | 23 | 10 | Tonarter: **en** tabell och **ett** index (Alex' kvittens 2026-09-12).  |
| `src/selftest.rs` | 395 | — | 3 | 5 | **Självtestet** (Fas 7.1) — den del av Windows-kriteriet som ingen CI-mätning kan svara på.  |
| `src/audio/loudness.rs` | 358 | — | 4 | 5 | Loudness measurement and normalization (ITU-R BS.1770 / EBU R128).  |
| `src/ui/tuner_modal.rs` | 351 | — | 1 | 0 |  |
| `src/audio/envelope.rs` | 339 | — | 61 | 10 |  |
| `src/audio/spectrum.rs` | 335 | ny (8.13) — toppradens nivå- och registermätare | 9 | 7 (+2 ign) | Registret i utgången — vilka frekvensband som bär energi — och mätarskalan (8.13).  |
| `src/audio/synth.rs` | 326 | stabil — motorn: mixer, bussar, sidokedjor (8.3) och sends | 97 | 0 |  Här bor bara **roten**: `SynthEngine`-tillståndet, `new` och kommandots namn. |
| `src/ui/patcher_view.rs` | 303 | — | 1 | 0 |  |
| `src/audio/dither.rs` | 296 | — | 7 | 8 | Dither vid kvantisering till fast punkt (Fas 6.5).  |
| `src/ui/song_structure_modal.rs` | 295 | — | 3 | 0 |  |
| `src/ui/launcher_view.rs` | 290 | byggs (Sprint 1, punkt 5, steg 2) — rutnätet, kvantiseringsvalet och scenknapparna. | 2 | 3 | **Rutnätet på skärmen — den mjukvarustyrda launch-ytan** (Sprint 1, punkt 5, steg 2).  |
| `src/audio/wav_reader.rs` | 283 | — | 4 | 3 |  |
| `src/audio/midi_input.rs` | 275 | — | 2 | 10 | **MIDI-klaviatur in — en väg, `midir`, på alla plattformar** (Fas 7.1).  |
| `src/ui/ai_assistant_view.rs` | 260 | — | 1 | 0 |  |
| `src/audio/drum.rs` | 252 | — | 5 | 3 |  |
| `src/audio/effects.rs` | 216 | — | 5 | 4 |  |
| `src/audio/filter.rs` | 194 | — | 31 | 6 |  |
| `src/ui/stem_view.rs` | 185 | — | 1 | 0 |  |
| `src/audio/keymap.rs` | 162 | — | 13 | 7 | **Multi-samples: en keymap av zoner** (Fas 8.4/7).  |
| `src/ui/app/waveform.rs` | 148 | stabil — exaktheten och nycklarna; ändra bara med ett prov som visar samma sak. | 26 | 0 | Vågformer i vyn — nyckeln, översikten och den sanna cachen.  |
| `src/ui/app/macros.rs` | 114 | byggs — steg 2. | 0 | 0 | Makron-modalen i appen (Fas 8.9 steg 2): inkopplingen till `ui::macros_modal`.  |
| `src/audio/mod.rs` | 109 | — | 0 | 0 |  |
| `src/ui/app/input_autoconfig.rs` | 109 | byggs (Sprint 1, punkt 1) — automatiken och knappen är samma funktion. | 0 | 0 | **Ingången ställer in sig själv** — Sprint 1, punkt 1.  |
| `src/rng.rs` | 82 | — | 2 | 4 | En liten deterministisk slumptalare (xorshift64\*).  |
| `src/ui/app/launcher.rs` | 81 | — | 5 | 0 | **App-glue:t för scenrutnätet** — Sprint 1, punkt 5, steg 2.  |
| `src/platform.rs` | 63 | — | 2 | 1 | **Plattformens egna sätt** (Fas 7.1) — det som skiljer sig utan att vara sökvägar.  |
| `src/audio/wav_writer.rs` | 48 | — | 3 | 0 |  |
| `src/ui/theme.rs` | 30 | — | 15 | 0 |  |
| `src/ui/mod.rs` | 20 | — | 0 | 0 |  |

## Rör inte-regler och publika ingångar

### `src/audio/ai_client.rs`
- **Publika ingångar:** `AiProvider`, `ALL`, `label`, `requires_key`, `default_base`, `default_model`, `known_models`, `from_code`, `AudioProvider`, `ALL`, `label`, `requires_key` … (+16)

### `src/audio/ai_generator.rs`
- **Publika ingångar:** `AiGeneratedClip`, `GenContext`, `AiMusicAssistant`, `generate_preset_clips`, `generate_from_prompt`, `selected_genre_label`, `selected_key_label`, `selected_target_label`, `generate`, `request_remote_generation`, `poll_remote_generation`, `GenStyle` … (+1)

### `src/audio/command.rs`
- **Status:** byggs — kommando-protokollet (AI-vägen)
- **Rör inte:** nya kommandon ska gå genom `Command` så att agenten och UI:t gör samma sak
- **Publika ingångar:** `Waveform`, `name`, `Preset`, `settings`, `AudioCommand`, `bus`, `track`, `as_bus`, `as_track`, `plan_track_order`, `SendTarget`, `StemSend` … (+6)

### `src/audio/dither.rs`
- **Publika ingångar:** `DEFAULT_SEED`, `TpdfDither`, `new`, `with_noise_shaping`, `quantize_to_16`, `quantize_i16`

### `src/audio/drum.rs`
- **Publika ingångar:** `DrumType`, `DrumVoice`, `new`, `trigger`, `reset`, `next_sample`

### `src/audio/effects.rs`
- **Publika ingångar:** `DelayParams`, `StereoDelay`, `new`, `process`, `ReverbParams`, `SimpleReverb`, `new`, `process`

### `src/audio/engine.rs`
- **Publika ingångar:** `AudioEngine`, `AudioSettings`, `config_path`, `load`, `save`, `new`, `new_with`, `reconfigure`, `send_command`, `set_output_silent`, `get_peak_level`, `song_position_secs` … (+6)

### `src/audio/envelope.rs`
- **Publika ingångar:** `EnvelopeStage`, `AdsrParams`, `identity`, `is_identity`, `velocity_gain`, `VelocityCurve`, `ALL`, `apply`, `label`, `AdsrVoice`, `new`, `gate_on` … (+4)

### `src/audio/exporter.rs`
- **Status:** stabil — offline-rendering och export
- **Rör inte:** exporten måste ge **samma ljud som högtalarna**; samma väg som uppspelningen, inte en parallell
- **Publika ingångar:** `VoiceSpec`, `RackChannel`, `PatternSnap`, `TrackRole`, `TrackSnap`, `TrackAudioSnap`, `FxState`, `RenderSpec`, `midi_to_freq`, `triggers_for_step`, `load_timeline_into_engine`, `render_project_offline` … (+12)

### `src/audio/factory_samples.rs`
- **Status:** stabil — fabriksbiblioteket och `merge_library`
- **Publika ingångar:** `GeneratedSample`, `new_stereo`, `compute_waveform_peaks`, `save_to_file`, `generate_all_factory_samples`, `ensure_factory_samples_directory`, `ScannedSampleItem`, `read_library_cache_items`, `ScanMsg`, `ScanPoll`, `LibraryScan`, `spawn` … (+3)

### `src/audio/filter.rs`
- **Publika ingångar:** `FilterParams`, `StateVariableFilter`, `new`, `reset`, `process_lowpass`, `SamplerFilter`, `filter_cutoff_at`

### `src/audio/hardware_control.rs`
- **Publika ingångar:** `ControlEvent`, `OscArg`, `parse_osc_message`, `osc_to_event`, `OscServer`, `start`, `received`, `McuInput`, `connect`, `received`, `device_list`, `connect` … (+3)

### `src/audio/input_profile.rs`
- **Status:** byggs (Sprint 1, punkt 1) — tabell, klassning och sysfs-läsning klara och prövade.
- **Rör inte:** `classify` är den enda vägen från ett enhetsnamn till en profil — lägg nya
- **Publika ingångar:** `InputKind`, `badge`, `InputProfile`, `PROFILES`, `ResolvedInput`, `is_instrument`, `classify`, `InputDefaults`, `MIC_DEFAULTS`, `INSTRUMENT_DEFAULTS`, `defaults_for`, `InputPlan` … (+9)

### `src/audio/keymap.rs`
- **Publika ingångar:** `SampleZone`, `full_range`, `covers`, `is_playable`, `zone_for_note`

### `src/audio/launcher.rs`
- **Status:** byggs (Sprint 1, punkt 5, steg 1) — modellen och dess regler är prövade; rutnätet
- **Rör inte:** `tick` är den **enda** vägen från ett klick till ett kommando. Ytorna får skriva
- **Publika ingångar:** `LaunchQuantize`, `is_point`, `SlotContent`, `is_empty`, `name`, `SlotState`, `Slot`, `LauncherCommand`, `Launcher`, `new`, `tracks`, `scenes` … (+8)

### `src/audio/loop_station.rs`
- **Status:** byggs (Sprint 1, punkt 7, steg 1) — modellen och dess regler är prövade. Glue:t
- **Rör inte:** `tick` är den enda vägen från ett tryck till ett kommando. Spelar en yta in själv
- **Publika ingångar:** `LoopPhase`, `LoopLayer`, `LoopCommand`, `LoopStation`, `new`, `has_loop`, `loop_length_bars`, `is_recording`, `press`, `undo`, `tick`

### `src/audio/loudness.rs`
- **Publika ingångar:** `LoudnessPreset`, `ALL`, `enabled`, `label`, `target_lufs`, `ceiling_dbtp`, `integrated_lufs`, `true_peak_db`, `normalize_loudness`, `normalize_to_preset`

### `src/audio/macro_chain.rs`
- **Status:** byggs — modellen, den rena körningen och batch-vägen (8.9 steg 1).
- **Rör inte:** en andra ljudväg. Stegen ska anropa de befintliga maskinerna, aldrig kopiera dem.
- **Publika ingångar:** `DEFAULT_OUT_DIR`, `LOG_FILE`, `MacroStep`, `label`, `writes_a_file`, `MacroChain`, `new`, `has_export`, `StepReport`, `FileRun`, `succeeded`, `signal_span_frames` … (+18)

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
- **Status:** stabil — slagletning, slicekarta, nudge, kantdämpning och dump (8.7 klart 2026-09-14)
- **Rör inte:** mät på en jämn ton först: en detektor är en mätning, och 4 ms enpolsfilter + centrerad tröskel är den enda variant som ger noll falska slag
- **Publika ingångar:** `OnsetParams`, `Slice`, `onset_envelope`, `detect_onsets`, `music_start_source_secs`, `slices_from_onsets`, `nudge_slice_boundary`, `grid_capacity`, `detect_slice_map`, `spectral_flux_onsets`, `slices_to_steps`, `window_for_note`

### `src/audio/patcher.rs`
- **Publika ingångar:** `NodeType`, `PatcherNode`, `PatchCable`, `ModularGraph`, `load_default_preset`, `add_node`, `PatchNodeSpec`, `PatchSpec`, `to_spec`, `has_cycle`, `PatchProcessor`, `new` … (+4)

### `src/audio/plugin_gui.rs`
- **Publika ingångar:** `X11Window`, `open`, `window_id`, `resize`, `poll`, `is_alive`, `X11Window`, `open`, `window_id`, `resize`, `poll`, `is_alive` … (+7)

### `src/audio/plugin_host.rs`
- **Publika ingångar:** `PluginFormat`, `name`, `badge_color`, `is_lv2_path`, `PluginCategory`, `label`, `icon`, `PluginDescriptor`, `FlStudioPreset`, `ScanPath`, `PluginManager`, `without_scan` … (+23)

### `src/audio/plugin_host_live.rs`
- **Status:** stabil (4.x) — CLAP-värden i egen process
- **Rör inte:** en plugin i uppspelningsvägen får aldrig kunna tysta eller krascha motorn
- **Publika ingångar:** `PARAM_IS_STEPPED`, `PARAM_IS_PERIODIC`, `PARAM_IS_HIDDEN`, `PARAM_IS_READONLY`, `PARAM_IS_BYPASS`, `PARAM_IS_AUTOMATABLE`, `PARAM_IS_MODULATABLE`, `PluginParameter`, `is_stepped`, `is_periodic`, `is_hidden`, `is_readonly` … (+126)

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
- **Publika ingångar:** `PIANO_ROLL_BASE_MIDI`, `PIANO_ROLL_ROWS`, `ROOT_NAMES`, `Scale`, `SCALES`, `is_minor`, `scale_at`, `root_name`, `key_label`, `scale_notes`, `in_scale`, `nearest_in_scale` … (+2)

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
- **Rör inte:** körordningen — `stem_order` och `stem_incoming` byggs i **samma** pass, så sändarens utgång finns när mottagarens kedja kör (8.3; provet `a_track_send_arrives_in_phase`).
- **Publika ingångar:** `NUM_BUSES`, `NUM_VCAS`, `BUS_NAMES`, `SynthEngine`, `new`

### `src/audio/synth/commands.rs`
- **Status:** stabil — kommandovägen från UI-tråden in i motorn.
- **Rör inte:** ordningen som `recompute_stem_order` räknar fram — mekanismen bakom sends mellan spår (8.3), och ett nej ska vara ett nej (slingan namnges).
- **Publika ingångar:** `handle_command`

### `src/audio/synth/process.rs`
- **Status:** stabil — motorns renderingsloop (en bildruta ljud i taget).
- **Rör inte:** ordningen i `process_stereo` — en sändares utgång måste finnas när mottagarens kedja kör (sends mellan spår, 8.3); mät fasen mot en kontroll på samma nivå, inte mot "2×".
- **Publika ingångar:** `process_stereo`

### `src/audio/synth/voices.rs`
- **Status:** stabil — rösterna och spåren; en ny rösttyp hör hit och får eget prov.
- **Rör inte:** `slice_edge_gain` — `fade_frames = 0` ska ge exakt den gamla vägen (1,0 överallt), inte nästan; det är den som skiljer ett klick från ett klipp.
- **Publika ingångar:** `Voice`, `Ducker`, `region_source_secs`, `StemVoiceTrack`, `AuditionVoice`, `slice_edge_gain`, `SLICE_FADE_SECS`, `SampleVoice`, `ScheduledNote`, `new`, `trigger`, `release` … (+10)

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
- **Publika ingångar:** `PlatformDefaults`, `APP_DIR`, `LIBRARY_DIR`, `ENV_PROJECTS_DIR`, `ENV_SAMPLES_DIR`, `ENV_CONFIG_DIR`, `ENV_DATA_DIR`, `ENV_STATE_DIR`, `ENV_CACHE_DIR`, `expand_tilde`, `parse_user_dir`, `Paths` … (+50)

### `src/platform.rs`
- **Publika ingångar:** `file_manager_command`, `open_dir`

### `src/rng.rs`
- **Publika ingångar:** `Rng`, `new`, `next_u64`, `next_f32`, `next_sym`

### `src/selftest.rs`
- **Publika ingångar:** `Check`, `ClockVerdict`, `clock_verdict`, `sound_was_heard`, `summary`, `audible_from_args`, `run`

### `src/ui/add_track_modal.rs`
- **Publika ingångar:** `AddTrackCategory`, `TrackTemplate`, `get_track_templates`, `AddTrackModalState`, `render_add_track_modal`

### `src/ui/ai_assistant_view.rs`
- **Publika ingångar:** `render_ai_assistant_view`

### `src/ui/app.rs`
- **Status:** byggs — roten: tillståndet, starten och bildrutan.
- **Rör inte:** `update` är den enda ingången per bildruta. En ny panel läggs i sin egen modul
- **Publika ingångar:** `SonixApp`, `tr`, `new`

### `src/ui/app/arranger.rs`
- **Status:** byggs — arrangören.
- **Rör inte:** enheten. Takt är inte sekund; vid ett tempobyte är bara takten rätt plats.
- **Publika ingångar:** `render_playlist_arranger`, `draw_transport_bar`, `draw_arranger_tools_bar`, `draw_arranger_ai_capsule`, `draw_arranger_utility_bar`, `draw_track_headers`, `draw_timeline_lanes`, `TimelineGeometry`, `DeferredActions`, `draw_region_inspector`

### `src/ui/app/browser.rs`
- **Status:** byggs — Sound Browser och biblioteksskanningen.
- **Rör inte:** \`merge_library\` är enda vägen till listan; bygg inte en andra.
- **Publika ingångar:** `init_sample_library_start`, `merge_library`, `library_items_from_scanned`, `assign_library_sample_to_channel`, `auto_assign_default_kit`, `channel_sample_trigger_command`, `ui_dbg`, `open_sample_in_vocal_studio`, `import_custom_sample`, `poll_library_scan`, `render_browser_sidebar`

### `src/ui/app/export.rs`
- **Status:** stabil — offline-renderingen och exporten.
- **Rör inte:** samma väg som uppspelningen; en parallell väg hörs som en annan låt.
- **Publika ingångar:** `export_separated_stems`, `export_wav`, `open_export_modal`, `export_sample_rate`, `build_render_spec`, `render_buffer`, `frozen_is_stale`, `frozen_path`, `freeze_track`, `unfreeze_track`, `render_batch_export_modal`, `execute_batch_export`

### `src/ui/app/frame.rs`
- **Status:** byggs — bildrutans faser.
- **Rör inte:** ordningen. Pollarna ligger före ritningen med flit (motorn ska veta
- **Publika ingångar:** `poll_background_work`, `tick_frame`, `handle_input`, `handle_dropped_files`, `draw_chrome`, `draw_workspace`, `screenshot_tick`

### `src/ui/app/import.rs`
- **Status:** byggs — importvägarna; nya format läggs här, inte i UI:t.
- **Rör inte:** `load_audio_or_report` är den enda vägen in till PCM; gå inte förbi den.
- **Publika ingångar:** `stem_files_for_import`, `list_stem_audio_files`, `duplicate_mp3_count`, `wav_sibling`, `zip_entry_names`, `stem_base_name`, `load_audio_or_report`, `report_unreadable`, `take_unreadable_sources`, `StemImportSource`, `WavQuestion`, `StemImportProgress` … (+24)

### `src/ui/app/input_autoconfig.rs`
- **Status:** byggs (Sprint 1, punkt 1) — automatiken och knappen är samma funktion.
- **Rör inte:** ordningen frekvens-före-monitor. Byter man den hörs fel tempo utan att något
- **Publika ingångar:** `resolved_inputs`, `apply_instrument_plan`, `autodetect_instrument_input`

### `src/ui/app/launcher.rs`
- **Publika ingångar:** `rebuild_launcher_from_sections`, `tick_launcher`, `render_launcher_view`

### `src/ui/app/macros.rs`
- **Status:** byggs — steg 2.
- **Rör inte:** exportens väg. Renderingen och skrivaren är **samma** som exportens
- **Publika ingångar:** `run_macro_on_project`, `render_macros_modal_view`

### `src/ui/app/midi.rs`
- **Status:** byggs — MIDI-vägarna.
- **Rör inte:** `notes_by_bar`/`pattern_bar_notes` är samma notbild som exporten skriver.
- **Publika ingångar:** `pattern_bar_notes`, `midi_channel_for_track`, `drum_channel_for_key`, `MidiImportReport`, `STEPS_PER_BAR`, `ARRANGEMENT_BARS`, `notes_by_bar`, `apply_bar_to_pattern`, `export_song_midi`, `import_midi_into_selected_pattern`, `render_midi_import_modal`

### `src/ui/app/mixer.rs`
- **Status:** byggs — mixern och FX-racket.
- **Rör inte:** kommandona bär hela inställningen; att bygga en ny nollar tyst frekvenser och Q.
- **Publika ingångar:** `classify_track_style`, `update_scope_history`, `load_pattern_into_ui`, `select_pattern`, `sync_active_pattern_from_ui`, `select_sound_for_channel`, `audition_library_sample`, `plugin_parameters`, `plugin_parameter`, `automation_curve`, `automation_points_mut`, `automation_target_range` … (+13)

### `src/ui/app/modals.rs`
- **Status:** byggs — dialogerna; en ny dialog läggs här och håller sin regel utanför.
- **Rör inte:** knapptexten är en instruktion, inte kosmetika — ändra texten när vägen ändras.
- **Publika ingångar:** `logo_texture_for`, `render_add_track_modal`, `render_import_sample_modal`, `render_hardware_controller_modal`, `render_wav_question_modal`, `render_suno_stem_import_modal`, `render_stem_import_progress_modal`, `render_project_load_progress_modal`, `render_recovery_modal`, `render_about_modal`, `render_project_manager_modal`, `render_ai_settings_modal` … (+11)

### `src/ui/app/piano_roll.rs`
- **Status:** byggs — piano roll och trummisen.
- **Rör inte:** en tabell och ett index för tonarten; två listor för samma sak driver isär.
- **Publika ingångar:** `get_scale_notes`, `humanize_active_pattern`, `transpose_active_pattern`, `arpeggiate_pattern`, `reverse_pattern`, `render_piano_roll_editor`, `apply_drummer_generation`, `render_session_drummer`, `render_touch_piano_keyboard`

### `src/ui/app/plugins.rs`
- **Status:** byggs — plugin-värdarna.
- **Rör inte:** feature-grinden `plugin-host`; sandlådans väg får inte bli standardvägen.
- **Publika ingångar:** `load_plugin_into_track`, `load_plugin_preset_into_track`, `remove_plugin_from_track`, `load_plugin_into_sandbox_track`, `close_all_plugin_guis`, `retire_all_plugin_handles`, `is_plugin_gui_open`, `open_plugin_gui`, `close_plugin_gui`, `poll_plugin_guis`, `sandbox_inspect`, `poll_plugin_sandbox` … (+5)

### `src/ui/app/project.rs`
- **Status:** byggs — projektformatet; migreringar flyttar men raderar aldrig.
- **Rör inte:** fältnamnen på disk är ett kontrakt mot filer som redan finns hos användaren.
- **Publika ingångar:** `RecoveryCandidate`, `collect_recovery_candidates`, `collect_recovery_candidates_in`, `RecentProject`, `RECENT_MAX`, `load_recent_projects`, `load_recent_projects_in`, `store_recent_projects_in`, `push_recent_project`, `SavedPattern`, `SavedChannel`, `SonixProjectData` … (+34)

### `src/ui/app/state.rs`
- **Status:** byggs — datamodellen; nya fält hör hit och ska ha ett ärligt standardvärde.
- **Rör inte:** alla fält är `pub` (syskonmodulerna läser dem); döp inte om ett fält som står i en projektfil.
- **Publika ingångar:** `midi_to_freq`, `note_name`, `ViewMode`, `ArrangerTool`, `RegionDragMode`, `RegionDragState`, `LibrarySampleItem`, `ChannelStrip`, `Pattern`, `TimeSnapMode`, `format_time_hundredths`, `format_bar_subdivisions` … (+39)

### `src/ui/app/stretch.rs`
- **Status:** byggs — sträckningen; cachenyckeln byts när motorn byts (motorns version i nyckeln).
- **Rör inte:** \`tempo_map\` är enda källan till takt↔tid; gå inte förbi den med egen matematik.
- **Publika ingångar:** `StretchPiece`, `stretch_pieces`, `stretch_ratio_for`, `source_bpm_from_region`, `geometry_source_bpm`, `stamped_tempo`, `tempo_change_note`, `stamp_source_tempo`, `region_source_span_samples`, `playback_for`, `stretched_offset_secs`, `FIRST_BEAT_SEARCH_SECS` … (+20)

### `src/ui/app/tests.rs`
- **Status:** byggs — proven hör till sina funktioner; flyttar du en funktion, flytta dess prov.
- **Rör inte:** proven läser privata fält genom `use super::*;`, så de måste ligga under `app`.

### `src/ui/app/timeline.rs`
- **Status:** byggs — ångringen och klippoperationerna.
- **Rör inte:** alla fält är \`pub\`; döp inte om ett fält som står i en projektfil.
- **Publika ingångar:** `ClipStartAlign`, `align_clip_start_to_point`, `timeline_point_for_source_secs`, `ClipGeometry`, `STEM_GROUP_TOLERANCE_BARS`, `plan_group_shift_by_head_secs`, `region_under_x`, `TimelineUndoSnapshot`, `find_region_at_playhead`, `split_selected_region_at_playhead`, `save_region_to_sound_browser`, `add_sample_item_to_timeline` … (+21)

### `src/ui/app/transport.rs`
- **Status:** byggs — uppspelningen.
- **Rör inte:** `step_duration` och motorns position är samma klocka; lägg inte en tredje.
- **Publika ingångar:** `steps_elapsed`, `ensure_mic_track_exists`, `toggle_timeline_recording`, `toggle_playback`, `stop_playback`, `seek_song_bar`, `seek_song_time`, `sync_track_audio_state`, `sync_group_state`, `get_max_project_bars`, `advance_sequencer`, `play_note` … (+7)

### `src/ui/app/waveform.rs`
- **Status:** stabil — exaktheten och nycklarna; ändra bara med ett prov som visar samma sak.
- **Rör inte:** `imported_track_waveform` är regeln som ger spåret samma cache som motorn fick.
- **Publika ingångar:** `waveform_key`, `overview_peaks_from`, `imported_track_waveform`, `waveform_preview_from_pcm`, `load_sample_pcm_arcs`, `ensure_waveform_cache`

### `src/ui/chord_generator_modal.rs`
- **Publika ingångar:** `ScaleType`, `all`, `intervals`, `GeneratedChord`, `ChordGeneratorState`, `get_chord_pads`, `load_progression_preset`, `play_chord_sound`, `audition_progression`, `render_chord_generator_modal`

### `src/ui/dice_generator_modal.rs`
- **Publika ingångar:** `DiceCategory`, `GeneratedStepNote`, `DiceGeneratorState`, `roll_dice`, `render_dice_generator_modal`

### `src/ui/fx_rack_modal.rs`
- **Publika ingångar:** `VisualEqNode`, `FxPedal`, `FxRackState`, `load_preset`, `build_master_fx_params`, `sync_to_engine`, `render_fx_rack_modal`

### `src/ui/launcher_view.rs`
- **Status:** byggs (Sprint 1, punkt 5, steg 2) — rutnätet, kvantiseringsvalet och scenknapparna.
- **Rör inte:** klick skriver **bara** tillstånd i modellen. Ingen väg härifrån får sätta slingan
- **Publika ingångar:** `quantize_label`, `slots_from_sections`, `render_launcher_window`

### `src/ui/macros_modal.rs`
- **Status:** byggs — steg 2. Klickas inte i CI (ingen skärm finns här), så logiken ligger i
- **Rör inte:** en andra uppsättning stegregler. Stegen bor i `macro_chain`.
- **Publika ingångar:** `MacroTarget`, `MacroStepKind`, `ALL`, `label`, `default_step`, `chain_file_name`, `MacroModalState`, `reload`, `select`, `list_input_files`, `progress_now`, `take_finished` … (+5)

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
- `src/audio/keymap.rs`
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
- `src/ui/app/launcher.rs`
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
