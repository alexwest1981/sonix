# Ableton Live 12.4.5 Suite — inventering på molekylnivå

Allt här är läst ur **installationen själv** (`~/Downloads/Ableton Live Suite 12.4.5 (x64)/Setup`,
6,1 GB), inte ur Abletons webbsidor eller ur minnet. Siffrorna är räknade. Där något inte gick att
läsa står det.

| Mätning | Värde |
| :--- | :--- |
| Format | **Inno Setup 6.4.0.1** (`Ableton Live 12 Suite Installer.exe` 6,6 MB + fyra `.bin`) |
| Uppackning | `innoextract 1.13.0` (1.9 klarar inte 6.4 — se §12) |
| Poster i innehållsförteckningen | **49 112** |
| Uppackat | **6,02 GiB**, varav `app/Resources` 5,36 GiB |
| Strängar i `Ableton Live 12 Suite.exe` (141 MiB) | **454 338** ASCII + **10 748** UTF-16 |
| Tredjepartspaket i `Legal/Third-party License Information.txt` | **595** |
| Typos i Abletons egna strängar | **6** hittade, ordagrant citerade i §6 |

Tre kompletta referensfiler hör hit: `docs/ableton-files.md` (alla 49 112 poster),
`docs/ableton-command-surface.md` (725 menykommandon + 523 dialoger + 200 statusrader + 2 302
ledtrådar). Inget i dem är ett urval.

**Säkerhet:** nedladdningen innehåller också `Patcher-Keygen-AUDIOWAREZ/` (keygen + patcher).
Den är **inte** öppnad, körd eller analyserad. Allt nedan kommer från `Setup/`, alltså den riktiga
installationen.

---

## 1. Strukturen

| Nivå | Filer | Storlek |
| :--- | ---: | ---: |
| `app/Program` | 2 940 | 442 MB |
| `app/Resources` | 46 160 | 5 481 MB |
| `app/Resources/Max` | 25 378 | 1 710 MB |
| `app/Resources/Core Library` | 16 365 | 2 021 MB |
| `app/Resources/Builtin` | 724 | 1 400 MB |
| `app/Resources/MIDI Remote Scripts` | 1 389 | 11 MB |
| `app/Resources/Builtin Lessons` | 1 016 | 25 MB |
| `app/Resources/Python` | 966 | 16 MB |
| `app/Resources/Schema` | 203 | 61 MB |
| `app/Program/Push` | 2 023 | 138 MB |
| `app/Program/Push2` | 545 | 1,8 MB |
| `app/Resources/Extensions` | 7 | 65,5 MB |

De tunga binärerna i `app/Program`: `Ableton Live 12 Suite.exe` **141 MB**,
`Ableton Live Engine.dll` **26,8 MB**, `DirectML.dll` 26,1 MB, `onnxruntime_abl.dll` 17,4 MB,
`musicai.dll` 3,6 MB, `Push3.exe` 30,9 MB, Qt5-sviten (Core/Gui/Qml/Quick/Widgets/Svg), `soxr.dll`,
`libmp3lame.dll`, `REX Shared Library.dll`, `USFLIB.dll`, `VideoExportMMF.dll`, `WebView2Loader.dll`.

**Ändelser som säger något** (hela listan i `docs/ableton-files.md`):

| Ändelse | Antal | Vad det är |
| :--- | ---: | :--- |
| `.js` | 7 653 | JavaScript i Max, Web Connector och web-UI |
| `.asd` | **6 138** | Abletons analysfil per sampling (transienter, vågform) |
| `.pyc` | 3 530 | kompilerad Python: remote-skript och Live API |
| `.adv` | 2 557 | device-preset, en ratt inställd på en enhet |
| `.maxpat` | 1 795 | Max-patcher, läsbar JSON |
| `.adg` | 1 072 | rack-preset (Instrument/Drum/Audio Effect Rack) |
| `.md` | 1 063 | markdown — **alla utom en är Max-runtimens licensfiler** |
| `.qml` | 345 | Push-skärmens gränssnitt |
| `.ascl` | **152** | Abletons Scala-utökning: mikrotonala stämningar |
| `.agr` | 219 | groove-mallar (swing) |
| `.alc` | 350 | Live Clips |
| `.amxd` | 110 | Max-devices |
| `.als` | 26 | Live Sets (mallar) |
| `.mxe64` | 864 | kompilerade Max-externals |

---

## 2. De 72 enheterna

**13 instrument** (antal förinställningar i parentes): Operator (332), Wavetable (271), Tension (108),
Drift (90), Meld (85), Sampler (77), Analog (59), Simpler (39), Collision (34), Electric (18),
Impulse (14), External Instrument, Max Instrument.

**48 ljudeffekter**: Legacy (113), Reverb (80), Hybrid Reverb (73), Roar (65), EQ Eight (63),
Echo (49), Delay (34), Corpus (27), Auto Filter (25), Shifter (25), Auto Pan-Tremolo (24),
Drum Buss (23), Saturator (23), Auto Shift (20), Grain Delay (20), Channel EQ (19), Compressor (19),
Glue Compressor (17), Chorus-Ensemble (16), Filter Delay (16), Pedal (16), Beat Repeat (14),
Amp (13), Multiband Dynamics (13), Phaser-Flanger (13), Shaper (13), Cabinet (12), Limiter (12),
Resonators (12), Spectrum (12), Utility (11), Erosion (10), Redux (10), LFO (9),
Spectral Resonator (9), EQ Three (8), Vinyl Distortion (8), Vocoder (8), Dynamic Tube (7),
Looper (7), Spectral Time (7), Envelope Follower (6), Gate (6), Overdrive (6),
External Audio Effect, Max Audio Effect, Tuner.

**11 MIDI-effekter**: Scale (37), Chord (22), Velocity (16), Note Echo (11), Random (9),
Arpeggiator (8), MPE Control (8), Envelope MIDI (6), Note Length (5), Pitch (4), Max MIDI Effect.

Jämfört med FL Studio (141 plugins, 51 generatorer + 90 effekter) är Ableton **färre men djupare**:
13 instrument där varje instrument har ett eget syntessätt, och 48 effekter där nästan alla är
Abletons egna algoritmer — inte varianter av samma krets.

---

## 3. Innehållet

| Vad | Antal |
| :--- | ---: |
| En-shot-samplingar | 5 538 |
| Multisamplingar | 1 104 |
| Loopar | 492 |
| Instrument Racks | 738 (Bass 117, Pad 114, Synth Keys 97, Synth Lead 75, Plucked 58, Mallets 46 …) |
| Drum Racks | 146 (Sampled 70, Electronic 51 …) |
| Audio Effect Racks | 149 (Mixing & Mastering 47, Space 33 …) |
| Device-presets | 2 557 |
| Groove-mallar | 220 i 88 grupper (Swing/Logic 64, Basic 30, MPC 23, Notator 10, SP 1200 10, Percussion) |
| Mikrotonala stämningar | **152** i 7 traditioner: Turkish Makam 36, Sundanese Gamelan 26, European Historical 22, Arabic Maqam 21, EDO 21, Just Intonation 13, Persian Radif 12 |
| MIDI-klipp | 242 (tonala) |
| Lektioner | 1 016 filer i 6 kategorier (What's New, Live Basics, Max for Live Devices, Hardware Setup) |
| Max-exempel | 1 000 |
| Analysfiler (`.asd`) | 6 138 |

Ett dokument i samlingen är värt att nämna särskilt:
**`Core Library/Tunings/Specification - Make your own tuning.ascl`** — specifikationen för ASCL är
*skriven som en ASCL-fil*, alltså ett format som demonstrerar sig självt, med kommentarsrader som
förklarar syntaxen.

---

## 4. Undersystemen

| Undersystem | Bevis | Vad det är |
| :--- | :--- | :--- |
| **Max for Live** | `Max.exe`, `MaxRT.exe`, `MaxRT_nocef.exe`, 25 378 resursfiler, 864 `.mxe64`, 1 000 exempel | en hel Max-runtime inuti DAW:n; användare kan bygga egna enheter |
| **Två Python-miljöer** | `Resources/Python` 966 filer; `Program/Push/python3_dist` med 374 site-packages | Lives inbäddade skript och Push-enhetens egen Python |
| **Live Object Model som källkod** | `Push/python/live_model` 64 filer, `Push/python/ableton` 89 | hela API:t för klienter, läsbart |
| **AI** | `musicai.dll` (3,6 MB), `onnxruntime_abl.dll` (17,4 MB), `DirectML.dll` (26,1 MB) | ONNX-inferens på GPU via DirectML — det är här stämsepareringen och klanglikheten bor |
| **Klanglikhet** | `Sound similarity feature file …` (8 strängar) | sök i biblioteket efter hur det *låter*, inte vad det heter |
| **Push-gränssnitt** | 345 `.qml`, `Qt5Qml/Quick`, 663 bilder, `Push2DisplayProcess.exe` | Push 2/3 ritar sin skärm med QML + CSS |
| **Extensions** | `Ableton AddOns.exe`, `Ableton Index.exe`, `Ableton Plugin Scanner.exe`, `Ableton Swapper.exe`, `Ableton Web Connector.exe`, `xdelta3.exe` | Live 12.2+ tredjepartsförlängningar, egen plugin-skanner, binär delta-uppdatering |
| **Kontrollerytor** | **149 mappar** i `MIDI Remote Scripts` | 149 hårdvarumappningar, varav 6 är ramverk (`_Framework`, `pushbase`, `ableton`) |
| **Inbäddad webbläsare** | `WebView2Loader.dll` + WebView2-installationsprogram (183 MB) | HTML-paneler |
| **CEF** | `license-cef.md` i Max | Max-runtimens webbläsare |
| **Video** | `VideoExportMMF.dll`, `Render To Disk No Video` | videoexport via Media Foundation |
| **ReWire** | `ReWire.dll` | bakåtkompatibel ReWire-värd |
| **Link** | `Ableton Link Audio`, `Ableton Link Start Stop Sync` | synk över nätverket, nu även ljud |
| **Mikrotonalitet** | `.ascl`, `Show Tuning Pool`, `Scala Tuning System`, `Retune Set On Tuning System Change` | Scala-stämningar som förstklassigt begrepp |
| **Tidskoder** | `Ruler SMTPE 24/25/29/30 Drop/Non Drop` | SMPTE-linjal |
| **Oversampling** | `HiQualityOversampling`, `Oversample` | 2× i masterkedjan som val |
| **Tredjepart** | 595 paket i licensfilen | bl.a. Eigen, LAME, libflac, libogg, libvorbis, onnx, onnxruntime, PortAudio, libxml2, YAML |

---

## 5. Kommandoytan

Ableton namnger sina gränssnittssträngar med prefix, vilket gör inventeringen exakt:
**725 `sMenu*`**, **523 `sDialog*`**, **200 `sStatus*`**, **2 302 `sInfoText*`**.
Alla ligger i `docs/ableton-command-surface.md`.

Ableton skeppar dessutom **sina egna UI-testers reguljära uttryck** i binären — `^I Capture and
Insert Scene$`, `^the Groove Pool should( not)? be open$`, `^the global tuning system's name should
be '(.*)'$`. Gränssnittet testas alltså med regexar mot åtkomliga element, och testskripten ligger
delvis kvar i den skeppade binären.

Operationerna som gör Ableton till Ableton, och var Sonix står:

| Ableton-operation (ur binären) | Sonix i dag |
| :--- | :--- |
| `Insert Time…`, `Delete Time`, `Duplicate Time`, `Cut Time`, `Paste Time` | ✅ Insert/Delete/Duplicate Time (byggda i dag); Cut/Paste Time kvar |
| `Consolidate`, `Consolidate To New Scene` | ◐ (`freeze` renderar men lämnar inget klipp) |
| `Bounce To New Track`, `Bounce Track In Place`, `Bounce Group In Place` | ✅ de två första (`bounce_track_to_new`, `freeze_track`); ✗ gruppversionen — grupper bakas inte som en enhet |
| `Capture MIDI` | ◐ (tagningar spelas in, inte "fånga allt du nyss spelade") |
| `Capture and Insert Scene` | ✗ |
| `Convert Melody/Harmony/Drums To New MIDI Track` | ◐ (`onset` + chopper ger slag, inte melodi) |
| `Slice To New MIDI Track`, `Slice To Drum Rack` | ✅ (chopper → slicemap → piano roll) |
| `Separate Stems To New Audio Tracks`, `Separate Stems For Time Selection` | ✅ (`neural_separator` + `stem_separator`) |
| `Chop Notes`, `Split Notes`, `Join Notes`, `Legato`, `Fit To Time Range` | ◐ (humanize/kvantisering finns; ingen Legato/Join) |
| `Quantize Settings`, `Groove Pool`, `Extract Groove`, `Extract Grooves` | ✅ groove-mallar: `.agr` läses och läggs på tagningen (116 av 219 filer är XML, 103 binära); ingen groove-pool med flera mallar samtidigt |
| `Follow Action` (A/B, chance, jump index, multiplier), `Create Follow Action Chain` | ✗ |
| Take lanes: `Insert Take Lane`, `Copy From Take Lane To Content Lane`, `Delete Unused Take Lanes`, `Swap Lane` | ◐ (MIDI-tagningar, ingen kompning i spellistan) |
| `Warp Mode`: Beats, Tones, Texture, Re-Pitch, Complex, Complex Pro (+ granularity, formants) | ◐ (WSOLA, bandspelare, formantbevarande pitch — inte sex lägen) |
| `Show Tuning Pool`, Scala/ASCL | ✅ (`Tuning` + `parse_ascl`; `.ascl` läses i menyn) |
| `Group Note Probabilities`, `Probability Group` | ✗ |
| `MPE Control`, `Per Note Pitch Bend`, `Note Chase` | ✗ (MPE finns inte alls) |
| `Create Branch`, `Sort Branches`, `Show Branch Names In Editor` | ✗ (arrangemangsversioner) |
| `Compare Copy`, `Compare Switch` | ✗ (A/B-jämförelse av två versioner) |
| `Exclude Macro From Snapshots`, `Exclude Macro From Randomization` | ◐ (makrokedjor finns; inga snapshots) |
| `Show Modulation Source`, `Show Controlling Device` | ◐ (automation finns, ingen källa→mål-vy) |
| `Extensions`, `Max For Live Window`, `Max For Live Developer Mode` | ✗ (inget skript-API; patchern är intern) |
| `Grid Quintuplet`, `Grid Septuplet` | ✗ (bara raka och trioler) |
| `Render To Disk`, `Render To Disk No Video` | ✅ (exporterar wav/flac/mp3/m4a, dither, loudness) |
| `Freeze`/`Unfreeze`, `Flatten` | ◐ (`freeze` finns, ingen `Flatten`) |
| `Solo In Place`, `Arm Exclusive`, `Chase Notes` | ◐ (solo/mute finns) |
| `Load Demo Set`, lektioner, `Navigate To Learn View` | ✗ (ingen inbyggd undervisning) |

---

## 6. Där en utvecklare stavat fel

Du bad om det, och det finns — i Abletons *egna* strängar, inte i tredjepartskod. Ordagrant ur
`Ableton Live 12 Suite.exe`:

| Sträng | Felet | Var |
| :--- | :--- | :--- |
| `Unhandled situation occured` | **occured** → occurred | felhantering |
| `unhandled situtation occured` | **situtation** *och* **occured** — två fel i samma rad | felhantering |
| `: ShowOwnPropertyView: failed Exception occured` | **occured** | debug |
| `Copy Ptr Adress to OS-Clipboard` | **Adress** → Address | debug-meny |
| `ASE_OK: Call was succesful` | **succesful** → successful | status |
| `If you need to know wether the Clip was triggered, use the is_triggered property.` | **wether** → whether | **Live API-dokumentationen** (den som användare läser) |
| `…the size of the psuedo-octave.` | **psuedo** → pseudo | **`Specification - Make your own tuning.ascl`**, alltså specifikationen för ett filformat |

Två av dem sitter i dokumentation som användare läser (API-docstringen och ASCL-specifikationen),
resten i fel- och debugtexter. Det är inte många med 454 338 strängar — men de finns.

Två saker till som är värda att veta, utan att vara stavfel:

- **`README.md` för Push-widgetarna** (den enda `.md` som inte är en licens) pekar på
  `https://jupiter.office.ableton.com/documentation/modules-widgets/` — en **intern värd** i en
  skeppad fil. Den som läser README:n på en maskin utan Abletons nät kommer ingenstans.
- I de interna symbolnamnen heter det `sMenuInsertMidiTrack` (21 gånger **Midi**), medan den
  användarvända texten skriver MIDI. Symbolnivån är inkonsekvent; gränssnittet är det inte.

---

## 7. Vad Ableton gör sämre (mätt, inte tyckt)

| Sak | Mätning |
| :--- | :--- |
| **Ingen Linux-version** | Installationen innehåller `Ableton Live 12 Suite.exe`, `InstallHelper.exe`, `.msi` för ljuddrivrutin — allt Windows. Sonix är en Linux-binär. |
| **Storleken** | 6,02 GiB, varav **1 710 MB Max-runtime** och 1 400 MB `Builtin`. En användare som aldrig öppnar Max betalar 1,7 GB för den. |
| **`.asd`-sidofiler** | 6 138 analysfiler i biblioteket, och Ableton skriver en `.asd` **bredvid varje sampling du importerar**. Sonix håller sina toppar i sin egen cache. |
| **Dokumentationen är delvis licenser** | 1 063 `.md`, varav 1 062 är Max-runtimens tredjepartslicenser. En läsare som söker dokumentation i `.md`-filerna hittar licenser. |
| **Python är enda skriptvägen** | Allt skript är `.pyc`/`.py`; ingen JS/WASM-API för användare (Extensions är ett slutet SDK med fem förkompilerade verktyg). |
| **Ingen CLAP** | `VST3` 257 strängar, `CLAP` 8 — och de 8 är detektion, inte värdskap. Sonix har CLAP-värd. |
| **Stängd projektfil** | `.als` är gzip'ad XML utan publicerad specifikation; `Schema/`-filerna är versionsstämplar, inte ett schema. Sonix projekt är läsbar JSON. |
| **32-bitars plugins** | Borta sedan Live 10. Sonix har sandlåda och kan köra dem externt. |
| **Ingen inbyggd testbarhet** | Ingen motsvarighet till `sonix --selftest`; Abletons egna UI-testregexar ligger i binären men går inte att köra. |

Och det Ableton gör **bättre** än vi, ärligt: 149 kontrollerytor mot våra MCU-ytor, 152
mikrotonala stämningar mot noll, Max for Live som ekosystem, lektioner inbyggda, och sex
warplägen där vi har två beteenden.

---

## 8. Gap-matrisen

| Område | Ableton | Sonix | Dom |
| :--- | :--- | :--- | :--- |
| **Instrument** | 13, varav 8 egna syntesmotorer (Operator FM, Wavetable, Meld, Drift, Tension/Collision fysik, Electric) | en subtraktiv synth, 2-op FM, sampler, SF2, trumsyntes | *de* |
| **Ljudeffekter** | 48, bl.a. Hybrid Reverb, Roar, Corpus, Spectral Time/Resonator, Vocoder, Grain Delay, Beat Repeat, Looper, Drum Buss | 20+ (EQ, komp, limiter, multiband, transient, saturator, chorus/flanger/phaser, convolver, steggrind, delay, reverb, de-esser, gate, tape-stop, pitch) | *delvis* |
| **MIDI-effekter** | 11 (Scale, Chord, Velocity, Note Echo, Random, Arpeggiator, MPE Control, Envelope MIDI, Note Length, Pitch) | ackord/arpeggio i synten, `SchedulesChord`, dice | *de* |
| **Tidsoperationer** | Insert/Delete/Duplicate/Cut/Paste Time | ✅ (`insert_time`, `delete_time`, `duplicate_time`) | *de* |
| **Groove** | 220 mallar + extrahera groove ur audio | swing i tagningen | *delvis* |
| **Kompning** | take lanes i spellistan | MIDI-tagningar | *delvis* |
| **Warp** | 6 lägen med granularitet/formanter | WSOLA + bandspelare + formant | *delvis* |
| **Mikrotonalitet** | 152 stämningar, Scala/ASCL, tuning pool | ✅ `.ascl` läses; 12-TET är standard | *de* |
| **MPE** | fullt (per-not pitch bend, MPE Control) | ✗ | *de* |
| **Skript/utbyggnad** | Max for Live + Extensions SDK + Python-API | patcher (intern graf), AI-kommandon | *de* |
| **Kontrollerytor** | 149 | MCU + MIDI-learn | *de* |
| **Lektioner** | 1 016 filer, interaktiva | nej | *de* |
| **Stämseparering** | via `musicai.dll` | Demucs/ONNX + klassisk | *vi* |
| **Projektformat** | `.als` (gzip-XML, ospecificerat) | läsbar JSON med migreringar | *vi* |
| **Plattform** | Windows + macOS | Linux, en binär | *vi* |
| **Testbarhet** | inget `--selftest` | 654 prov, fyra CI-ben, selftest 5/5 | *vi* |
| **Pluginformat** | VST3/VST2/AU | CLAP + VST3 + VST2 + sandlåda | *vi* |
| **Analysfiler** | `.asd` bredvid varje sampling | egen cache | *vi* |

**Räkning:** 8 områden där vi ligger efter, 5 där vi ligger delvis, 6 där vi ligger före.

---

## 9. Vad vi tar först

Samma ordning som för FL: minst kod per stängd lucka, och varje punkt mäts.

1. ~~**Tidsoperationer**~~ — **klara i dag**: `insert_time`, `delete_time`, `duplicate_time` på
   regioner + `shift_song_markers`, fyra prov. Menyn ligger i linjalens kontextmeny och arbetar på
   loopmarkeringen (annars en takt vid spelhuvudet).
2. ~~**Groove-mallar**~~ — **klar i dag**: `parse_agr`/`parse_agr_bytes` läser XML-mallarna,
   `Take::apply_groove` lägger förskjutningen på tagningen (som både uppspelning och export läser).
   Mätt mot Abletons egna filer: Swing 16ths 52 → 0,040 steg, MPC 3000 54 → 0,042 steg.
   Kvar: de 103 binära mallarna, och en pool med flera mallar samtidigt.
3. **Kompning i spellistan** (take lanes) — kräver en lane-modell ovanpå regionerna.
4. **Warp-lägen** — Beats/Tones/Texture/Complex är fyra analysstrategier ovanpå vår WSOLA.
5. **Mikrotonalitet** — ASCL-läsare + oktavdelning i synten.
6. **MPE** — per-not pitch bend kräver att `NoteOn` bär kanal (samma spärr som FL:s VFX-familj).
7. **Skript-API** — den stora: Max/Extensions är Abletons moat.

---

## 10. Gränssnittet — mätt mot vårt

| Mått | Ableton | Sonix | Vad det betyder |
| :--- | ---: | ---: | :--- |
| Teman som filer | **18** (`.ask`, XML) | 1 inbyggt | deras tema är data; vårt är kod |
| Färgvärden i ett tema | **276** poster / 222 namn | 17 konstanter | de kan tona en enda sak (t.ex. `ImplicitArm`) utan att röra resten |
| Ledtrådar (tooltips) | **2 302** `sInfoText*` | 135 `on_hover_text` | deras gränssnitt förklarar sig självt; vårt gör det delvis |
| Menykommandon | **725** | ~200 genvägar/knappar | deras är tangentbordsstyrt hela vägen |
| Fokus-kommandon | **10** (`Focus Arranger/Browser/Clip/Device Chain/Groove Pool/Mixer/Session/Transport/Tuning`) | 0 | de kan navigera mellan paneler utan mus |
| Zoom-kommandon | **~20** (Fit, In/Out Time, Played, Sustain Loop …) | få | deras zoom är adresserbar, inte bara hjul |
| Tillgänglighet | 6 `Accessibility Speak*`-kommandon | **aktiverat i dag** (AccessKit) | skärmläsare fungerar nu hos oss också |
| Ytor | **2 fasta** (Session + Arrangement) + flyttbara detaljpaneler | **12 vyer** (`ViewMode`) | se nedan — den viktigaste skillnaden |

**Den viktigaste skillnaden är inte antalet funktioner, utan modellen.** Ableton har två ytor som
aldrig byts ut, och detaljpanelerna (Clip, Device, Groove Pool, Browser, Tuning) flyttas runt dem.
Sonix byter hela vyn: `ChannelRack`, `PianoRoll`, `PlaylistArranger`, `SessionDrummer`,
`AlchemySynth`, `RemixFx`, `VocalStudio`, `AiMusicAssistant`, `ModularPatcher`, `StemSeparator`,
`PluginManager`, `EffectsMixer`. Tolv lägen betyder tolv sorters muskelminne, och att man tappar
sammanhanget när man går från arrangemanget till mixern. Abletons modell gör att låten alltid är
synlig; vår gör att varje vy är fokuserad men isolerad.

Det går att närma sig utan att kasta om allt: behåll vyerna som **paneler** i stället för sidor —
arrangemanget kvar i bakgrunden, och de fem vanligaste (pianorulle, mixer, clip, browser, patcher)
som utfällbara paneler ovanpå. Det är den enskilt största UX-skillnaden, och den är strukturell
(veckor), inte en knapp.

### Vad som krävs för "acceptabel nivå"

| # | Vad | Storlek | Varför den nivån räcker |
| :--: | :--- | :---: | :--- |
| ~~1~~ | ~~**Paneler i stället för tolv sidor**~~ — **klar i dag**: arrangemanget står kvar, fem vyer kan ligga som panel bredvid (Ctrl+1…5, Esc stänger) | L | den största upplevelseskillnaden |
| 2 | **Fokus + Tab-navigering** mellan arrangemang, mixer, browser, pianorulle | M | kvar: panelerna nås nu med Ctrl+1…5, men Tab-fokus mellan ytorna finns inte |
| 3 | **Temafiler** (JSON) med semantiska namn, 60–80 slots | M | `Theme::`-konstanterna används på **805 ställen**; de måste bli en palett som laddas vid start. Mekaniskt, men 805 rader |
| 4 | **Ledtrådar på varje kontroll** i mixer och arrangemang | M | deras 2 302 mot våra 135 är den tydligaste hjälpklacken; siffran är räknad, inte gissad |
| 5 | **Zoom-kommandon** (fit, in/out tid, valt område) | S | billigt, och deras modell är adresserbar |
| 6 | **18 kontrollerytor av 149** — de 18 vanligaste | M | varje yta är en Python-klass hos dem; vi behöver bara de vanliga |
| 7 | **Inbyggda lektioner** (10 stycken, inte 1 016) | M | undervisning är deras starkaste "gratis" funktion |

Punkt 1–2 är vad som gör att appen *känns* i närheten. Punkt 3–5 är vad som gör att den känns
färdig. Punkt 6–7 är vad som gör att en ny användare stannar.

---

## 10. Gränser för den här mätningen

- **Binärerna är lästa som strängar, inte disassemblerade.** Jag kan citera att Ableton har ett ord
  för en operation och visa exakt hur det stavas, men inte hur operationen är byggd.
- **`.adv`/`.adg`/`.amxd` är räknade, inte tolkade.** Antalet förinställningar per enhet är mätt;
  vilka parametrar de rör är inte läst (det ligger i XML/JSON inuti filerna).
- **Max-runtimens 25 378 filer är inventerade som filer** — 1 000 exempel och 864 externals är
  räknade, men jag har inte läst patchernas innehåll.
- **Ingen körning.** Live är ett Windows-program; inget i det här dokumentet bygger på att ha sett
  programmet arbeta.
