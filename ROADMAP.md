# 🗺️ Sonix Studio — Roadmap

> Levande utvecklingsplan. **Bocka av `[x]` allt eftersom.** Uppdatera procenten i [Framsteg](#-framsteg-i-siffror) när något blir klart.
>
> Läs tillsammans med **[README.md](README.md)** / **[README_SV.md](README_SV.md)** (funktionslista & ärlig status) och **[tools.md](tools.md)** (detaljerad historik P1–P22).

**Teckenförklaring**

| Markering | Betydelse |
| :--- | :--- |
| ✅ Klar | Implementerad, inkopplad och verifierad (byggt/testat) |
| 🟡 Pågår / delvis | Fungerar delvis eller är en approximation |
| ⬜ Väntar | Inte påbörjad |
| **S** / **M** / **L** / **XL** | Uppskattad storlek: liten / medel / stor / mycket stor |

---

## 📊 Framsteg i siffror

**Totalt: ~89 % klart** (av det som gränssnittet utlovar)

```text
[████████████████████████████████░░░░]  89 %
```

| Område | Klart | Kvar | Procent |
| :--- | :---: | :---: | :---: |
| Ljudmotor (synth, trummor, FX, patcher, per-spår) | 11 | 1 | **92 %** |
| Sequencer & arranger (timeline, rack, piano roll, sektioner) | 5 | 0 | **100 %** |
| Inspelning & sång (mic, takes, comping, pitch, harmonier) | 5 | 2 | **71 %** |
| Generatorer (ackord, tärning, drummer, tuner, add track) | 5 | 0 | **100 %** |
| AI (lokal + LLM + ljud + kontext) | 4 | 1 | **80 %** |
| Plugin-hantering | 2 | 6 | **25 %** |
| Export & projekt-I/O | 4 | 1 | **80 %** |
| Hårdvara (MCU/OSC) | 2 | 0 | **100 %** |
| Lokalisering & system (7 språk, motor) | 2 | 1 | **67 %** |
| Dokumentation (README, ROADMAP, tools) | 3 | 0 | **100 %** |

> **De två största kvarvarande bitarna:** **Plugin-hosting** (25 %, ej påbörjad värd) och **neural stem-separation** (DSP-approximation idag).

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

## ⬜ Fas 1 — Ärlighet & snabba vinster

Små, tydliga uppgifter som tar bort kvarvarande glapp mellan UI och funktion.

- [x] **1.1 Topologisk sortering i Modular Patcher** — *S* ✅
  - **Löst:** Kahn-sortering i `topo_order()` (`patcher.rs`); processorn utvärderar nu alltid källor före mål oavsett kabelordning. Cykler upptäcks och de noder som inte kan sorteras körs sist (föregående samples utsignal på återkopplingsvägen).
  - **Klart när:** En kabel kan dras "baklänges" och ljudet följer ändå grafen; cykel-test finns. ✅
  - **Filer:** `src/audio/patcher.rs`, `src/ui/patcher_view.rs` (varningsband vid cykel)

- [ ] **1.2 Matcha patcher-nodetiketter mot DSP** — *S*
  - **Gör:** Ta bort eller korrigera etiketter som lovar mer än DSP:n gör (t.ex. filtertyp/parametrar).
  - **Klart när:** Varje nod-etikett beskriver exakt vad koden gör.
  - **Filer:** `src/ui/patcher_view.rs`, `src/audio/patcher.rs`

- [ ] **1.3 Legacy "AI Provider Inställningar"-modal** — *S*
  - **Problem:** Fälten (`ai_provider_*`) läses aldrig och sparas inte.
  - **Gör:** Antingen koppla till `AiConfig` (`~/.config/sonix/ai.json`) **eller** ta bort modalen och hänvisa till AI-assistentens konfig.
  - **Klart när:** Ingen död konfig-yta finns kvar.
  - **Filer:** `src/ui/app.rs` (modal ~`:10370`)

- [ ] **1.4 Applicera samplingsfrekvens & buffertstorlek** — *M*
  - **Problem:** `audio_sample_rate_idx`/`audio_buffer_size_idx` sparas men används inte; enhetens standard gäller.
  - **Gör:** Bygg om cpal-strömmen med valt `SampleRate` + `BufferSize` och visa verklig status; hantera fel snyggt.
  - **Klart när:** Val i Ljudinställningar påverkar faktiskt strömmen.
  - **Filer:** `src/audio/engine.rs`, `src/ui/app.rs`

- [ ] **1.5 `.fst` "Apply Preset"** — *M*
  - **Alternativ A:** Avkoda `.fst` (binärt FL-format) och applicera.
  - **Alternativ B (trolig):** Dölj/inaktivera knappen med tydlig text tills en värd finns.
  - **Klart när:** Ingen no-op-knapp finns kvar.
  - **Filer:** `src/ui/plugins_view.rs`, `src/audio/plugin_host.rs`

---

## ⬜ Fas 2 — Ljudkvalitet & realtid

- [ ] **2.1 Realtids-autotune i ljudtråden** — *L*
  - **Gör:** Flytta pitchkorrigering från offline-buffert till realtidsblock med låg latens (t.ex. PSOLA/phase-vocoder), koppla till mikrofonkedjan.
  - **Klart när:** Sång kan korrigeras live med hörbar effekt och acceptabel latens.
  - **Filer:** `src/audio/vocal_harmonizer.rs`, `src/audio/recorder.rs`, `src/audio/engine.rs`

- [ ] **2.2 Riktig formantbevarande pitch-shift** — *M*
  - **Gör:** Ersätt spektral-tilt-approximationen med riktig formantanalys/bevarande.
  - **Klart när:** Pitch-shift ändrar tonhöjd utan "chipmunk"-artefakter.
  - **Filer:** `src/audio/vocal_harmonizer.rs`

- [ ] **2.3 Pitch-bevarande time-stretch** — *L*
  - **Problem:** Nuvarande är varispeed (ändrar tonhöjd).
  - **Gör:** WSOLA eller phase-vocoder i `StemVoiceTrack`/audition.
  - **Klart när:** Tempo kan ändras utan att tonhöjden ändras.
  - **Filer:** `src/audio/synth.rs`, `src/audio/vocal_harmonizer.rs`

- [ ] **2.4 Per-voice filter & ADSR i Alchemy-synthen** — *M*
  - **Gör:** Separata envelope/filter per röst (polyfonisk), inte globala.
  - **Klart när:** Ackord kan ha oberoende filter/envelope per ton.
  - **Filer:** `src/audio/synth.rs`

---

## ⬜ Fas 3 — Neural stem-separation

- [ ] **3.1 Integrera HTDemucs via ONNX Runtime** — *XL*
  - **Gör:** Lägg till `ort` (ONNX Runtime) + HTDemucs-modell; kör i bakgrundstråd med progress; behåll DSP-separatorn som fallback när modellen saknas.
  - **Klart när:** Vokaler/trummor/bas/övrigt separeras med neural kvalitet och licensierbar modell; test finns.
  - **Filer:** `src/audio/stem_separator.rs`, `Cargo.toml`, `src/ui/stem_view.rs`

---

## ⬜ Fas 4 — Plugin-hosting (största delen)

> Detta är det stora kvarvarande arbetet. Rekommenderad ordning nedan.

- [ ] **4.1 Välj ABI + host-modul** — *L*
  - **Rekommendation:** Börja med **CLAP** (`clack`-craten) — öppen, stabil, bra GUI-extension. Därefter VST3.
  - **Gör:** Ny `src/audio/plugin_host_live.rs` med `dlopen`/`libloading`, entrypoint-scanning, `PluginInstance`-trait.
  - **Klart när:** En CLAP-plugin kan laddas och rapportera sina parametrar.
  - **Filer:** `src/audio/plugin_host.rs`, `Cargo.toml`

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

- [ ] **5.1 Fler tester** för realtids- och plugin-vägar (integrationstester).
- [ ] **5.2 VCA-grupper & sub-mix-bussar** — *M* (om efterfrågat; medvetet borttagna).
- [ ] **5.3 Midi-inspelning/klaviatur-inmatning** (`midir`) till Piano Roll.
- [ ] **5.4 Automationskurvor** på tidslinjen.
- [ ] **5.5 Fler export-presets / loudness-normalisering.**

---

## 🎯 Nästa uppgift

**Fas 1.2 — Matcha patcher-nodetiketter mot DSP** (första ovalda punkten).

## 🛠️ Så här håller vi roadmapen levande

1. Bocka av `[ ]` → `[x]` när uppgiften är **byggd och testad** (`cargo test --release`).
2. Flytta avklarade punkter till **Fas 0** eller lämna kvar med ✅.
3. Uppdatera siffrorna i **[Framsteg](#-framsteg-i-siffror)** (klart/kvar/procent).
4. Lägg till en kort rad i **tools.md** (nästa P-nummer) med vad som gjordes.
