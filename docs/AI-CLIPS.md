# AI-klippor: design för nästa pass

Skrivet efter en mätning i koden 2026-10-26. Syftet är att nästa pass ska börja här i
stället för att mäta om samma saker.

## Utgångsläget, mätt

| Vad | Var | Sanning |
|---|---|---|
| Stegrutnät | `ui/app/state.rs:132` `ChannelStrip` | `steps: [bool; MAX_STEPS]` + `notes: [u8; MAX_STEPS]` — **ingen** array för anslag eller notlängd |
| AI:ns melodi-/basväg | `ui/app/ai_actions.rs` `ai_write_notes` | skriver `ch.steps[i]=true; ch.notes[i]=pitch` — alltså in i rutnätet, inte i en klippa |
| AI:ns trumväg | samma fil, `ai_write_lane` | samma rutnät |
| Takes (MIDI) | `midi_take.rs:178` `Take { notes: Vec<TakeNote> }` | `pos: f32`, `velocity`, `length_steps`, `probability`, `articulation` — **har allt MIDI behöver** |
| `.mid` I/O | `audio/smf.rs` | skriv + läs utan externa beroenden; `save_midi` finns i skript-API:t |
| Dragbara objekt | `AudioRegion` (`start_bar`, `length_bars`) | regioner i `playlist_tracks[].regions` går att flytta med musen |
| Uppspelning | `ui/app/transport.rs` `advance_sequencer` | schemalägger **per steg**, inte per `pos` → trioler spelar fel tills det ändras |

**Slutsatsen som förklarar "den satt fast":** AI:n skriver i mönstrets globala rutnät. Det
finns inget objekt på tidslinjen att greppa, och därför inget att spara eller flytta.

## Målet

En prompt ska ge en **klippa** som

1. håller den upplösning som efterfrågades (klart: `subdivision` i `ai_tools.rs`, 0.9.164),
2. går att **dra** i arrangemanget,
3. går att **lägga till i låten** vid spelhuvudet på valt spår,
4. går att **spara som Sample** (WAV via `exporter`, eller `.mid` via `smf` + preset),
5. behåller noter, anslag och längder när kanalens ljud byts ut.

## Stegen

1. **`PatternClip` som objekt.** Ny typ i `ui/app/state.rs`:
   `struct PatternClip { id, pattern: usize, lane: usize, start_bar: f32, length_bars: f32, name: String }`,
   lagd i `playlist_tracks[].clips: Vec<PatternClip>` (nytt fält, `#[serde(default)]` så gamla
   projekt öppnas).
2. **AI skriver noter, inte steg.** `ai_write_notes` → bygger `TakeNote` (pos ur `step * 0.25`
   takter, `velocity`/`length_steps` ur `vel`/`len`) i stället för `steps`/`notes`. Trummor får
   vara kvar i rutnätet — de är rutnät av naturen — men klippan pekar på mönstret.
3. **Klippan ritas och dras** i arrangemanget med samma mönster som `AudioRegion`: grepp,
   `start_bar` uppdateras, längd i takter. Flytt får inte röra noterna.
4. **Lägg till i låten:** knapp i AI-vyn → klippa vid spelhuvudet.
5. **Spara som Sample:** rendera klippans kanal offline (`exporter::render_project_offline`
   med bara den kanalen hörd) till WAV i `~/.config/sonix/samples/`, plus `.mid` bredvid via
   `smf::write_midi`. Statusraden ska säga båda sökvägarna.
6. **Trioler:** `advance_sequencer` schemalägger på `pos` (f32) i stället för stegindex; först
   då får `write_notes` ta bråktal i `step`. Prov: spelad position vs notens `pos` inom 5 ms.

## Acceptanskriterier

- "Ge mig ett trapbeat i 1/8" ger slag på varannan sextondel — och ett anrop som bryter mot
  upplösningen avvisas (klart).
- Klippan går att dra till takt 9 utan att noterna ändras.
- Klippan går att spara som WAV **och** `.mid`, och den sparade filen låter som klippan.
- Triolprov: en not på `pos 0.667` spelas 0,667 steg in i takten (inom 5 ms).

## Fällor att komma ihåg

- `ui.set_width`/`allocate_ui_with_layout` i egui — se `references/gui-style-and-branding.md`.
- Ny synlig sträng = i18n-nyckel i sex språk, och `packaging/check-docs-languages.sh` ska vara grön.
- Rör aldrig masterns volym från AI-vägen; skriv aldrig över en kanal med noter utan att säga det.
