# 🍊 SONIX STUDIO - Professional Native Linux DAW

> **Languages / Språk:** [🇬🇧 English](README.md) | [🇸🇪 Svenska](README_SV.md)

A modern, lightning-fast graphical Digital Audio Workstation (DAW) built with **Rust** and **egui**, featuring FL Studio-style direct timeline audio editing, magnetic loop snapping, FL Studio & VST3/CLAP plugin integration, Soundtrap-style creative panels, an integrated modular synthesizer, 16-step Channel Rack, Piano Roll, Vocal Studio with live microphone recording, Multichannel Effects Mixer, and Touch Instruments for Linux (PipeWire / Wayland / ALSA / JACK).

---

## 🚀 Getting Started

Launch directly from your Linux / Omarchy Desktop application menu, or run from terminal:

```bash
sonix
```

Or build and run from source code:

```bash
cd ~/Projects/sonix
cargo run --release
```

To automatically generate all 29 full-screen screenshots:

```bash
cargo run --release -- --capture-screenshots screenshots
```

---

## 🎨 Core Features in Sonix Studio

### 1. 🎼 FL Studio-Style Timeline & Multitrack Audio Arranger
* **Direct Clip Edge Dragging (Paint & Select Tools):** Grab the right edge to extend and loop clips seamlessly across bars, or drag left to shorten. Grab the left edge to trim/crop the start without destructive audio edits.
* **Continuous Waveform Phase Tracking:** Trimming the start shifts the waveform without squishing or distorting loop cycles.
* **Magnetic Loop-Snap (🧲):** Automatically locks onto exact whole-loop multiples (`1x`, `2x`, `3x`, `4x`, `8x` loop lengths) and timeline grid divisions (`1/16`, `Beat`, `Bar`). Hold `Alt` for free millisecond-level precision.
* **Bottom Region Inspector:** Fine-tune audio clips with coarse/fine nudge buttons (`±1 bar`, `±0.1s`), start-trim offset, gain slider, fade in/out curves, reverse playback, and loop multiplication.
* **Tool Palette:** Select (⇱), Paint (✎), Slice (✂), Erase (🗑), Mute (🔇).
* **Live In-Track Microphone Recording:** Record live vocals directly onto audio tracks with dedicated arm buttons (`⏺`), input monitoring, and real-time waveforms.

### 2. 🔌 FL Studio Native, .fst & 3rd-Party Plugin Integration
* **FL Studio Native Compatibility:** Built-in scanner and loader for FL Studio instruments and effects (`.dll` VSTi like Sytrus, Harmor, Harmless, Gross Beat, FL Studio VSTi).
* **FL Studio Preset (.fst) Management:** Read and import FL Studio preset files directly into your projects.
* **Universal Plugin Host:** Native Linux support for **CLAP**, **VST3**, and **LV2** plugins.
* **Windows VST Sandboxing:** Automatic Yabridge & Wine prefix scanning with isolated crash-safe process execution.
* **Custom Scan Directories:** Add custom VST/CLAP folders with recursive scanning and real-time status counters.

### 3. 🎙 Creative Studio Panels & Vocal Tools (Soundtrap-Inspired)
* **Hardware Microphone & Vocal Panel:** Device selector, hardware gain boost (+0dB to +24dB), noise gate, acoustic feedback suppression, and vocal presets (Broadcast, Warm Tube, Crystal Lead, Rap, Podcast).
* **Smart Chord & Harmony Matrix:** Generate scale-aware Roman numeral chord progressions (I, ii, iii, IV, V, vi, vii°), strum humanization, and one-click chord stamping into the Piano Roll.
* **Hardware Strobe Tuner:** Ultra-high precision chromatic strobe tuner with real-time pitch detection, ±cents readout, and frequency in Hz.
* **Beat & Melody Dice Generator:** Algorithmic groove and melody generator with scale constraints and syncopation controls.
* **Modular FX Pedalboard & Vocal Rack:** Vocal doubler, analog tube compressor, dynamic de-esser, noise gate, and resonant multi-filter.
* **Song Section Arranger:** Define and arrange Intro, Verse, Chorus, Bridge, Drop, and Outro song markers.
* **Add Track Studio Creator:** Modal for adding Vocals/Mic, Custom Audio, 808 Drums, Synth Lead, Bassline, or FX Bus tracks.

### 4. 🥁 Sonix Channel Rack (16-Step Sequencer)
* **Dedicated Channel Strips:** Kick, Snare, Closed Hat, Open Hat, Synth Lead, Sub Bass, and custom user channels.
* **4-Beat Step Buttons:** Tactile buttons grouped in fours with glowing center LEDs when active.
* **Per-Channel Knobs:** Quick access to volume, panning, pitch shifting, and sample chopping.

### 5. 🎹 Piano Roll & Interactive Touch Keyboard
* **Polyphonic Note Editor:** Dynamic note lengths, velocity editing, scale snapping, and brush tools.
* **Playable Virtual Keyboard:** Neon visual feedback with computer keyboard typing support (A-K).

### 6. 🎙 Vocal Studio & Intelligent Harmonizer
* **Take Lanes:** Record multiple vocal takes with non-destructive comping.
* **Pitch Detection & Autotune:** Real-time pitch correction and formant shifting.
* **4-Part Harmonizer:** Instant vocal harmonies and backing choir generation.
* **Custom Sampler:** Record acoustic instruments and one-shots directly via microphone.

### 7. 🎚 Master & Multitrack Effects Mixer
* **8+ Channel Strips:** Dedicated faders, peak VU meters, solo, mute, pan, and stereo width.
* **Sub-Mix Busses & VCA:** Dedicated Drum Bus, Vocal Bus, Synth Bus, and 4x VCA faders.
* **Parametric 3-Band EQ:** Visual curve graph with adjustable frequencies and Q-factor.
* **Master FX Rack:** Space Reverb, Stereo Delay, Chorus, Compressor, and Tube Drive.

### 8. 🤖 AI Music Assistant, Modular Patcher & Demucs Stems
* **AI Prompt Engine:** Natural language prompt-based generation for chords, basslines, and melodies (Suno AI, OpenAI, Claude, local Ollama).
* **Suno AI Stem Importer:** Drag & drop Suno ZIP archives with automated stem track extraction and alignment.
* **Modular Patcher Grid:** Visual node-based signal routing with virtual patch cables.
* **Demucs AI Stem Separator:** Neural source separation to isolate vocals, drums, bass, and instruments from full audio mixes.
* **Remix FX:** Kaoss-style XY matrix pad, stutter repeater, vinyl tape stop, and bitcrusher.

---

## ⌨️ Keyboard & Shortcut Reference

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
| **Ctrl + E** | **Export Master / WAV** | Open batch render queue to export WAV, MP3, or stems. |
| **Ctrl + S** | **Save Project** | Save the current project to disk. |
| **Ctrl + O** | **Open Project** | Open a saved project from disk. |
| **Ctrl + P** | **Audio Settings** | Open PipeWire, ALSA, and buffer size configuration. |

---

## 📸 Screenshots & UI Gallery (1920 × 1200 Fullscreen)

### 1. 🎼 Main Workspaces & Production

#### 1. 🎼 Timeline & Multitrack Arranger (Playlist)
Linear multitrack editor with FL Studio-style clip dragging, magnetic loop snapping, continuous waveforms, tool palette, and live microphone track.
![Timeline & Arranger](screenshots/01_tidslinje_arranger.png)

#### 2. 🥁 16-Step Channel Rack & Drum Sequencer
Tactile step buttons with glowing center LEDs, mute/solo status, dedicated volume/pan rotary knobs, and sample quick-select.
![Channel Rack](screenshots/02_channel_rack.png)

#### 3. 🎹 Piano Roll & Interactive Touch Keyboard
Polyphonic note editor with note lengths, velocity, scale snapping, and playable keyboard with neon visual feedback.
![Piano Roll](screenshots/03_piano_roll.png)

#### 4. 🎙 Vocal Studio & Microphone Recording (Take Lanes)
Professional recording console for vocals and acoustic instruments with pitch detection, autotune, formant shifting, and 4-part harmonizer.
![Vocal Studio Take Lanes](screenshots/04_vocal_studio_leads.png)

#### 5. 🎤 Vocal Studio: Custom Sound & Sampler Recording
Record acoustic instruments or custom samples directly from microphone and map them into synths and drum tracks.
![Vocal Studio Sampler](screenshots/05_vocal_studio_sampler.png)

#### 6. 🎚 Master & Multitrack Effects Mixer
Full mixing console with individual channel strips, VU meters, VCA groups, Sub-Mix busses, parametric 3-band EQ, and reverb/delay sends.
![Mixer](screenshots/06_effects_mixer.png)

---

### 2. 🎛️ Synthesizers, AI Engines & Signal Processing

#### 7. 🤖 AI Music Assistant & Prompt Engine
Generate chord progressions, melodies, basslines, and song ideas via natural language text prompts (Suno AI, OpenAI, Claude, or local Ollama).
![AI Assistant](screenshots/07_ai_music_assistant.png)

#### 8. ✨ Sonix Alchemy Synthesizer
Advanced hybrid synth with 4 oscillators (Sine, Sawtooth, Square, Triangle), Moog 24dB ladder filter, ADSR envelope, and an 8-point real-time morph vector pad.
![Alchemy Synth](screenshots/08_alchemy_synth.png)

#### 9. 🥁 Dynamic Session Drummer
Interactive XY control pad for groove complexity and energy dynamics, with humanize engine, style variations, and fill-ins.
![Session Drummer](screenshots/09_session_drummer.png)

#### 10. 🧩 Modular Patcher & The Grid
Visual modular node environment for connecting audio signals, filters, envelopes, LFOs, and distortion with virtual patch cables.
![Modular Patcher](screenshots/10_modular_patcher.png)

#### 11. 🧠 AI Stem Separator (Demucs Neural Engine)
Source separation to isolate vocals, drums, bass, and instruments directly from mixed tracks.
![Stem Separator](screenshots/11_stem_separator.png)

#### 12. 🔌 Plugin & VST/CLAP Bridge Manager
Seamless management of FL Studio Native plugins, `.fst` presets, CLAP, VST3, LV2, and Wine/Yabridge sandboxed Windows VSTs.
![Plugin Manager](screenshots/12_plugin_manager.png)

#### 13. 🎛 Remix FX (Live Performance Pad)
Live DJ performance effects with Kaoss-style XY matrix, stutter repeater, vinyl tape stop, bitcrusher, and filter sweeps.
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
Configure audio interfaces, buffer sizes (1.4 ms low-latency), sample rates, and PipeWire/ALSA drivers.
![Audio Settings](screenshots/14_dialog_ljudinstallningar.png)

#### 22. 🤖 AI Configuration & API Keys
Setup credentials and connections for Suno AI, OpenAI GPT, Claude AI, and local Ollama servers.
![AI Settings](screenshots/15_dialog_ai_installningar.png)

#### 23. 💾 Project Manager & Templates
Create new projects from music templates (Synthwave, Trap, House, Ambient), save, and load projects.
![Project Manager](screenshots/16_dialog_projekthanterare.png)

#### 24. 💿 Render & Master Export Queue
Multithreaded master audio rendering to WAV, MP3, or FLAC with batch export support for individual stems.
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



