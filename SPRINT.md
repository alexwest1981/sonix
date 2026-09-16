# SPRINT 1 — från "instrument in" till "loopbar live"

**Skapad 2026-09-16** efter Alex' tre frågor (instrumentstöd, Launchpad/looper, plugg-and-enjoy
för gitarrkablar) och de tre research-rapporterna i `~/.sonix-research-*.md`
(looper-semantik, Linux-ljudstacken, pad-protokoll).

Regeln för planen: **varje punkt ska kunna bli klar utan att Alex sitter vid datorn**, utom de som
är märkta `[Alex]`. Punkter utan märkning kan byggas, mätas och pushas autonomt.

---

## Läget före sprinten (mätt i koden 2026-09-16)

| Det som frågades | Svar i dag |
| :--- | :--- |
| MIDI-klaviatur → Sonix synth | **Ja** (`midi_input.rs`, `midir`, ansluter till alla in-portar automatiskt) |
| Ljud in (gitarr/bas/mic) | **Ja** — ingångsström, enhetsval, gain/VU, gate, monitor, autotune, tagningar |
| Live-ingång genom spårkedjan (amp-sim) | **Nej** — mikrofonen mixas in i mastern efter autotune (`process.rs` 8c) |
| MIDI **ut** (LED, extern synth) | **Nej** — ingen `MidiOutput` i kodbasen |
| VSTi/CLAP-instrument | **Nej** — "VST-instrument stöds inte" (roadmapen), MIDI-in i plugins bara mock |
| Launchpad/looper | **Nej** — ingen clip/slot-modell, ingen launch-kvantisering |
| Taktslinga i transporten | **Ja** (`loop_start_bar`/`loop_end_bar`) |
| Avancerad timing på MIDI-tagningar | **Ja** (`midi_take.rs`, steg + swing + kvantisering) |
| Instrumentkabel känns igen och ställs in själv | **Nej** — dagens urval är `namn.contains("samson"/"usb"/"mic")` |

---

## Punkt 1 — Instrumentkabeln känns igen (plugg and enjoy) · *S*

**Vad:** en tabell över kända kablar/interface (USB-id **och** namn), som klassar varje ingång som
*instrument*, *line* eller *mikrofon*. När en instrumentingång finns: välj den, sätt instrumentets
standardvärden (gain, gate, lågcut, monitor) och tala om exakt vad som gjordes.

**Rocksmith-kabeln är verifierad i `/usr/share/hwdata/usb.ids`:**
`12ba:00ff — Licensed by Sony Computer Entertainment America — "Rocksmith Guitar Adapter"`.
Maskinen saknar kabeln, så klassningen prövas mot tabellen och mot en sysfs-fixtur — inte mot hårdvara.

**Klart när:** enheten syns med rätt etikett i ljudmodalen, en knapp ger instrumentläge i ett klick,
och `input_profile`-proven faller om tabellen eller sysfs-läsningen ändras.

**Läget 2026-09-16:** byggd. Etiketter, knapp och automatiken vid appstart är på plats; 11 prov i
`input_profile` + 1 i18n-prov; 590 tester default / 639 med plugin-host, 0 varningar i alla fyra
byggena. **Kvar:** kvittens på riktig hårdvara och att valet minns mellan starter.

**Filer:** `src/audio/input_profile.rs` (ny), `src/audio/mod.rs`, `src/ui/app/modals.rs`, `src/i18n.rs`.

## Punkt 2 — Låg latens som standard, inte som manuellt val · *S–M*

**Vad:** research-rapporten mätte att Sonix öppnar strömmen på **44 100 Hz** medan PipeWires graf kör
**48 000 Hz** — alltså ligger den adaptiva resamplern i live-vägen — och att `BufferSize::Default`
mot pipewire-alsa kan ge en ~21 s ring (cpal #1029). 48 kHz + `Fixed(128)` tar bort båda.

**Klart när:** standardinställningen är 48 kHz och buffert 128 när inget sparats, självtestets
 realtidstal är mätt före/efter i samma körning, och en enhet som inte klarar 128 faller tillbaka
**med besked** i stället för tyst.

**Filer:** `src/audio/engine.rs` (`AudioSettings::default`), `src/selftest.rs`.

## Punkt 3 — Live-ingång genom spårkedjan (gitarr genom amp-sim) · *M*

**Vad:** i dag går monitorn rakt in i mastern. Punkt 3 ger ingången en **väg genom spåret** —
monitor -> spårets effektkedja -> master — så en amp-sim eller en kanalremsa hörs medan man spelar.

**Klart när:** en gitarr genom en lastad effekt hörs i monitor, latensen är mätt (inte gissad), och
en trasig plugin tystar inte ingången (8.5-regeln: säg fel, spela inte fel).

**Filer:** `src/audio/recorder.rs`, `src/audio/synth/process.rs`, `src/ui/app/modals.rs`.

## Punkt 4 — MIDI ut + en kontrolleryta som abstraktion · *M*

**Vad:** `midir` kan skriva lika väl som läsa, men ingen `MidiOutput` finns. Punkt 4 ger
(a) en MIDI-ut-port, (b) ett rutnäts-abstraktionslager (device descriptor: rutnät, knappar, modes,
feedback-tick) och (c) en Launchpad-drivrutin (färger via SysEx). Utan hårdvara prövas allt mot
ALSA:s `Midi Through`-port och en virtuell port (`snd-virmidi`).

**Klart när:** en modul kan skicka not/CC/SysEx till en port, feedback-ticken går utanför ljudtråden,
och proven täcker mappning + färgkodning utan hårdvara.

**Filer:** `src/audio/midi_output.rs` (ny), `src/audio/controller.rs` (ny), `src/audio/mod.rs`.

## Punkt 5 — Clip/slot-modellen och launch-motorn · *L*

**Vad:** slots med tillstånd (tom/armerad/spelar/queued), scener, launch-kvantisering mot takt/slag,
count-in och — sist — looppad inspelning: spela in medan transporten går, klipp till hela takter,
spela upp direkt i slingan.

**Byggstenar som redan finns:** taktslingan, MIDI-tagningarnas stegtajming, samplerns looplägen,
låtsektionerna (`SongSectionItem`) och importvägen som lägger ljud taktbundet.

**Klart när:** semantiken i research-rapporten (`~/.sonix-research-looper-sv.md`, de 8–12 gemensamma
reglerna) är implementerad som en motor med prov, och en inspelning kan startas och loopa utan att
längden gissas.

**Filer:** `src/audio/looper.rs` (ny), `src/audio/synth/*`, `src/ui/app/*`.

## Punkt 6 — Ljudingångens klocka och ärliga gränser · *S*

**Vad:** ingången öppnas med enhetens egen config och monitorn dräneras en sample per ut-sample —
samma rate *antas*. Punkt 6 gör antagandet till en kontroll: är rate:n olika, säg det och låt bli
att monitorera i fel tempo. (Bygger på samma fynd som punkt 2.)

**Filer:** `src/audio/recorder.rs`, `src/audio/input_profile.rs`.

---

## Saker som kräver Alex (och därför inte startas)

- `[Alex]` **RT-prioritet:** `rtkit` är inte installerat (`mod.rt: RTKit error … ServiceUnknown`), så
  PipeWires ljudtråd kör `SCHED_OTHER`. Utan RT är quantum 128 den ärliga nivån; 64 kräver RT.
- `[Alex]` **PipeWire-drop-in** för quantum 128 (`~/.config/pipewire/pipewire.conf.d/99-latency.conf`)
  — en systemändring, inte en appändring.
- `[Alex]` **Kabel/interface:** en gitarr rakt in i ALC1220:s line/mic är fel impedans; en USB-kabel
  med instrumentingång (Rocksmith, Guitar Link, iRig, fokusrite med instrumentläge) behövs för att
  kvittera punkt 1 på riktig hårdvara.
- `[Alex]` **Launchpad/fysisk kontroller** för kvittens av punkt 4 — tills dess prövas allt mot
  `Midi Through`.
- `[Alex]` 4.6/7.3 (Wine/yabridge) och 7.1 (Windows) ligger kvar som de var: de kräver honom.

## Ordning

1 → 2 → 6 → 3 → 4 → 5. Punkt 1 och 2 är små och tar bort det som mest skriker ("jag kopplade in och
inget hände" respektive "det svarar sent"). Punkt 5 är den stora och sist, eftersom den vilar på 3 och 4.
