# Skript i Sonix

Två språk, **samma funktioner**: Rhai (standard) och Lua 5.4. Ett skript som fungerar i det ena
fungerar i det andra — samma namn, samma betydelse, samma fel. Välj Lua genom att låta **första
raden** vara `--!lua`, eller genom att spara filen som `.lua`.

```
# projekt.rhai
set_bpm(140);
set_track_name(0, "Kick");
set_track_volume(0, 0.8);
add_note(0, 0, 0, 36, 1.0);
add_note(0, 0, 4, 38, 0.8);
log("noter: " + note_count(0, 0));
```

```lua
--!lua
set_bpm(140)
set_track_name(0, "Kick")
add_note(0, 0, 0, 36, 1.0)
log("noter: " .. note_count(0, 0))
```

## Funktionerna

Indextal är **nollbaserade**. Ett index utanför intervallet är ett fel med namn på vad som gick fel
(`track 99 is outside 0..2`), aldrig en tyst no-op.

### Transport och stegrutnät

| Funktion | Betydelse |
| :--- | :--- |
| `bpm()` / `set_bpm(v)` | tempo (kläms till det appen tillåter) |
| `pattern_count()` | antal mönster |
| `steps()` | antal steg i rutnätet |
| `channel_count()`, `channel_name(i)` | kanaler i mönstret |
| `step(ch, i)` / `set_step(ch, i, on)` | läser/sätter en ruta |
| `clear_channel(ch)` | tömmer en kanal |
| `log(s)` | skriver till skriptloggen (`print` i Lua går samma väg) |

### Spår

| Funktion | Betydelse |
| :--- | :--- |
| `track_count()` | antal spår |
| `track_name(i)` / `set_track_name(i, s)` | namn |
| `track_volume(i)` / `set_track_volume(i, v)` | volym, 0–1 |
| `track_mute(i)` / `set_track_mute(i, on)` | mute |

### Klipp och noter

| Funktion | Betydelse |
| :--- | :--- |
| `clip_count(track)` | antal klipp på spåret (1 per stegrutnät idag) |
| `clip_len(track, clip)` / `set_clip_len(track, clip, steps)` | längd i steg, 1–64 |
| `note_count(track, clip)` | antal noter |
| `note_at(track, clip, idx)` | `{step, note, velocity}` |
| `add_note(track, clip, step, note, velocity)` | lägger till, sorterat på steg; velocity 0–1 |
| `clear_notes(track, clip)` | tömmer klippet |

### Stickprov på fil

| Funktion | Betydelse |
| :--- | :--- |
| `sample_seconds(path)`, `sample_rate(path)`, `sample_frames(path)` | läser en ljudfil utan att ladda den i projektet |

## Vad som skrivs tillbaka

`Ctx` är enda sanningen: appen fyller den, skriptet ändrar den, appen skriver tillbaka. Ett klipp som
skriptet **inte** rört lämnas orört — stegrutnätet skrivs bara över för klipp där `edited` är satt.
Det gör att ett skript som bara läser aldrig kan tömma en kanal av misstag.

## Gränser

- **Velocity sparas inte** i stegrutnätet (det har ingen kolumn för det): `add_note` med velocity
  landar som steg + tonhöjd, och `note_at` läser tillbaka 1.0.
- **Ett klipp per spår** idag — appen har ett stegrutnät per kanal, inte en klipplista. `add_clip`
  finns därför inte än.
- **Lua körs med 5 miljoner instruktioner** som budget: `while true do end` avbryts med
  `script stopped after 5000000 instructions` (mätt: 21 ms). Rhai har motsvarande skydd.
- **Stdlib i Lua är begränsat** till `coroutine`, `table`, `string`, `utf8`, `math` — ingen `io`,
  `os`, `package` eller `debug`, så ett skript kan inte läsa filer eller starta processer.
