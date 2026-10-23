# Patternlängd — så ska flertaktsrutor samexistera med arrangemangsblocken

*Skrivet 2026-10-23 inför arbetet med 16 → 64 steg. Läget i koden just nu: **steg 1 och 2 är
gjorda**, steg 3 (block med längd) är kvar.*

## Nuläget

- Ett `Pattern` är **en takt**: 16 steg (`[bool; MAX_STEPS]` sedan steg 1, men bara 0–15 används).
- Arrangemanget är `clips: [Option<usize>; 32]` — **en pattern per takt**. Takten i ledet är
  både tid och plats: `clips[n] = Some(p)` betyder "pattern p spelar i takt n".
- MIDI-import delar en flertaktsfras i **en pattern per takt**, döpta `… ⋅ takt 2` osv, och
  lägger dem i `clips[n], clips[n+1], …`. Det är därför en 4-taktsmelodi idag blir fyra rutor.

## Målet

Samma som FL: ett pattern är **1–4 takter** (16–64 steg), och en ruta i arrangemanget är ett
**block med längd** — en 4-taktsruta är *en* ruta som spelar 64 steg, inte fyra rutor.

## Steg 1 — klart (0.9.19)

Rutnätet rymmer 64 steg och sparformatet tål båda storlekarna:

- `audio::tempo::MAX_STEPS = 64`, använd av `Pattern`, `ChannelStrip`, `piano_roll_sel` och
  exportens `RenderSpec`.
- Sparade rader är `Vec<bool>` / `Vec<u8>` i stället för fasta arrayer, och läses in med
  `row_bools`/`row_notes`/`grid_rows` som fyller ut korta rader med nollor. Ett projekt från
  16-stegstiden läses alltså oförändrat (prov: `a_sixteen_step_pattern_is_padded_when_it_is_read`).
- **Ingen beteendeförändring**: alla slingor går fortfarande `0..16`, och exporten renderar en takt.

## Steg 2 — patternlängd (klart, 0.9.20)

1. `Pattern.bars: usize` (1–4, `#[serde(default = "one_bar")]`).
2. Pianorullen ritar `bars * 16` steg och får en väljare **Takter 1–4**. Att korta ett pattern
   som har noter utanför den nya längden frågar först — annars kastas musik tyst.
3. Pattern-läget (uppspelning utan arrangemang) loopar `bars * 16` i stället för 16.
4. Exporten renderar `bars * 16` steg per pattern; `RenderSpec::num_bars` räknas om därefter.
5. `place_notes_into_patterns` får skriva inom hela rutnätet när målet är ett flertaktsmönster,
   men behåller dagens beteende (en pattern per takt) när målet är entakt.

Klart när: en tvåtaktsruta spelar 32 steg i pattern-läget, sparas och läses lika långt.

**Gjort:** `Pattern.bars` (1–4, `#[serde(default)]`), väljaren **Takter** i pianorullens
verktygsrad, rutnätet och velocity-fältet ritar `bars × 16` steg (båda riktningarna rullar),
pattern-läget loopar `bars × 16`, och alla rutnätsverktyg (transponera, arpeggio, humanize,
regler, urval) täcker hela 64-stegsrutnätet. Stegvolymerna breddades till 64 och sparas som
lista, så äldre filer med 16 värden läses oförändrat.

**Kvar till steg 3:** arrangemanget spelar bara mönstrets första takt (ett block = en takt).
Exporten räknar fortfarande hela projektet i takter om 16 steg, vilket är rätt så länge
arrangemanget är entaktsblock.

## Steg 3 — block med längd

`clips` byter form:

```rust
struct Clip { pattern: usize, start_bar: u32, bars: u32 }
// PlaylistTrack::clips: Vec<Clip>
```

- **Uppslag:** givet en takt `n` hittas det block vars `start_bar <= n < start_bar + bars`;
  steget som spelas är `(n - start_bar) * 16 + current_step`.
- **Uppspelningen** måste alltså kunna spela ett **stegfönster** i ett pattern, inte alltid från
  steg 0. Det är den enda riktiga ändringen i motorn: `current_step` blir global takt-relsativ,
  och sekvenseraren får `(pattern, offset_steg)`.
- **UI:t** ritar blockets bredd som `bars` takter och låter högerkanten trimmas. Överlapp
  förbjuds när ett block släpps (samma regel som idag, men över `bars` i stället för en takt).
- **Migrering:** en gammal `[Option<usize>; 32]` blir `Vec<Clip>` med `bars: 1` per post —
  exakt dagens beteende. Prov ska läsa en riktig gammal projektfil.
- **Import** fortsätter skapa en pattern per takt (det är fortfarande rätt för importerade
  fraser); inget i importen behöver ändras i det här steget.

## Risker och ordning

1. Exporten först (steg 2), eftersom den är lättast att mäta: en 2-taktsruta ska ge dubbelt så
   många sampel.
2. Därefter playback + UI (steg 3). De två hänger ihop: ett block utan stegfönster spelar bara
   första takten, och ett stegfönster utan block kan inte nås från arrangemanget.
3. Sist: överlappsregler och trimning i arrangemanget.

Inget av stegen får ändra en **entaktsruta**: den ska låta och se ut exakt som idag.
