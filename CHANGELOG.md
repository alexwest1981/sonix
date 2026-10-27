# Changelog

All notable changes to Sonix Studio are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.167] - 2026-10-27

**En död ljudström byggs om i stället för att tiga.** cpal 0.15 återhämtar sig inte från en
ALSA-ström som svarat `POLLERR`: `alsa::poll()` fortsätter svara fel, callbacken kallas aldrig
mer — men strömmen ser fortfarande `RUNNING` ut i `pactl`. Mätt: 40 000 fel/s, 1,5 GB logg, en
kärna bränd och noll ljud när play trycktes.

- **Hjärtslag per buffert** i ljudcallbacken. Står det stilla en sekund byggs strömmen om med
  samma `reconfigure` + `resync_engine_after_reconfigure` som ljudmodalen, och spelar vidare
  från samma position om transporten rullade. Cooldown 2 s så en enhet som vägrar starta inte
  byggs om varje bildruta.
- **Felloggen dämpad:** tre rader och sedan var 20 000:e fel, både i `err_fn` och i
  panikvägen — en död ström ska synas, inte äta disken.
- **Felräknaren hör till strömmen** som dog: `reconfigure` nollställer den, och vakthunden
  läser den innan omstarten, så statusraden säger den döda strömmens fel och inte appens total.

**AI:n lovar inte längre mer än rutnätet kan skriva.** `write_pattern` skriver en not per steg
utan anslag och längd; modellen kunde ändå be om trioler och legato som appen tyst kastade bort.

- Prompten säger vad som gäller (sextondelsrutnät, en not per steg, heltal), och `parse_notes`
  avvisar bråktal med ett begripligt fel i stället för "steget -1 ligger utanför".
- Varje stämma bär en `subdivision` (1/16 … 1/1); anropet avvisas om slagen ligger utanför den,
  med raden som visar vilka steg som bryter.
- `2.0` läses som 2 — JSON-bråktal föll tidigare igenom som fel.

## [0.9.165] - 2026-10-27

**AI:ns melodi blir ett objekt på tidslinjen.** Den skrev in i mönstrets rutnät, och där fanns
inget att greppa — ingen klippa att flytta, inget att spara. Nu skriver `write_notes` en
**klippa** (`PatternClip`) med egna noter: position, anslag och notlängd.

- **`PatternClip`** på spåret (`pattern_clips`): start och längd i `f32` takter som en
  ljudregion, `lane` mot kanalracket, och noterna i klippan. Sparas i projektfilen
  (`#[serde(default)]`, så gamla projekt öppnas som förut).
- **AI:n skriver noter, inte steg.** `write_notes` skapar klippan vid spelhuvudet på valt spår i
  stället för att tända rutor i rutnätet — notlängd och anslag följer med, vilket rutnätet inte
  kunde bära. Trummor ligger kvar i rutnätet; en klippa utan noter spelar mönstrets rutnät.
- **Klippan ritas och dras** i arrangemanget med snäpp och ångra. Flytten skriver bara
  `start_bar`, så noterna kan inte ändras av en förflyttning (prov i `state::clip_tests`).
- **Uppspelning:** en klippa spelar sin kanal, läser kanalens sampel vid varje slag (bytt ljud
  behåller noterna) och loopar sin egen längd.
- **Fyra döda AI-prov körs igen.** Tre prov i `ai_tools` hade förlorat sina `#[test]`-attribut i
  en tidigare commit och räknades aldrig — de ligger nu på rätt funktion, och `grid_of` känner
  igen de svenska pluralformerna ("åttondelar") som provet kräver.

Kvar till nästa pass: knappen som lägger klippan vid spelhuvudet, Sample-export (WAV + `.mid`),
och triolernas `pos`-schemaläggning — se `docs/AI-CLIPS.md`.


## [0.9.162] - 2026-10-26

**Sju språk, inte bara i appen.** Gränssnittet har kunnat sju språk sedan tidigare, men repot och
skrivbordsposten var engelska — och GitHub visar bara EN README, så en spansk eller tysk besökare
möttes av ett språk hen kanske inte läser.

- **`README_DA/NB/DE/ES/FR.md`** — korta README-filer på danska, norska, tyska, spanska och
  franska: vad det är, skärmdump, installationstabell, licens, och en länk till den engelska
  README:n för hela funktionslistan. Korta med flit: en full översättning av 400 rader ruttnar
  mellan releaserna, det här håller.
- **Språkrad i varje README** som länkar till alla sju, så ingen fil är en återvändsgränd.
- **Skrivbordsposten är översatt** (`Comment[sv/da/nb/de/es/fr]`, `GenericName[...]`,
  `Keywords[...]`) — appens namn och beskrivning syns nu på användarens språk i menyer och
  programstartare på alla distribuerade paket. `desktop-file-validate` godkänner den.
- **AppStream-metainfo** har `summary xml:lang` för samma sex språk, så mjukvaru-butiker visar en
  lokaliserad sammanfattning. `appstreamcli validate` godkänner den.
- **`packaging/check-docs-languages.sh`** är grinden: den faller när en README saknas, när
  språkraden inte länkar till alla, eller när .desktop/metainfo saknar ett språk. Körd i CI, och
  provad genom att ta bort en fil (den föll, som den ska).
- Release-tarballen tar med alla README-filer.

Manualen och CONTRIBUTING är kvar på engelska — det är sagt rakt ut i stället för halvöversatt.


## [0.9.161] - 2026-10-26

**Licensen är nu GPL-3.0-only.** Projektet var dubbellicensierat MIT OR Apache-2.0, vilket är
exakt motsatsen till vad som efterfrågades: båda licenserna tillåter vem som helst att ta koden,
stänga den och sälja den under ett annat namn. GPL-3.0 behåller det som var poängen — bidrag är
välkomna och ingen kan ta programmet och sälja en inlåst version.

- `LICENSE` är GPL-3.0-only, ordagrant ur SPDX (`GPL-3.0-only.txt`), med copyrightraden i README.
  `LICENSE-MIT` och `LICENSE-APACHE` är borta.
- **`CONTRIBUTING.md`** är ny: hur man bidrar, `git commit -s` (DCO) som enda pappersexercis,
  kodkonventionerna, och varför en tredjepartsplugin inte ärver programmets licens (CLAP/VST3 laddas
  som separata binärer vid körning).
- README (båda språken) förklarar licensen med vanliga ord: använd/byg/ändra/dela fritt, dela källkod
  när du delar en ändrad Sonix, inga CLA:er, plugins oberoende, och att **namnet** inte följer med
  koden — en fork bär eget namn och ikon. Samt att versioner till och med 0.9.160 behåller sina
  villkor, eftersom en licens inte är retroaktiv.
- `Cargo.toml`, `nfpm.yaml` (deb/rpm-licensfältet), AppStream-metainfo och release-tarballen pekar
  på den nya licensen; paketen skeppar `LICENSE` + `CONTRIBUTING.md` i stället för de två gamla
  filerna.

## [0.9.160] - 2026-10-26

**Fungerar på alla distributioner.** Tills i dag fanns binären bara som ett tarball byggt på en
Ubuntu-runner: glibc ≥ 2.35 för x86-64 och ≥ 2.39 för aarch64, vilket stänger ute Debian 11,
Ubuntu 20.04 och hela RHEL 8-serien — och en binär byggd på Arch begärde till och med GLIBC_2.44
och startade inte ens på Ubuntu 24.04. Nu byggs releasen i en **glibc 2.28**-container för båda
arkitekturerna, och flödet **stannar** om någon symbol begär nyare.

- **`packaging/check-glibc-floor.sh`** — grinden. Läser binärens GLIBC-symbolversioner och faller
  över golvet. Körs både i CI och i releaseflödet.
- **`packaging/verify-in-docker.sh`** — beviset. Installerar paketet i en riktig distribution och
  startar appen med Xvfb; ett nytt CI-jobb kör den på Debian 12, Ubuntu 22.04, Ubuntu 24.04,
  Fedora 41 och Rocky 9.
- **Fyra former av samma binär:** `.deb` (Debian, Ubuntu, Mint, Pop!…), `.rpm` (Fedora, RHEL,
  Rocky, Alma, openSUSE), **AppImage** (ingen installation alls) och tarballen som förut.
  `SHA256SUMS` ligger bredvid och alla filer attesteras med byggbevis.
- **`packaging/nfpm.yaml`** — paketberoendet deklareras per paketformat (`libasound2` för deb,
  `alsa-lib` för rpm) och `ffmpeg` som rekommendation, eftersom WAV/FLAC fungerar utan.
- **`packaging/io.github.alexwest1981.Sonix.{desktop,metainfo.xml}`** — desktoppost, ikon och
  AppStream-metadata som `appstreamcli validate` godkänner, så appen syns i programmenyer och
  mjukvaru-butiker med namn, ikon och skärmdumpar.
- README (båda språken) svarar nu på frågan en ny användare har: *vilken rad ska jag köra?* — en
  tabell per distribution, vad varje form installerar, och hur den tas bort.

## [0.9.159] - 2026-10-25

**aarch64-release.** Release-workflowen bygger nu båda arkitekturerna: x86_64 på `ubuntu-22.04`
och aarch64 på `ubuntu-24.04-arm`, som två jobb som laddar upp var sin artefakt och en tredje som
attesterar och publicerar dem tillsammans. Det var den sista varningen i OmaStore — utan en
arm-tillgång ser appen "inte installerbar" ut för aarch64-användare.

- `omastore.toml` deklarerar nu `linux.aarch64` med samma `{version}`-mönster.
- ARM-binären kräver glibc ≥ 2.39 (Ubuntu 24.04); x86_64 ligger kvar på ≥ 2.35.
- `.SRCINFO`/`PKGBUILD` deklarerar `alsa-lib` och `glibc`, så butiken kan kontrollera beroenden.

## [0.9.158] - 2026-10-25

Första releasen sedan 0.9.0 — **579 commits**. Den fullständiga listan står i
[CHANGELOG-DETAIL.md](CHANGELOG-DETAIL.md); det här är vad man märker.

- **Masterns FX-rack**: Love Philter (tre filter i serie), Luxeverb (FDN), Emphasis,
  Hyper Chorus, Maximus (tre band + master, solo per band, ritbara volymkurvor),
  16-bands vocoder med egna bandnivåer och Time Curve som bandspelarhastighet.
- **Tempot mäts ur ljudet** vid import och per klipp, och **tempokartan gäller överallt**:
  uppspelning, sökning, nedräkning och export. Klipp utan känt tempo behåller sina sekunder.
- **Edison, Newtone och Newtime** som redigerare på urval, med spektrum, tonhöjd och tid.
- **FLEX-fabriken**: åtta namngivna ljud som följer med appen.
- **38 skärmdumpar** i `screenshots/`, tagna mot den här versionen.

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

[Unreleased]: https://github.com/alexwest1981/sonix/compare/v0.9.167...HEAD
[0.9.167]: https://github.com/alexwest1981/sonix/compare/v0.9.165...v0.9.167
[0.9.159]: https://github.com/alexwest1981/sonix/compare/v0.9.158...v0.9.159
[0.9.158]: https://github.com/alexwest1981/sonix/compare/v0.9.0...v0.9.158
[0.9.0]: https://github.com/alexwest1981/sonix/releases/tag/v0.9.0
