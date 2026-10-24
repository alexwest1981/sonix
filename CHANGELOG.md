# Changelog

All notable changes to Sonix Studio are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed — the VERKTYG tab led back to the arranger

- `nav_group_of` skickade allt som inte var arranger eller mixer till SKAPA, så **fliken Verktyg
  hade noll vyer**. Ett klick satte `view_mode = last_view[3]`, som var `PlaylistArranger` — alltså
  tillbaka till arrangören, varje gång.
- **Verktygsvyerna har nu en egen grupp:** Patcher, Stems och Plugins. `last_view[3]` startar på
  Stems, och flikklicket kontrollerar att det sparade målet hör till fliken — annars tar den
  flikens första vy. Verifierat i bild: VERKTYG aktiv med Patcher/Stems/Plugins och
  Tärning/Ackord/FX-Rack.
- Provet räknar nu `[6, 2, 2, 3]` och kräver att **varje** flik har minst en vy.

### Added — project scripts (second execution surface)

- `audio::project_script` med `Ctx { bpm, pattern_count, channels }`: `bpm()`, `set_bpm()`,
  `steps()`, `channel_count()`, `channel_name()`, `step()`, `set_step()`, `clear_channel()`,
  `pattern_count()`. Skriptet arbetar på en **kopia** som bara skrivs tillbaka om det gick
  igenom, precis som tagning-skriptet.
- Pianorullens skriptruta har en väljare **🎵 Tagning / 🎛 Projekt**; projektläget bygger `Ctx`
  ur appens tempo och kanalsteg och lägger resultatet i statusraden.
- Tre prov: tempo + steg, ett trasigt skript lämnar projektet orört, och steg utanför rutnätet
  ignoreras.

### Added — note bends have an editor

- **BEND-raden i pianorullen skriver bend per ton när exakt en ton är markerad** (en rad i
  rutnätet = MIDI 48 + rad). Markeras flera rader finns ingen entydig ton, och då gäller
  kanalbenden som förut. Notbenden ritas i gult, kanalbenden i kanalfärg.
  Dubbelklick lägger en punkt, högerklick tar bort närmaste, drag flyttar den — samma grepp som
  kanalbenden, så ingen ny vana behövs.
- Prov `a_single_selected_row_names_the_note_to_bend`: ingen markering → kanalbend, rad 12 → MIDI
  60, två rader → kanalbend igen.

### Added — DeepSeek as a chat provider

- **`AiProvider::DeepSeek`** (`https://api.deepseek.com/v1`, `deepseek-flash` som förval,
  `deepseek-v4-pro` i listan). Talar OpenAI-formatet, så den går genom samma väg som OpenAI —
  bara namnet, bas-URL:en, modellistan och `from_code("deepseek")` är nya. `providers`-listan i
  båda inställningspanelerna byggs ur `AiProvider::ALL`, så valet dyker upp utan mer kod.
- Prov `provider_codes_and_models_are_consistent` täcker koden; `live_ai_generation_smoke`
  (`--ignored`, kräver nät och en ifylld `ai.json`) kör en riktig generering mot vald provider.

### Added — MIDI in scripts, and two inventory rows promoted

- Skript-API:t kan nu läsa och skriva MIDI: `load_midi(path)` fyller tagningen från en `.mid`
  (steg = PPQ/4 ticks, noter utanför takten viks in med `rem_euclid`) och `save_midi(path, bpm)`
  skriver den. Ingen ny parser — `smf` fanns; ytan är ny.
  Prov `a_script_can_save_and_load_midi`: spara två noter, läs tillbaka dem på steg 0 och 4.
- **Inventariet:** Sytrus och Toxic Biohazard flyttade från ◐ till ✅ — luckorna som stod i
  deras rader (ringmodulering/additivt i matrisen, fyra filtertyper, LFO:er, unison) är byggda
  sedan 0.9.69–0.9.107. Räknaren är nu **✅ 72 / ◐ 62** av 141.

### Added — bend per note (MPE) and talkback to a cue

- **`NoteBendPoint { pos, key, semitones }`** in the pattern: the same curve as `bends`, but keyed
  on the note number, so two notes on one channel can bend differently. `AudioCommand::NoteBend`
  / `NoteBendDelayed` reach the engine, and both live playback (`trigger_step`) and the exported
  file schedule them — the channel bend path is untouched.
  Prov `a_note_bend_reaches_only_its_note_on_a_shared_channel` (note 67 up an octave, note 60
  still at 1,0) and `the_export_schedules_the_steps_note_bend` (no channel bend is created).
- **Talkback routing:** `SetTalkbackCue(0|1|2)` = MIX / cue A / cue B, three buttons next to
  🎙 TALKBACK. With A selected the talk goes into the singer's cue and never into the main
  bus — which means never into a recording. Prov `talkback_can_be_routed_to_a_cue`:
  cue A 0,25 RMS, MIX below 0,01.

### Fixed — the take lanes were invisible in the demo

- Lane-vyn (0.9.46) ritar varje tagning som ett band i patternblocket och låter ett klick välja
  lane för den takten — men **demon hade bara en tagning**, så banden ritades aldrig och
  funktionen gick inte att se eller prova. Demon har nu tre tagningar på de tre första
  patternen, och banden syns (verifierat i bild: tre band, det spelade ljusast).
- Demons pianorulle är ett rutnät, inte tagningar, så `take` var tom; frasen fylls därför med
  fyra noter innan lanes skapas.

### Performance — a typical patch measured too

- The measurement now prints four cases. **Typical patch (unison 1): 5,7× realtid.** The ceiling
  cases are unchanged (2,1× / 1,9× / 1,5×).

### Cleanup — clippy 37 → 25

- Real fixes, not silences: `?` instead of `let…else`+return, `if let` instead of a one-arm
  `match`, `sort_by_key`, `&mut [T]` instead of `&mut Vec<T>`, a counter loop, a merged
  identical `if` branch, and `SharedResult<T>` as one alias for the three
  `Arc<Mutex<Option<Result<…, String>>>>` fields.
- `to_release` → `into_release` (the method takes `&mut self` and consumes the voice's state).
- Kvar är 25, nästan alla `too_many_arguments` och `needless_range_loop` i render- och DSP-loopar
  där en struct per sampel eller per bildruta vore dyrare än argumentlistan.

### Performance — dead operators cost nothing (measured)

- **Mätt först:** worst case (16 röster, 6 operatorer, 4 unisonstämmor, filter på alla) gick i
  **0,9× realtid** med 24 dB — alltså långsammare än ljudet. Med kurvan på: 0,8×.
- **Åtgärd:** en operator vars nivå är 0 och som inte används som källa i FM-/ring-/add-matrisen
  räknas inte alls (ingen fas, inget filter, ingen envelop). Standardpatchen har fem sådana.
- **Efter:** **1,9× realtid** (24 dB), **1,5×** med filterkurva, **2,1×** vid 12 dB — ungefär en
  fördubbling. Mätningen ligger kvar som `measure_six_op_worst_case` (ignorerad, körs för hand).
- Kvar: 16 röster × 4 stämmor × 6 aktiva operatorer är fortfarande det dyraste läget; en
  realistisk patch (unison 1) ligger på flera gånger realtid.

### Added — filter curve (Sytrus)

- `curve_gain[4]` at 200 / 800 / 3 000 / 8 000 Hz: four band-passes added on top of the dry
  voice signal, so the curve is drawn with four points. All 1,0 = neutral, and then no filters
  run at all — a patch without a curve costs nothing and sounds exactly as before.
- Applied to the **voice sum**, not per operator: four bands per operator per unison voice would
  be 96 filters per sample; this is 8 per voice.
  Prov `the_curve_boosts_its_band`: the 800 Hz band at 4,0 raises 800 Hz more than 1,5× relative
  to the fundamental.
- **FILTERKURVA** row above LP/BP/HP/NO; the frequency sits in the tooltip because four labels
  burst the panel width.

### Added — free time curve per operator (Sytrus)

- `shape[[f32; 8]; 6]` + `shape_secs[6]`: eight levels spread over a chosen time, linearly
  between them; the last point holds. `shape_secs = 0` falls back to the ADSR, so old patches
  are untouched. The curve feeds `env_now`, which means it drives level, PIT, F-ENV and CRV at
  once — one curve, all destinations.
  Prov `a_free_shape_replaces_the_adsr`: 1,0 → 0 over 0,25 s gives sound in the attack
  (>0.05 RMS) and silence after a tenth of a second (<0.001).
- **KURVA** row in the 6-op panel: seconds field plus eight vertical sliders.

### Added — filter slope per operator (Sytrus)

- `filter_slope: [u8; 6]`, 1 = 12 dB/octave (default, old patches unchanged) or 2 = 24 dB:
  the same filter a second time with its own state (`filters2`).
  Prov `a_steeper_filter_takes_more_above_the_cutoff`: one octave above the cutoff the 24 dB
  filter passes less than half of what the 12 dB one does.
- **12 / 24** buttons next to LP/BP/HP/NO in the 6-op panel.

### Changed — the synth bus is stereo, and operators can be panned

- **Bug fixed:** the engine collapsed every sample voice to mono (`(s_l + s_r) * 0.5`) before
  the FX chain, so a stereo sample and a channel's pan were thrown away. Left and right now go
  through their own filter state (`filter_state_r`) into separate buses.
- The reverb runs as two instances (`reverb`, `reverb_r`) instead of one mono feed, so a stereo
  source gets a stereo tail. For mono material both see the same input and the result is
  unchanged — 979 tests say so.
- `SixOpParams.pan: [f32; 6]` with a linear law (centre = 1,1, so old patches are untouched) and
  `next_sample_stereo` through `SixOpVoice` → `Voice` (own `filter_r`) → master.
  Prov `an_operator_pan_reaches_the_master` (left at −1 → right below 2 % of left) and
  `a_stereo_sample_keeps_its_channels` (right stays silent when the sample's right is silent).
- **PAN** slider per operator in the 6-op panel. Sytrus-raden är nu komplett sånär som på egna
  filterkurvor och en fri tidskurva.

### Fixed — the channel rack rows are a table

- The channel-name button grew with the name, so **Byt/Chop and the step grid started at a
  different x in every row** — the rack looked out of sync. The name field is now a fixed
  200 px cell with `TextWrapMode::Truncate`; long names show "…" and every column lines up.

### Added — envelope curve per operator (Sytrus)

- `env_curve: [f32; 6]` (−2…2, 0 = linear): same ADSR, different shape — Sytrus' envelope
  tension. Applied to the value, so the times are untouched.
  Prov `an_envelope_curve_changes_the_attack_shape`: at +2 the first 40 ms have >1.5× the RMS,
  at −2 less than 0.7×.
- **CRV** slider with KEY and F-ENV in the 6-op panel.

### Added — key tracking per operator (Sytrus)

- `key_track: [f32; 6]` (0…1) moves the operator's filter with the played pitch, so a high note
  keeps the timbre of a low one. Prov `key_tracking_keeps_the_timbre_across_octaves`: the
  brightness gap between 220 and 880 Hz drops by more than half at 1.0.
- **KEY** slider next to F-ENV in the 6-op panel.

### Added — the 6-op panel is in the screenshot set

- `ScreenshotTarget::SixOpPanel` → `31_syntes_6op.png`; the panel was never captured before, so
  changes to it could not be reviewed. Verified: PIT, F-ENV and LFO 2 rows render without clipping.

### Added — filter envelope and a second LFO per operator (Sytrus)

- `filter_env: [f32; 6]` (−4…+4 octaves): each operator's own ADSR opens its filter.
  Prov `a_filter_envelope_opens_the_filter`: at 660 Hz the open filter passes **>4×** what the
  closed one does, on an FM-brightened operator (a plain sine has no harmonics to filter).
- **LFO 2** (`lfo2_rate/depth/target`, own phase): pitch and cutoff can move at different speeds.
  Prov `a_second_lfo_can_move_the_filter_independently`.
- Both have UI in the 6-op panel: **F-ENV** next to **PIT** for the selected operator, and an
  **LFO 2** row under the first.
- `paint_knob` extracted in `widgets.rs`, so the channel rack and the mixer draw their knobs with
  the same code — the duplicate-control item from the GUI plan.

### Added — the API key is under Settings

- **Inställningar → 🤖 AI & API-nyckel…** opens the same window as AI & Providers: provider, base
  URL, key (masked), model, timeout — saved to `~/.config/sonix/ai.json`. Two ways in, one modal.

### Added — pitch envelope per operator (Sytrus)

- `SixOpParams.pitch_amount: [f32; 6]` (−24…+24 semitones, serde default 0) bends each operator's
  pitch with **its own ADSR** — the same envelope that shapes amplitude, which is what Sytrus does
  with a separate curve; documented as the simplification it is.
- A **PIT** slider sits with A/D/S/R for the selected operator. Old patches are untouched (0).
- Prov `a_pitch_envelope_moves_the_operator_an_octave`: +12 moves the fundamental 440 → 880 and
  leaves 440 below 0.05 of the peak — measured in the spectrum, not by ear.
- Clippy 48 → 37 on the way (`StereoPcm` alias for the repeated three-tuple, `drum_layers` typed).

### Changed — less chrome in the arranger

- The floating **BETA AI prompt capsule** is gone from the timeline; the AI view is where prompts
  belong. Its one unique action, **🎧 Generera ljud** (Suno/TTS), moved into that view — removing
  the capsule would otherwise have left the whole audio-generation path unreachable, which the
  compiler pointed out as seven dead functions.

### Changed — a new project starts ready to record

- `new_empty_project` (Ctrl+N) now arms the mic track, selects it, turns the metronome on and
  lands in the arranger instead of an empty channel rack. The big REC is one click away, which was
  the point of the Soundtrap comparison.

### Added — the assistant can change the synth

- `set_channel_instrument` now runs: `find_preset` matches a name case-insensitively in both
  directions against `SYNTH_PRESETS` (one shared table — the browser and the assistant read the
  same list), then applies it through the same path as the browser's **Ladda**.
- An unknown name is refused with the list of what exists, rather than silently doing nothing.
  Prov: `presets_are_found_by_partial_name` ("acid" → 303, "KEPLER EXO" → Kepler Exo, "theremin" → none).

### Added — sections in the arrangement

- `Section { name, start_bar, bars }` on the app, drawn in the timeline ruler as purple bands with
  the name and a line down through the arrangement.
- The assistant's `plan_sections` now has a **✅ Skapa sektionerna** button next to the prompt; it
  appears only while a plan is pending and writes nothing until clicked.
  `sections_from_plan` (pure) puts them back to back from bar 1 — prov: Intro 0/4, Vers 4/8,
  Refräng 12/8.

### Added — a localhost API for the AI service

- **`src/ai_service.rs`**: HTTP on `127.0.0.1:4827` (`SONIX_AI_API_PORT`, `0` = off) with
  `GET /v1/health`, `GET /v1/tools` (the same schema the model gets), `POST /v1/execute`
  (validated `{"calls":[…]}`) and `GET /v1/last` (receipts, cleared on read).
- **Three locks, none optional:** loopback only, `Authorization: Bearer` with a token written to
  `~/.config/sonix/ai_service_token` (0600), and the same validation as the assistant uses.
  Executed calls run on the UI thread, like everything else that touches patterns.
- Prov: `the_service_requires_the_token_and_serves_the_schema` (401 without token, 200 with) and
  `execute_queues_valid_calls_and_rejects_broken_json` (queue, 400 on bad JSON, receipts).
- **Verified end to end:** `POST /v1/execute {"set_tempo":132}` → `GET /v1/last` →
  `["tempo 132 BPM"]`, and a drum write → `["trumkomp 'house' i 2 takter"]`.
- Docs: `docs/AI-API.md`.

### Added — the assistant gets instructions and hands

- **`src/audio/ai_tools.rs`**: a system prompt that teaches the model Sonix's own vocabulary
  (channel, pattern, section, track) plus six rules — propose sections before writing, never
  overwrite without saying so, never touch the master — and a **tool schema** for `set_tempo`,
  `set_key`, `write_drum_pattern`, `plan_sections`, `mute_track`, `set_channel_instrument`.
  Responses in three shapes (`{"calls":…}`, a bare array, OpenAI `tool_calls`) parse to the same
  call, and prose without calls is an error instead of a silent no-op.
- **Drum patterns** are data (`DRUM_STYLES`: house, hiphop, rock), split into kick/snare/hat layers
  and written to the first three channels with General MIDI notes (36/38/42). **Guard:**
  `ensure_channels_free` refuses the whole write if any of the three channels already has steps —
  no half-written groove.
- **`src/audio/ai_client.rs: chat_with_tools`** sends prompt + schema to an OpenAI-compatible or
  Anthropic endpoint and returns either text or tool calls.
- **The AI view has 🤖 Kör assistenten**: the request runs in a background thread, the reply or the
  receipt ("tempo 128 BPM · trumkomp 'house' i 2 takter") is shown under the prompt. A section
  plan is parsed and stored for confirmation; it does not write anything yet.

### Added — downloadable themes from a GitHub registry

- **`github.com/alexwest1981/sonix-themes`** holds `index.json` (name, file, sha256) and the theme
  files. **Inställningar → 🎨 Tema → ⬇ Hämta teman…** fetches the index in a background thread —
  the frame never waits on the network — and lists what the registry offers; a click downloads,
  **verifies sha256** with `ring` and only then writes the file into `themes/`. A bad hash is
  refused with the reason.
- Three themes ship in the registry: **Nord** (cool dark), **Kol** (near-black dark) and **Papper**
  (warm light).
- Prov (ignored, network): `downloads_a_theme_from_the_registry` fetches the index, installs the
  first theme into a temp config dir and asserts a wrong hash is rejected.

### Added — theme engine

- **`~/.config/sonix/themes/*.json`** is a theme library: **Inställningar → 🎨 Tema** lists every
  file there and a click applies it. **📂 Öppna temamappen** opens the folder in the file manager,
  **💾 Exportera tema…** writes the current palette with **only the keys that differ** from its
  mode's defaults (plus `mode`), so a theme is a small file you can read.
- Prov `an_exported_theme_applies_again`: a theme with one changed colour exports as one key,
  applies back, and the palette takes it — the defaults are not written out.

### Added — light and dark mode

- `Palette` carries a `mode` (`dark`/`light`). `theme.json` may say `{"mode": "light"}` and any
  colours it names override the light defaults; **Inställningar → 🎨 Tema** switches, writing only
  the `mode` key so the user's own colours survive the switch.
- **Contrast is measured, not assumed:** `light_mode_keeps_text_readable` computes WCAG ratios for
  body text (≥ 4.5:1) and muted text (≥ 3:1) against panel, background and panel_alt in both modes.
- **`readable_on(colour)`** picks black or white for clip labels and the export buttons, so a
  saturated clip keeps a readable label in either mode.

### Changed — the mic track is an ordinary track now

- **Same height as the rest.** The mic track was drawn at double height with a white card and a
  white lane — a bright block in the middle of a dark arrangement. It now uses the normal row
  height, the normal panel background and the normal grid; only its cyan accent marks it.
- **Glare lowered on selection.** Selected regions filled at alpha 190 with a white 2 px outline
  and a white waveform; now 150, a cyan 1.6 px outline and a darker tint of the track colour in
  the waveform. Selection is still unmistakable, and nothing in the arrangement reads as a light
  panel any more.

### Changed — rounder clips

- Clip labels 10.5 → 13.5 px, the range Soundtrap uses on its regions; long names still fit inside
  the clip and truncate at its edge.

- Region corner rounding 4 → 8 px, matching the track headers and moving toward the 10–12 px
  Soundtrap uses on its regions.

### Changed — taller track rows

- Track row 46 → 64 px and the header's control block pushed down 8 px (30 on the mic track), so
  the name keeps the top line and the controls get their own line under it — the shape Soundtrap's
  headers have. Clips in the timeline grow with the row; no other view is touched.

### Changed — track headers follow the Soundtrap blueprint

- Corner rounding on the track-header cards 3 → 8 px and the track name 11 → 13.5 px (14 on the
  mic track), from measurements taken on Soundtrap's own studio image — see
  `docs/SOUNDTRAP-BLUEPRINT.md`. Clip height and row height are unchanged: raising them means
  moving the header's control rows, which is its own change.

### Changed — language and guide moved into the menus

- The language picker and the guide button were two loose controls at the end of the transport
  row. Language is now a submenu under **Inställningar** (`Språk (Svenska)`) and the guide button
  is gone — **Hjälp → Snabbguide & Manual (F1)** already did the same thing.

- **Transport is three large icon buttons.** ▶ ⏸ ⏹ at 48×30 with the word in the tooltip, in the
  Ableton manner, instead of three wide text buttons. Icon colour flips to black on a lit
  (green/orange) button so it never goes grey-on-bright.

Everything below was egui's default values; Sonix had never set a style at all.

- **Global form** (`theme::apply_style`): 8×5 px item spacing (was 8×3), 21 px row height
  (was 18), 9×4 button padding (was 4×2), 4 px corner rounding on widgets and 8 px on windows,
  panel/window fills taken from the palette, orange selection with black text.
- **Type scale:** XS 9 → 10.5, S 11 → 12.5, M 13 → 14.5, L 16 → 18, XL 22 → 24, and the same
  scale for egui's own `text_styles`, so widgets without an explicit size grow with the rest.
  Checked in screenshots: no clipping in the arranger, rack, piano roll or synth panel. Mixer
  channel names truncate with an ellipsis — they did before the change too.

- **Tempo and position sit at the timeline now.** The arranger toolbar has an editable BPM field
  and the bar.beat.step position (`1.1.1`) next to the time signature, metronome and pedal — the
  two numbers you look at while arranging, without lifting your eyes to the transport row. Same
  `bpm` field, so the two views can never disagree.

- **Double-click a channel's VOL or PAN knob to reset it.** The knob already knew how to snap back
  when it was given a default — `rotary_knob` just never passed one. `rotary_knob_reset` takes the
  reset value explicitly, so the mixer resets to 80 % and centre instead of guessing from the
  range. Other knobs are untouched until their defaults are decided.

- **Right-click a channel in the rack.** Mute/unmute or reset volume and pan (80 %, centred), with
  the channel name in the status line. The action is applied after the row loop, so it does not
  fight the channel borrow mid-draw.

- **Unison in the 6-op synth.** 1–4 voices per note, spread by 0–50 cents, summed with 1/√n so the
  level stays put. Each voice keeps its own phases, feedback history and filter state; the six
  envelopes tick once per sample and are shared, or they would run n times too fast. Default is
  one voice — every earlier patch renders exactly as before, which the existing tests confirm.
  Measured: RMS 0.7071 mono against 0.7018 with four voices at 20 cents.

- **An LFO in the 6-op synth.** Rate 0.05–40 Hz and depth 0–1, aimed at pitch (vibrato, about
  ±1 semitone at full depth) or cutoff (±2 octaves). Depth is 0 by default, so nothing moves until
  the slider does and older patches are untouched. Measured on a 440 Hz tone at 10 Hz: the carrier
  drops 1.0000 → 0.1543 and a sideband appears at 0.1949.

- **Four filter types per operator in the 6-op synth.** LP, BP, HP and notch, all from the same
  state-variable calculation (the highpass is input minus band and lowpass, the notch is input
  minus band), so a filter costs what it did before. LP is the default, so patches saved earlier
  filter exactly as they did. Measured on a 220 Hz tone with a 2 kHz cutoff: 0.9997 through the
  lowpass, 0.0120 through the highpass.

- **Ring modulation and additive mixing in the 6-op matrix.** The matrix was FM only; each cell
  now also has an RM row (the source multiplies the target's amplitude) and an ADD row (the
  source's sound is added to the target's). A FM/RM/ADD selector above the grid picks which row
  the same 36 boxes edit, and both new rows default to zero so old patches sound identical.
  Measured: at RM 1.0 the carrier drops 1.0000 → 0.0000 and sidebands appear at 0.5/0.5; ADD 1.0
  puts the source's fundamental at 1.0000 where it was 0.0000.

- **Release samples.** A channel can carry a damper sound (`🎧 Släppljud…` in its sampler panel,
  WAV). When the note ends — key release, step end, or pedal lift — the voice switches to that
  sound instead of fading, which is what a piano does when the string is damped. Without one,
  nothing changes. The path is saved with the project; the audio is read back on load.
  *Shortcut:* one mono release sample per channel, played by the same voice, so a stereo damper
  has to be summed. Unison or velocity-layered release samples would need a real zone model.
  Measured: 0.1242 after the key release with a release sample, 0.0000 without.

- **The pedal is visible in the piano roll.** A PEDAL strip under BEND draws the curve as a block
  (down from a point to the next) and a click toggles the pedal at that step — so a curve that was
  only audible can now be seen and edited. The demo project carries two points to show it.

- **SoundFont loop points are read.** An imported `.sf2` instrument used to end as soon as its
  sample ended; the zone now carries the `shdr` loop and the engine loops it, in playback and in
  the export (same helper, `keymap::zone_loop_01`). A zone without loop points still follows the
  channel's loop setting, so old projects are unchanged. The import hint text said loops were
  ignored — it now says they follow the zone.

- **The pedal is recorded with the take.** CC64 while recording writes a point into the pattern,
  and the curve is played back in pattern mode, from arrangement blocks, and in the export — the
  file now has the pedal the take had. Points land on step boundaries (16th-note resolution), so
  a half-step press snaps to the next step; positions between steps are kept in the curve but the
  engine sets the pedal at the step.

- **Pedal resonance.** With the sustain pedal down, the synth and the sampled instruments feed a
  bank of three tuned combs (A2, E3, A3), so a note leaves a ringing body behind it — what a piano
  does when the strings are free. Drums and stems never feed it, and with the pedal up nothing is
  fed at all, so a project without a pedal sounds exactly as before.
  *Shortcut:* three fixed combs, not a model of the strings — it adds body and decay, not an exact
  copy. Replace with modal synthesis if the difference has to be inaudible.

- **The pedal holds sampled notes too.** A sample struck while CC64 is down keeps sounding after
  the key is released, and lifting the pedal damps it over 40 ms instead of cutting. Without the
  pedal nothing changes — a one-shot still rings out as before. Measured: 0.0920 held 200 ms
  after the key release, 0.0272 and no active voice after the pedal lift.

- **Sustain pedal (MIDI CC64).** The pedal now reaches the engine: a key released while it is
  down keeps sounding, and lifting the pedal releases everything it was holding. It arrives from a
  keyboard as CC64 (>= 64 = down, the MIDI threshold) and from a 🎹 PEDAL button next to the
  metronome, which also shows the state when hardware presses it. Measured: held 200 ms after the
  key release, 0.2718 with the pedal against 0.0359 without.

- **Click a lane in the playlist to comp that bar.** The lane strips inside a pattern block are now
  clickable (Select tool only — with Paint/Erase the click still belongs to the clip), and the
  clicked lane becomes the comp for that bar. Hovering a strip lightens it, so the target is
  visible before the click.

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

- **Piano-roll scripts (Rhai).** A 📜 Skript menu in the piano roll runs `.rhai` files from
  `~/.config/sonix/scripts` against the active take: `count()`, `pitch/velocity/start(i)`,
  `set_pitch/set_velocity/set_start`, `transpose`, `scale_velocity`, and `print` which lands in
  the status bar. The script works on a **copy**, so a failure halfway leaves the take untouched;
  pitches are clamped to 0–127 and a runaway loop hits `max_operations` instead of freezing the
  UI. An example script is written to the folder the first time it is opened.
- **Theme files.** The palette is now a `Palette` with 29 semantic names (`bg`, `panel`,
  `orange`, `track_teal`, …) read from `~/.config/sonix/theme.json`. The file is written with
  the default palette on first run so it can be edited, only the keys you name are changed, and
  a broken file falls back to the defaults instead of stopping the app. Typography stays in code:
  a theme file should not be able to make the text unreadable.
- **Carrier input in the master chain.** A 16-band vocoder sits before the master FX chain
  with the rack's own synth as the carrier and the stems as the modulator, so a synth patch
  speaks the rhythm of a drum loop. The modulator is taken from the stem mix, not the master —
  the synth's own reverb tail otherwise opens the vocoder and it turns into a volume knob.
  `amount` 0 bypasses it completely; it is saved with the project and used by the exporter.
- **Talkback.** A 🎙 button in the control room routes the microphone to the monitors instead
  of into the master, and adds it *after* the control-room stage — so DIM lowers the music but
  not the voice, and someone listening to a cue bus hears it over that bus. Nothing reaches an
  export. Needs the microphone capture (which fills the monitor ring) to be running.
- **Per-operator low-pass in 6-op.** Each operator has a low-pass (cutoff + resonance) after
  its envelope and before the matrix, so a filtered modulator takes its overtones into the sound
  instead of only changing its own level. `cutoff >= 19 kHz` means off and costs nothing, so
  older patches are unchanged; edited with the same OP1…OP6 selector as the envelopes.
- **Per-operator envelopes in 6-op.** Each of the six operators has its own ADSR, applied
  *before* the matrix: a modulator whose envelope closes takes its sidebands with it, which is
  the difference between a sound and a click. Defaults are neutral (level 1, no contour), so
  patches saved before this change sound exactly as before; the envelope is edited one operator
  at a time in the panel.
- **6-op (Sytrus).** Six sine operators in a 6×6 phase-modulation matrix, each with its own
  frequency ratio and output level, reachable as the `Waveform::SixOp` oscillator. Sources with
  a lower index are read from the current sample and the rest from the previous one, which makes
  self-feedback (a saw out of a sine) possible. Three starting points in the panel: sine, chain
  and feedback saw; ratio, level and all 36 matrix cells are editable, and the whole patch travels
  as one command and is saved with the project and used by the exporter.
- **Lane view in the playlist.** A pattern block with more than one take now shows a thin row
  per lane, with the lane that plays in each bar lit (`Pattern::lane_for_bar`: the lane at the
  bar's first step, bar-specific comp before a global one). Read-only — the comp is still
  chosen in the takes window.
- **Comping per bar.** `CompSlice.bar` lets a comp apply to one bar (`None` keeps the old
  "every bar" behaviour), picked with *Takt* in the takes window; a bar-specific comp wins over
  a global one on the same step, and the export snapshot carries one take per bar.

### Fixed

- **The tab follows the view now.** The tab group was stored, so a shortcut or menu could take you
  to a view whose tab was not the highlighted one. It is derived from `view_mode`
  (`nav_group_of`) instead, and each tab remembers the last view you used in it, so clicking a tab
  goes back where you were rather than to the group's first view.

- **Right-clicking a pattern block no longer deletes it.** It opens a menu — open in the piano
  roll, duplicate into the next bar if it is free, or delete. Deleting by right-click was one
  misclick away from losing a block with no undo, and it is not how either Ableton or Soundtrap
  behaves. The Erase tool still deletes on left click.

- **The vocoder button in the vocal studio was unreadable when disabled.** Cyan fill with the
  default light label colour turns into grey-on-cyan as soon as there are no takes to vocode;
  it now uses black text like the other cyan buttons in the same view.

- **The lane view read the song bar instead of the pattern bar.** A one-bar block showed whichever
  comp happened to match the song's bar number, so a per-bar comp appeared on the wrong block
  (or on none). It now uses the bar inside the pattern, which is what `lane_for_bar` is keyed on.
  Found by rendering the arranger in screenshot mode with a two-take pattern.
- **The 6-op editor gets a window of its own.** Twelve knobs and 36 matrix cells did not fit the
  oscillator column: the matrix rows were cut off below the panel. A **⚙ 6-op-matris…** button
  now opens an editor with the presets, one row per operator, the envelope and filter for the
  selected operator, and the whole 6×6 matrix with `→1…→6` headers — verified in a rendered
  screenshot.
- **Documentation screenshots no longer show the recovery dialog.** Screenshot mode keeps the
  "unsaved work found" modal out of the way, so the captured images show the app and not a
  restored autosave prompt.
- **Talkback opens the microphone itself.** The button now starts an input stream when
  talkback is switched on and closes it when switched off, so talkback no longer needs a
  recording to be running. If the vocal studio is already capturing, its ring is reused instead
  of opening a second stream on the same device; with no microphone the status bar says so.
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

- **Theme edits apply without a restart.** The palette used to be read once into a `OnceLock`;
  `reload_if_changed` now compares the file's mtime once per frame and reloads when it moves.
  `Palette` is `Copy`, so `palette()` hands out a copy instead of a lock reference — the 2 476
  call sites did not change. Cost when the file is untouched: one `metadata` call per frame.

- **The tabs now follow what you do, not where things live:** SKAPA (rack, piano roll, vocal
  studio, Alchemy, drums, patcher, AI), ARRANGERA (timeline, video), MIXA (mixer, remix FX, stems,
  plugins) and VERKTYG (dice, chords, FX rack, tuner, song sections). The navigation starts on
  ARRANGERA because that is the view the app opens in. The button list is now one table
  (`VIEWS`) instead of thirteen hand-written blocks, so a view cannot be left out of a group by
  accident.

- **The view row is three tabs instead of thirteen buttons.** VY, STUDIOS and VERKTYG show only
  their own views, so the second row went from a wrapped block of everything to six buttons and a
  tab strip. Same buttons as before — nothing was moved, only hidden until its group is chosen.

- **FL inventory: three descriptions corrected.** Sytrus and Toxic Biohazard said "2-op FM
  exists"; they now describe the 6-op matrix with per-operator envelopes and filters, and name
  what is still missing (Sytrus mixes FM, ring modulation *and* additive in one cell; we do FM
  only). FL Keys mentions velocity layers and the new pedal. The ✅/◐ counts are unchanged — the
  matrix is still FM-only, so calling them done would be a lie.

- **Clippy debt, first pass.** 154 → 47 warnings: mostly `collapsible_if`, `needless_return`,
  `manual_clamp` and similar, applied with `cargo clippy --fix` and verified by the full suite and
  a rendered screenshot. Two orphaned doc blocks that described functions living elsewhere
  (`HighPassFilter`, `stretch_ratio_for`) were moved onto the items they document. The 47 that
  remain are structural — complex types and functions with many arguments — and are left alone
  rather than reformatted for a lint.

- **The envelope graph shrinks instead of overflowing.** It was a fixed 130 px, which pushed it
  out of its column in a narrow window; it now takes the width that is available, up to 130.

- **Dead branches removed in the arranger, mixer and piano roll.** A dozen `if` expressions had
  branches that returned the same colour (the REC button, the slicer highlight, the microphone
  card and clip fills, the MIDI-REC label, the hi-hat generator), so the condition never had an
  effect. Removing them changes no pixel; clippy's identical-block warnings drop from 12 to 2
  (the two that remain are the deliberate white microphone lane).

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
