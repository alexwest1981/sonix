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
| `set_key`, `set_channel_instrument` | — | I schemat men **saknar utförare**; svaret säger det |

## Vad som inte finns (och varför)

- **Ingen promptrutt.** Modellanropet går via appens AI-inställningar; att exponera det över HTTP
  vore att låta ett skript bränna API-kvoten utan att användaren ser det. Bygg hellre ett anrop i
  appen och skicka `execute` efteråt.
- **Ingen läsning av projektet.** Tjänsten kan bara göra det verktygen kan. Ett plugin som vill
  läsa arrangemanget får vänta på en läsrutt — den behöver först en strypt API-nyckel per plugin.
- **Ingen fjärråtkomst.** Ingen bind-adress, ingen tunnel, ingen CORS. Öppna inte porten.
