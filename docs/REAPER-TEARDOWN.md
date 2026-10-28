# REAPER 7.83 — vad som ligger i lådan, och vad Sonix kan ta

Nedplockat ur `~/Downloads/reaper783_linux_x86_64.tar.xz` (14 949 848 byte, hämtad 2026-10-28).
Allt nedan är mätt på artefakten, inte läst på reaper.fm.

## Artefakten

```
reaper_linux_x86_64/
├── REAPER/                    135 MB, det som installeras
│   ├── reaper                 12 881 232 B, ELF64 ET_EXEC x86-64, **stripped**
│   ├── libSwell.so              790 248 B — egna widgetbiblioteket (SWELL = Win32-API på Linux)
│   ├── Plugins/*.so          16 st: jsfx, flac, mp3dec, ogg, opus, wavpack, wave, cd, ddp,
│                              video, explorer, csurf, midi, elastique3, rubberband, soundtouch
│   ├── InstallData/Effects/  **265 filer, 80 586 rader läsbar DSP-källkod** (JSFX)
│   ├── InstallData/{Scripts,ColorThemes,KeyMaps,MIDINoteNames,Data}
│   ├── Resources/            2 107 png, 124 ico, 69 cur — teman och markörer
│   ├── whatsnew.txt          365 KB ändringslogg
│   └── Docs/REAPER Quick Start.pdf
├── install-reaper.sh
└── readme-linux.txt
```

ELF-layout (REA `inspect_binary_layout`, bevis `ev_130b4c0f…`): 30 sektioner, 9 segment, 345
dynamiska symboler, 252 relocationer, entry `0x41ac10`. Strippad — ingen funktionsnivå att läsa ur
binären utan disassembler, och det är inte där värdet sitter.

## Ändringsloggen han citerade — kontrollerad mot filerna

| Påstående | Var det syns | Utfall |
| :--- | :--- | :--- |
| `jsfx_has_set_midi_note_name`, `max_sliders`, `set_midi_note_name`, `slider_automate` | `Plugins/jsfx.so` (2/1/2/2 träffar) | **styrkt** — JSFX-motorn ligger i en egen `.so`, inte i huvudbinären |
| `$projectbase`, `$diskspace`, `$diskrectime` | `REAPER/reaper`, 1 träff var | styrkt |
| `Hidden FX`, `GainReduction_dB`, `ARRANGE_PROJECT_PINNED_H` | `REAPER/reaper`, 2 träffar var | styrkt |
| `D_FADEINDIR` / `D_FADEOUTDIR` | `REAPER/reaper`, 6 träffar | styrkt |
| `mouse_cap` (65536-biten för att dölja markören) | `REAPER/reaper`, 3 träffar | styrkt |

Not: `set_midi_note_name` finns **inte** i huvudbinären. Att söka där ger noll träffar och hade
kunnat läsas som "påståendet är falskt" — det är fel slutsats; motorn ligger i `jsfx.so`.

## Den läsbara skatten: 80 586 rader DSP

`InstallData/Effects` är inte binärt. Det är 265 textfiler i JSFX (REAPER:s eget sampelvisa
skriptspråk), sorterade per upphovsman, och **de flesta bär egen licens i filhuvudet**:

| Licens | Filer |
| :--- | ---: |
| LGPL | 47 |
| GPL (v2+/v3+/"Released under GPL") | 30+ |
| MIT (minst) | 3 |
| Public domain (Freeverb) | 1 |

Sonix är GPL-3.0-only, så LGPL (omblandningsbart till GPL), GPLv2+, GPLv3+ och MIT är alla
**porterbara**, med attribution kvar i filhuvudet. REAPER:s EULA förbjuder att distribuera *appen* —
inte att använda filer som har egen licens.

### Det som finns där och inte i Sonix

Sonix har redan Delay, EQ, Filter, Reverb, Vocoder, Gate, Compressor, Chorus, Saturation, Limiter,
Granular, De-Esser, Distortion och Phaser. Det här finns i REAPER-lådan men inte hos oss:

| Algoritm | Fil | Rader | Vad den gör som vi inte kan |
| :--- | :--- | ---: | :--- |
| **Spectral compressor** | `chmaha-fx/IntelligentSpectralCompressor.jsfx` | 1 159 | Kompression i STFT-domänen: frekvensberoende tröskel (center/slope/curve), nedåt- **och** uppåt-ratio med egen hi-freq-rolloff och knee, fönster 64–32 768 med 4–32× överlapp, tre sidokedjemodeller (pink noise / matching / compression), mix. Porterad från nih-plug (GPLv3+) |
| **Spektral penselmålning** | `filters/spectro_filter_paint` | 267 | Måla dämpning/förstärkning direkt på spektrogrammet — RX-liknande reparation med musen |
| **Bass manager** | `Liteon/bassmanager` | 354 | Mono under valfri frekvens + low-shelf, med fas- och nivåkontroll |
| **Inflator** | `chmaha-fx/Inflator.jsfx` | 598 | Uppåtkompression via udda ordningens vågformning (Oxford Inflator-idén) — mer "tryck" utan att toppa |
| **Maximizer (ZaMaximX2)** | `chmaha-fx/Maximizer.jsfx` | 466 | Lookahead-limiter med verklighetsnära gain staging; författaren har verifierat bit-identiskt mot brute-force |
| **Multibandskompressor** | `chmaha-fx/MultibandCompressor.jsfx` | 874 | 3–4 band med egna trösklar/attacker, plus splitter/joiner-paret i `loser/3Band*` (44–61 rader) som är återanvändbara byggstenar |
| **Gain reduction-scope** | `analysis/gain_reduction_scope` | 534 | Visar kompressionen över tid och frekvens — inte bara en siffra |
| **MIDI-humanisering** | `schwa/midi_humanizer`, `schwa/midi_modal_randomness` | 137 + 216 | Timing/velocity-humanisering och tonal slump inom en modell — passar AI-lagret |
| **Transient-/statistikaggregat** | `schwa/audio_statistics` | 343 | True peak, LUFS, crest, DC, fas — grunden för en render-rapport |
| **Klangfärgsfilter med 72 dB** | `Liteon/applefilter72db`, `butterworth24db`, `cheby24db` | 445 + 200 + 230 | Brantare filter än våra biquads |
| **De-esser i LR2** | `Liteon/deesser` | 553 | Bandpass/highpass-mål med Linkwitz-Riley, monitorlyssning |

## Vad Sonix kan ta, i ordning

1. **Spectral compressor.** Vi har `rustfft` och spektral infrastruktur redan; det här är det enda
   som ger en *ny sorts* kontroll (frekvensberoende dynamik) och den enda som gör en hörbar
   skillnad på en färdig mix. Licens: GPLv3+ → kompatibel.
2. **Spektral målning** (`spectro_filter_paint`). Interaktionsmodellen är hela poängen: måla på
   spektrogrammet, höra resultatet, ångra. Vi har `spectral_image` — det som saknas är pennan.
3. **Bass manager + Inflator.** Båda är små (354 + 598 rader), båda fyller ett hål (lågändes kontroll,
   "tryck" utan klippning), båda är billiga att portera.
4. **Splitter/joiner-paret i `loser`** som byggsten, sedan multibandskompressorn.
5. **Gain reduction-scope** som visualisering ovanpå vår kompressor — gör en befintlig effekt
   begriplig i stället för att lägga till en ny.
6. **MIDI-humanisering/modal slump** som AI-verktyg (vi har redan `write_script` och verktygsytan).

Sådant som är organisatoriskt och billigt, från hans lista: `$projectbase`/`$diskspace` som
export-wildcards, en "Hidden FX"-mapp i FX-bläddraren, `GainReduction_dB`-läsning för CLAP.

## Gränser

- **Ingen funktionsnivå-reversing gjord.** Binären är strippad (345 dynamiska symboler), och
  REAPER:s kärna är 12,9 MB C++ utan symboler. Att disassemblera den för att "se hur de gjort
  lanes" är inte vägen — beteendet syns i appen, och DSP:n ligger i klartext.
- **JSFX-koden är läst till struktur och parametrar, inte portad.** Innan en port: läs hela filen,
  behåll attributionshuvudet, och jämför mot originalet (flera är porter av x42/LGPL och nih-plug/GPL).
- **Tre tidssträckningsmotorer** (`elastique3.so` 1,7 MB, `rubberband.so`, `soundtouch.so`) är
  separata bibliotek med egna licenser; elastique är proprietary och får inte återanvändas. Sonix
  använder redan `signalsmith-stretch`.
- **80 586 rader är inte 80 586 rader värde.** Flera filer är varianter av samma sak (37 st i
  `sstillwell`, 36 i `loser`); tabellen ovan är urvalet, inte inventariet.
