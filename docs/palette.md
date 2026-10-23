# Paletten

> **29 färgroller, noll literala färger i gränssnittskoden.** Reglerna nedan är inte smak:
> de är det som gör att en färg betyder samma sak på varje skärm.

## Reglerna

1. **En accent har en betydelse.** Orange = aktivt/markerat/primär handling. Grön = ok/redo.
   Röd = fel/rec. Cyan = klipp och vald rad. Lila = bas och modulering. Gul = varning.
   Behöver något en sjunde betydelse är det en ny *roll*, inte en ny kulör.
2. **Spårfärger är innehåll, inte krom.** De tio `TRACK_*`-tonerna är medvetet dämpade (lägre
   mättnad och ljushet) så att de aldrig konkurrerar med en accent. En klipfärg får inte se ut
   som en aktiverad knapp.
3. **Tre ytor, två textnivåer.** `BG_DEEP < BG < PANEL < PANEL_ALT` är skalan; text finns i
   `TEXT` och `TEXT_MUTED`. En tredje, mörkare textnivå fanns (`TEXT_DIM`, 2,5–3,2:1) och togs
   bort — den gick inte att läsa på små etiketter.
4. **Ingen `Color32::from_rgb` utanför `ui/theme.rs`.** Undantaget är färger som räknas fram
   (gradienter, per-kanal-nyanser) — de står i koden med en kommentar om varför.

## Rollerna

| Roll | Hex | Kontrast mot `PANEL` | mot `BG` |
| :--- | :--- | ---: | ---: |
| `AMBER` | `#e6a050` | 7.4:1 | 8.3:1 |
| `BG` | `#121418` | 1.1:1 | 1.0:1 |
| `BG_DEEP` | `#0c0f14` | 1.2:1 | 1.0:1 |
| `BORDER` | `#2d3441` | 1.3:1 | 1.5:1 |
| `BORDER_SOFT` | `#3c4b5f` | 1.8:1 | 2.1:1 |
| `CABLE` | `#a08cff` | 6.0:1 | 6.8:1 |
| `CYAN` | `#00d7ff` | 9.5:1 | 10.7:1 |
| `GREEN` | `#2ecc71` | 7.8:1 | 8.8:1 |
| `GRID` | `#202630` | 1.1:1 | 1.2:1 |
| `HEADER` | `#1a1e24` | 1.0:1 | 1.1:1 |
| `ORANGE` | `#ff8c19` | 7.0:1 | 7.9:1 |
| `PANEL` | `#1c2028` | 1.0:1 | 1.1:1 |
| `PANEL_ALT` | `#22262e` | 1.1:1 | 1.2:1 |
| `PURPLE` | `#ac61f5` | 4.5:1 | 5.1:1 |
| `RED` | `#e74c3c` | 4.3:1 | 4.8:1 |
| `TEXT` | `#f0f4fa` | 14.8:1 | 16.7:1 |
| `TEXT_MUTED` | `#8c96a8` | 5.5:1 | 6.2:1 |
| `TRACK_AMBER` | `#b08c34` | 5.2:1 | 5.8:1 |
| `TRACK_BLUE` | `#5880c4` | 4.1:1 | 4.7:1 |
| `TRACK_CYAN` | `#2c84b4` | 3.9:1 | 4.4:1 |
| `TRACK_GREEN` | `#2e9468` | 4.3:1 | 4.9:1 |
| `TRACK_MAGENTA` | `#9454b0` | 3.2:1 | 3.6:1 |
| `TRACK_ORANGE` | `#c87028` | 4.5:1 | 5.1:1 |
| `TRACK_ROSE` | `#a85470` | 3.2:1 | 3.7:1 |
| `TRACK_STEEL` | `#6c788e` | 3.7:1 | 4.1:1 |
| `TRACK_TEAL` | `#348c96` | 4.1:1 | 4.7:1 |
| `TRACK_VIOLET` | `#7c68be` | 3.6:1 | 4.0:1 |
| `YELLOW` | `#ffd700` | 11.6:1 | 13.1:1 |

Textnivåerna: `TEXT` 14,8:1 mot `PANEL` (WCAG AAA), `TEXT_MUTED` 5,5:1 (AA). Accenterna ligger
4,3–11,6:1, alltså läsbara som text men aldrig tänkta som textfärg.

## Vad som hände med de gamla namnen

| Före | Efter |
| :--- | :--- |
| `FL_ORANGE`, `LCD_ORANGE` | `ORANGE` |
| `FL_GREEN` / `FL_RED` / `FL_YELLOW` | `GREEN` / `RED` / `YELLOW` |
| `FL_CYAN`, `LCD_TEXT` | `CYAN` |
| `FL_PURPLE` | `PURPLE` |
| `TEXT_BRIGHT` | `TEXT` |
| `BG_DARK` / `LCD_BG` / `LCD_BG_DEEP` | `BG` / `BG_DEEP` |
| `PANEL_BG` / `CHANNEL_BG` / `HEADER_BG` | `PANEL` / `PANEL_ALT` / `HEADER` |
| `BORDER_DARK` | `BORDER` |
| `TEXT_DIM` | borttagen (föll under WCAG AA) → `TEXT_MUTED` |
| 10 spårfärger med samma ljusstyrka som accenterna | 10 dämpade `TRACK_*` |

`FL_`-prefixet var ett arv från FL Studio-jämförelsen, inte en betydelse — det är borta.

## Mätt, inte tyckt

- **626 literala `from_rgb` i 362 unika färger** → 0 literaler (46 kvar, alla med uträknade
  komponenter: gradienter och per-kanal-nyanser).
- **ΔE (CIE76) per anrop:** median 8,6, medel 10,6, max 42. Det är en *synlig* förändring —
  därför ligger före/efter-bilder i `screenshots/` och siffran står här i stället för att gömmas.
- **Pixlar som ändrades mellan före och efter:** median 33 % av bildytan per vy.
- **Kontrast:** alla textroller ≥ 4,5:1 på alla fyra ytor (WCAG AA). `TEXT_DIM` togs bort för att
  den inte klarade det.

## Känt byte

Dämpningen köpte sammanhållning och betalade med differentiering: tio spårtoner på lägre
intensitet är svårare att skilja snabbt än tio skrikiga. Om det visar sig i arbetet är ratten
`TRACK_*`-tonernas ljushet — inte att ge spåren accentfärger igen.
