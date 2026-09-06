# 🍊 SONIX STUDIO - Professionell Linux DAW

Ett modernt, blixtsnabbt grafiskt musikproduktionsprogram skapat i **Rust** och **egui** med en integrerad modulär synthesizer, linjär flerspårs-audioredigerare med Suno AI-stöd, 16-stegs Channel Rack, Piano Roll och Touch-instrument för Linux (PipeWire / Wayland / ALSA / JACK).

---

## 🚀 Starta appen

Starta direkt från applikationsmenyn i Omarchy / Linux Desktop, eller kör från terminalen:

```bash
sonix
```

Eller kompilera och kör från källkod:

```bash
cd ~/Projects/sonix
cargo run --release
```

För att automatiskt generera skärmdumpar av alla vyer och modaler:

```bash
cargo run --release -- --capture-screenshots screenshots
```

---

## 📸 Skärmdumpar & Gränssnitt (Galleri)

### 1. 🎼 Huvudarbetsytor & Produktion

#### 1. 🎼 Tidslinje & Flerspårs-Arranger (Playlist)
Linjär flerspårsredigerare med stöd för Suno AI-stems, kontinuerliga vågformer, verktygspalett (Pointer, Paint, Slice ✂, Mute, Trash), snap-grid och transportkontroller.
![Tidslinje & Arranger](screenshots/01_tidslinje_arranger.png)

#### 2. 🥁 16-Stegs Channel Rack & Trummaskin
Taktila steg-knappar med vita center-LEDs, mute/solo-status, dedikerade rotary-rattar för volym och panorering per kanal samt snabbval av samplingar.
![Channel Rack](screenshots/02_channel_rack.png)

#### 3. 🎹 Piano Roll & Interaktivt Touch-Klaviatur
Polyfonisk noteditor med notlängder, anslagsdynamik (velocity), skal-snäppning samt spelbart klaviatur med neonvisuell feedback.
![Piano Roll](screenshots/03_piano_roll.png)

#### 4. 🎙 Sångstudio & Mikrofoninspelning (Take Lanes)
Professionell inspelningskonsol för sång och instrument med realtids pitch-detektering, autotune, formant-skiftning och 4-stämmig harmoniserare.
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
Visuell modulär nod-miljö i Bitwig/FL Patcher-stil för att koppla ihop ljudsignaler, filter, envelopes, LFO och distorsion med kablar.
![Modular Patcher](screenshots/10_modular_patcher.png)

#### 11. 🧠 AI Stem Separator (Demucs Neural Engine)
Källseparation som delar upp färdiga mixar eller låtar i separata spår för sång, trummor, bas och instrument.
![Stem Separator](screenshots/11_stem_separator.png)

#### 12. 🔌 Plugin & VST/CLAP Bridge Manager
Sömlös hantering av CLAP-, VST3- och LV2-plugins för Linux samt isolerad process-sandboxing för Windows VST via Yabridge/Wine.
![Plugin Manager](screenshots/12_plugin_manager.png)

#### 13. 🎛 Remix FX (Live Performance Pad)
DJ-liveeffekter med Kaoss-liknande XY-matris, stutter-repeater, vinyl tape stop, bitcrusher och filter sweeps.
![Remix FX](screenshots/13_remix_fx.png)

---

### 3. ⚙️ Dialogrutor, Modaler & Konfiguration

#### 14. ⚙ Ljud- & Systeminställningar
Konfiguration av ljudkort, buffertstorlek (1.4 ms low-latency), samplingsfrekvens och PipeWire/ALSA-drivrutin.
![Ljudinställningar](screenshots/14_dialog_ljudinstallningar.png)

#### 15. 🤖 AI-Konfiguration & API-nycklar
Anslutningsinställningar för Suno AI, OpenAI GPT, Claude AI och lokal Ollama-server.
![AI Inställningar](screenshots/15_dialog_ai_installningar.png)

#### 16. 💾 Projekthanterare & Projektmallar
Skapa nya låtprojekt från mallar (Synthwave, Trap, House, Ambient) samt spara och öppna projekt.
![Projekthanterare](screenshots/16_dialog_projekthanterare.png)

#### 17. 💿 Render & Master Export Queue
Flertrådad master-rendering till WAV, MP3 eller FLAC med stöd för batch-export av enskilda stems.
![Render Queue](screenshots/17_dialog_render_queue.png)

#### 18. 📥 Suno AI Stem Importör
Automatisk inläsning och uppackning av Suno AI-stems direkt från zip-arkiv eller mappar.
![Suno Import](screenshots/18_dialog_suno_import.png)

#### 19. 🎛 Hårdvarukontroller & MCU/OSC
Stöd för externa hårdvarukontroller (Behringer X-Touch, Novation Launchpad) via ALSA MIDI och OSC.
![MIDI Controller](screenshots/19_dialog_midi_controller.png)

#### 20. 🔍 Fokuserad Stem & Region Editor
Detaljerad ljudregionsredigerare med volymkurva, fade in/out, reverse och sample slicing.
![Stem Focus Editor](screenshots/20_dialog_stem_focus_editor.png)

#### 21. 📖 Interaktiv Hjälpguide & Manual
Inbyggd snabbguide med kortkommandon, signalflödesscheman och arbetsflödesbeskrivningar.
![Hjälpguide](screenshots/21_dialog_hjalpguide_manual.png)

#### 22. ℹ Om Sonix Studio
Information om versionsnummer, ljudmotor, arkitektur och licens.
![Om Sonix](screenshots/22_dialog_om_sonix.png)

