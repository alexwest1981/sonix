# FL Studio 26.1.7 — inventering på molekylnivå

Allt i det här dokumentet är läst ur **installationsfilen själv**
(`flstudio_win64_26.1.7.5654.exe`, 1 055 MiB), inte ur Image-Lines webbsidor eller ur minnet.
Där något inte gick att läsa står det uttryckligen. Siffrorna är räknade, inte gissade.

| Mätning | Värde |
| :--- | :--- |
| Fil | PE32 GUI, Intel i386, 5 sektioner, `FileVersion 26.1.7.0`, 1 106 029 776 byte |
| Kod i exehead | 28 160 B (`.text` 0x6e00) — NSIS-laddaren, ingen musik |
| NSIS | `NullsoftInst` vid 0x10a04, förstaheader vid 0x10a00 |
| Block 0 | 400 987 B komprimerat → **3 519 116 B** (LZMA, `5d 00 00 80 00`, 8 MB ordbok) |
| Payload | LZMA-blockkedja; **125 block, 68 MB** avkodade. `FL64.exe`, `FLEngine_x64.dll`, `FLMManaged.dll`, ASIO-drivern och 17 andra PE-filer ligger i klartext i scratch |
| `7z` | öppnar **inte** filen (PE-detekteringen vinner över NSIS-hanteraren) |
| Kataloger i filtabellen | **1505** |
| Filer i filtabellen | **21 339** poster |
| Läsmetod | NSIS förstaheader → LZMA raw-avkodning → UTF-16-strängar → (katalog, fil)-par |

Tre kompletta referensfiler hör till (§5 och §11): `docs/fl-files.md` (alla 21 339 filposter),
`docs/fl-command-surface.md` (alla 6 256 menyrader) och `docs/fl-parameter-vocabulary.md`
(28 657 etiketter). Inget i dem är ett urval.

Block 0 innehåller installationsskriptet, **licenstexten** (RTF, ~12 500 strängar) och
filtabellen. Filtabellen är det som ger inventariet: varje post är en katalogsträng följd av
sina filnamn, alltså hela trädet utan att något måste gissas.

**Vad som lästes ur binärerna:** `FL64.exe` (22,2 MB), `FLEngine_x64.dll` (3,3 MB) och
`FLMManaged.dll` (9,3 MB) kom ut hela, och strängarna i dem är lästa (§5). Det som *inte* är gjort
är **disassemblering** — ingen kod är avkodad, så inget påstående om hur en plugin låter eller
exakt hur en operation är implementerad bygger på mätning.

---

## 1. Produktstrukturen

Trädet på tre nivåer, med antal filer per nod:

| Nivå | Filer |
| :--- | ---: |
| `Data\Patches` | 16445 |
| `Plugins\Fruity` | 2988 |
| `Data\System` | 989 |
| `System\Tools` | 392 |
| `System\Config` | 111 |
| `Data\Templates` | 72 |
| `Data\Demo projects` | 46 |
| `Help\html` | 45 |
| `Shared\Python` | 35 |
| `Artwork\Wallpapers` | 32 |
| `System\Hardware specific` | 30 |
| `Artwork\Themes` | 26 |
| `System\Texts` | 23 |
| `Shared\Artwork` | 17 |
| `System\Languages` | 14 |
| `DirectWave\Orchestral` | 13 |
| `Image-Line` | 8 |
| `Shared\Temp` | 8 |

### Filändelserna — FL:s egna format

| Ändelse | Antal | Vad det är |
| :--- | ---: | :--- |
| `.fst` | 7796 | FL-preset (kanal, effekt, generator) |
| `.wav` | 3911 | samplad ljudfil |
| `.wv` | 1978 | WavPack-komprimerat sampel (ModeAudio-paketen) |
| `.mid` | 1719 | Standard MIDI-fil (trumloopar, scores) |
| `.tbio` | 715 | Toxic Biohazard-program |
| `.ilv` | 497 | ZGameEditor: HUD-prefab |
| `.fsc` | 398 | **FL-score** — pianorullens frasbibliotek |
| `.mrp` | 300 | Morphine-program |
| `.jpg` | 288 | konstverk/omslag |
| `.png` | 278 | konstverk/omslag |
| `.ogg` | 278 | samplad ljudfil |
| `.zla` | 270 | ZGameEditor: combo (bakgrund/förgrund/process) |
| `.flac` | 269 | samplad ljudfil |
| `.nfo` | 241 | plugin-databaspost / paketinfo |
| `.dll` | 235 | binär (plugin eller bibliotek) |
| `.flms` | 224 | FL Mobile-synthprogram |
| `.fnv` | 220 | **Harmor-envelope/-map** |
| `.ini` | 220 | Dashboard-panelens layout / FL Mobile-instrument |
| `.flp` | 160 | FL-projekt (mallar, demos) |
| `.tga` | 140 | plugin-konstverk |
| `.hdprg` | 98 | Hardcore-program |
| `.scm` | 90 | LilyPond/Guile (notation) |
| `.bmp` | 85 | plugin-konstverk (rattar/paneler) |
| `.txt` | 74 | textdokument |
| `.rtf` | 72 | plugin-info (visas i racket), licenser |
| `.svg` | 67 | konstverk/font |
| `.pyc` | 66 | — |
| `.ttf` | 63 | typsnitt |
| `.ps` | 53 | — |
| `.scr` | 49 | — |

---

## 2. De 141 pluggarna

**51 generatorer** och **90 effekter**. Status mot Sonix i dag: **✅ 88** har en motsvarighet, **◐ 47** delvis, **✗ 0** saknas, **— 6** ej tillämpliga (**65 %** av de 135 som går att jämföra). Siffrorna räknas ur tabellerna, inte ur minnet.


**Statusnycklarna:** ✅ = motsvarighet som gör jobbet · ◐ = halva · ✗ = saknas · **—** = ej tillämplig i en Linux-DAW (mobilapp, dött protokoll, Windows-bunden hårdvara). De sex som är — räknas inte som luckor; de står kvar i tabellen med skälet skrivet, så inget försvinner tyst.
*Statuskolumnen granskad mot `src/` 22 okt 2026, rad för rad: en ✅ bär namnet på
mekanismen i kommentaren. Åtta rader var stale — chorus, phaser, granulizer och vocoder
stod som ✗ trots att §11 listade dem som byggda. Två rader rättade 23 okt: Formula Controller
och Transmitter var byggda men stod kvar som ✗.*

Räknat **per plugin**, inte per värde: en animerad dansare (`Fruity Dance`) väger lika tungt som
`Harmor` i den här räkningen. Vikten står i gap-matrisen i stället (avsnitt 8).

Siffran i kolumn 2 är antalet filer FL levererar i pluginens egen mapp (förinställningar,
konstverk, data) — alltså ett mått på hur mycket innehåll som följer med varje plugin.

### Generatorer (51)

| Plugin | Filer | Sonix | Kommentar |
| :--- | ---: | :---: | :--- |
| FL Studio Mobile | 344 | — | **ej tillämplig**: mobilappen är en egen produkt för iOS/Android. En Linux-DAW kan inte vara den, och Sonix har inget att vinna på att imitera dess pekgränssnitt. |
| Morphine | 199 | ◐ | **fyra operatorkällor** (0.9.137): `OPERATOR_GLASS/REED/BELL/VOICE` och `morph_spectrum4` — en ratt går genom alla fyra, med B och C exakt på tredjedelarna (provat), och rösten bär två formanttoppar (delton 3 och 8). Kvar: egna modulationskedjor och de 199 förvalen |
| Harmor | 164 | ◐ | **bild → spektrum**: `audio::spectral_image` läser en PNG, väger samman alla kolumner och gör bildens nederkant till grundtonen och överkanten till överton 16 (`HARMONIC_COUNT`), normaliserat till 1,0 — knappen **🖼 Från bild…** i dragstångspanelen fyller tabellen. Prov: ljus nederkant → grundtonen 1,0 / toppen 0,0, och tvärtom. Kvar: tidsaxeln (bilden som spelas upp över tid), `.fnv`-envelopes, fler än 16 partialer och blur/prism |
| Dashboard | 90 | ◐ | Patch-modulen ritar en egen graf, men ingen panel med rattar |
| Toxic Biohazard | 78 | ✅ | **6-op-matris** med FM, ringmodulering och additiv blandning (0.9.69/0.9.70), fyra filtertyper, 12/24 dB, envelop med fri tidskurva, två LFO:er, klaviaturskala, pan per operator och unison (0.9.98–0.9.107) |
| GMS | 52 | ◐ | egen subtraktiv synth |
| Wave Traveller | 50 | ◐ | **`AudioCommand::PlayScratch`**: urklippet spelas fram och tillbaka (ping-pong) i vald handhastighet och ett bestämt antal halva pass, så rösten tystnar i stället för att loopa. Prov: efter klippets slut går riktningen bakåt och rösten är tyst när passen är slut. Kvar: LFO/envelope som styr **positionen** löpande (Wave Travellers egen modulering), vinylbrus och flera lager |
| FL Keys | 47 | ✅ | samplern + SF2-läsaren kan ladda piano, keymappen har anslags- och registerzoner, och **sustainpedal (CC64)** finns sedan 0.9.62 — via MIDI och 🎹 PEDAL i verktygsraden. Sedan 0.9.63 håller pedalen även **samplade** noter och dämpar dem på 40 ms. **Pedalresonans** finns sedan 0.9.64 (tre stämda kammar, matade av synt och samplingar). **Pedalen spelas in och följer med till filen** sedan 0.9.65 (16-delars noggrannhet), och **SF2-looparna läses** sedan 0.9.66. **Släppljud per kanal** finns sedan 0.9.68. Raden är därför ✅ på funktionsnivå — ett piano går att spela med pedal, anslag och dämpare |
| Sytrus | 40 | ✅ | **6 operatorer + 6×6-matris** med FM, ringmodulering **och** additivt i samma cell (0.9.69), fyra filtertyper med 12/24 dB (0.9.70/0.9.105), envelop per operator med fri tidskurva (0.9.106), två LFO:er (0.9.100), filter- och tonhöjdsenvelop, klaviaturskala, pan och filterkurva per operator samt unison (0.9.72–0.9.107). Mätt: typisk patch 5,7× realtid, värsta fallet 1,9× |
| Autogun | 26 | ✅ | `instruments::autogun(seed)` — ett spelbart ljud ur ett frö: attacken kläms så noten hörs, filtret så det släpper igenom. Knappen 🎲 i presetlistan |
| Minisynth | 19 | ✅ | synten (4 vågformer, ADSR, SVF, FM) |
| Fruity Slicer 2 | 14 | ◐ | choppern hittar slag och dumpar till piano roll/step |
| FL Slayer | 12 | ◐ | **`Waveform::Guitar`**: strängen från Pluck med plektrumet **en bit in på strängen** — utsignalen läses av på två ställen och differensen släcker de övertoner som har en nod vid plektrumet — plus mjukt rörsteg (tanh). Mätt samma ton (220 Hz): nollgenomgångar 0,0676 → 0,1298 (ljusare), crestfaktor 8,14 → 7,04 (komprimerad), tonhöjd 220 Hz. Kvar: sex strängar med egen stämning, förval och pedaler |
| FPC | 11 | ◐ | drum.rs + steprad; ingen pad-bank med egna lager |
| Fruity Envelope Controller | 10 | ✅ | **Ritbar kurva** som modulationskälla (0.9.135): `ModSource::Curve { rate_hz, points: [f32; 16] }` — sexton punkter, linjärt interpolerade, slutna i varvet och `Copy` så matrisen kan skickas till ljudtråden utan allokering. Ritas med klick/drag i 🕹 Modulationsmatris. Prov: platt kurva är konstant, halvvägs mellan två punkter blir värdet 0,5, varvet sluter till första punkten och en slot sveper med egen takt |
| BassDrum | 9 | ◐ | kick i drum.rs |
| BeepMap | 9 | ✅ | `instruments::beepmap_amplitudes(bytes)` — byte → 16 deltonsnivåer, normaliserade. Verktyg → BeepMap |
| FLEX | 9 | ◐ | **egen förvalsbank**: `audio::flex` sparar reglagen (vågform, ADSR, filter, sub, unison, detune, glide, drive, dragstänger) som namngivna ljud i `~/.local/share/sonix/flex_presets.json`, med ▶-lyssning, Ladda och 🗑 i bläddraren. Tempfil + rename, så en krasch inte lämnar en halv fil; en trasig fil ger tom bank i stället för att appen inte startar. Prov: rundtur bevarar alla parametrar. Kvar: FL:s eget bibliotek och FLEX-specifika vågformer |
| Ogun | 8 | ◐ | `Waveform::Inharmonic` — sex deltoner på 1 / 2,76 / 5,4 / 8,93 / 13,34 / 18,64. Mätt: 607 Hz 0,2027 mot 440 Hz 0,0018. ✗ egna modulations- och effektkedjor |
| Fruity Dance | 5 | ◐ | **💃 Fruity Dance** i menyn: en streckfigur som dansar i takt. Posen räknas i `ui::dance` (två studs per takt, armar i motfas, ben växelvis) och ritas med fem linjer och ett huvud — ingen bild, ingen tråd. Prov: posen är periodisk över en takt, speglar sig mellan kvart och trekvart och håller sig inom rutan. Kvar: FL:s egna dansare och bildsekvenser |
| Wasp | 5 | ◐ | subtraktiv synth |
| Fruity DrumSynth Live | 4 | ◐ | drum.rs (syntetiska trummor) |
| MIDI Out | 4 | ✅ | audio/midi_output.rs (byggd i dag) |
| BooBass | 3 | ◐ | subtraktiv bas |
| DirectWave | 3 | ◐ | sampler + keymap + SF2; ingen egen vågformseditor |
| SimSynth | 3 | ✅ | subtraktiv synth |
| Soundfont Player | 3 | ✅ | audio/sf2.rs (byggd i dag) |
| Wasp XT | 3 | ◐ | subtraktiv synth |
| 3x Osc | 2 | ✅ | synten |
| Fruit Kick | 2 | ◐ | kick i drum.rs |
| Fruity DX10 | 2 | ✅ | 2-op FM (byggd i dag) |
| Fruity Granulizer | 2 | ✅ | `Granular` — granulär motor som mastersteg, sex rattar (korn, täthet, position, spridning, hastighet, mix) |
| Fruity Keyboard Controller | 2 | ✅ | `ModSource::Velocity` och `ModSource::Key` — anslag och tonhöjd centrerad kring C4 |
| Fruity Slicer | 2 | ◐ | choppern |
| Fruity Video Player | 2 | ✅ | `audio::video` + videovy: referensfilm synkad till spelhuvudet, bildrutan hämtas med ffmpeg |
| Harmless | 2 | ◐ | additiv syntes (64 dragstänger sedan 0.9.34, tabellbaserad) plus syntens filter- och envelopkedja — dess två halvor finns, dess egna förval gör det inte |
| Plucked! | 2 | ✅ | `Waveform::Pluck` — Karplus-Strong: brus i en dämpad fördröjningslinje. Mätt: 220 Hz in ger 220,7 Hz ut, RMS 0,1588 → 0,0750 |
| ReWired | 2 | — | **ej tillämplig**: ReWire lades ned 2020 (Propellerhead) och stöds inte av moderna DAW:er. Motsvarigheten i dag är `AudioCommand`-bryggan och VST3, som redan finns. |
| Sakura | 2 | ◐ | **båge** över samma stränglinje som Plucked! (`Waveform::Bowed`): brus exciterar linjen varje varv, så tonen bär medan anslaget hålls, genom kroppens två resonanser. Mätt: 220 Hz in → 220,2 Hz ut, RMS 0,0485 → 0,0647 över 2 s. Kvar: bågtryck/stråkposition och artikulationer (pizzicato, harmonic) |
| Sawer | 2 | ◐ | en oscillator; Sawer är supersaw med 7 röster |
| Slicex | 2 | ◐ | choppern (slag → noter), ingen egen editor |
| Drumaxx | 1 | ◐ | `audio::drum::DrumVoice` — tio syntetiserade röster (kagge, virvel, klapp, tre hi-hats, crash, två tommar, metronom) med tonhöjdsenvelop och brus, triggade per kanal i motorn (`[DrumVoice; MAX_DRUMS]`). ✗ egna modeller per pad och pad-redigering — rösterna är fasta |
| Drumpad | 1 | ◐ | steprad + drum.rs |
| FL Studio Mobile Rack | 1 | — | **ej tillämplig**: mobil-appens rack (se FL Studio Mobile). |
| Fruity Voltage Controller | 1 | ✅ | `ModSource::Voltage { value }` — fast läge, mätt att det står still |
| Kepler | 1 | ◐ | `Preset::Kepler` — en analog röst: sågtand, sub en oktav under, filter med envelop. ✗ CS-80:s egna kretsar modelleras inte, och inget av dess 100+ förval finns |
| Kepler Exo | 1 | ◐ | `Preset::KeplerExo` — pulsvåg, två röster, mer detune. ✗ samma brist som Kepler |
| PoiZone | 1 | ◐ | subtraktiv synth |
| Transistor Bass | 1 | ◐ | filter + synth, ingen 303-modell |
| Fruity Wrapper | 0 | ✅ | CLAP/VST3/VST2-värd + sandlåda |
| Patcher | 0 | ✅ | patcher.rs (nodgraf) |

### Effekter (90)

| Plugin | Filer | Sonix | Kommentar |
| :--- | ---: | :---: | :--- |
| ZGameEditor Visualizer | 1423 | ◐ | **ZGameEditor-lite**: `ui::visualizer` ritar vågformen ur `scope_history` och spektrumstaplar ur `spectrum_levels` — samma mätning som mixern använder, ingen andra FFT — med toppmarkör som faller lugnt. **🌊 Visualizer...** i menyn. Prov: markören lyfts till nivån och faller ett steg per bildruta, aldrig under nivån; tysta band får en synlig linje. Kvar: 1 423 GPU-förval, lager, text/3D-scener och videoexport |
| Hardcore | 42 | ✅ | `Hardcore` — pedalkedja: komp → drive → wah → delay, byggd av masterkedjans egna kretsar; de 98 `.hdprg`-programmen är inte lästa |
| Distructor | 37 | ✅ | `Distructor` — tre kurvor (mättnad, klippning, vikning) i serie eller parallellt, tonläge efter |
| Fruity Love Philter | 23 | ✅ | `LovePhilter` (0.9.125): tre filter i serie — lågpass under, bandpass i mitten, högpass över — kring en gemensam brytfrekvens, med SPREAD i oktaver och en envelopföljare som flyttar banken upp till fyra oktaver. Prov: 1 kHz passerar medan 8 kHz dämpas; envelopen öppnar banken och stänger sig i tystnad. De 43 förvalen är inte lästa |
| Pitcher | 22 | ◐ | vocal_harmonizer med formantbevarande pitch |
| Edison | 20 | ✅ | `ui::edison_modal` (0.9.127, spektrum 0.9.140): vågform med dragbart urval, sju operationer och **spektrum över urvalet** (48 logaritmiska band, 40 Hz–16 kHz, −60…0 dB). Redigerar en **kopia** och lägger resultatet som ett nytt spår — källfilen rörs aldrig. Kvar: loopverktyg och destruktiv redigering på plats |
| Maximus | 15 | ✅ | multibandskompressorn är **tre band med egen tröskel per band** (0.9.138) och nu **master-bandet** (0.9.149): en kompressor över summan, efter banden — FL:s MASTER-knapp. Prov: med alla band på 0 sänker master summan till under 75 %. **Kvar:** ritbara volymkurvor per band och solo per band. *Rättelse: raden sa "5 band" — Maximus har LOW/MID/HIGH + MASTER, inte fem band.* |
| Vocodex | 13 | ✅ | `vocode` (16 band, 100 Hz–8 kHz, bärvågen följer sången) och masterns vocoder har nu **bandredigeraren** (0.9.143): 16 nivåreglage, 0–2, i FX-racket under fliken 🎚 Vocoder, sparade i projektet. Prov: ett stängt band släpper igenom under 35 % av det öppna. Kvar: FL:s egen bandlayout och formantskifte per band |
| Soundgoodizer | 5 | ◐ | saturator + kompressor |
| Fruity Convolver | 4 | ✅ | convolvern (byggd i dag) |
| Fruity HTML NoteBook | 4 | ◐ | textanteckningar per kanal finns; HTML-rendering i anteckningarna gör det inte |
| Fruity Peak Controller | 4 | ✅ | `ModSource::Env` — envelopföljare på masternivå, snabb upp/långsam ner |
| Fruity Scratcher | 4 | ◐ | samma maskin som Wave Traveller: **🖐** i samplebläddraren scratchar samplet (0,7×, sex halva pass). Prov: `a_scratch_ping_pongs_and_stops_after_its_passes`. Kvar: riktig vinylbrusmodell, handrörelsens kurva och scratch-mönster |
| Control Surface | 3 | ◐ | hardware_control.rs (MCU) |
| Fruity Formula Controller | 3 | ✅ | `src/formula.rs` + `ModSource::Formula` — formel som modulationskälla (`11532ea`) |
| Fruity NoteBook | 3 | ✅ | `PlaylistTrack::notes` — ett anteckningsfält per kanal i kanalinställningarna, `#[serde(default)]` så äldre projekt öppnas med tomma. Prov: `channel_notes_are_empty_in_old_files_and_survive_a_round_trip` |
| EQUO | 2 | ◐ | EQ:n |
| Effector | 2 | ✅ | `Effector` — XY-yta över fyra lägen (svep, upprepning, port, kross); X och Y styr respektive krets |
| Frequency Splitter | 2 | ◐ | crossovern i multibandskompressorn |
| Fruity Big Clock | 2 | ✅ | `ui::big_clock` + **⏱ Stor klocka** i menyn: takt.slag.steg i 72 px och spelad tid (`m:ss.mmm`) i 40 px, räknat ur `current_step` och tempot. Prov: steg 16 vid 120 BPM → `2.1.1` / `0:02.000` |
| Fruity Delay 2 | 2 | ✅ | StereoDelay |
| Fruity Delay Bank | 2 | ✅ | `DelayBank` — tre linjer med egen tid och nivå, återkoppling, stereospridning |
| Fruity Fast Dist | 2 | ✅ | saturatorn |
| Fruity Flangus | 2 | ✅ | `ModDelay` som flanger (2,5–4,5 ms); ingen fler-röstsvariant |
| Fruity LSD | 2 | — | **ej tillämplig**: DirectMusic är Windows-specifikt och dessutom avvecklat. Motsvarande behov täcks av MIDI-ut och VST3. |
| Fruity Limiter | 2 | ✅ | Limiter |
| Fruity Multiband Compressor | 2 | ✅ | multibandskompressorn (byggd i dag) |
| Fruity PanOMatic | 2 | ◐ | pan + volym; ingen LFO-pan |
| Fruity Parametric EQ | 2 | ✅ | StereoEq / TrackEqSettings |
| Fruity Parametric EQ 2 | 2 | ✅ | 4 band + spektrum i FX-racket |
| Fruity Reeverb 2 | 2 | ✅ | SimpleReverb |
| Fruity Send | 2 | ✅ | sends per spår |
| Fruity Soft Clipper | 2 | ✅ | saturatorn |
| Fruity Spectroman | 2 | ✅ | spectrum.rs + mätarna |
| Fruity Squeeze | 2 | ✅ | `BitCrusher` — BITS 1–16, NEDSAMPLING 1–32, TON (lågpass efter krossen) |
| Fruity Stereo Enhancer | 2 | ◐ | doubler/width |
| Fruity Stereo Shaper | 2 | ✅ | `StereoParams::apply` — bredd på sidan (mitten står still), balans som viktning |
| Fruity Vocoder | 2 | ✅ | `vocode` i sångstudion, läggs som egen tagning |
| Fruity WaveShaper | 2 | ✅ | saturatorn |
| Fruity Wrapper | 2 | ✅ | plugin-värden |
| Fruity X-Y Controller | 2 | ✅ | två spännings-slots på olika mål — en pad är två rattar |
| Fruity dB Meter | 2 | ✅ | nivåmätarna |
| Gross Beat | 2 | ✅ | steggrinden (volymkurvan), RemixFX och **⏱ TIME CURVE** (0.9.142): masterns egen historik läses tillbaka enligt en 16-punkters tidskurva — stutter, tape stop och halv fart, med tonhöjden följande. Prov: halv fart ger 110 Hz ur 220, rak kurva är genomgång, fryst kurva håller ljudet stilla. Bara bromsa, inte skynda: 2× kräver en egen inspelningsbuffert. Kvar: ritbara kurvor och 2×-läget |
| Newtone | 2 | ✅ | `ui::newtone_modal` (0.9.128): kromatisk analys till toner, klick för att välja och ±1 halvton per ton (eller hela frasen), formantbevarande omskrivning via `pitch_shift_variable`. Prov: 220 Hz + 2 halvton = 247 Hz. Arbetar på en kopia → nytt spår. Kvar: dra toner fritt i tid, vibratoverktyg |
| Patcher | 2 | ✅ | patcher.rs |
| Spreader | 2 | ◐ | doubler |
| Wave Candy | 2 | ✅ | **🌊 Visualizer** (0.9.134): vågform, spektrumstaplar med toppmarkör och en **goniometer** — skopet bär nu `(mid, side)` per sampel, så L/R ritas som en roterad punkt; mono blir lodrät, ur fas vågrät. Prov: mono ger x = 0, ur fas y = 0, och paret håller ihop även när ringen lämnar ett udda tal |
| Emphasis | 1 | ✅ | **✨ EMPHASIS** i masterns kedja (0.9.131): högpassat band (1–10 kHz) klipps asymmetriskt, så **både** jämna och udda övertoner föds, och bara det tillförda blandas in (DC-blockat). Prov: 200 Hz in ger 400 och 600 Hz med >26 dB över torrt, grundtonen kvar |
| Emphasizer | 1 | ✅ | Samma motor som Emphasis (`audio::master_fx::exciter`): övertonstillägget finns nu, DRIVE och FREQ styr det |
| FL Studio Mobile Rack FX | 1 | — | **ej tillämplig**: mobil-appens effekter (se FL Studio Mobile). |
| Frequency Shifter | 1 | ✅ | `FrequencyShifter` — Hilbert 63 tapp, en sidofrekvens, ±2 000 Hz, stereoläge |
| Fruity 7 Band EQ | 1 | ✅ | 0.9.123: masterns grafiska EQ har sju band (60/170/350/1k/3,5k/8k/12k), prov att 1 kHz-bandet lyfter 1 kHz men inte 8 kHz |
| Fruity Balance | 1 | ✅ | pan/volym |
| Fruity Bass Boost | 1 | ◐ | EQ |
| Fruity Blood Overdrive | 1 | ✅ | saturatorn |
| Fruity Center | 1 | ✅ | 0.9.123: `StereoParams.center` (−1 mitten bort, +1 sidorna bort), prov på mono, sida och lika kanaler |
| Fruity Chorus | 1 | ✅ | `ModDelay` — svep 12–20 ms, kvarts periods kanalskillnad, mix 0 bit-identisk |
| Fruity Compressor | 1 | ✅ | Compressor |
| Fruity Delay | 1 | ✅ | StereoDelay |
| Fruity Delay 3 | 1 | ✅ | StereoDelay (ping-pong) |
| Fruity Fast LP | 1 | ✅ | filtret |
| Fruity Filter | 1 | ✅ | filtret |
| Fruity Flanger | 1 | ✅ | `ModDelay` med flanger-inställning |
| Fruity Free Filter | 1 | ✅ | filtret |
| Fruity Mute 2 | 1 | ✅ | mute/solo |
| Fruity NoteBook 2 | 1 | ✅ | samma fält (FL har två versioner av samma sak) |
| Fruity Phase Inverter | 1 | ✅ | `StereoParams::apply` — polaritet per kanal, exakt invertering |
| Fruity Phaser | 1 | ✅ | `Phaser` — 4 allpass, 200 Hz–2 kHz, stilla vid djup 0 |
| Fruity Reeverb | 1 | ✅ | SimpleReverb |
| Fruity X-Y-Z Controller | 1 | ✅ | tre spännings-slots; samma mekanik som X-Y |
| Hyper Chorus | 1 | ✅ | **🌊 HYPER CHORUS** (0.9.133): fyra `ModDelay`-röster med egna basfördröjningar (9/13,5/19/26 ms) och LFO-faktorer (×1,0/1,13/0,87/1,27), panorera(de ut i sidorna och en WIDTH som kan göra utfallet monokompatibelt. Prov: mono in ger sidmaterial (bredd), WIDTH 0 ger L == R exakt, MIX 0 är ren genomgång, och toppen stannar under full skala |
| Low Lifter | 1 | ✅ | `ShelvesParams` — nedre hyllan (RBJ), samma krets som tonkonturen |
| Luxeverb | 1 | ✅ | `Luxeverb` (0.9.126): modulerat fyrlinjigt FDN-reverb med dämpning i återkopplingen, RT60-styrd DECAY, PREDELAY och WIDTH. Prov: svansen faller, DECAY styr längden, PREDELAY håller tyst. Det gamla Schroeder-reverbet finns kvar som send |
| Multiband Delay | 1 | ✅ | `MultibandDelay` — tre band, tre tider, samma delningsfilter som den flerbandiga kompressorn |
| Newtime | 1 | ✅ | `ui::newtime_modal` + `audio::newtime` (0.9.129): tidsankare (källa → uttid) med dragbara markörer, WSOLA-sträckning per segment och rendering till ett nytt spår. Prov: oförändrad tid = bitidentisk, flyttat slut ändrar längden med tonhöjden kvar, korsade markörer kläms. Kvar: markörer direkt på tidslinjen i stället för i en kopia |
| Pitch Shifter | 1 | ✅ | formantbevarande pitch (stretch/vocal_harmonizer) |
| Razer Chroma | 1 | — | **ej tillämplig**: Razer Chromas SDK är Windows-bundet och datorn har Corsair K60 (styrs via `corsair-rgb-lighting`). Att måla någon annans tangentbord är inte en DAW-funktion. |
| Transient Processor | 1 | ✅ | transient-shapern (byggd i dag) |
| Transmitter | 1 | ✅ | data → ljud: händelser, förval och wav (`ade2c1a`) |
| Transporter | 1 | ✅ | samma fönster som Big Clock — FL:s äldre variant visar samma två tal |
| Tuner | 1 | ✅ | tuner_modal |
| VFX Color Mapper | 1 | ◐ | `ui::note_color` + **🎨 Färg** i pianorullen: tolv färger, en per tonklass, så samma ton har samma färg i alla oktaver (C ljusast, ledtonerna dämpade). Kvar: färg → parameter |
| VFX Envelope | 1 | ✅ | `ModSource::Adsr` — attack/decay/sustain/release som modulationskälla, med `ModMatrix::note_off` kopplad till `AudioCommand::NoteOff`. Prov: 0,50 → 0,89 → 0,50 (sustain), och under 0,05 efter not-av |
| VFX Key Mapper | 1 | ◐ | `audio::key_mapper` — en zon transponerar noten och kan byta kanal/skala anslaget; första träffen vinner. Not → **parameter** går via modulationsmatrisen (`ModSource::Key` → volym/panorering/tonhöjd/cutoff), men inte per zon som i FL |
| VFX Keyboard Splitter | 1 | ✅ | `audio::key_mapper` — fyra zoner över klaviaturet, varje zon kan skicka noten till egen MIDI-kanal (1–16), transponera och skala anslaget. Följer med projektfilen. Prov: zon 36–47 → kanal 2, +12, ×0,75 ger (52, 60, 2) |
| VFX Level Scaler | 1 | ✅ | anslaget skalas per zon (`velocity_scale` 0–2×) **och** formas av en kurva (0,2–5: under 1 lyfter svaga slag, över 1 trycker ner dem), klämt till 1–127 så en svag not aldrig blir note-off. Prov: 64 med kurva 0,5 → 90; med kurva 2,0 → 32 |
| VFX Script | 1 | ◐ | **VFX Script (lite)**: formelspråket i `formula.rs` blir en automationskurva — **📜 Skript...** i menyn, formel (`t` = takter), mål och **✍ Skriv kurvan** skriver 16 punkter per takt i spårets bana, som sedan går att rätta för hand. Prov: `t` över två takter ger 8 punkter 0,5 → 1,0, dålig syntax ger ett läsbart fel. Kvar: egen skriptmotor med tillstånd, händelser och GUI-bygge |
| VFX Sequencer | 1 | ◐ | SchedulesChord (ackord/arpeggio) + dice-generator |
| Vintage Chorus | 1 | ◐ | samma `ModDelay`; "vintage" är FL:s egen röstning, inte en egen motor |
| Vintage Phaser | 1 | ◐ | samma `Phaser`; ingen egen kurva för den varianten |

---

## 3. Innehållet som följer med

**Plugin-förinställningar** — 7993 filer

| Plugin | Förinställningar |
| :--- | ---: |
| Generators\Drumpad | 959 |
| Generators\Sytrus | 797 |
| Generators\Toxic Biohazard | 715 |
| Generators\Sakura | 481 |
| Generators\Harmless | 390 |
| Generators\Sawer | 390 |
| Generators\PoiZone | 384 |
| Generators\Harmor | 326 |
| Generators\Morphine | 300 |
| Effects\ZGameEditor Visualizer | 290 |
| Generators\Kepler | 273 |
| Effects\Fruity Convolver | 259 |
| Generators\Ogun | 210 |
| Effects\LuxeVerb | 163 |
| Generators\Patcher | 150 |
| Generators\Kepler Exo | 149 |
| Effects\Patcher | 135 |
| Generators\Drumaxx | 130 |
| Effects\Hardcore | 98 |
| Generators\MiniSynth | 91 |
| Effects\VFX Script | 86 |
| Generators\FL Slayer | 64 |
| Generators\SimSynth | 59 |
| Generators\Fruity DX10 | 56 |
| Generators\BassDrum | 52 |
| Effects\Distructor | 46 |
| Generators\Wasp | 45 |
| Effects\Fruity Love Philter | 43 |
| Generators\Wasp XT | 40 |
| Generators\MIDI Out | 38 |

**Ljudpacks (egna samplingar)** — 5135 filer i 10 grupper

| Grupp | Filer |
| :--- | ---: |
| Drums | 1908 |
| Drums (ModeAudio) | 1642 |
| Legacy | 926 |
| Instruments | 489 |
| Vocals | 47 |
| Shapes | 41 |
| SFX | 36 |
| Risers | 23 |
| Loops | 15 |
| FLEX | 8 |

**Scores — pianorullens frasbibliotek** — 2111 filer i 7 grupper

| Grupp | Filer |
| :--- | ---: |
| FPC drumloops | 1718 |
| Chopping | 206 |
| Quantization | 82 |
| Arpeggiator | 42 |
| Reference scales | 36 |
| Stamps | 26 |
| Automation | 1 |

**Impulsresponser (Fruity Convolver)** — 260 filer i 13 grupper

| Grupp | Filer |
| :--- | ---: |
| Reverb Devices | 41 |
| Halls | 35 |
| Chambers | 33 |
| Exteriors | 30 |
| Guitar Cabinet | 20 |
| Underground | 20 |
| Real Spaces | 17 |
| Small Rooms | 17 |
| Spring Reverb | 16 |
| Corridors | 13 |
| FX | 12 |
| Special | 5 |
| Sampled | 1 |

**Kanalpresets** — 245 filer i 8 grupper

| Grupp | Filer |
| :--- | ---: |
| 3x Osc | 169 |
| Automation clips | 57 |
| BeepMap | 7 |
| Various | 6 |
| Fruity Slicer | 2 |
| Plucked! | 2 |
| Sampler | 2 |
| Useful | 0 |

**Mallar (Templates)** — 72 filer i 7 grupper

| Grupp | Filer |
| :--- | ---: |
| Utility | 24 |
| Advanced | 23 |
| Minimal | 8 |
| Other | 8 |
| Performance | 6 |
| Power User | 2 |
| Empty | 1 |

**Demoprojekt** — 45 filer i 4 grupper

| Grupp | Filer |
| :--- | ---: |
| Demo songs | 20 |
| Product demos | 10 |
| Visual | 9 |
| Performance mode | 6 |

**System** — 933 filer i 8 grupper

| Grupp | Filer |
| :--- | ---: |
| Plugin databases | 374 |
| FL Mobile | 325 |
| MIDI import | 137 |
| Render tests | 80 |
| Riff machine | 7 |
| Default | 6 |
| icon.ico,0 | 4 |
| Articulator | 0 |

**System/Config — inställningar som är filer** — 104 filer i 12 grupper

| Grupp | Filer |
| :--- | ---: |
| Thumbnails | 37 |
| Typing to piano | 32 |
| Toolbar presets | 11 |
| Browser | 10 |
| Touch keyboard | 6 |
| Mapping | 3 |
| Note color presets | 2 |
| Audio scripts | 1 |
| Master | 1 |
| Piano roll scripts | 1 |
| FL Studio Remote Scripts | 0 |
| Screenshots | 0 |

**Hårdvarumappningar (kontrollerytor)** — 30 filer i 30 grupper

| Grupp | Filer |
| :--- | ---: |
| Livid OhmRGB | 7 |
| Akai APC | 5 |
| Behringer BCF2000 | 3 |
| Roland PCR | 3 |
| Numark Orbit | 2 |
| Akai FL Studio Fire | 1 |
| DJ TechTools Midi Fighter 3D | 1 |
| Electrix Tweaker | 1 |
| Forward CCs to current plugin Port 10 | 1 |
| Native Instruments MASCHINE | 1 |
| Native Instruments MASCHINE JAM | 1 |
| Native Instruments MASCHINE MIKRO | 1 |
| Native Instruments TRAKTOR KONTROL F1 | 1 |
| Novation Launchkey MK2 | 1 |
| Novation ReMOTE | 1 |
| Behringer CMD LC-1 | 0 |
| Keith McMillen QuNeo | 0 |
| Korg NanoPAD | 0 |
| Korg NanoPAD 2 | 0 |
| Korg nanoKONTROL | 0 |
| Korg nanoKONTROL 2 | 0 |
| Livid Base | 0 |
| Livid Block | 0 |
| Livid CNTRL-R | 0 |
| Nektar Impact series | 0 |
| Novation Launch Control XL | 0 |
| Novation Launchkey | 0 |
| Novation Launchpad | 0 |
| Novation Launchpad MK2 | 0 |
| Novation Launchpad Pro | 0 |

### Det som är värt att stanna vid

- **0 `.mid` + 0 `.fsc`** i `Scores`: FL skeppar ett frasbibliotek (trumloopar, chopping, kvantisering, arpeggiomönster, referensskalor, "stamps") som pianorullen kan hämta. Det är innehåll, inte kod — men det är därför FL känns fullt av idéer direkt efter installation.
- **0 impulsresponser** i 13 kategorier (hallar, kammare, utomhus, gitarrlådor, underjord, fjäderreverb, korridorer, FX). Vår convolver (byggd i dag) har ingen medföljande bank — den kan bara ladda en fil användaren pekar ut.
- **0 touch-klaviatur, 0 verktygsradspresets, 0 notfärgspresets, 0 skript/layouter** — gränssnittet är konfigurerbart i filer, inte bara i dialoger.
- **13 kontrollerytor** har egna mappningar: Akai APC, Akai FL Studio Fire, Behringer BCF2000, DJ TechTools Midi Fighter 3D, Electrix Tweaker, Livid OhmRGB, MASCHINE (3 varianter), TRAKTOR KONTROL F1, Novation Launchkey MK2, Novation ReMOTE, Numark Orbit, Roland PCR.
- **Bilderna finns med**: `.tga`/`.bmp` per plugin (rattar, paneler, bakgrunder), `.png`-omslag för plugin-databasen. Varje plugin är också en liten designprodukt.

---

## 4. Undersystemen — det som inte är plugins

Här ligger mycket av det som gör FL till FL, och det syns bara i filstrukturen:

| Undersystem | Bevis i filen | Vad det är |
| :--- | :--- | :--- |
| **Piano roll-skript** | `System\Config\Piano roll scripts\Piano roll script reference.txt` | ett skript-API för pianorullen (användarskript, inte bara inbyggda verktyg) |
| **Ljudskript** | `System\Config\Audio scripts\{Amp,FX,Generation,Regions,Reference.txt}` | skriptbara ljudverktyg i fyra kategorier |
| **Browser-snapshots** | `System\Config\Browser\Snap1-3.scr` | sparade sökningar/filter i browsern |
| **Tangentbord → piano** | `System\Config\Typing to piano\Layout {Piano,Janko,FPC (C4),Slicex (C4),Performance mode}.scr` + förklarande txt | fem layouter; Janko är en alternativ klaviaturlayout |
| **Touch-klaviatur** | `System\Config\Touch keyboard\*.tkp` (6) | Keys, 2 rader, Pads 4×4, Pads 8×4, FPC |
| **Verktygsraden** | `System\Config\Toolbar presets\*.tpr` (6) | Compact, Default, macOS, FL 12, FL 24 |
| **Notfärger** | `System\Config\Note color presets\*.ncp` (2) | Default, High contrast |
| **Chord Progression** | `System\Tools\Chord progression\**` (51 filer, Python `.pyc`) | ackordgenerator med **Markov-kedjor** (`States\markov.pyc`), stiltabeller per genre (Afrobeats, Amapiano, Boom bap, D&B, Drill, Dubstep, Lo-fi …), samt moduler för baslinje, trummönster, arpeggion och "chopper" |
| **Riff machine** | `Data\System\Riff machine\*.fst` (7) | melodi-/riffgenerator med förval |
| **Notation** | `System\Tools\LilyPond\**` (318 filer) | LilyPond + Guile + Ghostscript: notutskrift inbyggd i DAW:n |
| **BeatSlicer (äldre)** | `System\Tools\BeatSlicer\zx_bs_d.exe` + 8 `.zco`-skal | den gamla fristående slicern, kvar för bakåtkompatibilitet |
| **Plugin-brygga** | `System\Tools\Bridge\{32bit,64bit}\ilbridge.exe` | 32↔64-bits brygga för gamla plugins |
| **Kontrollpanelsskapare** | `System\Tools\Control Creator\ControlCreator.exe` | gör egna paneler (Dashboard) |
| **MCP** | `System\Tools\MCP\MCPTools.pyc` | en MCP-brygga i FL självt (AI-verktygsåtkomst) |
| **MIDI-import** | `Data\System\MIDI import\*.fst` (137) | General MIDI → kanalpresets, alltså en mappningstabell för importerade `.mid` |
| **Plugin-databaser** | `Data\System\Plugin databases\**` (374) | kategoriserade listor med `.nfo` per plugin |
| **Rendertester** | `Data\System\Render tests\**` (80) | egna regressionstester för ljudrenderingen |
| **Video** | `avcodec/avformat/avfilter/avutil/swscale/swresample` (ffmpeg 58/56/7/5) | videoavkodning i projektet |
| **Videout** | `Processing.NDI.Lib.x64.dll`, `SpoutLibrary.dll` | NDI och Spout — bild ut till andra program |
| **Stämseparering** | `Licenses\demucs.txt` | Demucs (samma familj som vår `neural_separator`) |
| **Inbäddad Python** | `Shared\Python\**`, `python312.dll`, `libpython2.4.dll` | skriptmotor i både FL och LilyPond |
| **Razer Chroma** | `Razer Chroma_x64.dll` | tangentbords-RGB som svarar på musiken |
| **WebView2** | `WebView2Loader.dll` + `MicrosoftEdgeWebView2RuntimeInstallerX64.exe` | HTML-paneler (bl.a. HTML NoteBook) |
| **ASIO-driver** | `ILWASAPI2ASIO{,_x64}.dll`, `registerAsioDriver.bat` | egen ASIO-brygga över WASAPI |
| **VST-version av FL** | `FL Studio VSTi.dll`, `FL Studio VSTi (Multi).dll` | FL kan köras som plugin i en annan DAW |
| **ReWire** | `ReWire.dll`, `ReWired_x64.dll`, `FLReWire_x64.dll` | ReWire-värd |
| **Mobil-racket** | `FL Studio Mobile_x64.dll`, `Data\System\FL Mobile\**` (325) | mobilappens instrument och effekter i racket |
| **FL Cloud** | `System\Installers\FL Cloud Plugins Installer.exe` | molnpluggar installeras vid sidan om |
| **Diagnostik** | `System\Tools\Diagnostics\fldiagnostic.{exe,ini}` | felsökningsverktyg |

---

## 5. Kommandoytan — vad appen kan göra, inte vad den skeppar

Filnamnen visar plugins och innehåll. De visar inte **operationerna**. Därför lästes strängarna
ur `FL64.exe` (block 31 i payloaden, 22,2 MB), `FLEngine_x64.dll` (block 43, 3,3 MB) och
`FLMManaged.dll` (block 39, 9,3 MB):

| Mätning | Värde |
| :--- | ---: |
| ASCII-strängar i `FL64.exe` | **225 791** |
| UTF-16-strängar i `FL64.exe` | **16 231** |
| Meny-/kommandoliknande rader (filtrerade) | **6 256** |
| ASCII-strängar i `FLEngine_x64.dll` | 24 964 |

Det är **strängar, inte disassemblerad kod**: jag kan visa att appen har ett ord för en operation
och citera det, men inte hur den är implementerad. Raderna nedan är hämtade ordagrant.

| FL-operation | Bevis (ordagrant ur binären) | Sonix |
| :--- | :--- | :--- |
| Bounca spår till audio | `Consolidate this track` · `Consolidate track(s)` · `Select a folder to consolidate to` | ✅ `bounce_track_to_new`: renderar spåret offline till 32-bitars float i projektets `Bounces/`, läser tillbaka filen och lägger den på ett **nytt** spår, medan källan tystas — så klippet finns kvar och går att flytta |
| Spela in allt du spelar i en logg och dumpa den | `Auto dump score` · `Dump score log` · `Convert to score and dump to piano roll` · `Detecting score...` | ✗ (tagningar spelas in per tagning, inte som en logg) |
| Dela klipp på taktdelar | `Chop in bars` · `Chop in 8th beats` · `Chop in 16th beats` · `Chop in 32nd beats` · `Chop chords` | ✅ 0.9.122: `chop_region` delar klippet i 1/½/¼ takt från klippmenyn; prov på kontinuitet, ljudoffset och id:n |
| Auto-slice med känslighet | `Auto-slice with dull/medium/sharp sensitivity` · `Accentuate beat-aligned slices` | ✅ 0.9.122: `autoslice_selected_region` läser klippets PCM, hittar anslagen med `detect_onsets` och delar där (känslighet 1,5; kräver inläst ljud) |
| Extrahera groove | `Flatten (groove)` · `Keep the groove only` · `Load groove pattern...` · `Save groove pattern...` | ✅ (extraktion; `Flatten`/`Keep` är inte byggda) |
| Strumma ackord | `Bypass strum` | ◐ (strum endast i Chord Matrix-avlyssningen) |
| Artikulation per not | `Articulate...` | ✗ |
| Arpeggion som verktyg | `Arpeggiate...` | ◐ (arpeggio i synten + `SchedulesChord`) |
| Skala nivåer | `Scale levels...` | ✅ 0.9.122: `Scale levels` som −3/+3 dB på valt klipp (`scale_region_volume`, klämt 0–2) |
| Tidsförskjuta audio fritt | `Edit in time warper` | ✅ 0.9.129: Newtime — ankare (källa, uttid) med WSOLA mellan, markörerna flyttas fritt; `open_newtime_for_selected_region` |
| Klicka bort skarvar | `Declick in` · `Declick out all regions` · `Bypass declicker` | ◐ `audio::repair::declick` lagar hopp större än 0,25 över 32 sampel, och knappen **🔧 Klicka bort skarvar** kör den på spåret. ✗ FL:s läge per region (bara in, bara ut) och en bypass |
| Automatiska övergångar | `Automatic crossfades` · `Create crossfade` · `Crossfade (bleeding)` | ◐ `PlaylistTrack::create_crossfades` sätter ut-toning på det tidigare och in-toning på det senare klippet, lika långa som överlappet (halverat om överlappet är större än klippet), och knappen **✚ Korsa överlappande klipp** i fade-fönstret kör den. ✗ `Create crossfade` på ett urval, och `bleeding`-varianten utanför regionens kant |
| Byta källa för ett klipp | `Pattern clip sources` · `Automation clip sources` · `Current clip source` | ✅ | `swap_track_source` — nytt ljud, samma volym, pan, EQ, sends, inserts, automation och placering; vågformscachen nollas så den byggs om |
| Spågrupper i spellistan | `All playlist track groups` · `Export all playlist track groups` · `Collapse all grouped tracks` | ◐ | `TrackGroup` + `track_hidden`: en ihopfälld grupp ritar bara sitt första spår, provat. ✗ exportera grupper som egna filer, grupperna sparas inte i projektet |
| Tagningar vid loopinspelning | `Group loop recorded takes` · `Ignore silent takes` · `Pickup (takeover mode)` | ◐ (MIDI-tagningar med kompning) |
| Markörer med egen funktion | `Add audio time marker` · `Add key marker` · `Change to marker loop` | ◐ (regioner, men ingen markörloop) |
| Taktartsbyten | `Add time signature change...` · `Import time signatures` · `Beats per bar` | ◐ (taktart finns, byten i tempokartan oklart) |
| Rendering i bakgrunden | `Background rendering` · `HQ RENDERING` · `Don't render the tail (rendering loops)` | ✗ (export blockerar) |
| Exportera mixern som separata spår | `Split mixer tracks` | ✅ (`export_stems`, 58 filer nämner stämmor) |
| Fler exportformat | `Export Flac` · `Export M4a` · `Export MP3` · `Export OGG` | ✅ (wav/flac/mp3/m4a) |
| Stänga av oanvända plugins automatiskt | `Smart disable` · `Switch smart disable for all plugins` | ✗ (vi har sandlåda och frysning i stället) |
| Räkna underrun | `Underruns 0` | ✅ (`realtime_bench`) |
| Gör om allt till en patch | `Patcherize all` · `Patcherizing will lose automation links` | ✗ (patcher finns, ingen massomvandling) |
| Redigera vidare i Edison | `Open in Edison` · `Edison Declipper` | ◐ 0.9.127: `open_edison_for_selected_region` — högerklick på klippet → *Redigera i Edison*, arbetar på kopia → nytt spår. Declipper saknas (finns bara `repair::declick`) |
| Slumpa alla parametrar | `Randomize all parameters` · `Randomize bands` | ◐ (tärning i pianorullen, inte per plugin) |
| NDI ut | `Enable NDI output` · `Add NDI` | ✗ |
| Molnbackup av projekt | `Back up to FL Cloud storage` · `Download project backup` | ✗ (lokala versioner i `autosave.rs`) |
| Förmätning före inspelning | `Is precount enabled` · `Recording precount` | ✗ (metronom finns) |
| Alternativ ångringsmodell | `Alternate undo mode` · `Enable undo for large samples` | ◐ (ångring finns, en modell) |
| Analysera klang för åter syntes | `Analyze timbre...` · `Analyze single-cycle waveform...` (Harmor) | ✅ `audio::timbre` — FFT ur samplingen → 64 partialer; knappen sitter i dragstångspanelen (⚙ fliken) |
| Riff-generator | `Riff machine...` · `Piano roll - riff machine` | ◐ | `src/riff.rs` — fras ur fyra skalor, repeterbar med samma frö, inom rullens omfång. ✗ FL:s riff-maskin har flera faser (pattern, generera, verktyg) |
| Ackordgenerator | `Generate chord progression...` · `Chord Progression Tool opened` | ◐ (`chord_generator_modal`, utan Markov) |

Hela ordlistan — 28 657 korta etiketter (rattar, reglage, kolumnrubriker) — ligger i
`docs/fl-parameter-vocabulary.md`, och alla 6 256 menyrader i `docs/fl-command-surface.md`.
Tabellen ovan är urvalet som namnger *funktioner*.

**Vad det här avsnittet ändrar:** åtta av operationerna ovan är *sådant FL gör utan en plugin* —
`Consolidate`, score-loggen, groove-extraktion, `Patcherize`, `Split mixer tracks`, bakgrundsrendering,
declick och korsningar. De syns inte i filtabellen, hur noggrant man än räknar den.

---

## 6. Installationsprogrammet

Sådant som syns i skriptet och som säger något om hur produkten är byggd:

- **Flera mål**: `\FL.exe` (32-bit) och `\FL64.exe`, med `FLEngine_x64.dll` bakom — motorn är en egen DLL, inte appen.
- **Skrivningar i registret**: `Software\Image-Line\Shared\Paths`, `Software\SoftVoice\ImageLine`, `Software\WoW6432Node\SoftVoice\ImageLine` (32/64-bro), samt `…\Uninstall\` för avinstallation.
- **Genvägar**: `More....lnk`, `Plugin Manager.lnk`, `Help.lnk`, `What's new.lnk`, `Reset settings.lnk`, `Install plugin version (64bit).lnk`, `Uninstall.lnk`, `FL Studio online.lnk`.
- **Hjälpen är två `.chm`** (`\Help\FL.chm`, `\help\fl.chm`) **plus** det webbaserade `Help\html\FD\web` (HTML + css + typsnitt) — alltså två parallella help-system.
- **Hjälpfiler per plugin**: 72 `.rtf` (`<plugin>_Info.rtf`), som visas inuti racket.
- **Licenserna ligger i klartext** (`System\Texts\Licenses\*.txt`) och avslöjar komponenterna: blend2d, delphiffmpeg, delphizip, **demucs**, flac, freetype, lame, ogg vorbis.
- **Uppdateringstexter**: `NewVersion.txt`, `NotReggedTxt.txt`, `BoxEdition.txt`, `Legacy_ChanFX.txt`, `Legacy_PatBlocks.txt` — de två sista är migreringsnotiser för gamla projektformat.

---

## 7. Sonix i dag — mätt i koden

96 filer, **82 572 rader** i `src/`. Områdena nedan är modulerna, inte önskelistan.

| Område | Moduler (rader) | Vad som faktiskt finns |
| :--- | :--- | :--- |
| **Ljudmotor** | `engine.rs` (578), `synth.rs` (336), `synth/process.rs` (787), `synth/commands.rs` (717), `synth/voices.rs` (497), `realtime_bench.rs` (504) | cpal-ström, röster med eget filter + filterenvelop, spår/bussar/VCA/sends/sidokedjor, PDC, frysning, realtidsmätning |
| **Instrument** | synten, `drum.rs` (251), samplern (i `voices.rs`/`process.rs`), `keymap.rs` (161), `sf2.rs` (359, i dag) | 4 vågformer, **2-op FM** (i dag), ADSR, SVF per röst, drum-syntes, sampler med loop/ping-pong/ADSR/filter, multi-sample-zoner, **SoundFont 2** |
| **Effekter** | `master_fx.rs` (1 895), `effects.rs` (215), `filter.rs` (193), `envelope.rs` (338) | EQ (4 band + per spår), kompressor, doubler, de-esser, gate, filter, limiter, **transient**, **saturator**, **multiband**, **convolver**, **steggrind**, RemixFX, tape-stop, formantbevarande pitch, autotune, tuner, spektrum, loudness, dither |
| **Pluggar** | `plugin_host_live.rs` (3 331), `plugin_vst3.rs` (1 981), `plugin_vst2.rs` (907), `plugin_sandbox.rs` (944), `sandbox_audio.rs` (601), `plugin_gui.rs` (467), `plugin_host.rs` (1 152) | CLAP + VST3 + VST2 i process, sandlåda utanför process, X11-värd för plugin-GUI, parameterlista/automation, latenskompensation |
| **Redigering** | `arranger.rs` (2 778), `timeline.rs` (1 301), `piano_roll.rs` (925), `midi_take.rs` (563), `onset.rs` (1 040), `stretch.rs` (1 766) + `ui/app/stretch.rs` | spellista med regioner/tonhöjning/fades/reverse/loop, pianorulle, tagningar med swing och kvantisering, skalor/tonarter, chopper med slagletning → slicemap → piano roll, tempokarta + WSOLA-sträckning |
| **Mixning** | `mixer.rs` (3 067), automation i `state.rs`/`frame.rs` | kanalrack, bussar, VCA, sends, sidokedja, mätare, spektrum, automation (spår/buss/plugin) med ångring |
| **Sekvensering** | `launcher.rs` (504), `loop_station.rs` (506), `ui/launcher_view.rs` (289) | scenrutnät och loop-pad (Sprint 1) |
| **Noder** | `patcher.rs` (781), `ui/patcher_view.rs` (302) | nodgraf med kablar |
| **Import/export** | `import.rs` (1 366), `exporter.rs` (2 232), `recorder.rs` (1 699), `wav_reader.rs` (282), `wav_writer.rs` (47), `smf.rs` (705), `metadata.rs` (1 824) | Suno-stämmor (zip/mapp), wav, MIDI in/ut, export med dither/loudness, AI-härkomst borttagen ur filer |
| **Projekt** | `project.rs` (2 050), `autosave.rs` (429), `paths.rs` (1 013), `platform.rs` (62) | sparform med migreringar, autosave med versioner, kanoniska sökvägar, plattformsskillnader |
| **AI** | `ai_client.rs` (714), `ai_generator.rs` (789), `neural_separator.rs` (462), `stem_separator.rs` (884) | molnanrop, generering, Demucs via ONNX, klassisk separator |
| **Arbetsflöde** | `macro_chain.rs` (1 206), `macros_modal.rs` (780), `browser.rs` (632), `factory_samples.rs` (1 574) | makrokedjor över filer, sökbart bibliotek, fabriksljud |
| **MIDI** | `midi_input.rs` (274), `midi_output.rs` (118, i dag), `hardware_control.rs` (492) | klaviatur in (autoanslutning), **ut** (i dag), MCU-ytor |
| **Gränssnitt** | `i18n.rs` (2 895), `theme.rs`, `widgets.rs` (733) + 20 vyer | svenska/engelska, tema, egna widgets |

**Kvalitetsläget just nu:** 644 tester (default) / 693 (`plugin-host`), 0 varningar i alla fyra
byggena, fyra CI-ben (default, plugin-host, Windows, neural) och `--selftest` 5/5 mätbara delar.

---

## 8. Gap-matrisen

Varje rad är ett *område* i FL mätt ovan, ställt mot vad Sonix har. Kolumnen **Dom** är min
bedömning, inte ett mätvärde: "vi" = vi har en motsvarighet som gör jobbet, "delvis" = halva
kedjan finns, "de" = vi har inget som motsvarar det.

| Område | FL | Sonix | Dom |
| :--- | :--- | :--- | :--- |
| **Syntes** | 51 generatorer: subtraktiv, FM (Sytrus 6 operatorer), additiv (Harmor/Ogun/Morphine), fysisk modellering (Sakura/Plucked!/Slayer), granulär, bild→ljud (BeepMap) | en subtraktiv synth + 2-op FM + sampler + SF2 | de |
| **Trummor** | FPC, Drumaxx, Drumpad, DrumSynth Live, 1 908 trumsamplingar + 1 718 MIDI-loopar | drum.rs (syntetiska), steprad, egna samplingar | delvis |
| **Modulationseffekter** | chorus ×3 (Fruity/Hyper/Vintage), flanger ×2, phaser ×2, Love Philter, Effector | chorus, flanger, phaser i masterns kedja (byggda i dag) + doubler; Love Philter/Effector kvar | delvis |
| **Dynamik** | Limiter, Compressor, Multiband, Maximus, Transient, Soft Clipper, Squeeze | limiter, kompressor, multiband, transient, saturator (i dag) | vi |
| **Reverb/delay** | Reeverb 1/2, Convolver, Luxeverb, 260 impulser, Delay 1/2/3, Delay Bank, Multiband Delay | algoritmiskt reverb, stereodelay (ping-pong), convolver + **genererad impulsbank** (sex rum, i dag) | delvis |
| **EQ/filter** | Parametric EQ 1/2, 7 Band, Free Filter, Filter, Fast LP, Frequency Shifter/Splitter | 4-band + per spår + SVF + drive | delvis |
| **Vocoder** | Vocodex, Fruity Vocoder | `vocode` (16 band, bärvåg som följer sången) | de |
| **Modulationskällor** | Peak/Envelope/Formula/Keyboard/Voltage/XY/XYZ-kontroller, VFX-sviten | inget (automation finns, men inte som källa som styr andra) | de |
| **Notbearbetning** | VFX Key Mapper, Keyboard Splitter, Level Scaler, VFX Script, Sequencer | velocity-kurva + pitch per kanal, SchedulesChord, dice-generator | delvis |
| **Slicing** | Slicer, Slicer 2, Slicex, BeatSlicer (äldre), Edison | chopper: slagletning → slicemap → steg/piano roll | vi |
| **Ljudredigering** | Edison (inspelare + editor), Newtone (pitch), Newtime (time-align), declick, korsningar | recordern, sträckning, "sätt takt 1 här", declick + korsningar (i dag) | delvis |
| **Performance** | Gross Beat, Performance mode, touch-klaviatur, 4×4/8×8-mallar | launcher-rutnät, loop-pad, steggrind (i dag), RemixFX | delvis |
| **Noder/modularitet** | Patcher, Dashboard, Control Creator | patcher.rs (graf), MCU-ytor | delvis |
| **Notation** | LilyPond + Guile + Ghostscript, notutskrift | inget | de |
| **Video** | Fruity Video Player, ZGameEditor Visualizer, NDI, Spout | `audio::video` + videovy (referensfilm in, ingen bild ut) | de |
| **Skript** | piano roll-skript, ljudskript, VFX Script, inbäddad Python 3.12, MCP | inget skript-API (AI-vägen är kommandon, inte skript) | de |
| **Ackord/generering** | Chord Progression (Markov + genrestilar), Riff machine | chord_generator_modal, dice_generator, song_structure | delvis |
| **Scores/bibliotek** | 2 111 fraser, 260 impulser, 7 993 presets, 5 135 paket | factory_samples + SF2-inläsning; ingen frasbank, ingen impulsbank | delvis |
| **Kontrollerytor** | 13 hårdvarumappningar + egen Control Surface | MCU + MIDI-learn + in/ut | delvis |
| **Tangentbord** | 5 layouter (bl.a. Janko), touch-klaviatur i 6 varianter | datortangentbord som klaviatur, en layout | delvis |
| **Gränssnitt** | verktygsrads-presets, notfärger, tema-skins, 241 plugin-`.nfo` | tema, i18n, FX-rack | delvis |
| **Projektformat** | .flp (binärt), .fst-presets, .fsc-scores, .flms | .json (läsbart), .mid, .wav | vi |
| **Molntjänst** | FL Cloud (mixing/mastering-pluggar, samples, lagring) | AI-anrop via OmniRoute | delvis |
| **Stämseparering** | Demucs (licens angiven) | Demucs via ONNX + klassisk separator | vi |
| **Pluggar** | VST2/VST3/CLAP via wrapper, 32-bits brygga | CLAP/VST3/VST2 + sandlåda + X11-GUI | vi |
| **Prestanda** | okänt (ej mätt) | realtidsmätning, 4 CI-ben, 0 varningar | vi |

**Räkning:** 6 områden där vi har en motsvarighet, 12 delvis, 7 där vi inte har något.
De sju tomma är: syntesdjup (additiv/spektral/fysisk/granulär), chorus/flanger/phaser,
vocoder, modulationskällor, notation, video och skript-API.

---

## 9. Byggt i dag (2026-10-21)

| FL | Sonix nu | Beviset |
| :--- | :--- | :--- |
| Transient Processor | `TransientShaper` | bit-identisk vid 0 dB, +12 dB lyfter anslaget ~4×, −12 dB kropp ger 0,251 av en jämn ton |
| Soft Clipper / Fast Dist / WaveShaper | `Saturator` | tredjeton 4,6 / 13,1 / 24,4 % vid drive 2/4/8, aldrig över full skala |
| Maximus / Multiband Compressor | `MultibandCompressor` | 100 Hz → 0,248, 5 kHz → 1,0000; provet faller med `amounts = 0` |
| Fruity Convolver | `Convolver` | latens exakt 256 sampel; `[0,5, −0,5]` ger ±0,5 på plats 256/257 |
| Gross Beat (volymkurvan) | `StepGate` | 120 BPM → 6 000 sampel/steg; steg 1 RMS exakt 0 |
| DX10 / Sytrus (2-op FM) | `FmParams` | sidoband 0,00351 → 0,06796 när index 0 → 4 |
| Soundfont Player / DirectWave | `audio/sf2.rs` | syntetisk `.sf2` med två sampel ger rätt intervall, grundton och PCM |
| MIDI Out | `audio/midi_output.rs` | `--selftest` öppnade `Midi Through` och skickade noten |
| Fruity / Hyper / Vintage Chorus | `ModDelay` (12 ms ± 8 ms, kvarts periods kanalskillnad) | mix 0 är bit-identisk genomgång; svepet mätt till 12–20 ms, flangern 2,5–4,5 ms |
| Fruity Flanger (+ Flangus) | samma `ModDelay`, 0,6 återkoppling | kammen sveper 2 ms; provet faller om svepet står stilla |
| Fruity / Vintage Phaser | `Phaser` (4 allpass, 200 Hz–2 kHz) | kammen vandrar: nivån svänger 10× mer med LFO än utan, och står exakt still vid `depth = 0` |
| 260 impulser i 13 kategorier | `synthesize_ir` + `IR_BANK` (6 rum) | deterministisk LCG: topp normaliserad till 1,0, svansen faller >4× från 15 % till 90 % av längden, klick genom banken ger mätbar svans vid 125 ms |
| `Declick in/out all regions` | `declick()` — 3 ms golv på varje regionkant | provet mäter golvet: 0 → 0,003 s, och en egen fade på 0,25 s rörs inte |
| `Create crossfade` / `Automatic crossfades` | `PlaylistTrack::create_crossfades` | korsningen blir exakt överlappet (1 takt av 1); ett stort överlapp kapas vid halva regionen (1,5 → 1,0); regioner som möts utan överlapp får ingen |

---

## 10. Kvar — i den ordning jag skulle ta dem

| # | Vad | Varför den ordningen | Storlek |
| :--: | :--- | :--- | :---: |
| ~~1~~ | ~~**Chorus / Flanger / Phaser**~~ | **Klar i dag**: `ModDelay` + `Phaser` i masterns kedja, tre pedaler, tre prov som biter. | *S* |
| ~~2~~ | ~~**Impulsbank till convolvern**~~ | **Klar i dag**: sex genererade rum (0,6–4,5 s) + knappar i FX-racket. FL:s 260 är inspelningar; våra är modeller, och statusraden säger det. | *S* |
| 3 | **Modulationskällor** (peak/envelope/formel → parameter) | låser upp Love Philter, PanOMatic, VFX-familjen och gör automation tvådimensionell. Kräver en målmodell i motorn. | *M* |
| 4 | **Granulizer** | enda kvarvarande *instrument*-gapet som är rimligt i den här kodbasen (kanalens 13 initialiseringar + ett grain-läge i `voices.rs`). | *M* |
| 5 | **Vocoder** | kräver bärvågsingång i masterns kedja — bygg den först, annars blir vocodern halv. | *M* |
| 6 | **Frasbank (Scores) + Riff machine** | innehåll, inte kod: `.fsc`/`.mid` som pianorullen kan hämta. FL:s 2 111 filer visar formen. | *M* |
| 7 | **Skript-API för pianorullen** | FL:s `Piano roll script reference.txt` är en hel yta; vår AI-väg är kommandon, inte skript. | *L* |
| 8 | **Notation (LilyPond-vägen)** | FL bäddar in hela LilyPond; vi kan exportera MusicXML och låta LilyPond vara externt. | *M* |
| 9 | **Video/visualizer** | egen mediepipeline; lägst värde för en DAW utan videospår. | *L* |
| 10 | **Additiv/spektral syntes** (Harmor/Ogun/Morphine) | ett synthprojekt, inte en ratt. | *XL* |
| 11 | **`Consolidate` spår → klipp** | `freeze` renderar men lämnar inget klipp; FL:s `Consolidate this track` är samma DSP med ett annat slut — billigt ovanpå frysningen. | *S* |
| ~~12~~ | ~~**Korsningar + declick**~~ | **Klar i dag**: `declick()` lägger 3 ms golv på varje klippkant, `create_crossfades()` tonar överlapp. Menyn har `🔀 Skapa korsning`. | *S* |
| 13 | **Score-loggen** (`Auto dump score`) | den enda funktionen i listan som ändrar hur man *arbetar*: spela in allt, dumpa det som blev bra. | *M* |

---

## 11. Gränser för den här mätningen

- **125 av payloadens block avkodades (68 MB)**, inklusive `FL64.exe` (22,2 MB),
  `FLEngine_x64.dll` (3,3 MB), `FLMManaged.dll` (9,3 MB), ASIO-drivern och 13 PE-filer till.
  Strängarna i de tre stora binärerna är lästa; **koden är inte disassemblerad**, så hur en plugin
  låter eller exakt hur en operation är byggd är inte mätt.
- **Resten (≈987 MB) gick inte att plocka ut, och det är mätt varför.** Filens förstaheader är
  `flags=0x30`, `0xDEADBEEF`, `NullsoftInst`, blockstorlek 3 519 116, total 1 105 606 786 — alltså
  NSIS med **solid LZMA**, där ett block refererar föregående blocks ordbok. `7z l -tNsis` avvisar
  filen (`Cannot open the file as [Nsis] archive`, även med stubbens 0x10a00 byte bortskurna), och
  en egen avkodare tappar aligeringen vid post 124 där posterna börjar blandas med strängposter
  (`Tip=Demo project files`, `Visible=False`). Att forcera resten skulle kräva en riktig
  solid-block-läsare. **De 200+ plugin-DLL:ernas innehåll är därför namngivet men inte läst.**
- **Referensfilerna är kompletta i sina dimensioner**: alla filposter (21 339), alla menyrader
  (6 256), hela etikettordförrådet (28 657). Det som saknas är binärinnehåll, inte inventarium.
- **Statusarna ✅/◐/✗ är på funktionsnivå.** "✅ Fruity Limiter" betyder att vi har en limiter
  med tak och release, inte att vår kurva är identisk med FL:s.
- **Ingen körning av FL.** Allt är statisk analys; inga påståenden om beteende i drift bygger på
  observation av programmet.
- **Antalet filer per plugin är ett innehållsmått**, inte ett kvalitetsmått: ZGameEditor har
  1 423 filer och är ändå det minst viktiga för en DAW.
- **FL-versionen är 26.1.7** (build 5654). En annan version kan ha andra plugins; listan gäller
  den här filen.
