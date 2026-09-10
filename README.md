# 🍊 SONIX STUDIO - Professional Native Linux DAW

> **Languages / Språk:** [🇬🇧 English](README.md) | [🇸🇪 Svenska](README_SV.md)

A modern, lightning-fast graphical Digital Audio Workstation (DAW) built with **Rust** and **egui**, featuring FL Studio-style direct timeline audio editing, magnetic loop snapping, FL Studio/VST3/CLAP plugin **scanning & cataloguing**, Soundtrap-style creative panels, an integrated modular synthesizer, 16-step Channel Rack, Piano Roll, Vocal Studio with live microphone recording, a multichannel Effects Mixer, and Touch Instruments for Linux. Audio runs through the system's realtime backend via **cpal/ALSA** (PipeWire works through its ALSA layer). See **[Features & Implementation Status](#-features--implementation-status)** for exactly what is real today and what is still to come. Full development plan: **[ROADMAP.md](ROADMAP.md)**.

---

## 🚀 Installation (Linux / Omarchy)

Sonix is a native Linux DAW written in **Rust** and **egui**. You run it by building from source with Cargo. The built-in mode that captures the 29 screenshots is only used by maintainers (see the note under the gallery below) and is **not** part of installing or running the app.

> ⚡ **Fastest way (guided installer):**
> Download and run `install.sh` – it confirms every step for you (dependencies, compilation, binary + start-menu entry) with no other terminal commands needed:
> ```bash
> bash <(curl -fsSL https://raw.githubusercontent.com/alexwest1981/sonix/master/install.sh)
> ```
> If you already have a clone, just run `bash install.sh` from inside the folder. After a code change you can refresh the binary + start-menu entry with `bash install.sh --refresh`.
>
> The installer asks which interface language you want (English, Svenska, Dansk, Norsk, Deutsch, Español, Français). You can always switch languages live from the **🌐 menu** in the app's top toolbar — the choice is remembered between sessions.

### Prerequisites

- 64-bit Linux with a running audio server — **PipeWire** (recommended), **JACK** or **ALSA**.
- **Rust** (stable, edition 2024) plus a C toolchain.
- A couple of runtime helpers used by specific features: `zenity` (file dialogs) and `unzip` (Suno ZIP import). Both optional.
- Optional, for Windows VSTs: **yabridge** + **Wine**.

Install the prerequisites:

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

### 1. Clone and build

```bash
git clone https://github.com/alexwest1981/sonix.git
cd sonix
cargo build --release
./target/release/sonix
```

During development you can skip the manual build step and use `cargo run --release` instead.

### 2. Install the binary (optional but recommended)

Installs `sonix` into `~/.cargo/bin` so you can launch it from anywhere or from an application menu:

```bash
cargo install --path .
sonix
```

### 3. Application menu entry (Omarchy / Linux desktop)

After `cargo install`, create a launcher (adjust the paths if your clone lives elsewhere):

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

Then log out/in once, or refresh the menu immediately with:

```bash
update-desktop-database ~/.local/share/applications
```

### Troubleshooting

- **No sound / no audio device:** make sure PipeWire (or JACK/ALSA) is running and your microphone/interface is connected. Open the audio settings in-app with **Ctrl + P** to pick a device and buffer size.
- **Window fails to open:** you need a working Wayland or X11 session with Mesa/OpenGL drivers.

---

## 🎨 Features & Implementation Status

**Legend:** ✅ Real & wired · 🟡 Partial / approximation · 🔜 Not yet implemented

> Everything marked ✅ is genuinely functional in the audio engine or project state — not a mock-up. The 🟡 items work but are deliberately labelled so nothing is over-sold.

### 1. 🎼 Timeline & Multitrack Arranger — ✅ Real
* **Direct Clip Edge Dragging (Paint & Select Tools):** Grab the right edge to extend and loop clips across bars, or drag left to shorten. Grab the left edge to trim/crop the start without destructive edits.
* **Continuous Waveform Phase Tracking:** Trimming the start shifts the waveform without squishing or distorting loop cycles.
* **Magnetic Loop-Snap (🧲):** Locks onto exact whole-loop multiples (`1x`, `2x`, `3x`, `4x`, `8x`) and grid divisions (`1/16`, `Beat`, `Bar`). Hold `Alt` for free precision.
* **Bottom Region Inspector:** Coarse/fine nudge (`±1 bar`, `±0.1s`), start-trim offset, gain, fade in/out, reverse playback, and loop multiplication.
* **Tool Palette:** Select (⇱), Paint (✎), Slice (✂), Erase (🗑), Mute (🔇); quick-cut at playhead (Ctrl+B).
* **Live In-Track Microphone Recording:** Arm buttons (`⏺`), input monitoring and real-time waveforms on audio tracks.

### 2. 🥁 Sonix Channel Rack (16-Step Sequencer) — ✅ Real
* **Channel Strips:** Kick, Snare, Closed/Open Hat, Synth Lead, Sub Bass and custom user channels.
* **4-Beat Step Buttons** with glowing center LEDs.
* **Per-Channel Knobs:** Volume, pan, pitch (semitones + cents) and a **sample chopper** (start/end fractions + transient detection).

### 3. 🎹 Piano Roll & Interactive Touch Keyboard — ✅ Real
* **Polyphonic Note Editor:** Note lengths, velocity editing, scale snapping and brush tools.
* **Playable Virtual Keyboard:** Neon feedback with computer-keyboard typing (A–K).

### 4. 🎚 Mixer Console & Per-Track Effects — ✅ Real
* **Channel Strips:** Faders, real peak VU meters, solo, mute, pan and stereo width.
* **Per-Track 3-Band Parametric EQ:** Draggable visual curve + quick presets.
* **Per-Track Dynamics & Sends:** Compressor, reverb/delay sends and pitch — all sent to the audio engine.
* **Master FX Rack:** Gate → 4-band EQ → compressor → de-esser → filter/drive → doubler → limiter, plus reverb & delay, with a **real gain-reduction meter** and a visual EQ editor.
* **Remix FX (live):** Kaoss-style XY pad with beat-repeat/stutter/reverse and a real tape-stop varispeed effect.
* 🟡 **Note:** VCA groups and sub-mix buses are **not** implemented (the old dead controls were removed).

### 5. 🎛 Analog Alchemy Synth — ✅ Real
* 4 waveforms (sine, saw, square, triangle), ADSR, resonant filter, drive and an 8-snapshot morph vector pad.
* 🟡 **Note:** The filter is a 2-pole (12 dB) state-variable low-pass — not a 24 dB Moog ladder — and filter/ADSR are global, not per-voice.

### 6. 🎙 Vocal Studio & Harmonizer — ✅ Real (offline processing)
* **Take Lanes:** Multiple takes with non-destructive comping; waveform crop/slice/normalize.
* **Pitch Editor:** Draggable note blobs (Melodyne-style) and scale-aware correction.
* **Autotune & 4-Part Harmonizer** with per-voice level/formant controls.
* **Custom Sampler:** Record acoustic one-shots via microphone and map them to instruments/drums.
* **Hardware Mic Panel:** Input device selector, hardware gain boost (+0 to +24 dB), noise gate, feedback suppression and vocal character presets.
* 🟡 **Note:** Tuning and harmony run **offline** on the recorded buffer (not in the realtime audio thread). Pitch detection is autocorrelation-based and "formant" is a spectral-tilt approximation.

### 7. 🎛 Creative Generators — ✅ Real
* **Smart Chord & Harmony Matrix:** 12 scales, Roman-numeral progressions, voicings, strum humanizer, arpeggiator → Piano Roll.
* **Beat & Melody Dice Generator:** 5 categories, genre presets, live preview → pattern.
* **Session Drummer:** XY complexity/energy pad with 6 genre presets.
* **Song Section Arranger:** Intro/Verse/Chorus/Bridge/Drop/Outro → timeline.
* **Add Track Studio:** 19 templates across 6 categories.
* **Hardware Strobe Tuner:** Real pitch detection, ±cents readout, 7 tuning presets and a reference tone.

### 8. 🧩 Modular Patcher — 🟡 Real DSP, order-sensitive
* Visual node grid with patch cables: MidiIn, Oscillator (sine/saw/square/triangle + sync/PWM), Filter (LP/HP/BP), ADSR Envelope, LFO, Delay, Reverb, Distortion, AudioOut.
* 🟡 **Note:** Nodes are evaluated in creation order (no topological sort), so cables must follow signal order. A few node labels describe more than the DSP does.

### 9. 🤖 AI Music Assistant & Generation — ✅ Real (with honest limits)
* **Real HTTP integration:** OpenAI, Anthropic, OpenRouter and local Ollama for text→patterns; audio generation via OpenAI TTS and Stability Stable Audio. Project context (key/BPM/selection) is injected into prompts.
* **Local fallback:** a deterministic, rule-based composer (scale/key aware) when no API is configured.
* **Suno/AI Stem Importer:** Unzip and decode real audio, detect BPM and map stems onto the timeline.
* 🟡 **Note:** There is **no** Suno API integration. "Suno" here means importing Suno-exported stem packs and using the local generator — not calling Suno.

### 10. 🧠 Stem Separator — 🟡 DSP approximation
* Splits a mix into vocals/drums/bass/instruments using spectral band-splitting, center-channel extraction and transient gating, plus a real onset-autocorrelation BPM estimator.
* 🟡 **Note:** This is **not** the Demucs neural network. It is a lightweight DSP separator; quality is well below a neural model.

### 11. 🔌 Plugin Manager — 🟡 Catalogue only (no host yet)
* Real recursive scanning of VST3/CLAP/LV2/VST2/`.fst` folders, ELF/PE binary verification, Wine & yabridge detection, and a one-click `yabridgectl sync`.
* 🔜 **Note:** Sonix does **not** yet load, run, or show plugin GUIs. See **[Plugin Support — Current Reality](#-plugin-support--current-reality)**.

### 12. 💿 Export & Project I/O — ✅ Real
* Offline render of the full project (real samples, timeline audio, FX) to **WAV** (16/24-bit & 32-bit float) and **FLAC** (in-app encoders); **MP3/OGG/AAC** via `ffmpeg` when installed.
* Clean metadata tagging (Sonix Studio only), master mix or per-track stems.
* Project save/load and template projects.

### 13. 🎛 Hardware Control — ✅ Real
* Real ALSA MIDI (MCU-style) input and a real UDP OSC server; live device list and bound port are shown in-app.

### 14. 🌐 Localisation & System — ✅ Real
* **7 interface languages** (English, Svenska, Dansk, Norsk, Deutsch, Español, Français), switchable live and remembered between sessions.
* **cpal/ALSA realtime engine** with a lock-free command ring and a crash-safe audio callback.
* **Audio Settings** (Ctrl+P) apply the chosen sample rate and buffer size by rebuilding the output stream live; the choice is persisted to `~/.config/sonix/audio.json` and restored on launch. The current host/device and the real active stream config are shown.

---

## 🧭 What's Left & How Far From "Real"

A candid status of the remaining gaps. The audio engine, timeline, mixer, generators and I/O are real; the items below are the honest exceptions.

| Area | Status | Remaining work |
| :--- | :--- | :--- |
| Audio engine (synth, drums, samples, FX, render) | ✅ Real | — |
| Timeline, Piano Roll, Channel Rack, Mixer | ✅ Real | — |
| Vocal Studio (record, comp, pitch edit, harmony) | ✅ Real | — |
| Generators (Chord, Dice, Drummer, Sections, Tuner) | ✅ Real | — |
| AI (LLM + audio APIs) | ✅ Real | Local fallback is rule-based, not a neural model |
| Hardware MIDI / OSC | ✅ Real | — |
| Export (WAV/FLAC in-app, MP3/OGG/AAC via ffmpeg) | ✅ Real | — |
| Modular Patcher | ✅ Real | Topological sort + labels match the DSP |
| Stem separation | 🟡 Partial | Integrate a real neural model (e.g. HTDemucs via ONNX) |
| Vocal tuning | 🟡 Partial | Realtime (audio-thread) autotune + true formant preservation |
| Time-stretch | 🟡 Partial | Pitch-preserving time-stretch (currently varispeed) |
| Audio Settings | ✅ Real | Live stream rebuild + persisted; shows real host/device/stream |
| Legacy "AI Settings" modal | ✅ Real | Now edits the same `AiConfig` and saves to disk |
| `.fst` "Apply Preset" button | 🔜 Missing | Button is a no-op; needs `.fst` decoding |
| Plugin hosting (VST3/CLAP/LV2/VST2) | 🔜 Missing | Full host required — see below |

**Overall:** roughly **85–90 %** of the features advertised in the UI are genuinely implemented and wired to the audio engine. The two largest outstanding pieces are **plugin hosting** (not started) and **neural stem separation** (currently a DSP approximation).

The full, prioritised development plan with check-off phases lives in **[ROADMAP.md](ROADMAP.md)**.

---

## 🔌 Plugin Support — Current Reality

> **Short answer: not yet.** Sonix can *find and catalogue* plugins, but it cannot *run* them.

**What works today (✅)**
* Recursive scanning of standard **VST3 / CLAP / LV2 / VST2** folders and FL Studio `.fst` locations (Linux + Wine paths).
* Binary verification (ELF/PE), file size and a "verified" badge.
* Wine and `yabridgectl` detection, plus a one-click `yabridgectl sync`.

**What is missing to actually host FL Studio / third-party plugins (🔜)**
1. A plugin host library / ABI — e.g. `clack`/CLAP, a VST3 binding, `lv2`, or `libloading` + VST2/3 entry points. **No loading code exists today.**
2. Instantiating plugins and routing audio + MIDI in and out of the realtime engine, with delay compensation.
3. Plugin state/preset save-load. (`.fst` is an FL Studio proprietary format and is not decoded.)
4. Embedded or floating plugin GUIs, and out-of-process sandboxing for crash isolation.
5. For FL Studio's own instruments (Sytrus, Harmor, Gross Beat, …) and FL Studio VSTi, the only viable route is their **VST/VST3 builds run through Wine + yabridge** — the native FL `.dll` formats are not a standard plugin API.

**Therefore:** FL Studio and other third-party plugins are **not usable in Sonix yet**. None of the "opens the plugin GUI" or "crash-safe sandbox" claims apply — the Plugin Manager is a catalogue and a Yabridge setup assistant, nothing more.

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
| **Ctrl + E** | **Export Master / Stems** | Offline-render the full project (real samples, timeline audio & FX) to WAV, FLAC, MP3, OGG or AAC with clean Sonix-only metadata. |
| **Ctrl + S** | **Save Project** | Save the current project to disk. |
| **Ctrl + O** | **Open Project** | Open a saved project from disk. |
| **Ctrl + P** | **Audio Settings** | Open PipeWire, ALSA, and buffer size configuration. |

---

## 📸 Screenshots & UI Gallery (1920 × 1200 Fullscreen)

> ℹ️ These previews are generated by maintainers with `cargo run --release -- --capture-screenshots screenshots` (opens a fullscreen instance and captures each view). Not needed for installing or running Sonix.

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
Professional recording console for vocals and acoustic instruments with pitch detection, offline autotune, formant tilt, and a 4-part harmonizer.
![Vocal Studio Take Lanes](screenshots/04_vocal_studio_leads.png)

#### 5. 🎤 Vocal Studio: Custom Sound & Sampler Recording
Record acoustic instruments or custom samples directly from microphone and map them into synths and drum tracks.
![Vocal Studio Sampler](screenshots/05_vocal_studio_sampler.png)

#### 6. 🎚 Master & Multitrack Effects Mixer
Full mixing console with individual channel strips, VU meters, per-track 3-band parametric EQ, and reverb/delay sends.
![Mixer](screenshots/06_effects_mixer.png)

---

### 2. 🎛️ Synthesizers, AI Engines & Signal Processing

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

#### 11. 🧠 Stem Separator (DSP Engine)
Source separation to isolate vocals, drums, bass, and instruments from mixed tracks (lightweight spectral DSP — not a neural model).
![Stem Separator](screenshots/11_stem_separator.png)

#### 12. 🔌 Plugin & VST/CLAP Bridge Manager
Cataloguing of FL Studio `.fst` presets and CLAP, VST3, LV2, and Wine/Yabridge plugin locations (scanning & verification — hosting not yet implemented).
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
Offline full-project rendering (real channel WAV samples + timeline audio stems + FX) to WAV (16/24-bit & float), FLAC (24-bit lossless), MP3 (320 kbps), OGG Vorbis and AAC/M4A. Choose master mix or per-track dry/wet stems, sample rate and clean metadata (title/artist/album/genre/year/comment) tagged only with **Sonix Studio** – never AI/provider info. WAV/FLAC are encoded in-app; MP3/OGG/AAC use `ffmpeg` when installed.
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



