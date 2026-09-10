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
| Ljudmotor (synth, trummor, FX, patcher, per-spår) | 11 | 1 | **92 %** |
| Sequencer & arranger (timeline, rack, piano roll, sektioner, automation) | 6 | 0 | **100 %** |
| Inspelning & sång (mic, takes, comping, pitch, harmonier) | 7 | 0 | **100 %** |
| Generatorer (ackord, tärning, drummer, tuner, add track) | 5 | 0 | **100 %** |
| AI (lokal + LLM + ljud + kontext) | 4 | 1 | **80 %** |
| Stem-separation (DSP + neural ONNX) | 1 | 0 | **100 %** |
| Plugin-hantering | 3 | 5 | **38 %** |
| Export & projekt-I/O (presets, loudness-normalisering) | 5 | 0 | **100 %** |
| Hårdvara (MCU/OSC/MIDI) | 3 | 0 | **100 %** |
| Lokalisering & system (7 språk, motor) | 3 | 0 | **100 %** |
| Dokumentation (README, ROADMAP, tools) | 3 | 0 | **100 %** |

> **Största kvarvarande biten:** **Plugin-hosting** (38 % — en CLAP-värd kan nu ladda plugins och läsa parametrar; ljud/MIDI-routing, state, GUI och sandbox återstår). Neural stem-separation är byggd (opt-in via `--features neural` + en HTDemucs-ONNX).

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

- [ ] **4.2 Instansiering + audio/MIDI-routing + PDC** — *XL*
  - **Gör:** Koppla plugin-instans in/ut i realtidsgrafen, buffert-hantering, delay-kompensation.
  - **Klart när:** En plugin kan spela upp/processa ljud i ett spår utan klick.
  - **Beroende:** 4.1

- [ ] **4.3 State/preset save-load** — *M*
  - **Gör:** Spara/ladda plugin-state i projektet; stöd pluginens egna presets.
  - **Klart när:** Projekt återställer plugin-state korrekt.
  - **Beroende:** 4.2

- [ ] **4.4 Plugin-GUI** — *L*
  - **Gör:** CLAP GUI-extension / X11-embedding; visa "öppna GUI" i spåret.
  - **Klart när:** Pluginens egna fönster kan öppnas och styra ljudet.
  - **Beroende:** 4.2

- [ ] **4.5 Out-of-process sandbox** — *L*
  - **Gör:** Kör plugins i separat process med delat minne/IPC så att en krasch inte tar ner Sonix.
  - **Klart när:** En kraschande plugin kan återstartas utan att Sonix stänger.
  - **Beroende:** 4.2

- [ ] **4.6 Wine/yabridge-väg för FL Studio & Windows-VST** — *L*
  - **Gör:** Ladda `.so`-bryggor från yabridge som vanliga plugins; verifiera Sytrus/Harmor/Gross Beat/FL Studio VSTi.
  - **Klart när:** En yabridge-brygga kan spelas genom Sonix.
  - **Beroende:** 4.1–4.2

---

## ⬜ Fas 5 — Polish & utbyggnad (efter behov)

- [x] **5.1 Fler tester** för realtids- och plugin-vägar (integrationstester). ✅
  - **Löst:** 21 nya tester som täcker de tidigare otestade realtids-DSP-byggstenarna och plugin-vägen. Realtid: `envelope.rs` (ADSR attack→decay→sustain→release, idle tyst, reset), `filter.rs` (lågpass dämpar höga frekvenser, reset nollställer state), `drum.rs` (trigger låter och går till idle, reset tystar, **alla 10 rösttyper** ger ändligt/begränsat och hörbart ljud), `effects.rs` (delay-eko exakt efter delaytiden, bypass vid mix 0, reverb-svans med energi, bypass), `master_fx.rs` (kompressor dämpar över tröskel men släpper igenom under, limiter håller taket, gate stänger/öppnar, hela master-kedjan passerar ljud och rapporterar gain reduction), `engine.rs` (AudioSettings serde-roundtrip + default för partiella configfiler). Plugin: `plugin_host.rs` (skanning klassificerar `.clap`/`.vst3` med rätt format/kategori och verifierar ELF-magi; `.fst` hamnar som preset, aldrig som plugin).
  - **Klart när:** Realtids- och plugin-vägarna har tester som fångar regressioner. ✅ (101 tester, 0 varningar)
  - **Filer:** `src/audio/envelope.rs`, `filter.rs`, `drum.rs`, `effects.rs`, `master_fx.rs`, `engine.rs`, `plugin_host.rs`
- [ ] **5.2 VCA-grupper & sub-mix-bussar** — *M* (om efterfrågat; medvetet borttagna).
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

## 🎯 Nästa uppgift

**Fas 4.2 — Instansiering + audio/MIDI-routing + PDC** (*XL*): koppla CLAP-instansen in i realtidsgrafen (buffert-hantering, `clap.audio-ports`/`clap.note-ports`, delay-kompensation). Därefter 4.3–4.6. Alternativt **Fas 5.2** (VCA-grupper, *M*, om du vill ha tillbaka dem). Fas 2, neural stem-separation (3.1) samt plugin-hostens laddning (4.1), MIDI, automation, loudness och realtids-/plugin-tester i Fas 5 är nu klara.

## 🛠️ Så här håller vi roadmapen levande

1. Bocka av `[ ]` → `[x]` när uppgiften är **byggd och testad** (`cargo test --release`).
2. Flytta avklarade punkter till **Fas 0** eller lämna kvar med ✅.
3. Uppdatera siffrorna i **[Framsteg](#-framsteg-i-siffror)** (klart/kvar/procent).
4. Lägg till en kort rad i **tools.md** (nästa P-nummer) med vad som gjordes.
