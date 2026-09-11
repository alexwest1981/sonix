# 🗺️ Sonix Studio — Roadmap

> Levande utvecklingsplan. **Bocka av `[x]` allt eftersom.** Uppdatera procenten i [Framsteg](#-framsteg-i-siffror) när något blir klart.
>
> Läs tillsammans med **[README.md](README.md)** / **[README_SV.md](README_SV.md)** (funktionslista & ärlig status) och **[tools.md](tools.md)** (detaljerad historik P1–P27).

**Teckenförklaring**

| Markering | Betydelse |
| :--- | :--- |
| ✅ Klar | Implementerad, inkopplad och verifierad (byggt/testat) |
| 🟡 Pågår / delvis | Fungerar delvis eller är en approximation |
| ⬜ Väntar | Inte påbörjad |
| **S** / **M** / **L** / **XL** | Uppskattad storlek: liten / medel / stor / mycket stor |

---

## 📊 Framsteg i siffror

**Totalt: ~99 % klart** (av det som gränssnittet utlovar)

```text
[██████████████████████████████████████]  99 %
```

| Område | Klart | Kvar | Procent |
| :--- | :---: | :---: | :---: |
| Ljudmotor (synth, trummor, FX, patcher, per-spår) | 12 | 0 | **100 %** |
| Sequencer & arranger (timeline, rack, piano roll, sektioner, automation) | 6 | 0 | **100 %** |
| Inspelning & sång (mic, takes, comping, pitch, harmonier) | 7 | 0 | **100 %** |
| Generatorer (ackord, tärning, drummer, tuner, add track) | 5 | 0 | **100 %** |
| AI (lokal + LLM + ljud + kontext) | 4 | 1 | **80 %** |
| Stem-separation (DSP + neural ONNX) | 1 | 0 | **100 %** |
| Plugin-hantering | 10 | 1 | **91 %** |
| Export & projekt-I/O (presets, loudness-normalisering) | 5 | 0 | **100 %** |
| Hårdvara (MCU/OSC/MIDI) | 3 | 0 | **100 %** |
| Lokalisering & system (7 språk, motor) | 3 | 0 | **100 %** |
| Dokumentation (README, ROADMAP, tools) | 3 | 0 | **100 %** |

> **Största kvarvarande biten:** **Plugin-hosting** (91 % — en CLAP-värd kan ladda plugins, läsa parametrar, **processa ljud i ett spår med PDC**, **spara/ladda plugin-state i projektet och applicera pluginens egna presets**, **läsa och driva `clap.gui`-livscykeln på huvudtråden**, **köra en plugin i en separat process — med kraschdetektering, automatisk omstart och ljud över delat minne**, samt **ladda, inspektera och spela både VST3- och VST2-moduler via handrullade ABI:er med parametrar, state och PDC**; kvar är att verifiera en **riktig** yabridge-brygga (Wine) och X11-fönstret på en riktig display). Neural stem-separation är byggd (opt-in via `--features neural` + en HTDemucs-ONNX).

### Nya områden — Fas 6–9 (ingår **inte** i de 99 % ovan)

Siffrorna ovan mäter **det gränssnittet redan utlovar**. Fas 6–9 är nytt scope, riktat mot att Sonix ska hålla professionell nivå — inte bara hålla vad UI:t lovar.

| Område | Klart | Kvar | Procent |
| :--- | :---: | :---: | :---: |
| **Tier 0** — Trovärdighet (sökvägar, autosave, undo, MIDI-I/O, kvantisering, dither, rundgång, **projektfilen**) | 6 | 8 | **75 %** |
| **Tier 1** — Plattform & prestanda (backend-utbrytning, realtidsmätning, yabridge, starttid) | 0 | 4 | **0 %** |
| **Tier 2** — Arbetsflödesdjup (freeze, tempo map, routing, sampler) | 0 | 4 | **0 %** |
| **Tier 3** — AI-kilen (agent, lokal modell, moln-API) | 0 | 3 | **0 %** |

> **Prioritet just nu: Tier 0 (Fas 6).** Ordningen är inte förhandlingsbar: en proffsmusiker som tappat ett projekt en gång bryr sig inte om hur bra AI:n är. Tier 0 mäts i att inget arbete går förlorat och att allt går att ångra.

---

## ✅ Fas 0 — Klar baslinje (det som redan är äkta)

- [x] **Kärnmotor** — oscillatorer, trumsyntes, delay/reverb, filter/envelope, master-FX-kedja, Remix FX, patcher-DSP.
- [x] **Sequencer** — tidslinje/multitrack, Channel Rack (16 steg), Piano Roll, låtsektioner, transport/loop.
- [x] **Mixer** — faders, riktiga VU-mätare, per-spår EQ/kompressor/sends/pitch, GR-mätare.
- [x] **Sångstudio** — mikrofoninspelning, take lanes/comping, pitch-editor, offline autotune, 4-stämmig harmonizer, sampler.
- [x] **Generatorer** — ackordmatris, tärningsgenerator, Session Drummer, strobetuner, spårskapare.
- [x] **AI** — lokal regelbaserad kompositör, extern LLM (OpenAI/Anthropic/OpenRouter/Ollama), ljud-API (OpenAI TTS / Stability), projektkontext.
- [x] **Stem-import** — Suno ZIP-import + komprimerade format via `ffmpeg`.
- [x] **Export** — WAV/FLAC i appen, MP3/OGG/AAC via `ffmpeg`, ren metadata, projekt spara/ladda.
- [x] **Hårdvara** — riktig ALSA MIDI (MCU) + UDP OSC-server.
- [x] **Plugin-katalog** — rekursiv skanning, ELF/PE-verifiering, Wine/yabridge-detektion, `yabridgectl sync`.
- [x] **System** — cpal/ALSA-realtidsmotor, kraschsäker callback, 7 språk.
- [x] **Ärlighet** — P1–P22 avklarade; 57 tester, 0 varningar.

---

## ✅ Fas 1 — Ärlighet & snabba vinster (KLAR)

Små, tydliga uppgifter som tar bort kvarvarande glapp mellan UI och funktion.

- [x] **1.1 Topologisk sortering i Modular Patcher** — *S* ✅
  - **Löst:** Kahn-sortering i `topo_order()` (`patcher.rs`); processorn utvärderar nu alltid källor före mål oavsett kabelordning. Cykler upptäcks och de noder som inte kan sorteras körs sist (föregående samples utsignal på återkopplingsvägen).
  - **Klart när:** En kabel kan dras "baklänges" och ljudet följer ändå grafen; cykel-test finns. ✅
  - **Filer:** `src/audio/patcher.rs`, `src/ui/patcher_view.rs` (varningsband vid cykel)

- [x] **1.2 Matcha patcher-nodetiketter mot DSP** — *S* ✅
  - **Löst:** `Dual Wavetable OSC` → `Oscillator`, `SVF 24dB Filter` → `SVF Filter (12 dB)`, `Stereo Delay` → `Delay`, `Shimmer Space Reverb` → `Space Reverb`, `Analog Tube Drive` → `Tube-style Drive`. Även samma överlöften utanför patchern: FX-pedalen `24dB FILTER` → `SVF FILTER (12 dB)`, synthens `MOOG 24dB FILTER` → `RESONANT SVF FILTER (12 dB)`, hjälptexten och `MANUAL.md` (inkl. felaktig "Noise"-vågform → Triangle).
  - **Klart när:** Varje nod-etikett beskriver exakt vad koden gör. ✅
  - **Filer:** `src/audio/patcher.rs`, `src/ui/patcher_view.rs`, `src/ui/fx_rack_modal.rs`, `src/ui/app.rs`, `src/i18n.rs`, `MANUAL.md`

- [x] **1.3 Legacy "AI Provider Inställningar"-modal** — *S* ✅
  - **Löst:** Modalen redigerar nu den **riktiga** `AiConfig` (samma som AI Music Studio) och sparar till `~/.config/sonix/ai.json` (chmod 0600). Alla döda `ai_provider_*`-fält (`suno_key`, `stable_audio_key`, `openai_key`, `claude_key`, `ollama_endpoint`) är borttagna. Menyvalet döpt till "⚙ AI-inställningar & API-nycklar...".
  - **Klart när:** Ingen död konfig-yta finns kvar. ✅
  - **Filer:** `src/ui/app.rs`, `src/i18n.rs`

- [x] **1.4 Applicera samplingsfrekvens & buffertstorlek** — *M* ✅
  - **Löst:** `AudioEngine::new_with()` tar emot sparade preferenser och `AudioEngine::reconfigure()` bygger om cpal-strömmen live med valt `SampleRate` + `BufferSize` (faller tillbaka på enhetens standard om kombinationen inte stöds). Modalen visar verklig värd/enhet/aktiv ström, applicerar valet och sparar till `~/.config/sonix/audio.json`; efter omstart återsänds synth-/mixer-/stem-state (`resync_engine_after_reconfigure`). Tog även bort den döda "limiter"-kryssrutan och den kosmetiska drivrutinsväljaren.
  - **Klart när:** Val i Ljudinställningar påverkar faktiskt strömmen. ✅
  - **Filer:** `src/audio/engine.rs`, `src/audio/mod.rs`, `src/main.rs`, `src/ui/app.rs`, `src/i18n.rs`

- [x] **1.5 `.fst` "Apply Preset"** — *S* ✅
  - **Löst (Alternativ B):** Knappen är nu inaktiverad och förklarar i tooltip att applicering kräver plugin-hosting (VST/CLAP) som ännu inte finns. Undertexten klargör att `.fst` endast katalogiseras som metadata (ingen avkodning). Ingen falsk "Applicerade preset"-bekräftelse längre.
  - **Klart när:** Ingen no-op-knapp finns kvar. ✅
  - **Filer:** `src/ui/plugins_view.rs`, `src/i18n.rs`

---

## ✅ Fas 2 — Ljudkvalitet & realtid (KLAR)

- [x] **2.1 Realtids-autotune i ljudtråden** — *L* ✅
  - **Löst:** Ny **`RealtimeAutotune`** (i `vocal_harmonizer.rs`) som körs i mikrofon-callbacken: rullande 2048-sample analysfönster, pitchdetektering var 1024:e sample, snapning mot vald skala/root via `snap_midi_to_scale`, korrigering begränsad till ±2 semitoner och utjämnad per sample (coefficient `0.0002 + speed*0.0015`). Pitchskiftet görs av en ny **`StreamingPitchShifter`** (WSOLA över cirkelbuffert, 512-frame/256-hop, korskorrelationssökning, ~12 ms latens). **Direktlyssning** är nu verklig: mikrofonens efter-autotune-signal skickas via en delad ring (`AudioCommand::SetMonitorRing` → `SynthEngine::monitor_ring`) och mixas in i master-bussen i `process_stereo` (noll extra latency). `LiveMicrophoneCapture` fick `monitor_enabled`/`autotune_*`-atomics, `process_mic_block` tar `&mut RealtimeAutotune`, och streambygget refaktorerades till en gemensam `build_input_stream`-hjälpare (tog bort dupliceringen mellan `new()` och `open_device_by_index()`). UI: kryssrutan **🎙️ Realtids-Auto-Tune** i Sångstudion (styrka = AUTO-TUNE-ratten). Inspelning sker torrt; autotunen hörs i monitor.
  - **Klart när:** Sång kan korrigeras live med hörbar effekt och acceptabel latens. ✅ (tester: `streaming_pitch_shifter_shifts_up_octave`, `realtime_autotune_corrects_flat_note`)
  - **Filer:** `src/audio/vocal_harmonizer.rs`, `src/audio/recorder.rs`, `src/audio/synth.rs`, `src/audio/command.rs`, `src/ui/app.rs`, `src/ui/vocal_studio_view.rs`, `src/i18n.rs`

- [x] **2.2 Riktig formantbevarande pitch-shift** — *M* ✅
  - **Löst:** Ersatte den gamla spektral-tilt-approximationen med en riktig **STFT-baserad formantkorrigering**: granulär pitch-shift följt av cepstral envelop-analys (radix-2 FFT, inbyggd, ingen ny dependency) där varje frame skalas så att den pitchade signalens spektrala envelop matchar originalets (valfritt frekvenswarpad av `formant_shift`). `apply_formant_tilt` borttagen.
  - **Klart när:** Pitch-shift ändrar tonhöjd utan "chipmunk"-artefakter. ✅ (test: `formant_preserving_shift_keeps_formant_while_shifting_pitch`)
  - **Filer:** `src/audio/vocal_harmonizer.rs`

- [x] **2.3 Pitch-bevarande time-stretch** — *L* ✅
  - **Löst:** Ny **WSOLA**-sträckare (`Wsola` i `vocal_harmonizer.rs`): Hann-fönstrade grains med 50 % överlapp, normaliserad korskorrelation (coarse-to-fine) för fasjustering. Audition-uppspelningen i Sångstudion (`AuditionVoice`) använder nu WSOLA när stretch ≠ 1.0 i framlänges rakt spel, och resampplar sedan för pitch/rate — så tempo ändras utan att tonhöjden följer med (varispeed tidigare). Även `time_stretch()`-hjälpare för hela buffertar, kopplad till **SPEED**-ratten i Stem Separator (tillämpas vid släpp, körs från original-ljudet så den inte staplas).
  - **Klart när:** Tempo kan ändras utan att tonhöjden ändras. ✅ (test: `wsola_time_stretch_preserves_pitch_and_scales_duration`)
  - **Filer:** `src/audio/vocal_harmonizer.rs`, `src/audio/synth.rs`, `src/audio/stem_separator.rs`, `src/ui/stem_view.rs`, `src/ui/widgets.rs`, `src/ui/app.rs`

- [x] **2.4 Per-voice filter & ADSR i Alchemy-synthen** — *M* ✅
  - **Löst:** Varje röst har nu eget filtertillstånd (`Voice.filter`) och ett eget **filter-envelope** (`Voice.filter_env`) vars nivå modulerar cutoff per ton (i oktaver). Globala filterfältet på synth-bussen är borttaget. Ny ratt **"ENV ±oct"** i filterpanelen (−6..+6) skickar `AudioCommand::SetFilterEnv`. Standard 0 → ingen ljudförändring. Envelope-*parametrarna* (ADSR + filter-envelope-form) är fortfarande patch-globala så rattar hörs live, men varje tons envelope löper oberoende.
  - **Klart när:** Ackord kan ha oberoende filter/envelope per ton. ✅ (tester: `voices_have_independent_filter_envelopes`, `filter_env_amount_changes_timbre`, `filter_env_zero_amount_is_transparent`)
  - **Filer:** `src/audio/synth.rs`, `src/audio/command.rs`, `src/audio/exporter.rs`, `src/ui/app.rs`

---

## ⬜ Fas 3 — Neural stem-separation

- [x] **3.1 Integrera HTDemucs via ONNX Runtime** — *XL* ✅
  - **Löst:** Ny modul `src/audio/neural_separator.rs` som kör en Demucs-familj (HTDemucs) ONNX-modell via `ort`. Beroendet är **opt-in** (`--features neural`), så standardbygget förblir offline/dependency-fritt. Modellen hittas via `$SONIX_DEMUCS_ONNX` eller i `~/.config/sonix/models/` (`htdemucs.onnx`, `demucs.onnx`, `htdemucs_ft.onnx`). Inferensen körs i en **bakgrundstråd med progress**: linjär resampling till 44,1 kHz, global normalisering (som Demucs `apply_model`), 7,8 s-segment med triangulär överlappning (crossfade) och layout-flexibel utläsning (`[B,S,C,T]`, `[B,S*C,T]`, `[S*C,T]`). När ingen modell finns (eller featuren är av) används den inbyggda DSP-separatorn som fallback och UI:t visar ärligt vilken backend som kördes. Stem-vyn har status för modellsökväg + en live-progressbar.
  - **Klart när:** Vokaler/trummor/bas/övrigt separeras med neural kvalitet och licensierbar modell; test finns. ✅ (modellen är användarens eget val; 10 nya tester för modell-sökning, resampling, segmentering, crossfade, utdata-layout och DSP-fallback — 111 tester, 0 varningar)
  - **Filer:** `src/audio/neural_separator.rs`, `src/audio/stem_separator.rs`, `src/audio/mod.rs`, `Cargo.toml`, `src/ui/app.rs`, `src/ui/stem_view.rs`, `src/i18n.rs`

---

## ⬜ Fas 4 — Plugin-hosting (största delen)

> Detta är det stora kvarvarande arbetet. Rekommenderad ordning nedan.

- [x] **4.1 Välj ABI + host-modul** — *L* ✅
  - **Löst:** Ny modul `src/audio/plugin_host_live.rs` som talar **CLAP 1.x C-ABI** direkt (ingen tung host-crate) och laddar `.clap`-filer med `dlopen` via `libloading`. Hela laddaren är **opt-in** (`--features plugin-host`), så standardbygget förblir dependency-fritt. Modulen löser upp en `.clap`-fil eller ett bundle (hittar rätt `.so`), validerar `clap_entry`, kör `entry.init()`, hämtar `clap.plugin-factory`, skapar en instans och läser **descriptor + parametrar** via `clap.params`. `deinit()`/`destroy()` körs i rätt ordning före `dlclose` (Drop-guards). `inspect(path)` ger en ärlig snapshot (info, parameterlista, eller fel) som UI:t visar; plugin-databasen har knappen **"🔎 Ladda & inspektera"** för verifierade CLAP-plugins med en parameterpanel. Ett riktigt mock-CLAP-plugin kompileras av `build.rs` och används i ett end-to-end-test.
  - **Klart när:** En CLAP-plugin kan laddas och rapportera sina parametrar. ✅ (7 nya tester: bundle/`.so`-upplösning, felhantering för saknad fil/paket utan binär/icke-CLAP-bibliotek, samt end-to-end-laddning av mock-pluginen med descriptor och 2 parametrar — 113 tester default, 120 med featuren, 0 varningar)
  - **Filer:** `src/audio/plugin_host_live.rs`, `src/audio/plugin_host.rs`, `src/audio/mod.rs`, `Cargo.toml`, `build.rs`, `tests/fixtures/mock_clap.c`, `tests/fixtures/empty.c`, `src/ui/plugins_view.rs`, `src/i18n.rs`

- [x] **4.2 Instansiering + audio/MIDI-routing + PDC** — *XL* ✅
  - **Löst:** CLAP-instansen kopplas nu in i realtidsgrafen. `PluginProcessor`-trait + `ClapProcessor` kör riktig ljudprocessning via `clap.audio-ports` (stereo in/ut), `clap.note-ports` läses in och `clap.latency` rapporteras. Eftersom motorn renderar sample-för-sample medan CLAP processar block buffrar `PluginInsert` `DEFAULT_BLOCK_FRAMES` (128) frames, kör pluginen på hela blocket och spelar ut resultatet sample-för-sample. `PdcDelay` + motor-PDC (`SynthEngine::process_stereo`) fördröjer icke-plugin-bussar till projektets maxlatens och varje plugin-spår med `max_latency - egen_latens`, så spåren förblir faslinjerade utan klick/kamfilter. `SetTrackPlugin`/`ClearTrackPlugin` (via `SetTrackPlugin { insert: None }`) och `SetPluginParameter` finns som kommandon; plugin-viewn har en **"▶ Ladda in"**-knapp per inspekterad plugin med spårväljare. Ett riktigt mock-CLAP-plugin kompileras av `build.rs` och processar ljud end-to-end i test.
  - **Klart när:** En plugin kan spela upp/processa ljud i ett spår utan klick. ✅ (PDC-alignment verifierad i motor-tester; 123 tester default, 131 med featuren, 0 varningar. MIDI: note-port-upptäckt + note-plumbing via mock, ingen motoromfattande MIDI-routing ännu.)
  - **Filer:** `src/audio/plugin_host_live.rs`, `src/audio/command.rs`, `src/audio/synth.rs`, `src/ui/plugins_view.rs`, `src/ui/app.rs`, `src/audio/plugin_host.rs`, `src/i18n.rs`, `tests/fixtures/mock_clap.c`

- [x] **4.3 State/preset save-load** — *M* ✅
  - **Löst:** CLAP-värden stödjer nu `clap.state` (opak state-blob via `clap_ostream`/`clap_istream`) och `clap.preset-load/2` (pluginens egna presets via `from_location`). `PluginProcessor`-traitet har `save_state`/`load_state`/`preset_load` (med default som säger "stöds ej"), och `PluginInsert` delegerar vidare. **State-operationerna körs alltid på huvudtråden** enligt CLAP-kontraktet: vid inladdning fångas en färsk state-blob innan processorn skickas till ljudtråden, och vid projektladdning instansieras + återställs plugin på huvudtråden. Projektet sparar nu `plugin_slots` (index-justerade mot motorns stämspår) med `.clap`-sökväg + state som `SavedPluginData`; äldre projekt utan fältet laddas oförändrat (`#[serde(default)]`). UI:t visar **"🎛 Aktiva plugin-inserts per spår"** med en **"🗑 Ta bort"**-knapp, samt ett **"Native preset (sökväg)"**-fält + **"▶ Ladda in med preset"**. Mock-pluginen (`tests/fixtures/mock_clap.c`) implementerar båda extensionerna.
  - **Klart när:** Projekt återställer plugin-state korrekt. ✅ (State round-trip + preset-load verifierade mot mock-pluginen; projekt-JSON-roundtrip och bakåtkompatibilitet testade. 125 tester default, 135 med featuren, 0 varningar.)
  - **Filer:** `src/audio/plugin_host_live.rs`, `src/ui/app.rs`, `src/ui/plugins_view.rs`, `src/audio/plugin_host.rs`, `tests/fixtures/mock_clap.c`
  - **Beroende:** 4.2

- [x] **4.4a Plugin-GUI-ABI + livscykel** — *M* ✅
  - **Löst:** CLAP-värden läser och driver nu **`clap.gui`** mot den fullständiga vtable:n (`is_api_supported`, `get_preferred_api`, `create`, `destroy`, `get_size`, `can_resize`, `set_size`, `set_parent`, `show`, `hide`; resterande fält deklareras för korrekt layout). `PluginProcessor`-traitet fick GUI-metoder (default "stöds ej"), `PluginInsert` delegerar, och `ClapProcessor` håller GUI-livscykeln (`gui_created`) samt river GUI:t i `Drop` före `deactivate`. `clap_window_t` skickas med korrekt `x11`-union. Inspektionen rapporterar nu pluginens GUI-kapacitet (`PluginGuiCapability`) och plugin-panelen visar **"🖼 Plugin-GUI: x11 320×240 (kan ändra storlek)"** eller **"stöds inte"**. Mock-pluginen (`tests/fixtures/mock_clap.c`) implementerar en riktig (headless) GUI-vtable.
  - **Klart när:** Hosten kan skapa/visa/dölja/ta bort en plugins GUI och läsa dess storlek. ✅ (Livscykel + `set_parent` + `set_size` + storleks-round-trip verifierade mot mock-pluginen. 125 tester default, 138 med featuren, 0 varningar.)
  - **Kvar till 4.4b:** själva X11-fönstret och den **delade plugin-instansen** (samma instans för ljud och GUI), samt en aktiv "öppna GUI"-knapp i spåret. → **Klart i 4.4b nedan.**
  - **Filer:** `src/audio/plugin_host_live.rs`, `src/ui/plugins_view.rs`, `src/i18n.rs`, `tests/fixtures/mock_clap.c`
  - **Beroende:** 4.2

- [x] **4.4b Plugin-GUI-fönster (X11)** — *L* ✅
  - **Löst:** Plugin-instansen delas nu mellan ljud- och huvudtråden: `ClapProcessor` håller bara ljudbuffertarna medan själva kärnan (`ClapCore`) ligger bakom `Arc<ClapCore>` (`unsafe impl Send + Sync`), och `PluginHandle(Arc<dyn PluginCore>)` är det huvudtråds-handtag som GUI:t använder. `PluginInsert::core_handle()` ger handtaget, och `App` håller en `PluginHandle` per spår så att kärnan aldrig droppas på ljudtråden (utbytta/borttagna plugins flyttas till `retired_plugin_handles` och släpps först vid app-exit). Ny modul `src/audio/plugin_gui.rs` skapar ett riktigt **X11-fönster** via `libX11` (dlopen med `libloading`, `XCreateSimpleWindow`, `XSetWMProtocols` för fönsterstängning) och bäddar in editorn med `gui_set_parent`; `GuiSession` sköter livscykeln (create → set_parent → show, hide → destroy i `Drop`) och pollar X11-händelser per UI-frame. Plugin-hanteraren har nu **"🪟 Öppna GUI" / "🪟 Stäng GUI"** per aktiv insert, och ett stängt fönster river sessionen automatiskt.
  - **Klart när:** Pluginens egna fönster kan öppnas och styra ljudet. ✅ (Delad-instans-invarianten + GUI-sessionens avvisande/ärliga fel verifierade headless mot mock-pluginen: `shared_core_outlives_the_processor`, `mock_plugin_reaches_the_x11_stage_without_display`. 127 tester default, 142 med featuren, 0 varningar. **OBS:** själva X11-fönstret kan inte köras i en headless miljö — det kompileras och ABI:n testas, men fönsterhosting kräver en riktig display.)
  - **Filer:** `src/audio/plugin_host_live.rs`, `src/audio/plugin_gui.rs`, `src/audio/mod.rs`, `src/ui/app.rs`, `src/ui/plugins_view.rs`, `src/i18n.rs`
  - **Beroende:** 4.4a

- [x] **4.5a Out-of-process sandbox — processgräns + krasch/omstart** — *M* ✅
  - **Löst:** Samma binär re-exekveras som en sandbox-arbetare (`--plugin-sandbox-worker`) och talar ett längdprefixat JSON-protokoll över stdin/stdout (`write_frame`/`read_frame`, 4-byte LE + payload, 64 MiB-tak). Arbetaren (`serve`) laddar pluginen en gång och svarar på `Ping`, `Info`, `Parameters`, `SetParameter`, `SaveState`, `LoadState`, `Reset` och `Shutdown`. Supervisorn `SandboxHost` spawnar/pollar arbetaren och **startar automatiskt om den vid krasch** (max 3 omstarter), med `SandboxState::{Running,Restarted,Crashed,Stopped}` och graceful `Shutdown` → kill. Plugin-hanteraren har nu **"🧪 Sandbox-inspektera"** som läser info + parametrar ur en separat process och visar status. Ljudet går ännu in-process (delat-minne-transporten är 4.5b), men en kraschande plugin kan nu detekteras och återstartas utan att Sonix stänger.
  - **Klart när:** En kraschande plugin kan återstartas utan att Sonix stänger. ✅ (Verifierat headless: `frames_round_trip`, `oversized_frame_is_rejected`, `worker_answers_the_control_protocol`, `supervisor_restarts_a_crashed_worker`, `supervisor_gives_up_after_the_restart_budget`, `shutdown_stops_supervision`. 127 tester default, 148 med featuren, 0 varningar i alla fyra byggkombinationer.)
  - **Filer:** `src/audio/plugin_sandbox.rs`, `src/audio/mod.rs`, `src/main.rs`, `src/ui/app.rs`, `src/ui/plugins_view.rs`, `src/i18n.rs`
  - **Beroende:** 4.2

- [x] **4.5b Out-of-process sandbox — delat-minne-ljudtransport** — *L* ✅
  - **Löst:** Själva ljudprocessningen körs nu i sandbox-arbetaren. Ny modul `src/audio/sandbox_audio.rs` skapar en anonym minnesregion (`memfd_create` + `ftruncate` + `mmap(MAP_SHARED)`) som både värd och arbetare mappar. Regionen har en atomisk `AudioHeader` (magic/version/layout, in-/ut-ringarnas index, heartbeat, underrun/overflow-räknare, reset- och shutdown-flaggor) följd av två SPSC-ringbuffertar (4 slots) med råa `f32`-stereoblock. Värden (`SandboxProcessor`, en riktig `PluginProcessor`) publicerar ett inputblock och tar ett outputblock per ljudcallback och rapporterar **`block_frames + plugin_latens`** så att motorns PDC kompenserar transporten; vid underrun/crash skickas tystnad och nästa block re-synkas. Arbetaren (`serve_audio_from_args`) kör en kontrolltråd (äger stdin/stdout) och en ljudloop som äger pluginen och pumpar ringen; `SandboxRequest::Latency` läser pluginens egen latens. Vid (om)attach sätter arbetaren `reset`, som värden kvitterar genom att nollställa ringarna, så en omstartad plugin aldrig spelar gammalt ljud. Plugin-hanteraren har knappen **"🧪 Ladda in i sandbox"**; appen håller en `SandboxHost` per stämspår (`plugin_sandboxes`) och startar om/tar bort vid upprepade krascher.
  - **Klart när:** En kraschande plugin tappar inte ljudet mer än en omstart och Sonix påverkas inte. ✅ (Verifierat headless: `blocks_round_trip_through_shared_memory`, `worker_processes_a_stream_in_lockstep`, `underrun_is_reported_when_the_worker_has_not_run`, `reset_gates_the_worker_until_the_host_acknowledges`, `attach_rejects_a_bad_descriptor`, `sandbox_processor_reports_transport_latency`, `audio_worker_streams_blocks_over_shared_memory`. 127 tester default, 155 med featuren, 0 varningar i alla fyra byggkombinationer. **OBS:** end-to-end med en riktig CLAP-plugin kan inte köras här — ingen plugin finns på disk och miljön är headless; transporten verifieras mot en syntetisk processor.)
  - **Filer:** `src/audio/sandbox_audio.rs`, `src/audio/plugin_sandbox.rs`, `src/audio/mod.rs`, `src/ui/app.rs`, `src/ui/plugins_view.rs`, `src/i18n.rs`, `Cargo.toml`
  - **Beroende:** 4.5a

- [ ] **4.6 Wine/yabridge-väg för FL Studio & Windows-VST** — *L*
  - **Gör:** Ladda `.so`-bryggor från yabridge som vanliga plugins; verifiera Sytrus/Harmor/Gross Beat/FL Studio VSTi.
  - **Klart när:** En yabridge-brygga kan spelas genom Sonix. (Både VST3-vägen — 4.6a/4.6b — och VST2-vägen — 4.6c — är klara och verifierade headless mot mock. Kvar: att köra en **riktig** yabridge-brygga med Wine + display.)
  - **Beroende:** 4.1–4.2

  - [x] **4.6a VST3-modul, ABI, laddning & inspektion** — *M* ✅
    - **Löst:** yabridge producerar VST3 (ELF-`.so`, inte bundles) och VST2 — inte CLAP. Ny modul `src/audio/plugin_vst3.rs` implementerar en **minimal, handrullad VST3-ABI** utan VST3 SDK (samma offline/lättviktsprincip som CLAP-hosten): modul-entrypoints (`InitDll`/`ExitDll`/`GetPluginFactory`), `#[repr(C)]`-layouter för `PFactoryInfo`/`PClassInfo`/`ParameterInfo`, vtable-structs i exakt ABI-ordning för `FUnknown`/`IPluginBase`/`IPluginFactory`/`IComponent`/`IEditController`, korrekt **non-COM big-endian** `INLINE_UID`-kodning och de riktiga IID:erna. `resolve_vst3_binary` hanterar både macOS/Windows-liknande bundles (`Contents/<arch>-linux/*.so`) och en direkt `.vst3`-ELF. `VstInstance`/`RawHandles` sköter livscykeln (queryInterface/terminate/release i rätt ordning, `ExitDll` efter objekten) och implementerar `PluginInstance`, så `inspect` returnerar riktig info + parametrar (namn, enheter, steg, flaggor mappade till CLAP-flaggor). `plugin_host_live::inspect` dispatchar `.vst3` till den nya modulen. Ljud/state skjuts medvetet till 4.6b (`load_processor` ger ett ärligt fel). Fixturen `tests/fixtures/mock_vst3.c` är en **äkta** VST3-modul (factory + component + controller + processor) som build.rs kompilerar och exponerar som `SONIX_MOCK_VST3`.
    - **Klart när:** En VST3-modul kan laddas och inspekteras med riktiga parametrar. ✅ (Verifierat headless mot mock: `detects_vst3_extension_case_insensitively`, `rejects_non_vst3_library`, `loads_mock_module_and_reports_parameters`, `inspect_snapshot_matches_load`, `missing_file_is_an_honest_error`. 127 tester default, 160 med featuren, 0 varningar i alla fyra byggkombinationer. **OBS:** verifieras mot mock-modulen, inte mot en riktig yabridge-brygga — ingen VST3/Wine finns på disk och miljön är headless.)
    - **Filer:** `src/audio/plugin_vst3.rs`, `src/audio/mod.rs`, `src/audio/plugin_host_live.rs`, `tests/fixtures/mock_vst3.c`, `build.rs`
    - **Beroende:** 4.1

  - [x] **4.6b VST3-ljud, parametrar/state & UI** — *L* ✅
    - **Löst:** `VstProcessor` (i `src/audio/plugin_vst3.rs`) implementerar nu `PluginProcessor` fullt ut: `IAudioProcessor::process` körs på riktiga stereo-block (`AudioBusBuffers`/`ProcessData`, `kRealtime`/`kSample32`), `getLatencySamples` rapporteras till motorns PDC, `IEditController::setParamNormalized` + realtids-`IParameterChanges` driver parametrar, `setActive`-cykeln används för `reset`, och state sparas/laddas via en host-ägd `IBStream` (`IComponent::getState`/`setState` + `IEditController::setComponentState`). `plugin_host_live::load_processor` dispatchar `.vst3` till den nya vägen (ersätter det tidigare ärliga felet), så en VST3-brygga laddas, processar ljud i ett spår med PDC och sparar/laddar state precis som CLAP-vägen. Fixturen `tests/fixtures/mock_vst3.c` fick riktig state-I/O (2 f64), `inputParameterChanges`-läsning och **8 samplars latens** (nollställs vid `setActive`) för att exercera PDC på riktigt.
    - **Klart när:** En yabridge-VST3-brygga kan spelas genom Sonix. ✅ (Verifierat headless mot mock: `loads_processor_and_reports_latency`, `processor_applies_parameter_changes`, `processor_state_round_trips`, `processor_reset_clears_the_latency_buffer`. 127 tester default, 164 med featuren, 0 varningar i alla fyra byggkombinationer. **OBS:** verifieras mot mock-modulen — en riktig yabridge-brygga kräver Wine + display, och VST2-bryggor täcks ännu inte.)
    - **Filer:** `src/audio/plugin_vst3.rs`, `src/audio/plugin_host_live.rs`, `tests/fixtures/mock_vst3.c`
    - **Beroende:** 4.6a

  - [x] **4.6c VST2-ABI (yabridge VST2-bryggor)** — *L* ✅
    - **Löst:** Ny modul `src/audio/plugin_vst2.rs` implementerar en **minimal, handrullad VST2-ABI** utan Steinberg-SDK (samma offline/lättviktsprincip som CLAP/VST3-hosten): `VSTPluginMain` (med `main`-fallback), en `#[repr(C)]` `AEffect` i exakt `aeffect.h`-ordning (192 byte på 64-bit), `'VstP'`-magikontroll och en riktig `audioMaster`-callback (version 2400, sample rate/block size via en registrerad `HostContext`, `kVstProcessLevelRealtime`, engelska, vendor/product). Dispatchar de opcodes som behövs för att beskriva och köra en effekt: `effOpen`/`effClose`, `effGetEffectName`/vendor/product/version, `effGetPlugCategory`, `effGetParamName`/`Label`/`Display`/`effGetParameterProperties` (steg via `kVstParameterIsSwitch`), `effSetSampleRate`/`effSetBlockSize`/`effSetProcessPrecision`/`effMainsChanged`, `effGetChunk`/`effSetChunk` (program-chunk, med f32-parameterfallback) och `effGetVstVersion`. `Vst2Processor` implementerar `PluginProcessor` fullt ut: `processReplacing` på riktiga stereo-block (mono/extra kanaler hanteras), `setParameter`/`getParameter`, `effMainsChanged`-cykeln för `reset`, program-chunk för `save_state`/`load_state` och `initialDelay` → motorns PDC. `plugin_host_live::{inspect,load_processor}` dispatchar `.so` (utan `.vst3`/`.clap`) till den nya modulen; CLAP-mocken döptes om till `.clap` så att den extensionsbaserade routningen är entydig. Fixturen `tests/fixtures/mock_vst2.c` är en **äkta** VST2-plugin (`VSTPluginMain` + dispatcher + `processReplacing` + chunk-state + 8 samplars latens) som build.rs kompilerar och exponerar som `SONIX_MOCK_VST2`.
    - **Klart när:** En yabridge-VST2-brygga kan laddas och spelas genom Sonix. ✅ (Verifierat headless mot mock: `detects_vst2_extension_case_insensitively`, `rejects_non_vst2_library`, `loads_mock_module_and_reports_parameters`, `inspect_snapshot_matches_load`, `missing_file_is_an_honest_error`, `loads_processor_and_reports_latency`, `processor_applies_parameters_and_latency`, `processor_state_round_trips`, `processor_reset_clears_the_latency_buffer`. 127 tester default, 173 med featuren, 0 varningar i alla fyra byggkombinationer. **OBS:** verifieras mot mock-modulen — en riktig yabridge-brygga kräver Wine + display.)
    - **Filer:** `src/audio/plugin_vst2.rs`, `src/audio/mod.rs`, `src/audio/plugin_host_live.rs`, `tests/fixtures/mock_vst2.c`, `build.rs`
    - **Beroende:** 4.6b


---

## ⬜ Fas 5 — Polish & utbyggnad (efter behov)

- [x] **5.1 Fler tester** för realtids- och plugin-vägar (integrationstester). ✅
  - **Löst:** 21 nya tester som täcker de tidigare otestade realtids-DSP-byggstenarna och plugin-vägen. Realtid: `envelope.rs` (ADSR attack→decay→sustain→release, idle tyst, reset), `filter.rs` (lågpass dämpar höga frekvenser, reset nollställer state), `drum.rs` (trigger låter och går till idle, reset tystar, **alla 10 rösttyper** ger ändligt/begränsat och hörbart ljud), `effects.rs` (delay-eko exakt efter delaytiden, bypass vid mix 0, reverb-svans med energi, bypass), `master_fx.rs` (kompressor dämpar över tröskel men släpper igenom under, limiter håller taket, gate stänger/öppnar, hela master-kedjan passerar ljud och rapporterar gain reduction), `engine.rs` (AudioSettings serde-roundtrip + default för partiella configfiler). Plugin: `plugin_host.rs` (skanning klassificerar `.clap`/`.vst3` med rätt format/kategori och verifierar ELF-magi; `.fst` hamnar som preset, aldrig som plugin).
  - **Klart när:** Realtids- och plugin-vägarna har tester som fångar regressioner. ✅ (101 tester, 0 varningar)
  - **Filer:** `src/audio/envelope.rs`, `filter.rs`, `drum.rs`, `effects.rs`, `master_fx.rs`, `engine.rs`, `plugin_host.rs`
- [x] **5.2 VCA-grupper & sub-mix-bussar** — *M*. ✅
  - **Löst:** Motorn har nu **4 sub-mix-bussar** (Vocal, Trummor, Synth, FX) och **4 VCA-grupper** (`NUM_BUSES`/`NUM_VCAS`/`BUS_NAMES` i `synth.rs`). Varje `StemVoiceTrack` har `bus: usize` + `vca: Option<usize>` (sätts med `AudioCommand::SetStemTrackRouting` och clampas i motorn). `AudioCommand::SetBusState`/`SetVcaState` styr volym/mute/solo per grupp. I render-loopen är ett spår hörbart om `any_solo` och (eget solo ELLER grupp-solo), annars om varken eget eller grupp-mute är på; grupp-gainen (`bus_volume * vca_volume`) appliceras **post-fader precis före mastern**. I mixern finns nu en **TIER 1B**-rad med 4 buss- och 4 VCA-strips (fader + M/S), och i den valda kanalens konsol finns **Buss:**- och **VCA:**-väljare per spår. Buss/VCA-state sparas/laddas i projektet (`SonixProjectData`/`SavedTrackData`, `#[serde(default)]` med unity-defaults) och följer med till **offline-exporten** (`RenderSpec` + `TrackAudioSnap`, neutralt vid enstaka stem-export). 6 nya tester (buss-/VCA-gain, mute, solo, routing-clamp).
  - **Klart när:** Spår kan routas till bussar/VCA:er som styr volym, mute och solo, och det hörs samt överlever spar/ladda och export. ✅
  - **Filer:** `src/audio/synth.rs`, `src/audio/command.rs`, `src/audio/exporter.rs`, `src/ui/app.rs`, `src/i18n.rs`
- [x] **5.3 Midi-inspelning/klaviatur-inmatning** (ALSA Seq) till Piano Roll. ✅
  - **Löst:** Ny modul `src/audio/midi_input.rs` med `MidiKeyboardInput` som öppnar en ALSA-sequencer-in-port (`"Sonix MIDI In"`, klient `"Sonix Keys"`) och vidarebefordrar note-on/off som `ControlEvent::MidiNote { note, velocity, on }`. (Projektet använder redan `alsa`-craten, så ingen ny `midir`-dependency behövdes.) Ren hjälpfunktion `note_to_roll_offset(note, base, rows)` transponerar hela oktaver så att toner utanför rutnätet hamnar på rätt rad (testad). I `app.rs` spelar `handle_midi_note()` upp toner via `play_note_velocity()` och, när `midi_record_armed` är på och transporten rullar, skriver `record_midi_note_at_step()` in dem i `piano_roll_grid`/kanal 6 och synkar mönstret. `advance_sequencer()` håller kvar hållna toner över steg. Auto-anslutning sker en gång vid start; status, enhetslista och anslut/koppla-från finns i Hårdvarukontroller-modalen, plus en MIDI-indikator och **⏺ MIDI-REC**-knapp i Piano Roll-headern. 4 nya tester (75 totalt, 0 varningar).
  - **Klart när:** En extern klaviatur kan spela upp och spela in toner i Piano Roll under uppspelning. ✅
  - **Filer:** `src/audio/midi_input.rs`, `src/audio/hardware_control.rs`, `src/audio/mod.rs`, `src/ui/app.rs`, `src/i18n.rs`
- [x] **5.4 Automationskurvor** på tidslinjen. ✅
  - **Löst:** Ny datamodell `AutomationParam` (Volym, Panorering, Reverb-send, Delay-send), `AutomationPoint` och `AutomationLane` med linjär interpolering (`value_at`). Varje `PlaylistTrack` har `automation: Vec<AutomationLane>`; kurvorna sparas/laddas i projektet (`SavedTrackData.automation` + `PreloadedTrackData.automation`, `#[serde(default)]`). Under uppspelning utvärderar `apply_automation()` alla aktiva kurvor vid `song_time` och skickar ändrade värden till motorn via `SetStemTrackState`/`SetTrackMix` (med per-parameter-cache så inga kommandon spammas). I arranger-vyn finns en **Automation PÅ/AV**-knapp + parameter-väljare i ROW 2, och en dedikerad redigeringsfil under Master-spåret: vänsterklicka lägger till punkter, dra flyttar (snäpps med valt `TimeSnapMode`), högerklicka tar bort. `value_at`-interpolering, serde-roundtrip och parameter-index/ranges testas (71 tester, 0 varningar).
  - **Klart när:** En kurva kan ritas per spår/parameter och hörs påverka ljudet under uppspelning samt överlever spara/ladda. ✅
  - **Filer:** `src/ui/app.rs`, `src/i18n.rs`
- [x] **5.5 Fler export-presets / loudness-normalisering.** ✅
  - **Löst:** Ny DSP-modul `src/audio/loudness.rs` med en **äkta** ITU-R BS.1770 / EBU R128-implementation: K-viktning (high-shelf + high-pass-biquads designade för valfri samplerate), grindad integrerad loudness (400 ms-block, 75 % överlapp, absolut grind −70 LUFS, relativ grind −10 LU) och **true peak** via 4× polyphase-oversampling (windowed-sinc-FIR). `normalize_loudness()` mäter, applicerar gain mot målet och sänker vid behov ytterligare så att äkta peak hamnar under taket. I exportmodalen finns nu en **preset-väljare** (Streaming/Apple Music/Broadcast/Club/FLAC Master som sätter format + samplerate + loudness i ett klick) och en **loudness-väljare** (Av, −14, −16, −23, −9 LUFS med −1/−0.3 dBTP-tak). Normaliseringen körs på master-mixen; stems lämnas orörda för extern mixning. Statusraden visar uppmätt och mål-LUFS.
  - **Klart när:** En exporterad master kan träffa ett valt LUFS-mål med äkta-peak-tak, verifierat av tester. ✅
  - **Filer:** `src/audio/loudness.rs`, `src/audio/mod.rs`, `src/ui/app.rs`, `src/i18n.rs`

---

## ⬜ Fas 6 — Trovärdighet inför professionell nivå (Tier 0)

> **Varför denna fas ligger först:** en DAW kan ha världens bästa AI och ändå vara oanvändbar professionellt om den tappar arbete, inte kan ångra mixerändringar eller inte pratar med andra DAW:er. Ingenting i Fas 7–9 är värt något förrän detta sitter.
>
> **Nuläge, verifierat i koden (2026-09-11):** **0 träffar** på `autosave`/`recover`/`backup`; undo-historiken i `app.rs` (`undo_stack`/`redo_stack`) matas från **14 anropsställen**, samtliga tidslinjeoperationer (klipp, duplicera/ta bort spår, klistra in, mute, reverse, rensa); **ingen** SMF/MIDI-filkod finns (`smf`/`midi_file`/`import_midi`/`export_midi` = 0 träffar); ingen dither (`dither` = 0 träffar); kvantisering finns bara som rutnätssnap vid inmatning (`snap_time_secs`, `piano_roll_snap_to_scale`), inte som efterarbete.
>
> **Sökvägar (samma genomgång):** sökvägar byggs på **10+ ställen** med egen `env::var("HOME")`-logik (inklusive en egen `expand_tilde` i `plugin_host.rs`), fyra filkategorier heter olika saker fast de betyder samma (`Samples` vs `User_Samples`, `Exporterat` vs "Renders"), konfigfilerna heter `config.json` (bara språk) + `audio.json` (ljud) utan inbördes system, ONNX-modeller (stora filer) ligger i **konfigkatalogen**, ljudtrådens kraschlogg skrivs till **musikmappen** (`~/Music/Sonix/audio_crash.log`), och två ställen faller tillbaka på en **hårdkodad `/home/alex`** (`app.rs:1408` exportmapp, `app.rs:11472` Suno-scan). Det finns ingen läslista för senaste projekt och ingen "visa i filhanteraren". → **6.0 löser detta först.**

- [x] **6.0 En enda sökvägsmodul + kanonisk filstruktur** — *M* ✅
  - **Löst:** `src/paths.rs` är nu den enda platsen som bygger sökvägar: XDG följs (`XDG_MUSIC_DIR` läses även ur `~/.config/user-dirs.dirs`, eftersom variabeln normalt inte är exporterad), `SONIX_PROJECTS_DIR`/`SONIX_SAMPLES_DIR`/`SONIX_CONFIG_DIR`/`SONIX_DATA_DIR`/`SONIX_STATE_DIR`/`SONIX_CACHE_DIR` överstyr, och `~/` expanderas. 15 anropsställen migrerade; de hårdkodade `/home/alex`-fallbackarna är borta (exportmappen ×2, Suno-scan, projektväljarens standardvärden, ett maskinberoende test). Kraschloggen flyttad från **musikmappen** till `~/.local/state/sonix/logs/`, sample-bibliotekets cache till `~/.cache/sonix/`, ONNX-modellerna till `~/.local/share/sonix/models/`, och sample-sökningen täcker nu även den kanoniska `Samples/` (tidigare försvann sparade samples ur webbläsaren vid omstart). `sonix --paths` skriver ut hela kartan med ✓/· per post. Migreringen flyttar men **raderar aldrig**, skriver aldrig över ett mål som redan har filer, och tål att köras varje start (verifierad mot isolerad `HOME`).
  - **Klart när:** Ingen modul utanför `paths.rs` bygger sökvägar av `$HOME` ✅ · `sonix --paths` listar kartan ✅ · migreringen testad ✅ · dokumenterad i README/MANUAL ✅ (kartan + `SONIX_*`-variablerna står nu i README.md, README_SV.md och MANUAL.md kap. 13).
  - **Kvar (medvetet):** plugin-databasen persisteras fortfarande inte, och `mic_settings.direct_monitoring` är en dubblett av `vocal_track.monitoring_on` som inte styr något (tas i **6.6** ✅ — där konstaterat att den fortfarande är död).
  - **Bevis:** 10 nya tester i `paths.rs` (kanonisk layout, fyra åtskilda rötter, `user-dirs.dirs`-parsning, överstyrningar, legacy-exportmapp, idempotent migrering, `ensure_dirs`) — 153 tester default, 199 med `plugin-host`, 0 varningar.
  - **Filer:** `src/paths.rs` (ny), `src/main.rs`, `src/ui/app.rs`, `src/ui/plugins_view.rs`, `src/audio/engine.rs`, `src/audio/synth.rs`, `src/audio/ai_client.rs`, `src/audio/neural_separator.rs`, `src/audio/factory_samples.rs`, `src/audio/plugin_host.rs`, `src/i18n.rs`
  - **Beroende:** —

- [x] **6.1 Autosave, kraschåterställning & versionshistorik** — *M* ✅
  - **Löst:** Ny modul **`src/autosave.rs`** (ren filsystemlogik, ingen GUI- eller ljudberoende) plus inkoppling i `app.rs`:
    - **Atomisk skrivning överallt.** `write_atomic()` skriver temp-fil i samma katalog och `rename`ar på plats — `save_project` använder den nu också (tidigare `fs::write` rakt på projektfilen). En avbruten skrivning kan därmed aldrig lämna en halv projektfil som den aktiva.
    - **Två utlösare.** Var 60:e sekund (`INTERVAL_SECS`) och efter **strukturella ändringar**. Den senare går inte via `push_undo()` direkt: `push_undo` anropas *först* i varje muterande funktion, alltså **innan** ändringen är genomförd — en autosave som skrevs där fångade läget före och hoppades över som "oförändrat". Mätt i GUI: `Ctrl+D` kl. 12:57:06 gav ingen skrivning alls; filen kom 12:58:11 (+65 s) och då från 60 s-timern. Ändringen *märks* därför i `push_undo` (`autosave_pending`, liksom i `undo`/`redo`) och skrivs vid **frame-gränsen** i `update()`, när ändringen är genomförd — max ~33 ms senare med fönstret fokuserat (200 ms oanvänt). Fortfarande högst en skrivning per 10 s, så en snabb redigeringsföljd inte skriver en fil per steg.
    - **Bara när något faktiskt ändrats.** Ett fingeravtryck (FNV-1a över den serialiserade projektfilen) jämförs mot förra autosaven *och* mot filen på disk. Ett orört projekt skriver inga kopior, och en orörd start skapar ingen falsk varning (utgångsläget seedas vid konstruktion).
    - **Rotation:** de **5 senaste versionerna per projekt** behålls; äldre tas bort per projekt, inte globalt.
    - **Återställningsdialog vid start** (`render_recovery_modal`) som bara listar autosaves som är **nyare än sin manuella projektfil** (eller saknar manuell fil = krasch före första sparningen), med ålder i läsbar form, sökväg i tooltip och **Återställ** per rad. En återställd kopia **pensioneras** till `*.restored` i stället för att raderas, så frågan inte ställs igen men filen finns kvar.
    - **`recent.json` + meny:** "🕘 Senaste projekt" (8 poster, senaste först, dubletter flyttas upp, poster vars fil raderats utanför Sonix filtreras bort) och "📂 Visa projektmappen i filhanteraren" (`xdg-open` mot `paths().projects_dir()`).
    - **Dokumenterat:** filkartan, `SONIX_*`-variablerna och autosave-beteendet i `README.md`, `README_SV.md` och `MANUAL.md` (kap. 13) — vilket stänger 6.0:s sista dellinje.
  - **Bevis:** 12 nya tester — 10 i `autosave.rs` (slug är filsystemsäker, filnamn tur och retur, fingeravtryck är innehållskänsligt, atomisk skrivning ersätter hela filen utan temp-rester, samma sekund skriver inte över förra versionen, sortering nyaste först, rotation behåller rätt 5 per projekt, "erbjud bara om nyare", pensionering döljer men behåller, ålder i rätt enhet) och 2 i `app.rs` (`recovery_offers_newer_autosaves_and_skips_already_saved_work`, `recent_list_keeps_the_newest_projects_and_drops_missing_files`) — mot isolerad `HOME`, så testerna rör aldrig riktiga projekt. **174 tester default, 220 med `plugin-host`, 0 varningar.**
  - **Bevisat i GUI (2026-09-11, BenQ/DP-3, ws 2, sandlådad `XDG_MUSIC_DIR` + `SONIX_*`):** appen startades, `Ctrl+D` duplicerade ett spår (statusraden: *"Duplicated track 'Drums & Beat' below the original track!"*), sedan **`kill -9`** mitt i sessionen. Vid omstart visade appen modalen **"Unsaved work found"** — *"Sonix closed before the project was saved. These automatic copies are newer than the file on disk:"* — med raden `Untitled Project — 5 min ago` och knapparna **Restore** / **Continue without restoring**. Autosavens innehåll innehöll den duplicerade kanalen, alltså arbetet och inte bara ett tomt projekt. Kraschåterställningen fungerar därmed från krasch till dialog, inte bara i enhetstester.
  - **Kvar (ärligt):**
    - **Restore-knappen och den omedelbara skrivningen är inte klickade i GUI än** — fönstret tog fokus vid start och du arbetade i ett annat fönster, så jag skickade inga tangenter dit (bara när appen hade fokus). Fixen är ordningsoberoende till sin konstruktion och enhetstesterna är gröna, men just de två stegen bör ses med egna ögon: klicka **Restore** → projektet ska komma tillbaka och en ny autosave skrivas inom ~1 s; gör sedan en ändring (t.ex. `Ctrl+D`) och kontrollera att en ny fil med ny tidsstämpel dyker upp direkt i `~/.local/state/sonix/autosave/`.
    - Autosaven täcker det som ligger i projektfilen. Inspelade tagningar ligger redan som filer i projektmappen och överlever därför en krasch, men **plugin-databasen persisteras fortfarande inte** (oförändrat från 6.0).
    - Upp till 60 sekunders arbete kan tappa om appen dödas *utan* en strukturell ändring i mellanrummet (en fader- eller rattändring är ingen undo-punkt). Ska det bli tightare är nästa steg fler krokpunkter, inte kortare intervall — annars roterar historiken bort sig själv under mixning.
  - **Filer:** `src/autosave.rs` (ny), `src/main.rs`, `src/ui/app.rs`, `src/i18n.rs`, `README.md`, `README_SV.md`, `MANUAL.md`
  - **Beroende:** **6.0** ✅

- [x] **6.2 Undo/redo för mixer, FX och automation** — *M* ✅
  - **Löst:** Ångringshistoriken omfattar nu mixerns ljudbild, inte bara tidslinjen:
    - **Snapshoten bär hela ljudbilden.** `TimelineUndoSnapshot` har fått `bus_volume`/`bus_muted`/`bus_solo` och `vca_faders`/`vca_muted`/`vca_solos` — de låg utanför `playlist_tracks`, så en bussändring gick inte ens att fånga. Spårfälten (volym, pan, mute/solo, EQ, kompressor, sends, automation, routing) fanns redan i snapshoten men saknade anrop.
    - **Ångringen hörs, inte bara syns.** `undo()`/`redo()` kör nu `restore_snapshot()`, som anropar `sync_track_audio_state()` per spår och `sync_group_state()` — förut kördes bara `sync_track_regions()`, så en ångrad volym visades i UI:t men låg kvar i motorn.
    - **Mixerändringar skapar ångringspunkter.** Ett reglage ändras varje frame under ett drag, så en punkt per frame skulle fylla historiken på ett enda drag. I stället hålls mixerns *viloläge* — `mixer_digest()`, en hash av varje värde motorn tar emot — och läggs som ångringspunkt första gången digesten ändras under en interaktion (pekare nere i denna eller förra frameen, eller en MCU/OSC-kontroll). Nytt viloläge tas först när pekaren släppts: **ett drag ger en ångring, inte sextio.** Programmatiska ändringar (projektladdning, preset, själva ångringen) flyttar bara viloläget.
  - **Klart när:** En felaktig mixerändring kan ångras med Ctrl+Z ✅ (kod + enhetstest) · undo följt av redo ger samma ljudande state ✅ (`mixer_digest_covers_every_mixed_field` räknar upp varje fält motorn tar emot och failar om ett fält glöms i digesten eller inte bärs av snapshoten).
  - **Bevisat i GUI (2026-09-11, BenQ/DP-3, mixervyn):** Alex drog i en fader → statusraden visade *"Autosaved 'Untitled Project'"* (den raden sätts bara av `push_undo_snapshot`, alltså skapades ångringspunkten) och en autosave skrevs: spår 1:s volym **0.788** kl. 13:38:13 → **1.198** kl. 13:38:29. Sedan **återställde Ctrl+Z inställningarna**, och upprepade tryck gick till slut till *"Nothing to undo"* — exakt en ångringspunkt per drag, och stacken tar slut när den ska. Dessutom: F6 → mixervyn ✅, `Ctrl+Z` utan föregående mixerändring → "Nothing to undo" ✅ (inga falska ångringspunkter från att bara titta på mixern).
  - **Bevis:** 180 tester default, 0 varningar. CI grön (`1adfa25`, `34593203114`).
  - **Kvar (ärligt):**
    - **Plugin-parametrar och master-FX** (delay/reverb/drive) ingår inte — de ligger utanför `playlist_tracks`. Nästa steg: låt snapshoten bära `plugin_slots` och master-FX-kedjan, och skicka dem till motorn vid `restore_snapshot`.
    - Automation ingår via `playlist_tracks[].automation`, men de *enskilda* punkterna har ingen egen etikett i historiken.
    - **Statusraden hade två skribenter** (hittat under GUI-testet): ångringen märker ändringen för autosave, autosaven skriver nästa frame och ersatte *"↶ Ångrade …"* inom millisekunder, så bekräftelsen gick inte att läsa. Åtgärdat i `a98fc7a`: `maybe_autosave()` rör inte statusraden om den redan visar en ångring/omgörning.
  - **Filer:** `src/ui/app.rs`, `src/i18n.rs`
  - **Beroende:** —

- [x] **6.3 MIDI-fil import/export (SMF)** — *M* ✅
  - **Löst:** Egen SMF-kodek i **`src/audio/smf.rs`** (ren data in/ut, inga app- eller ljudberoenden — samma hållning som WAV, loudness och FLAC), plus inkoppling i `app.rs`:
    - **Skrivning (format 1):** ledspår med tempo (µs/kvartsnot) och taktart, därefter ett spår per projektspår med namn. Noter sorteras i tidsordning och **not-off skrivs före not-on vid samma tick**, så en ny ton på samma tangent inte klipps av föregående not-off (testat).
    - **Läsning:** klarar det filer från *andra* DAW:er faktiskt innehåller — running status, främmande meta-events, SysEx, SMPTE-avvisning med begripligt fel, och noter som aldrig får note-off stängs vid spårets slut i stället för att tappas. Allt felaktigt ger ett `Err`, aldrig panik: testat mot varje trunkering av en fil (`for cut in 1..len`).
    - **Mappning (rena funktioner, testbara utan GUI):** `pattern_bar_notes` speglar `trigger_song_step` — trummor → kanal 0–5 → MIDI-kanal 10, synthspåret → piano-roll-rutnätet (kanal 6 som reserv när rutnätet är tomt), basen → kanal 7. Exporten följer **arrangemanget** (`track.clips[bar]` över 32 takter), inte bara det valda patternet; är arrangemanget tomt exporteras det valda patternet som en takt så knappen inte ger en tom fil.
    - **Import** går till det valda patternet och routar dit appen själv spelar: percussion → trumkanalerna, 48–71 → piano-rollen, **under 48 → baskanalen** (appens basspår spelar kanal 7 med råa notnummer). Det sista var en riktig bugg som round-trip-testet avslöjade: först slängdes basstämmor som "utanför rutnätet".
    - **Inget tyst tapp:** noter efter första takten och tangenter utan kanal *räknas* och rapporteras i statusraden ("hoppade över N efter första takten och M utanför rutnätet"). Tempot i filen används bara när projektet står kvar på 120 BPM, annars vore importen en tyst tempoändring.
    - **UI:** "🎼 Exportera sång som MIDI (.mid)" skriver till `paths().exports_dir()` (atomiskt, temp+`rename`), och "🎼 Importera MIDI-fil (.mid)…" öppnar en dialog med sökvägsfält och målpattern. Alla nya strängar har engelska i18n-nycklar.
  - **Klart när:** En MIDI-fil från en annan DAW kan importeras, redigeras och exporteras tillbaka med noter och tempo intakta (round-trip-test) ✅ · den exporterade filen öppnar korrekt externt ✅ (se beviset).
  - **Bevis:** **192 tester** default, 0 varningar. CI grön.
    - `audio::smf`: 7 tester — VLQ mot specifikationens egna exempelvärden (0x0FFFFFFF som fyra byte), round-trip av noter/tempo/spårnamn, överlappande noter på samma tangent, handbyggd *främmande* fil (running status utan statusbyte, okänt meta-event, annan PPQ 96) läst korrekt, oavslutade noter, SMPTE-avvisning och skräpinput utan panik.
    - `ui::app`: 5 tester — mappningen trummor/synth/bas, **export → import tillbaka till samma rutor**, vad importen tvingas hoppa över, att en fil med 96 PPQ hamnar på rätt steg (och inte skalas fel), samt att melodiska spår aldrig får percussionkanalen 9.
    - **Externt:** en `.mid` skriven av modulen (via ett fristående program som inkluderar `smf.rs` med `#[path]`, `/tmp/smf_check.rs`) lästes av **ffprobe/libmodplug**: format korrekt, **duration 00:00:02.00** (8 åttondelar i 120 BPM = 2,0 s), 5 kanaler — och `ffmpeg` renderade den till ljud (peak 17427, rms 6064). En **oberoende avkodare skriven från specifikationen i Python** (`/tmp/smf_verify.py`, ingen delad kod) gav exakt: `format=1 spår=4 ppq=480`, `tempo 120.00`, 14 noter i rätt spårnamn, lead `[60,62,64,65,67,69,71,72]` på ticks `[0,240,…,1680]` med längd 230, trummor på kanal 9 med tangenter 36/38, bas `(36,0,360)` och `(43,480,360)`.
    - Tonhöjdsanalys på det renderade ljudet gick **inte** att göra: maskinen saknar `timidity.cfg`, så libmodplug spelar ett fallback-ljud. Att filen lästes och lät är belagt; att *rätt toner* lät är inte mätt.
  - **Kvar (ärligt):**
    - Importen lägger noter i **första takten** (pattern är 16 steg). Längre filer tappas inte tyst — de räknas — men de kommer inte in. Nästa steg är att lägga noter över flera takter i arrangemanget.
    - **Ingen notlängd eller velocity per not** följer med in: appens rutor är steg/av-tända med en gemensam steg-velocity, så en not blir ett steg. Exporten skriver däremot varje nots längd korrekt ut.
    - Noter utanför 48–71 som *inte* är bastoner (t.ex. ett leadsolo på MIDI 80) hoppas över och räknas.
  - **Filer:** `src/audio/smf.rs` (ny), `src/audio/mod.rs`, `src/ui/app.rs`, `src/i18n.rs`
  - **Beroende:** —

- [ ] **6.4 Kvantisering & humanisering av inspelad MIDI** — *S* (visade sig vara **M**) — **båda stegen klara, kvar: reglage**
  - **Varför den blev större:** Live-inspelningen kvantiserar redan **vid inmatningen** — `record_midi_note_at_step` skriver noten vid steggränsen, så tidpunkten *inom* steget kastades, och något per-not-anslag fanns inte. Det fanns alltså ingen otajt data att kvantisera i efterhand; tagningen måste börja sparas först.
  - **Steg 1 klart (2026-09-11):** ny modul **`src/midi_take.rs`** — tagningen som ren data (position i takt i *steg*, notnummer, anslag):
    - `quantize(strength, swing)` drar varje not mot sin **närmaste** rutnätslinje (både bakåt och framåt, det är därför `quantize_moves_to_the_nearest_line_in_both_directions` finns), och svängen flyttar udda 16-delar framåt (triolkänsla vid 1.0). Använder **projektets** sväng.
    - `humanize(timing, velocity, seed)` lägger på medveten mänsklig variation. Slumptalet är en egen liten xorshift **med frö**, så samma tagning + frö ger samma resultat — annars vore funktionen omöjlig att testa. Fröet räknas upp vid varje tryck, så två tryck inte ger exakt samma tagning.
    - `tightness()` = medelavståndet från noterna till rutnätet i steg. Det är **mätetalet**: statusraden skriver "0.28 → 0.00 steg otajt" i stället för ett omdöme.
    - `playback_slot()` räknar ut vilket steg som ska trigga en not och hur långt in i steget den ska klinga: en not strax före steg 3 (2.7) triggas av steg 2 och klingar 0.7 steg in — alltså *före* slaget, som den spelades.
    - **Inspelningen sparar tagningen:** `handle_midi_note` lägger notens faktiska position (steg + fas ur sekvenserns stegklocka) i det valda patternets `take`, och att arma ⏺ MIDI-REC börjar en **ny** tagning så att flera försök inte växer ihop. Tagningen följer med i projektfilen (Fas 6.7) och äldre filer läses med tom tagning.
    - **UI:** i piano-rollen står "Tagning: N noter · X steg otajt" plus **🎯 Kvantisera** och **🌀 Humanisera**, och statusraden visar före → efter.
  - **Klart när (steg 1):** En slarvigt inspelad tagning kan kvantiseras till takten och humaniseras tillbaka med bevarad karaktär, verifierat av tester på notdata ✅
  - **Bevis:** **215 tester** default, 0 varningar. `midi_take` har **16 tester**: en tight tagning mäter 0.0; otajtheten är medelavståndet (0.25/0.0/0.25 → 0.5/3); full styrka snäpper till rutnätet; halv styrka flyttar halvvägs; kvantisering går till **närmaste** linje i båda riktningarna; noll styrka ändrar inget; svängen flyttar udda linjer men lämnar jämna; humaniseringen är deterministisk per frö (och olika för olika frön); den håller sig inom takten och anslagsgränserna; en måttlig humanisering **behåller noternas steg** (mätbar "bevarad karaktär"); humanisering följt av hård kvantisering är tillbaka på exakt samma steg; notens position räknas rätt ur steg + fas (även vid klockglapp utanför 0–1). I `app`: tagningen överlever projektfilens round-trip, och ett pattern sparat före 6.4 läses med tom tagning.
  - **Steg 2 klart (2026-09-11): mikro-tajmingen hörs.**
    - `AudioCommand::NoteOnDelayed { note, freq, velocity, delay_samples }` — motorns **befintliga** kö `scheduled_notes` (samma som `StrumChord` använder) tar noten samplenoggrant efter en fördröjning, i stället för att tvinga fram den på steggränsen. Fördröjning 0 fire:ar nästa sample, så en not utan mikro-tajming låter exakt som förut.
    - Den **rena** funktionen `plan_for_step(take, step, step_samples, is_on)` avgör vad som ska hända vid ett steg: noter vars `floor(pos)` är detta steg spelas med `Take::playback_slot`s fördröjning och tagningens anslag, och rutor som tagningen sköter (**samma rutnätssteg**) hoppas över i rutnätsloopen så att ingen not triggas två gånger. `is_on(not, rutnätssteg)` gör att en not som spelades in men sedan **klickats bort** inte klingar — rutnätet är fortfarande sanningen om vad som är på.
    - Kopplad i **båda** trigger-vägarna: pattern-läget (`trigger_step`) och sångläget (`trigger_song_step`), som använder `step_samples(step)` räknat ur motorns egen frekvens.
  - **Bevis (steg 2):** **223 tester** default, 0 varningar. `plan_for_step` har 6 tester: tom tagning ger tom plan (uppspelningen sköts då av rutnätet precis som förut, vilket `a_plan_without_a_take_is_empty` slår fast); en sen not (2.25) får 0.25 stegs fördröjning **och tar över sin rutnätsruta**; en tidig not (2.7) spelas från **steget före** sin ruta och hoppas över på rutan; en not vars ruta släckts spelar inte; en tight not får fördröjning 0; och planen frågar om **rätt** ruta (noten 2.7 frågar om ruta (60, 3) medan den spelas från steg 2). Motorn har två nya tester: `a_delayed_note_fires_after_its_delay` (ingen röst innan fördröjningen, en röst efter) och `a_zero_delay_note_fires_immediately`.
  - **Kvar (ärligt):**
    - **Fasta värden i stället för reglage:** kvantiseringen kör styrka 1.0, humaniseringen ±0.08 steg / ±15 % anslag. Styrka, sväng och mängd ska bli reglage.
    - **Fångstens upplösning är en bildruta** (≈16 ms vid 60 Hz), eftersom sekvenserns stegklocka (`last_step_time`) går i UI-tråden. Det räcker för att skilja "på slaget" från "efter slaget", men är inte samplenoggrant.
    - **Ingen ångring av tagningen:** kvantisering/humanisering rör `take`, som `TimelineUndoSnapshot` inte bär.
    - **Offline-exporten renderar på rutnätet**, inte med tagningens mikro-tajming: `exporter.rs` bygger sin egen kommandolista ur kanal 6/7:s steg. Uppmärksammat under arbetet — och den läser den *flattened* spegeln, så ett polyfont pianoroll-steg blir **en** not i exporten medan uppspelningen spelar alla rader (detta är äldre än 6.4).
    - Bara piano-rollens noter (kanal 6) får mikro-tajming i uppspelningen; trummor och bas kan kvantiseras/humaniseras som data men deras tagning spelas ännu inte.
  - **Filer:** `src/midi_take.rs` (ny), `src/main.rs`, `src/ui/app.rs`, `src/i18n.rs`
  - **Beroende:** 6.3 (samma notmodell)

- [x] **6.7 Projektfilen sparar hela arbetet (mönster, kanalrack, stegvolymer)** — *S* ✅ *(hittad under 6.3-arbetet)*
  - **Fyndet:** Projektformatet (`SonixProjectData`) innehöll **bara** `name, bpm, swing, master_volume, master_pan, tracks` (+ bussar/VCA:er och plugin-slots). Mönstren, kanalracket och stegvolymerna fanns inte med — och inläsningen saknade helt kod för att läsa tillbaka dem. En sparad låt behöll alltså *vilket* pattern som spelar i vilken takt (`clips`) men tappade **vad som står i det**: trumkompet och piano-rollen var borta när filen öppnades igen. **Verifierat mot användarens egna projektfiler** (`~/Music/Sonix/Projects/*.sonix`, fem filer i tre formatvarianter): ingen av dem har notdata, och storleken kommer från ljudregionerna. Det var alltså inte ett sent refaktoreringsfel — formatet har aldrig burit musiken. Hittades när 6.3:s export skulle kopplas mot patterns och sparandet granskades.
  - **Löst:** `SavedPattern` och `SavedChannel` + fyra nya fält i projektformatet (`patterns`, `selected_pattern`, `step_velocities`, `channels`) med `#[serde(default)]`, så äldre filer läses som förut och då lämnas appens standardpatterns orörda (`step_velocities` är `Option` just för att en gammal fil inte ska nollställa dem). `pcm_audio` och `waveform_preview` sparas **inte** — ljudet ligger redan på disk och läses tillbaka från `sample_path`, vågformen räknas om ur ljudet; att spara dem hade blåst upp filen med hundratals kilobyte per kanal utan att tillföra något.
  - **Klart när:** Efter en sparning och en ny inläsning står noterna, kanalernas steg/toner/inställningar och stegvolymerna tillbaka, och en fil från före ändringen öppnas fortfarande ✅
  - **Bevis:** **197 tester** default, 0 varningar.
    - `saved_channel_round_trip_keeps_every_field` är en **fullständighetsvakt**: den sätter varje fält i `ChannelStrip` till ett omisskännligt värde och failar om ett fält läggs till utan att följa med i sparandet.
    - `project_file_carries_the_notes_and_the_drum_rack`: hela vägen genom `serde_json` — noter, rutnät, kanalsteg, stegvolymer. Testet kontrollerar också att `piano_roll_grid` **står i filen**, inte bara i minnet.
    - `old_project_files_without_patterns_still_load`: en fil i det gamla formatet (bara `name, bpm, swing, master_volume, master_pan, tracks`) läses, och `step_velocities` är då `None`.
    - Inläsningsvägen är utbruten till den rena funktionen `restore_saved_music` (samma grepp som `collect_recovery_candidates_in` i 6.1): `restoring_a_saved_project_puts_the_notes_back` visar att noterna kommer tillbaka och att `selected_pattern: 99` kläms till ett giltigt index; `restoring_an_old_project_leaves_the_defaults_alone` visar att en gammal fil **inte** rör appens egna pattern och rack.
  - **Kvar (ärligt):** Att spara ett beat, stänga och öppna filen igen är **inte** kört i GUI av mig — klicket uteblev och fönstret tog fokus. Logiken är enhetstestad inklusive inläsningsvägen; en GUI-kvittens (spara → öppna) återstår, precis som `Restore`-knappen i 6.1.
  - **Filer:** `src/ui/app.rs` (format + mappning + `restore_saved_music` + tester)
  - **Beroende:** —

- [ ] **6.5 Dither vid export** — *S*
  - **Gör:** TPDF-dither (och valfritt noise shaping) när mastern kvantiseras till 16-bitars WAV; av för 24/32-bitars och FLAC.
  - **Klart när:** 16-bitars export dithras (test: dekorrelerat brusgolv, inte korrelerat kvantiseringsbrus) och 24-bitars export är bitidentisk med dagens.
  - **Filer:** `src/audio/wav_writer.rs`, `src/audio/exporter.rs`, `src/ui/app.rs`
  - **Beroende:** —

- [x] **6.6 Återkopplingssäkring för direktlyssning (rundgång)** — *S* ✅
  - **Bakgrund:** Direktlyssningen adderade mikrofonen **1:1** in i masterbussen (`synth.rs` 8c), så **mastervolymen var det enda reglaget** som bröt en rundgång — och den var **på** som standard (`recorder.rs`, `monitoring_on: true`). Med högtalare i stället för hörlurar blir det rundgång, precis som rapporterat ("försvinner när jag sänker main vol").
  - **Rapport 2 ("baston som får hela rummet att vibrera"):** samma väg, men lågfrekvent. Mätt på maskinen: micken (Samson Q2U) har **78 Hz vid −45,7 dBFS** i rummet, och de två kryssrutorna som skulle skydda mot det — `low_cut_80hz` och `feedback_reduction` — **läste ingen kod**: de var bara fält med default `true` respektive dekorativ etikett. Utan högpass gick rummets lågfrekvens rakt in i mastern; vid LF har rummet mest akustisk förstärkning, så slingan låser sig på en rumsmod och blir en baston.
  - **Löst:** `monitoring_on: false` som standard, **MONITOR-ratt** (`AudioCommand::SetMonitorLevel`) med egen nivå, hover-varning för högtalare, **riktigt 80 Hz-högpass** (RBJ-biquad, Butterworth-Q) i mikrofonvägen **före** gaten och monitor-ringen — med 5 Hz DC-spärr kvar även när kryssrutan är av — och **riktig anti-rundgång**: `HowlDetector` klassa en stabil, stark ton över tid (autokorrelationen som tunern redan kör) och en smal bandspärr (Q = 20) läggs på frekvensen i ljudtråden, med ~1 s hålltid efter att tonen tystnat.
  - **Bevis:** 9 nya tester (`low_cut_removes_dc_offset`, `low_cut_removes_sub_and_keeps_voice` — −3 dB i hörnet och ≥ 12 dB vid 40 Hz, `low_cut_passes_voice_band` < 1 dB vid 1 kHz, `notch_kills_howl_frequency` ≥ 12 dB, `notch_leaves_neighbour_frequency_untouched`, samt fyra för detektorn: kräver stabilitet, ignorerar glissando, kräver nivå över gaten, håller spärren efter tonen). 162 tester default, 208 med `plugin-host`, 0 varningar.
  - **Kvar:** (a) detektorn kan inte skilja en ihållande *rundgång* från en ihållande *ton utifrån* (t.ex. en synthpad högtalarna spelar och micken hör) — hårdvaru-suppressorer använder tillväxt över tid som tredje kriterium, vilket är nästa steg; (b) bara **en** spärr, medan en rundgång kan migrera mellan två frekvenser (2–3 spärrar är standard hos hårdvaruburkar); (c) slå ihop de två kryssrutorna — `mic_settings.direct_monitoring` styr fortfarande ingenting (dubblett av `vocal_track.monitoring_on`).
  - **Filer:** `src/audio/recorder.rs`, `src/ui/app.rs`, `src/ui/vocal_studio_view.rs`, `src/audio/synth.rs`, `src/audio/command.rs`
  - **Beroende:** —
---

## ⬜ Fas 7 — Plattform & prestanda (Tier 1)

> **Varför:** FL Studio finns på Windows och macOS. Så länge Sonix är Linux-only kan den inte tävla som produkt — bara vara bäst i en nisch. `cpal` och `egui` är redan plattformsoberoende; det som låser är ALSA-MIDI (`midi_input.rs`), X11-pluginfönstret (`plugin_gui.rs`) och paketeringen (`install.sh`).

- [ ] **7.1 Bryt ut ALSA/X11 till backend-gränssnitt och porta mot Windows** — *XL*
  - **Gör:** Inför traits för ljud-, MIDI- och plugin-fönsterbackend (ALSA → cpal/WASAPI/CoreAudio, ALSA-seq → `midir`, X11 → HWND/NSView) med Linux-vägen som första implementation.
  - **Klart när:** `cargo check --target x86_64-pc-windows-msvc` passerar för allt utom plugin-GUI:t, och en Windows-build startar, spelar upp ljud och tar emot MIDI.
  - **Filer:** `Cargo.toml`, `src/audio/engine.rs`, `src/audio/midi_input.rs`, `src/audio/plugin_gui.rs`, `src/main.rs`, `install.sh`
  - **Beroende:** 6.1–6.3 (data-säkerhet och projekt-I/O ska vara stabilt innan portering)

- [ ] **7.2 Realtidsmätning i CI (xruns, latens, CPU-skalning)** — *M*
  - **Gör:** Mät och logga underruns, callback-tid och CPU-belastning vid 64/128/256/512 frames på ett referensprojekt med N spår, och lägg trösklar i CI så att regresser failar. Mixningen sker i dag i cpal-callbacken utan parallell spårrendering (inga `thread::spawn` i render-vägen) — **mät först, optimera sedan**.
  - **Klart när:** CI rapporterar latens/CPU/xrun per buffertstorlek och failar vid regress över satt tröskel.
  - **Filer:** `tests/realtime_bench.rs` (ny), `.github/workflows/ci.yml`, `src/audio/engine.rs`
  - **Beroende:** —

- [ ] **7.3 Verifiera en riktig yabridge-brygga (Wine + display)** — *M*
  - **Gör:** Kör en faktisk yabridge-producerad brygga (Sytrus/Harmor/Gross Beat/FL Studio VSTi) genom Sonix — laddning, inspektion, ljud med PDC, state och X11-fönstret. Punkten ligger kvar som **4.6** i Fas 4; den flyttas hit när Tier 0 är klar, eftersom den är Tier 1.
  - **Beroende:** 4.1–4.2

- [ ] **7.4 Skanningen av ljudbiblioteket får inte blockera starten** — *M* (fynd från GUI-testet 2026-09-11)
  - **Mätt:** Med det riktiga biblioteket (`~/Music/Sonix/Sample_Packs`, 14 GB / 30 235 filer) skannade appen **10 630 samplar på 125,2 s** innan fönstret kom upp — ~10 GB lästes från disk (8 MB/s mitt i), och under hela tiden syntes ingenting: ingen ruta, ingen förloppsindikator. Samma start med tomt bibliotek: **0,4 s**. Cachen (`~/.cache/sonix/library_cache.tsv`, 116 932 rader) skrivs först *efter* en full skanning, så en avbruten start lämnar en ofullständig cache.
  - **Redan åtgärdat (commit `docs`/`fix` samma dag):** de tre orsakerna till att cachen kastades i onödan är borta — `cache_fingerprint()` beskriver nu innehåll (`mtime:storlek` per rot) i stället för absoluta sökvägar, ett äldre fingeravtryck accepteras som engångsbrygga och skrivs om på plats, och cachen skrivs atomiskt (temp + `rename`) precis som autosaven. Verifierat i GUI: nya binären läste **10 630 samplar ur cachen** och uppgraderade huvudet, i stället för att skanna om.
  - **Kvar (detta är kvarvarande arbete):** när cachen *verkligen* är ogiltig (nya samples, första starten) blockerar skanningen fortfarande fönstret i upp till två minuter utan återkoppling. Skanningen behöver flyttas bort från startvägen: visa fönstret direkt, skanna i en tråd och fyll biblioteksvyn när den är klar (samma mönster som `poll_stem_separation`/`poll_remote_audio_generation` redan använder), plus en synlig rad om att biblioteket indexeras.
  - **Klart när:** Fönstret är interaktivt inom ~1 s även med en ogiltig cache, skanningen rapporterar förlopp i statusraden, och en enhetstestbar brytning finns mellan "bibliotek klart" och "GUI redo".
  - **Filer:** `src/audio/factory_samples.rs`, `src/ui/app.rs`, `src/main.rs`
  - **Beroende:** 6.0 ✅

---

## ⬜ Fas 8 — Arbetsflödesdjup (Tier 2)

> **Varför:** Detta är där en FL-användare faktiskt byter DAW eller inte. Kom efter Tier 0/1 — annars bygger vi bredd på en grund som tappar arbete.

- [ ] **8.1 Freeze / bounce-in-place** — *M* (0 träffar i dag; `freeze` finns bara som text om att UI:t inte fryser)
  - **Klart när:** Ett spår med plugin kan frysas till ljud, spelas identiskt och tinas upp igen utan att inställningar tappas (test: renderad längd/latens).
  - **Filer:** `src/audio/exporter.rs`, `src/audio/synth.rs`, `src/ui/app.rs`

- [ ] **8.2 Tempo map (variabelt tempo)** — *L* (`tempo_map`/`song_tempo` = 0 träffar; i dag ett globalt `bpm`)
  - **Klart när:** Tempobyten på tidslinjen styr uppspelning, automation och export korrekt.
  - **Filer:** `src/ui/app.rs`, `src/audio/synth.rs`, `src/audio/exporter.rs`

- [ ] **8.3 Flexibel routing (sends/returns utöver de fyra fasta bussarna)** — *L*
  - **Klart när:** Ett valfritt antal bussar/returns kan skapas, routas och sparas — inte bara Vocal/Trummor/Synth/FX.
  - **Filer:** `src/audio/synth.rs`, `src/audio/command.rs`, `src/ui/app.rs`

- [ ] **8.4 Multisample-sampler / slicer** — *L* (`multisample` = 0 träffar)
  - **Klart när:** En multisamplad patch över flera oktaver och en slice-uppdelning av en loop kan spelas från klaviaturen och sparas i projektet.
  - **Filer:** `src/audio/factory_samples.rs`, `src/ui/app.rs`, nytt `src/audio/sampler.rs`

---

## ⬜ Fas 9 — AI-kilen (Tier 3)

> **Varför sist och varför alls:** AI är inte det som gör Sonix professionellt — men det är det enda området där Sonix kan bli **bäst i världen**, eftersom ingen annan DAW erbjuder en offline co-producer vars resultat går att **mäta**. Bygg det på Tier 0/1, inte i stället för dem.

- [ ] **9.1 LLM → Command-agent med mät-loop** — *L*
  - **Gör:** Låt LLM:en (Ollama lokalt eller OpenAI/Anthropic/OpenRouter) svara med en **kommandolista** i stället för enbart 16-stegs-clips, och applicera den via den befintliga `command.rs`-ytan (~40 kommandon: mix, EQ, delay/reverb/drive, master-FX, buss/VCA, stems, presets). Verifiera med den mätning som redan finns i `loudness.rs` (LUFS, true peak): rendera → mät → justera tills målet nås, och visa målet i statusraden.
  - **Klart när:** "Sänk sången 2 dB", "halvtidsdelay på leaden" och "mastra till −9 LUFS" utförs som riktiga kommandon, och LUFS-målet verifieras med mätning (test på intent → kommandon + mät-loop).
  - **Filer:** `src/audio/ai_client.rs`, `src/audio/ai_generator.rs`, `src/ui/ai_assistant_view.rs`, `src/audio/loudness.rs`, `src/ui/app.rs`
  - **Beroende:** 6.2 (allt agenten gör måste gå att ångra)

- [ ] **9.2 ACE-Step 1.5 lokalt som `AudioProvider`** — *L*
  - **Gör:** Ny provider mot en lokal ACE-Step-server (`acestep-api`, egen port) med `base_url` enligt samma mönster som Ollama. Ger text→låt, **repaint** (regenerera en vald takt — mer användbart i en DAW än att rulla en hel låt), cover, multi-track-lager och stems. Licens: MIT enligt repots licenssida. 2B-turbo kräver <4 GB VRAM (RTX 3060 Ti 8 GB räcker); XL (4B) kräver ≥12 GB och är därför inte aktuell.
  - **Klart när:** En prompt genererar ett riktigt spår i tidslinjen och repaint ersätter ett valt tidsintervall utan att röra resten (test med mockad server).
  - **Filer:** `src/audio/ai_client.rs`, `src/ui/ai_assistant_view.rs`, `src/ui/app.rs`, `src/i18n.rs`
  - **Beroende:** 6.3 (för att notmaterial ska kunna flyttas ut/in)

- [ ] **9.3 Valfri molnprovider: ElevenLabs Music API** — *M*
  - **Gör:** Officiell musik-API (upp till 5 min, exakthet i ms för längd, upp till sex separata stems, inpainting av avsnitt, kommersiell licens — annons/film/TV/spel kräver utökad licens). Läggs **vid sidan av** de befintliga providerna och märks tydligt som molntjänst med kostnad per generering.
  - **Klart när:** En generering hämtas, dekodas och importeras som spår med samma väg som Etapp C (`trigger_remote_audio_generation`/`poll_remote_audio_generation`), och fel/kvot felrapporteras ärligt.
  - **Filer:** `src/audio/ai_client.rs`, `src/ui/ai_assistant_view.rs`, `src/ui/app.rs`
  - **Beroende:** 9.2 (samma provider-mönster)

> **Medvetet inte aktuellt:** *Suno* har ingen officiell publik API — tredjeparts-wrappers bryter mot deras villkor och lägger användarens prompts hos en mellanhand. *Mozart AI* har ingen publik utvecklar-API alls (deras tjänst är byggd på ElevenLabs Music API, dvs. 9.3). Suno förblir **import**, exakt som README beskriver.

---

## 🎯 Nästa uppgift

**6.4 Kvantisering & humanisering av inspelad MIDI** (*S*, visade sig vara *M*) — **steg 1 och 2 är klara:** tagningen sparas med sin tajming, kvantisering/humanisering har mätetal i statusraden, och uppspelningen följer nu mikro-tajmingen och anslaget via `AudioCommand::NoteOnDelayed` och den befintliga kön i motorn. Humaniseringen är alltså hörbar, inte bara mätbar. **Nästa konkreta uppgift är reglage** (styrka, sväng, humaniseringsmängd i stället för fasta värden) och att låta offline-exporten rendera tagningen i stället för bara rutnätet.

Därefter i Tier 0 (Fas 6): 6.5 dither.

Nyss klart: **6.4 steg 1** (tagningen sparas med sin tajming, kvantisering/humanisering med mätetal), **6.7** projektfilen sparar hela arbetet (mönster, kanalrack, stegvolymer — hittad och stängd under 6.3), **6.3** MIDI-fil import/export (egen SMF-kodek, arrangemanget exporteras, import routar till rätt kanaler, externt validerad) samt **6.2** mixer/FX/automation i undo-historiken.

Tidigare klart: Fas 2 (formant-bevarande pitch, WSOLA-time-stretch, per-voice filter/ADSR), neural stem-separation (3.1) samt plugin-hostens laddning (4.1), instansiering + audio/PDC (4.2), state/preset save-load (4.3), GUI-ABI/livscykel (4.4a), GUI-fönster (4.4b), sandbox-processgräns (4.5a), sandbox-ljudtransport (4.5b), VST3-modul/ABI/inspektion (4.6a), VST3-ljud/state (4.6b), VST2-ABI/ljud/state (4.6c), MIDI-inspelning (5.3), automation (5.4), loudness-normalisering (5.5), VCA-grupper/sub-mix-bussar (5.2) och realtids-/plugin-tester (5.1). Kvar i Fas 4: 4.6 (riktig yabridge-brygga — flyttad till Tier 1, se **7.3**).

## 🛠️ Så här håller vi roadmapen levande

1. Bocka av `[ ]` → `[x]` när uppgiften är **byggd och testad** (`cargo test --release`).
2. Flytta avklarade punkter till **Fas 0** eller lämna kvar med ✅.
3. Uppdatera siffrorna i **[Framsteg](#-framsteg-i-siffror)** (klart/kvar/procent).
4. Lägg till en kort rad i **tools.md** (nästa P-nummer) med vad som gjordes.
