# JSFX: REAPERs effekter och instrument i Sonix

REAPER 7.83 skeppar **256 JSFX-filer** med sammanlagt ~80 000 rader DSP i
`InstallData/Effects`: kompressorer, EQ, deesser, limiter, förvrängning, MIDI-verktyg,
samplers och nio instrument (bland dem en orgel, en FM-synt och en trummaskin).

Sonix porterar dem **inte en och en**. Vi tolkar dem: `src/audio/jsfx/` är en EEL2-motor
(lexer → parser → virtuell maskin) som kör filerna som de är. Skälet är detsamma som när en
enda `PatternClip` fick ersätta en klippa per stämma — *en motor i stället för 256 portar*.
Den som skriver egen JSFX får den att köra direkt, utan att vi rör koden.

## Vad som mäts

Mätningen är gjord på den här maskinen (i7-8700K, en kärna) mot REAPER 7.83:s egen filkatalog.

| Mätning | Resultat |
|---|---|
| Filer som tolkas och kör `@init` | **256 av 256** |
| Tid att läsa in hela biblioteket (parsa + `@init`) | 5,4 s, varav orgeln `LordOfTheRinks` 5,3 s |
| `sstillwell/1175` (kompressor), 1 s stereo | 263 ms → **3,8× realtid** |
| `loser/3BandSplitter`, 1 s stereo | 109 ms → **9,2× realtid** |
| `Liteon/deesser`, 1 s stereo | 290 ms → **3,4× realtid** |
| `chmaha-fx/Maximizer.jsfx`, 1 s stereo | 656 ms → **1,5× realtid** |
| `TolerableTines` (instrument), 1 s | 143 ms |

Tolkade effekter är alltså 1,5–9× realtid per instans. Det räcker för en handfull effekter på
en kanal, inte för en hel mix med tio. Nästa steg om det blir en gräns är att kompilera EEL2
till bytekod i stället för att vandra AST:n — mätningen ovan är gjord på en ostadig version och
är den siffra som ska slås.

## Instrumenten

Instrumenten spelar inte alltid "bara för att de tolkas". De som är sample-baserade letar efter
ljudfiler som **inte skeppas med REAPER**:

| Instrument | Låter direkt? | Varför |
|---|---|---|
| `TolerableTines` | låter, men fel nivå | ren syntes; utgången mäter ~1947 RMS (skulle vara < 1) — skalningen i dess `fragment`/`memcpy`-väg är inte utredd |
| `MajorGeneralMIDI` | svagt (topp 0,0026) | EAS-syntes, men nivåerna styrs av fil-/bankladdning |
| `DrNine` | nej | laddar `DrNine/DrNine*.flac` via `file_open` — filerna följer inte med REAPER |
| `JamSynth` | nej | samma sak: `file_open` |
| `SynthyMcSynthface` | nej | samma sak |
| `ChamberOrgan` | nej | importerar sin stopptabell (fungerar) men stannar på MIDI-filtret |
| `LordOfTheRinks` | nej | bygger wavetabeller i 5 s, hör sen — inte utredd |

**Det är samma beteende som i REAPER**: utan samplingarna är DrNine tyst där också. Att `file_open`
svarar "ingen fil" är alltså rätt svar, inte en gissning.

Nivåerna mäts utan konstgjord gräns: `Jsfx` skriver igenom det filen skriver och stoppar bara
NaN/Inf. Vad som klipper är värden och DAC, inte tolken — en limiter efter en het synt ska se
samma nivå som i REAPER.

```
TolerableTines.jsfx: 4 slider, not 60, topp 44100.0000 vid prov 125, rms 1947.4839, 44100 prov på 357 ms
```

## Fil-API:t är stubbat

`file_open`, `file_riff`, `file_mem`, `file_avail`, `file_text`, `file_var` och `file_close`
svarar `-1`/`0` och **bokförs i `Jsfx::stubs`**. Det syns i `--jsfx-note`:

```
DrNine.jsfx: 113 slider, not 60, topp 0.0000 …  [stubbar: file_open]
```

Att stubben redovisas är poängen: en effekt som tystnar ska gå att skilja från en effekt som
inte kunde läsas. Samma gäller `@gfx` (UI:t ritas av REAPER, inte av oss) och `@serialize`
(tillståndet sparas i projektet) — de hoppas över vid tolkningen, vilket är rätt: en effekt som
bara ritar ett mätinstrument låter som ingenting.

## Språket

EEL2 är litet men har fler hörn än man tror. De här finns för att REAPER:s egna filer använder dem:

- `^` är **potens**, inte xor; bitvis xor skrivs `xor` eller `~`.
- `x[]` är förkortning för `x[x]`.
- `cond ? ( a; b; )` — en gren får vara ett block, och `:` får saknas (annars 0).
- `while ( kropp...; villkor )` — kroppen ligger *inuti* parentesen, ingen kropp efter.
- `loop (i = 0; 128, kropp)` — initiering före `;`, antal efter.
- `<? ... ?>` är kodgenerator-block (chmaha-fx skriver sin egen källa med `printf`); de hoppas
  över, eftersom de inte gör något med ljudet.
- `/* ... */` kommentarer förekommer även om EEL2 bara har `//`.
- `import Dependencies/X.jsfx-inc` klistras in; den importerade filens filhuvud hoppas över.
- `rmsa.rms.init(...)` — punktade namn är namnrymder: `this.x` i funktionen blir `rmsa.x`.
- `$pi`, `$e`, `$x90`, `$'F'` — konstanter, hex och teckenkod.

Enskilda variabler och arrayer delar ett `f64`-minne, som i REAPER: `slider1..64` bor på plats
0–63, så en fil som skriver `slider1 = 5` beter sig som originalet.

## I effektkedjan

En `.jsfx`-fil laddas där en CLAP/VST-plugin hade laddats: `plugin_host_live::load_processor` känner
igen ändelsen och ger en `PluginProcessor` (backend `"JSFX"`). Filens sliders blir parametrar med
filens eget intervall, `set_parameter` kör `@slider` som en riktig vridning, och `reset` kör om
`@init` — samma som att ta bort och lägga tillbaka effekten.

Mappen `~/.config/sonix/jsfx` ligger i plugin-sökningen och `.jsfx` klassas som formatet *JSFX
(REAPER)*, inte som en VST2 som inte går att ladda. Vill du se REAPERs egen låda i listan, lägg
till dess `InstallData/Effects` som sökväg — eller kopiera hit den:

```bash
cp -r /sökväg/REAPER/InstallData/Effects ~/.config/sonix/jsfx
```

`shortcut: instrument tar ingen MIDI genom PluginProcessor (traitet har ingen notingång), så
JSFX-instrument låter i `--jsfx-note` men är tysta i kedjan tills MIDI-vägen finns.`

## Köra dem

```bash
# vad finns i ett bibliotek, och tolkas det?
sonix --jsfx-list ~/.local/share/sonix/jsfx
sonix --jsfx-list /sökväg/till/REAPER/InstallData/Effects

# kör en effekt över en wav-fil; sliderar kan sättas som index=värde
sonix --jsfx-run effect.jsfx in.wav ut.wav 4=0.35

# spelar en ton genom ett instrument och mäter toppen
sonix --jsfx-note JamSynth.jsfx 60

# visar ett påstående per rad, till för att hitta raden ett fel pekar på
sonix --jsfx-dump effect.jsfx init
```

Egen JSFX läggs i `~/.config/sonix/jsfx` (samma mapp som `--jsfx-list` läser utan argument).

## Prov

`cargo test --bin sonix jsfx` kör enhetstesterna (parser, motor, MIDI, oändliga loopar).
Kör dem mot det riktiga biblioteket genom att peka ut det:

```bash
SONIX_JSFX_DIR=/sökväg/till/REAPER/InstallData/Effects cargo test --bin sonix corpus_tests -- --nocapture
```

De två proven kräver **alla 256 filer** respektive att minst fyra instrument ger ljud från en
nedtryckt tangent. Filerna skeppas inte med Sonix (de är REAPERs), därför är proven opt-in.
