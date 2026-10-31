# Backlogg — allt som återstår, i ordning

Källa: `docs/REAPER-BINARY-MAP.md` och `docs/ABLETON-BINARY-MAP.md`. Storlek: **S** ≈ dagar,
**M** ≈ veckor, **L** ≈ månader. Ordningen är vald efter nytta per arbetsdag, inte efter vilken DAW
posten kommer från.

## 1. Görs nu (S–M, byggstenarna finns, hörs direkt)

| # | Post | Källa | Storlek | Vad som ska mätas |
| :--: | :--- | :--- | :---: | :--- |
| 1 | ✅ **Cytomic-filtren**: Sallen-Key MS2/MD/SD, StateV OSR, Ladder PRD med drive i återkopplingen | Ableton | **M** | Frekvensgång per typ (3 dB-punkt, resonanstopp), THD vid drive |
| 2 | ✅ **Multibandssaturator (`Roar`)**: 3 stag, 12 shaper-lägen, 8 filtertyper, feedback | Ableton | **M** | Övertoner per shaper (THD per läge), bandläckage |
| 3 | ✅ **MIDI-effekter** — skala, transponering, anslag, kanal, ackord, notlängd, eko, slump, arpeggio, strum | Ableton | **M** | 250 ms → 12 000 sampel; eko 2 400/4 800; slump 1..127 |
| 4 | ✅ **DDP-export** (CUE/BIN, CD-TEXT, ISRC) | REAPER | **S–M** | Filstruktur mot spec, red book-ordning |
| 5 | ✅ **Lua 5.4 bredvid Rhai** (vendored mlua, samma tolv funktioner, 5M-instruktionsbudget) | REAPER | **S** | Samma skript ger identisk `Ctx`; oändlig loop stoppas på 21 ms |

## 1b. Mätt 2026-10-30 (klara)

| Post | Mätning |
| :--- | :--- |
| Cytomic-filtren | SK −12,5 dB/okt · SVF −12,3 · ladder −24,0 · topp +26,2 dB · självsvängning 0,75 rms · THD 0,02 → **5,27 %** med drive |
| Multibandssaturator | bandsumma **0,122 dB** · THD 5,2/9,7/31,6/92,1 % per kurva · grind < −240 dBFS · tak exakt 1,0 |
| DDP-export | 5 s → 375 sektorer / 882 000 byte · `DDP 2.00`-magic · ISRC i PQDESCR |

## 2. Sedan (M, kräver ett lager)

| # | Post | Källa | Storlek |
| :--: | :--- | :--- | :---: |
| 6 | ✅ **LV2-hosting** (egen `libloading`-host; `livi` krävde lilv vid bygge) | REAPER | **M** |
| 7 | ✅ **Rubber Band** som warp-läge (dlopen, återfall till WSOLA) | REAPER | **M** | 1,5× → 6 615 framer, 440 → 440 Hz, +1 oktav → 880 Hz |
| 8 | ✅ **Hybrid Reverb** (FDN + konvolution) | Ableton | **M** | RT60 0,81/1,51/3,01 s mot 0,80/1,50/3,00 · densitet 62 % |
| 9 | ✅ **JACK-backend** (`--features jack`, `SONIX_AUDIO=jack`) — 48 128 frames/47 cykler, RMS 0,354 | REAPER | **M** |
| 10 | ◐ **Kontrollerytor**: MCU + **BCF2000** + **FaderPort** klara (rena parsers, testade byte för byte). Kvar: BabyHUI, OSC-yta, LED/display tillbaka till enheten | båda | **M** |
| 11 | ✅ **VST3/VST2 genom sandboxen** (VST2 ej testad — ingen VST2-binär finns här) | REAPER | **M** |
| 12 | ✅ **Opus, AIFF och WavPack** avkodas i processen (WavPack: 0,000000 dB mot ffmpeg, bit-exakt) | REAPER | **S–M** |

## 3. Stort (L) — inte en ratt

| # | Post | Källa | Storlek | Varför |
| :--: | :--- | :--- | :---: | :--- |
| 13 | ✅ **Wavetable-motor** (mipmaps, morph, 7 unisonstämmor, filter) — källa via `SetInstrument` | Ableton | **L** | 14 kHz alias −110,5 dB mot −26,0 naivt · morph 0 → 0,3245 · 7 toppar |
| 14 | ✅ **Fysisk modellering** (`Modal`: 64 resonatorer + klubba) | Ableton | **L** | 220 Hz → 220,0 · B=0,05 flyttar partial 8 +104,9 % · T60 1,000 s |
| 15 | ◐ **Skript-API**: 28 funktioner i Rhai, Lua och Python (opt-in). REAPER har 732 `RPR_*` — ytan är L | REAPER | **L** |
| 16 | ◐ **Python** finns som opt-in skriptspråk (28 funktioner). Kvar: Abletons **Live Object Model** — 60 moduler, 2 260 `.pyc` | Ableton | **L** |
| 17 | **Max for Live-ekosystem** | Ableton | **L** | Vi har en graf, inte ett ekosystem |
| 18 | ◐ **Videomotor**: `frame_at` + cache + **realtidsuppspelning 25,0 fps** klart. Kvar: kodning, FX-kedja | REAPER | **L** |
| 19 | **Video-processor-DSL** (`//@param`, `//@gmem`) | REAPER | **L** | Kräver realtidsuppspelning först |
| 20 | **élastique Pro** | båda | **L** | Kommersiell licens, inte kod |
| 21 | **Disassemblera Abletons DSP per enhet** | Ableton | **L per enhet** | Enda vägen till exakta kurvor; klassnamnen (382 `O*Processor`) pekar ut dem |

## Stängt 2026-10-30

Omsampling i r8brain-klass (`audio::resample`, −75,6 dB) · kodekar i processen (symphonia) ·
anslagsmått i `--ai-features` · neural separation i alla paket (dlopen) · JSFX-instrument som källa ·
flikraden · `daw-bold`-temat i registret · TolerableTines-minnesfelet.

## Utanför listan

Hårdvara: Windows-porten (ingen laptop) och en riktig yabridge-kvittens. De väntar på maskin.

## Instrument — vägen till ett bibliotek

| Väg | Vad man får | Status |
| :--- | :--- | :--- |
| **Inbyggt** | wavetable, modal, syntens röster | klart |
| **JSFX** | REAPER:s instrument (EEL2) | klart |
| **SF2 / SFZ** | gratisbibliotek: Sonatina, VSCO2, Salamander | klart (SFZ round-robin ej cyklad) |
| **Yabridge + ni-wine** | Native Instruments: Kontakt 8, Massive X, Reaktor | scan + docs klara; installationen kräver `yay -S ni-wine` (Alex) |
| **LV2 / VST3 / CLAP** | Linux-plugins | klart |

