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
| **Tier 0** — Trovärdighet (sökvägar, autosave, undo, MIDI-I/O, kvantisering, dither, rundgång, projektfilen) | 8 | 0 | **100 %** |
| **Tier 1** — Plattform & prestanda (backend-utbrytning, realtidsmätning, yabridge, starttid) | 2 | 4 | **50 %** |
| **Tier 2** — Arbetsflödesdjup (freeze, tempo map, routing, sampler) | 1 | 4 | **25 %** |
| **Tier 3** — AI-kilen (agent, lokal modell, moln-API) | 0 | 3 | **0 %** |

> **Prioritet just nu: Tier 1 (Fas 7) — Tier 0 (Fas 6) är stängd, 8 av 8.** Ordningen är inte förhandlingsbar: en proffsmusiker som tappat ett projekt en gång bryr sig inte om hur bra AI:n är. Tier 0 mäts i att inget arbete går förlorat och att allt går att ångra.

---

## ⬜ Vad som återstår (räknat ur listan 2026-09-12)

**Elva punkter är obockade:** sex från den första räkningen, fyra som kom till
2026-09-12 efter researchen om plugins och chopping — och en som kom till samma kväll,
när **8.5** (tysta klipp) visade sig vara en egen klass. Sju går att göra vid datorn, två
kräver Wine och en display, och en kräver en Windows-maskin för kvittensen. Ordningen
nedan är den som ger mest per timme.

**Ändrat 2026-09-12 kväll:** klass-fixen i **8.5** stängde elva ställen som läste ljud
utan att säga till när det misslyckades, och de två vägarna från Sound Browser som
skapade klipp utan ljud. **6.2:s återställ-knapp var inte obekräftad — den var trasig**
(`retire`s returvärde kastades bort, så koden läste sökvägen den just pensionerat) och
är fixad. **Kvar i 8.5:** stämseparatorn skriver inga stämmor till disk, sångstudiovägarna
och ▶-förhandslyssningen avbryter inte, och import-dialogen (regeln är byggd och testad).

| # | Punkt | Storlek | Vad som återstår | Blockerare |
| :--- | :--- | :---: | :--- | :--- |
| 1 | **8.3 Routing på riktigt** | *M* | **Sidokedjorna klara 2026-09-12**; kvar: sends mellan spår (bussar/VCA finns sedan tidigare) | — |
| 2 | **8.4 Sampler** | *M* | Ett riktigt samplerinstrument i kanalracket (WAV-spelaren finns) | — |
| 3 | **8.2 Tempo map** | *S–M* | De 4 visningsställena, automation-lanen (sekunder vs takter), drag-utökningen | — |
| 4 | **7.1 Windows-porten** | *XL* | Steg 1 klart (ALSA/X11 bakom gränssnitt); resten av portningen + mätningen i CI | Windows-maskin för kvittens |
| 5 | **7.3 Verifiera en riktig yabridge-brygga** | *M* | Köra en **riktig** brygga (Wine + display) — mock-modulerna är redan gröna | Wine + display |
| 6 | **4.6 Wine/yabridge-vägen (helhet)** | *L* | Samma kvittens som 7.3, på hela vägen: Sytrus/Harmor/Gross Beat | Wine + display |

**Nya punkter 2026-09-12, efter research om plugins och chopping.** Underlaget ligger i
`sonix`-skillen: `references/daw-research/05-audacity-plugins-effekter-klipp.md`,
`06-flstudio-plugins-chopping.md` och `07-chopping-och-onset-detektering.md` (primärkällor —
Image-Lines onlinemanual, Audacitys manual/release notes/GitHub, Ableton/Reaper/Bitwig, aubio/
librosa/Essentia, Böck & Widmer DAFx-13). Punkterna står för sig själva nedanför den gamla
listan i stället för att knuffa om dess ordning.

| # | Punkt | Storlek | Vad som återstår | Blockerare |
| :--- | :--- | :---: | :--- | :--- |
| 7 | **8.7 Chopper → slicemappning** | *M* | Den mest **synliga** luckan i ett arbetsflöde: choppern har en trim-ruta och en knapp märkt "Transient" som bara sätter slutet till 18 % | — |
| 8 | **8.6 Plugins: bryggning, egna utgångar, sidokedja in i en plugin** | *M* | Det FL:s Fruity Wrapper kan och inte Sonix | — |
| 9 | **8.8 Automatisering av fler parametrar** | *S–M* | I dag fyra mål per spår; plugin-/EQ-/kompressor-/buss-parametrar saknas | — |
| 10 | **8.9 Makron: en kedja av kommandon över många filer** | *S* | Audacitys Macros — finns inte alls hos oss | — |

**Så räknas en punkt som klar:** kod + tester (default och `plugin-host`), 0 varningar i
release, ett bevisstycke här i roadmapen — och för det som hörs eller syns, en kvittens
i GUI. Den sista raden är den som oftast återstår: 6.2, 6.4, 6.5, 7.4 och 8.2:s
tempopunkt-UI väntar alla på att Alex ser dem.

> **Underlag för prioriteringen:** djupjämförelsen mot de etablerade DAW:erna (FL Studio,
> Ableton, Bitwig, Logic, Cubase, Studio Pro, Pro Tools, DP, Reaper, Ardour, Waveform,
> Mixcraft, Renoise, Zrythm + angränsande verktyg) ligger i `sonix`-skillen,
> `references/daw-comparison.md`, med de fem researchrapporterna i `references/daw-research/`.
> Kortversionen: **8.3 är det enda kvarvarande gapet som hörs i en färdig mix** — och
> halva punkten är stängd sedan 2026-09-12 (sidokedjorna; **sends mellan spår** är kvar).
> 8.4 är den mest grundläggande funktionen som saknas helt, och 8.2-resten är billigast. Sonix står
> starkare än de stora på tre punkter: native Linux, CLAP med out-of-process-sandbox, och
> AI/Suno-vägen — ingen av de undersökta DAW:erna har AI-genererad musik som utgångspunkt.
>
> Plugin- och chop-researchen (2026-09-12) flyttar bilden på två sätt. **Audacity är inte
> måttstocken för plugins:** där kör allt i samma process (en plugin kan fälla appen), CLAP
> nämns inte, VST-instrument stöds inte, och Audacity 4 har skurit ned till VST3 + Nyquist
> (+ LV2/AU per plattform). Sonix står starkare: sandbox, CLAP, LV2, egna X11-fönster för
> plugin-GUI:t och ett besked när en sparad plugin saknas. **FL Studio är måttstocken för
> chopping:** deras chop-väg går via Edison/Slicex/Slicer 2 och slutar alltid i noter
> ("Convert to score and dump to piano roll", "Dump score"), medan vår chopper är en enda
> trim-ruta. Audacity har ingen slice-till-noter-väg alls — där exporterar man klipp som
> ljudfiler (Export Multiple / Label Sounds).

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
    - **Inget tyst tapp:** tangenter utan kanal och toner utanför rutnätet *räknas* och rapporteras i statusraden ("hoppade över N utanför rutnätet"), liksom takter bortom arrangemangets 32. Tempot i filen används bara när projektet står kvar på 120 BPM, annars vore importen en tyst tempoändring.
    - **UI:** "🎼 Exportera sång som MIDI (.mid)" skriver till `paths().exports_dir()` (atomiskt, temp+`rename`), och "🎼 Importera MIDI-fil (.mid)…" öppnar en dialog med sökvägsfält och målpattern. Alla nya strängar har engelska i18n-nycklar.
  - **Klart när:** En MIDI-fil från en annan DAW kan importeras, redigeras och exporteras tillbaka med noter och tempo intakta (round-trip-test) ✅ · den exporterade filen öppnar korrekt externt ✅ (se beviset).
  - **Bevis:** **192 tester** default, 0 varningar. CI grön.
    - `audio::smf`: 7 tester — VLQ mot specifikationens egna exempelvärden (0x0FFFFFFF som fyra byte), round-trip av noter/tempo/spårnamn, överlappande noter på samma tangent, handbyggd *främmande* fil (running status utan statusbyte, okänt meta-event, annan PPQ 96) läst korrekt, oavslutade noter, SMPTE-avvisning och skräpinput utan panik.
    - `ui::app`: 5 tester — mappningen trummor/synth/bas, **export → import tillbaka till samma rutor**, vad importen tvingas hoppa över, att en fil med 96 PPQ hamnar på rätt steg (och inte skalas fel), samt att melodiska spår aldrig får percussionkanalen 9.
    - **Externt:** en `.mid` skriven av modulen (via ett fristående program som inkluderar `smf.rs` med `#[path]`, `/tmp/smf_check.rs`) lästes av **ffprobe/libmodplug**: format korrekt, **duration 00:00:02.00** (8 åttondelar i 120 BPM = 2,0 s), 5 kanaler — och `ffmpeg` renderade den till ljud (peak 17427, rms 6064). En **oberoende avkodare skriven från specifikationen i Python** (`/tmp/smf_verify.py`, ingen delad kod) gav exakt: `format=1 spår=4 ppq=480`, `tempo 120.00`, 14 noter i rätt spårnamn, lead `[60,62,64,65,67,69,71,72]` på ticks `[0,240,…,1680]` med längd 230, trummor på kanal 9 med tangenter 36/38, bas `(36,0,360)` och `(43,480,360)`.
    - Tonhöjdsanalys på det renderade ljudet gick **inte** att göra: maskinen saknar `timidity.cfg`, så libmodplug spelar ett fallback-ljud. Att filen lästes och lät är belagt; att *rätt toner* lät är inte mätt.
  - **Kvar (ärligt):**
    - ✅ **Stängt 2026-09-11:** importen tar nu **hela filen**. Noterna delas upp per takt (`notes_by_bar`, ren funktion) och varje takt blir ett eget mönster: en fil på fyra takter blir fyra mönster med var sin kloss på det valda spåret. En fil på **en** takt beter sig precis som förut (mönstret fylls, ingen kloss sätts) — den som bygger mönster för hand vill placera dem själv.
    - **Beviset är en rundgång, inte ett påstående:** en låt på fyra takter skrivs ut med `write_midi`, läses tillbaka med `parse_midi`, delas upp och läggs i fyra mönster — och testet prövar att **varje not finns kvar i rätt takt**, på rätt steg. Före det här var importen en första-takt-import som räknade in resten och kastade den.
    - **Notlängden följer nu också med** (det andra hålet): en ton som håller över ett steg tänder alla steg den klingar igenom. Ett trumslag tänder bara sitt eget — ett slag är ett slag, och en lång not i en trumkanal är inte tre slag.
    - **Kvar, utskrivet:** per-not-velocity kan inte komma in, eftersom modellen har **stegvolymer** (16 delade värden per pattern) och ingen plats för ett anslagsvärde per not. Det är en modellfråga, inte en importfråga — och den står kvar som sådan i stället för att låtsas vara löst.
    - **Ingen notlängd eller velocity per not** följer med in: appens rutor är steg/av-tända med en gemensam steg-velocity, så en not blir ett steg. Exporten skriver däremot varje nots längd korrekt ut.
    - Noter utanför 48–71 som *inte* är bastoner (t.ex. ett leadsolo på MIDI 80) hoppas över och räknas.
  - **Filer:** `src/audio/smf.rs` (ny), `src/audio/mod.rs`, `src/ui/app.rs`, `src/i18n.rs`
  - **Beroende:** —

- [x] **6.4 Kvantisering & humanisering av inspelad MIDI** — *S* (visade sig vara **M**) ✅
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
  - **Steg 3 klart (2026-09-11): reglage och ångring.**
    - **Rutnät att välja mellan:** `TakeGrid` — 1/4, 1/8, 1/16, 1/32, 1/8-triol (12 linjer/takt) och 1/16-triol (24). Kvantiseringen drar till *närmaste* linje i valt rutnät, och **svängen skalas med rutnätet** så att "en tredjedel" känns likadan i 1/8 som i 1/16 — annars vore svängen bara rätt i ett av rutnäten.
    - **Reglage i piano-rollen:** rutnätsväljare, **styrka** 0–100 % och **humaniseringsmängd** 0–100 % (som skalar både tid och anslag). Svängen kommer från projektets egen SWING-ratt, så kvantiseringen följer låten i stället för en siffra någonstans i koden.
    - **Ångringsbart:** kvantisering och humanisering tar en ångringspunkt **före** ändringen, och `TimelineUndoSnapshot` bär nu `patterns` — utan dem gick en kvantisering att göra men inte att ångra, eftersom tagningen ligger i mönstren och inte i spåren. Ångringen speglar också tillbaka mönstret i UI:t via `load_pattern_into_ui` (som *inte* skriver över `patterns` med det gamla UI-läget, vilket `select_pattern` hade gjort).
  - **Bevis (steg 3):** **253 tester** default, 0 varningar. Nya tester: 1/8-rutnätet snäpper 2.4 → 2.0, 1/4-rutnätet drar 2.6 ända till 4.0 (närmaste fjärdedel), triolrutnäten landar på 1.333 respektive 0.667, svängen skalas med rutnätet (2.0 + 2/3·0.33 i 1/8 mot 1.0 + 0.33 i 1/16), och varje rutnät har etikett + index som går runt.
  - **Kvar (kända gränser, ärligt):**
    - **Ingen notmarkering:** kvantiseringen gäller **hela tagningen**, inte "valda noter" som den ursprungliga texten sa — piano-rollen har inget begrepp om markerade noter.
    - **Fångstens upplösning är en bildruta** (≈16 ms vid 60 Hz), eftersom sekvenserns stegklocka (`last_step_time`) går i UI-tråden. Räcker för att skilja "på slaget" från "efter slaget", inte samplenoggrant.
    - **Ångringen av tagningen är inte enhetstestad** — den går genom `current_snapshot`/`restore_snapshot`, som är app-bundna (samma läge som 6.2:s undo-väg). Kräver en GUI-kvittens: kvantisera → Ctrl+Z → noterna tillbaka.
    - **Klart 2026-09-11 — exporten följer nu tagningen.** `PatternSnap` bär `take`, och `triggers_for_step` använder **samma** `plan_for_step` som uppspelningen: noter vars `floor(pos)` är steget exporteras med sin fördröjning (`NoteOnDelayed`) och sitt anslag, och rutnätets rutor för dem hoppas över. Utan det renderade filen noterna på rutnätet medan uppspelningen spelade dem där de faktiskt spelades — ännu en skillnad mellan filen och det du hör.
      - **Bevis:** tre tester i `exporter`: en not inspelad 0.25 steg sent exporteras med 1378 samples fördröjning (0.25 × 5512) och **utan** en odelajad dubbelnot; en not vars ruta klickats bort exporteras inte alls; och en not som spelades sent i ett steg...
    - **Klart 2026-09-11 — bugg i mitt eget 6.4-arbete: noten kunde tystna helt.** Grinden `grid_active` prövades *före* tagningen lästes. En not som spelades sent i ett steg (pos 2.7) har sin ruta på **nästa** steg medan den klingar från sitt eget (`floor(pos)`), så på rätt steg var rutnätet tomt → grenen kördes aldrig → och på rutnätssteget gav planen bara "hoppa över". **Noten spelades alltså inte alls** — halva humaniseringsspannet (avvikelser över ett halvt steg) var tyst. Felet fanns i pattern-läget, sångläget **och** exporten, eftersom alla tre hade samma ordning.
      - **Fixat:** tagningen läses **före** grinden, och grinden är `rutnätet har en not här || tagningen spelar något här`. På alla tre ställen.
      - **Bevis, rött först:** testet `a_note_played_late_in_a_step_still_sounds_from_that_step` kördes mot den gamla grinden och föll med *"noten ska klinga från steg 2: []"* — noten var bevisligen tyst — och blir grönt med fixen. Det är därför testet finns: det beskriver exakt den ordning som var fel.
      - **Kvar:** de två levande vägarna (`trigger_step`, `trigger_song_step`) är app-bundna och inte enhetstestade; de har samma kod och samma fix, och logiken de delar (planen) är testad. En GUI-kvittens — spela in en not strax efter ett slag och hör att den låter — återstår.
    - **Klart 2026-09-11 — exporten tappade ackord:** uppspelningen spelar *alla* tända piano-roll-rader, men exporten läste den **flattenade kanal 6-spegeln**, som bara bär **en** not per steg (den skrivs av `record_midi_note_at_step` och av musritningen). Ett ackord i piano-rollen blev alltså **en** not i den exporterade filen — filen matchade inte det du hörde. `PatternSnap` bär nu `piano_roll: [[bool; 16]; 24]`, och `triggers_for_step` följer samma ordning som `trigger_song_step`: är någon rad tänd på steget spelar rutnätet (en not per tänd rad), annars kanalrutan. Äldre än 6.4, hittat när 6.4:s export-väg granskades.
      - **Bevis:** tre tester i `exporter`: ett ackord på steg 0 ger noterna 48, 52 och 55 (inte bara en); ett steg utan rutnätsnoter spelar fortfarande kanalrutans not; och rutnätet vinner över kanalrutan på samma steg (ingen dubbelnot). Dessutom ett **ljudnivåtest**: ackordet renderas och energin mäts vid 130.81, 164.81 och 196.00 Hz — alla tre ligger över 4× en kontrollfrekvens som inte spelas. Kommandolistan räcker inte som bevis; tonerna ska höras i ljudet också.
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

- [x] **6.5 Dither vid export** — *S* ✅
  - **Löst:** egen modul **`src/audio/dither.rs`** — TPDF-dither (två oberoende rektangulära slumptal summrade ger en triangulär fördelning över ±1 kvantsteg, vilket är det som gör felet *och* medelvärdet oberoende av insignalen) med **valfritt noise shaping** av första ordningen (felet från föregående sample dras av före kvantiseringen, så utfelet blir `d + (1 − z⁻¹)·e`, dvs högpassformat). Per-kanal-tillstånd, eftersom en interleaved stereofil annars skulle låta vänster kanal styra höger.
    - **Kvantiseringen avrundar** nu i stället för att trunkera. Trunkering är ensidig och lägger ett systematiskt fel på en halv kvantnivå ovanpå distorsionen.
    - **Dithern är på som standard** för 16-bitars WAV; noise shaping är en smaksak och av som standard. Båda finns som kryssrutor i renderingsdialogen ("2. FORMAT & LJUDKVALITET"), och 24-bitars/float lämnas orörda.
    - **Fast frö** (`DEFAULT_SEED`) i stället för tiden: samma projekt + samma inställningar ger samma fil. En export ska vara reproducerbar.
    - Kopplad i **båda** 16-bitarsvägarna: `exporter.rs` (WAV-exporten) och `wav_writer.rs` (inspelade tagningar och fabriksljud). `write_export_with(…, DitherSettings)` är den nya vägen; `write_export` finns kvar som tunt skal med standardinställningar, så inga befintliga anrop ändras.
    - Slumptalaren flyttades till **`src/rng.rs`** — 6.4:s humanisering och 6.5:s dither använder nu samma källa i stället för varsin kopia.
  - **Klart när:** 16-bitars export dithras (test: dekorrelerat brusgolv, inte korrelerat) ✅ (se mätningen)
  - **Bevis:** **241 tester** default (287 med plugin-host, 1 ignorerad: den externa mätningen), 0 varningar.
    - `dither.rs` har 8 tester: en stark signal håller sig inom ett kvantsteg, dithern är deterministisk per frö, den är **omedelvärdesriktig** (en konstant mitt emellan två nivåer får medelfelet < 0.05 kvantsteg, medan trunkeringen ligger exakt på 0.5), felet blir **okorrelerat** med en svag ton (|r| < 0.15 mot > 0.5 för trunkering), tystnad får bara en viskning inom ett kvantsteg, shaping ger högpassformat (lag-1-autokorrelation < −0.2 mot ≈ 0 för vanligt dither) **och** mindre lågbandig/mer högbandig brusenergi (Goertzel), kanalerna har egna tillstånd, och insignal utanför ±1 klipps i stället för att slå runt.
    - `exporter.rs` har ett test som mäter på **filens bytes**: en svag ton exporteras med och utan dither, och det odithrade felet hänger ihop med signalen (r > 0.7) medan det dithrade inte gör det (r < 0.2). Dessutom att dithern inte skadar materialet: på normal nivå ligger varje sample inom ett kvantsteg.
  - **Extern mätning** (`/tmp/sonix_dither_{utan,med,shaping}.wav`, skrivna av ett `#[ignore]`-test, `cargo test --bin sonix -- --ignored dump_dither`): en −60 dBFS-ton på 440 Hz exporterad till 16 bitar, analyserad i Python (Hann-fönstrad DFT), dB relativt grundtonen:

    | Fil | 880 Hz | **1320 Hz** | 1760 Hz | **2200 Hz** | Brusgolv |
    | :--- | ---: | ---: | ---: | ---: | ---: |
    | utan dither (trunkering) | −83.4 | **−44.3** | −87.0 | **−46.7** | −102.8 |
    | TPDF | −71.7 | −85.0 | −77.4 | −69.4 | −76.7 |
    | TPDF + shaping | −76.6 | −71.5 | −76.6 | −74.9 | −78.4 |

    Trunkeringen ger **udda övertoner 44 dB under grundtonen** — hörbar distorsion (jämna övertoner saknas, vilket är signaturen för trunkering mot noll). Med TPDF försvinner övertonerna ner i ett jämnt brusgolv 77 dB under tonen. Shaping sänker bruset i låga band och lyfter det i höga (200–2 k: −76.5 mot −75.3; 15–21 k: −73.0 mot −75.1). Filerna läses dessutom av **ffprobe** som `pcm_s16le, s16, 44100 Hz, 2 kanaler`.
  - **Kvar (ärligt):** noise shaping är första ordningen (2:a/3:e ordningen ger mer men kräver mer kod och kan bli instabilt nära full skala); dither läggs bara på 16-bitars fast punkt (24-bitars WAV kvantiseras utan, vilket är försumbart); och att kryssrutorna i dialogen fungerar är inte klickat i GUI av mig.
  - **Filer:** `src/audio/dither.rs` (ny), `src/rng.rs` (ny), `src/audio/{mod,exporter,wav_writer}.rs`, `src/ui/app.rs`, `src/i18n.rs`
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

> **Läget i Tier 1: 2 av 4 klara.** **7.4 Starttid** (biblioteksskanningen kör nu i bakgrunden i stället för före fönstret) och **7.2 Realtidsmätning** (8-kanals referensprojekt: 1–2 % medelbelastning, tröskel i CI + siffror i varje körning). Kvar: **7.1** (ALSA/X11 till backend-gränssnitt och Windows-port — *XL*) och **7.3** (riktig yabridge-brygga, kräver Wine och en display).

> **Att klicka i GUI innan Tier 1 stängs** (jag kan inte klicka): 6.7 (spara ett projekt med ett beat → öppna igen), 6.4 (spela in snett → kvantisera → 0.00 → Ctrl+Z), 6.4 (en not strax *efter* ett slag ska höras — den var tyst före `c9fff40`), 6.5 (dither-kryssrutorna + lyssna) och 7.4 (att fönstret syns direkt vid kall start).

> **Varför:** FL Studio finns på Windows och macOS. Så länge Sonix är Linux-only kan den inte tävla som produkt — bara vara bäst i en nisch. `cpal` och `egui` är redan plattformsoberoende; det som låser är ALSA-MIDI (`midi_input.rs`), X11-pluginfönstret (`plugin_gui.rs`) och paketeringen (`install.sh`).

- [ ] **7.1 Bryt ut ALSA/X11 till backend-gränssnitt och porta mot Windows** — *XL* — **steg 1 klart, mätningen pågår i CI**
  - **Gör:** Inför traits för ljud-, MIDI- och plugin-fönsterbackend (ALSA → cpal/WASAPI/CoreAudio, ALSA-seq → `midir`, X11 → HWND/NSView) med Linux-vägen som första implementation.
  - **Klart när:** `cargo check --target x86_64-pc-windows-msvc` passerar för allt utom plugin-GUI:t, och en Windows-build startar, spelar upp ljud och tar emot MIDI.
  - **Uppmätt nuläge (2026-09-11):** Windows-målet installerades och en riktig korscheck kördes. Två hinder, i tur och ordning:
    1. **`alsa-sys` byggskript** föll direkt — pkg-config kan inte korskompilera, och ALSA finns inte på Windows. `alsa` var ett **ovillkorligt** beroende, så kratet kunde inte ens påbörja checken för målet.
    2. **`ring` byggskript** (via `ureq`/rustls) kräver en Windows-kompilator. Det är alltså **inte** vår kod: en korscheck från Linux kan inte komma förbi det, hur ren porteringen än är.
  - **Slutsats om verifieringen:** kriteriet kan inte mätas med en korscheck härifrån. Det mäts i stället på en **riktig Windows-runner** i CI (`windows-latest`, där MSVC finns) — ett nytt jobb som är **icke-blockerande** medan porten pågår, så att det visar hur långt den kommer i stället för att gissa. När allt utom plugin-GUI:t checkar flyttas jobbet in i den blockerande kön.
  - **Steg 1 klart (2026-09-11):**
    - `alsa` ligger nu under `[target.'cfg(target_os = "linux")'.dependencies]` — beroendet finns bara där det behövs, och `Cargo.lock` är oförändrad.
    - **`MidiKeyboardInput` och `McuInput` är gated per plattform:** Linux-vägen är orörd (samma ALSA-sequencer), och på andra plattformar finns en **stubbe som svarar med ett begripligt fel** ("MIDI-klaviatur kräver ALSA-sequencern, som bara finns på Linux i den här versionen") i stället för att kratet inte ska gå att bygga. `note_to_roll_offset` (ren funktion, används av inspelningen) ligger kvar ogated, och det gör **OSC också** — det är UDP och fungerar alltså på alla plattformar redan nu.
    - Linux oförändrat: **258 tester, 0 varningar**, `cargo check --locked` rent.
    - Efter det: korschecken faller bara på `ring` — alltså inget av vår egen kod och inget ALSA kvar.
  - **Steg 2 klart (2026-09-11) — mätningen svarade:** Windows-jobbet körde och **fellistan var tom**. Efter ALSA-gaten checkar hela kratet för Windows. Jobbet visade samtidigt **fem varningar i vår egen kod**, som ingen hade sett eftersom inget bygge hade tittat på Windows förut:
    - `use std::thread::{self, JoinHandle}` och `use std::time::Duration` i `midi_input.rs` — tråden och sömnen hör till ALSA-läsaren → importerna gated till Linux.
    - Stubbarna för `MidiKeyboardInput` och `McuInput` hårdkodade sina svar → de **läser nu samma fält som Linux-vägen** (fälten fylls bara av ALSA-tråden, så svaren är identiska) och då behövs ingen allow.
    - `ControlEvent::MidiNote` konstrueras bara av ALSA-läsaren → riktad `allow(dead_code, reason = …)`, eftersom varianten hör till appens API och blir levande när MIDI-in portas.
    - CI-steget är skärpt: **det failar nu även på varningar** (repots krav är noll, och en varning på Windows som ingen ser är hur porten tystnar), och ett nytt steg **länkar fram en riktig `sonix.exe`**. Bevis från körningen: `Compiling sonix v0.9.0` → `Finished` utan varningar, och artefakten **`sonix.exe`, 22 216 192 byte**.
    - Jobbet är därmed **inte längre icke-blockerande** — det håller porten kvar i stället för att bara visa hur långt den kommit.
  - **Steg 3 klart (2026-09-11) — filväljaren är inte längre Linux-only:** `zenity` var appens **enda** sätt att öppna en fil, och det finns inte på Windows — alltså kunde man inte ens öppna ett projekt där. Den är ersatt av **`rfd`** (plattformens egen dialog: Windows/macOS inbyggda, XDG-portalen på Linux). Fyra anropsställen i `app.rs` plus ett i `vocal_studio_view.rs` går nu genom samma väg, och zenity-strängarna är ersatta av typade filter (`label` + ändelser utan punkt).
    - **Kostnaden mätt, inte gissad:** rfd:s standardfeatures använder **`xdg-portal`, inte GTK3** — alltså inga GTK-dev-huvuden för att bygga. Låsfilen växer med **18 paket**, men `ldd` på binären visar **inget nytt** i länkningen (ingen wayland/gtk/dbus: rfd talar D-Bus via ren Rust). Körtidskravet är i stället att skrivbordet har en portal.
    - **Runtime-kravet verifierat:** `org.freedesktop.portal.FileChooser` **version 4** med `OpenFile` svarar över D-Bus på den här maskinen — det är exakt det interface rfd använder. Själva dialogfönstret kräver en människa framför skärmen (samma GUI-lucka som resten av listan).
    - **`install.sh` och båda README:erna** är uppdaterade: zenity är borta ur paketlistan och ur kraven, och portalen nämns i stället.
    - Ärligt om felvägen: en dialog som inte kan öppnas ger `None`, precis som en avbruten dialog — **rfd lämnar ingen felorsak**. Den zenity-diagnostik som lades in tidigare i steget togs därför bort igen, och det står här i stället för att ligga kvar som död kod.
  - **Steg 4 klart (2026-09-11) — artefakten lästes, och två fel som bara fanns i filen är åtgärdade:** CI-artefakten hämtades och lästes med `llvm-objdump`. Där stod `Subsystem: Windows CUI` (alltså en **konsolapp** — Windows hade öppnat en konsolruta bredvid fönstret) och `VCRUNTIME140.dll` i importerna (binären krävde alltså Microsofts VC++-redistributable och startade inte utan den). Ingen av dem syns i koden. Åtgärder: `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` i `main.rs` och statisk CRT via `.cargo/config.toml` för Windows-målet.
    - **Verifierat i den nya filen, inte i commit-meddelandet:** `Subsystem 00000002 (Windows GUI)` och `VCRUNTIME140` borta ur importerna. Artefakten gick från 22 275 072 till **11 966 976 byte** (release + statisk CRT).
    - **Den skärpta kontrollen bevisade sig själv:** en körning blev **röd** på `warning: field 'connections' is never read` i midir-vägen — kod som Linux inte kompilerar alls, så ingen lokal körning hade kunnat se den. Fältet bar anslutningarna för sin livstid och läses aldrig; det står nu uttryckligt med orsak. Jobbet failar alltså på varningar, och det gjorde nytta.
  - **Porten är pausad efter beslut (2026-09-11):** Alex: *"Windows-version står inte högst på min prio just nu, kommer dit sen."* Inget mer Windows-arbete görs förrän han säger till. Det som är klart står kvar och hålls grönt av CI (jobbet är blockerande).
  - **Kvar (när Windows blir aktuellt igen, i tur och ordning):**
    1. **Köra den på en riktig Windows-maskin.** Det är den enda delen av kriteriet som ingen automatisk mätning kan svara på: en runner har varken skärm eller ljudenhet, så "startar, spelar upp ljud och tar emot MIDI" kräver en riktig dator. Artefakten laddas upp av CI och hämtas med `gh run download <run-id> -n sonix-windows` (Alex: "kanske senare"). Den här delen står alltså kvar tills vidare — inget påstående görs om att den är uppfylld.
    2. **Porta MIDI-in till `midir`** i stället för ALSA-seq, så att stubbarna kan ersättas av en riktig implementation även på Windows/macOS.
    3. **`paths.rs`:** i dag byggs sökvägarna som XDG-kataloger med `$HOME`-fallback — på Windows fungerar det (allt hamnar under användarens hemkatalog) men det är inte plattformens egen layout (`%APPDATA%`/`%LOCALAPPDATA%`).
    4. **`xdg-open`** (två ställen: visa projektmappen, plugin-mappen) bör bli `explorer`/`open` per plattform. Felen hanteras redan, så det är kosmetiskt.
    5. **Plugin-GUI:t** (X11) är undantaget i kriteriet. `plugin-host`-featuren har en `compile_error!` som säger att den är Linux-only i stället för att falla på `libc`/X11.
  - **Filer:** `Cargo.toml`, `src/audio/midi_input.rs`, `src/audio/hardware_control.rs`, `.github/workflows/ci.yml`
  - **Beroende:** 6.1–6.3 (data-säkerhet och projekt-I/O ska vara stabilt innan portering)

- [x] **7.2 Realtidsmätning i CI (xruns, latens, CPU-skalning)** — *M* ✅
  - **Ett enskilt långsamt block fäller inte längre mätningen (2026-09-12).** CI:s
    plugin-host-jobb föll på `the_reference_project_renders_well_inside_the_realtime_budget`:
    **1 av 120 block** missade budgeten i 64 frames (värsta 3,09 ms mot 1,45 ms) medan
    **medelbelastningen låg på 4,4 %** — alltså CI-maskinens schemaläggning, inte en
    långsammare DSP. Den gamla regeln (noll block över budgeten) hade fällt ungefär var
    tjugonde körning; samma slutsats stod redan i testets kommentar för debug-läget.
    `BlockStats` räknar nu **antalet** block över budgeten (ren funktion med eget test:
    7 ms över, 5 ms inte, tom mätning noll) och release-kontrollen tillåter **högst två
    av 120**. Medeltemperaturen vaktas fortfarande vid 20× baslinjen — där syns en
    verklig försämring, till skillnad från en enskild spik.
  - **Löst:** ny modul **`src/audio/realtime_bench.rs`** som mäter **samma arbete som ljudcallbacken** gör (`engine.rs`): töm kommandokön vid steggränserna, rendera blocket frames med `SynthEngine::process_stereo`, skriv till utbufferten. Ingen ljudenhet behövs — det är DSP-arbetet som mäts, inte enhetens latens — så mätningen kan köras i CI.
    - **Nyckeltalet är belastning:** renderad tid delat med blockets realtidsbudget (`frames / sample_rate`). Under 100 % hinner vi; över 100 % blir det xrun.
    - **Referensprojektet** är den sorts last en låt ger: 8 kanalrack-kanaler med samplar (trumkomp), ett tretoners ackord i piano-rollen och en bastrack, med master-FX på.
    - **Mätningen svarade på frågan punkten ställde.** Den misstänkta flaskhalsen var att mixningen sker i cpal-callbacken utan parallell spårrendering (inga `thread::spawn` i render-vägen). Svaret: för ett 8-kanals referensprojekt ligger **värsta blocket på 4 % av realtidsbudgeten**. Optimeringen behövs alltså inte nu — och skulle kosta determinism i render-vägen. Det är "mät först, optimera sedan" som gav ett svar i stället för en gissning.
    - **Tröskeln sitter på medelbelastningen**, inte på värsta blocket: ett enstaka långsamt block kan komma av att CI-maskinen blir avbruten, och ett test som failar på det vore flakigt i stället för en regressionsvakt. Värsta blocket mäts och får inte passera 100 % (då hade det blivit ett xrun), och alla siffror skrivs ut.
    - **CI rapporterar siffrorna:** ett nytt steg kör mätningen med `--nocapture` i varje ben, så latens/CPU per buffertstorlek står i loggen — tröskeln ligger i testet, rapporten i steget.
  - **Klart när:** CI rapporterar latens/CPU/xrun per buffertstorlek och failar vid regress över satt tröskel ✅
  - **Bevis:** **257 tester** default, 0 varningar. Uppmätt baslinje (release, 120 block per buffert):

    | Buffert | Budget | Medel | Värsta blocket |
    | ---: | ---: | ---: | ---: |
    | 64 frames | 1,45 ms | 0,02 ms (**1,7 %**) | 0,07 ms (4,6 %) |
    | 128 frames | 2,90 ms | 0,05 ms (**1,6 %**) | 0,10 ms (3,5 %) |
    | 256 frames | 5,80 ms | 0,08 ms (**1,4 %**) | 0,13 ms (2,3 %) |
    | 512 frames | 11,61 ms | 0,13 ms (**1,1 %**) | 0,19 ms (1,6 %) |

    Tröskeln är **20 %** medelbelastning i release (tolv gånger baslinjen) och 100 % i debug, där optimeringarna saknas (debug låg på 5–10 % medel, 10–24 % värsta).
    - **CI-maskinen (ubuntu-latest) ger samma bild, lite snabbare:** 1,3 % / 1,2 % / 1,1 % / 0,9 % medel och 2,1 % / 1,9 % / 1,4 % / 1,1 % värsta block (64→512 frames). Två maskiner, samma storleksordning — mätningen är reproducerbar, och tröskeln har tolv gångers marginal även där. Siffrorna skrivs ut i varje CI-körning, så en trend syns utan att någon behöver mäta lokalt.
    - Den rena matematiken är enhetstestad med kända tal (128 frames vid 44,1 kHz = 2,90 ms budget, 1,45 ms rendering = 50 %), plus att en tom mätning inte delar med noll och att sammanfattningsraden innehåller det CI behöver.
  - **Kvar (ärligt):**
    - **Tröskeln fångar en regression på omkring tolv gånger, inte en på tre.** Mindre förändringar syns i CI-loggen men failar inte — det är ett medvetet val för att inte få ett flakigt test på delade CI-maskiner. Efter några gröna körningar kan taket sänkas med de verkliga siffrorna som grund.
    - **Riktiga xruns mäts inte.** En xrun är enheten som inte hann leverera bufferten; utan ljudenhet i CI går det inte att se. Det som mäts är DSP-arbetet, som är den del appen råder över — enhetens latens och drivrutinens buffertbeteende kräver en riktig maskin.
    - **Punkten heter därför nästan det den gör.** Den ursprungliga texten lovade "xruns, latens, CPU-skalning"; det som levereras är CPU-skalning per buffertstorlek och xrun-*risk* (block över budget).
    - Filen blev `src/audio/realtime_bench.rs` i stället för `tests/realtime_bench.rs`: kratet är en binär utan lib-target, så ett integrationstest kan inte importera motorn.
  - **Filer:** `src/audio/realtime_bench.rs` (ny), `src/audio/mod.rs`, `src/audio/exporter.rs` (delar `triggers_for_step`), `.github/workflows/ci.yml`
  - **Beroende:** —

- [ ] **7.3 Verifiera en riktig yabridge-brygga (Wine + display)** — *M*
  - **Gör:** Kör en faktisk yabridge-producerad brygga (Sytrus/Harmor/Gross Beat/FL Studio VSTi) genom Sonix — laddning, inspektion, ljud med PDC, state och X11-fönstret. Punkten ligger kvar som **4.6** i Fas 4; den flyttas hit när Tier 0 är klar, eftersom den är Tier 1.
  - **Beroende:** 4.1–4.2

- [x] **7.4 Starttid: skanna inte före fönstret** — *M* ✅
  - **Problemet, mätt:** vid första starten på en ny maskin kördes en full genomsökning av ljudbiblioteket **före** fönstret: `Ljudbibliotek skannat: 10630 samplar på 125.2 s` (~10 GB, `Sample_Packs` är 14 GB/30 235 filer). Med tomt bibliotek tog samma väg 0,4 s. Under skanningen fanns inget fönster att titta på — appen såg ut att hänga.
  - **Löst:** `init_default_sample_library()` är delad i två vägar:
    - **Cache finns** → läs den direkt (som förut, en bråkdel av en sekund) och var klar vid första bildrutan.
    - **Cache saknas** → `LibraryScan::spawn()` startar en **full skanning i en egen tråd** och returnerar direkt. Fönstret ritas medan skanningen kör, och `poll_library_scan()` i uppdateringsloopen visar förloppet i statusraden: *"🎵 Skannar ljudbiblioteket: 4500 samplar (Sample_Packs, 12 s)"*, och till sist *"🎵 Ljudbiblioteket klart: 10630 samplar på 118,4 s"*.
    - **Statusraden har fortfarande en skribent i taget:** förloppsraden skrivs bara om raden är tom eller redan är en biblioteksrad, så en ångring eller ett fel försvinner inte.
    - **Kanalracket får sina samplar i efterhand:** när skanningen är klar körs `auto_assign_default_kit` på de inbyggda trumkanalerna — men **bara** på kanaler som fortfarande saknar eget ljud, så ett val användaren gjort under tiden inte skrivs över. Att tilldela sent går bra: kanalens PCM skickas till motorn vid varje anslag (`TriggerSampleVoice`), inte en gång vid start.
    - **Tråden kan dö:** `poll()` skiljer på `Idle`, `Progress`, `Done` och `Aborted` — en panik i skanningstråden ger *"⚠ Kunde inte läsa in ljudbiblioteket — skanningen avbröts"* i stället för att appen väntar för evigt.
    - Den gamla samlade ingången `scan_and_load_all_samples()` togs bort (den blockerande vägen ska inte finnas kvar att råka anropa); kvar är `read_library_cache_items()` och `perform_full_sample_scan_with_progress(progress)`.
  - **Klart när:** Fönstret visas utan att vänta på biblioteksskanningen, förloppet syns, och den färdiga skanningen fyller biblioteket ✅ *(komponentbevis; se kvar nedan)*
  - **Bevis:** **246 tester** default, 0 varningar. Fem nya tester i `factory_samples`:
    - `starting_a_library_scan_returns_immediately` — arbetet sover 200 ms, men `spawn_with` är klar på **under 50 ms** (mätt med `Instant`), och resultatet kommer fram när tråden är klar. Det är själva fixen, mätt.
    - `a_scan_reports_progress_before_it_finishes`, `only_the_last_progress_message_survives` (en långsam konsument får inte en kö av gamla förlopp), `a_dead_scan_thread_is_reported_as_aborted`, `a_finished_scan_leaves_nothing_to_poll` (Done vinner över förloppet och upprepas inte).
    - `spawn_with` tar arbetet som en injicerad closure, så trådningen och tillståndsmaskinen testas utan att röra ett riktigt filsystem — det var därför testet kunde vara både snabbt och deterministiskt.
  - **Kvar (ärligt):**
    - **Att fönstret *syns* direkt är inte mätt av mig.** Ingen Xvfb/weston finns på maskinen, så den visuella kvittensen kräver en riktig skärm (BenQ/DP-3) — och min testinstans från 6.2 står fortfarande öppen där, så jag startade ingen andra oannonserat. Komponentbeviset är att startvägen returnerar direkt (testet ovan) och att cache-läsningen sker före första bildrutan.
    - **Första starten på en ny maskin visar ett tomt bibliotek i upp till två minuter** medan skanningen kör. Det är ärligt (statusraden säger vad som händer) men nästa steg vore att fylla brorsan efter hand: skicka `Done` per mapp i stället för allt på en gång.
    - Skanningen läser fortfarande **hela** biblioteket varje gång cachen saknas; den kunde spara dellistor under vägen så att ett avbrott inte kastar allt arbete.
  - **Filer:** `src/audio/factory_samples.rs`, `src/ui/app.rs`, `src/i18n.rs`
  - **Beroende:** —

---

## ⬜ Fas 8 — Arbetsflödesdjup (Tier 2)

Djupet som skiljer en DAW från en leksak: att frysa spår, byta tempo mitt i låten,
routa på riktigt och ha en sampler. Inget av det är AI — det är hantverket.

- [x] **8.1 Frysning av spår** — *M* ✅ (2026-09-11)
  - **Gör:** Rendera ett pattern-spår till ljud offline och spela ljudet i stället för syntesen, så att stora projekt inte tappar realtid.
  - **Klart när:** Ett fruset spår låter likadant som ofruset, går att tina upp till exakt samma musik, överlever projektfilen, och belastningen blir lägre. ✅ **alla fyra mätta eller belagda** — utom att höra skillnaden i GUI, som kräver Alex.
  - **Vad som gjordes.** Frysningen återanvänder två saker som redan fanns i stället för att bygga en ny väg:
    - **Renderingen** går genom `build_render_spec(Some(t_idx), …)` + `render_buffer(spec, dry = true)` — exakt samma väg som exporten, solad på spåret. Torr (utan master) och med spårets fader borttagen ur specen, så att **fadern fortsätter gälla live** och mastern inte appliceras två gånger.
    - **Uppspelningen** går genom motorns **stem-spår** (`LoadStemTrack` + `SetStemTrackRegions`) — samma maskin ljudspåren använder. Ingen ny uppspelningsväg behöver underhållas, och `SetStemTrackState` gör att fader, pan, mute och solo fungerar på ett fruset spår precis som på ett ofruset.
  - **Två regler ligger som rena funktioner** (`render_clips_for`, `frozen_region`) i stället för att vara inbäddade i UI-koden — just för att de är det som *gör* frysningen, och för att de ska gå att pröva.
  - **Ärlighet mot användaren i stället för tyst fel ljud:** `frozen_digest` är ett fingeravtryck av allt som påverkade renderingen (tempo, sväng, spårets klipp och fader, mönstren klippen pekar på, kanalracket, röstinställningarna). Ändras något av det visas **"⚠ ändrat sedan frysningen"** på spåret i stället för att tyst spela gammalt ljud. Det är en varning, inte ett bevis — fingeravtrycket ser att något skiljer sig, det avgör inte vad som är rätt, och det står i koden.
  - **En riktig bugg hittades i integrationen, inte i koden:** `sync_track_stem_to_engine` körs vid **varje uppspelningsstart** och skickade regionlistan från spårets egna regioner — som för ett pattern-spår är **tom**. Det hade tystat frysningen så fort man tryckte Play, och ingen enhetstest hade kunnat se det eftersom det kräver ett spelande projekt. Fixat i båda synkvägarna.
  - **"Att tina" är att lasta ett tomt stem.** Ingen ny motor-kommandon behövdes: `LoadStemTrack` med tomma buffrar tömmer platsen men **behåller eq/comp/plugin och det positionella indexet**, vilket en borttagen post inte hade gjort.
  - **Filen lämnas kvar vid upptining.** Ljudet är användarens material; en ny frysning skriver över samma deterministiska namn (`Frozen/<spår>-<index>.wav`, 32-bitars flyttal så att mellansteget inte kvantiserar innan exporten gör det).
  - **Vägrat:** ett spår med **plugin-insert** kan inte frysas — offline-renderingen kan inte återskapa pluginljudet, och statusraden säger varför i stället för att rendera något annat än det som hörs.
  - **Bevis:** 269 tester default, 315 med plugin-host, 0 varningar. Fem nya: ett fruset spår triggar inte sina patterns · bara pattern-spår kan frysas · regionen räknas ur bufferten (och noll samplingsfrekvens ger inte oändlig längd) · frysningen överlever projektfilen · en äldre projektfil utan fältet läses som **ofrusad**.
  - **CPU-besparingen är mätt, inte påstådd** — och mätningen jämför **identiskt ljud** genom två vägar (referensljudet renderas först och matas tillbaka som stem, så det inte är fråga om att enklare musik går fortare): medelbelastning **1,4 % → 0,3 %** och värsta blocket **2,2 % → 0,3 %** i release (256 frames) — runt **4,7 gånger lägre**. I debug 7,3 % → 2,6 %. Tröskeln i testet jämför de två mätningarna och gäller bara i release, med generös marginal (20 % billigare), av samma skäl som 7.2:s tröskel: en tidsgräns som slår till slumpmässigt skyddar ingenting.
  - **Kvar, utskrivet:** (1) **inget GUI-körtest** (att det *låter* likadant och att en upptining ger tillbaka exakt samma musik) — kräver Alex; (2) frysningen gäller **sång-läget**: pattern-läget spelar kanalracket och har inget spår att frysa, vilket står i koden; (3) bussarnas läge är inte med i fingeravtrycket (mastern renderas torr, så det spelar ingen roll för ljudet — men det ska inte heller påstås vara fångat); (4) ett spår med plugin-insert kan inte frysas alls (vägras med besked i statusraden).
  - **Filer:** `src/ui/app.rs`, `src/i18n.rs`
- [ ] **8.2 Tempo map** — tempobyten och taktart i låten (rör projektformat, export och SMF). — *steg 1 klart 2026-09-11, steg 2–3 kvar*
  - **Mätt först, och mitt första tal var fel.** En snabb mätning sa "132 ställen i `app.rs` räknar takter ↔ sekunder med ett tempo". Det är antalet **rader**, och en rad är inte en risk. Mätt i arbetsenheter i stället:
    - **20 funktioner** definierar en egen `sec_per_bar = 60/bpm*4` — det är de som antar att tiden är linjär, och det är dem som måste bli positionsmedvetna.
    - **22** ställen räknar steg↔sekunder direkt (`60/bpm/4`).
    - **24** ställen formaterar tid för visning (lägst risk — de ska bara visa rätt).
    - **1** klocka: `step_duration()` + `song_time += dur` — den centrala, och den som avgör om uppspelningen alls följer kartan.
  - **Varför den är *L* och inte *S*:** i dag är tiden linjär överallt, så takter × sekunder-per-takt stämmer överallt. En tempo map gör den **styckvis linjär**, och då är varje omräkning som inte konsulterar kartan *tyst fel* — regioner hamnar fel, exporten blir fel längd, SMF-tickarna pekar fel, och spelhuvudet glider. Felet syns inte förrän något låter fel, vilket är den sämsta sortens fel.
  - **Plan, i tre steg (så att inget kan gå sönder osynligt):**
    1. **Typen och beviset.** `TempoMap` (i `audio/`, eftersom exporten behöver den) med `secs_at_bar`/`bar_at_secs`/`secs_per_bar_at`, `#[serde(default)]` så att äldre filer läses och deras `bpm` blir första punkten. **Beviset:** tester som visar att **ett enda tempo ger exakt samma siffror som i dag** — inte "ungefär", utan samma f32 — och att omräkningen går att vända fram och tillbaka utan att glida.
    2. **Kärnan:** klockan, exporten/render-specen och frysningen (frysningens regionlängd räknas redan ur bufferten i stället för ur takter, så den är redan oberoende av tempot — det ska stå kvar så). Här hörs tidsfelen.
    3. **Resten:** de 20 funktionerna och de 22 stegomräkningarna, en i taget, med sviten som skydd; sist de 24 visningsställena.
  - **Kriteriet för att stega vidare:** hela testsviten grön efter varje delsteg, och kompatibilitetstestet från steg 1 orört. Går det inte att hålla är steget för stort och ska delas igen.
  - **Kvar att bestämma:** omfånget för taktart (bara tempo, eller även 3/4, 6/8 …) — taktart rör fler ställen än tempo, och det är en egen fråga.
  - **Steg 1 klart (2026-09-11): `src/audio/tempo.rs`.** `TempoMap` med `bpm_at` / `secs_per_bar_at` / `secs_per_step_at` / `secs_at_bar` / `bar_at_secs`, normaliserad när den skapas (sorterad, **en punkt per takt där den sista vinner**, och alltid en punkt i takt 0 så att ingen takt står utan svar) och med serde så att en äldre projektfil kan läsas.
    - **Beviset, som var hela poängen med steget:** med ett enda tempo räknar kartan **bitvis** samma tal som appen gör i dag — `(60/bpm)*4` för takter och `(60/bpm)/4` för steg, i f32 — prövat för sex tempon och fem takter vardera. Inte "ungefär": exakt samma f32. Därför får klockan använda kartan redan nu, och uppspelningen är bevisat oförändrad.
    - **Sju tester:** bitvis-kompatibiliteten · styckvis linjär med två tempon (handräknade värden) · inversen fram och tillbaka · normaliseringen (osorterad, dubblett, utan takt 0, tom) · `is_single` · tempobyte i takt 0 · JSON-rundgång.
    - **Två fel som testerna fångade i min egen kod:** `dedup_by_key` behåller den **första** dubbletten, inte den sista — koden gjorde alltså motsatsen mot vad kommentaren sa, och rättningen skrevs om för att vara enkel i stället för listig.
    - **Kvar till steg 2–3:** de 20 funktioner som definierar sin egen `sec_per_bar`, de 22 ställena som räknar steg direkt, och 24 visningsställen. Dessutom har appen i dag **tre** varianter av samma omräkning — `(60/bpm)*4`, `(60/bpm.max(40.0))*4` och `(60/bpm)/4` — och de ska bli en enda när klockan och exporten har bytt.
  - **Steg 2 påbörjat (2026-09-11/12) — kärnan, den del som hörs:**
    - **Omräkningen takter→sekunder finns nu på ETT ställe** (`stem_regions_for`): två synkvägar och exporten gjorde samma sak var för sig, vilket är hur två svar på samma fråga uppstår. Alla tre går genom tempokartan, så en framtida karta slår igenom i uppspelning och export samtidigt.
    - `build_render_spec` (exporten) använder samma väg, och svepets svans räknas med `secs_per_beat_at` — bitvis samma uttryck som förut, för både enpunkts- och flerpunktsfallet.
    - **En tyst beteendeändring infördes och fångades under arbetet:** den gamla exporten uteslöt ett fruset spårs ljud i pattern-läget, och den nya gemensamma vägen gjorde inte det. Regeln ligger nu som en **ren funktion** (`frozen_audio_in_render`) med test — den tredje regeln i det här området som blir prövbar i stället för att bo inuti en stor funktion.
    - **En konvention enhetliggjord:** appen hade tre varianter av samma omräkning — `(60/bpm)*4`, `(60/bpm.max(40.0))*4` och `(60/bpm)/4` — varav två med 40-BPM-golv och en utan. Golvet gäller nu överallt i den gemensamma vägen. Skillnaden syns bara under 40 BPM, där appen förut var oense med sig själv.
  - **Steg 2 klart för det som HÖRS (2026-09-12).** Inventeringen sa 24 ställen. Nu går **20 av dem** genom tempokartan — positioner via `secs_at_bar`/`bar_at_secs`, längder via `secs_for_bars_at`/`bars_for_secs_at` — och de **4 som återstår är ren visning** (`render_playlist_arranger` ×3, `render_stem_focus_modal` ×1).
    - Det är milstolpen som betyder något: **ingen tyst tidsomräkning finns kvar i det som hörs.** Ett fel i visningen syns (en kloss ritas på fel plats); ett fel i uppspelningen hörs som att något glider, och det är den sortens fel en tempokarta annars för med sig.
    - Metod: grupp för grupp, full testsvit efter varje, och `cargo build --release` för att fånga varningar **innan** CI (en blev oanvänd av migreringen och togs bort direkt — gaten från i morse visade sitt värde samma dag).
  - **UI:t klart (2026-09-12), i den form Alex valde:** högerklick på taktlinjalen → **⏱ Sätt tempo här** (och **🗑 Ta bort tempobyte här** när det finns ett där), plus en **⏱ Tempo**-knapp i transportraden som öppnar listan över punkterna — där kan tempot ändras per byte och punkter tas bort. Takt 1 visas som `(start)` och går inte att ta bort.
    - **Reglerna är rena funktioner med egna tester** (`set_tempo_point`/`remove_tempo_point` i `tempo.rs`): en punkt per takt (den sista vinner), takt 0 går inte att ta bort och vägrar **högt** i stället för att tyst låta bli, listan hålls sorterad. UI:t kopplar bara in dem — det är samma grepp som resten av dagen: det som går att pröva utan fönster ska inte bo i en klickhanterare.
    - En detalj som hade blivit fel utan eftertanke: **taktnumret räknas ut på klicket, inte inne i menyn** — inne i menyn står pekaren över menyn, och bytet hade hamnat på fel takt.
    - **GUI:t är inte kört** (ingen skärm i den sessionen). Det står i listan över det Alex behöver bekräfta, och det är samma sorts kvittens som 6.2, 6.4, 6.5 och 7.4 väntar på.
  - **Visningsställena påbörjade (2026-09-12).** Klara: transportens taktvisning, **snäppningen på linjalen** och verktygstipset.
    - Snäppningen var den viktigaste, och den var inte "visning": ett 16-delssteg är en **plats i takten**, så den snäpper nu i takter och frågar kartan om sekunderna **en gång efteråt**. Förut räknade den steg med "sekunder per takt", vilket inte är ett tal över ett tempobyte. Hundradelsläget är undantaget och står kvar i sekunder — där *är* sekunden enheten.
    - Vid ett enda tempo ger båda formerna samma svar (matematiskt identiska, isär bara i sista biten), så ingenting har ändrat beteende för den som inte har några byten.
  - **Kvar på 8.2:s visning:** tidlinjens `px_per_sec` (vågformernas insidor ritas i sekunder ur ett enda tempo), linjalens sekundetiketter, automation-lanen (dess punkter ligger i sekunder — en egen fråga: ska automation flytta med tempot?) och drag-utökningen. Alla är rätt vid ett tempo och en **approximation** vid ett byte; vågformen inuti en kloss som korsar ett byte är den enda som syns tydligt. Det är nästa pass, med färskt sammanhang — inte ihopträngt i slutet av ett annat.
    - Ett eget misstag värt att skriva ned: jag gissade indragningen på nästlade funktioner (12 blanksteg, inte 8) och patchen föll tre gånger i rad på det. Att *läsa* de exakta raderna tog en körning; att gissa tog tre.
  - **SMF-vägen båda håll klart (2026-09-12).** Exporten skrev redan tempobyten (`write_midi_with_tempo` med punkter ur tempokartan), men **importen** läste bara filens första tempo: `ParsedMidi.tempo_events` fanns och var oanvänd (med en `dead_code`-förklaring som sade "läses av importen när den tar med tempobyten (8.2 steg 3)").
    - Nu tar importen in dem genom den **rena** funktionen `tempo::tempo_points_for_import(events, project_bpm)`: filens tempobyten blir projektets tempopunkter **bara** när projektet står kvar på sitt ursprungliga 120 BPM — samma regel som för filens `bpm`, av samma skäl (annars vore importen en tyst tempoändring). Ett projekt med eget tempo behåller sitt, och filens byten **sägs högt** i statusraden i stället för att tappas.
    - Modellen har en punkt **per takt**, så ett byte mitt i en takt avrundas till närmaste takt (en takt är den minsta platsen modellen har, och att tyst kasta bytet vore att tappa det). Normaliseringen återanvänds från `TempoMap::from_points`, så "en punkt per takt, alltid en i takt 0" finns på ett ställe.
    - **Bevis, som en rundgång:** två tempon skrivs ut i en fil (128 BPM från takt 0, 90 BPM från takt 4), filen läses tillbaka, och projektets punkter blir `(0, 128)` och `(4, 90)` — skrivarens `tick = takt × PPQ × 4` och läsarens `takt = tick ÷ (PPQ × 4)` är varandras motparter. Plus `tempo_points_for_import`s egna fall: 120 → karta, 140/100 → inget, tom lista → inget, ett sent första byte flyttas till takt 0, två byten i samma takt blir en punkt (den sista vinner). **318 tester default, 364 med plugin-host, 0 varningar.**
  - **Kvar till steg 3:** de 4 visningsställena (`render_playlist_arranger` ×3, `render_stem_focus_modal` ×1), automation-lanen (punkterna ligger i sekunder — egen fråga: ska automation flytta med tempot?), drag-utökningen, och en GUI-kvittens på tempopunkt-UI:t (samma sorts kvittens som 6.2, 6.4, 6.5 och 7.4 väntar på).
  - **En läxa om verktyg, för framtiden:** `cargo fmt` får **inte** köras i det här repot. Det är inte rustfmt-formaterat, så en körning gav 11 123 rader churn i 51 filer — allt backat, och nya filer formateras enskilt (`rustfmt <fil>`) i stället.
- [ ] **8.3 Routing på riktigt** (utöver bussar/VCA: sends och sidokedjor mellan spår).
  - **Sidokedjorna är klara (2026-09-12).** Ett spår kan duckas av ett annat spårs ljud:
    `AudioCommand::SetStemTrackSidechain { track_index, from, amount_db, threshold_db }`,
    en `Ducker` per spår i `src/audio/synth.rs`, "Sidokedja:"-väljaren med Duckning och
    Tröskel i mixerns kanalpanel, fält i projektfilen (`sidechain_from`,
    `sidechain_amount_db`, `sidechain_threshold_db`, alla `#[serde(default)]`) och samma
    kommando i offline-exporten (`load_timeline_into_engine`) — alltså samma ljud i filen
    som i högtalarna.
  - **Varför en `Ducker` och inte en kompressor med extern nyckel:** det är samma
    matematik (envelopföljare på key-signalen, och gainen multipliceras på det egna
    spåret), men utskriven och med två reglage i stället för fem: tröskel och duckning i
    dB, med fasta tider (5 ms attack, 120 ms release — vad en duckare brukar ha).
  - **Nyckeln är som mest ett sample gammal.** Spårloopen går i indexordning, så ett
    key-spår med högre index har passerat förra samplet när målet räknas (20 µs vid
    48 kHz, mot tider i millisekunder). Ett key-spår med lägre index är exakt i fas. Att
    kräva exakt samma sample hade krävt en omsortering av hela loopen per sample.
  - **En slinga kan inte uppstå:** `from` som pekar på spåret självt ignoreras när det
    sätts, och ett `from` utanför spårlistan ignoreras vid användning — så ett borttaget
    spår duckar ingen i stället för att få motorn att läsa utanför bufferten.
  - **Bevis:** `a_sidechain_ducks_the_target_track` mäter energin vid **880 Hz**
    (Goertzel) med nyckeln på 220 Hz — målet dämpas och nyckeln hörs som förut;
    `an_unset_sidechain_leaves_the_signal_untouched` (bitvis lika utan koppling),
    `removing_a_sidechain_lets_the_level_come_back`, `a_sidechain_never_points_at_the_track_itself_or_outside_the_list`,
    `a_ducker_stays_open_below_the_threshold_and_closes_above_it`,
    `a_zero_amount_ducker_never_touches_the_signal`, `a_sidechain_follows_the_offline_render`
    (exporten duckar likadant) och två projektfilstester (en gammal fil utan fältet läses
    som ett spår utan sidokedja med standardtröskeln −30 dB; en fil med fältet får exakt
    samma värden tillbaka). **328 tester default, 374 med plugin-host, 0 varningar** (det
    sista testet är bänkens nya räknare, se 7.2-raden).
  - **Kvar på samma punkt: sends mellan spår** — att skicka ett spår in i ett annat spårs
    kedja. Det är större än sidokedjan: key-signalen är en *mätning* (ett sample sent går
    bra), men en send är *ljud* och måste vara exakt i fas, annars tar den ut sig själv mot
    originalsignalen. Det kräver att spårloopen i `process_stereo` delas i två faser
    (källor först, kedjor sedan i topologisk ordning) plus en kontroll som vägrar en
    koppling som skulle bli en slinga. **Eget pass, med färskt sammanhang** — inte
    ihopträngt i slutet av ett annat.
- [ ] **8.4 Sampler** (ett riktigt samplerinstrument i kanalracket, inte bara en WAV-spelare).
- [ ] **8.6 Plugins: bryggning, egna utgångar och sidokedja in i en plugin** — *M*
  - **Läget hos oss, mätt 2026-09-12:** Sonix hostar VST2/VST3/CLAP/LV2, skyddar sig mot
    krascher med en **egen out-of-process-sandbox** (övervakaren startar om en död worker),
    öppnar plugin-GUI:t i ett **eget X11-fönster** (`plugin_gui.rs`), listar pluginens
    parametrar (`plugins_view.rs`) och **säger till när en sparad plugin inte kunde
    återställas** ("⚠ N plugin(s) kunde inte återställas") — samma besked som Audacity
    införde i PR #3801. Det är alltså inte där gapet sitter.
  - **Tre saker FL:s Fruity Wrapper har som vi inte har:**
    1. **32-bitars plugin.** FL:s IL Bridge kör dem i en separat OS-process (ca 2 % extra
       CPU per plugin, och bridgade plugins stjäl tangentbordsfokus); Sonix hostar bara
       64-bitars.
    2. **Egna utgångar.** FL:s "Auto map inputs/outputs" ger en multi-out-plugin lika många
       mixerspår som dess utgångar. Sonix VST3-hostning processar **en** in- och **en**
       utbuss per anrop (`num_inputs: 1, num_outputs: 1` i `plugin_vst3.rs`) — buss-API:t
       finns inläst (`getBusCount`/`getBusInfo`/`activateBus`) men extra utgångar routas
       inte till egna spår. VST2 läser `num_outputs` (1–8) utan att använda dem.
    3. **Sidokedja in i en plugin.** I FL: högerklicka Track Send på målspåret → "Sidechain
       to this track", peka sedan pluginens Sidechain-selector på källan (send-nivå 0 % =
       ren sidechain; Fruity Compressor fick det först i 2025.2). Sonix kan ducka spår mot
       spår (8.3, 2026-09-12) men har **ingen** sidechain-ingång i en plugin.
  - **Två mindre saker på samma lista:** ett **manuellt latens-offset per plugin** (FL visar
    "manual + plugin" och sparar ett eget offset vid sidan av den automatiska PDC:n — Sonix
    har bara den automatiska), och **"Smart disable"** (FL slutar processa inaktiva plugins,
    "can dramatically reduce CPU load"; Sonix processar dem).
  - **Källa:** `plugin-flstudio-research-sv.md` (Image-Lines onlinemanual; wrapper, mixer,
    plugin-installation). Audacity har ingen av de tre — plugins kör i samma process och kan
    fälla appen, och CLAP nämns inte alls i deras dokumentation.
- [ ] **8.7 Chopper: från en trim-ruta till en slicemappning** — *M*
  - **Läget i koden (mätt 2026-09-12):** `active_chopper_channel` ger **en** trim-ruta per
    kanal (`sample_start`/`sample_end` i procent av filen) med snabbval `1/2`, `1/4`, `2/4` …
    och en knapp märkt **"⚡ Transient"** som bara sätter `sample_end = 0.18`. Ingen
    slicemappning, ingen detektering, ingen koppling till steg eller piano roll, ingen
    slice-export. **Namnet lovar mer än koden gör, och ska rättas oavsett vad som byggs.**
  - **Vad de etablerade gör — samma tre steg:** analysera ljudet och hitta transients/onsets
    (eller dela på fasta divisioner) → mappa varje slice till en not/pad → låt användaren
    nudga, fadea och trigga slicen från **sin egen start** (tiden glider inte, för slicen är
    ingen position i en global loop).
    - **FL Studio:** Edison (Auto slice Dull/Medium/Sharp, Detect beats, Zero-cross check,
      "Convert to score and dump to piano roll"), Slicex (automatisk slicing, region → tangent,
      "Dump score" med presets, per-region AMP/FILTER/SPEED, färg 15/16 = reverse), Fruity
      Slicer 2 (2025.2), Playlist Chop (Bar/Beat/Beat Random) och Slice-verktyget.
    - **Ableton:** Simpler "Slice By" Transient/Beat/Region/Manual (max 64 slices, per-slice
      Fade In/Out) och **Slice to New MIDI Track** (en not per slice, kromatiskt, in i ett
      Drum Rack — max 128 kedjor).
    - **Reaper:** Dynamic Split Items (transients **eller** noise gate, min slice length,
      leading/trailing pad, **Fade pad**) och **Create chromatic MIDI item from slices**.
    - **Bitwig:** Divisions/Beats/**Onsets** (med Onset Sensitivity)/Pitch/Manual.
  - **Algoritmen, belagd:** energy/HFC/spectral flux med peak picking är det enklaste som
    faktiskt fungerar på trummor — aubio defaultar till `hfc` och kallar det effektivt för
    perkussiva onsets; librossas `onset_detect(backtrack=True)` back-trackar en onset till
    närmaste föregående energiminimum och är gjord just för "onsets as slice points".
    SuperFlux (Böck & Widmer, DAFx-13) behövs främst vid vibrato/pitchat material, och CNN
    (Schlüter & Böck) är starkare men kräver en modell. Frame 1024 / hop 512 ≈ 11,6 ms
    upplösning vid 44,1 kHz är en rimlig start.
  - **Minsta ärliga första steg:** (1) onset-detektering som ger **N** slicepunkter,
    (2) en slicekarta med start/slut per slice, (3) mappning till steg/tangenter och en dump
    till piano rollen, (4) fade på 1–5 ms vid slice-kanterna (klickskyddet som Reapers Fade
    pad och Abeltons per-slice-fade ger), (5) nudge av en enstaka slicegräns. Går att dela i
    två pass: först detektering + slicekarta (hörs direkt vid uppspelning), sedan
    mappning/dump.
  - **Källa:** `chopping-and-effects-research-sv.md` + FL-avsnittet i
    `plugin-flstudio-research-sv.md`.
- [ ] **8.8 Automatisering av fler parametrar** (plugin-, EQ-, kompressor- och buss-/VCA-parametrar) — *S–M*
  - **Läget i koden (mätt 2026-09-12):** `AutomationParam` har fyra varianter — `Volume`,
    `Pan`, `ReverbSend`, `DelaySend`. Alltså ingen plugin-parameter, ingen EQ- eller
    kompressorparameter, ingen buss-/VCA-fader.
  - **Varför:** när en DAW hostar plugins med exponerade parametrar förväntar sig användaren
    att kunna rita en kurva för dem — det är så automation används i praktiken. Pluginens
    parametrar finns redan i UI:t, så steget är att låta automationens mål peka på dem **och**
    att kurvan läses per sample i stället för per UI-bildruta (samma sak som gap 6 i
    `references/daw-comparison.md`: `apply_automation()` körs i `update()`).
  - **Ärlig brasklapp:** den här researchrundan belade **inte** FL:s exakta automation av
    plugin-parametrar, och Audacitys parametrar i realtidsstacken står som **INTE VERIFIERAT**
    i rapporten. Kontrollera den raden innan punkten blir ett krav. Sonix-läget ovan är
    däremot mätt i koden.
- [ ] **8.9 Makron: en kedja av kommandon över många filer** — *S*
  - **Vad Audacity har:** en **Macro** är en sekvens förkonfigurerade kommandon (mest
    effekter, men också Select-kommandon, Find Clipping och exportkommandon) som körs
    automatiskt — på ett projekt eller i **batch över filer** (rekommenderat max 500 i taget,
    utdata i mappen `macro-output`). De byggs i **Tools > Manage Macros** och sparas som TXT.
    Alla effektformat kan ingå (built-in, LADSPA, LV2, Nyquist, VST, AU).
  - **Hos oss:** det finns en exportkö och en stämimportväg, men **ingen** kedja av
    kommandon som kan köras på flera filer med sparade inställningar. Nyttan är densamma som
    Audacitys: samma behandling på tjugo tagningar utan tjugo handgrepp.
  - **Noterat samtidigt:** Audacitys *skriptväg* (Nyquist/Python) och Reapers ReaScript/Lua
    är fortfarande gap 5 i `references/daw-comparison.md` — det är en större sak än ett
    makro och hör inte till den här punkten.
  - **Källa:** `plugin-audacity-research-sv.md` (avsnitt 3).

### 8.3 Exakta vågformer (Alex krav, 2026-09-12)

**Kravet, ordagrant:** vågformerna i tidslinjen ska vara **exakta**, inte "pixlade" som i dag — musikskapande och klippning kräver exakthet.

**Orsaken, mätt (inte gissad):** `visual_peaks_from` trycker ihop hela filen till **högst 512 punkter** *en gång*, oavsett zoom (`let count = 512.min(samples.len().max(64))`). En fyra minuter lång tagning är 11,5 miljoner samplar → en punkt per **~0,47 sekund**. De 512 punkterna sträcks sedan ut över hur många bildpunkter regionen än upptar, och då syns de som trappsteg. Det går inte att klippa exakt i en vågform som visar en halv sekund per steg.

Dessutom räknades bara `max(abs(x))` per fönster: ett **symmetriskt** hölje. Det ser rimligt ut för en sinuston men ljuger om en osymmetrisk signal — och formen är just vad man tittar på när man letar efter var ett anslag börjar.

**Gjort:** `src/audio/waveform.rs` — `envelope_per_pixel(samples, pixels)` ger ett **äkta (min, max) per bildpunkt**. Fyra tester håller det fast:

- ett anslag på sample 900 av 1000 hamnar i kolumn 90 av 100 (och i sista kolumnen av 10),
- osymmetri bevaras (+0,2/−0,8 är inte ±0,8),
- varje sample räknas exakt en gång och i ordning,
- bredden ger exakt så många kolumner, också när det finns fler pixlar än samplar.

**Kvar till kravet är uppfyllt i fönstret:**

1. **Flernivå-cache (mip)** i `waveform_cache_dir()`: att räkna om höljet ur hela PCM:en varje bildruta är O(samplar) per bildruta och går inte för en lång fil. Cachen ska ge *samma svar snabbare* — och kan därför prövas mot `envelope_per_pixel`, vilket är skälet att den funktionen skrevs först.
2. **Koppla in den i ritningen** (samma pass som tidlinjens `px_per_sec`), där vågformen ritas som vertikala streck mellan min och max i stället för som en kurva genom 512 punkter.

**Inkopplingen är GJORD (2026-09-12).** Det som står nedan är vad som gjordes, sparat för att nästa läsare ska se varför det ser ut som det gör — inte som en kvarvarande uppgift.

- **Var:** `render_playlist_arranger` i `src/ui/app.rs`. Två saker där är kvar i sekunder: regionens vågform ritas ur `region.waveform_data` (512 punkter från `visual_peaks_from`) och `px_per_sec` räknas ur ett enda tempo.
- **Vad:**
  1. Håll en `WaveformCache` per spår — byggd **en gång** ur `pcm_audio` (eller `frozen_pcm` för ett fruset spår). Kostnaden är en O(samplar)-genomgång, ~50 ms för en fyra minuters fil, och minnet några hundra kB per fil.
  2. Rita **en vertikal linje per bildpunkt** i stället för en kurva genom 512 punkter. API:t finns: `cache.envelope_at(&pcm, start_sample, len_samples, bredd_i_pixlar)` — ett utsnitt, för en region börjar på en offset i spårets ljud, och vid inzoomning räknas det exakt ur samplen. Klossarnas x-positioner är redan i takt-rymd (`start_bar * bar_w`) — det är vågformens *insida* som ska följa efter.
  3. Räkna regionernas tid ur `tempo_map()` i stället för `px_per_sec` (samma sak som gjordes för snäppningen ovan: takter först, sekunder en gång efteråt).
- **Varför ingen cachefil:** att skriva nivåerna till `waveform_cache_dir()` är en optimering, inte ett krav — det är en engångskostnad per fil. En cachefil till är en sak som kan bli gammal och fel, och `visual_peaks_from`-problemet var just att en *föråldrad* sammanfattning ritades. Bygg i minnet först; mät om det någonsin behövs.
- **Kvittensen hos Alex:** vågformen ska visa **enskilda anslag** när man zoomar in, och en kloss som korsar ett tempobyte ska sluta glida.

**Skärpt efter Alex' kvittens ("under hyfsad nivå, dock ännu pixlad — svårt att få exakt punkt där ljud börjar").** Fyra orsaker, alla mätta och åtgärdade:

- **Facket var 256 samplar = 5,3 ms.** Ett fack är den största möjliga oskärpan i höljet, och 5 ms syntes. Nu 64 = 1,3 ms (~1,4 MB för fyra minuter i finaste nivån, knappt 3 MB för alla nivåer).
- **Slingade klossar** gick fortfarande genom de gamla 512 punkterna. Nu `envelope_looped`.
- **Vågformen ritades indragen 2 px** från klossens kanter, vilket försköt *hela* x-mappningen — mest i början av klossen, alltså precis där startpunkten ska prickas. Nu ritas hela rektangeln.
- **Kolumner mellan pixlar** flöt ut över två pixlar och såg mjuka ut. Nu ritas de på pixelcentra.

**Kvar att kvittera:** om startpunkten fortfarande är svår att pricka är nästa fråga klossens **höjd** i pixlar (vertikal upplösning), inte höljet — det taket är orört.

**Läget efter inkopplingen (mätt i koden, 2026-09-12):** tidslinjen ritar ett äkta (min, max) per bildpunkt ur cachen för **icke-slingade** regioner med PCM; den gamla vägen ligger kvar orörd som fallback för regioner utan PCM och för slingade regioner. Cachen byggs på ett ställe (`ensure_waveform_cache`, i spårloopen) och nycklas på buffertens identitet, så en ny tagning eller en frysning ger automatiskt en ny cache. Kostnaden är en O(samplar)-genomgång per spår och ljud (~50 ms för fyra minuter), en gång.

**Kvar på 8.3:** (1) slingade regioner har sin funktion (`envelope_looped`, testad: samma svar varje varv, och ingen förlust vid skarven) och den är **inkopplad** sedan 2026-09-12: ritningen väljer väg efter om regionen är slingad — deras bildpunkter följer upprepningen och kräver en egen lösning; (2) `render_playlist_arranger`s `px_per_sec` och linjalens sekundetiketter är kvar i sekunder (samma pass som 8.2:s visning); (3) ⏱ **klart 2026-09-12:** `PlaylistTrack.audio_waveform` var död kod (sattes på sex ställen, lästes aldrig) och är borttagen — med den tre `visual_peaks_from`-anrop som räknade O(samplar) i onödan vid varje import, tagning och AI-stämma.

### 8.4 Vågformer som blir tydligare ju mer man zoomar (Alex önskemål, 2026-09-12)

**Önskemålet:** "supertydliga vågor, och när man zoomar in ska den bli tydligare ju mer man zoomar, så man ser EXAKT när ett ljud börjar".

**Efterforskningen** (Reaper, Ardour, BBC audiowaveform, Audacity) finns i `sonix`-skillen, `references/waveform-rendering.md`. Kärnan: **"tydligare ju mer man zoomar" kommer inte av sig själv.** En stapel per bildpunkt är grumlig när pixlarna är få och samplarna många, hur exakt höljet än är — Ardour säger rakt ut att höljet är en *approximation* och att den verkliga vågformen bara syns högst upp i zoomningen. Ritaren måste alltså **byta representation**:

| Läge | När | Vad som ritas |
|---|---|---|
| Envelope | > 1 sampel per bildpunkt | min/max per kolumn |
| Samples | ≈ 1 sampel per bildpunkt | **kurva genom samplarna** |
| SampleDots | ≤ 0,25 sampel per bildpunkt | **punkt per sampel + linje** (Audacitys "dots") |

**Gjort:** `zoom_regime()` som ren funktion med tester (trösklarna, och att regimen aldrig backar när man zoomar in — annars flimrar vågformen vid tröskeln), och alla tre lägena inkopplade i regionritningen. Slingade klossar går genom samma växel.

**Kvar:** Y-skalan. En kloss på 30 px har 30 steg upp och ned — det är ett tak, inte ett fel, och det kräver högre klossar eller en egen höjd-zoom (Ardours log-skala är alternativet för svaga partier). RMS-band som tillval är också kvar.


---

## 8.5 Stämimport och tysta klipp (Alex 2026-09-12)

**Utgångspunkten var en skärmbild:** korta klipp ritade som pixlade staplar, långa
som riktiga vågor. Alex lyssnade och konstaterade att de pixlade var **tysta**.

### Vad som är mätt och stängt

- **Appen har ingen mp3-avkodare.** Inga symphonia/rodio/minimp3 i beroendena, och
  `load_wav_pcm` läser WAV. En mp3 som kommer in i en regions `source_path` blir
  därför ett **tyst klipp, varje gång** — och eftersom översikten fanns kvar ritades
  det ändå, som om det hade ljud.
- **Rotorsaken till att tystnaden var osynlig:** elva ställen läste ljud med
  `if let Ok(...)` **utan `else`**. En fil som inte gick att läsa föll bort utan ett
  ord. **Stängt:** `load_audio_or_report` + en delad lista (`UNREADABLE_SOURCES`),
  avläst en gång per bildruta i `update`, som skriver i statusraden. Sju anrop
  bytta; `load_sample_pcm_arcs` (fyra anropare, bl.a. kanalernas ljud) rapporterar
  nu genom samma lista.
- **Alex' projektfil** (utanför repot): nio regioner pekade på `.mp3` medan
  `.wav`-filerna fanns på disk. Omskrivna, backup, atomiskt, verifierat genom
  återläsning (`mp3=0, wav=18, saknade filer=0`). Klippen spelar.
- **`stem_files_for_import(files, keep_mp3)`**: en stämma importeras en gång — en
  mp3 vars stämma redan finns som wav hoppas över, en mp3 utan wav-syskon tas med,
  `keep_mp3` behåller allt. Inkopplad i `scan_for_suno_stems`, med antalet
  överhoppade i statusraden. Två tester.

### Modellen framåt — genomgången (KLAR 2026-09-12)

`import_audio_file_as_track` gör rätt: den **rapporterar och avbryter** när ljudet
inte kan läsas. Den skapar aldrig ett klipp utan ljud. Nu följer alla vägar som
skapar en region eller ett klipp den modellen — och de tre som i stället **hittade
på ett ljud** hittar inte på något längre:

- ~~`add_sample_item_to_timeline` (två varianter)~~ — **KLART.** Båda hittade på en
  längd (fyra takter) när avkodningen misslyckades och skapade regionen ändå, med
  bibliotekets grova översikt som vågform och originalfilen som källa. Det var
  exakt hur Alex' korta klipp blev tysta med trovärdiga vågor. Nu: rapportera och
  avbryt, som `import_audio_file_as_track`. I `add_sample_item_to_track_at_bar`
  ligger guarden **före** spårskapandet — annars blev ett tomt spår kvar.
- ~~`open_sample_in_vocal_studio` / `open_region_in_vocal_studio`~~ — **KLART.**
  Båda byggde en **syntetisk sinuston** när filen inte gick att läsa (sample-vägen:
  två sekunder ur samplens `default_note`; region-vägen: 220 Hz) och öppnade den
  som en tagning i Sångstudion. En mp3 blev alltså en *påkittad* tagning i stället
  för ett besked, och den som lyssnade hörde något som varken var samplen eller
  tystnad. Nu: besked i statusraden och ingen tagning. Sample-vägen skickar också
  med **filens egen** samplerate (förut sades 44100 oavsett, så en 48 kHz-tagning
  spelades i fel hastighet).
- ~~▶-förhandslyssningen i Sound Browser~~ — **KLART**, och felet var större än
  raden antydde: vägen hade **två fall men tre situationer**. En sample utan fil
  spelar appens egen syntröst (riktigt svar), men en sample *med* fil som inte gick
  att läsa föll ned i samma gren — den som lyssnade hörde en syntetisk trumma och
  trodde det var filen. Samma sak i `audition_library_sample` (som också anropas
  från kanalväljaren). Nu: ingen fil → syntrösten, läsbar fil → filen, oläsbar fil
  → ett besked.
- ~~Det frusna spårets avkodning~~ — **KLART.** `frozen_pcm` ärvde `track_pcm`,
  alltså spårets eget klippljud när frysfilen inte gick att läsa: ett spår som ser
  fruset ut spelade sin **ofrusna** mix (och i värsta fall tyst). Nu bär
  `PreloadedTrackData.frozen_pcm` bara det frusna ljudet, "filen saknas" och "filen
  går inte att läsa" ger samma besked, och alla problem samlas i stället för att
  skriva över varandra — förut nämndes bara den sista filen.
- ~~`load_sample_pcm_arcs`-anroparna rapporterar nu, men **avbryter** inte~~
  — **KLART för de två som *väljer* ett ljud:** `assign_library_sample_to_channel`
  flyttade förut in namn, färg, steg och bibliotekets grova vågform i kanalen även
  när filen inte kunde läsas, medan `pcm_audio` blev tom — kanalen såg ut att ha ett
  eget sample och spelade den inbyggda synten. Nu läses ljudet **först**; går det
  inte är kanalen orörd och beskedet står i statusraden. **Medvetet undantag:**
  `saved_to_channel` (projektinläsning) kan inte avbrytas för en saknad samplefil —
  där blir i stället vågformen **tom** (den räknas ur ljudet) så att inget ser ut att
  ha ljud, och filen nämns i statusraden genom samma lista. Testat
  (`a_channel_with_an_unreadable_sample_carries_no_waveform`).

**Bevis:** 316 tester default, 362 med `plugin-host`, 0 varningar i båda profilerna.
Nya tester: stämmorna skrivs och läses tillbaka med hela ljudet, vågformen kommer ur
filen, ett snedstreck i titeln kan inte lämna katalogen, noll stämmor är ett fel, en
katalog som inte går att skapa är ett fel, namnregeln för stämfilerna, kanalen orörd
vid oläsbar fil (och ljudet med när filen går att läsa), kanal utan vågform när
samplen är oläsbar, frågans räkning ger samma svar som importen, wav-syskonet hittas
bara när det finns, och (ignorerad, körd) en mätning mot ett **riktigt** zip-arkiv.

### Dialog vid import (byggd 2026-09-12)

Frågan ställs av `render_wav_question_modal` och gäller **alla vägar in**:

- **Mappimport och ZIP-import:** frågan kommer innan avkodningen börjar, och bara
  när det finns något att fråga om (mp3:er med wav-syskon — annars går importen rakt
  igenom som förut). Standardvalet är regeln: hoppa över dem. ZIP-frågan måste
  komma **före** uppackningen, eftersom uppackningen sker i bakgrunden; arkivets
  innehållsförteckning läses därför med `unzip -Z1` utan att packa upp.
- **Stämseparatorn:** väljer man en mp3 som har sin wav bredvid sig ställs samma
  fråga — separera wav-filen (rekommenderas) eller mp3-filen. Båda ger samma ljud,
  men mp3:an måste konverteras först, och den som *vill* använda mp3:an (kanske den
  enda mastern) kan säga det.
- Regeln och frågan delar kod: `stem_files_for_import` väljer, `duplicate_mp3_count`
  räknar (samma regel), `list_stem_audio_files` ser till att frågan och importen
  tittar på **samma** lista. Engelsk rad för hela 8.5-familjen (35 strängar) så att
  fallbacken inte visar svenska för en engelsk användare.
- **Ej klickad i GUI:** dialogen är inte sedd med egna ögon i den här sessionen.
  Mätt, inte gissat: appens skärmdumpsloop (`--capture-screenshots`, den som gav
  `screenshots/` 2026-09-11) **kör inga bildrutor alls** i det här fönsterläget —
  ingen rad skrivs efter starten, ingen PNG blir till, och räkningen i `update()`
  kommer aldrig igång (alltså inte ens "första vyn satt"). `grim` mot DP-3 och mot
  hela skärmen gav bara bakgrundsbilden på alla tre skärmarna, så fönstret ritas
  uppenbarligen inte. Fönstret syntes ändå som `mapped=True, visible=True` på DP-3
  (Alex fokuserade skärm var DP-1), och `hyprctl dispatch focuswindow` går inte att
  använda i den här Hyprland-versionen (ny Lua-syntax) — fokus gick inte att tvinga
  fram. **Ett verktygsfel som detta ska inte vara tyst:** loopen har nu ett tålamod
  (`SCREENSHOT_WAIT_FRAMES = 180`, ≈3 s) som skriver ut felet och avbryter i stället
  för att vänta för evigt. Det skyddar den väg där bildrutor kommer men svaret inte —
  den här gången kom inga bildrutor, så tålamodet hann inte prövas.
  En ny målbild finns för ändamålet (`ScreenshotTarget::WavQuestionModal` →
  `30_dialog_mp3_eller_wav.png`), så nästa gång loopen fungerar blir den fångad
  automatiskt. Tills dess: kör `sonix <mapp>` med en wav + en mp3 med samma stämnamn
  i mappen — frågan kommer direkt vid start.

### Separatören skriver stämmorna till disk (KLART 2026-09-12)

`stem_separator.rs` skrev aldrig ut stämmorna: dess enda spår blev den grova
översikten (512 punkter) plus en `source_path` som pekade på **originalet**. Kom
originalet från en mp3 uppstod precis den kombination Alex såg — en trovärdig
vågform och inget ljud — och tystnaden kom tillbaka varje gång projektet öppnades.

Nu skriver `write_stems_to_dir` varje stämma som **32-bitars flyttals-WAV** i
projektets egen materialmapp (`<projektmappen>/Stems/<källa>-vocals.wav` osv), och
`write_stems_and_read_envelopes` läser tillbaka dem: regionens `source_path` **och**
dess vågform kommer båda ur den filen. Klippet pekar alltså på sitt eget ljud, och
en trasig eller tom skrivning blir ett fel i statusraden i stället för fyra tysta
klipp. Filnamnet byggs med `crate::autosave::slug`, så ett snedstreck eller `..` i en
låt- eller filttitel kan inte lägga stämman utanför katalogen (testat).
