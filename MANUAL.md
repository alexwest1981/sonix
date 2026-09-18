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

### mp3 eller wav? — frågan innan importen
Finns både en mp3 och en wav för **samma stämma** frågar Sonix vilka filer som ska läsas in, och frågan kommer **innan** avkodningen börjar:

* **Hoppa över mp3:erna (rekommenderas)** — standard. Sonix spelar wav; en mp3 som måste konverteras först är ett extra varv när wav-filen redan finns.
* **Ta med mp3:erna också** — allt läses in.
* En stämma som **bara** finns som mp3 tas alltid med — då *är* mp3:an stämman.

Frågan gäller alla vägar in: ZIP-arkiv, mapp, drag & drop — och stämseparatorn, där frågan i stället är *vilken* fil som ska separeras när vald mp3 har sin wav bredvid sig.

### Stämseparatorn (🧠 Stems)
Vyn **🧠 Stems** separerar en färdig mix i fyra stämmor — sång, trummor, bas och instrument. Motorn är din lokala HTDemucs-modell om en sådan finns installerad (kräver en build med `--features neural`), annars Sonix egen DSP-separator; statusraden säger vilken som användes.

När du lägger stämmorna i arrangeraren skrivs de som **32-bitars WAV-filer** i projektets egen materialmapp — `~/Music/Sonix/Projects/<projekt>/Stems/<källa>-vocals.wav` (och `-drums`, `-bass`, `-instruments`). Klippet pekar alltså på **sitt eget ljud** i stället för på originalet, och spelar därför även nästa gång projektet öppnas. Går en fil inte att skriva, eller inte att läsa tillbaka, skapas **inga klipp** och statusraden säger vad som gick fel.

### Ljud som inte går att läsa — Sonix tiger inte
WAV läses nativt. mp3, flac, ogg och m4a läses genom **ffmpeg** där vägen stöder det (stämseparatorn gör det); tidslinjen, kanalracket och Sångstudion arbetar med WAV.

En fil som inte kan läsas blir därför **aldrig ett tyst klipp som ser ut att ha ljud**: vägen rapporterar och avbryter (eller hoppar över filen) och statusraden namnger filen. Samma sak gäller en inspelning eller ett fruset spår vars fil försvunnit. Vill du ha en mp3 på tidslinjen, konvertera den först:

```
ffmpeg -i låt.mp3 låt.wav
```

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

### Sidokedja (duckning) — att låta ett spår styra ett annat

Välj ett spår i mixern och leta upp raden **Sidokedja:** i kanalpanelen (bredvid Buss och VCA):

* **Sidokedja:** vilket spår som ska styra. *Ingen* = avstängt. Ett spår kan inte ducka sig självt.
* **Duckning (dB):** hur mycket spåret sänks när key-signalen är stark (0–24 dB).
* **Tröskel (dB):** hur stark key-signalen måste vara innan duckningen börjar (−60–0 dB).

Vanligast är att låta basen ge plats åt kicken, eller att låta sången trycka ned en pad.
Ställ tröskeln så att bara kicken öppnar duckningen, och duckningen så pass att det hörs —
4–8 dB räcker oftast. Tiderna är fasta (snabb attack, ~120 ms release), så det klickar inte
och spåret är tillbaka fort.

Sidokedjan följer med i **exporten** och sparas i **projektfilen**, så filen låter som det du
hörde i högtalarna. Ett äldre projekt öppnas utan sidokedja, precis som förut.

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
| `<projektmappen>/Frozen/` | Frusna spår som 32-bitars WAV (mellansteg — skrivs om när du fryser igen) |
| `<projektmappen>/Stems/` | Separerade stämmor som 32-bitars WAV, skrivna när de läggs i arrangeraren |
| `~/Music/Sonix/Samples/` | Dina egna samples (sparas hit när du klipper ur tidslinjen) |
| `~/Music/Sonix/Factory_Samples/` · `Templates/` | Fabriksljud (genereras första gången) och projektmallar |
| `~/.config/sonix/` | `config.json` (språk), `audio.json` (ljudström), `ai.json` (API-nycklar, endast läsbar för dig) |
| `~/.local/share/sonix/` | `models/` (ONNX-modeller för stem-separation), `plugins/` |
| `~/.local/state/sonix/` | `recent.json`, `window.json`, `logs/`, **`autosave/`** |
| `~/.cache/sonix/` | Vågforms-cache och sample-bibliotekets index |

Vill du flytta något (t.ex. projekt till en extern disk) sätter du `SONIX_PROJECTS_DIR`, `SONIX_SAMPLES_DIR`, `SONIX_CONFIG_DIR`, `SONIX_DATA_DIR`, `SONIX_STATE_DIR` eller `SONIX_CACHE_DIR` innan du startar.

### Vad innehåller en projektfil?
En `.sonix`-fil är läsbar JSON och innehåller allt du arbetat med: tempot, spåren med volym/pan/mute/solo, klipp och ljudregioner, automation, EQ/kompressor, sändningar, buss- och VCA-nivåer, plugin-inställningar — och sedan **Fas 6.7** även **mönstren med noterna** (trumsteg, piano-roll, kanalernas toner), **Channel Racket** med kanalernas inställningar, och **stegvolymerna**.

Kanalernas ljud sparas **inte** i filen — bara sökvägen till samplen. Ljudet ligger redan på disk, läses tillbaka när projektet öppnas och vågformen ritas om ur det. Det håller projektfilen liten: en kanal kostar några rader i stället för hundratals kilobyte.

En fil som sparades före Fas 6.7 saknar de fälten. Den öppnas som förut — då står standardpatterns och standardracket kvar i stället för att nollställas.

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

---

## 14. MIDI in och ut (.mid)

Sonix läser och skriver standard-MIDI-filer (SMF) med en egen kodek — ingen extern
konverterare behövs, och filen du skickar ut kan läsas av andra program (den är
prövad mot `ffprobe` och mot en oberoende avkodare).

### Exportera

**Menyn → 🎹 MIDI → Exportera.** Hela arrangemanget skrivs ut: ett spår per
spårtyp, trummor på kanal 10, tempo och taktart i ledspåret. Ljudspår (regioner)
hoppas över — de har ingen MIDI-motsvarighet.

### Importera

**Menyn → 🎹 MIDI → Importera.** Välj en `.mid`-fil:

- **Noterna routas dit appen spelar dem.** Percussion (kanal 10) till
  trumkanalerna, toner i 48–71 till piano-rollen, och allt under 48 till
  baskanalen — en basstämma från en annan DAW hamnar alltså på basspåret i
  stället för att slängas.
- **Notlängden följer med.** En ton som håller över ett steg tänder alla steg den
  klingar igenom. Ett trumslag tänder bara sitt eget steg.
- **Hela filen kommer in, takt för takt.** En fil på fyra takter blir fyra
  mönster, och de placeras som klossar på det valda spåret (takt 1–4), så att du
  ser dem och kan spela upp dem direkt. En fil på **en** takt fyller bara det
  markerade mönstret utan att sätta någon kloss — den som bygger mönster för hand
  vill placera dem själv.
- **Tempot** i filen föreslås bara om projektet står kvar på sin ursprungs-BPM.
  Annars vore en import en tyst tempoändring.
- **Statusraden säger vad som hände**: hur många noter, i hur många mönster, och
  vad som eventuellt hoppades över (tangenter utan trumkanal, toner utanför
  rutnätet, takter bortom arrangemangets 32).

### Vad som inte följer med

**Anslaget per not.** Modellen har stegvolymer — sexton delade värden per mönster
— och ingen plats för ett anslagsvärde per not. Det är en modellfråga, inte en
importfråga, och den står kvar som sådan.

Filer med en annan upplösning än 480 PPQ (t.ex. 96, som är vanligt) skalas rätt:
importen läser filens egen upplösning i stället för att anta en.

---

# Appendix A — README detail (English)
Moved out of `README.md` so the front page stays readable: per-feature detail and caveats (A1–A14), the complete shortcut table (A15), and the remaining 23 gallery views (A16). The Swedish manual above is unchanged.

## A1–A14. Feature detail (the bullets README keeps are the summary + every caveat)

### 1. 🎼 Timeline & Multitrack Arranger — ✅ Real
* **Continuous Waveform Phase Tracking:** Trimming the start shifts the waveform without squishing or distorting loop cycles.

### 1. 🎼 Timeline & Multitrack Arranger — ✅ Real
* **Magnetic Loop-Snap (🧲):** Locks onto exact whole-loop multiples (`1x`, `2x`, `3x`, `4x`, `8x`) and grid divisions (`1/16`, `Beat`, `Bar`). Hold `Alt` for free precision.

### 1. 🎼 Timeline & Multitrack Arranger — ✅ Real
* **Bottom Region Inspector:** Coarse/fine nudge (`±1 bar`, `±0.1s`), start-trim offset, gain, fade in/out, reverse playback, and loop multiplication.

### 1. 🎼 Timeline & Multitrack Arranger — ✅ Real
* **Tool Palette:** Select (⇱), Paint (✎), Slice (✂), Erase (🗑), Mute (🔇); quick-cut at playhead (Ctrl+B).

### 1. 🎼 Timeline & Multitrack Arranger — ✅ Real
* **Live In-Track Microphone Recording:** Arm buttons (`⏺`), real zero-latency direct monitoring and real-time waveforms on audio tracks.

### 1. 🎼 Timeline & Multitrack Arranger — ✅ Real
* **Automation Curves (📈):** Draw per-track curves for volume, pan, reverb and delay sends. Left-click to add points, drag to move (snaps to the grid), right-click to delete. Curves play back in real time and are saved with the project.

### 2. 🥁 Sonix Channel Rack (16-Step Sequencer) — ✅ Real
* **4-Beat Step Buttons** with glowing center LEDs.

### 2. 🥁 Sonix Channel Rack (16-Step Sequencer) — ✅ Real
* **Per-Channel Knobs:** Volume, pan, pitch (semitones + cents) and a **sample chopper** (start/end fractions + transient detection).

### 3. 🎹 Piano Roll & Interactive Touch Keyboard — ✅ Real
* **Playable Virtual Keyboard:** Neon feedback with computer-keyboard typing (A–K).

### 3. 🎹 Piano Roll & Interactive Touch Keyboard — ✅ Real
* **Real MIDI Keyboard Input (ALSA Seq):** Open a real MIDI-in port ("Sonix MIDI In"), connect a hardware keyboard with `aconnect`, play it live, and arm **⏺ MIDI-REC** to write held notes straight into the Piano Roll grid during playback.

### 4. 🎚 Mixer Console & Per-Track Effects — ✅ Real
* **Per-Track 3-Band Parametric EQ:** Draggable visual curve + quick presets.

### 4. 🎚 Mixer Console & Per-Track Effects — ✅ Real
* **Per-Track Dynamics & Sends:** Compressor, reverb/delay sends and pitch — all sent to the audio engine.

### 4. 🎚 Mixer Console & Per-Track Effects — ✅ Real
* **Master FX Rack:** Gate → 4-band EQ → compressor → de-esser → filter/drive → doubler → limiter, plus reverb & delay, with a **real gain-reduction meter** and a visual EQ editor.

### 4. 🎚 Mixer Console & Per-Track Effects — ✅ Real
* **Remix FX (live):** Kaoss-style XY pad with beat-repeat/stutter/reverse and a real tape-stop varispeed effect.

### 4. 🎚 Mixer Console & Per-Track Effects — ✅ Real
* **VCA Groups & Sub-Mix Buses:** 4 sub-mix buses (Vocal, Drums, Synth, FX) and 4 VCA groups, each with fader + mute/solo; per-track bus/VCA routing. Group gain is applied post-fader just before the master, and the state is saved in the project and honoured by offline export.

### 5. 🎛 Analog Alchemy Synth — ✅ Real
* ✅ **Per-voice:** Each note has its own filter state and filter envelope (the **ENV ±oct** knob sweeps the cutoff per note); envelope parameters are patch-global so knobs are heard live.

### 6. 🎙 Vocal Studio & Harmonizer — ✅ Real
* **Pitch Editor:** Draggable note blobs (Melodyne-style) and scale-aware correction.

### 6. 🎙 Vocal Studio & Harmonizer — ✅ Real
* **Autotune & 4-Part Harmonizer** with per-voice level and formant controls; harmony voices use **real formant preservation** (STFT + cepstral envelope correction) so pitch shifts no longer sound "chipmunk".

### 6. 🎙 Vocal Studio & Harmonizer — ✅ Real
* **Real-time Auto-Tune & direct monitoring:** The 🎙️ Real-time Auto-Tune checkbox corrects the microphone in the audio thread (rolling pitch detection + WSOLA streaming shifter) and is heard through true zero-latency direct monitoring (ring buffer → master bus). Strength = the AUTO-TUNE knob. Recording stays dry.

### 6. 🎙 Vocal Studio & Harmonizer — ✅ Real
* **Custom Sampler:** Record acoustic one-shots via microphone and map them to instruments/drums.

### 6. 🎙 Vocal Studio & Harmonizer — ✅ Real
* **Hardware Mic Panel:** Input device selector, hardware gain boost (+0 to +24 dB), noise gate, feedback suppression and vocal character presets.

### 7. 🎛 Creative Generators — ✅ Real
* **Beat & Melody Dice Generator:** 5 categories, genre presets, live preview → pattern.

### 7. 🎛 Creative Generators — ✅ Real
* **Session Drummer:** XY complexity/energy pad with 6 genre presets.

### 7. 🎛 Creative Generators — ✅ Real
* **Song Section Arranger:** Intro/Verse/Chorus/Bridge/Drop/Outro → timeline.

### 7. 🎛 Creative Generators — ✅ Real
* **Add Track Studio:** 19 templates across 6 categories.

### 7. 🎛 Creative Generators — ✅ Real
* **Hardware Strobe Tuner:** Real pitch detection, ±cents readout, 7 tuning presets and a reference tone.

### 9. 🤖 AI Music Assistant & Generation — ✅ Real (with honest limits)
* **Local fallback:** a deterministic, rule-based composer (scale/key aware) when no API is configured.

### 9. 🤖 AI Music Assistant & Generation — ✅ Real (with honest limits)
* **Suno/AI Stem Importer:** Unzip and decode real audio, detect BPM and map stems onto the timeline.

### 10. 🧠 Stem Separator — ✅ Real (DSP + optional neural HTDemucs)
* **Optional neural backend:** build with `--features neural` and place an HTDemucs-family `.onnx` model in `~/.config/sonix/models/` (or set `SONIX_DEMUCS_ONNX`) to run genuine Demucs separation through ONNX Runtime — background thread, live progress, 44.1 kHz resampling and overlap-add crossfading. Without a model it falls back to the DSP path and the UI states which backend ran.

### 11. 🔌 Plugin Manager — 🟡 Catalogue + opt-in CLAP/VST3/VST2 host
* **Opt-in CLAP host:** build with `--features plugin-host` and Sonix can `dlopen` a `.clap` plugin, validate its `clap_entry`, instantiate it, read its descriptor **and parameters** (the "🔎 Load & inspect" button shows them), **run audio through it on a stem track** with plug-in delay compensation (the "▶ Load" button inserts it), **save/restore its state with the project** and **read/drive its `clap.gui` lifecycle**.

### 11. 🔌 Plugin Manager — 🟡 Catalogue + opt-in CLAP/VST3/VST2 host
* **Plugin GUI in its own window:** an active insert opens the plugin's own UI with **"🪟 Open GUI"** — the host creates a real **X11 window** and embeds the editor via `clap.gui` (`set_parent`/`show`), and the audio and the GUI share the **same plugin instance**. The window is polled every UI frame and torn down cleanly (hide → destroy) when closed.

### 11. 🔌 Plugin Manager — 🟡 Catalogue + opt-in CLAP/VST3/VST2 host
* **Crash isolation in a separate process:** **"🧪 Sandbox inspect"** runs the plugin in its own process (the same binary re-executed with `--plugin-sandbox-worker`) over a length-prefixed JSON protocol and reads its info + parameters there; a supervisor **automatically restarts it on a crash** (up to three attempts) so Sonix does not go down. **"🧪 Load into sandbox"** goes further: the plugin's **audio processing itself** runs in that separate process, streaming stereo blocks over a **shared-memory ring buffer** (`memfd_create` + `mmap`), with the transport's one-block latency compensated by PDC and automatic re-sync after a restart.

### 12. 💿 Export & Project I/O — ✅ Real
* Clean metadata tagging (Sonix Studio only), master mix or per-track stems.

### 12. 💿 Export & Project I/O — ✅ Real
* **Loudness normalization (EBU R128):** Real ITU-R BS.1770 gated loudness (K-weighting) and true-peak ceiling via 4× oversampling. One-click export presets (Streaming −14, Apple Music −16, Broadcast −23, Club/Loud −9 LUFS).

### 12. 💿 Export & Project I/O — ✅ Real
* Project save/load and template projects.

### 14. 🌐 Localisation & System — ✅ Real
* **cpal/ALSA realtime engine** with a lock-free command ring and a crash-safe audio callback.

### 14. 🌐 Localisation & System — ✅ Real
* **Audio Settings** (Ctrl+P) apply the chosen sample rate and buffer size by rebuilding the output stream live; the choice is persisted to `~/.config/sonix/audio.json` and restored on launch. The current host/device and the real active stream config are shown.

## A15. Keyboard & Shortcut Reference

| Key / Shortcut | Function | Description |
| :--- | :--- | :--- |
| **Space** | **Play / Pause** | Toggle playback in Song or Pattern mode. |
| **R** | **Record Arm** | Arm active microphone track for live recording. |
| **Alt (Hold)** | **Free Slip / Trim** | Bypass magnetic loop and grid snapping for free editing. |
| **F1** | **User Manual / Help** | Open built-in interactive manual and help center. |
| **F3** | **Timeline / Arranger** | Switch to the linear multitrack audio playlist. |
| **F4** | **Channel Rack** | Switch to the 16-step sequencer and drum machine. |
| **F5** | **Piano Roll** | Switch to the graphical note editor and keyboard. |
| **F6** | **Mixer Console** | Switch to the multichannel mixer and effects rack. |
| **F7** | **Analog Synthesizer** | Switch to the synth editor with ADSR, Filter, and Morph Pad. |
| **F8** | **Vocal Studio** | Switch to the vocal recording console and harmonizer. |
| **Ctrl + I** | **Import Stems** | Open dialog to import Suno ZIP archives or audio files. |
| **Ctrl + E** | **Export Master / Stems** | Offline-render the full project (real samples, timeline audio & FX) to WAV, FLAC, MP3, OGG or AAC with clean Sonix-only metadata. |
| **Ctrl + S** | **Save Project** | Save the current project to disk. |
| **Ctrl + O** | **Open Project** | Open a saved project from disk. |
| **Ctrl + P** | **Audio Settings** | Open PipeWire, ALSA, and buffer size configuration. |

---

## A16. Gallery — the remaining views

#### 7. 🤖 AI Music Assistant & Prompt Engine
Generate chord progressions, melodies, basslines, and song ideas via natural language text prompts (OpenAI, Anthropic/Claude, OpenRouter, or local Ollama), with a built-in rule-based generator as fallback.
![AI Assistant](screenshots/07_ai_music_assistant.png)

#### 8. ✨ Sonix Alchemy Synthesizer
Advanced hybrid synth with 4 oscillator waveforms (Sine, Sawtooth, Square, Triangle), a resonant state-variable filter, ADSR envelope, and an 8-point morph vector pad.
![Alchemy Synth](screenshots/08_alchemy_synth.png)

#### 9. 🥁 Dynamic Session Drummer
Interactive XY control pad for groove complexity and energy dynamics, with humanize engine, style variations, and fill-ins.
![Session Drummer](screenshots/09_session_drummer.png)

#### 10. 🧩 Modular Patcher & The Grid
Visual modular node environment for connecting audio signals, filters, envelopes, LFOs, and distortion with virtual patch cables.
![Modular Patcher](screenshots/10_modular_patcher.png)

#### 11. 🧠 Stem Separator (DSP + optional Neural ONNX)
Source separation to isolate vocals, drums, bass, and instruments from mixed tracks. Runs a lightweight spectral DSP separator by default, or a real HTDemucs ONNX model when built with `--features neural` and a model is installed.
![Stem Separator](screenshots/11_stem_separator.png)

#### 12. 🔌 Plugin & VST/CLAP Bridge Manager
Cataloguing of FL Studio `.fst` presets and CLAP, VST3, VST2, LV2, and Wine/Yabridge plugin locations (scanning & verification — CLAP, **VST3 and VST2** processing opt-in, LV2 hosting not yet implemented).
![Plugin Manager](screenshots/12_plugin_manager.png)

#### 13. 🎛 Remix FX (Live Performance Pad)
Live DJ performance effects with Kaoss-style XY matrix, stutter repeater, vinyl tape stop, and reverse.
![Remix FX](screenshots/13_remix_fx.png)

---

### 3. 🎙 Creative Studio Panels & Vocal Modules (Soundtrap-Inspired)

#### 14. 🎤 Microphone & Hardware Vocal Panel
Input device selector, hardware gain boost (+0dB to +24dB), noise gate, acoustic feedback suppression, and vocal character presets.
![Microphone Settings](screenshots/21_dialog_mikrofon_installningar.png)

#### 15. 🎹 Smart Chord & Harmony Matrix
Scale-aware Roman numeral chord matrix, strum spread humanizer, block/arpeggiated styles, and one-click Piano Roll stamping.
![Chord Generator Matrix](screenshots/22_dialog_ackord_matris.png)

#### 16. 🎯 Hardware Strobe Tuner
Ultra-high precision chromatic strobe tuner with real-time frequency analysis, ±cents deviation, and 440Hz calibration.
![Hardware Strobe Tuner](screenshots/23_dialog_strobe_tuner.png)

#### 17. 🎲 Beat & Melody Dice Generator
Algorithmic generator for drum rhythms and melodic hooks with musical scale locks and syncopation controls.
![Dice Generator](screenshots/24_dialog_tarning_generator.png)

#### 18. 🎛 Modular FX Pedalboard & Vocal Rack
Multi-effect chain with Vocal Doubler, Analog Tube Compressor, Dynamic De-Esser, Noise Gate, and Resonant Filter.
![Modular FX Rack](screenshots/25_dialog_modular_fx_rack.png)

#### 19. 🎼 Song Section Arranger
Define and arrange Intro, Verse, Chorus, Bridge, Drop, and Outro sections on the timeline.
![Song Structure Arranger](screenshots/26_dialog_latstruktur_arrangemang.png)

#### 20. ➕ Add Track Studio Creator
Modal for quickly instantiating Vocals/Mic, Sample Audio, 808 Drums, Synth Lead, Bassline, or FX Bus tracks.
![Add Track Modal](screenshots/27_dialog_lagg_till_spar.png)

---

### 4. ⚙️ Dialogs, Modals & System Configuration

#### 21. ⚙ Audio & Driver Settings
Shows the real cpal host and output device, the active stream (sample rate · buffer · channels), and lets you apply a new sample rate/buffer by rebuilding the stream live (persisted to `~/.config/sonix/audio.json`).
![Audio Settings](screenshots/14_dialog_ljudinstallningar.png)

#### 22. 🤖 AI Configuration & API Keys
Setup credentials and connections for OpenAI, Anthropic/Claude, Stability Stable Audio, and local Ollama servers.
![AI Settings](screenshots/15_dialog_ai_installningar.png)

#### 23. 💾 Project Manager & Templates
Create new projects from music templates (Synthwave, Trap, House, Ambient), save, and load projects.
![Project Manager](screenshots/16_dialog_projekthanterare.png)

#### 24. 💿 Render & Master Export Queue
Offline full-project rendering (real channel WAV samples + timeline audio stems + FX) to WAV (16/24-bit & float), FLAC (24-bit lossless), MP3 (320 kbps), OGG Vorbis and AAC/M4A. Choose master mix or per-track dry/wet stems, sample rate and clean metadata (title/artist/album/genre/year/comment) tagged only with **Sonix Studio** – never AI/provider info. WAV/FLAC are encoded in-app; MP3/OGG/AAC use `ffmpeg` when installed. Includes **EBU R128 loudness normalization** (BS.1770 K-weighted gated measurement + true-peak ceiling via 4× oversampling) with one-click delivery presets (Streaming/Apple Music/Broadcast/Club).
![Render Queue](screenshots/17_dialog_render_queue.png)

#### 25. 📥 Suno AI Stem Importer
Automated loading and unzipping of Suno AI stems directly from zip archives or folders.
![Suno Import](screenshots/18_dialog_suno_import.png)

#### 26. 🎛 Hardware Controllers & MCU/OSC
Hardware controller support for Behringer X-Touch, Novation Launchpad, ALSA MIDI, and OSC network control.
![MIDI Controller](screenshots/19_dialog_midi_controller.png)

#### 27. 🔍 Focused Stem & Region Editor
Detailed audio region editor with volume envelopes, fade in/out curves, reverse, and sample slicing.
![Stem Focus Editor](screenshots/20_dialog_stem_focus_editor.png)

#### 28. 📖 Interactive Help Guide & Manual
Built-in comprehensive manual with shortcut lists, signal flow diagrams, and workflow guides.
![Help Guide](screenshots/28_dialog_hjalpguide_manual.png)

#### 29. ℹ About Sonix Studio
Version information, audio engine details, system architecture, and license.
![About Sonix](screenshots/29_dialog_om_sonix.png)

