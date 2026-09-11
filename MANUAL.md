# 📖 Sonix Studio – Komplett Master Manual & Bruksanvisning
*Den ultimata guiden till Sonix Studio: Native Linux Digital Audio Workstation, Stämeditor, Sequencer & Synthesizer.*

---

## Innehållsförteckning
1. [Introduktion & Systemarkitektur](#1-introduktion--systemarkitektur)
2. [Snabbstart & Arbetsflöde](#2-snabbstart--arbetsflöde)
3. [Komplett Tangentbords- & Kortkommandoreferens](#3-komplett-tangentbords--kortkommandoreferens)
4. [Tidslinje & Arrangör (Arranger / Playlist Timeline)](#4-tidslinje--arrangör-arranger--playlist-timeline)
5. [Multi-Track Stämimport (Import Stems)](#5-multi-track-stämimport-import-stems)
6. [Dedikerad Stämeditor & Ljudfokus (Stem Detail & Focus Editor)](#6-dedikerad-stämeditor--ljudfokus-stem-detail--focus-editor)
7. [Sonix Channel Rack & Stegsequencer (F4)](#7-sonix-channel-rack--stegsequencer-f4)
8. [Piano Roll & Synthesizer (F5 / F7)](#8-piano-roll--synthesizer-f5--f7)
9. [Vocal Studio, Melodyne & Harmonizer (F8)](#9-vocal-studio-melodyne--harmonizer-f8)
10. [Mixer Console, Effektrack & Routing (F6)](#10-mixer-console-effektrack--routing-f6)
11. [Export & Master-rendering (Ctrl+E)](#11-export--master-rendering-ctrle)
12. [Ljudinställningar (PipeWire/ALSA/JACK) & Hårdvarukontrollers (MCU/OSC)](#12-ljudinställningar-pipewirealsajack--hårdvarukontrollers-mcuosc)
13. [Filplatser, autosparning & kraschåterställning](#13-filplatser-autosparning--kraschåterställning)

---

## 1. Introduktion & Systemarkitektur

Sonix Studio är en blixtsnabb, professionell musikproduktionsmiljö (DAW) skriven i **Rust** och byggd med **egui**. Den är särskilt optimerad för Linux-miljöer (PipeWire, ALSA, Wayland, X11, Hyprland m.fl.) med absolut realtidsprestanda och noll latens.

### Kärnfunktioner:
* **Högprecisions ljudmotor:** Dedikerad realtids-audiotråd (`cpal` + ringbuffert `rtrb`) med 32-bit float intern ljudbehandling.
* **Hundradelssekunds-precision (0.01s):** Alla tidskoder, klipppunkter, fadelängder och redigeringsverktyg opererar med exakt hundradelsprecision (`MM:SS.cs`).
* **Multi-Track Stämstöd:** Importerar och mixar fullständiga stämpaket (från Suno AI, FL Studio, Ableton, Logic, m.fl.) med äkta avkodade vågformer och automatisk BPM-detektering.
* **Icke-blockerande asynkron inläsning:** Stora WAV- och ZIP-filer packas upp och avkodas i bakgrunden med realtidsförloppsindikator.
* **Integrerad Syntes & Sångstudio:** Inbyggd 4-vågforms subtraktiv synth, resonant SVF-filter, Delay, Reverb, Melodyne-liknande pitch-editor och 4-stämmig intelligent harmonizer.

---

## 2. Snabbstart & Arbetsflöde

Ett typiskt produktionsflöde i Sonix Studio:

1. **Starta & Importera**: 
   - Klicka på **`📦 IMPORT STEMS`** i verktygsfältet eller tryck `Ctrl+I`.
   - Välj ett ZIP-paket eller en mapp med ljudspår. Sonix läser in stämmorna, genererar vågformer och sätter automatiskt tempo och loop.
2. **Arrangera & Klipp**:
   - Zooma in med `Ctrl + Scroll` eller klicka på `100%`, `200%`, `400%`.
   - Välj saxverktyget (**`✂ Klipp (0.01s)`**) och klicka i tidslinjen för att dela ett klipp med hundradelsprecision.
   - Aktivera **`🏃 Följ tidslinje: PÅ`** för att låta skärmen rulla automatiskt under uppspelning.
3. **Fokusera på enskild stämma**:
   - Klicka på **`🔍`** på spårhuvudet eller dubbelklicka på spåret för att öppna **Stämeditorn**.
   - Klicka på **`🎧 ISOLERA STÄMMA (SOLO)`** för att stänga ute allt annat.
   - Finjustera volym med `±0.1 dB`, ställ in 3-bands grafisk EQ och mjuka fade in/out-kurvor i hundradelar.
4. **Bygg vidare med Trummor & Synth**:
   - Tryck `F4` för att öppna **Channel Rack** och klicka in ett 16-stegs beat.
   - Tryck `F5` för att spela in melodier och ackord i **Piano Roll**.
5. **Mixa & Exportera**:
   - Tryck `F6` för att balansera nivåer i mixern.
   - Tryck `Ctrl+E` för att exportera hela låten eller enskilda stämmor till kristallklar 48kHz WAV.

---

## 3. Komplett Tangentbords- & Kortkommandoreferens

| Tangent / Genväg | Funktion | Beskrivning |
| :--- | :--- | :--- |
| **Mellanslag (Space)** | **Play / Pause** | Startar eller pausar uppspelningen i låt- eller mönsterläge. |
| **F1** | **Bruksanvisning / Manual** | Öppnar denna detaljerade manual och hjälpcenter. |
| **F3** | **Tidslinje / Arranger** | Växlar till den linjära flerspårs-audiotidslinjen. |
| **F4** | **Channel Rack** | Växlar till 16-stegs trummaskinen och mönstereditorn. |
| **F5** | **Piano Roll** | Växlar till det grafiska notinmatningsfönstret. |
| **F6** | **Mixer Console** | Växlar till 8-kanals effektrack och masterbussen. |
| **F7** | **Analog Synthesizer** | Växlar till Alchemy Synth med ADSR, Filter och LFO. |
| **F8** | **Vocal Studio** | Växlar till sångstudion, Melodyne-editorn och harmonizern. |
| **F9** | **AI Music Studio** | Växlar till AI Music Prompt Engine & Assistenten. |
| **F10** | **Modulär Patcher** | Växlar till den modulära nod- och kabelpatchern. |
| **Ctrl + N** | **Nytt Projekt** | Skapar ett tomt projekt och nollställer tidslinjen. |
| **Ctrl + O / Ctrl + P** | **Projektbläddrare** | Öppnar fil- och projekthanteraren. |
| **Ctrl + S** | **Spara Projekt** | Sparar hela projektet med spår, regioner, EQ och mixerinställningar. |
| **Ctrl + I** | **Importera Stämmor** | Öppnar importdialogen för ZIP-stempaket och mappar. |
| **Ctrl + E** | **Exportera WAV** | Öppnar exportkön och renderingsdialogen. |
| **Delete / Backspace** | **Ta bort markerat** | Raderar det för tillfället markerade ljudklippet. |
| **Ctrl + Scroll** | **Dynamisk Zoom** | Zoomar in/ut horisontellt på tidslinjen där muspekaren befinner sig. |
| **A, W, S, E, D, F...** | **Klaviaturtangenter** | Spelar synthen live i realtid via datortangentbordet. |

---

## 4. Tidslinje & Arrangör (Arranger / Playlist Timeline)

Arrangören är hjärtat i Sonix Studio där alla stämmor, ljudklipp och mönster sätts samman längs en kontinuerlig tidsaxel.

### 4.1. Högprecisions-tidslinje & Linjal (Ruler)
Tidslinjen är indelad i fyra dynamiska nivåer som visas beroende på zoomgrad:
1. **Takter (Bars):** Stora vertikala linjer märkta `1, 2, 3, 4...` med tidskod i minuter, sekunder och hundradelar (`MM:SS.cs`).
2. **Beats (Fjärdedelar):** Fjärdedelsstreck (`.2, .3, .4`) som aktiveras vid normal zoom.
3. **1/16-delssteg:** Detaljerade steglinjer för rytmisk redigering.
4. **Hundradelssekunder (0.01s):** Millisekundstikar som visas vid djup inzoomning för millimeterexakta snitt.

### 4.2. Redigeringsverktyg
* **`⇱ Välj`**: Markerar ljudregioner för redigering, flytt eller detaljinspektion.
* **`✎ Rita`**: Placerar ut mönster och aktiva klipp på spåret.
* **`✂ Klipp (0.01s)`**: Saxverktyg. Klicka var som helst på ett ljudklipp för att klyva det i två oberoende regioner. Snittet följer valt snäppläge.
* **`🔇 Muta`**: Klicka på ett klipp för att tysta det utan att radera det.
* **`🗑 Radera`**: Klicka på ett klipp för att direkt ta bort det från spåret.

### 4.3. Snäpplägen (Time Snap Grid)
* **`⚡ 0.01s (Fri)`**: Snäpper inte till musikaliskt rutnät, utan tillåter helt fri redigering ned till **0.01 sekunder (10 millisekunder)**. Perfekt för att klippa bort andetag, klick eller transientsynkning.
* **`1/16`**: Snäpper till närmaste sextondelsnot baserat på projektets BPM.
* **`Beat`**: Snäpper till varje hel taktslag (fjärdedel).
* **`Takt`**: Snäpper till hela takter.

### 4.4. Automatisk tidslinjerullning (`🏃 Följ tidslinje`)
* **`🏃 Följ tidslinje: PÅ`**: Tidslinjen rullar automatiskt mjukt så att spelhuvudet alltid hålls centrerat på skärmen under uppspelning.
* **`⏸ Följ tidslinje: AV`**: Tidslinjen låses i sitt läge så att du kan scrolla och redigera andra delar av låten utan att vyn kastas iväg när musiken spelar.

### 4.5. Klippinspektor (Region Inspector)
När ett ljudklipp är markerat dyker en inspektorpanel upp längst ned på tidslinjen:
* **Starttid & Längd**: Visar exakt starttakt, startsekund (`0.00s`), längd och samplingsposition.
* **Finjustering**: Knappar för att flytta klippet framåt/bakåt med `±0.01s` eller `±0.10s`.
* **Klipp & Dela**: Knappen `✂ Dela vid spelhuvud (0.01s)` klyver klippet exakt där spelmarkören står.
* **Snabbknapp för Stämeditor**: Knappen `🔍 Öppna Stämeditor` öppnar spårets fulla ljudfokus.

---

## 5. Multi-Track Stämimport (Import Stems)

Sonix stöder fullständig flerspårs-stämimport från alla källor:

### Så här importerar du:
1. Tryck på knappen **`📦 IMPORT STEMS`** i verktygsfältet eller **`Ctrl+I`**.
2. **Upptäckta paket**: Programmet skannar automatiskt dina mappar `~/Music` och `~/Downloads` efter stempaket.
3. **Manuell filväljare**: Klicka på `📁 Välj ZIP-fil från datorn...` eller ange en mapp/filsökväg.
4. **Drag & Drop**: Dra en `.zip`-fil direkt från filhanteraren och släpp den i Sonix-fönstret.

### Intelligent Spåravkodning:
* Sonix läser ut **äkta 48kHz WAV-data** och bygger högupplösta vågformstoppar.
* Tolkar automatiskt spårens namn och tilldelar ikoner, färger och mixer-routing:
  - `Lead Vocal` ➔ 🎙 Sångbuss med lila färgkod.
  - `Backing Vocals` ➔ 🗣 Körbuss.
  - `Drums / Kick / Snare` ➔ 🥁 Trumbuss med cyan färgkod.
  - `Bass` ➔ 🎸 Basbuss med gul färgkod.
  - `Guitar / Keys / Synth` ➔ 🎹 Synthbuss med grön/orange färgkod.
  - `FX / Other` ➔ ✨ Effektsändning.
* Bakgrundsinläsningen visar en **progress bar** som visar exakt vilket spår som avkodas just nu.

---

## 6. Dedikerad Stämeditor & Ljudfokus (Stem Detail & Focus Editor)

För att arbeta ostört med ett enskilt ljudspår har Sonix en dedikerad **Stämeditor**:

### Hur du öppnar:
* Klicka på **`🔍`**-ikonen på spårhuvudet till vänster i tidslinjen.
* **Dubbelklicka** direkt på spårhuvud-kortet i Arrangören.
* Klicka på `🔍 Öppna Stämeditor` i Klippinspektorn.

### Funktioner i Stämeditorn:

#### 1. 🎧 Direkt isolering (Solo Isolate)
Högst upp i editorn finns en stor isoleringsknapp (`🎧 ISOLERA STÄMMA (SOLO)`). När denna är aktiv tystas alla andra spår så att du kan lyssna enbart på den valda stämman i realtid medan låten spelar.

#### 2. Flik 1: Volym, Panorering & Dynamik
* **Exakt Volym**: Skjutreglage samt `±0.1 dB`-knappar för exakt nivåjustering och snabb nollställning (`0 dB`).
* **Panorering**: Balansreglage med snabbcentrering (`Center`).
* **Kompressor**: Justerbar tröskelnivå (*Threshold* -30 dB till 0 dB) och kompressionsgrad (*Ratio* 1:1 till 8:1).
* **Pitch Shifter**: Transponera stämman uppåt eller nedåt med ±12 halvtoner.
* **Effektsändningar**: Reverb Send och Delay Send reglage.

#### 3. Flik 2: 3-Bands Grafisk Parametrisk EQ
* **Visuell Frekvenskurva**: Visar i realtid hur filtrets kurva ser ut från 20 Hz till 20 000 Hz mot ett stödraster med dB-skala.
* **Low Shelf (Bas)**: Gain (±12 dB), brytfrekvens (40–400 Hz) och `±1 dB` snabbknappar.
* **Mid Peak (Mellanregister)**: Gain (±12 dB), centrumfrekvens (200 Hz – 6 kHz) och Q-faktor (0.5 till 3.0).
* **High Shelf (Diskant)**: Gain (±12 dB), brytfrekvens (3 kHz – 16 kHz) och `±1 dB` snabbknappar.
* **Förinställningar**: Snabblägen för `🎙 Sång: Luft & Värme`, `🥁 Trummor: Punch & Snap`, `🎸 Gitarr: Presence`, `🔊 Bas: Deep Sub` samt `🔄 Nollställ (Flat)`.

#### 4. Flik 3: Exakt Fading & Envelope (0.01s Precision)
* **Fade In / Fade Out**: Ställ in kurvlängder med hundradelssekunds-precision.
* **Finjustering**: Knappar för `±0.01s` och `±0.10s`.
* **Snabbval**: `20 ms`, `50 ms`, `100 ms`, `500 ms`, `1.00 s`.
* **Applicera på alla**: Ändringen slår omedelbart igenom på alla klipp i den valda stämman.

#### 5. Flik 4: Klipp & Vågform
* Detaljerad översiktsvågform med tidsmarkör och klickbar navigering.
* Lista över spårets samtliga regioner med snabbknappar för att dela (`✂ Dela`), duplicera (`⎘ Duplicera`) eller radera klipp.

#### 6. Navigering mellan stämmor
Längst ned kan du enkelt bläddra till `◀ Föregående stämma` eller `Nästa stämma ▶` utan att behöva stänga fönstret.

---

## 7. Sonix Channel Rack & Stegsequencer (F4)

Channel Racket är den klassiska 16-stegs trummaskinen och mönstereditorn i stil med FL Studio.

* **16 Steg per kanal**: Kick, Snare, Clap, Closed Hat, Open Hat, Crash, 303 Bassline och Synth Lead.
* **Mönster (Patterns P1–P4)**: Skapa olika rytmiska variationer för vers, refräng och brygga.
* **Swing-kontroll**: Skjutreglage för att ge trummorna ett naturligt sväng och groove.
* **Slumpgenerator (`⚡ Slumpa Beats`)**: Skapar omedelbart inspirerande trumrytmer.
* **Velocity Per Steg**: Justera anslagskraften för varje enskilt steg i den nedre velocity-raden.

---

## 8. Piano Roll & Synthesizer (F5 / F7)

### Piano Roll (F5)
* Grafiskt notinmatningsfönster över 3 oktaver (C3 till B5).
* Klicka på rutnätet för att aktivera toner på specifika steg.
* Klicka på tangenterna till vänster för att provlyssna tonhöjden i realtid.
* **Slumpa Melodi**: Knappen `⚡ Slumpa Melodi` skapar melodiska slingor i vald skala.

### Analog Alchemy Synthesizer (F7)
* **Oscillator**: 4 vågformer – *Sawtooth* (sågtand), *Square/Pulse* (fyrkant), *Sine* (sinus) och *Triangle* (triangel).
* **ADSR Envelope**: Attack, Decay, Sustain och Release för exakt klangformning.
* **Resonant SVF-lågpassfilter (12 dB)**: Cutoff-frekvens (20 Hz – 18 kHz) och resonans (0.0 – 0.95).
* **Saturation & Drive**: Analog rörvärme och distortion.
* **Live Klaviatur**: Spela synthen live med tangentbordets tangenter:
  - `A` = C, `W` = C#, `S` = D, `E` = D#, `D` = E, `F` = F, `T` = F#, `G` = G, `Y` = G#, `H` = A, `U` = A#, `J` = B, `K` = C.

---

## 9. Vocal Studio, Melodyne & Harmonizer (F8)

Vocal Studio är en komplett miljö för sångbearbetning och stämsång.

* **Melodyne ARA2 Pitch Editor**: Visar sångens toner som interaktiva "blobs" på ett tonhöjdsraster. Klicka och dra toner för att justera tonhöjd och timing.
* **4-Voice Intelligent Harmonizer**: Skapa fylliga sångarrangemang med upp till 4 stämmor. Ställ in volym, stereobredd (cents) och formantförskjutning per stämma.
* **Comping & Tagningar**: Spela in flera sångtagningar (`Take 1`, `Take 2`...) och klipp ihop den perfekta slutversionen.
* **Sampling & Ljudinspelare**: Spela in instrument, klappar, rösteffekter eller miljöljud via din mikrofon och spara direkt i projektets samlingsbibliotek.

---

## 10. Mixer Console, Effektrack & Routing (F6)

Mixern ger full kontroll över balans, frekvenser och rumsakustik:

* **8 Stereokanaler + Master Bus**: Full uppsättning med skjutreglage (Faders), panorering, Solo och Mute.
* **Stereo VU Peak Meters**: Högupplösta LED-mätare som varnar för klippning och digital distorsion.
* **3-Bands Parametrisk EQ per kanal**: Justera Low, Mid och High direkt på mixerbordet.
* **Reverb & Delay Sends**: Justera rumsstorlek, efterklangstid och stereofördröjning.
* **Master Limiter / Maximizer**: Säkerställer hög kommersiell ljudstyrka utan digital distorsion.

---

## 11. Export & Master-rendering (Ctrl+E)

När din produktion är klar exporterar du enkelt till högkvalitativt ljud:

* **Export Song (Hel låt)**: Renderar hela tidslinjen och alla stämmor till en sammanslagen masterfil.
* **Export Pattern (Mönster)**: Exporterar det aktiva Channel Rack-mönstret som en sömlös loop.
* **Batch Export (Stämmor i paket)**: Exporterar alla aktiva spår som separata WAV-filer till mappen `./exports/`, perfekt för extern mixning eller mastring i andra studior.
* **Format**: 32-bit float / 24-bit PCM WAV i 44.1 kHz eller 48 kHz.

---

## 12. Ljudinställningar (PipeWire/ALSA/JACK) & Hårdvarukontrollers (MCU/OSC)

### Ljudmotor & Drivrutiner:
* **cpal + systemets standardvärd**: Sonix öppnar standardutgången via cpal (på Linux ALSA, vilket PipeWire/JACK exponerar). Aktuell värd och enhet visas i Ljudinställningar.
* **Samplingsfrekvens & buffertstorlek**: Välj 44.1/48/96 kHz och 128/256/512/1024 samples. Valet **appliceras direkt** genom att ljudströmmen byggs om live och sparas till `~/.config/sonix/audio.json` (återställs vid start). Använd 128/256 för realtidsspelning och 512/1024 för tunga mixningsprojekt.
* **Fönsterhantering & Wayland**: Sonix är helt anpassad för Hyprland, GNOME Wayland och KDE. Om fönstret täcks eller minimeras dras omritningshastigheten ned mjukt för att spara ström, samtidigt som ljudet fortsätter spela utan minsta hack.

### Hårdvarukontrollers:
* **Mackie Control Universal (MCU)**: Stöd för motoriserade reglagebord (t.ex. Behringer X-Touch, Mackie Control).
* **Open Sound Control (OSC)**: Trådlös styrning via surfplattor (TouchOSC / Lemur) via UDP-port 8000/9000.

---
## 13. Filplatser, autosparning & kraschåterställning

### Var ligger allt?
Alla sökvägar byggs av en enda modul och följer XDG-standarden. Kör `sonix --paths` i terminalen för hela kartan på din maskin (✓ = finns, · = skapas vid behov).

| Plats | Innehåll |
| :--- | :--- |
| `~/Music/Sonix/Projects/` | Dina projekt: `<namn>.sonix` + en mapp `<namn>/` för projektets egna media |
| `~/Music/Sonix/Samples/` | Dina egna samples (sparas hit när du klipper ur tidslinjen) |
| `~/Music/Sonix/Factory_Samples/` · `Templates/` | Fabriksljud (genereras första gången) och projektmallar |
| `~/.config/sonix/` | `config.json` (språk), `audio.json` (ljudström), `ai.json` (API-nycklar, endast läsbar för dig) |
| `~/.local/share/sonix/` | `models/` (ONNX-modeller för stem-separation), `plugins/` |
| `~/.local/state/sonix/` | `recent.json`, `window.json`, `logs/`, **`autosave/`** |
| `~/.cache/sonix/` | Vågforms-cache och sample-bibliotekets index |

Vill du flytta något (t.ex. projekt till en extern disk) sätter du `SONIX_PROJECTS_DIR`, `SONIX_SAMPLES_DIR`, `SONIX_CONFIG_DIR`, `SONIX_DATA_DIR`, `SONIX_STATE_DIR` eller `SONIX_CACHE_DIR` innan du startar.

### Autosparning och återställning
* **Var 60:e sekund** autosparas projektet om något har ändrats, och **direkt efter strukturella ändringar** (klipp, duplicera, ta bort spår, mute, inklistring, rensning). En skrivning sker högst var tionde sekund, så en snabb redigeringsföljd inte fyller disken.
* **5 versioner per projekt** behålls i `~/.local/state/sonix/autosave/`; äldre versioner roteras bort automatiskt. Det ger dig möjlighet att gå tillbaka till ett läge från några minuter sedan.
* Skrivningen är **atomisk** (temp-fil + `rename`). Om datorn dör mitt i en skrivning är projektfilen på disk antingen den gamla eller den nya — aldrig en halv.
* Har Sonix avslutats utan att du sparade (krasch, strömavbrott, `kill -9`) visas en **återställningsdialog vid nästa start** med de autosaves som är nyare än projektfilen. Välj **Återställ** för att öppna läget och granska det, eller **Fortsätt utan att återställa**.
* En återställd kopia **pensioneras** (döps om till `*.restored`) i stället för att raderas — frågan ställs inte igen, men filen finns kvar.
* En autosave skrivs bara när innehållet faktiskt skiljer sig från det du senast sparade, så ett orört projekt skapar inga kopior och ingen falsk varning.

### Senaste projekt & filhanteraren
* **🕘 Senaste projekt** i projektmenyn listar de åtta senast öppnade/sparade projekten (från `~/.local/state/sonix/recent.json`). Poster vars fil raderats utanför Sonix filtreras bort automatiskt.
* **📂 Visa projektmappen i filhanteraren** öppnar `~/Music/Sonix/Projects/` i systemets filhanterare — Sonix har ingen egen filbläddrare, utan lämnar över till din.

---

*Sonix Studio Pro – Skapad med kraften av Rust & Linux Audio.*
