# Sonix Studio — Verktygsstatus & åtgärdsplan

> Inventering av vilka verktyg som är **klara (fungerar på riktigt)** respektive vilka som bara är **placeholders** (UI finns, men ingen funktion bakom).
> Bedömning baserad på om verktygets handling når ljudmotorn (`AudioEngine::send_command` → `SynthEngine::handle_command`) eller skrivs till disk/tidslinje — eller bara muterar döda UI-fält.

---

## 1. Verktyg som är klara (REAL)

Dessa utför verkligt arbete: de processar ljud, muterar spellistan/kanalerna eller skriver filer.

### Kärnmotor & ljud
- **Alchemy-synthen** (`src/audio/synth.rs`) — riktiga oscillatorer (Sine/Saw/Square/Triangle/Noise), Moog-liknande SVF-filter, ADSR, drive. Morph-paden skickar riktiga `SetAdsr`/`SetFilter`/`SetWaveform`.
- **Trumsyntes** (`src/audio/drum.rs`) — syntetiserade 808/909-ljud via pitch/amp-enveloper + brus, inte samplingar.
- **Effekt-DSP** (`src/audio/effects.rs`) — `StereoDelay` (ping-pong med feedback) och `SimpleReverb` (Schroeder comb+allpass) körs i syntmotorn.
- **Envelope & filter** (`src/audio/envelope.rs`, `src/audio/filter.rs`) — riktiga DSP-block.
- **Oscilloskop** (`src/ui/widgets.rs:142`) — ritar verklig utsignal via `drain_scope_samples`.

### Sequencer & arranger
- **Timeline / multitrack-arranger** — direkt klädredigering, magnetisk loop-snap, region-inspektör, verktygspalett, reverse, fades.
- **Channel Rack (16-steg)** — steg skickar riktiga `NoteOn`/`TriggerDrum`.
- **Piano Roll & touch-klaviatur** — noter spelas upp på riktigt.
- **Session Drummer (XY-pad)** — `apply_drummer_generation` (`app.rs:7212`) skriver riktiga steg till kanaler och synkar mönster, inkl. kick/snare-variation, hi-hat-stil, fills och toms (P13).
- **Chord Matrix** — ackordsyntes + live-pads spelar riktigt; infogning skriver till kanaler. (Endast grundton — se P14.)

### Inspelning & redigering
- **Live mikrofoninspelning** (`src/audio/recorder.rs`) — riktig `cpal`-instream, gain, noise gate, VU.
- **Vocal Studio: klipp/normalisera/radera/exportera** — riktiga PCM-operationer.
- **Vocal Studio audition** — `PlayAudition`/`SetAuditionParams` spelar riktiga samples (pitch/stretch).
- **Suno ZIP-import** — riktig `unzip` + `load_wav_pcm`/`load_audio_pcm` (ffmpeg för komprimerade format) → `LoadStemTrack`.

### Export & innehåll
- **Export/Render-kö** (`src/audio/exporter.rs`) — offline-rendering av hela projektet. WAV 16/24/32-bit in-process, FLAC via `flacenc`, MP3/OGG/AAC via `ffmpeg`.
- **Factory sample-bibliotek** (`src/audio/factory_samples.rs`) — genererar riktiga WAV-loopar och skannar disk.
- **Add Track (instrument/synth/trum-spår)** — skapar riktiga `PlaylistTrack`. (Import-mallar tomma — se P15.)
- **Plugin-upptäckt** (`scan_disk`, `import_file`) — riktig rekursiv filsystemsscanning av `.vst3/.clap/.lv2/.dll/.so/.fst`. (Laddning fejkas — se P4.)
- **AI Music Assistant: "Applicera på Sub Bass/303 Lead"** — skriver riktiga steg/noter till sequencern. (Genereringen är nu regelbaserad musikteori — se P18.)

---

## 2. Placeholders & ofullständiga — prioriterad åtgärdslista

Sorterad efter prioritet (högst påverkan/tydligast löfte först). "Verktyg" = användarsynlig funktion.

### P1 — FX Rack (Soundtrap Studio Effects) · ✅ REAL (implementerad)
Ny DSP-modul `src/audio/master_fx.rs`: 4-bands parametrisk master-EQ, kompressor, de-esser, noise gate, doubler/chorus, resonant master-filter med drive och brickwall-limiter — allt allokeringsfritt. Nya kommandon `AudioCommand::SetMasterFx(MasterFxParams)` + `SetTrackEq`. `src/ui/fx_rack_modal.rs` bygger `MasterFxParams` från hela racket och skickar det **varje frame** medan modalen är öppen (realtid) samt vid "Tillämpa". Reverb-pedalen mappas till `SetReverb`. Offline-export replikerar kedjan via `FxState.master_fx`. GR-mätaren kan läsas via `MasterFxChain::compressor_gain_reduction_db()`.

### P2 — Modular Patcher (nod-graf) · ✅ REAL
Riktig DSP-graf i `src/audio/patcher.rs`: `PatchSpec`/`PatchNodeSpec` (serialiserbar från `ModularGraph::to_spec`) och `PatchProcessor` som evaluerar grafen sample-för-sample. Per-nod-DSP: MidiIn (pitch/gate/velocity), Oscillator (sin/såg/fyrkant/triangel, pitch-CV), SVF-filter (LP/HP/BP + cutoff/res-CV), ADSR-envelope, LFO, stereo Delay, Reverb, Distortion (tanh), AudioOut (volym/pan). Nya kommandon `SetPatcherGraph`, `SetPatcherEnabled`, `PatcherNoteOn`, `PatcherNoteOff`; `SynthEngine` mixar in patchern i `process_stereo` (steg 8b) före master-FX. `src/ui/patcher_view.rs` har nu en "🔊 Ljudmotor"-toggle + testklaviatur (15 tangenter) som triggar riktiga noter; `app.rs` `sync_patcher_graph()` skickar grafen när den ändras (jämför `last_patcher_spec`). Tester: `processor_renders_audible_signal`, `envelope_opens_and_releases`.

### P3 — AI Stem Separation · ✅ REAL (DSP)
`src/audio/stem_separator.rs` har nu en riktig (om än lättviktig) källseparering i ren Rust/DSP: `separate_stems(left, right, sr)` banddelar mixen (bas = lågpass 180 Hz mono, trummor = högpass 250 Hz med transient-gate, sång = center-bandpass 300–4500 Hz duckad av trummor, instrument = residual), normaliserar och returnerar fyra stämspår. `StemProject::separate_from_pcm` fyller `stem_audio` + vågformspeaks (`visual_peaks_from`). Flödet är riktigt: `stem_view` "Välj fil & separera" → asynkron filväljare → `app.start_stem_separation` avkodar via `load_audio_pcm` → separerar → `load_separated_stems_to_engine` skickar `LoadStemTrack`/`SetStemTrackState`. Mute/Solo/Vol/Pan i vyn synkas varje frame via `sync_stem_separator_engine` och är nu hörbara. "Exportera Stems" (`export_separated_stems`) lägger in de fyra spåren i Song Arranger. Test: `separation_preserves_length_and_is_finite`. `load_demo_stems`/`trigger_ai_separation` borttagna.

### P4 — Plugin-laddning & "Öppna GUI" · ✅ REAL (ärlig hanterare)
Påhittade plugins och mätvärden borttagna. `populate_known_plugins()` (som fabricerade Sytrus/Pro-Q/Vital m.fl. med `is_loaded: true` och fake-CPU/latens) är helt borttagen; `PluginManager::default()` listar nu **endast** filer som faktiskt finns på disk. `verify_plugin_artifact(path)` kontrollerar riktiga magiska bytes (ELF `\x7fELF` / PE `MZ`) eller letar binär i plugin-paket (`walk_find_so`), och `PluginDescriptor` har nu `file_size_bytes`, `verified`, `verify_note` i stället för `cpu_usage`/`latency_samples`. `import_file` kräver att filen existerar och verifierar den. Wine-version och yabridge-status detekteras på riktigt (`detect_wine_version`/`detect_yabridge_installed`), och "Kör yabridgectl sync" kör `run_yabridge_sync()` på riktigt. I vyn visas filstorlek + "Verifierad/Ej verifierad" samt en fungerande "📂 Visa i mapp" (`xdg-open`); de oärliga "Ladda Plugin"/"Öppna GUI"-knapparna är borttagna. Tester: `manager_does_not_fabricate_plugins`, `verify_detects_elf_and_rejects_garbage`, `manual_import_requires_real_file`, `manual_plugin_import_records_real_file`.

### P5 — Vocal Harmonizer / Auto-Tune / Melodyne ARA2 · ✅ REAL (implementerad)
`src/audio/vocal_harmonizer.rs` har nu riktig DSP: granulär pitch-shifter (`pitch_shift_variable`/`pitch_shift`), formant-tilt, skalsnappning (`snap_midi_to_scale` + `scale_mask`) och per-frame Auto-Tune (`auto_tune`, styrka från AUTO-TUNE-ratten). Melodyne-vyn analyserar vald tagning till riktiga notblock (`analyze_take`), Auto-Tune-knappen korrigerar tagningens PCM mot vald skala, och "Skapa körstämmor" renderar varje aktiverad stämma som en egen tagning. Enhetstester verifierar pitch-shift, autotune och blob-analys.

### P6 — Strobe Tuner (pitch-detektion) · ✅ REAL (implementerad)
Mikrofon-callbacken matar nu en alltid-på rullande analysbuffer (`analysis_buffer`, 16k samples) även när inspelning är avstängd. `detect_pitch_hz()` (normaliserad autokorrelation med oktavskydd + parabolisk interpolation) körs mot bufferten och `TunerState` visar verklig frekvens, cent-offset, auto-strängdetektering och ingångsnivå. Den manuella slajdern finns kvar som fallback när mikrofonen är tyst.

### P7 — MCU/OSC hårdvarukontroller · ✅ REAL
Ny modul `src/audio/hardware_control.rs`:
- **OSC:** riktig UDP-server (`OscServer`) som lyssnar på vald port, parsar OSC-meddelanden (adress + typ-taggar `,f`/`,i`/`,s`, inkl. `#bundle`) och mappar till transport (play/stop/record), spårvolym/mute, buss- och VCA-volym. Mottagningsräknaren är verklig, och "Skicka Test Ping" skickar ett riktigt UDP-paket.
- **MCU:** riktig ALSA-sequencer-ingång (`McuInput`) via `alsa`-craten. Öppnar en `Sonix MCU In`-port, läser Note/CC/PitchBend-händelser och mappar MCU-transportknappar, mute-knappar och motorfaders till appen. Enhetslistan hämtas från ALSA:s klient/port-uppräkning; anslutning sker med t.ex. `aconnect`.
- Appen pollar `ControlEvent` varje frame (`poll_hardware_control`) och applicerar dem (transport, spårvolym/mute som synkas till motorn, buss/VCA, MIDI Learn).
Inga påhittade `mcu_connected`/`osc_rx_count`-värden kvar; allt speglar verklig anslutning/trafik.

### P8 — Remix FX: Stutter / Reverse · ✅ REAL (implementerad)
Motorn har nu `RemixFx` (beat-repeat 1/4, 1/8, 1/16 samt reverse) och `TapeStop` (varispeed-ramp till tystnad) i `src/audio/master_fx.rs`, applicerade på master-bussen i `process_stereo`. Nya kommandon `SetRemixFx { mode, bpm }` och `SetTapeStop { active }`. Tape Stop ändrar inte längre projektets BPM utan är en riktig effekt; stutter-knapparna skickar tempo-medveten repeat till motorn. Enhetstester verifierar repeat och fade.

### P9 — Suno/AI-spårgenerering · ✅ REAL (procedural ljudmotor)
`trigger_suno_ai_generation` genererar nu riktig stereo-PCM via `render_generated_audio` (`src/audio/ai_generator.rs`) i stället för enbart en vågform. Fyra stilar finns: `Drums` (kick/snare/hihat-grid), `Bass` (sågtand + lowpass, ackordföljd i moll), `Synth` (detunade såg-pads) och `Vocal` (vibrato + formanter). Resultatet normaliseras, kopplas till `pcm_audio`, får en riktig peak-envelope som `audio_waveform` och laddas in i motorn via `sync_track_stem_to_engine` → spåren är hörbara. Multi-stem-läget skapar fyra synkade spår. Deterministiskt via prompt-seed (ingen extern modell, men verkligt ljud).

### P10 — Song Section Arranger: "Applicera" · ✅ REAL (implementerad)
Applicera bygger nu riktiga `SongMarker`-poster (`src/ui/app.rs:217`, fält `song_markers`) från sektionerna: namn, färg (`SectionType::name_and_color`), start-takt och längd, och renderar dem som ett markörband med etiketter i tidslinjens ruler (`app.rs:4964`). Loop-gränserna sätts fortfarande, men markörerna visas nu på tidslinjen.

### P11 — 3-bands EQ (mixer/kanal) · ✅ REAL (implementerad)
`TrackEqSettings` + `StereoEq` (low-shelf/peaking/high-shelf, L/R) i `src/audio/master_fx.rs`. Nytt kommando `AudioCommand::SetTrackEq { track_index, settings }` appliceras per stem/audio-spår i `SynthEngine::process_stereo`. `SonixApp::sync_track_audio_state`/`sync_track_stem_to_engine` skickar EQ, EQ-panelen och ON/OFF-pillen triggar sync, och offline-export bär `TrackAudioSnap.eq`.

### P12 — Dice/Idea Spark: döda reglage · ✅ REAL (implementerad)
`roll_dice` använder nu `genre_preset` (skala/root/fyra-på-golvet), `density`, `humanize_pct` (velocity/skip) och `syncopation_pct` (off-beat-förskjutning). Provspelningen är sekvenserad: `is_previewing` driver en 16-delsstegs scheduler (120 BPM) som triggar trummor/noter steg för steg via `trigger_preview_step`, med stoppknapp som skickar NoteOff.

### P13 — Session Drummer: döda reglage · ✅ REAL (implementerad)
`apply_drummer_generation` läser nu samtliga reglage: `drummer_kick_snare_var` väljer bland fyra kick/snare-varianter, `drummer_hihat_var` väljer bland fyra hi-hat-mönster, `drummer_fills` lägger till fill-slag i taktens slut och `drummer_toms_on` genererar ett tom-fill (TomHigh/TomLow via `drummer_tom_steps`/`drummer_tom_high`) som triggas i både live- (`trigger_step`) och låtuppspelning (`trigger_song_step`).

### P14 — Chord Matrix: infogning & audition · ✅ REAL (implementerad)
Infogning skriver nu hela voicingen: alla ackordtoner läggs i `piano_roll_grid` (utöver grundtonen i kanalnoten) och spelas upp polyfoniskt i både live- (`trigger_step`) och låtuppspelning (`trigger_song_step`). Provspelning använder nya `AudioCommand::StrumChord`, som schemalägger noter i `SynthEngine` (`scheduled_notes`, tickas i `process_stereo`): `strum_spread_ms` styr fördröjningen mellan ackordtonerna, `arpeggiator_mode` väljer Block/Upp/Ner/Slump, och "Provspela Hela Sekvensen" lägger ut ackorden i tid (ett ackord per taktslag) via `start_samples`. Reglagen har nu UI-kontroller i modalen.

### P15 — Add Track: import/stems-mallar · ✅ REAL (implementerad)
Import-mallarna ("📁 Ljudfil / Sample Import" och "📦 Suno AI Stems Spår") skapar inte längre tomma spår: `render_add_track_modal` returnerar en `TrackTemplate`-begäran, `SonixApp` öppnar en asynkron filväljare (`spawn_async_file_picker`, WAV-filter) och `import_audio_file_as_track` läser riktig PCM via `load_wav_pcm`, bygger vågform, lägger spåret i tidslinjen med en spelbar region och synkar till ljudmotorn (`sync_track_stem_to_engine`).

### P16 — Suno-import av komprimerade format · ✅ REAL (implementerad)
Ny `load_audio_pcm` (`src/audio/wav_reader.rs`) avkodar WAV nativt och skickar MP3/FLAC/OGG/M4A/AIFF m.fl. genom `ffmpeg` (`-f f32le -ac 2 -ar 44100`), med riktig stereo-PCM som resultat. `background_decode_stems` använder den för alla filändelser och ritar en ärlig platt linje om avkodningen misslyckas (ingen fejkad sinusvåg).

### P17 — Död kod / städning · ✅ KLAR
- `SynthEngine::process_sample` borttagen (`src/audio/synth.rs`).
- `PluginFormat::short_name` borttagen (`src/audio/plugin_host.rs`).
- `Preset::name` borttagen (`src/audio/command.rs`).
- Gamla exportvägen i `wav_writer.rs` (`render_to_wav`, `render_song_arrangement_to_wav`, `SongArrangementExport`) borttagen; endast `write_pcm_f32_to_wav` kvar (används av `recorder.rs`/`factory_samples.rs`). Re-export i `mod.rs` rensad.
- Fabricerad VU-mätare ersatt: mixerns fader använder nu `self.engine.get_peak_level()` (verklig uppmätt master-peak) i stället för `vol_val * 0.85` (`src/ui/app.rs`).
- Oanvända `PluginDescriptor`-fält (`is_loaded`, `is_fl_compatible`, `has_gui`) borttagna.
- Fejkad `populate_demo_pitch_blobs` (hårdkodade notblock) borttagen (`src/audio/vocal_harmonizer.rs`).

### P18 — AI Music Assistant: genereringen · ✅ REAL (regelbaserad)
`generate_from_prompt` (`src/audio/ai_generator.rs`) är nu en riktig generativ motor: den tolkar prompten för genre (Trap/Synthwave/Neo-Soul/House/Acid/Funk), tonart (t.ex. "a-moll", "c-dur"), BPM, syncopation/16-delar och mål (bas/lead/trum/ackord), och komponerar steg + noter utifrån skala (naturlig moll, dorisk, mixolydisk) och ackordgrader. Resultatet appliceras riktigt på kanal 6/7. Ingen extern modell/API krävs; statusraden är tydlig med att det är regelbaserat.

### P19 — Varningsstädning · ✅ KLAR (0 varningar)
`cargo build --release` och `cargo test --release` (43 tester) är nu helt varningsfria.
- Döda metoder/konstanter borttagna: `Compressor::gain_reduction_db`, `MasterFxChain::compressor_gain_reduction_db`, `DOUBLER_MAX`, `midi_to_freq` (`vocal_harmonizer.rs`).
- Oanvända enum-varianter borttagna: `PluginFormat::FlPresetFst`, `PluginCategory::Sampler`, `RegionDragMode::LoopRepeat`, hela `BusRouting`/`VcaGroup`.
- Oanvända fält borttagna: `AudioTake` (`take_number`/`start_bar`/`length_bars`/`is_muted`), `CustomSoundClip` (`id`/`pcm_samples`/`sample_rate`), `CompRegion` (helt), `VocalStudioTrack.name`, `ChannelStrip.target_bus`/`vca_group`/`pdc_latency_samples`, `PlaylistTrack.automation_enabled`, `PitchBlob`-frekvenser, `HarmonyVoice.pan`, `VocalHarmonizer.autotune_scale_snap`/`selected_blob_idx`, `FxRackState`-rester, samt `SonixApp`-fälten `eq_low/mid/high`, `metronome_volume`, `chorus_*`, `compressor_amount`, `vca_mutes`, `pdc_enabled`.
- `PatchProcessor::is_active` är `#[cfg(test)]` (används av tester); `RemixFx::is_active` används nu som guard i `process_stereo`.
- `OscServer` exponerar `bound_port`/`last_error` i MCU/OSC-modalen (verklig lyssningsport + fel visas, inte den begärda).
- Fejkad `"GR: -4.5 dB"` i FX-racket ersatt med neutral `"GR"`.

### P20 — AI-API-integration · ✅ KLAR (Etapp A + B + C + D)
Verklig HTTP-väg till en extern LLM och ljud-API med offline-first-fallback.
- Ny modul `src/audio/ai_client.rs`: `AiProvider` (`Offline`/`OpenAi`/`Anthropic`/`OpenRouter`/`Ollama`), `AiConfig` (provider, base_url, model, api_key, timeout, max_tokens, temperature), `config_path()`/`load()`/`save()` (skriver `~/.config/sonix/ai.json` med chmod `0600`), `is_ready()`, samt `generate_clips(&AiConfig, prompt) -> Result<Vec<AiGeneratedClip>, String>`.
- Transport: `ureq` + rustls (inget tokio/OpenSSL). Bearer-auth, timeout, OpenRouter-headers, robust statusfelhantering.
- **Etapp A** (OpenAI-kompatibelt `/chat/completions`): OpenAI, OpenRouter och Ollama.
- **Etapp B** (Anthropic native `/messages`): `x-api-key` + `anthropic-version`-headers, `system`-fält och `max_tokens`; svaret läses via `/content/0/text`. Modellväljare i UI (`known_models()` per provider + fritext).
- **Etapp C** (ljudgenerering → spår): `AudioProvider` (`Offline`/`OpenAiTts`/`Stability`), `generate_audio(&AiConfig, prompt, sekunder) -> Result<Vec<u8>, String>`. OpenAI TTS via JSON `/audio/speech`; Stability Stable Audio via `/v2beta/audio/{modell}/text-to-audio` (multipart byggd manuellt). `trigger_remote_audio_generation()` körs i bakgrundstråd och `poll_remote_audio_generation()` (i `update`) dekodar svaret med `load_audio_pcm` och importerar som riktigt spår; faller tillbaka på lokal DSP (`generate_suno_local`) vid fel/offline.
- **Etapp D** (kontextmedveten assistent): `GenContext` (projektnamn, BPM, tonart/`key_pc`, `is_minor`, valt spår, vald region) matas in via `SonixApp::refresh_ai_context()` och läggs till i API-prompten (`build_remote_prompt`) samt styr lokal generering (tonart/tempo) när `use_context` är på. UI-kryssruta `🎯 Använd projektkontext` + sammanfattning av aktiv kontext.
- Strikt JSON: systemprompt begär ett JSON-array av klip (`steps`/`notes` exakt 16, noter 0–127). Parsern tål markdown-staket och plockar första `[`/`{`; validerar längder och returnerar tydligt fel vid avvikelse.
- Env-overrides: `SONIX_AI_PROVIDER`, `SONIX_AI_API_KEY`, `SONIX_AI_BASE_URL`, `SONIX_AI_MODEL`, `SONIX_AI_AUDIO_PROVIDER`, `SONIX_AI_AUDIO_KEY`, `SONIX_AI_AUDIO_BASE_URL`, `SONIX_AI_AUDIO_MODEL`. Nycklar loggas aldrig.
- `AiMusicAssistant` (`src/audio/ai_generator.rs`): nya fält `config`/`last_error`/`pending` (trådning via `Arc<Mutex<Option<Result<..>>>>`), `use_context`/`context`, och metoder `generate()` (väljer API om `is_ready()`, annars lokal motor), `request_remote_generation()`, `poll_remote_generation()` (applicerar resultat, faller tillbaka på regelbaserad motor vid fel).
- UI (`src/ui/ai_assistant_view.rs`): pollar bakgrundsanropet, spinner + status, projektkontext-rad, samt en ihopfällbar konfig-panel (provider, bas-URL, modellväljare, API-nyckel som password, ljud-provider + ljud-modell, Spara).
- Tester: 10 nya enhetstester i `ai_client`/`ai_generator` (OpenAI-/Anthropic-svarsparse, markdown-staket, felaktig steglängd, offline ej redo, provider/modell-konsistens, ljud-provider-defaults, multipart-body, kontext i prompt, kontextstyrd lokal generering). `cargo test --release` = **53 tester**, 0 varningar.

### P21 — "Inget får vara fejk": sista genomgången · ✅ KLAR
Systematisk genomgång av återstående platshållare i Vocal Studio, FX-racket, `app.rs` samt ljudbackend. `cargo test --release` = **57 tester**, 0 varningar.

**Vocal Studio** (`recorder.rs`, `vocal_harmonizer.rs`, `vocal_studio_view.rs`, `engine.rs`, `app.rs`)
- `CustomSoundClip` lagrar nu riktig PCM (`pcm_samples`/`sample_rate`); `add_new_take` genererar en hörbar 220 Hz-testton istället för tystnad; `visual_peaks_from()` ritar verklig vågform.
- Demo-populering borttagen ur `Default`; `list_devices` fabricerar ingen mikrofon; inspelning/preview kräver riktig enhet (annars tydligt "standby"-läge). `start_recording`/`stop_recording` returnerar `Result`.
- `PitchBlob` har riktig `pitch_offset` (fejkad `text_lyric` borttagen); interaktiv blob-canvas (`Sense::click_and_drag`) + "Applicera notkorrigering" via `apply_blob_corrections()`.
- `AudioEngine` exponerar `audition_active`/`is_audition_playing()` som speglar den riktiga uppspelningen.

**FX Rack-modal** (`master_fx.rs`, `engine.rs`, `fx_rack_modal.rs`)
- Ny `MasterFxChain::gain_reduction_db()`; `AudioEngine` bär `master_gr_db: Arc<AtomicU32>` (`master_gain_reduction_db()`), så GR-mätaren visar verklig kompressor-reduktion.
- Fejkade fält (`noise_gate_thresh`/`filter_cutoff_hz`/`filter_res`) borttagna — gate-tröskel, filter-cutoff/resonans läses nu ur pedalkedjan; riktig mini-EQ-kurva + 4 node-gain-ratten.

**Mixer/EQ-konsol** (`app.rs`, `command.rs`, `synth.rs`)
- `track_dirty`-flagga gör att alla EQ-/comp-/send-/pitch-reglage faktiskt skickar `SetTrackMix`.
- Ny `AudioCommand::SetTrackMix`; `StemVoiceTrack` fick riktig per-spår-kompressor, reverb/delay-sends och tape-style `pitch_ratio` (tillämpas på `sample_pos`). Process-blocket kör EQ→comp→sends.

**Ljudbackend**
- `effects.rs`: `SimpleReverb::new(sample_rate)` skalar comb/allpass-längder efter samplingsfrekvens (anropare uppdaterade).
- `patcher.rs`: tidigare ignorerade pinnar kopplade — Oscillator **Sync** (hard-sync på stigande flank, nytt `prev_sync`) + **PWM** (pulskvot från `p3`+input), LFO **Rate CV**, Delay **Time CV**, Reverb **Mix CV**.
- `stem_separator.rs`: verklig `estimate_bpm()` (autokorrelation av onset-envelope); UI visar "— BPM" när tempot inte kan fastställas istället för påhittade 126.
- `plugin_host.rs`: ärlig metadata — `version` härleds ur filnamnet (`extract_version`, annars "—"), `is_sandboxed`/`sandboxing_enabled` (aldrig implementerad sandbox) borttagna; UI visar "✔ Verifierad binär" och ärlig körningsmodell ("endast metadata-skanning").
- `wav_reader.rs`: tom WAV-data ger nu tystnad (`vec![0.0]`) istället för fejkad `0.1`-envelope.
- Nytt test: `stem_separator::tests::bpm_estimate_detects_click_track` (detekterar 120 BPM ur klick-spår).

**Medvetet lämnat (ej fejk):** `populate_demo_data` körs endast i screenshot-läge (dokumentationsfixtur); `load_demo_project` bygger ett riktigt spelbart mönsterprojekt (endast progress-sleeps är simulerade). `factory_samples.rs` genererar alla 16 samples med riktig DSP.

### P22 — Dokumentation & ärlighet i README · ✅ KLAR
Full inventering (tre parallella granskningar av ljud-DSP, UI och I/O/AI/hårdvara) och omskrivning av `README.md` så att inget överlövas.
- Ny sektion **"Features & Implementation Status"** med statusmarkörer ✅/🟡/🔜 för samtliga 14 funktionsområden (timeline, channel rack, piano roll, mixer, synth, vocal studio, generatorer, patcher, AI, stem-separator, plugin-hanterare, export, hårdvara, system).
- Ny sektion **"What's Left & How Far From Real"** — tabell över kvarvarande luckor (patcher-topologi, neural stem separation, realtime-tuning, time-stretch, ljudinställningar, legacy AI-modal, `.fst`, plugin-hosting).
- Ny sektion **"Plugin Support — Current Reality"** — tydligt besked: plugin-hanteraren katalogiserar/verifierar men **kör inte** plugins; vad som krävs för riktig VST3/CLAP/LV2/VST2-host och FL Studio via Wine+yabridge.
- Rättade överlöften: Demucs-neural → DSP-approximation; "sandboxed/process-isolerad" borttaget; "FL Studio & VST3/CLAP-integration" → skanning/katalog; "Moog 24 dB ladder" → 2-polig SVF; VCA/sub-mix-bussar markerade som ej implementerade; "1.4 ms buffert" korrigerat; bitcrusher borttagen (finns ej); Suno API → endast import.
- Ärlig text i `plugins_view.rs` (FL/Yabridge-assistenten steg 2 & 4) + engelska översättningar i `i18n.rs`.
- Verifierat: `cargo test --release` = **57 tester**, 0 varningar.

### P23 — Modular Patcher: topologisk sortering · ✅ KLAR
Patcher-grafen utvärderas inte längre i skapandeordning. Ny `topo_order()` (`patcher.rs`) gör en Kahn-sortering över patchkablarna så att en källa alltid körs före sina mål — kablar kan dras i valfri ordning. `PatchProcessor` lagrar den beräknade `order` och `process()` itererar den i stället för `0..nodes.len()`.
- **Cykeldetektion:** `ModularGraph::has_cycle()` returnerar sant vid återkoppling; noder som inte kan sorteras läggs sist och använder föregående samples utsignal på feedback-vägen.
- **UI:** `patcher_view.rs` visar ett varningsband ("⚠ Återkoppling/cykel i patchen…") när `graph.has_cycle()` är sant. Engelsk översättning i `i18n.rs`.
- **Tester:** `topo_order_places_sources_before_targets`, `topo_order_detects_cycle_and_keeps_all_nodes`, `evaluation_order_is_independent_of_node_listing_order` (reverserad nodlista ger bit-identisk utsignal), `graph_reports_feedback_cycle`. `cargo test --release` = **61 tester**, 0 varningar.

### P24 — Etiketter matchar DSP:n · ✅ KLAR
Tog bort överlöften i nod-/filteretiketter så att de beskriver vad koden faktiskt gör.
- **Patcher:** `Dual Wavetable OSC` → `Oscillator`, `SVF 24dB Filter` → `SVF Filter (12 dB)`, `Stereo Delay` → `Delay`, `Shimmer Space Reverb` → `Space Reverb`, `Analog Tube Drive` → `Tube-style Drive` (både standardpresetet och `add_node`).
- **FX-rack:** pedalen `🎛 24dB FILTER` → `🎛 SVF FILTER (12 dB)`.
- **Synthen:** `MOOG 24dB FILTER` → `RESONANT SVF FILTER (12 dB)`; hjälptexten uppdaterad.
- **MANUAL.md:** "Moog-filter"/"Moog 24dB Resonant Lowpass" → resonant SVF (12 dB); rättade även felaktig vågform (`Noise` → `Triangle`).
- `cargo test --release` = **61 tester**, 0 varningar.

### P25 — Legacy AI-modal kopplad till riktig config · ✅ KLAR
Den gamla "AI Provider Inställningar"-modalen redigerade döda `ai_provider_*`-fält som aldrig lästes eller sparades.
- **Löst:** Modalen redigerar nu `self.ai_assistant.config` (samma `AiConfig` som AI Music Studio) och sparar via `config.save()` till `~/.config/sonix/ai.json` (chmod 0600). Text-provider + ljud-provider, bas-URL, modell och API-nycklar.
- **Borttaget:** Alla döda fält (`ai_provider_suno_key`, `ai_provider_stable_audio_key`, `ai_provider_openai_key`, `ai_provider_claude_key`, `ai_provider_ollama_endpoint`).
- **i18n:** Nya nycklar i `tr_en`; menyvalet döpt till "⚙ AI-inställningar & API-nycklar...".
- `cargo test --release` = **61 tester**, 0 varningar.

### P26 — Samplingsfrekvens & buffert appliceras på ljudströmmen · ✅ KLAR
Ljudinställningarnas reglage sparades men applicerades aldrig; enhetens standard användes.
- **Motor:** `AudioEngine::new_with()` tar emot sparade preferenser; `AudioEngine::reconfigure()` bygger om cpal-strömmen live med valt `SampleRate` + `BufferSize` och faller tillbaka på enhetens standard om kombinationen inte stöds. `apply_preferences()` kontrollerar stöd via `supported_output_configs()`.
- **Persistence:** `AudioSettings` (sample_rate, buffer_frames) sparas/laddas från `~/.config/sonix/audio.json`; main använder `new_with` vid start.
- **UI:** Modalen visar verklig cpal-värd/enhet och den aktiva strömmen; "🔁 Tillämpa på ljudströmmen" bygger om, sparar och återsänder all persistent synth-/mixer-/stem-state (`resync_engine_after_reconfigure`).
- **Borttaget (dött):** `audio_driver_idx` (kosmetisk drivrutinsväljare) och `audio_limiter_enabled` (kryssruta utan effekt; riktiga limitern sitter i FX-racket).
- `cargo test --release` = **61 tester**, 0 varningar.

### P27 — `.fst` "Apply Preset" är inte längre en no-op · ✅ KLAR
Knappen "⚡ Tillämpa Preset" visade en falsk bekräftelse utan att göra något (`.fst` avkodas inte och ingen plugin-värd finns).
- **Löst (Alternativ B):** Knappen är nu **inaktiverad** med tooltip: "Kräver plugin-hosting (VST/CLAP), vilket ännu inte är implementerat. Preseten visas endast som metadata." Undertexten klargör att `.fst` bara katalogiseras.
- **i18n:** Nya `tr_en`-nycklar.
- `cargo test --release` = **61 tester**, 0 varningar.

### P28 — Per-röst filter & filter-envelope i synthen (Fas 2.4) · ✅ KLAR
Filtret låg tidigare på synth-bussen (globalt). Eftersom SVF:en är linjär lät per-röst-filter med samma parametrar identiskt — den hörbara vinsten kräver per-röst modulation.
- **Löst:** `Voice` har nu eget `filter` (StateVariableFilter) och eget `filter_env` (AdsrVoice). Globala `SynthEngine.filter` borttaget (även i `exporter.rs`). Nytt `AudioCommand::SetFilterEnv { amount, adsr }`; cutoff = `base * 2^(amount * env)`. Ny ratt **"ENV ±oct"** (−6..+6) i filterpanelen; standard 0 = ingen ljudförändring. Envelope-parametrar är patch-globala (rattar hörs live) men varje tons envelope löper oberoende.
- **Tester:** `filter_env_zero_amount_is_transparent`, `filter_env_amount_changes_timbre`, `voices_have_independent_filter_envelopes`.
- `cargo test --release` = **64 tester**, 0 varningar.

### P29 — Riktig formantbevarande pitch-shift (Fas 2.2) · ✅ KLAR
`apply_formant_tilt` var en billig 1-polig spektral-tilt som inte bevarade formanter (pitch-shift lät "chipmunk").
- **Löst:** Ny `formant_preserving_shift(input, sr, pitch, formant)` i `vocal_harmonizer.rs`. Granulär pitch-shift följt av STFT (egen radix-2 FFT, ingen ny dependency) där cepstral liftering skattar formant-envelopen hos original och pitchad signal, och varje frame skalas så den pitchade signalens envelop matchar originalets (valfritt frekvenswarpad av `formant_shift`, β = 2^(semi/12)). Endast magnitud korrigeras; pitchad fas behålls. `apply_formant_tilt` borttagen; `render_harmony` använder den nya funktionen.
- **Test:** `formant_preserving_shift_keeps_formant_while_shifting_pitch` (syntetisk vokal: +12 st flyttar f0 120→240 Hz medan formanten stannar nära 700 Hz).
- `cargo test --release` = **65 tester**, 0 varningar.

### P30 — Pitch-bevarande time-stretch (Fas 2.3) · ✅ KLAR
Stretch var varispeed: `AuditionVoice` multiplicerade uppspelningshastigheten med `time_stretch_ratio`, vilket ändrade både tempo och tonhöjd. `StemChannel.time_stretch` (SPEED-ratten) lästes aldrig av ljudmotorn.
- **Löst:** Ny **WSOLA**-sträckare `Wsola` i `vocal_harmonizer.rs`: Hann-fönstrade grains (frame 1024, hop 512 = 50 % överlapp), normaliserad korskorrelation för fasjustering med **coarse-to-fine**-sökning (search 128) så den klarar långa buffertar. `AuditionVoice` har nu ett `wsola`-fält; när stretch ≠ 1.0 i framlänges spel sträcks ljudet med WSOLA (`ratio = tsr·sr_ratio·pitch`) och resampplas sedan (`step = sr_ratio·pitch`) — tempo ändras, tonhöjd bevaras. `time_stretch()`-hjälpare för hela buffertar är kopplad till **SPEED**-ratten i Stem Separator: den tillämpas vid släpp och körs från `stem_audio_base` (originalet) så upprepade drag inte staplar artefakter. `rotary_knob_full()` rapporterar drag-släpp.
- **Test:** `wsola_time_stretch_preserves_pitch_and_scales_duration` (220 Hz-sin, 2.0× ≈ dubbel längd och 0.5× ≈ halv längd, tonhöjd kvar på 220 Hz).
- `cargo test --release` = **66 tester**, 0 varningar.

### P31 — Realtids-autotune i ljudtråden (Fas 2.1) · ✅ KLAR
Tuning kördes bara offline på den inspelade bufferten, och `Direct Monitoring`/`monitoring_on` var döda kontroller — det fanns ingen monitorväg alls.
- **Löst:** Ny **`RealtimeAutotune`** i `vocal_harmonizer.rs`: rullande 2048-sample analys, pitchdetektering var 1024:e sample, `snap_midi_to_scale` mot root/skala, korrigering begränsad till ±2 semitoner och utjämnad per sample (`coeff = 0.0002 + speed·0.0015`). Ny **`StreamingPitchShifter`** (WSOLA över cirkelbuffert, frame 512 / hop 256, korskorrelationssökning ±256, ~12 ms latens) gör själva pitchskiftet. `LiveMicrophoneCapture` fick `monitor_ring`, `monitor_enabled` och `autotune_*`-atomics; `process_mic_block` tar `&mut RealtimeAutotune` och fyller monitor-ringen med det autotunade (torr signal spelas in). Streambygget refaktorerades till gemensam `build_input_stream()` (tar bort dupliceringen mellan `new()` och `open_device_by_index()`). Ny `AudioCommand::SetMonitorRing` registrerar ringen i `SynthEngine`, som dränerar den per block och mixar in den i master-bussen i `process_stereo` — riktig noll-latens direktlyssning. `sync_live_effects()` och `SonixApp::sync_mic_monitoring()` synkar monitor/autotune varje frame (och `SetMonitorRing` återsänds efter reconfigure). UI: kryssrutan **🎙️ Realtids-Auto-Tune** i Sångstudion (styrka = AUTO-TUNE-ratten).
- **Tester:** `streaming_pitch_shifter_shifts_up_octave` (220 Hz → ~440 Hz vid ratio 2.0), `realtime_autotune_corrects_flat_note` (30 cent flat A4 dras till 440 Hz).
- `cargo test --release` = **68 tester**, 0 varningar.

### P32 — Automationskurvor på tidslinjen (Fas 5.4) · ✅ KLAR
Spårparametrar (volym, panorering, reverb/delay-send) kunde bara sättas statiskt — ingen möjlighet att rita tidsvarierande kurvor.
- **Löst:** Ny datamodell i `app.rs`: `AutomationParam` (Volym, Panorering, Reverb-send, Delay-send med `range()`/`index()`), `AutomationPoint { time_secs, value }` och `AutomationLane { param, enabled, points }` med linjär interpolering via `value_at()` (konstant före första/efter sista punkten, sorterade punkter). `PlaylistTrack` har `automation: Vec<AutomationLane>` + `automation_last: [f32; 4]` (cache). Kurvorna sparas/laddas i projektet via `SavedTrackData.automation` och `PreloadedTrackData.automation` (`#[serde(default)]` → gamla projekt laddar utan kurvor). Under uppspelning kör `SonixApp::apply_automation()` varje frame: utvärderar alla aktiva kurvor vid `song_time` och skickar bara **ändrade** värden till motorn (`SetStemTrackState` för volym/pan, `SetTrackMix` för sends), så inga kommandon spammas. Cachen nollställs vid play-start. UI: **📈 Automation: PÅ/AV** + parameterväljare i ROW 2, samt en dedikerad redigeringsfil under Master-spåret (`render_automation_lane`): vänsterklicka = lägg till punkt, dra = flytta (snäpps med valt `TimeSnapMode`), högerklicka = ta bort närmaste punkt. Spelhuvudet sträcker sig nu ner över automationsfilen.
- **Tester:** `test_automation_value_at_interpolates`, `test_automation_lane_serde_roundtrip`, `test_automation_param_ranges_and_index`.
- `cargo test --release` = **71 tester**, 0 varningar.

### P33 — MIDI-klaviaturinspelning till Piano Roll (Fas 5.3) · ✅ KLAR
Det gick att spela med datortangentbordet men inte med ett riktigt MIDI-klaviatur, och inget kunde spelas in i Piano Roll.
- **Löst:** Ny modul `src/audio/midi_input.rs` med `MidiKeyboardInput`, som öppnar en ALSA-sequencer-in-port (`Seq::open(None, Capture)`, klient `"Sonix Keys"`, port `"Sonix MIDI In"`) och skickar note-on/off som den nya `ControlEvent::MidiNote { note, velocity, on }` (velocity 0 = note-off). **Ingen `midir`-dependency behövdes** eftersom projektet redan använder `alsa`-craten — funktionellt likvärdigt och äkta. Ren, testad hjälpfunktion `note_to_roll_offset(note, base, rows)` transponerar hela oktaver så att toner utanför rutnätet hamnar på rätt rad. I `app.rs`: `handle_midi_note()` spelar via nya `play_note_velocity()` och, när `midi_record_armed` är på och transporten rullar, skriver `record_midi_note_at_step()` in hållna toner i `piano_roll_grid`/kanal 6 och synkar mönstret; `advance_sequencer()` håller kvar hållna toner över steg. Auto-anslutning sker en gång vid start; status, enhetslista och anslut/koppla-från finns i Hårdvarukontroller-modalen, plus MIDI-indikator och **⏺ MIDI-REC**-knapp i Piano Roll-headern. Fält: `midi_input`, `midi_keyboard_connected`, `midi_device_name`, `midi_note_count`, `midi_record_armed`, `midi_held_notes`.
- **Tester:** `note_to_roll_offset` (in-range, oktavtransponering, kort rutnät, tomt rutnät) i `midi_input.rs`.
- `cargo test --release` = **75 tester**, 0 varningar.

---

## 3. Sammanfattning

| Verktyg | Status |
|---|---|
| Alchemy-synth, trummor, delay/reverb, filter, envelope | REAL |
| Timeline/arranger, Channel Rack, Piano Roll, export | REAL |
| Automationskurvor (volym/pan/sends) | ✅ REAL (P32) |
| MIDI-klaviaturinspelning till Piano Roll | ✅ REAL (P33) |
| Mikrofoninspelning, vocal audition, factory-samples | REAL |
| Session Drummer | REAL (P13) |
| Dice Generator | REAL (P12) |
| Chord Matrix | REAL (P14) |
| AI Music Assistant | REAL (P18) |
| AI-API-integration (LLM + ljud + kontext) | ✅ REAL (P20, Etapp A–D) |
| Suno-import | REAL (P16) |
| Add Track | REAL (P15) |
| Song Section Arranger | REAL (P10) |
| Strobe Tuner | REAL (P6) |
| EQ (3-bands) | REAL (P11) |
| Remix FX (stutter/reverse) | REAL (P8) |
| MCU/OSC | REAL (P7) |
| FX Rack | REAL (P1) |
| Modular Patcher | ✅ REAL (P2) |
| AI Stem Separation | ✅ REAL (P3) |
| Plugin-host (ladda/GUI) | ✅ REAL (P4, ärlig hanterare) |
| Vocal Harmonizer / Auto-Tune | REAL (P5) · realtids-autotune + direktlyssning REAL (P31) |
| Suno/AI-spårgenerering | REAL (P9) |

**Föreslagen arbetsordning:** P1 → P11 → P6 → P5 → P8 → P10 → P12 → P13 → P14 → P15 → P16 → P18 → P7 → P9 → P2 → P3 → P4 → P17. (**Alla prioriteter klara.**)
