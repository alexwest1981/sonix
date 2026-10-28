# Teman

Ett tema är **en JSON-fil med färger**. Filen behöver bara skriva det den ändrar — resten är
standardpaletten — så ett tema som bara byter bakgrund och accent är tre rader.

```json
{
  "mode": "dark",
  "bg": "#101418",
  "panel": "#1a1f26",
  "orange": "#ff8c19"
}
```

Färger får skrivas som **hex** (`#rrggbb` eller `#rrggbbaa`) eller som array `[r, g, b, a]`.
Hex läses vid inläsning; när appen sparar ett tema skrivs arrayer, eftersom det är fältets form.

`mode` är `"dark"` (standard) eller `"light"`. Det avgör vilken standardpalett filen läggs ovanpå:
ett tema som bara säger `{"mode": "light"}` ger den ljusa paletten orörd, och ett tema som byter
två färger i ljust läge behöver inte räkna om resten.

## Nycklarna

`sonix --theme-palette` skriver hela listan med standardvärden (och `--theme-palette light` den
ljustema varianten). Det är också svaret på "vilka nycklar finns?" — listan nedan är densamma.

| Nyckel | Styr |
|---|---|
| `mode` | `dark` eller `light`; vilken standardpalett filen bygger på |
| `bg_deep` | djupaste bakgrunden (arrangörens botten, modala skuggor) |
| `bg` | fönsterbakgrund |
| `header` | verktygsrader och rubriker |
| `panel` | paneler, kort, rutor |
| `panel_alt` | varannan rad i listor |
| `grid` | rutnät och taktlinjer |
| `border` | ramar |
| `border_soft` | avdelare |
| `text` | brödtext |
| `text_muted` | sekundär text, etiketter |
| `orange` | appens accent (primär) |
| `green` | spelar, OK, aktiv |
| `red` | inspelning, fel, borttaget |
| `cyan` | markering, länk |
| `purple` | sekundär accent, valt objekt |
| `yellow` | varning |
| `amber` | mjuk accent, mätare |
| `cable` | kablar i patchern |
| `scrim` | dimma bakom modaler |
| `track_cyan` … `track_steel` | de tio spårfärgerna (`track_cyan`, `track_magenta`, `track_orange`, `track_green`, `track_teal`, `track_blue`, `track_rose`, `track_amber`, `track_violet`, `track_steel`) |

## Skapa ett tema

**I appen**: ställ in färgerna du vill ha, öppna temamenyn och välj att spara — då skrivs bara de
nycklar som avviker mot standardpaletten, så filen blir kort och läsbar.

**För hand**: lägg en `.json` i `~/.config/sonix/themes/` och välj den i temamenyn. Appen läser om
filen när den ändras, så du kan redigera medan fönstret står öppet.

Börja från listan:

```bash
sonix --theme-palette > mitt-tema.json
```

## Vad ett tema bör klara

- **Kontrast**: `text` mot `bg` och mot `panel` ska gå att läsa; `text_muted` ska synas men inte
  konkurrera. Ett tema som bara byter accent brukar klara sig; ett som byter bakgrund måste också
  se över `text`, `border` och `grid`.
- **Spårfärgerna** används för klipp och vågformer. Tio färger som ligger för nära varandra gör
  spåren omöjliga att skilja åt.
- **`red`/`green`** betyder inspelning respektive spelar — byt nyans, inte betydelse.

## Dela

Registret är `github.com/alexwest1981/sonix-themes`: `index.json` listar teman med filnamn och
sha256, och appen hämtar tema-filen från samma mapp.

```json
{
  "version": 1,
  "themes": [
    { "name": "Nord", "file": "nord.json", "sha256": "5bcd4f13…" }
  ]
}
```

Ett tema installeras bara om hashen stämmer — en fil som inte är den registret pekar på sparas
aldrig. Så här räknar du hashen:

```bash
sha256sum nord.json
```

Lägg filen i repot, lägg raden i `index.json`, öppna en pull request.

## Prov

`cargo test --bin sonix theme` prövar inläsningen: att en fil bara behöver skriva det den ändrar,
att hex läses som array, att `mode: light` ger den ljusa paletten, och att **den här filen räknar
upp varje nyckel i `Palette`** (ett prov läser den, så en ny färg utan dokumentation faller).
