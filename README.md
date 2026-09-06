# 🍊 SONIX STUDIO - Professional Native Linux DAW

> **Languages / Språk:** [🇬🇧 English](README.md) | [🇸🇪 Svenska](README_SV.md)

A modern, lightning-fast graphical Digital Audio Workstation (DAW) built with **Rust** and **egui**, featuring an integrated modular synthesizer, linear multitrack audio editor with Suno AI stem support, 16-step Channel Rack, Piano Roll, Vocal Studio, Multichannel Effects Mixer, and Touch Instruments for Linux (PipeWire / Wayland / ALSA / JACK).

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

To automatically generate all 22 full-screen screenshots:

```bash
cargo run --release -- --capture-screenshots screenshots
```

---

## 🎨 Core Features in Sonix Studio:

### 1. 🎛️ Metallic Top Toolbar & LCD Display:
* **Transport Controls:** Tactile controls for `▶ PLAY`, `⏸ PAUSE`, `⏹ STOP`, `⏺ REC`, and Song/Pattern Loop.
* **Digital LCD:** Displays real-time BPM, elapsed playback time, and precise bar position with centisecond precision (`BAR 01 : 03 : 12 (+00cs)`).
* **Live Oscilloscope:** High-resolution real-time waveform visualizer and spectrum display.
* **Tactile Rotary Knobs:** High-precision rotary knobs for Master VOL, Master PAN, and Stereo Width.

### 2. 🎼 Suno AI Stem Import & Linear Multitrack Audio Editing:
* **Direct Zip/Folder Import:** Drag and drop or browse Suno Stem archives/folders (`vocals`, `drums`, `bass`, `guitar`, `keys`, `back_vocals`, etc.).
* **Real-time Waveforms:** Continuous audio regions with pre-rendered and live RMS and peak waveforms.
* **Precision Slice Tool (✂):** Cut audio regions anywhere on the timeline with millimeter accuracy (0.01s centisecond snap).
* **Tool Palette:** Select / Pointer (⇱), Paint (✎), Slice (✂), Erase (🗑), Mute (🔇).
* **Multitrack Audio Streaming:** Real-time synchronized multitrack mixing directly from the audio engine with zero latency.

### 3. 🥁 Sonix Channel Rack (16-Step Sequencer):
* **6+ Dedicated Channels:**
  1. 💥 `808 Kick Drum`
  2. 🥁 `909 Snare Drum`
  3. ⚡ `Crisp Closed Hi-Hat`
  4. 🌊 `Open Hi-Hat`
  5. 🎹 `303 Acid Synth Lead`
  6. 🎸 `Sub Bassline`
* **4-Beat Step Buttons:** Tactile buttons grouped in fours with **glowing white center LEDs** when activated.
* **Mute [M] & Solo [S]** with status LED indicators.
* **Mini VOL & PAN Knobs** for each channel.
* **Sample Chopper & Pitch Shift:** Slice and retune samples directly in the channel strip.

### 4. 🎛️ Sonix Analog Synthesizer & Alchemy Synth:
* **Filter & Resonance:** Moog 24dB Ladder Lowpass filter with `CUTOFF` (Hz) and `RESO` (Q).
* **ADSR Envelope:** `ATTACK`, `DECAY`, `SUSTAIN`, `RELEASE`.
* **Live Envelope Graph:** Real-time visual envelope curve display.
* **Oscillators:** `∿ Sine`, `⩘ Sawtooth`, `⊓ Square`, `⋀ Triangle`.
* **Alchemy Vector Morph Pad:** 8-point real-time morph pad for smooth transitions between sound textures.

### 5. 🎹 Sonix Piano Roll & Touch Keyboard:
* **Interactive Note Editor:** Polyphonic piano roll with dynamic note lengths and velocity.
* **Touch Keyboard:** Playable virtual keyboard with neon visual feedback and computer keyboard typing support (A-K).
* **Scale Snapping & Chord Stamp:** Snap to Major, Minor, Pentatonic, or stamp chords with a single click.

### 6. 🎙 Vocal Studio & Intelligent Harmonizer:
* **Microphone Recording:** Record lead vocals and acoustic takes with Take Lanes and real-time zero-latency monitoring.
* **Pitch Detection & Autotune:** Real-time pitch correction and formant shifting.
* **4-Part Harmonizer:** Generate automated vocal harmonies and choir backing tracks.
* **Sampler Recorder:** Record custom acoustic instruments and samples directly via microphone.

### 7. 🎚 Master & Multitrack Effects Mixer:
* **Multichannel Mixer:** Channel strips with faders, peak VU meters, Solo, Mute, Pan, and Stereo Width.
* **Sub-Mix Busses & VCA:** Dedicated Drum Bus, Vocal Bus, Synth Bus, and 4x VCA faders.
* **Parametric 3-Band EQ:** Graphical EQ curve visualizer with adjustable frequency and gain per band.
* **Master Effects:** Space Reverb, Stereo Delay, Chorus, Compressor, and Tube Drive.

### 8. 🤖 AI Music Assistant, Patcher & Plugins:
* **AI Prompt Engine:** Generate melodies, basslines, chord progressions, and arrangement ideas via natural language prompts (Suno AI, OpenAI, Claude, Ollama).
* **Modular Patcher Grid:** Bitwig/The Grid modular environment with visual nodes and patch cables.
* **AI Stem Separator:** Demucs Neural Engine source separation to isolate vocals, drums, bass, and instruments from finished songs.
* **Plugin Manager & Wine:** CLAP, VST3, LV2 plugin host with isolated process sandboxing for Windows VSTs via Yabridge/Wine.
* **Remix FX:** Live DJ performance pad with Kaoss XY matrix, stutter repeater, vinyl tape stop, bitcrusher, and filter sweeps.

---

## ⌨️ Keyboard & Shortcut Reference

| Key / Shortcut | Function | Description |
| :--- | :--- | :--- |
| **Space** | **Play / Pause** | Toggle playback in Song or Pattern mode. |
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
Linear multitrack editor with Suno AI stem support, continuous waveforms, tool palette (Pointer, Paint, Slice ✂, Mute, Trash), snap-grid, and transport controls.
![Timeline & Arranger](screenshots/01_tidslinje_arranger.png)

#### 2. 🥁 16-Step Channel Rack & Drum Sequencer
Tactile step buttons with glowing white center LEDs, mute/solo status, dedicated rotary knobs for volume and pan per channel, and sample quick-select.
![Channel Rack](screenshots/02_channel_rack.png)

#### 3. 🎹 Piano Roll & Interactive Touch Keyboard
Polyphonic note editor with note lengths, velocity, scale snapping, and playable keyboard with neon visual feedback.
![Piano Roll](screenshots/03_piano_roll.png)

#### 4. 🎙 Vocal Studio & Microphone Recording (Take Lanes)
Professional recording console for vocals and acoustic instruments with real-time pitch detection, autotune, formant shifting, and 4-part harmonizer.
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
Visual modular node environment in Bitwig/FL Patcher style for connecting audio signals, filters, envelopes, LFOs, and distortion with virtual patch cables.
![Modular Patcher](screenshots/10_modular_patcher.png)

#### 11. 🧠 AI Stem Separator (Demucs Neural Engine)
Source separation to isolate vocals, drums, bass, and instruments directly from mixed tracks.
![Stem Separator](screenshots/11_stem_separator.png)

#### 12. 🔌 Plugin & VST/CLAP Bridge Manager
Seamless management of CLAP, VST3, and LV2 plugins for Linux with isolated crash-safe process sandboxing for Windows VSTs via Yabridge/Wine.
![Plugin Manager](screenshots/12_plugin_manager.png)

#### 13. 🎛 Remix FX (Live Performance Pad)
Live DJ performance effects with Kaoss-style XY matrix, stutter repeater, vinyl tape stop, bitcrusher, and filter sweeps.
![Remix FX](screenshots/13_remix_fx.png)

---

### 3. ⚙️ Dialogs, Modals & Configuration

#### 14. ⚙ Audio & Driver Settings
Configure audio interfaces, buffer sizes (1.4 ms low-latency), sample rates, and PipeWire/ALSA drivers.
![Audio Settings](screenshots/14_dialog_ljudinstallningar.png)

#### 15. 🤖 AI Configuration & API Keys
Setup credentials and connections for Suno AI, OpenAI GPT, Claude AI, and local Ollama servers.
![AI Settings](screenshots/15_dialog_ai_installningar.png)

#### 16. 💾 Project Manager & Templates
Create new projects from music templates (Synthwave, Trap, House, Ambient), save, and load projects.
![Project Manager](screenshots/16_dialog_projekthanterare.png)

#### 17. 💿 Render & Master Export Queue
Multithreaded master audio rendering to WAV, MP3, or FLAC with batch export support for individual stems.
![Render Queue](screenshots/17_dialog_render_queue.png)

#### 18. 📥 Suno AI Stem Importer
Automated loading and unzipping of Suno AI stems directly from zip archives or folders.
![Suno Import](screenshots/18_dialog_suno_import.png)

#### 19. 🎛 Hardware Controllers & MCU/OSC
Hardware controller support for Behringer X-Touch, Novation Launchpad, ALSA MIDI, and OSC network control.
![MIDI Controller](screenshots/19_dialog_midi_controller.png)

#### 20. 🔍 Focused Stem & Region Editor
Detailed audio region editor with volume envelopes, fade in/out curves, reverse, and sample slicing.
![Stem Focus Editor](screenshots/20_dialog_stem_focus_editor.png)

#### 21. 📖 Interactive Help Guide & Manual
Built-in comprehensive manual with shortcut lists, signal flow diagrams, and workflow guides.
![Help Guide](screenshots/21_dialog_hjalpguide_manual.png)

#### 22. ℹ About Sonix Studio
Version information, audio engine details, system architecture, and license.
![About Sonix](screenshots/22_dialog_om_sonix.png)



