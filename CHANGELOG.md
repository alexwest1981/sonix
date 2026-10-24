# Changelog

All notable changes to Sonix Studio are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Warp mode per clip.** The mode used to be global for the project; each audio clip now has
  its own (`AudioRegion.warp_mode`, right-click → 🎚 Warp-läge), and split halves inherit it.
  The mode is part of the stretch cache key — otherwise switching modes would keep playing the
  old file — while the default (`Complex`) is left out so cached files stay valid. Older
  projects open with `Complex`, exactly as they sounded before.
- **Cue sends A/B.** Every track has CUE A/CUE B (post-fader), and the Control Room listens
  to MIX, A or B — the cue carries only the tracks sent to it and never reaches an export
  (a fresh engine always listens to Main). Measured: 400 Hz sent to A, 1 200 Hz not sent;
  the mix carries both (0.272/0.269) and cue A carries 400 Hz at 0.277 with 1 200 Hz at
  0.0018 (0.7 %, −43 dB). Sends are saved with the project.
- **One partial at a time.** In SPEKTRAL ENVELO, *Partial* selects 0 (the whole template, with
  tilt) or a single partial; A/D/S then writes an override on top of the template. Proven by
  `one_partial_can_have_its_own_contour`.

### Added

- **Comping per bar.** `CompSlice.bar` lets a comp apply to one bar (`None` keeps the old
  "every bar" behaviour), picked with *Takt* in the takes window; a bar-specific comp wins over
  a global one on the same step, and the export snapshot carries one take per bar.

### Fixed

- **Takes now play in bars 2–4 of a multi-bar block in exported audio.** The exporter passed the
  block's step index (0–63) to a take that is one bar long, so only the grid played there. The
  take is now read with `pib % 16` and its comp bar from `pib / 16`; proven by
  `a_take_note_plays_in_the_second_bar_of_a_block` (fails without the modulo).
- **Saving a project keeps the FX rack.** The master rack (pedal chain, Visual EQ, compressor
  release, convolver) was not part of the project file, so a reopened song came back with the
  stock rack. Pedals are stored by id and values — names and colours come from the catalogue —
  and an id the catalogue no longer has is skipped instead of dropping the chain. EQ points and
  the convolver are restored too, and the rack is pushed to the engine on load. Proven by
  `the_fx_rack_survives_a_save_and_load`.
- **Saving a project keeps the synth patch.** `SonixProjectData` had no voice at all: waveform,
  ADSR, filter, FM, sub, unison, glide, filter drive/key tracking, drawbars, partial envelopes,
  formant and time axis were lost on save, so a reopened project sounded like the default patch.
  They now live in a `synth` block (`SavedSynth`, `#[serde(default)]` so older files open as
  before) and are pushed to the engine on load. Proven by `the_synth_patch_survives_a_save_and_load`
  (the 64th drawbar, a per-partial override and the formant survive JSON).
- **High notes no longer alias.** The additive table was one unband-limited table for the
  whole keyboard, so a C7 with all 64 drawbars folded partial 20 down to 6.1 kHz. The table
  now holds one sum per distinct partial limit and the note picks its octave band; the voice
  normalizes over its own band, so a high note neither aliases nor drops 15 dB. Measured on
  C7: alias/fundamental 0.51 → 0.0045 (−41 dB), fundamental ×6.

### Added

- **Take-lane comping.** `Pattern.comp` picks which lane plays from a given step until the
  next comp, and `comped_take()` merges the notes; playback and export both use it. In the
  takes window, set the step and click ✂ on a lane. Coverage is implicit, so a comp placed
  inside another splits it without recomputing boundaries. Measured by
  `comping_plays_each_lane_in_its_own_steps`. Comping is per bar, because takes are one bar
  long.
- **Time axis for the additive oscillator** — the spectral formant glides from its current
  partial to a target partial over a set time, per voice, so notes that start later sweep from
  the beginning. Measured: two curves starting on partial 1 give the same harmonic centroid at
  note-on (4.61 / 4.63) and different centroids at 0.5 s (6.42 / 4.59). This is a linear
  formant glide, not an arbitrary curve over time.
- **Spectral filter for the additive oscillator** — a Gaussian formant over partial index
  rather than Hz, so it moves timbre without moving pitch. Measured: centroid 4.59 with the
  formant on partial 2 and 8.69 on partial 9, while the fundamental reads 110.0 Hz in both.
  Amount 0 leaves the sound untouched; the panel exposes formant, width and amount.
- **Per-partial envelopes for the additive oscillator** — each of the 64 drawbars can have
  its own attack, decay and sustain, so a note's spectrum can darken as it rings. Measured:
  harmonic centroid falls from 4.3 to 1.03 over 500 ms when partial 8 decays in 0.25 s while
  partial 1 sustains. Levels update every 32 samples; the worst case (16 voices × 64 drawn
  partials with individual contours) runs at 2.0× real time, against 240× without contours.
  The panel exposes a spectral envelope switch with attack, decay, sustain and a tilt that
  makes partial k decay k× faster; the command takes exact per-partial values.
- **64-partial additive synthesis** — the additive oscillator now has 64 drawbars instead of
  16, rendered from one shared table per harmonic setting. Measured in release with 16 voices:
  the oscillator runs at 241× real time and the full voice with filter and envelope at 30×,
  where a `sin()` per partial managed 3.4×. The table is rebuilt when a drawbar moves and is
  not band-limited, so high notes can fold partials above Nyquist.
- **Formant-preserving real-time pitch shift for stems** — the mixer's PITCH
  knob and the stem editor's "Tonhöjd" slider now transpose a stem ±12
  semitones in real time while keeping the formants in place, at 1-cent
  resolution. Holding Shift fine-tunes; double-click resets to 0. Offline
  export applies the same shift.
- **[PLUGINS.md](PLUGINS.md)** — a complete catalogue of every built-in Sonix
  effect, channel processor and instrument (FX Rack pedals, per-track strip,
  master bus utilities, Vocal Studio, synth/drum/patcher, plugin host), what
  each one does and its controls, linked from both READMEs.

### Fixed

- **Exports now include the whole synth patch.** `FxState` carried the oscillator waveform,
  filter and FX but not the voice: drawbars, per-partial envelopes, formant, time axis, sub,
  unison, glide, filter drive/key tracking and FM were silent in the file while audible live.
  They now live in `SynthVoiceState` inside `FxState` and `build_offline_engine` applies them.
  Render tests confirm the drawn partial and the sub oscillator reach the exported audio.

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
