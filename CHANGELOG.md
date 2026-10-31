# Changelog

All notable changes to Sonix Studio are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.230] - 2026-10-31

**VST3-modulens `ModuleEntry` anropas** — buggen som gjorde att **alla yabridge-plugins** kraschade
hosten: yabridge skapar sin brygga (och därmed fabriken) inuti `ModuleEntry`, och utan anropet föll
`GetPluginFactory` på `assert(bridge)` i `vst3-chainloader.cpp:156` och processen dog. Nu anropas
den före `InitDll`/`GetPluginFactory`, som VST3-specen säger.

**Mätt:** `Kontakt 8` laddar — `Native Instruments v8.14.1`, **4 145 parametrar** (2 065
automatiserbara); `Massive XL` — `v2.0.0 (R0)`, 2 105 parametrar, latens 32 ramar; `Supercharger`,
`Reaktor 6`, `Raum`, `Replika`, `Guitar Rig 7` startar alla. Före fixen: core dump på varje.

Provet `a_module_that_only_hands_out_a_factory_after_module_entry_still_loads` kompilerar
`tests/fixtures/mock_vst3.c`, som **vägrar lämna ut någon fabrik** förrän entry-anropet skett.
Bevisat med ctypes: utan `ModuleEntry` → `NULL`, efter → giltig pekare.

**Rättat på vägen:** `--features python` kompilerade inte — lokalen `ctx` i
`the_language_header_is_not_executed` skuggade hjälpfunktionen `ctx()`. Och video-FX-provet gör ett
försök till när en kodning faller under last (de andra video-proven kodar samtidigt).

## [0.9.229] - 2026-10-31

**SFZ round-robin cyklas** (0.9.229): `SampleZone.seq` bär `seq_position`/`seq_length`, och
`RoundRobin::pick` håller en räknare per `(kanal, ton)`. Mätt med 220/990/330/660 Hz-samples: sex
anslag på c4 → **220, 990, 220, 990, 220, 990**; en zon utan seq-data ger **220, 220, 220**; två
toner delar inte räknare (c4/e4 → 220, 990, 330, 660, 220, 330). Exporten använder samma väljare,
och `seq` följer med i projektfilen.

**LV2-state sparas i projektet**: `LV2_State_Interface` (`save`/`restore`), `Lv2Instance::save_state`
/ `restore_state`, och paret ligger i `SavedPluginData.lv2_state`. **Mätt:** gain 0,5 → spara →
sätt 1,0 → återställ → **0,500** (både parametern och det hörda ljudet); en plugin utan
state-interface ger tom vektor och ingen felkod; serde-rundturen bevarar `[(1, [0,0,0,63])]`.

**`.ins`-instrumentdefinitioner** (`audio::ins`): Cakewalks textformat ger externa syntars program
sina namn, med **🎛 Ladda .ins…** i MIDI-inställningarna. **Mätt:** två banker med 3 + 2 patchar
läses (`Bank A: [0] Piano, [7] Strings, [12] Soft Pad`, `Bank B: [0] Bass, [3] Organ`);
`program_name(0, 12)` → `Soft Pad`, `(0, 99)` → `None`; kommentarer hoppas över; en patchrad utan
bank ger `line 3: patch '12=Soft Pad' has no [Bank] section above it`.

**Video-FX-kedja** (`audio::video_fx`): `Brightness`, `Tint`, `Pixelate` i en kedja som körs på
samma ställe i uppspelning och export. **Mätt:** brightness 1,5 på mellangrått ger **128 → 192**;
tom kedja är **bit-identisk** (sha256 `964675f643d5f9e7` före och efter); pixelate 8 gör blocket
enhetligt; samma kedja ger **samma sha256** i uppspelnings- och exportvägen (`17625aa095f37fb2`);
en riktig export med brightness 0,0 avkodad tillbaka ger medelnivå **122,8 → 0,0**.

1 309 prov gröna, 0 varningar.

## [0.9.228] - 2026-10-31

**SFZ-import** (`audio::sfz`): region/group/global, notnamn eller siffror, relativa samplevägar,
loop-punkter, round-robin-zoner. Zonerna blir `SampleZone`s — samma som SF2-vägen — så
uppspelningen är oförändrad. Mätt: 3 regioner ger 3 zoner med rätt tangentintervall, grundtoner och
loop-punkter (`low.wav 36..59 root 60 loop (20,120)`, `mid.wav 60..71 root 64`, `high.wav 72..96`);
`c4→60`, `f#3→54`, `a#2→46`, `60→60`; ett saknat sampel fäller bara sin egen region.

**Instrumentbibliotek** (`audio::instrument_library` + panel i kanalracken): `~/.local/share/sonix/instruments/`
skannas efter `.sf2`, `.sfz` och JSFX, plus de två inbyggda källorna (`builtin:wavetable`,
`builtin:modal`). Ett klick sätter källan eller lägger zonerna i kanalens keymap; plugins listas men
laddas från plugin-fliken (de kräver en instans per spår).

**yabridgectl:s config läses** (`e8dc136`): sökvägarna till bryggade Windows-plugins är inte längre
gissningar, och `~/.wine-ni` — där `ni-wine` installerar Native Access och Kontakt — skannas.
Mätt: två `plugin_dirs` ger exakt två sökvägar, tom config ger inga. `docs/YABRIDGE.md` beskriver
hela vägen från `yay -S ni-wine` till ett spelande plugin.

1 296 prov gröna, 0 varningar.

## [0.9.227] - 2026-10-31

**Python 3 som tredje skriptspråk** (`--features python`, opt-in): samma 28 funktioner som Rhai och
Lua, samma `Ctx`, och `print` går till skriptloggen. Standardbygget länkar fortfarande ingen
libpython (`ldd | grep -c python` = **0**); med featuren syns `libpython3.14.so.1.0`.

Mätt: samma sångskript i Python och Lua ger **identisk `Ctx`**; `SyntaxError at line 2: expected ':'`
och `ZeroDivisionError at line 2: division by zero` bär typ och rad; `while True: pass` stoppas efter
**1,02 s** med `project script exceeded its 1 s budget`. Alla funktioner anropade en gång ger 19
loggrader utan fel.

**Rättat: rubrikraden kördes som kod.** `--!python` är ingen giltig Python-kommentar, så varje
skript med rubriken hade fallit — raden tas nu bort innan motorn ser den, och ett prov håller fast
det (`the_language_header_is_not_executed`).

1 285 prov gröna (standard), 0 varningar.

## [0.9.226] - 2026-10-31

**WavPack avkodas i processen** (`audio::wavpack`, dlopen mot `libwavpack`). Mätt: 44 100 Hz,
**88 200 frames**, dominant 440,0 Hz, och mot ffmpeg:s egen avkodning **0,000000 dB** i RMS med
**0,000000000** som största sampelskillnad — förlustfritt är förlustfritt, även vid 24 bitar.
`load_audio_pcm` ger nu filens egen frekvens (48 kHz-fixture blev 48 000 frames, inte ffmpeg-vägens
44,1). `docs/WAVPACK.md` säger hur biblioteket installeras.

**Videokodning** (`audio::video_export`, `--export-video <klipp|bild> <ljud> <ut.mp4> [fps]`): en
ffmpeg-process, `push_frame`/`finish`, bound på 60 s med kill, halvfärdig fil städas. Mätt på egen
körning: 2 s video + 2 s ljud → **50 frames, 25/1 fps, 320×240**, varaktigheter 2,000/2,000/2,000 s,
fem röda bilder ger R 253 / G 0 / B 0. Fel storlek avvisas: `frame is 6144 bytes, expected 9216`.

**Skript-APIt utökat**: tracks, volym, mute, klipplängd, noter (`note_at`, `add_note`,
`clear_notes`) och `log` — identiskt i Rhai och Lua. Mätt: samma sångskript i båda språken ger
**byte-identisk `Ctx`**; index utanför intervallet ger `track 99 is outside 0..2` i båda; 8 noter på
2 klipp tar **0,98 ms** (Rhai) och **0,10 ms** (Lua). `docs/SCRIPTING.md` listar hela ytan.

1 284 prov gröna, 0 varningar.

## [0.9.225] - 2026-10-30

**JACK-backend** bakom `--features jack`. DSP:n bröts ut ur cpal-callbacken till en `Renderer` som
båda backends kallar — ingen duplicerad kedja — och blocklängden får variera per cykel.
Standardbygget rör inte libjack alls (`ldd | grep -c jack` = **0**); med featuren syns
`libjack.so.0`. Väljs med `SONIX_AUDIO=jack`, annars cpal, och en rad i loggen säger vilket.
Mätt mot PipeWire:s JACK-server: **48 128 frames i 47 cykler** vid 48 kHz, **RMS 0,354**.

**Realtidsvideo.** `Playback` håller **en** ffmpeg-process med rawvideo-rör och en 6-bilders ring på
en egen tråd; `latest()` blockerar aldrig. Mätt på 25 fps-klippet: **50 bilder på 2,00 s = 25,0 fps**,
latens **median 0,43 ms** (max 1,03), 1 000 `latest()`-anrop på **0,018 ms**, sök 2,5 s ger
**20,83 nivåers** bildskillnad och spelet fortsätter, `stop()` tar död på barnet (`/proc/<pid>` borta).

1 268 prov gröna (standard), 0 varningar.

## [0.9.224] - 2026-10-30

**AIFF/AIFC avkodas i processen** (`audio::aiff`, ren Rust): COMM med 80-bitars extended
samplingsfrekvens för hand, SSND, 8/16/24/32 bitar samt AIFC `NONE`, `sowt`, `fl32`. Mätt:
44 100 Hz och **88 200 frames** för en 2 s-fil, dominant **440,0 Hz**; 24-bit mot 16-bit skiljer
**0,000 dB** i RMS; `fl32` når **0,999756** utan klipp; 80-bitarsläsaren ger 44 100/48 000/96 000
exakt. Trasig SSND: `chunk 'SSND' claims 88208 bytes but only 44081 remain in the file`.

**Kontrollerytorna svarar nu tillbaka.** `encode_feedback` skriver de byteströmmar ytorna förväntar
sig — BCF2000-display via sysex + ring-LED:er, FaderPort LED/segment, BabyHUI-reglage — och skickas
ut på den MIDI-port användaren öppnat, dedupat mot senaste tillståndet. **BabyHUI** tillagd i både
parser och encoder. Round trip mätt för alla fyra ytorna: BCF ger tillbaka
`DisplayText{"PLAY REC 120.0 BPM\nTRACK 3 100"}` + 8 reglage.

**Videomotor** (`audio::video_engine`): `frame_at(t)` dekoderar en bild på begäran med 8-bilders
LRU-cache. Mätt på en 3 s 320×240 25 fps-klipp: **230 400 byte** per bild, cacheträff
**0,086 ms mot 64,660 ms** (754×), baklängessök 2,5 → 0,5 → 2,5 ger **0,00 nivåers** skillnad,
20 slumpvisa sök ger 20/20 hela bilder (60–100 ms per dekodering).

1 258 prov gröna, 0 varningar.

## [0.9.223] - 2026-10-30

**Opus avkodas i processen.** `audio::opus` läser Ogg-sidorna själv (OpusHead, pre-skip, granule)
och anropar `libopus` via `dlopen` — `ffmpeg` behövs inte längre för `.opus`/`.ogg`/`.oga`.
Korsvisat mot ffmpeg på samma fil: **96 000 frames, 2,0000 s, 440,0 Hz, RMS 0,088384 — skillnad
0,000 dB**. Stereo: RMS 0,062477 i båda kanalerna. Skräpfil ger `Err("expected an Ogg page at byte
0, found [74, 68, 69, 73]")`.

**Två kontrollerytor till**: Behringer **BCF2000** (reglage på CC 0–7, knappar på note on, LC-display
via sysex) och **PreSonus FaderPort** (reglage på CC 0, transport på CC 16–23 → Stop/Play/Record/
Rewind/FF/Loop/Punch/User). Båda är **rena parsers** (`feed(&[u8]) -> Vec<SurfaceAction>`) som
skickar in i samma `ControlEvent`-kö som MCU:n redan använder — inget andra väg in i appen.
Testat byte för byte: fragmenterad sysex mitt i meddelandet ger exakt en `DisplayText` när sista
byten kommer, 4 KiB skräp ger inga handlingar, och nästa CC parsa ändå.

1 243 prov gröna, 0 varningar.

## [0.9.222] - 2026-10-30

**Fysisk modellering som källa** (`Modal`): upp till 64 modala resonatorer, olinjäritet
`f_n = f0·n·√(1+B·n²)`, materialkurva för avklingning och en klubba vars hårdhet styr pulsens
längd (1–20 ms). Mätt: starkaste partial vid 220 Hz-stöt **220,0 Hz**; partial 8 flyttas
**1760 → 3607 Hz** med B = 0,05 (+104,9 %); T60 1,00 s mätt **1,000 s**, material 1,0 ger partial 8
**0,533 s**; hård klubba bär **0,143 %** över 2 kHz mot **0,001 %** för mjuk; fem stötar toppar 0,277.

**VST3 och VST2 genom sandboxen.** Rutten fanns bara för CLAP, och `.so` inuti ett `.vst3`-paket
tolkades som VST2. Nu rapporterar barnet sin backend, och ett misslyckande faller tillbaka
in-process med orsaken i loggen i stället för tystnad. Mätt mot Surge XT Effects VST3:
backend **`VST3 (sandbox)`**, 4 800 frames, **topp 0,437446 / RMS 0,181184**; skräpsökväg faller
tillbaka på **12,3 ms**.

**Rättat: wavetablen kunde krascha ljudtråden.** `WavetableSource::process_stereo` skar båda
kanalerna ur den vänstra bufferten (`split_at_mut`), så varje block med 512 frames gav
`range end index 512 out of range for slice of length 0`. Felet gick igenom därför att provet jag
lade till hamnade **inuti** en annan testfunktion och därför aldrig kördes. Pro vet flyttat ut,
kör nu, och mäter **topp 0,213** vid C4.

1 231 prov (standard) och 1 308 (plugin-host) gröna.

## [0.9.221] - 2026-10-30

**Wavetable-motor.** `audio::wavetable`: 2 048-samplers ramar, bandbegränsade mipmaps per oktav
(FFT-trunkerade, inte medelvärden), tabellmorph mellan ramar, upp till 7 unisonstämmor med
`cents = spridning · 25 · (2i/(n−1) − 1)`. Tre tabeller genereras i kod — inga binärfiler i repot.

Nås som **källa** genom samma `SetInstrument` som ett JSFX-instrument (knappen *🧬 Wavetable som
källa*), så den spelas från keyboard, MIDI och klipp och går genom mastern som allt annat.

Mätt: aliasgolv på 14 kHz **−110,5 dB** mot **−26,0 dB** för en naiv enkel-mip; morph höjer
H2..H8-andelen 0,0000 → 0,3245; 7 stämmor ger **7** toppar inom ±2 % av 1 kHz (1 stämma ger 1);
10 s vid 220 Hz toppar 1,366 utan NaN.

1 223 prov gröna, 0 varningar.

## [0.9.220] - 2026-10-30

**Rubber Band som warp-läge.** Nytt läge `Rubber Band` i klippets warp-meny: R3-motorn lånas in
med `dlopen` när `librubberband.so` finns, annars faller läget tillbaka på `Complex` (WSOLA) — ett
projekt som valt läget öppnas alltså och låter som förut på en maskin utan biblioteket.

Mätt (egen körning): 1,5× på 4 410 sampel → **6 615 framer**, dominant frekvens **440,0 Hz** efter
sträckningen (tonhöjden står kvar), +1 oktav ger **880,0 Hz**, formantbevarande skiljer **0,603** vid
pitch 2,0 men är bit-identiskt vid pitch 1,0 (inget att bevara utan tonhöjdsändring).

**Hybrid Reverb** (`Hybrid Reverb` i effektlistan): 4 allpass-diffusorer → 8-linjers FDN med
Hadamard-mix och dämpning i loopen, plus konvolutionshalva med 250 ms syntetisk IR. Mätt: RT60
0,80 → **0,81 s**, 1,50 → **1,51**, 3,00 → **3,01**; densitet **62,1 %** (algoritmisk) och **56,1 %**
(konvolution) av samplen över −40 dBFS i första 100 ms; peak 3,099 vid 0,9 in och 12 s decay; mix 0
är bit-identisk med torrt, mix 1 korrelerar **−0,002** med insignalen.

1 218 prov gröna, 0 varningar.

## [0.9.219] - 2026-10-30

**LV2-hostning.** `.lv2`-sökvägar avvisades förut; nu skannas bundles, listas och bearbetas
(`audio::lv2`). Egen host på `libloading` + `LV2_Descriptor` i stället för `livi`: lilv kräver
`lilv-0`/`serd-0` via pkg-config både vid bygge och körning, och CI/paketet har dem inte.

Mätt mot ett egenkompilerat testplugin (maskinen har bara LV2-spec-bundles, inga riktiga plugins):
enhetsgain **1,000000**, `gain=0,5` ger **0,500000**, efter `reset()` 1,000000 igen.

**MIDI-effekterna klara:** notlängd, eko och slumpat anslag på zonkartan. Mätt: 250 ms vid
48 kHz → exakt 12 000 sampel (44,1 kHz → 11 025, 333 ms → 14 685); tre pass med 100 ms eko ger
två extra triggers på 4 800 och 9 600 sampel med gain 0,6 och 0,36; slump 0,5 över 200 anslag på
velocity 64 sprider sig över hela 1..127.

1 206 prov (standard) och 1 263 (plugin-host) gröna.

## [0.9.218] - 2026-10-30

**Ackordläge på MIDI-ingången.** Tangentbordet kan spela ackord: åtta lägen (oktav, kvint, power,
dur, moll, septima, sus4) ovanpå zonkartan, som redan har transponering, skalstämning och
anslagskurva — alltså resten av Abletons MIDI-effekter, som visade sig redan finnas.

Släppet räknar fram samma intervall ur samma not, så ingen tabell över hållna noter behövs
(`KeyMap::chord_notes` är deterministisk). Mätt i prov: moll från C4 ger `60 63 67`, samma vid
släpp, yttertangenten 127 ger bara sig själv, trasigt lägesindex ger dur i stället för panik.
1 200 prov gröna.

## [0.9.217] - 2026-10-30

Tre poster ur backloggen, alla med sin mätning.

**Cytomic-filtren** (`audio::filter_topologies`): Sallen-Key MS2 med LP/BP/HP och drive i
resonansåterkopplingen, SVF med 2× oversampling, ladder med poldiffusion. Nås som effekten
`Cytomic-filter` i pluginlistan. Mätt: SK −12,5 dB/okt, SVF −12,3, ladder −24,0; resonanstopp
+26,2 dB vid res 0,9; självsvängning håller 0,75 rms utan insignal; THD vid 200 Hz går från
0,02 % till **5,27 %** när drivet slås på (ladder 0,05 → 4,00 %).

**Multibandssaturator** (`audio::master_fx::roar`, effekt `Roar`): tre band (LR2, 200 Hz/2 kHz),
sex kurvor, aterkoppling med grind, 1–3 steg, tak vid 1,0. Mätt: bandsumman avviker högst
**0,122 dB**; THD per kurva 5,2 % (tanh), 9,7 % (vik), 31,6 % (klipp), 92,1 % (wrap); grinden
håller tystnad under −240 dBFS med aterkoppling 0,95. Kurvorna är våra egna — Abletons är inte
läsbara ur binären.

**DDP-export** (`audio::ddp`, `--export-ddp`, `--export-cue`): DDPID/DDPMap/PQDESCR/CDTEXT +
AUDIO.DAT i 2352-byte sektorer, ISRC valideras, icke-44,1 kHz avvisas. Mätt: 5 s → 375 sektorer,
882 000 byte, `DDP 2.00`-magic, ISRC i PQDESCR.

1 199 prov gröna, 0 varningar.

## [0.9.216] - 2026-10-30

**Komprimerat ljud avkodas i processen.** Import gick förut alltid via det externa `ffmpeg`-programmet
för allt som inte var WAV — saknades binären kunde inget komprimerat öppnas alls. Nu läser
**symphonia** (ren Rust) FLAC, MP3, Vorbis, WAV och MP4/AAC direkt i processen, med `ffmpeg` kvar som
reserv för det som återstår (Opus, WavPack, AIFF).

Prov: en FLAC skrivs med vår egen encoder och läses genom `load_audio_pcm` — nivå och
samplingsfrekvens bevaras, utan att `ffmpeg` är inblandat. 1 187 prov gröna.

## [0.9.215] - 2026-10-30

**Omsampling med fönstrat sinc i stället för linjär interpolation.** Linjär interpolation är ett
tvåpunkters lågpass: den dämpar diskanten olika mycket beroende på var i intervallet sampeln hamnar
och läcker vikningsdistorsion nedåt. Stämsepareringen drabbades värst — modellerna arbetar i 48 kHz
medan filen ofta är 44,1 kHz, alltså **två** omsamplingar per separation.

**Mätt** (6 kHz-ton, 44,1 → 48 → 44,1 kHz, felet mot originalet): **−75,6 dB** med sinc mot
**−27,8 dB** med linjär — 48 dB bättre. Provet kräver att sinc ligger på minst halva felet.
Separationen tar fortfarande 3,4 s på fem sekunder (omsamplingen är en bråkdel av tiden; 32 taggar
per sampel körs en gång per fil, inte i realtid).

Modulen är `src/audio/resample.rs`; `resample_linear` finns kvar för de ställen där interpoleringen
är en detalj (fx-racket), och de två jämförs i provet så skillnaden är mätt och inte påstådd.

## [0.9.213] - 2026-10-29

**ONNX Runtime laddas vid körning — neural separation i alla paket.** Ett länkat ORT kräver glibc
≥ 2.32, så manylinux-paketet (golv 2.28) kunde aldrig bära separationen. Nu öppnas
`libonnxruntime.so` med `dlopen`: binären startar utan den, manylinux-bygget har `neural` på, och
den som vill separera lägger filen i `~/.local/share/sonix/models/`, bredvid binären eller sätter
`ORT_DYLIB_PATH`. Saknas den blir beskedet en sökväg i stället för en panik.

**Versionen måste vara 1.23.x.** Mätt: 1.22.0 laddar och separerar (`neural: true`) men kraschar vid
avslut med "double free or corruption"; 1.23.0 går rent, samma fil och samma modell. `-neural`-tarballen
bär nu `libonnxruntime.so` bredvid binären så den fungerar direkt.

**Ett JSFX-instrument går genom hela mastern.** Det returnerade tidigare direkt efter volym och
`tanh`, så gate, remix-FX, tape stop, control room och talkback hoppades över — instrumentet lät
annorlunda än rösterna beroende på vad som var kopplat. Slutsteget ligger nu i `finish_master`,
delat av båda vägarna, och instrumentet passerar dessutom masterns FX-kedja (EQ, kompressor,
de-esser, limiter) som rösterna gör.

## [0.9.212] - 2026-10-29

**Flikar som flikar, nivåer som syns.** Kategorierna (Skapa / Arrangera / Mixa / Verktyg) och deras
underverktyg låg i samma wrapped-rad, så man inte kunde se vilken nivå man var på. Nu: transport,
kategorier och underverktyg på tre rader, och fliklisten ritar en riktig list — bakgrund över hela
bredden, baslinje, flikar med rak underkant som vilar på den, och den valda fliken öppnar baslinjen
och bär accentlinjen. Provet mäter geometrin (listen > 200 px, alla flikars underkant på baslinjen).
MCU flyttad från transporten till statusraden — den är hårdvara, inte ett verktyg.

**`--ai-features`**: spektralt tak, energi över taket, brusgolv (p10), RMS-spridning och sann crest.
Fyra siffror om varför en detektor säger "AI", utan omdöme. `docs/AI-FEATURES.md`.

**UI-förslaget (Vercel/v0)**: färgerna är tagna i `daw-bold`-temat, kostnaden för resten är räknad
i `docs/UI-PROPOSAL.md`.

**`--ai-features` mäter det en detektor tittar på.** Spektralt tak, energi över taket, brusgolv
(p10), korttids-RMS-spridning och sann crest — fyra siffror, inget omdöme. Se `docs/AI-FEATURES.md`.

**Flikraderna syns som två nivåer.** Ovalda flikar har ingen yta längre (bara den valda bär färg),
och en avdelare med indrag skiljer kategorira­den från underverktygen — förut flöt de ihop och
flikarna såg ut att sväva.

**Exporten blir hela låten, inte 32 takter.** Längden kom från `loop_end_bar` klippt till 4–32
takter, så en låt med långa stämmor blev en minut i 128 bpm och resten tyst. Nu används samma mått
som tidslinjen ritas med (`get_max_project_bars`), och ett prov håller fast att ljud förbi takt 32
renderas.

**Metronomen sitter i den globala raden**, bredvid ▶/⏸/⏹, inte bara i arrangörens verktygsrad — en
påslagen klick gick inte att stänga av från de andra sidorna. Ett reglage, samma `metronome_enabled`.

Gränsen för JSFX-instrumentet står nu i knappens hover: det är en rå källa, syntens filter/ADSR/
effekter används inte.

**Instrumentet ligger på syntens masternivå.** Det gick tidigare rakt ut oanpå `master_volume`, så
volymen hoppade när man kopplade in en JSFX. Nu går det genom samma gain och `tanh` som rösterna.
Kvar utanför: master-FX-svansen (gate, DJ-FX) — den ligger efter rösternas slut och nås inte från
instrumentgrenen.

## [0.9.211] - 2026-10-29

**Ett JSFX-instrument kan vara syntens källa.** `AudioCommand::SetInstrument { path }` sätter en JSFX
som spelar i stället för syntens röster — det är den enda plats där noter och en JSFX finns
samtidigt, så keyboard, MIDI-in och klip spelar den utan att någon av de 60 notavsändarna rörs.
Blocket renderas på begäran (512 frames) och läses en frame i taget; noter gäller från nästa block,
som i REAPER.

I pluginlistan får en JSFX-rad knappen **🎹 JSFX** som kopplar in den som instrument. Prov:
`a_jsfx_instrument_can_be_the_synth_source` — tyst utan not, hörs efter `NoteOn`, tystnar på
`NoteOff`.

*Känd gräns*: instrumentet är rå källa — syntens filter, ADSR och effekter hoppas över.

## [0.9.210] - 2026-10-29

**Noter kan nå ett JSFX-instrument i kedjan.** `PluginProcessor` har fått `accepts_notes`/`note_on`/
`note_off` (tomma standardmetoder, så CLAP och VST är orörda), och `JsfxProcessor` köar noterna och
skickar in dem i motorn i nästa block — noten klingar från blockets början, aldrig mitt i.

Provet är ett instrument som bara låter när det får en not (`midirecv`): tyst → not → hörs →
not av → tyst. Det avslöjade ett riktigt motorfel: MIDI-läskurvan nollställdes inte per block, så
`midirecv` läste förbi en tom kö och **panikade** i andra blocket efter noter. Kurvan klampas nu och
hoppar till träffen i stället för ett steg.

## [0.9.209] - 2026-10-29

**Variablerna flyttade ovanför tabellerna — ett instrument spelade 2 048 röster.** EEL2 delar ett
enda minne mellan variabler och tabeller, och REAPERs egna instrument lägger tabeller på låga
adresser (`acnt = 0; aamp = 2048; ph = 4096; mt = 6144;`). Våra variabler låg på plats 64 och framåt,
så `acnt[i] = 0` skrev över dem — och i `TolerableTines` blev följden att alla 2 048 platser i
notmatrisen såg satta ut: **2 048 samtidiga röster, 1 947 RMS**. Variablerna bor nu från 2 MiB, och
`alloc` delar ut ovanför dem.

Uppmätt före/efter på samma ton (not 60): topp 44 100 → **0,060**, rms 1 947 → **0,024**. Alla 256
JSFX-filer tolkas fortfarande.

## [0.9.208] - 2026-10-28

**Ett släpp som kan separera.** Release-jobbet byggde utan `neural` (ORT går inte att länka mot
glibc 2.28), så varje nedladdad binär var DSP-only. Nu byggs **två** släpp av samma commit:

- `sonix-<ver>-<ark>-unknown-linux-gnu.tar.gz` (manylinux, glibc 2.28) — som förut, DSP-separation.
- `sonix-<ver>-x86_64-neural.tar.gz` (ubuntu-24.04) — `neural` är standard, alla fem modellerna
  fungerar, högre glibc-krav. Jobbet vägrar publicera om binären inte innehåller ONNX Runtime.


**Teman når hela ritytan.** Elva overlegg och markeringar låg hårdkodade som vita/svarta
alpha-färger och namngivna accenter (loopregion, puck-glow, trimhandtag, kompband, live-region,
mätare, Edison-panel). De går nu via paletten med samma alfa, så ett ljust tema inte får
vitt-på-vitt. Kvar utanför paletten: två **beräknade** färger (`patcher_view.rs:269`,
`vocal_studio_view.rs:1068`) som räknar fram en nyans ur ett index — de är gradienter, inte teman.

## [0.9.207] - 2026-10-28

**Neural separation är på som standard.** Att den var opt-in (`--features neural`) gjorde att en
vanlig `cargo build --release` gav en binär där modellväljaren tyst föll tillbaka på DSP — man valde
en modell i listan och fick något annat, med "ingen neural modell hittades" som förklaring trots att
filerna låg där. Nu är `neural` en standardfeature:

- `cargo build --release` → alla fem modellerna fungerar.
- `cargo build --release --no-default-features --features plugin-host` → beroendefritt bygge utan
  ONNX Runtime (säger "Byggd utan neural-stöd", inte "ingen modell hittades").
- Release-jobbet bygger med `--no-default-features` eftersom ORTs färdiga bibliotek kräver glibc
  2.32/2.38 och vårt golv är 2.28.

## [0.9.206] - 2026-10-28

**Teman går att skriva för hand, och listan över nycklar finns.** Motorn fanns (JSON-fil per tema,
registret på GitHub med sha256, spara/installera/välja i menyn) men ingenting berättade vilka
nycklar som finns — den enda listan en temaskapare har var `theme.rs`. Nu:

- **`sonix --theme-palette [light]`** skriver hela paletten som JSON med hex-färger.
- **Hex i temafiler**: `"bg": "#101418"` läses som `[16,20,24,255]`, så en fil som skrivs för hand
  inte behöver räkna byte. Arrayer fungerar som förut, och gamla teman är orörda.
- **`docs/THEMES.md`**: formatet, alla 29 nycklar med vad de styr, vad ett tema bör klara av
  kontrast, hur hashen räknas och hur ett tema skickas till registret.
- Ett prov läser `docs/THEMES.md` och kräver att **varje nyckel i `Palette` står där** — en ny färg
  utan dokumentation faller i CI.

## [0.9.205] - 2026-10-28

**Släppta byggen kan köra stämsepareringen.** Release-workflowen byggde bara med
`--features plugin-host`, så katalogens fem ONNX-modeller syntes i listan men kunde aldrig köras —
och felet förklarades med "Ingen neural modell hittades", som om filerna saknades. Nu byggs släppta
binärer med `--features plugin-host,neural`.

- **`sonix --separate <in.wav> <ut-mapp> [modell-id]`**: kör en modell över en fil, skriver en wav
  per stämma och rapporterar modellfilens sökväg, tid, `neural: true/false` och rms per stämma.
- **Ärlig orsak till fallback**: en build utan `neural` säger "Byggd utan neural-stöd" i stället för
  att påstå att modellen saknas (7 språk).
- Mätt på 4 s stereomix: `scnet` 3,1 s, `htdemucs` 12,2 s, `htdemucs_6s` 12,2 s, `mel_roformer`
  20,8 s, `bs_roformer_sw6` 30,7 s — alla med `neural: true`.
- **Släppta binärer kan inte bära `neural`.** ONNX Runtimes färdiga bibliotek är byggda mot nyare
  glibc (`__libc_single_threaded`, `__isoc23_strtol`) och länkningen faller på "undefined symbol"
  mot vårt 2.28-golv. Release-jobben bygger därför som förut, och en binär med `neural` byggs
  lokalt: `cargo build --release --features neural`. Nästa steg om släppen ska kunna separera är
  `ort`s `load-dynamic` (dlopen vid körning) i stället för länkning vid bygge.

## [0.9.202] - 2026-10-28

**Markera alla regioner** i spårrubrikens meny: på spåret eller i hela projektet. Samma
markering som klick och ram ger, så Delete, Ctrl+C, razor och nudge arbetar på den direkt.
Nya i18n-nycklar i alla sex tabellerna.

## [0.9.201] - 2026-10-28

**Spårdragningen startar bara i namnytan.** Mute-, VU- och detaljknapparna sitter till höger
i rubriken; ett drag som började på dem flyttade spåret i stället för att göra det man pekade
på. Nu krävs att trycket ligger i rubrikens vänstra del.

Verifierat i fönster: appen startades på en egen Xvfb-skärm och drevs med syntetiska
mushändelser (XTEST) — spårdragningen flyttade "Trummor & Beat" från plats 1 till 3, och
klick + Ctrl+klick gav två cyanmarkerade spår med rätt primär. Ångra tog tillbaka både
klippen och spårflytten.

## [0.9.200] - 2026-10-28

**Nudga med piltangenterna.** `←`/`→` flyttar hela markeringen ett snäppsteg, `Shift` en
hel takt. Steget följer snäppläget — 0,01 s räknas om via taktens längd, 1/16, beat och takt
är redan i takter (`nudge_step_bars`, eget prov). Ett ångra-steg per nudge, och pianorullen
behåller sina egna pilar.

## [0.9.199] - 2026-10-28

**Mapp-routing i ett svep.** `Ctrl+Alt+B` skickar de markerade spåren till den första lediga
bussen, så de får en gemensam fader i mixern; `Ctrl+Alt+Shift+B` tar dem tillbaka till
standardbussen. Valet finns också i spårrubrikens meny. Buss 3 (standardvägen) räknas aldrig
som ledig, och när alla bussar används säger statusraden det i stället för att gissa.
Bussvalet sparas i projektet som förut.

## [0.9.198] - 2026-10-28

**Spårordning med musen.** Dra i spårrubriken och en orange infogningslinje visar var spåret
hamnar; släpp flyttar det dit. Ett klick utan rörelse markerar fortfarande som förut, och
`move_selected_track` är nu ett tunt lager över `move_track(from, to)` så meny, tangentbord
och drag använder samma regel.

## [0.9.197] - 2026-10-28

**Spår kan flyttas i listan.** `Ctrl+Shift+↑/↓` eller menyvalet i spårrubriken; motorn
synkas om efter flytten och markeringen följer med, så nästa kommando träffar samma spår
som före. Regeln för indexflytten är en egen funktion med prov (flytt åt båda hållen).

## [0.9.196] - 2026-10-28

**Dra regioner mellan spår.** Dragningen följer spåret pekaren står över och flyttar
regionerna dit vid släpp — hela klungan behåller sitt inbördes spåravstånd, och en del som
skulle hamna utanför projektet läggs tillbaka i sitt eget spår i stället för att försvinna.
Bara Move-läget byter spår; trimning rör bara kanterna.

## [0.9.195] - 2026-10-28

**Tidsmarkeringen syns i verktygsraden**, med sina kommandon i verktygstipset — annars är
razor en hemlighet för den som läst koden. `docs/REAPER-WORKFLOW-GAPS.md` har fått en
snabbkommandoöversikt för hela flödet.

## [0.9.194] - 2026-10-28

**S, R och L arbetar också på hela markeringen.**

- `S`/`B`/`C` delar varje markerad region som spelhuvudet står inom; regioner som inte
  når spelhuvudet lämnas orörda i stället för att kapas på måfå.
- `R` vänder hela markeringen, `L` loopar hela markeringen.
- Delningen använder samma beskurna kopia som razor-dupliceringen, så sampling-offset och
  vågform följer med på båda halvorna.

## [0.9.193] - 2026-10-28

**Delete, M och Ctrl+D arbetar på hela markeringen.** De tre vanligaste kommandona tog
fortfarande bara den primära regionen — markeringen syntes men hördes inte.

- `Delete` tar bort alla markerade regioner (bakifrån, så indexen håller) och stänger
  gapen enligt ripple-läget.
- `M` mutar hela markeringen i samma riktning som den primära.
- `Ctrl+D` duplicerar varje markerad region direkt efter sig.

## [0.9.192] - 2026-10-28

**Ramarkering över flera spår och razor-duplicera.**

- Ramen går nu över så många spår man drar: spåret under pekaren blir ramens andra kant,
  och släppet markerar alla regioner som överlappar intervallet i alla spår däremellan.
- `Ctrl+Alt+D` duplicerar tidsmarkeringen: klipper ut området, gör plats genom att skjuta
  allt efter det och lägger kopian direkt efter — sampling-offset och vågform följer med.
- Prov: en beskuren kopia täcker exakt området, och en region utanför kopieras inte.

## [0.9.191] - 2026-10-28

**Tidsmarkering från regeln.** Shift+drag i regeln markerar tid i stället för att flytta
spelhuvudet, och området syns som ett cyan band i regeln. Samma markering som rammarkeringen
lämnar, så `Ctrl+Shift+X` (razor) arbetar på båda.

## [0.9.190] - 2026-10-28

**Ramarkering, ripple-redigering och razor.** De tre sätten att arbeta på ett *område* i
stället för på en region i taget.

- **Ramarkering**: drag som börjar i tom yta markerar regionerna man drar över, ritar en
  cyan ram och lämnar intervallet som tidsmarkering. Gäller spåret man drar i.
- **Ripple** (knapp i verktygsraden, av → spår → alla): borttagning stänger gapet och
  inklistring skjuter undan det som ligger efter, på samma spår eller alla.
- **Razor** (`Ctrl+Shift+X`): tar bort tidsmarkeringen — regioner helt innanför tas bort,
  de som sticker in kapas, och en region som spänner över hela området delas i två med
  korrekt sampling-offset och beskuren vågformsbild. `Escape` rensar markeringen.
- Prov: intervallurvalet tar bara regioner som överlappar (och klarar omvänd dragning),
  och ripple-knappen cyklar av → spår → alla.

## [0.9.189] - 2026-10-28

**Objektgrupper, Alt+drag-kopia, klistra på ursprunglig plats och ångra-historik.**
Resten av kopiera/gruppera-flödet, det som REAPER gör utan att man tänker på det.

- **Objektgrupper** (`Ctrl+G` på regioner, `Ctrl+Shift+G` löser upp): region-id → grupp-id i
  `item_groups`, så en grupp kan sträcka sig över spår. Ett klick på en grupperad region
  markerar hela gruppen, och en dragning flyttar hela gruppen. Grupperna sparas i projektet.
- **Alt+drag kopierar** i stället för att flytta: klonen får nya id:n, ärver grupptillhörighet
  och blir den man drar i.
- **Ctrl+Shift+V klistrar på ursprunglig plats** — klungan kommer tillbaka där den kopierades,
  inte vid spelhuvudet.
- **Ctrl+Alt+Z öppnar ångra-historiken**: stegen listas nyast överst och ett klick hoppar
  tillbaka dit (`undo_to`), i stället för tio Ctrl+Z.
- Prov: markeringen expanderar till hela objektgruppen och lämnar ogrupperade regioner i fred.

## [0.9.188] - 2026-10-28

**Hela markeringen flyttar med.** Multi-markeringen från 0.9.187 hade en halv sanning:
man kunde markera fem regioner men bara den man drog i rörde sig.

- `RegionDragState::multi_initial` fångar de övriga markerade regionernas starttakter när
  dragningen börjar, och Move-grenen lägger samma förskjutning på dem — räknad från
  starten, inte kumulativt per bildruta.
- Markeringen uppdateras före `push_undo`, så klungan som följer med är den som gäller
  efter klicket.
- Vid släpp synkas alla spår som var med i dragningen, inte bara det man höll i.
- `docs/REAPER-WORKFLOW-GAPS.md` uppdaterad: multi-flytten är ✅, kvar är rammarkering,
  objektgrupper och mappar med routing.

## [0.9.187] - 2026-10-28

**Markera flera spår och regioner — och kopiera dem tillsammans.** Flödet saknade
grunden: allt utgick från ett spår och en region, så Ctrl+C kopierade alltid en enda sak.

- `selected_tracks` och `selected_regions` som mängder. Den primära posten ligger kvar i
  `selected_timeline_track` / `selected_audio_region`, så de 47 ställen som redan läser
  dem fortsätter att fungera.
- Klick: Ctrl/Cmd växlar, Shift tar spannet, vanligt klick börjar om. Markeringen kan
  aldrig bli tom — `clicked_selection` är provad för alla fyra fallen.
- Ctrl+C kopierar alla markerade regioner som en klunga med spår- och taktavstånd; Ctrl+V
  klistrar in den vid spelhuvudet på det primära spåret. Utan regionmarkering kopieras
  hela spår, som klistras in efter det sista markerade.
- Ctrl+A markerar alla spår (när inget textfält äger tangenten). Ctrl+G grupperar de
  markerade spåren, Ctrl+Shift+G löser upp.
- `prune_selection` en gång per bildruta: borttagna spår och regioner städas ur
  markeringen i stället för att bli spöken.
- Spårhuvudet visar hela markeringen: primärt spår i sin färg, övriga i cyan ram.
- Nytt: `docs/REAPER-WORKFLOW-GAPS.md` — hela flödet mot REAPER, med vad som är kvar.

## [0.9.186] - 2026-10-28

**Korsning vid spelhuvudet och alla grupper på en gång.** De två sista raderna i FL-kommandoytan
som gick att bygga utan ett nytt urvalslager.

- **✚ Korsa vid spelhuvudet**: bara skarven närmast spelhuvudet korsas, inte hela spåret.
  `create_crossfade_at(bar)` väljer det överlappande paret vars skarv ligger närmast och
  använder samma regel som `create_crossfades` (halva regionen som tak). Prov: med två skarvar
  rörs bara den vid spelhuvudet, och utan överlapp händer ingenting.
- **📁 Fäll ihop/upp alla**: `set_all_groups_collapsed` fäller alla grupper och räknar bara dem
  som faktiskt ändrades, så statusraden inte säger "3" när två redan stod rätt.
- Inventariet uppdaterat: `Create crossfade`-raden och gruppraden stämmer nu med koden.

## [0.9.185] - 2026-10-28

**Spågrupperna sparas i projektet.** De fanns bara i minnet: en grupp och dess ihopfällda
läge försvann vid varje sparning, och ingen fil kunde säga vilket spår som hörde till vilken
grupp. Inventariet hade det som ✗ — nu är det stängt.

- `SonixProjectData.track_groups` och `SavedTrackData.group`, båda `#[serde(default)]` så en
  fil från före fältet öppnas som ogrupperad i stället för att vägra.
- Laddningen sätter tillbaka både grupplistan och varje spårs grupp, och gruppens
  ihopfällda läge följer med.
- Prov: en grupp med `collapsed` överlever JSON-rundturen, och projektfilen bär den.

## [0.9.184] - 2026-10-28

**Exportarbetaren flyttad till `audio::exporter` — och prövad med en riktig fil.** Trådvägen
från 0.9.182 var bara kompilerad; nu finns `render_files` där renderingskoden hör hemma, med
`ExportFile`/`ExportOutcome`, och ett prov som renderar en takt, skriver en WAV och läser
storleken tillbaka. Samma kod som UI-tråden använder — alltså är det den vägen som prövas.

- Ett tomt jobb är ett giltigt jobb: inga filer, inget fel, ingen panik.
- Framstegen räknas i samma atomär som `poll_render_job` läser, så tråden och UI:t delar en
  sanning i stället för två.

## [0.9.183] - 2026-10-28

**Export av spågrupper som egna filer.** Det sista av FL-kommandoytan som handlade om export:
gruppbakningen fanns, men ingen väg att få ut varje grupp som en fil.

- Fjärde valet i exportens omfång: **Spågrupper – en fil per grupp**. Varje grupp får en fil
  med bara sina spår hörda, via samma `isolate_group` som gruppbakningen använder — gruppens
  spår solas och övriga spårs tidslinjeljud töms, så filen bär gruppen och inget annat.
- Tomma grupper hoppas över, och en grupp vars namn blir tomt i filnamnet får `GruppN`.
- Gruppfilerna går genom samma bakgrundstråd som resten av exporten.

## [0.9.182] - 2026-10-28

**Exporten renderar i bakgrunden.** FL:s `Background rendering` var det sista i kommandoytan
som handlade om arbetsflöde i stället för om en effekt: förut frös fönstret medan filerna
skrevs, nu kan man jobba vidare och statusraden säger till när det är klart.

- Specarna byggs på UI-tråden (de är ren data), sedan tar en tråd över: `render_files` kör
  samma `build_offline_engine` + `render_project_offline` som förut, hoppar över tomma stammar,
  normaliserar mastern och skriver filerna.
- `RenderJob` bär tråden, en atomär räknare och antalet filer; `poll_render_job` räknar om
  förloppet varje bildruta och hämtar resultatet när tråden är klar. Panik i tråden blir
  "avslutades oväntat" i statusraden i stället för en evig progressbar.
- Startknappen är avstängd medan en export kör, så två exporter inte kan skriva samma fil.
- Beskeden vid klart/fel är desamma som den blockerande exporten gav, inklusive
  normaliseringsraden (LUFS, mätt nivå, tak).

## [0.9.181] - 2026-10-28

**Tre MIDI-verktyg på tagningen: arpeggio, rullning och choke.** De bor där Sonix faktiskt har
notlängder — i tagningen — i stället för i en effekt som saknar MIDI-ingång.

- **🎹 Arpeggio**: noter på samma slag blir en följd. Tonerna cyklas i tonhöjdsordning, `octaves`
  låter följden klättra, och varje steg får vald längd. Riktningen är samma ratt som
  strumningen (Ner = högsta först) — en riktning att lära sig, inte två.
- **🔁 Rulla ×4**: varje not med känd längd upprepas fyra gånger inom sin egen längd, en
  stutter som stannar där frasen slutar. Okänd längd hoppas över.
- **🔗 Choke**: en not som klingar när nästa börjar kapas vid nästas start. Basslines och mono
  leads blir rena, och noter med okänd längd rörs inte — att korta en not vars slut vi inte
  känner är att gissa.
- **Mätt:** ett treklang på 4 steg med 0,25-steg blir 16 noter (60, 64, 67, 60 …), oktaver
  klättrar 60→72→76, rullning ger 4 noter på 0/1/2/3 med 0,9 längd, och choke kapar 2,0 → 1,0
  medan okänd längd står kvar. 1162 prov gröna.

## [0.9.180] - 2026-10-28

**Spektral målning: dämpa ett ljud genom att måla på dess spektrogram.** Verktyget ligger i
spårets verktygsrad och är den interaktion som REAPERs `spectro_filter_paint` är — utan att en
rad är lånad.

- `audio::spectral_edit`: STFT 1024/256 med Hann, spektrogram i dBFS, mask per (bildruta, bin)
  och `apply` som gör masken till ljud igen. Vyn är högst 1024 × 256 rutor även för långt
  ljud, och raderna är vända så hög frekvens ligger överst.
- `Mask::apply` lägger en framruta på N sampel: utan den är de första ~768 samplen för svaga,
  eftersom överlapp-adderingen bara är exakt där alla fyra fönstren bidrar.
- `ui::spectral_editor`: spektrogram som textur, målning som RGBA-lager, pensel (1–12 rutor)
  och dämpning 0…−60 dB. `Verkställ` byter spårets buffer och synkar stammarna — bara om något
  målats.
- **Mätt:** tom mask rekonstruerar insignalen (< 1e-3), 1 kHz-målning dämpar 1 kHz ≥30 dB och
  lämnar 4 kHz inom 1,5 dB, och fönstret ritas i en headless egui-bildruta.

## [0.9.179] - 2026-10-28

**Elva egna effekter, byggda här.** `src/audio/builtin_fx/` är ett register i stället för nio
nya fält i mastern: en `EffectDef` bär id, namn, kategori och sina parametrar, och effekten
laddas på samma väg som en plugin (`builtin:<id>`) — alltså i varje kanals kedja, med
automatable parametrar, utan fil och utan `--features plugin-host`.

- **Filter 72**: Butterworth låg-/högpass med 24, 48 eller 72 dB per oktav. Mätt: 24 dB ger
  −49 dB två oktaver över gränsen, 72 dB minst 30 dB brantare.
- **Tilt**, **Stege 24** (fyrpolig ladder med mättnad) och **Fasrotation** (allpass, 180° vid
  gränsfrekvensen).
- **Bass Manager**: mono under vald frekvens med LR4-delning, hylla och fas. Motfas bas
  försvinner; diskanten rörs inte; delningen är nivåneutral med mono av.
- **Ringmodulator** och **Pitch** (±12 semitoner, två korsande läspunkter; 0 semitoner är
  bitidentisk passthrough).
- **Spektral kompressor**: STFT 1024/256, frekvensberoende tröskel (center + lutning), nedåt-
  och uppåtkoppling med mjuk knee, attack/release per bin, mix. 768 sampel latens som PDC:n
  kompenserar. Ratio 1 rekonstruerar insignalen.
- **Inflator**: udda ordningens vågformning; toppfaktorn sjunker och toppen stannar under 1.
- **Justera**: sampelnoggrann fördröjning och polaritet per kanal — fasjustering av två
  mikrofoner på samma källa. 1 ms vid 48 kHz hamnar på sampel 48.
- **Rattar-rutan**: en inbyggd effekt har ingen egen GUI, så GUI-knappen öppnar en egui-ruta
  med en slider per parameter (värdet läses ur instansen, skrivs till samma handtag som
  ljudtråden läser). `PluginCore::parameter_value` svarar `None` när en backend inte kan, och då
  ritas ingen ratt.
- **Kurva**: waveshaper med åtta punkter — rak kurva är passthrough, fallande inverterar,
  mättande sänker toppfaktorn. Punkterna är parametrar, så den ritas i Rattar-rutan.
- **Gain reduction-scope**: masterns kompression över tid (~8 s) i FX-racket, 0 dB överst och
  −24 dB nederst. Mätningen kommer ur `master_gain_reduction_db`, historiken är ringformad.
- **Formatsfilter** i plugin-listan har en **Sonix**-flik, och de inbyggda ligger i listan även
  när cachen laddas — de finns inte som fil och kan därför aldrig hittas av en diskskanning.
- `docs/BUILTIN-FX.md`: vad de gör, vad som är mätt, och hur en ny läggs till (tre steg).

## [0.9.178] - 2026-10-28

**JSFX kan läsa ljudfiler, och tabeller skrivs som de ska.** `file_open`/`file_riff`/`file_avail`/
`file_mem`/`file_close` är inte längre stubbar: en WAV som ligger bredvid `.jsfx`-filen (eller i
`~/.config/sonix/jsfx`) öppnas, kanaler och frekvens rapporteras, och proverna kopieras in i
JSFX-minnet. `mem_set_values(dest, v1, v2, …)` skriver verkligen tabellen — den var en tom stubbe,
och REAPERs filer använder den för att bygga sina tabeller.

- `Jsfx::compile_in(source, mapp, srate)`: filens mapp är känd redan när `@init` körs, vilket är när
  en sampler öppnar sina ljudfiler.
- `@slider` körs vid laddning (som REAPER) — och därför finns nu `set_midi_note_name`,
  `slider_show`/`slider_hide`/`slider_name` m.fl. som ärliga no-ops: de ändrar bara hur REAPER visar
  saker, men koden ska inte falla på att de anropas.
- Bara WAV avkodas; FLAC (DrNine m.fl.) svarar fortfarande "ingen fil" och bokförs i `stubs`.

## [0.9.177] - 2026-10-28

**JSFX går att lägga i effektkedjan.** En `.jsfx`-fil laddas nu där en CLAP/VST-plugin laddas:
`load_processor` känner igen ändelsen och ger en `PluginProcessor` (backend `"JSFX"`), med filens
sliders som parametrar (`set_parameter` kör `@slider`, `reset` kör om `@init`). Det fungerar i
standardbygget — JSFX är egen Rust-kod och kräver inte `--features plugin-host`.

- `PluginFormat::Jsfx` i plugin-listan: `.jsfx` klassas som *JSFX (REAPER)* i stället för som en
  VST2 som inte går att ladda.
- `~/.config/sonix/jsfx` ligger i sökningen, och `.jsfx` hittas av skanningen.
- `Jsfx::compile` kör `@slider` en gång vid laddning, som REAPER — utan det var en effekt tyst fram
  till första vridningen.
- `Jsfx::reset` nollställer minnet (sliderna behålls) och kör `@init` igen.
- `shortcut`: instrument tar ingen MIDI genom `PluginProcessor` (traitet har ingen notingång), så
  JSFX-instrument låter i `--jsfx-note` men är tysta i kedjan än.

## [0.9.176] - 2026-10-28

**REAPERs 256 JSFX-effekter och instrument kör i Sonix.** `src/audio/jsfx/` är en EEL2-motor
(lexer, parser, virtuell maskin) som tolkar filerna som de är i stället för att porta dem en och
en — samma val som gjorde att en `PatternClip` räckte för alla stämmor. Alla 256 filer i REAPER
7.83:s `InstallData/Effects` tolkas och kör sin `@init`; de fyra referenseffekterna mäter
1,5–9× realtid per instans.

- **Språket**: `^` är potens (xor skrivs `xor`/`~`), `x[]` = `x[x]`, `?:`-grenar får vara block
  och `:` får saknas, `while ( kropp; villkor )`, `loop (i = 0; n, kropp)`, `<? ?>`-generatorblock,
  `/* */`, `import`, punktade namnrymder (`this.x` → `rmsa.x`), `$pi`/`$x90`/`$'F'`.
- **Motorn**: `slider1..64` på plats 0–63 som i REAPER, eget `f64`-minne, egna funktioner med
  lokala namn, MIDI in/ut, `fft`/`fft_real`/`ifft_real` via `rustfft`, 50 M steg per påstående
  som skydd mot `while(1)`. En effekt som kastar fel **tystnar hela blocket** i stället för att
  elda 2 M steg per prov.
- **Ärligt om stubbarna**: `file_open`/`file_riff`/`file_mem` m.fl. svarar "ingen fil" och bokförs
  i `Jsfx::stubs`, så en effekt som tystnar går att skilja från en som inte kunde läsas. `@gfx`
  och `@serialize` hoppas över (UI:t är REAPERs, tillståndet är projektets).
- **Instrumenten**: `TolerableTines` låter direkt; `DrNine`, `JamSynth` och `SynthyMcSynthface`
  laddar samplingar som inte skeppas med REAPER (de är tysta där också); `ChamberOrgan` importerar
  sin stopptabell och tolkas, men stannar på MIDI-filtret.
- **CLI**: `--jsfx-list <mapp>`, `--jsfx-run <fil> <in.wav> <ut.wav> [slider=värde]`,
  `--jsfx-note <fil> [not]`, `--jsfx-dump <fil> [sektion]`.
- **Prov**: `SONIX_JSFX_DIR=… cargo test --bin sonix corpus_tests` kräver alla 256 filer och att
  minst fyra instrument ger ljud från en nedtryckt tangent.
- **Docs**: `docs/JSFX.md` med mätningarna, språkets hörn och vad som är stubbat.

## [0.9.175] - 2026-10-28

**Fem separationsmodeller att välja mellan.** Katalogen `neural_separator::MODELS` ersätter den
gamla 4/6-boolen: varje modell bär sina stämmor, sin nedladdnings-URL, sin licensnot, sin
storlek — och sitt **format**, för nu finns två.

- **Två sorters export.** Demucs-familjen tar vågform. RoFormer-familjen och SCNet är exporterade
  *mellan* sin STFT och iSTFT, så appen gör fönstringen själv (`audio::stft`, ny modul med
  STFT/iSTFT, Hann eller inget fönster, skalning per export). Rundturen `istft(stft(x))` är provad.
- **Katalogen:** `htdemucs` (4), `htdemucs_6s` (6), `bs_roformer_sw6` (6 — den enda öppna export
  där gitarr och piano faktiskt separeras: 9,1 / 7,8 dB mot 3,1 / 1,6), `mel_roformer`
  (sång 12,1 dB) och `scnet` (4 stämmor, 13 MB).
- **Stemmor-vyn** har en lista i stället för en kryssruta: modell, not om kvalitet/licens, `✓` när
  den är hämtad, och hämtning per modell med storlekskontroll (13 MB-modellen krävde förut 100 MB
  för att godkännas).
- **Sångmodeller ger sång + rest**: resten räknas som `mix − sång`, så en sångmodell lämnar aldrig
  tystnad efter sig.
- **Namnen läses ur sessionen** (`spec_real`/`spec_imag`/`mix_spec` var inte samma i alla exporter),
  och ett formfel rapporterar modellens egna in-/utdata i stället för bara "Invalid rank".

Uppmätt med `SONIX_NEURAL_SMOKE=<id>` på 4 s syntetisk mix (CPU): `htdemucs` 0,997 · `htdemucs_6s`
0,998 · `scnet` 0,863 · `bs_roformer_sw6` 0,929 · `mel_roformer` 1,000 av källans RMS — alla
kör genom vår inferens. `docs/STEM-SEPARATION.md` har tabellerna, kontrakten och gränserna.


## [0.9.174] - 2026-10-28

**AI:n får mäta.** Sex nya verktyg. Assistenten kan inte höra — så den läser appens egna mätvärden
och föreslår utifrån dem i stället för att gissa.

- **`analyze_mix`** mäter LUFS och topp per spår (spår för spår genom samma offline-render som
  exporten använder, `solo_track`) plus sektionerna. Ett nivåförslag kommer nu ur siffror.
- **`groove_report`** mäter spårets slag mot sextondelsrutnätet: median och värsta avvikelse i ms.
- **`master_to_target`** sätter närmaste loudness-preset, renderar, normaliserar på samma väg som
  exporten och rapporterar uppmätt LUFS före och efter samt topp.
- **`chop_channel`** slår upp kanalens ljud vid sina transients (samma detektor som mixerpanelen)
  och lägger en slice per sextondel i ett **tomt** mönster — ett komp som spelar rörs inte.
- **`rename_track`** döper spår, t.ex. stämmorna efter en separation.
- **`write_script`** kör Rhai mot den valda tagningen — mot en **kopia**, så ett fel halvvägs
  lämnar tagningen orörd.

Dessutom: kontexten till modellen innehåller nu spåren (pattern/ljud/klippa/tystad) och sektionerna,
inte bara kanalerna. `docs/AI-API.md` har hela verktygstabellen — den påstod felaktigt att
`set_channel_instrument` saknade utförare, och att `set_key` var byggt.


## [0.9.173] - 2026-10-27

**Tutti — och fixarna som kom av att använda appen.** Projektet kan lämnas till det delade
projektformatet och tas tillbaka igen. Vägen dit lagade det som mötte oss: ljudströmmen, tempot i
gamla filer, melodin som satt fast i rutnätet, och en MIDI-slinga som spelade av sig själv.

**Nyheter**

- **Tutti ut och in** (0.9.172/0.9.173): `sonix --export-tutti` skriver en tutti-folder som
  `tutti check` godkänner, och `sonix --import-tutti` gör en tutti-folder till ett `.sonix`-projekt.
  Rundtur export → import → export ger ett identiskt dokument; Sonix egna fält (warp, EQ,
  kompressor, swing, tonart) bärs i `se.alexwest1981.sonix` så en runda genom ett annat program
  inte tappar dem. `.sonix` är kvar som appens eget format.
- **AI:ns melodi blir en klippa på tidslinjen.** `write_notes` skapar en `PatternClip` med egna
  noter (position, anslag, notlängd) i stället för att tända rutor i mönstret. Klippan ritas och
  dras som en ljudregion, och en flytt rör bara startpositionen.
- **Högerklick på en not i pianorullen** (den sista punkten i "högerklick överallt"): radera,
  duplicera ett steg senare, anslag ±.
- **`sonix --version` / `-V`** skriver `Sonix Studio <version>` utan att öppna fönster eller
  ljudenhet — en rad att klistra in i en felrapport.
- **Flikar i toppraden**: grupperna (Skapa/Arrangera/Mixa/Verktyg) och vyerna är mappa-flikar med
  accentlinje på den valda, inte knappar.

**Fixar**

- **Midi Through är en slinga, inte ett instrument.** MIDI-in kopplade upp sig mot varje in-port,
  även ALSA:s loopback — och `--selftest` skickade sin testnot dit. En pågående Sonix hörde då sin
  egen testnot och spelade den. Slingor hoppas nu över, och testnoten går till en riktig utgång.
- **512 frames som standard** (0.9.168): 128 dog på en lastad maskin; en död ljudström byggs nu om
  i stället för att tiga, och felräknaren hör till strömmen som dog.
- **Stämmornas tempo kommer ur arkivet, aldrig ur 120** (0.9.169), och en gammal fil med 120 i
  klippen lagas vid inläsning (0.9.171).
- **Exporten säger rätt installationskommando**: ffmpeg-raden läses ur `/etc/os-release` (apt, dnf
  eller pacman) i stället för att alltid föreslå `pacman`.
- **Textrutan äger tangenterna.** Ett mellanslag i ett projektnamn startade uppspelningen; nu tiger
  transport- och arrangergenvägarna medan en textruta har fokus. En not som startats med musen i
  pianorullen släpps också när fliken lämnas i stället för att ligga kvar och spela.
- **Fasmätaren skriver vad bildrutan gjorde** när den sprack (0.9.170).


## [0.9.167] - 2026-10-27

**En död ljudström byggs om i stället för att tiga.** cpal 0.15 återhämtar sig inte från en
ALSA-ström som svarat `POLLERR`: `alsa::poll()` fortsätter svara fel, callbacken kallas aldrig
mer — men strömmen ser fortfarande `RUNNING` ut i `pactl`. Mätt: 40 000 fel/s, 1,5 GB logg, en
kärna bränd och noll ljud när play trycktes.

- **Hjärtslag per buffert** i ljudcallbacken. Står det stilla en sekund byggs strömmen om med
  samma `reconfigure` + `resync_engine_after_reconfigure` som ljudmodalen, och spelar vidare
  från samma position om transporten rullade. Cooldown 2 s så en enhet som vägrar starta inte
  byggs om varje bildruta.
- **Felloggen dämpad:** tre rader och sedan var 20 000:e fel, både i `err_fn` och i
  panikvägen — en död ström ska synas, inte äta disken.
- **Felräknaren hör till strömmen** som dog: `reconfigure` nollställer den, och vakthunden
  läser den innan omstarten, så statusraden säger den döda strömmens fel och inte appens total.

**AI:n lovar inte längre mer än rutnätet kan skriva.** `write_pattern` skriver en not per steg
utan anslag och längd; modellen kunde ändå be om trioler och legato som appen tyst kastade bort.

- Prompten säger vad som gäller (sextondelsrutnät, en not per steg, heltal), och `parse_notes`
  avvisar bråktal med ett begripligt fel i stället för "steget -1 ligger utanför".
- Varje stämma bär en `subdivision` (1/16 … 1/1); anropet avvisas om slagen ligger utanför den,
  med raden som visar vilka steg som bryter.
- `2.0` läses som 2 — JSON-bråktal föll tidigare igenom som fel.

## [0.9.165] - 2026-10-27

**AI:ns melodi blir ett objekt på tidslinjen.** Den skrev in i mönstrets rutnät, och där fanns
inget att greppa — ingen klippa att flytta, inget att spara. Nu skriver `write_notes` en
**klippa** (`PatternClip`) med egna noter: position, anslag och notlängd.

- **`PatternClip`** på spåret (`pattern_clips`): start och längd i `f32` takter som en
  ljudregion, `lane` mot kanalracket, och noterna i klippan. Sparas i projektfilen
  (`#[serde(default)]`, så gamla projekt öppnas som förut).
- **AI:n skriver noter, inte steg.** `write_notes` skapar klippan vid spelhuvudet på valt spår i
  stället för att tända rutor i rutnätet — notlängd och anslag följer med, vilket rutnätet inte
  kunde bära. Trummor ligger kvar i rutnätet; en klippa utan noter spelar mönstrets rutnät.
- **Klippan ritas och dras** i arrangemanget med snäpp och ångra. Flytten skriver bara
  `start_bar`, så noterna kan inte ändras av en förflyttning (prov i `state::clip_tests`).
- **Uppspelning:** en klippa spelar sin kanal, läser kanalens sampel vid varje slag (bytt ljud
  behåller noterna) och loopar sin egen längd.
- **Fyra döda AI-prov körs igen.** Tre prov i `ai_tools` hade förlorat sina `#[test]`-attribut i
  en tidigare commit och räknades aldrig — de ligger nu på rätt funktion, och `grid_of` känner
  igen de svenska pluralformerna ("åttondelar") som provet kräver.

Kvar till nästa pass: knappen som lägger klippan vid spelhuvudet, Sample-export (WAV + `.mid`),
och triolernas `pos`-schemaläggning — se `docs/AI-CLIPS.md`.


## [0.9.162] - 2026-10-26

**Sju språk, inte bara i appen.** Gränssnittet har kunnat sju språk sedan tidigare, men repot och
skrivbordsposten var engelska — och GitHub visar bara EN README, så en spansk eller tysk besökare
möttes av ett språk hen kanske inte läser.

- **`README_DA/NB/DE/ES/FR.md`** — korta README-filer på danska, norska, tyska, spanska och
  franska: vad det är, skärmdump, installationstabell, licens, och en länk till den engelska
  README:n för hela funktionslistan. Korta med flit: en full översättning av 400 rader ruttnar
  mellan releaserna, det här håller.
- **Språkrad i varje README** som länkar till alla sju, så ingen fil är en återvändsgränd.
- **Skrivbordsposten är översatt** (`Comment[sv/da/nb/de/es/fr]`, `GenericName[...]`,
  `Keywords[...]`) — appens namn och beskrivning syns nu på användarens språk i menyer och
  programstartare på alla distribuerade paket. `desktop-file-validate` godkänner den.
- **AppStream-metainfo** har `summary xml:lang` för samma sex språk, så mjukvaru-butiker visar en
  lokaliserad sammanfattning. `appstreamcli validate` godkänner den.
- **`packaging/check-docs-languages.sh`** är grinden: den faller när en README saknas, när
  språkraden inte länkar till alla, eller när .desktop/metainfo saknar ett språk. Körd i CI, och
  provad genom att ta bort en fil (den föll, som den ska).
- Release-tarballen tar med alla README-filer.

Manualen och CONTRIBUTING är kvar på engelska — det är sagt rakt ut i stället för halvöversatt.


## [0.9.161] - 2026-10-26

**Licensen är nu GPL-3.0-only.** Projektet var dubbellicensierat MIT OR Apache-2.0, vilket är
exakt motsatsen till vad som efterfrågades: båda licenserna tillåter vem som helst att ta koden,
stänga den och sälja den under ett annat namn. GPL-3.0 behåller det som var poängen — bidrag är
välkomna och ingen kan ta programmet och sälja en inlåst version.

- `LICENSE` är GPL-3.0-only, ordagrant ur SPDX (`GPL-3.0-only.txt`), med copyrightraden i README.
  `LICENSE-MIT` och `LICENSE-APACHE` är borta.
- **`CONTRIBUTING.md`** är ny: hur man bidrar, `git commit -s` (DCO) som enda pappersexercis,
  kodkonventionerna, och varför en tredjepartsplugin inte ärver programmets licens (CLAP/VST3 laddas
  som separata binärer vid körning).
- README (båda språken) förklarar licensen med vanliga ord: använd/byg/ändra/dela fritt, dela källkod
  när du delar en ändrad Sonix, inga CLA:er, plugins oberoende, och att **namnet** inte följer med
  koden — en fork bär eget namn och ikon. Samt att versioner till och med 0.9.160 behåller sina
  villkor, eftersom en licens inte är retroaktiv.
- `Cargo.toml`, `nfpm.yaml` (deb/rpm-licensfältet), AppStream-metainfo och release-tarballen pekar
  på den nya licensen; paketen skeppar `LICENSE` + `CONTRIBUTING.md` i stället för de två gamla
  filerna.

## [0.9.160] - 2026-10-26

**Fungerar på alla distributioner.** Tills i dag fanns binären bara som ett tarball byggt på en
Ubuntu-runner: glibc ≥ 2.35 för x86-64 och ≥ 2.39 för aarch64, vilket stänger ute Debian 11,
Ubuntu 20.04 och hela RHEL 8-serien — och en binär byggd på Arch begärde till och med GLIBC_2.44
och startade inte ens på Ubuntu 24.04. Nu byggs releasen i en **glibc 2.28**-container för båda
arkitekturerna, och flödet **stannar** om någon symbol begär nyare.

- **`packaging/check-glibc-floor.sh`** — grinden. Läser binärens GLIBC-symbolversioner och faller
  över golvet. Körs både i CI och i releaseflödet.
- **`packaging/verify-in-docker.sh`** — beviset. Installerar paketet i en riktig distribution och
  startar appen med Xvfb; ett nytt CI-jobb kör den på Debian 12, Ubuntu 22.04, Ubuntu 24.04,
  Fedora 41 och Rocky 9.
- **Fyra former av samma binär:** `.deb` (Debian, Ubuntu, Mint, Pop!…), `.rpm` (Fedora, RHEL,
  Rocky, Alma, openSUSE), **AppImage** (ingen installation alls) och tarballen som förut.
  `SHA256SUMS` ligger bredvid och alla filer attesteras med byggbevis.
- **`packaging/nfpm.yaml`** — paketberoendet deklareras per paketformat (`libasound2` för deb,
  `alsa-lib` för rpm) och `ffmpeg` som rekommendation, eftersom WAV/FLAC fungerar utan.
- **`packaging/io.github.alexwest1981.Sonix.{desktop,metainfo.xml}`** — desktoppost, ikon och
  AppStream-metadata som `appstreamcli validate` godkänner, så appen syns i programmenyer och
  mjukvaru-butiker med namn, ikon och skärmdumpar.
- README (båda språken) svarar nu på frågan en ny användare har: *vilken rad ska jag köra?* — en
  tabell per distribution, vad varje form installerar, och hur den tas bort.

## [0.9.159] - 2026-10-25

**aarch64-release.** Release-workflowen bygger nu båda arkitekturerna: x86_64 på `ubuntu-22.04`
och aarch64 på `ubuntu-24.04-arm`, som två jobb som laddar upp var sin artefakt och en tredje som
attesterar och publicerar dem tillsammans. Det var den sista varningen i OmaStore — utan en
arm-tillgång ser appen "inte installerbar" ut för aarch64-användare.

- `omastore.toml` deklarerar nu `linux.aarch64` med samma `{version}`-mönster.
- ARM-binären kräver glibc ≥ 2.39 (Ubuntu 24.04); x86_64 ligger kvar på ≥ 2.35.
- `.SRCINFO`/`PKGBUILD` deklarerar `alsa-lib` och `glibc`, så butiken kan kontrollera beroenden.

## [0.9.158] - 2026-10-25

Första releasen sedan 0.9.0 — **579 commits**. Den fullständiga listan står i
[CHANGELOG-DETAIL.md](CHANGELOG-DETAIL.md); det här är vad man märker.

- **Masterns FX-rack**: Love Philter (tre filter i serie), Luxeverb (FDN), Emphasis,
  Hyper Chorus, Maximus (tre band + master, solo per band, ritbara volymkurvor),
  16-bands vocoder med egna bandnivåer och Time Curve som bandspelarhastighet.
- **Tempot mäts ur ljudet** vid import och per klipp, och **tempokartan gäller överallt**:
  uppspelning, sökning, nedräkning och export. Klipp utan känt tempo behåller sina sekunder.
- **Edison, Newtone och Newtime** som redigerare på urval, med spektrum, tonhöjd och tid.
- **FLEX-fabriken**: åtta namngivna ljud som följer med appen.
- **38 skärmdumpar** i `screenshots/`, tagna mot den här versionen.

## [0.9.0] - 2026-09-11

First public release. Everything advertised in the UI is backed by the real
audio engine; the remaining gaps are documented honestly in
[README.md](README.md#-whats-left--how-far-from-real) and [ROADMAP.md](ROADMAP.md).

### Added

- **Audio engine** — real-time cpal/ALSA engine with a crash-safe callback;
  oscillator synth (Analog Alchemy), drum synthesis, per-voice filter and
  filter-envelope, master FX chain, delay/reverb, Remix FX (stutter/reverse),
  tape-stop, and modular patcher DSP with topological sorting.
- **Sequencer & arranger** — multitrack timeline with linear audio regions,
  slice tool, loop-dragging, 16-step Channel Rack, Piano Roll with MIDI
  keyboard recording, song-section arranger, and automation curves.
- **Mixer** — faders, real VU and gain-reduction meters, per-track 3-band
  EQ / compressor / sends / pitch, VCA groups and sub-mix buses.
- **Vocal Studio** — microphone recording, take lanes and comping, pitch
  editor, offline and real-time autotune, formant-preserving pitch shift,
  4-voice harmonizer, sampler, and WSOLA pitch-preserving time-stretch.
- **Generators** — chord matrix, beat/melody dice, Session Drummer, strobe
  tuner, and add-track templates.
- **AI** — local rule-based composer, external LLM support
  (OpenAI / Anthropic / OpenRouter / Ollama), audio generation APIs
  (OpenAI TTS / Stability), and project-context awareness.
- **Stem separation** — DSP separator plus optional neural HTDemucs via ONNX
  (`--features neural` with a user-supplied model), and Suno ZIP import.
- **Plugin hosting** (opt-in `--features plugin-host`) — in-process CLAP host,
  per-track audio processing with PDC, plugin state/preset save-load, CLAP GUI
  in its own X11 window, out-of-process sandbox with crash detection/restart
  and shared-memory audio transport, plus hand-rolled VST3 and VST2 ABI hosts
  with parameters, state and PDC.
- **Export & project I/O** — WAV/FLAC in-app, MP3/OGG/AAC via `ffmpeg`,
  offline full-project renderer with real samples and clean metadata, export
  presets, loudness normalization, and project save/load.
- **Hardware** — real ALSA MIDI (MCU) control and a UDP OSC server.
- **Localization** — 7 languages: Swedish, English, Danish, Norwegian, German,
  Spanish and French.
- **Documentation** — English and Swedish README, roadmap, tooling history and
  a full user manual.
- **Release infrastructure** — dual-licensed under MIT OR Apache-2.0, with
  GitHub Actions CI (default, `plugin-host` and `neural` builds).

### Fixed

- Tape-stop idle passthrough: no ring-buffer latency and no initial silence
  when the effect is disabled.
- Parallel-test race in the CLAP test fixture by giving each mock plugin
  instance its own state.
- Stale "not implemented" plugin/sandbox texts and the `.fst` preset
  rationale, which now state the real reason (the closed format cannot be
  decoded).

[Unreleased]: https://github.com/alexwest1981/sonix/compare/v0.9.175...HEAD
[0.9.175]: https://github.com/alexwest1981/sonix/compare/v0.9.174...v0.9.175
[0.9.174]: https://github.com/alexwest1981/sonix/compare/v0.9.173...v0.9.174
[0.9.173]: https://github.com/alexwest1981/sonix/compare/v0.9.167...v0.9.173
[0.9.167]: https://github.com/alexwest1981/sonix/compare/v0.9.165...v0.9.167
[0.9.159]: https://github.com/alexwest1981/sonix/compare/v0.9.158...v0.9.159
[0.9.158]: https://github.com/alexwest1981/sonix/compare/v0.9.0...v0.9.158
[0.9.0]: https://github.com/alexwest1981/sonix/releases/tag/v0.9.0
