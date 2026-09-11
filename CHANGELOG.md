# Changelog

All notable changes to Sonix Studio are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Formant-preserving real-time pitch shift for stems** — the mixer's PITCH
  knob and the stem editor's "Tonhöjd" slider now transpose a stem ±12
  semitones in real time while keeping the formants in place, at 1-cent
  resolution. Holding Shift fine-tunes; double-click resets to 0. Offline
  export applies the same shift.

### Changed

- **Stem pitch no longer changes timing** — the previous per-track pitch
  control was a varispeed resample that altered pitch *and* playback speed. It
  is replaced by a dedicated WSOLA + cepstral-envelope shifter whose latency is
  covered by the existing plug-in delay compensation, so the stem stays aligned
  with the rest of the project.

## [0.9.0] - 2026-09-11

First public release. Everything advertised in the UI is backed by the real
audio engine; the remaining gaps are documented honestly in
[README.md](README.md#-whats-left--how-far-from-real) and [ROADMAP.md](ROADMAP.md).

### Added

- **Audio engine** — real-time cpal/ALSA engine with a crash-safe callback;
  oscillator synth (Analog Alchemy), drum synthesis, per-voice filter and
  filter-envelope, master FX chain, delay/reverb, Remix FX (stutter/reverse),
  tape-stop, and modular patcher DSP with topological sorting.
- **Sequencer & arranger** — multitrack timeline with linear audio regions,
  slice tool, loop-dragging, 16-step Channel Rack, Piano Roll with MIDI
  keyboard recording, song-section arranger, and automation curves.
- **Mixer** — faders, real VU and gain-reduction meters, per-track 3-band
  EQ / compressor / sends / pitch, VCA groups and sub-mix buses.
- **Vocal Studio** — microphone recording, take lanes and comping, pitch
  editor, offline and real-time autotune, formant-preserving pitch shift,
  4-voice harmonizer, sampler, and WSOLA pitch-preserving time-stretch.
- **Generators** — chord matrix, beat/melody dice, Session Drummer, strobe
  tuner, and add-track templates.
- **AI** — local rule-based composer, external LLM support
  (OpenAI / Anthropic / OpenRouter / Ollama), audio generation APIs
  (OpenAI TTS / Stability), and project-context awareness.
- **Stem separation** — DSP separator plus optional neural HTDemucs via ONNX
  (`--features neural` with a user-supplied model), and Suno ZIP import.
- **Plugin hosting** (opt-in `--features plugin-host`) — in-process CLAP host,
  per-track audio processing with PDC, plugin state/preset save-load, CLAP GUI
  in its own X11 window, out-of-process sandbox with crash detection/restart
  and shared-memory audio transport, plus hand-rolled VST3 and VST2 ABI hosts
  with parameters, state and PDC.
- **Export & project I/O** — WAV/FLAC in-app, MP3/OGG/AAC via `ffmpeg`,
  offline full-project renderer with real samples and clean metadata, export
  presets, loudness normalization, and project save/load.
- **Hardware** — real ALSA MIDI (MCU) control and a UDP OSC server.
- **Localization** — 7 languages: Swedish, English, Danish, Norwegian, German,
  Spanish and French.
- **Documentation** — English and Swedish README, roadmap, tooling history and
  a full user manual.
- **Release infrastructure** — dual-licensed under MIT OR Apache-2.0, with
  GitHub Actions CI (default, `plugin-host` and `neural` builds).

### Fixed

- Tape-stop idle passthrough: no ring-buffer latency and no initial silence
  when the effect is disabled.
- Parallel-test race in the CLAP test fixture by giving each mock plugin
  instance its own state.
- Stale "not implemented" plugin/sandbox texts and the `.fst` preset
  rationale, which now state the real reason (the closed format cannot be
  decoded).

[Unreleased]: https://github.com/alexwest1981/sonix/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/alexwest1981/sonix/releases/tag/v0.9.0
