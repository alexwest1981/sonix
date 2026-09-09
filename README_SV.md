# 🍊 SONIX STUDIO - Professionell Linux DAW

> **Språk / Languages:** [🇸🇪 Svenska](README_SV.md) | [🇬🇧 English](README.md)

Ett modernt, blixtsnabbt grafiskt musikproduktionsprogram skapat i **Rust** och **egui** med FL Studio-inspirerad direkt redigering i tidslinjen, magnetisk loop-snäppning, integration av FL Studio- & VST3/CLAP-plugins, Soundtrap-inspirerade kreativa paneler, integrerad modulär synthesizer, 16-stegs Channel Rack, Piano Roll, Sångstudio med mikrofoninspelning i realtid, multikanals mixerbord och touch-instrument för Linux (PipeWire / Wayland / ALSA / JACK).

---

## 🚀 Installation (Linux / Omarchy)

Sonix är en inhemsk Linux-DAW skriven i **Rust** och **egui**. Du kör den genom att kompilera från källkod med Cargo. Det inbyggda läget som tar de 29 skärmdumparna används bara av underhållarna (se notisen under galleriet nedan) och är **inte** en del av att installera eller köra programmet.

> ⚡ **Snabbaste sättet (guidad installerare):**
> Ladda ner och kör `install.sh` – den bekräftar varje steg åt dig (beroenden, kompilering, binär + startmenypost) och kräver inga terminalkommandon utöver:
> ```bash
> bash <(curl -fsSL https://raw.githubusercontent.com/alexwest1981/sonix/master/install.sh)
> ```
> Har du redan en klon kör du bara `bash install.sh` inifrån mappen. Efter en kodändring kan du uppdatera binär + startmenypost med `bash install.sh --refresh`.

### Förutsättningar

- 64-bitars Linux med en ljudserver i gång — **PipeWire** (rekommenderas), **JACK** eller **ALSA**.
- **Rust** (stable, edition 2024) plus en C-kompilator.
- Ett par körtidsverktyg som används av vissa funktioner: `zenity` (fildialoger) och `unzip` (Suno ZIP-import). Båda valfria.
- Valfritt, för Windows-VST: **yabridge** + **Wine**.

Installera förutsättningarna:

**Omarchy / Arch Linux:**

```bash
sudo pacman -S --needed base-devel alsa-lib zenity unzip rustup
rustup default stable
```

**Debian / Ubuntu:**

```bash
sudo apt install build-essential libasound2-dev zenity unzip curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

**Fedora:**

```bash
sudo dnf install gcc-c++ alsa-lib-devel zenity unzip curl
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

### 1. Klona och kompilera

```bash
git clone https://github.com/alexwest1981/sonix.git
cd sonix
cargo build --release
./target/release/sonix
```

Under utveckling kan du hoppa över det manuella byggsteget och använda `cargo run --release` i stället.

### 2. Installera binären (valfritt men rekommenderat)

Installerar `sonix` i `~/.cargo/bin` så att du kan starta programmet var som helst ifrån eller från en applikationsmeny:

```bash
cargo install --path .
sonix
```

### 3. Menypost i applikationsmenyn (Omarchy / Linux desktop)

Skapa efter `cargo install` en startare (justera sökvägarna om din klon ligger på annan plats):

```bash
mkdir -p ~/.local/share/applications
cat > ~/.local/share/applications/sonix.desktop <<EOF
[Desktop Entry]
Name=Sonix Studio
Comment=Native Linux DAW
Exec=$HOME/.cargo/bin/sonix
Icon=$HOME/Projects/sonix/assets/sonix.png
Terminal=false
Type=Application
Categories=Audio;AudioVideo;Music;
StartupWMClass=sonix-daw
EOF
```

Logga sedan ut/in en gång, eller uppdatera menyn direkt med:

```bash
update-desktop-database ~/.local/share/applications
```

### Felsökning

- **Inget ljud / ingen ljudenhet:** kontrollera att PipeWire (eller JACK/ALSA) är i gång och att din mikrofon/audiointerface är anslutet. Öppna ljudinställningarna i programmet med **Ctrl + P** för att välja enhet och buffertstorlek.
- **Fönstret startar inte:** du behöver en fungerande Wayland- eller X11-session med Mesa/OpenGL-drivrutiner.

---

## 🎨 Huvudfunktioner i Sonix Studio

### 1. 🎼 FL Studio-inspirerad Tidslinje & Flerspårs-Arranger
* **Dra direkt i klippkanter (Paint & Select):** Ta tag i högerkanten för att förlänga och loopa klipp sömlöst över takter, eller dra åt vänster för att korta av. Ta tag i vänsterkanten för att trimma/klippa bort början utan destruktiva ingrepp.
* **Kontinuerlig vågformsfasning:** Justering av starttrim flyttar ljudvågens startpunkt naturligt utan att komprimera eller förvränga efterföljande loopcykler.
* **Magnetisk Loop-Snap (🧲):** Låser automatiskt mot exakta multiplar av hela looplängder (`1x`, `2x`, `3x`, `4x`, `8x`) samt tidslinjens rutnät (`1/16`, `Beat`, `Bar`). Håll `Alt` för fri millimeterprecision.
* **Nedre Region-Inspektor:** Finjustera ljudklipp med grov- och finjusteringsknappar (`±1 takt`, `±0.1s`), offset-reglage för starttrim, gain/volym, fade in/out-kurvor, reverse-uppspelning och loop-multiplikator.
* **Verktygspalett:** Välj (⇱), Rita (✎), Klipp (✂), Radera (🗑), Muta (🔇).
* **Direkt mikrofoninspelning i spår:** Spela in leadsång och akustiska instrument direkt på audiospåret med dedikerad armeringsknapp (`⏺`), ingångsmonitorering och realtidsvågformer.

### 2. 🔌 FL Studio Native, .fst & Plugin-integration
* **FL Studio Native-kompatibilitet:** Inbyggd skanning och laddning av FL Studio-instrument och effekter (`.dll` VSTi såsom Sytrus, Harmor, Harmless, Gross Beat, FL Studio VSTi).
* **FL Studio Preset (.fst)-hantering:** Läs och importera FL Studio-presets direkt in i dina projekt.
* **Universell Plugin-värd:** Inbyggt Linux-stöd för **CLAP**-, **VST3**- och **LV2**-plugins.
* **Sandboxing för Windows VST:** Automatisk avsökning av Yabridge & Wine-prefix med isolerad, kraschsäker processexekvering.
* **Anpassade sökvägar:** Lägg till egna VST/CLAP-kataloger med rekursiv genomsökning och realtidsräknare.

### 3. 🎙 Kreativa Studiopaneler & Sångverktyg (Soundtrap-Inspirerat)
* **Hårdvarumikrofon & Röstpanel:** Enhetsväljare, hårdvarugain (+0dB till +24dB), brusgrind (noise gate), akustisk återkopplingsdämpning och sångförinställningar (Broadcast, Warm Tube, Crystal Lead, Rap, Podcast).
* **Smart Ackord- & Harmonimatris:** Skapa skalanpassade ackordföljder med romerska siffror (I, ii, iii, IV, V, vi, vii°), humaniserad strum-förskjutning och direktstämpling av ackord till Piano Roll.
* **Hårdvaru-Strobetuner:** Ultrahögprecisions kromatisk strobetuner med frekvensanalys i realtid, ±cents-avvikelse och 440 Hz-kalibrering.
* **Beat- & Meloditärning:** Algoritmisk slumpgenerator för trumgrooves och melodislingor med tonartslås och synkopkontroller.
* **Modulärt FX-Pedalbord & Vocal Rack:** Vocal doubler, analog rörkompressor, dynamisk de-esser, brusgrind och resonant multifilter.
* **Låtstruktur-Arrangör:** Definiera och arrangera Intro, Vers, Refräng, Stick, Drop och Outro på tidslinjen.
* **Spårskapare:** Snabbvalsmodal för att skapa Sång/Mikrofon-, Ljudsample-, 808 Trum-, Syntlead-, Bas- eller FX-buss-spår.

### 4. 🥁 Sonix Channel Rack (16-Stegs Sequencer)
* **Dedikerade kanalstrippar:** Kick, Snare, Closed Hat, Open Hat, Synth Lead, Sub Bass och egna användarkanaler.
* **4-Takts Stegknappar:** Taktila knappar grupperade i fyror med lysande vita center-LEDs vid aktivering.
* **Kanalrattar:** Snabb åtkomst till volym, panorering, pitch shifting och sample slicing.

### 5. 🎹 Piano Roll & Interaktivt Touch-Klaviatur
* **Polyfonisk Noteditor:** Dynamiska notlängder, anslagsdynamik (velocity), skalsnäppning och penselverktyg.
* **Spelbart Virtuellt Klaviatur:** Neonvisuell feedback med stöd för datortangentbord (A-K).

### 6. 🎙 Sångstudio & Intelligent Harmonizer
* **Take Lanes:** Spela in flera sångtagningar med icke-destruktiv comping.
* **Pitch Detection & Autotune:** Realtids pitchkorrigering och formantskiftning.
* **4-Stämmig Harmonizer:** Generera omedelbara stämmor och bakgrundskörer.
* **Egen Sampler:** Spela in akustiska instrument och samplingar direkt via mikrofonen.

### 7. 🎚 Master & Flerspårs Effects Mixer
* **8+ Kanalstrippar:** Dedikerade faders, VU-mätare, solo, mute, pan och stereobredd.
* **Sub-Mix Bussar & VCA:** Dedikerad Drum Bus, Vocal Bus, Synth Bus samt 4x VCA-faders.
* **Parametrisk 3-Bands EQ:** Visuell frekvenskurva med justerbara frekvenser och Q-faktor.
* **Master Effektrack:** Space Reverb, Stereo Delay, Chorus, Kompressor och Tube Drive.

### 8. 🤖 AI Musikassistent, Modulär Patcher & Demucs Stems
* **AI Prompt Engine:** Textprompt-baserad generering av ackordföljder, basgångar och melodier (Suno AI, OpenAI, Claude, lokal Ollama).
* **Suno AI Stem-importör:** Dra och släpp Suno ZIP-arkiv med automatisk extrahering och tidslinjeplacering.
* **Modulärt Patcher-nät:** Visuell nodbaserad signalrouting med virtuella patchkablar.
* **Demucs AI Stem Separator:** Källseparation för att isolera sång, trummor, bas och instrument från färdiga mixar.
* **Remix FX:** Kaoss-liknande XY-matris, stutter-repeater, vinyl tape stop och bitcrusher.

---

## ⌨️ Tangentbords- & Snabbkommandoreferens

| Tangent / Genväg | Funktion | Beskrivning |
| :--- | :--- | :--- |
| **Mellanslag (Space)** | **Play / Pause** | Startar eller pausar uppspelningen i låt- eller mönsterläge. |
| **R** | **Record Arm** | Armera aktivt mikrofonspår för direktinspelning. |
| **Alt (Håll ned)** | **Fri Trim / Slip** | Förbikoppla magnetisk loop- och rutnätssnäppning för fri redigering. |
| **F1** | **Bruksanvisning / Manual** | Öppnar den inbyggda manualen och snabbguiden. |
| **F3** | **Tidslinje / Arranger** | Växlar till den linjära flerspårs-audiotidslinjen. |
| **F4** | **Channel Rack** | Växlar till 16-stegs trummaskinen och mönstereditorn. |
| **F5** | **Piano Roll** | Växlar till det grafiska notinmatningsfönstret. |
| **F6** | **Mixer Console** | Växlar till flerspårs-mixern och effektracken. |
| **F7** | **Analog Synthesizer** | Växlar till synteditorn med ADSR, Filter och Morph. |
| **F8** | **Vocal Studio** | Växlar till sångstudion, inspelning och harmonizern. |
| **Ctrl + I** | **Importera Stämmor** | Öppnar dialogen för att importera Suno- eller ljudfiler. |
| **Ctrl + E** | **Exportera Master / Stems** | Offline-renderar hela projektet (riktiga samples, tidslinje-audio & FX) till WAV, FLAC, MP3, OGG eller AAC med ren Sonix-metadata. |
| **Ctrl + S** | **Spara Projekt** | Sparar det aktuella projektet. |
| **Ctrl + O** | **Öppna Projekt** | Öppnar ett sparat projekt från disk. |
| **Ctrl + P** | **Ljudinställningar** | Öppnar konfiguration för PipeWire, ALSA och buffertstorlek. |

---

## 📸 Skärmdumpar & Gränssnitt (Galleri 1920 × 1200 Helskärm)

> ℹ️ Dessa förhandsvisningar genereras av underhållarna med `cargo run --release -- --capture-screenshots screenshots` (öppnar en helskärmsinstans och tar skärmdumpar av varje vy). Krävs inte för att installera eller köra Sonix.

### 1. 🎼 Huvudarbetsytor & Produktion

#### 1. 🎼 Tidslinje & Flerspårs-Arranger (Playlist)
Linjär flerspårsredigerare med FL Studio-liknande klippdragning, magnetisk loop-snäppning, kontinuerliga vågformer, verktygspalett och direkt mikrofoninspelning.
![Tidslinje & Arranger](screenshots/01_tidslinje_arranger.png)

#### 2. 🥁 16-Stegs Channel Rack & Trummaskin
Taktila steg-knappar med vita center-LEDs, mute/solo-status, dedikerade rotary-rattar för volym och panorering per kanal samt snabbval av samplingar.
![Channel Rack](screenshots/02_channel_rack.png)

#### 3. 🎹 Piano Roll & Interaktivt Touch-Klaviatur
Polyfonisk noteditor med notlängder, anslagsdynamik (velocity), skalsnäppning samt spelbart klaviatur med neonvisuell feedback.
![Piano Roll](screenshots/03_piano_roll.png)

#### 4. 🎙 Sångstudio & Mikrofoninspelning (Take Lanes)
Professionell inspelningskonsol för sång och akustiska instrument med realtids pitch-detektering, autotune, formantskiftning och 4-stämmig harmoniserare.
![Sångstudio Take Lanes](screenshots/04_vocal_studio_leads.png)

#### 5. 🎤 Sångstudio: Spela in Egna Ljud & Sampler
Spela in egna akustiska instrument eller samplingar direkt från mikrofon och mappa om dem till syntar och trumspår.
![Sångstudio Sampler](screenshots/05_vocal_studio_sampler.png)

#### 6. 🎚 Master & Flerspårs Effects Mixer
Fullt mixerbord med individuella kanalstrippar, VU-mätare, VCA-grupper, Sub-Mix bussar, parametrisk 3-bands EQ och rumsreverb/delay sends.
![Mixer](screenshots/06_effects_mixer.png)

---

### 2. 🎛️ Syntar, AI-Moduler & Signalbehandling

#### 7. 🤖 AI Music Assistant & Prompt Engine
Skapa ackordföljder, melodier, basgångar och strukturidéer via naturlig textprompt (Suno AI, OpenAI, Claude eller lokal Ollama).
![AI Assistent](screenshots/07_ai_music_assistant.png)

#### 8. ✨ Sonix Alchemy Synthesizer
Avancerad hybridsynt med 4 oscillatorer (Sinus, Sågtand, Fyrkant, Triangel), Moog 24dB filter, ADSR-envelope och en 8-punkters realtids-morph-vektorplatta.
![Alchemy Synth](screenshots/08_alchemy_synth.png)

#### 9. 🥁 Dynamic Session Drummer
Interaktiv XY-kontrollmatris för groove-komplexitet och dynamik, med humanize-motor, stilvariationer och fill-ins.
![Session Drummer](screenshots/09_session_drummer.png)

#### 10. 🧩 Modular Patcher & The Grid
Visuell modulär nod-miljö för att koppla ihop ljudsignaler, filter, envelopes, LFO och distorsion med virtuella patchkablar.
![Modular Patcher](screenshots/10_modular_patcher.png)

#### 11. 🧠 AI Stem Separator (Demucs Neural Engine)
Källseparation som delar upp färdiga mixar eller låtar i separata spår för sång, trummor, bas och instrument.
![Stem Separator](screenshots/11_stem_separator.png)

#### 12. 🔌 Plugin & VST/CLAP Bridge Manager
Sömlös hantering av FL Studio Native-plugins, `.fst`-presets, CLAP-, VST3- och LV2-plugins samt isolerad process-sandboxing för Windows VST via Yabridge/Wine.
![Plugin Manager](screenshots/12_plugin_manager.png)

#### 13. 🎛 Remix FX (Live Performance Pad)
DJ-liveeffekter med Kaoss-liknande XY-matris, stutter-repeater, vinyl tape stop, bitcrusher och filter sweeps.
![Remix FX](screenshots/13_remix_fx.png)

---

### 3. 🎙 Kreativa Studiopaneler & Sångmoduler (Soundtrap-Inspirerat)

#### 14. 🎤 Mikrofon & Hårdvaru-Röstpanel
Enhetsväljare, hårdvarugain (+0dB till +24dB), brusgrind (noise gate), akustisk återkopplingsdämpning och sångkaraktärer.
![Mikrofoninställningar](screenshots/21_dialog_mikrofon_installningar.png)

#### 15. 🎹 Smart Ackord- & Harmonimatris
Skalanpassad ackordmatris med romerska siffror, strum spread-humanisering, block/arpeggio-stilar och direktstämpling till Piano Roll.
![Ackordmatris](screenshots/22_dialog_ackord_matris.png)

#### 16. 🎯 Hårdvaru-Strobetuner
Ultrahögprecisions kromatisk strobetuner med realtids frekvensanalys, ±cents-avvikelse och 440 Hz-kalibrering.
![Strobe Tuner](screenshots/23_dialog_strobe_tuner.png)

#### 17. 🎲 Beat- & Meloditärning
Algoritmisk generator för trumrytmer och melodislingor med musikaliska tonartslås och synkopkontroller.
![Tärningsgenerator](screenshots/24_dialog_tarning_generator.png)

#### 18. 🎛 Modulärt FX-Pedalbord & Vocal Rack
Multieffektkedja med Vocal Doubler, Analog Rörkompressor, Dynamisk De-Esser, Brusgrind och Resonant Filter.
![Modulärt FX-Rack](screenshots/25_dialog_modular_fx_rack.png)

#### 19. 🎼 Låtstruktur-Arrangör
Definiera och arrangera sektioner för Intro, Vers, Refräng, Stick, Drop och Outro på tidslinjen.
![Låtstruktur](screenshots/26_dialog_latstruktur_arrangemang.png)

#### 20. ➕ Spårskapare (Add Track Creator)
Modal för att snabbt skapa Sång/Mikrofon-, Ljudsample-, 808 Trum-, Syntlead-, Bas- eller FX-buss-spår.
![Lägg till spår](screenshots/27_dialog_lagg_till_spar.png)

---

### 4. ⚙️ Dialogrutor, Modaler & Konfiguration

#### 21. ⚙ Ljud- & Systeminställningar
Konfiguration av ljudkort, buffertstorlek (1.4 ms low-latency), samplingsfrekvens och PipeWire/ALSA-drivrutin.
![Ljudinställningar](screenshots/14_dialog_ljudinstallningar.png)

#### 22. 🤖 AI-Konfiguration & API-nycklar
Anslutningsinställningar för Suno AI, OpenAI GPT, Claude AI och lokal Ollama-server.
![AI Inställningar](screenshots/15_dialog_ai_installningar.png)

#### 23. 💾 Projekthanterare & Projektmallar
Skapa nya låtprojekt från mallar (Synthwave, Trap, House, Ambient) samt spara och öppna projekt.
![Projekthanterare](screenshots/16_dialog_projekthanterare.png)

#### 24. 💿 Render & Master Export Queue
Offline-rendering av hela projektet (riktiga Channel Rack-WAV-samples + tidslinje-audio-stems + FX) till WAV (16/24-bit & float), FLAC (24-bit lossless), MP3 (320 kbps), OGG Vorbis och AAC/M4A. Välj mastermix eller per-spår torra/våta stems, samplingsfrekvens och ren metadata (titel/artist/album/genre/år/kommentar) som bara taggas med **Sonix Studio** – aldrig AI-/leverantörsinfo. WAV/FLAC kodas i appen; MP3/OGG/AAC via `ffmpeg` om det är installerat.
![Render Queue](screenshots/17_dialog_render_queue.png)

#### 25. 📥 Suno AI Stem Importör
Automatisk inläsning och uppackning av Suno AI-stems direkt från zip-arkiv eller mappar.
![Suno Import](screenshots/18_dialog_suno_import.png)

#### 26. 🎛 Hårdvarukontroller & MCU/OSC
Stöd för externa hårdvarukontroller (Behringer X-Touch, Novation Launchpad) via ALSA MIDI och OSC.
![MIDI Controller](screenshots/19_dialog_midi_controller.png)

#### 27. 🔍 Fokuserad Stem & Region Editor
Detaljerad ljudregionsredigerare med volymkurva, fade in/out, reverse och sample slicing.
![Stem Focus Editor](screenshots/20_dialog_stem_focus_editor.png)

#### 28. 📖 Interaktiv Hjälpguide & Manual
Inbyggd snabbguide med kortkommandon, signalflödesscheman och arbetsflödesbeskrivningar.
![Hjälpguide](screenshots/28_dialog_hjalpguide_manual.png)

#### 29. ℹ Om Sonix Studio
Information om versionsnummer, ljudmotor, arkitektur och licens.
![Om Sonix](screenshots/29_dialog_om_sonix.png)



