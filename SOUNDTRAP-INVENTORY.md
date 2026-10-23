# Soundtrap — inventering och gap mot Sonix

> Låst 2026-10-23. Soundtrap (Spotify) är en **webb-DAW**; den jämförs här bara för att se vilka
> funktioner de har som vi inte har. Soundtraps sida är läst samma dag (Music Makers, produktsidorna
> för DAW/patterns/sampler/piano roll/song templates/record-voice, prissidan och supportartiklarna —
> källförteckningen med URL:er per påstående ligger i [`docs/soundtrap-kallor.md`](docs/soundtrap-kallor.md)).
> Vår egen kolumn är **mätt i koden**, inte läst ur en roadmap: varje rad har fil, symbol och radnummer,
> och varje nollpåstående har mönstret som söktes. Räkningen i tabellen är gjord programmatiskt ur raderna.
>
> Två researchagenter tog fram underlaget (en åt Soundtrap, en åt oss), och **båda hade fel om oss**:
> samplern och glide beskrevs som saknade trots att `keymap.rs`/`sf2.rs` och `instruments.rs` finns.
> Rättelserna står i tabellen. En agent är ett underlag, inte ett facit.

---

## 1. Vad Soundtrap är

Webbläsarbaserad studio med molnlagring, auto-save, realtidssamarbete (video och chatt i studion),
projektkommentarer, loopbibliotek, färdiga låtmallar och ett antal virtuella instrument
`[belagt: soundtrap.com/musicmakers]`. Editorn har volym, pan, effekter per spår, automation för
volym/pan/filter-sweep `[belagt: support.soundtrap.com/hc/en-us/articles/205662071]`, piano roll med
notlängd, tonhöjd, velocity och kvantisering `[belagt: soundtrap.com/content/product/online-piano-roll]`
och Pattern Beatmaker som stegsequencer med 100+ kit `[belagt: soundtrap.com/content/product/patterns-beatmaker]`.

Genomgående: **Soundtrap säljer enkelhet och delning, inte djup.** Det som är låst bakom betalning är
rösttuning, automation, sampler, VST-värd och vissa masteringstilar
`[belagt: support.soundtrap.com/hc/en-us/articles/360023190973]`.

## 2. Mätt status i Sonix, rad för rad

| Funktion Soundtrap har | Sonix | Bevis (fil:rad, symbol, mönster) | Kommentar |
| :--- | :---: | :--- | :--- |
| Realtidssamarbete — flera i samma session, video och chatt i studion | ✗ | `rg -i 'collab|websocket|multiplayer|video.?chat' src` → 0 träffar | Ingen nätkod alls; projektet är en lokal fil. |
| Projektkommentarer på delat projekt | ✗ | `rg -i '\bcomment(s)?\b' src` → bara kodkommentarer | Kanalanteckningar finns (`PlaylistTrack::notes`), inte kommentarer i tidslinjen. |
| Molnlagring och delningslänk | ✗ | `rg -i 'share.?link|cloud.?project|upload' src` → 0 träffar | Export skriver lokal fil (`exporter.rs:655`). |
| Auto-save och återställning efter krasch | ✅ | `autosave.rs:30` 60 s, `:100 write_atomic`, `:205 prune` (fem versioner/projekt), dialog `ui/app/modals.rs:872`, prov `ui/app/tests.rs:1463` | Starkare än Soundtraps: atomisk skrivning, fingeravtryck mot oförändrade kopior. |
| Time Restore — namngiven versionshistorik | ◐ | `autosave.rs:190 latest_per_project` | Fem tidsstämplade versioner; inga namn, inget gränssnitt att bläddra i. |
| Patterns beatmaker — stegsequencer | ✅ | `audio/exporter.rs:263 triggers_for_step`, `:72 ChannelSnap.steps`, `MANUAL.md:327`, prov `exporter.rs:1181` | Sexton steg, pattern- och song-läge renderas i motorn. |
| Trumkit och 808 med glide | ✅ | `audio/drum.rs:5 DrumType`, `:38 trigger`; glide som röstparameter `instruments.rs:53` | Glide finns som parameter på rösten — 808-glid är förval, inte en saknad funktion. |
| Multi-sampler: zonmappning, grundton, ADSR | ✅ | `audio/keymap.rs:22 SampleZone` (`root`, `key_low/high`, anslagsintervall), `audio/sf2.rs` läser SoundFont-instrumentlagret, `ui/app/mixer.rs:1713` | `references/daw-comparison.md` säger "ingen riktig sampler" — den raden är föråldrad sedan keymap/sf2 byggdes. |
| Piano roll med notlängd och kvantisering | ✅ | `ui/app/piano_roll.rs:163`, `midi_take.rs:149 TakeNote` + `:162 length_steps`, kvantisering i `midi_take.rs` | `ROADMAP.md:49` säger att notlängder saknas; samma fil `:55` säger att de byggdes efteråt. Koden vinner. |
| Automation som kurvor: volym, pan, filter | ✅ | `ui/app/timeline.rs:201 bus_automation`, `AutomationLane`/`AutomationPoint` i `ui/app/state.rs`, prov `ui/app/tests.rs:1751` | Kurvor i tidslinjen; filter sweep som modulationsmål. |
| Sångkedja: reverb, dist, EQ, delay, kompressor | ◐ | `audio/effects.rs`, `audio/master_fx.rs:135`, rack `ui/fx_rack_modal.rs` | Effekterna finns i racket; att var och en kan sättas **på ett sångspår** är inte mätt, bara att racket finns. |
| Rösttuning — realtid och efterhand | ✅ | `audio/vocal_harmonizer.rs:606` skal­snappning, `:935` pitchspårning, `ui/vocal_studio_view.rs:79` realtids­knapp med styrka | Realtids-Auto-Tune finns; provet `:1445` mäter frekvenstoppen, inte en livespelning. |
| Voice Transform — pitch med bevarade formanter | ✅ | `ui/vocal_studio_view.rs:1161 formant_shift`, pitch shifter ±12 halvtoner med bevarade formanter (`ui/app/modals.rs:3216`) | Samma två rattar som Soundtrap säljer som Voice Transform. |
| Auto-mastering med namngivna stilar, på vid export | ◐ | Fem kedjemallar i `ui/fx_rack_modal.rs:980` (bl.a. "Clean Master"), LUFS-mål −14/−1 dBTP i `audio/loudness.rs:137/229/270` | Mallarna och mätningen finns; ingen automatisk efterbearbetning av varje export, och inga Soundtrap-stilar (Cassette, Sub Boost). |
| Låt­mallar att utgå från | ◐ | `ui/app/project.rs:1435 list_templates`, `:1454 save_as_template`, `:1475 new_from_template`, `MANUAL.md:469` 19 spårmallar | Man kan spara och ladda mallar; inga medföljande arrangerade låtstarter (intro/vers/refräng). |
| Licensierat loop- och ljudbibliotek | ◐ | `audio/factory_samples.rs:123 generate_all_factory_samples` (DSP-genererat vid första start), `ui/app/browser.rs:369` | Eget genererat innehåll, ingen katalog. Soundtrap annonserar 24 000+ loopar. |
| Tredjepartsplugins | ✅ | `audio/plugin_vst3.rs:1227`, `plugin_vst2.rs:401`, `plugin_host_live.rs:421`, sandbox i egen process | Vi ligger före: Soundtrap har VST3/AU-**effekter** bara i desktop-betan, utan VSTi och utan masterkanal. |
| MIDI-import och -export | ✅ | `ui/app.rs:593` MIDI-fil in/ut, export `ui/app/midi.rs:293` | Per spår, som Soundtrap, plus SMF i tempokartan. |
| Export: MP3, WAV, OGG, M4A | ✅ | `audio/exporter.rs:567 ExportFormat` — WAV 16/24/32f, FLAC, MP3 320, OGG, AAC | Fler format än Soundtraps fyra, plus EBU R128 och dither. |
| Webbläsare och mobilapp | ✗ | `rg -i 'wasm|webassembly|service.?worker|pwa' src Cargo.toml` → 0 träffar | Medvetet avstått: `ROADMAP.md:89` säger att FL Mobile är en annan plattform och motor. |
| Arbeta offline utan nät | ✅ | `src/main.rs` desktopprogram, all DSP lokalt | Soundtrap förutsätter webbstudio och molnlagring. |

**Räknat:** 12 ✅ · 5 ◐ · 4 ✗ av 21 rader — **57 % i nivå, 81 % helt eller delvis**. De fyra ✗ är
alla samma sak: nätet (samarbete, kommentarer, molnlänk) plus webbläsar-/mobilversionen.

## 3. Gapet, rangordnat efter vad det ändrar i arbetssättet

1. **Delad live-session.** Det är Soundtraps hela produkt och det enda gapet som ändrar *hur* man
   arbetar: idag är ett projekt en fil på en disk. Kräver nätkod (session, operationer, konflikt­lösning),
   inte en ratt — **L/XL**, och det är ett produktbeslut, inte en feature.
2. **Molnprojekt med delningslänk.** Billigare än (1) och ger 80 % av nyttan för en hemstudio: en
   `.sonix`-fil som kan hämtas via länk. **M** (server + lagring), men börja med export + delbar fil.
3. **Innehåll: loopar och arrangerade låtstarter.** Vi har `save_as_template`/`new_from_template` och
   DSP-genererade fabriksljud, men **ingen katalog** — Soundtrap har 24 000+ loopar, 100+ kit och
   låtmallar med intro/vers/refräng. Detta är innehåll, inte kod, och det är den största *upplevda*
   skillnaden (samma slutsats som `references/daw-comparison.md` drar om FL och Ableton).
4. **Auto-mastering som händer av sig själv.** Vi har fem kedjemallar (`fx_rack_modal.rs:980`) och
   LUFS/true peak-mätning; Soundtrap kör mastering på varje export som standard och låter användaren
   välja stil (Punchy, Louder, Cassette …). Gapet är en efterbearbetningsväg i exporten plus fler
   stilar — **S**, och det enda på listan som är billigt.
5. **Versionshistorik med namn.** Vi har fem tidsstämplade autosaves; Soundtrap har Time Restore som
   en funktion man hittar. **S** att döpa och visa dem.

## 4. Var Sonix är före — proportionen måste sägas

- **Pluginvärd:** VST2/VST3/CLAP i egen sandboxad process. Soundtrap har VST3/AU-**effekter** bara i
  en desktop-beta, utan VST-instrument och utan plugins på masterkanalen
  `[belagt: support.soundtrap.com/hc/en-us/articles/38552795853202]`.
- **Riktig sampler:** `SampleZone` med grundton, tangent- och anslagsintervall, plus SoundFont-import.
- **Export:** sju format, EBU R128, true peak, dither — mot Soundtraps fyra.
- **Tagningar och komp:** take-lanes i sångstudion, som Soundtrap inte har alls.
- **Offline och ägande:** ett projekt är en fil; ingen prenumeration, ingen molntvång.
- **Djup i MIDI:** MIDI-fil in/ut, formelstyrning, capture MIDI, notlängder, legato/chop/join.

## 5. Vad vi inte ska ta

| Soundtrap-funktion | Skäl att avstå |
| :--- | :--- |
| Videochatt inuti DAW:n | Säljs som produktens kärna; kostar en hel nätstack och hör inte till en native-desktop-DAW. |
| Molntvång och plan-gating | Motsatsen till varför Sonix finns. Lås aldrig en ratt bakom en nivå. |
| Webbläsar- och mobilversion | `ROADMAP.md:89`: annan plattform, annan motor. Att port:a egui till wasm är ett eget projekt. |
| "Enkelhet" som förminskning | Soundtraps begränsning (tre automationsparametrar, ingen MPE, ingen notskrift) är inte en förebild. |

## 6. Vad jag inte kunde belägga

- Om varje Soundtrap-effekt går att automatisera (supporten visar bara att *en* effektkontroll kan
  visas i automationslanen).
- Soundtraps faktiska latens, projektgränser och antal samtidiga kollaboratörer — inga siffror i källorna.
- Att alla fem sång­effekterna kan sättas **på ett sångspår** hos oss: racket finns, routingen är inte mätt.
- MPE och MIDI-CC-lanes hos Soundtrap: frånvaro belagd, inte bekräftad.
