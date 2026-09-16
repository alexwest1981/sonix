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

**Detaljerna är nu researchade** (`~/.sonix-research-pads-sv.md`): Launchpads rutnät skickar
`note = 11 + kolumn + 10 × rad`, kanal 1/2/3 = statisk/blink/puls; färger sätts med SysEx
`F0 00 20 29 02 7F … F7` (Programmer mode vid anslutning, `0E 00` tillbaka till Live-, respektive
appläget vid avslut) och enheten hittas med Device Inquiry. Abletons modell är fyra begrepp —
Live Object Model (läs/anropa/**observera**), Element (typ + kanal + nummer), Component (beteende)
och Layer/Mode (samma knappar, annan uppgift) — och ljuset är **händelsedrivet**: pad → note →
element → `clip_slot.fire()` → tillståndet ändras → lyssnare → ny färg. En shadow-diff är enda
vägen till ljuset (skicka bara det som ändrats). Push använder ett eget SysEx-protokoll, APC kör
CC+not — därför ska drivrutinen vara en tabell över rutnät/knappar och inte en `if` per modell.

**Klart när:** en modul kan skicka not/CC/SysEx till en port, feedback-ticken går utanför ljudtråden,
och proven täcker mappning + färgkodning utan hårdvara (virtuell port räcker — allt utom LED,
latenskänsla och firmware-egenheter går att bygga och mäta utan en Launchpad).

**Filer:** `src/audio/midi_output.rs` (ny), `src/audio/controller.rs` (ny), `src/audio/mod.rs`.

## Punkt 5 — Clip/slot-modellen och launch-motorn · *L*

**Vad:** slots med tillstånd (tom/armerad/spelar/queued), scener, launch-kvantisering mot takt/slag,
count-in och — sist — looppad inspelning: spela in medan transporten går, klipp till hela takter,
spela upp direkt i slingan.

**Byggstenar som redan finns:** taktslingan, MIDI-tagningarnas stegtajming, samplerns looplägen,
låtsektionerna (`SongSectionItem`) och importvägen som lägger ljud taktbundet.

**Klart när:** semantiken i research-rapporten (`~/.sonix-research-looper-sv.md` — **12 gemensamma
regler** och **6 MVP-punkter**, 19 källor) är implementerad som en motor med prov, och en inspelning
kan startas och loopa utan att längden gissas.

**Läget 2026-09-16 (steg 1 + 2 byggda, på Alex' fråga "kan vi bygga en mjukvarustyrd?"):**
modellen `src/audio/launcher.rs` (slot/scen-tillstånd, kö, kvantisering, åtta regler ur
research-rapporten, `tick` som enda väg från klick till kommando) och ytan
`src/ui/launcher_view.rs` + `src/ui/app/launcher.rs` (**🎛 Scenrutnät (mjuk Launchpad)** i
Verktygsmenyn: rutnät ur låtsektionerna, färgen visar kön, scenknapp per rad, ⏹ Stoppa allt).
13 nya prov; kvantiseringsprovet mätt med flit-bort. **Kvar:** tangentbordsstyrning (spela
rutnätet med en hand), inspelning till en slot (det som gör det till en *looper* och inte bara
en startare), och att MIDI-ut (punkt 4) driver exakt samma modell.

**Filer:** `src/audio/launcher.rs` (modellen), `src/ui/launcher_view.rs` (ytan),
`src/ui/app/launcher.rs` (glue:t), `src/audio/synth/*` (nästa steg: inspelning till slot).

## Punkt 6 — Ljudingångens klocka och ärliga gränser · *S*

**Vad:** ingången öppnas med enhetens egen config och monitorn dräneras en sample per ut-sample —
samma rate *antas*. Punkt 6 gör antagandet till en kontroll: är rate:n olika, säg det och låt bli
att monitorera i fel tempo. (Bygger på samma fynd som punkt 2.)

**Läget 2026-09-16:** byggd. Regeln är en ren funktion (`rate_verdict`) med prov, och vakten i
`sync_mic_monitoring` håller monitorn av och skriver varför: *"⚠ ingången går i 44 100 Hz och
motorn i 48 000 Hz — monitorn är av (annars hörs fel tempo)"*. Provet är mätt med flit-bort-metoden
(jämförelsen vänd → provet faller på exakt den raden). **Kvar:** att resampla i monitorvägen, så att
olika tempo går att monitorera i stället för att bara vägras — det är nästa steg, inte ett prov.

**Filer:** `src/audio/input_profile.rs` (`rate_verdict`), `src/ui/app/transport.rs` (vakten).

## Punkt 7 — Loop-pad (loopstation): spela in ett lager i taget · *M–L*

**Vad:** ett tryck spelar in exakt en loop-cykel, tar stopp av sig själv vid cykelns slut och börjar
genast spela; nästa tryck lägger ett lager ovanpå (ett nytt spår), och ångra tar bort det sista.
Målet Alex satte: *"spela in en hel låt med bara sin röst"* (Petebox' Creep-cover).

**Reglerna** (samma research som startaren — `~/.sonix-research-looper-sv.md`): inspelning börjar
vid **nästa kvantpunkt** och transporten startar själv om den stod stilla; `Rec-Length` är
**loopens** längd, inte hur länge knappen hölls; övergången till uppspelning sker **utan att
stanna**; när ingen slinga finns får **första lagret bestämma** längden (förfluten tid avrundad
uppåt till takt, minst en takt); flera pass per tryck är valbara (`At Rec-End`); och stannar
transporten mitt i avbryts tagningen i stället för att bli ett tyst spår som ligger och skräpar.

**Det som gör det nära i Sonix** (mätt 2026-09-16): spåren är redan PCM i minnet
(`StemVoiceTrack { left/right: Arc<Vec<f32>>, start_time_secs, .. }`), tagningen är redan PCM
(`AudioTake { pcm_samples, sample_rate }`), transporten har redan en taktslinga som rullar tillbaka,
och mikrofonen spelar redan in med monitor och autotune. Kvar är **kopplingen**: tagning → spår vid
slingans start, och att stoppa inspelningen på takten i stället för på en knapp.

**Steg:**
1. Modellen och reglerna — ✅ 2026-09-16 (`src/audio/loop_station.rs`, 10 prov).
2. Kopplingen: tagningen blir ett `StemVoiceTrack` vid slingans start, ett **● Lager**-tryck i
   gränssnittet, och stoppet sker på takten (appens tick).
3. Per lager: volym, mute, solo — och lagren syns som vanliga spår i arrangemanget.
4. Röstbekvämligheter: autotune per lager, panorering, och "harmoniera med dig själv" genom den
   befintliga stäm-apparaten.

**Filer:** `src/audio/loop_station.rs` (modellen), `src/ui/app/loop_pad.rs` (nästa steg),
`src/audio/synth/voices.rs` (spåret lagret blir).

---

---

## Saker som kräver Alex (och därför inte startas)

- `[Alex]` **RT-prioritet:** mätt i rapporten (`~/.sonix-research-linux-ljud-sv.md`) — `rtkit` finns
  inte ens som paket på maskinen, journalen säger `mod.rt: RTKit error: …ServiceUnknown` följt av
  *"RTKit does not give us MaxRealtimePriority, using 1"*, `LimitRTPRIO=0` i systemd-uniten, och
  `chrt -p` visar att `pipewire`, `module-rt` **och `data-loop.0`** (själva ljudtråden) kör
  `SCHED_OTHER` med `rtprio -`. Ljudtråden har alltså ingen realtidsprioritet alls.
- `[Alex]` **PipeWire-drop-in** för quantum 128 (`~/.config/pipewire/pipewire.conf.d/99-latency.conf`)
  — en systemändring, inte en appändring. Rapporten mätte också att **Pro Audio-profilen inte är
  aktiv** (kort 0 står på `output:analog-stereo+input:analog-stereo`), vilket är själva skälet till
  att PipeWire kör timer-baserade wakeups i stället för JACKs IRQ-drivna graf.
- `[Alex]` **Kabel/interface:** en gitarr rakt in i ALC1220:s line/mic är fel impedans; en USB-kabel
  med instrumentingång (Rocksmith, Guitar Link, iRig, fokusrite med instrumentläge) behövs för att
  kvittera punkt 1 på riktig hårdvara.
- `[Alex]` **Launchpad/fysisk kontroller** för kvittens av punkt 4 — tills dess prövas allt mot
  `Midi Through`.
- `[Alex]` 4.6/7.3 (Wine/yabridge) och 7.1 (Windows) ligger kvar som de var: de kräver honom.

## Ordning

1 → 2 → 6 → 3 → 4 → 5. Punkt 1 och 2 är små och tar bort det som mest skriker ("jag kopplade in och
inget hände" respektive "det svarar sent"). Punkt 5 är den stora och sist, eftersom den vilar på 3 och 4.
