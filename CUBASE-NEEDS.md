# Vad Cubase 15 lär oss att vi behöver

Allt underlag är utplockat ur installationsfilen, inte läst på webben. Siffrorna är
räknade; påståenden om vår egen kod är **mätta med grep**, inte ihågkomna.

## 1. Vad som plockades ut

| Fil | Innehåll |
|---|---|
| `docs/cubase-files.md` | 2 171 filposter med Cubases egna sökvägar |
| `docs/cubase-command-surface.md` | 1 148 kommandon (Eucon) + 21 527 strängar i fem fack |
| `docs/cubase-presets.md` | **alla 610 förval** med orden som står inuti varje regel |
| `docs/cubase-devices.md` | 27 enhetsnamn, 36 plugin-namn ur `Cubase Plug-in Set.vst3`, parameterlistor |
| `docs/cubase-templates.md` | 38 projektmallar |
| `docs/cubase-content.md` | 5 `.vstsound` (831 + 120 + … namn), `AudioPlugs.srf`, mallarnas spårnamn |

Plus `CUBASE-INVENTORY.md` med komponenter, gap-matris och det som inte är mätt.

## 2. Vad förvalen faktiskt gör

608 förvalsnamn, räknade på första ordet och på målet:

| Verb | Antal | | Mål | Antal |
|---|---|---|---|---|
| delete | 29 | | note | 105 |
| select | 24 | | select | 86 |
| toggle | 19 | | track | 72 |
| open | 16 | | event | 45 |
| shift | 14 | | velocity | 44 |
| transpose | 11 | | part | 36 |
| nudge | 10 | | transpose | 31 |
| quantize | 8 | | delete | 29 |
| copy | 8 | | channel | 20 |
| double | 7 | | position | 18 |

**Läsningen:** Cubases förval är till övervägande del **massoperationer på noter och event** —
välj enligt ett villkor, ändra eller ta bort. Inte ljud, inte mixning: *urval och bulk-edit*.
Det är också vad de 247 Logical Editor-förvalen är till för.

Vi hade (mätt med grep i `src/`): kvantisering ✅, sväng ✅, humanisering ✅, groove ✅ —
men **ingen enda bulk-operation** utöver det: ingen transponering, inget anslagsverktyg,
ingen radering enligt villkor, ingen urvalsmodell.

## 3. Vad vi tar, i ordning — med minsta ingrepp

| # | Vad | Storlek | Minsta ingrepp |
|---|---|---|---|
| ~~1~~ | ~~Transponera tagningen~~ | **klart** | `Take::transpose` + fyra knappar |
| ~~2~~ | ~~Anslaget på tagningen~~ | **klart** | `Take::scale_velocity` + två knappar |
| 3 | **Urval i pianorullen** (villkor: anslag under X, ton = Y, position inom Z) | M | `Take` får ett `selected: bool` per not; operationerna ovan går på urvalet när det finns, annars allt |
| 4 | **Radera enligt villkor** | S | följer av 3: `retain(keep)` + "Ta bort valda" |
| 5 | **Logical Editor-lite**: en rad regler (fält, villkor, värde, åtgärd) | M | 247 förval hos dem är facit för vilka fält som behövs: `lePosition`, `leType`, `leProperty`, `leValue` |
| 6 | **Arranger Track** — sektioner som kan flyttas om | M | markörer finns redan (`shift_song_markers`); sektioner är en lista av intervall med namn |
| 7 | **Chord Track** — ett spår andra följer | M | vi har ackordgenerator (`chord_generator_modal.rs`), inte ett spår |
| 8 | **Control Room** — monitor, cue, talkback | M | 50 av deras 1 148 kommandon; vi har master ut |
| 9 | **Projektmallar** | S | 38 hos dem, 0 hos oss; en mall är ett sparat projekt |
| 10 | **Stämmseparering med modell** | L | `neural_separator.rs` finns, ingen modell skeppas |

## 4. Byggt nu

`Take::transpose` och `Take::scale_velocity`, med knappar i pianorullens tagsektion
(±1/±12 halvtoner, ±10 % anslag) och ångra. Två nya prov:
`transpose_and_velocity_edit_the_take_in_place` bevisar att noter kläms till MIDI-omfånget
i stället för att tappas, och att noll faktor stänger av anslaget.

Det var det minsta som stängde den största uppmätta luckan (§2) utan att röra
schemaläggning, export eller tagningens format.

## 5. Vad vi **inte** tar, och varför

| Saknas hos oss | Varför inte |
|---|---|
| Notskrift (ScoringEngine, 103 MB) | XL. Kräver layout motor, symbolteckensnitt och sidbrytning; ingen av våra användare har frågat efter noter. |
| VariAudio (pitch per ton) | L. Kräver tonsegmentering först; vår pitch/stretch är global. |
| VST Connect (fjärrinspelning) | XL. Nätverksprodukt med egen server; vi har redan filbaserat utbyte. |
| Eucon/kontrollerytor | M. 27 enhetsnamn och 1 148 kommandon är en hel branschstandard; tangentbord + panelgenvägar räcker. |
| MediaBay | M. Vi har filbläddrare; en databas över samplingar löser inget vi saknar i dag. |
| Dolby Atmos/ADM, head tracking, REX, ProRes | Branschspecifika moduler (20 MB, 9 MB, 2 MB). Ingen efterfrågan. |
| FFmpeg/SoXR som vi inte äger | Vi har egen avkodning och SoXR-klassad resampling; att byta är risk utan vinst. |
| `.vstsound`-biblioteken | Steinbergs eget format, krypterat innehåll (licenstexter i filerna). Kan inte läsas lagligt och behövs inte. |

## 6. Vad som inte är mätt

- **Hur deras stämmseparering låter.** ONNX-modellen ligger i en DLL vi inte kan köra.
- **`.vstsound`-innehållet.** Namnen i `docs/cubase-content.md` är strängar ur filhuvudet —
  parameternamn som `treble`, `drive`, `presence`, `speakertype`. Själva ljuden är packade.
- **Gränssnittet i drift.** Ingen skärm i sessionen; allt är ur filerna.
- **Antalet användare som behöver notskrift.** Min bedömning, inte ett mätvärde.
