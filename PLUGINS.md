# 🔌 Sonix Built-in Plugins, Effects & Processors

> **Every processor on this page is 100 % native Rust and runs on the real-time audio thread — nothing here is a mock-up or a placeholder.** Each DSP block is allocation-free while processing and is driven by plain `Copy` parameter structs sent over the lock-free command ring buffer.
>
> This page covers Sonix's **own built-in** effects, channel processors and instruments. For hosting **third-party** CLAP / VST3 / VST2 plugins, see the [Plugin Support section of the README](README.md#-plugin-support--current-reality).

**Contents**
1. [FX Rack — the master effect chain](#1-fx-rack--the-master-effect-chain)
2. [Per-track channel strip (Mixer)](#2-per-track-channel-strip-mixer)
3. [Master bus utilities](#3-master-bus-utilities)
4. [Vocal Studio](#4-vocal-studio)
5. [Instruments & sound generators](#5-instruments--sound-generators)
6. [Third-party plugin host](#6-third-party-plugin-host)
7. [Master signal chain](#7-master-signal-chain)

---

## 1. FX Rack — the master effect chain

The **FX Rack** (Soundtrap-style pedalboard) is the editing surface for Sonix's master-bus DSP chain. It has three tabs — **Effect Chain**, **Visual EQ** and **Effects Library** — and ships with five presets: *Warm Studio Vocal*, *Punchy Modern Pop*, *Lo-Fi Nostalgia & Warmth*, *80s Gated Arena Reverb* and *Clean Master*.

Each pedal has an on/off switch, three macro knobs and a dry/wet **Mix**. Knobs are normalised `0.0–1.0` and mapped to real units inside `build_master_fx_params()`.

| Pedal | Category | What it does | Knobs |
|---|---|---|---|
| 📊 **Visual EQ** | Clean Up | 4-band parametric equaliser with an interactive curve and draggable band nodes (Low / Low-Mid / High-Mid / Air). Each band has frequency, gain and Q. | Low · Mid · High |
| 🗜 **Dynamic Compressor** | Glue Mix | Studio compressor with a live gain-reduction meter. Threshold `-60…0 dB`, ratio `1:1…12:1`, attack `1…100 ms`; release is set on the rack (`compressor_release_ms`). Mix acts as makeup gain. | Thresh · Ratio · Attack |
| 🪐 **Vocal Doubler** | Vocals | Creates a wide, doubled stereo image from a mono source using a short modulated delay (≈22 ms) with an LFO, then mid/side width widening. Great for thickening vocals and synths. | Strength · Mod · Width |
| 🌌 **Studio Reverb** | Echo & Delay | Room/plate-style algorithmic reverb (parallel comb filters + damping). Room size `0.2–0.95`, damping `0.05–0.9`, wet/dry mix. | Decay · Low Cut · High Cut |
| 🎯 **De-Esser** | Clean Up | Tames harsh *s* / *t* sibilance on vocals by dynamically reducing a high band (`4…12 kHz`), with adjustable amount. | Thresh · Freq · Reduct |
| 🚪 **Noise Gate** | Clean Up | Silences background noise and bleed when the singer is not performing. Threshold, attack (`1…50 ms`) and release (`20…500 ms`). | Thresh · Attack · Release |
| 🎛 **SVF Filter (12 dB)** | Sound Design | Resonant state-variable low-pass / high-pass filter with analogue-style saturation (`drive`). Cutoff is logarithmic `20 Hz…20 kHz`, resonance `Q 0.1…8`. | Cutoff · Reson · Drive |
| 🛡 **Master Limiter** | Loudness | Brick-wall limiter that prevents digital clipping and maximises loudness. Ceiling `0.1…1.0`, release `5…250 ms`, input boost `0.5…3.0×`. | Ceiling · Release · Boost |

> The **Effects Library** tab is a searchable/filterable browser over these same pedals, with categories *Clean Up, Glue Mix, Enhance, Sound Design, Drums, Bass, Distortion, Vocals, Loudness, Echo & Delay*.

---

## 2. Per-track channel strip (Mixer)

Every timeline track in the **Sonix Pro Multi-Track Mixer** has a full channel strip. All parameters are saved with the project and applied identically during live playback and offline export.

| Processor | What it does | Controls |
|---|---|---|
| **Fader & Pan** | Track level and stereo position. | Volume `0…1.25`, Pan `-1…+1` |
| **Mute / Solo** | Per-track mute and solo (solo is global). | toggles |
| **3-band EQ** | Low shelf, mid peaking (with Q) and high shelf, in series per channel. | Low freq/gain, Mid freq/gain/Q, High freq/gain |
| **Compressor** | Per-track dynamics control. | Threshold `-60…0 dB`, Ratio |
| **Reverb send** | Feeds the track into the master reverb bus. | Send `0…1` |
| **Delay send** | Feeds the track into the master ping-pong delay. | Send `0…1` |
| **Pitch shifter** | Real-time, **formant-preserving** pitch shift of the track (`±24 semitones`) using a stereo WSOLA engine with cepstral spectral-envelope correction. | `-24…+24 st` (knob; Shift = fine, double-click = reset) |
| **Plugin insert** | Optional CLAP / VST3 / VST2 insert on the track (see §6). | slot |

---

## 3. Master bus utilities

Beyond the FX Rack pedals, the master bus exposes these processors directly from the mixer:

| Processor | What it does | Controls |
|---|---|---|
| **Master Reverb** | Algorithmic reverb on the instrument bus (also driven by the FX Rack *Studio Reverb* pedal). | Room size · Damping · Mix |
| **Master Delay** | Stereo **ping-pong** delay with feedback. | Time `20…1000 ms`, Feedback `0…0.9`, Mix |
| **Master Filter** | Resonant SVF filter with drive (mirrors the FX Rack filter pedal). | Cutoff · Resonance · Drive |
| **Stompbox** | Master drive/tone saturation for a warm "pedal" colour. | Drive · Tone |
| **Stereo Width** | Mid/side stereo widener on the master. | `0…2` |
| **Remix FX** | DJ-style beat-repeat / stutter / **reverse** stutter locked to project tempo. Captures one subdivision of the master bus and loops it. | Off · 1/4 · 1/8 · 1/16 · Reverse |
| **Tape Stop** | Varispeed tape-stop / spin-up effect on the final master. | toggle |
| **Master Volume** | Final output level with soft `tanh` saturation. | `0…1.25` |

---

## 4. Vocal Studio

The **Vocal Studio (F8)** contains Sonix's pitch, timing and harmony processors. These operate on recorded takes and/or the live microphone.

| Processor | What it does |
|---|---|
| **Realtime Auto-Tune** | Corrects the live microphone to the selected scale in real time; the **Auto-Tune** knob sets strength from *Natural* to *hard T-Pain robot*. |
| **Pitch Editor (note blocks)** | Analyses a take, draws real pitch blobs, and lets you drag note blocks to snap them to a target scale. *Analyse Pitch* → drag → *Apply Note Correction*. |
| **4-Voice Harmonizer** | Generates up to four harmony voices (Lead, +3 st, −5 st, +12 st by default) from one take, each with its own interval, volume, on/off and formant shift. Rendered as new take lanes. |
| **Formant-preserving pitch shift** | Shifts pitch while keeping the vocal formants natural (avoids the "chipmunk" effect). Used by the harmonizer and the per-track shifter. |
| **WSOLA time-stretch** | High-quality time-stretch / playback-speed change (`0.25×…3.0×`) that preserves pitch. |
| **Take tools** | Per-take volume, pitch (semitones + cents), time-stretch, reverse, and a one-click *Reset effects*. |
| **Sampler / custom sounds** | Records and maps your own sounds and samples as playable instruments. |

---

## 5. Instruments & sound generators

| Instrument | What it does | Controls |
|---|---|---|
| **Analog Alchemy synth** | Integrated subtractive synthesizer. | Waveform (Sine / Saw / Square / Triangle), SVF filter (cutoff, resonance), filter envelope amount, amp ADSR (attack/decay/sustain/release) |
| **Channel Rack (16-step)** | FL Studio-style step sequencer for drums, basslines and synth patterns. | per-step triggers, velocity, swing |
| **Drum Machine** | 10 synthesized drum voices, no samples required: Kick, Snare, Clap, Closed Hat, Open Hat, Crash, Low Tom, High Tom, Metronome High, Metronome Low. | per-voice level/tune, pattern steps |
| **Modular Patcher** | Node-based modular graph processed on the audio thread. Node types: MIDI/Note IN, Oscillator, SVF Filter, ADSR Envelope, LFO Modulator, Delay, Space Reverb, Tube Saturation, Master Audio OUT. | per-node knobs + patch cables; cycle detection |

---

## 6. Third-party plugin host

Separate from the built-ins above, Sonix can host external plugins:

- **CLAP** — native loading, parameter inspection, per-track audio processing, PDC, state save/load, preset loading, and the plugin's own GUI in its own X11 window.
- **VST3 / VST2** — loading, parameter inspection, per-track audio processing, PDC and state save/load.
- **Out-of-process sandbox** — plugins can run in a separate process with crash restart and shared-memory audio transport.
- **Plugin Manager** — scans and catalogues all formats.

Requires building with `--features plugin-host`. LV2, MIDI instrument routing and a verified Wine/yabridge bridge are still outstanding — see the [README](README.md#-plugin-support--current-reality) and [ROADMAP.md](ROADMAP.md).

---

## 7. Master signal chain

For reference, the engine's master bus runs in this order every sample:

1. Instrument / sample / drum submix + per-track channel strips (EQ → compressor → pitch → sends) + plugin inserts
2. Master **Reverb**
3. Master **Delay** (ping-pong)
4. Stem audio (with per-track PDC)
5. **Master FX chain**: Noise Gate → 4-band EQ → Compressor → De-Esser → SVF Filter → Doubler → Limiter
6. Master volume (soft `tanh`)
7. **Remix FX** (beat-repeat / reverse)
8. **Tape Stop**

---

*All built-in processors are covered by unit tests in `src/audio/` (`master_fx.rs`, `effects.rs`, `filter.rs`, `envelope.rs`, `drum.rs`, `patcher.rs`, `vocal_harmonizer.rs`).*
