# REAPER-arbetsflödet: markera, gruppera, kopiera

Jämförelse av det flöde som REAPER bygger nästan all redigering kring — markering,
gruppering, kopiering — och vad Sonix har i dag. Status är läst ur koden, inte ur minnet:
det som står ✅ finns och har ett prov, ◐ finns delvis, ✗ finns inte.

| Steg i flödet | REAPER | Sonix | Status |
|---|---|---|---|
| Markera ett spår | klick | `click_track` | ✅ |
| Markera flera spår | Ctrl+klick växlar, Shift+klick tar spannet | `clicked_selection` (samma regler) | ✅ 0.9.187 |
| Markera alla spår | Ctrl+A | Ctrl+A, bara när inget textfält äger tangenten | ✅ 0.9.187 |
| Se vad som är markerat | radmarkering i TCP | primärt spår i spårfärg, övriga i cyan ram | ✅ 0.9.187 |
| Markera en region | klick | `click_region` | ✅ |
| Markera flera regioner | Ctrl/Shift+klick, rammarkering (högerdrag) | Ctrl/Shift+klick, spann inom samma spår | ◐ ingen rammarkering |
| Kopiera regioner | Ctrl+C | Ctrl+C, klunga med spår- och taktavstånd | ✅ 0.9.187 |
| Kopiera hela spår | Ctrl+C på spårmarkering | Ctrl+C utan regionmarkering | ✅ 0.9.187 |
| Klistra in | Ctrl+V vid spelhuvudet | Ctrl+V, klungan behåller sin form | ✅ 0.9.187 |
| Klistra in på ursprunglig position | `Paste to original position` | — | ✗ |
| Duplicera | Ctrl+D, Alt+drag | Ctrl+D (region eller spår) | ◐ ingen Alt+drag-kopia |
| Flytta flera regioner samtidigt | alla markerade följer med | `RegionDragState::multi_initial`, samma förskjutning | ✅ 0.9.188 |
| Gruppera spår | mappar (folder) med djup och routing | `TrackGroup`, Fäll ihop/upp, Ctrl+G | ◐ grupp utan routing |
| Gruppera objekt | Ctrl+G, gruppmatris | — | ✗ |
| Ripple-redigering | av/spår/alla | — | ✗ |
| Razor edits | dela/duplicera över markerat område | — | ✗ |
| Tidsmarkering | drag i regeln, loop-punkter | loop_start/loop_end i takter | ◐ ingen tidsmarkering |
| Ångra-historik | fönster med namngivna steg | `push_undo` med namn, ingen lista | ◐ |
| Musmodifierare | fritt mappbara | hårdkodade | ✗ |

## Vad 0.9.187 lade till

- `selected_tracks` och `selected_regions` som mängder, med den primära posten kvar i
  `selected_timeline_track` / `selected_audio_region` så all befintlig kod arbetar vidare.
- `clicked_selection`: Ctrl växlar, Shift tar spannet, vanligt klick börjar om, och
  markeringen kan aldrig bli tom.
- `prune_selection` en gång per bildruta: borttagna spår och regioner försvinner ur
  mängderna i stället för att ligga kvar som spöken.
- `build_region_clipboard` / `plan_region_paste`: klungan bär spåravstånd och taktavstånd,
  och delar som skulle hamna utanför projektet hoppas över i stället för att skapa spår.
- Ctrl+G grupperar markerade spår, Shift+Ctrl+G löser upp.

## Nästa steg, i den ordning de låser upp mest

1. **Rammarkering** i spårbanan (högerdrag) — kräver att `Sense::drag` skiljs från
   regiondragningen, som redan äger vänsterdrag.
2. **Objektgrupper** (`Ctrl+G` på regioner, `group_id` + gemensam förskjutning) — bygger på
   samma förskjutningslogik som multi-flytten.
3. **Mappar med routing** — en grupp som också äger volym/mute för sina medlemmar.
