# AI-tjänstens API

Ett litet HTTP-gränssnitt på **localhost**, så ett plugin, ett skript eller en extern editor kan
driva Sonix-assistenten utan att länka in sig i appens interna typer.

Tjänsten startar med appen och lyssnar på `127.0.0.1:4827` (byt port med `SONIX_AI_API_PORT`,
`0` stänger av den).

## Auth

Varje anrop kräver `Authorization: Bearer <token>`. Token skapas första gången appen startar och
ligger i:

```
~/.config/sonix/ai_service_token      (rättigheter 0600)
```

```bash
TOKEN=$(cat ~/.config/sonix/ai_service_token)
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:4827/v1/health
```

Utan token: `401`. Tjänsten lyssnar **aldrig** på annat än loopback.

## Rutter

### `GET /v1/health`

```json
{"ok": true}
```

### `GET /v1/tools`

Verktygsschemat, samma JSON som modellen får (`ai_tools::tool_schema`). Ett plugin kan läsa det för
att veta vad som går att göra — och för att bygga sin egen meny.

### `POST /v1/execute`

Kör verktygsanrop. Kroppen är samma form som modellen svarar med:

```bash
curl -s -X POST http://127.0.0.1:4827/v1/execute \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"calls":[
        {"tool":"set_tempo","args":{"bpm":128}},
        {"tool":"write_drum_pattern","args":{"pattern":0,"style":"house","bars":2}}
      ]}'
```

Svar `202`:

```json
{"queued":2,"note":"executed by the app on its UI thread"}
```

Anropen läggs i en kö och körs i appens egen tråd — de rör pattern och kanaler som ägs där.
Ogiltig JSON eller ett okänt verktyg ger `400` och ingenting körs.

### `GET /v1/last`

Eftersom `execute` är asynkront måste svaret gå att hämta. Kvittona ligger i den ordning appen
utförde dem, och **listan töms när den läses**:

```bash
curl -s -H "Authorization: Bearer $TOKEN" http://127.0.0.1:4827/v1/last
# ["tempo 132 BPM", "trumkomp 'house' i 2 takter"]
```

Ett avvisat anrop syns också här, med orsaken — t.ex. `"trumkomp: kanalen 'Kick' har redan steg"`.

## Verktyg

| Verktyg | Argument | Vad som händer |
| :--- | :--- | :--- |
| `set_tempo` | `bpm` 40–240 | Sätter projektets tempo |
| `write_drum_pattern` | `style` (`house`/`hiphop`/`rock`), `bars` 1–4 | Skriver kick/virvel/hi-hat på kanal 1–3, GM-noter 36/38/42. **Vägrar** om någon av kanalerna redan har steg |
| `mute_track` | `track`, `muted` | Tystar/slår på ett spår i arrangemanget |
| `plan_sections` | `sections: [{name, bars}]` | Tolkar och validerar en sektionsplan; skriver inget än |
| `set_key` | — | Inte byggt än; svaret säger det |
| `set_channel_instrument` | `channel`, `instrument` | Byter syntljud mot en av `SYNTH_PRESETS` — namnet måste finnas, AI:n får inte hitta på ett ljud |
| `write_notes` | `channel`, `notes: [{step, pitch, len, vel}]` | Skapar en **klippa på tidslinjen** med egna noter (se `AI-CLIPS.md`), inte rutor i rutnätet |
| `write_pattern` | `lanes: [{channel, steps, subdivision, note}]` | Skriver en stämma per kanal; **vägrar** om kanalen har steg, och om slagen inte ligger på den upplösning som efterfrågades |
| `load_drum_kit` | `genre` | Väljer samplingar ur biblioteket för trumrollerna och lägger dem på trumkanalerna |
| `analyze_mix` | — | **Mäter**: LUFS och topp per spår (första 32 takterna, spår för spår via offline-render) plus sektionerna. Skriver inget |
| `groove_report` | `track` | **Mäter**: spårets slag mot sextondelsrutnätet — median och värsta avvikelse i ms. Skriver inget |
| `master_to_target` | `lufs` | Sätter närmaste loudness-preset, renderar och normaliserar som exporten gör, och rapporterar uppmätt LUFS före/efter samt topp |
| `chop_channel` | `channel`, `spectral` | Slår upp kanalens ljud vid sina transients (`onset::detect_slice_map`), skriver slicarna och en slice per sextondel i ett **tomt** mönster |
| `rename_track` | `track`, `name` | Byter namn på ett spår (högst 40 tecken) |
| `write_script` | `source` | Kör Rhai mot den **valda tagningen** — mot en kopia, så ett fel lämnar tagningen orörd (`audio::script`) |

### Mät eller gissa

De sex sista verktygen finns därför att modellen inte kan höra. Siffrorna kommer från appen:
`audio::loudness` (LUFS, true peak), `audio::onset` (slag) och `render_project_offline` (samma
rendering som exporten använder, ett spår i taget via `solo_track`). Modellen får läsa mätvärdena
och föreslå — den får aldrig påstå en nivå den inte kan mäta.

Mätningarna spelar de **första 32 takterna**; en nivå som skiljer sig mellan vers och refräng syns
inte i dem (`shortcut` i `ai_actions.rs`).

## Vad som inte finns (och varför)

- **Ingen promptrutt.** Modellanropet går via appens AI-inställningar; att exponera det över HTTP
  vore att låta ett skript bränna API-kvoten utan att användaren ser det. Bygg hellre ett anrop i
  appen och skicka `execute` efteråt.
- **Ingen läsrutt för projektet.** Tjänsten kan bara göra det verktygen kan. `analyze_mix` och
  `groove_report` ger *mätvärden* (nivåer, slag), inte arrangemanget som data — en riktig läsrutt
  behöver först en strypt API-nyckel per plugin.
- **Ingen fjärråtkomst.** Ingen bind-adress, ingen tunnel, ingen CORS. Öppna inte porten.
