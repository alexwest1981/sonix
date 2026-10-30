# UI-förslag (Vercel/v0) — vad som är taget, vad som återstår

Förslaget är en Next.js-mockup (`sonix-ui-redesign.zip`): en tät arrangörsyta med mörk botten,
orange primäraccent, cyan tidskod, 3–10 px rundade hörn och Inter. Det är en **vy**, inte en port:
Sonix ritar i egui, så det som kan flyttas rakt över är färgerna — resten är lay-out och typografi.

## Taget: `daw-bold.json`

Färgerna är lästa ur mockupens `.daw-*`-klasser och lagda på Sonix palett
(`~/.config/sonix/themes/daw-bold.json`). Filen är valbar i temamenyn och kräver ingen kodändring.

| Mockup | Nyckel | Värde |
|---|---|---|
| `.daw-shell`, `.arrangement` | `bg_deep` | `#0d1015` |
| `.daw-topbar`, `.daw-toolbar` | `bg`, `header` | `#111419` |
| `.daw-tabs`, `.daw-clip` | `panel` | `#191d23` |
| `.sound-browser` | `panel_alt` | `#1c2129` |
| `.track-lane`-rutnät | `grid` | `#242b35` |
| `.daw-clip`-ram, `.daw-toolbar` | `border` | `#323842` |
| avdelare | `border_soft` | `#2d343d` |
| brödtext | `text` | `#dce1e8` |
| etiketter, `clip-meta` | `text_muted` | `#8b95a3` |
| `.browser-pill`, `.stop-button`, aktiv flik | `orange` | `#ff941b` |
| `.bpm b`, `.timecode`, `.ruler` | `cyan` | `#19d4ed` |
| `.green-tool` | `green` | `#20d477` |
| `.record` | `red` | `#ed4d47` |
| `.browser-row:nth-child(2n)` | `purple` | `#a85bcb` |
| `.master-track` | `amber` | `#ff8b18` |
| spår 1–6 i `page.tsx` | `track_*` (tio) | se filen |

**Uppmätt kontrast** (WCAG, mot mockupens egna ytor): text/bg 14,0 · text/panel 12,9 ·
text_muted/bg 6,1 · text_muted/panel 5,6 · orange/bg 8,4 · cyan/bg 10,3 · green/bg 9,4 ·
red/bg 5,1. Allt över AA (4,5), alltså läsbart utan att mockupen ljusas upp.

## Kvar: det som inte är färg

| Del av förslaget | Var det sitter i Sonix | Storlek |
|---|---|---|
| Rundade hörn 3–10 px | 197 `Rounding::same(...)` i `src/ui/` + `apply_style` | ~200 ställen |
| Inter som typsnitt | 6 ställen (`FontDefinitions`) | litet, men kräver att Inter buntas |
| 38 px flikrad, orange aktiv flik | `src/ui/app/frame.rs`, flikwidgeten | litet |
| 123 px spårhöjd, 228 px spårhuvud | arrangörens radlayout | **klart**: raden var redan 120 px, huvudet är nu 228 px |
| 282 px ljudwebbläsare, `browser-pill` | `browser.rs` | medel |
| Master-spår inramat i orange | mixern/arrangören | litet |
| Rundade klipp med `⌘ 00:01.94`-metadata | klippritningen | medel |

## Taget: flikraden

Flikraden (`src/ui/widgets.rs::tabs`) följer nu förslaget: den valda fliken är **fylld med
`orange`** och bär mörk text (`bg_deep`), 5 px rundade överkanter, och accentlinjen är borta —
fyllningen gör fliken till en flik. Provet `the_tab_strip_paints_one_tab_per_label_and_marks_the_active_one`
räknar shapes i en huvudlös egui-ruta och kräver **exakt en** orange fylld flik och **noll**
accentlinjer, så ändringen är mätt och inte bara påstådd.

Spårhuvudets vänsterkant är redan förslagets 4 px färgstapel (`color_bar` i `arranger.rs`) — den
delen behövde ingen ändring.

Ordningen jag skulle ta resten i: **spårhuvudets höjd → klipp-metadata → rundning svep sist**.
Rundningen sist därför att den rör flest filer och minst betydelse per rad; flikraden och
spårhuvudet är det man ser först.

Ett globalt `Rounding` i `apply_style` ger mycket av "rundheten" på ett ställe, men de 197
uttalade ställena vinner över standardvärdet — så det blir halvverkat utan svepet. Det ska göras
med ögonen på skärmen, inte blint.
