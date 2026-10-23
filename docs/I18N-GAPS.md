# I18N-GAPS — hårdkodade strängar och saknade översättningar

Läge: `master` @ 65af62b. Allt här är enligt `scripts/i18n_audit.py`, som kan köras om
från repo-roten:

```bash
python3 scripts/i18n_audit.py
```

Svenska är nyckelspråket: `t("…")`/`self.tr("…")`/`tstatus!("…")` slår upp **samma
svenska sträng** i `en`/`da`/`no`/`de`/`es`/`fr`. En literal som aldrig skickas
genom någon av dem blir svensk i alla sju språk.

## Sammanfattning

- **40 hårdkodade literaler i UI-anrop** hittades (10 i de tre
  efterfrågade formerna `ui.label(`/`ui.button(`/`.on_hover_text(`, 30 i övriga
  widget- och hjälparanrop). 12 av dem är språklig text, resten symboler
  och enheter.
- **2 nycklar saknas i någon tabell** (6 tabellrader totalt): en nyckel finns bara i
  `en`, en finns i alla utom `en`. Se [Saknade översättningar](#saknade-översättningar).
- **0 nycklar** som skickas till `t()`/`tr()` saknar tabellpost helt (1584 nycklar
  kontrollerade).

## Metod och gränser

`scripts/i18n_audit.py` gör tre saker:

1. Plockar ut `"…" => "…"` ur varje `fn tr_<språk>` i `src/i18n.rs` (både
   enradsform, `=> { "…" }`-formen och poster utan avslutande komma) och jämför
   nyckelmängderna mellan de sex tabellerna.
2. Scannar `src/**/*.rs` efter nycklar som skickas till `t(`/`tr(`/`translate(` och
   rapporterar dem som ingen tabell har.
3. Scannar `src/ui/**/*.rs` efter strängliteraler som är **direkta argument** till
   widget- och hjälparanrop (`label`, `button`, `on_hover_text`, `small_button`,
   `collapsing`, `hint_text`, `Slider::text`, …) utan att gå genom `t()`.
   Kommentarer, `#[cfg(test)]`-moduler och `tests.rs` hoppas över.

Gränser, värda att känna till innan listan styckas:

- Scannen ser **bara literaler som är argument till ett anrop**. En literal som
  lagras i ett fält eller en tabell och renderas på annat håll fångas inte — se
  [Familjer som scannen inte ser](#familjer-som-scannen-inte-ser).
- Många sådana datatabeller är redan korrekta: `add_track_modal.rs` (`t(tmpl.title)`),
  `fx_rack_modal.rs` (`t(pedal.description)`), manualens `chapters`/`f_keys`/
  `shortcuts`, `tuner_modal.rs` (`t(*label)`), `piano_roll.rs` (`t(field.label())`)
  och färgnamnen i `arranger.rs` (`t(c_name)`) går genom `t()` vid rendering och är
  **inte** luckor.
- Loggrader (`eprintln!`/`println!`), `write!`-token i playlist-exporten,
  `contains()`-heuristiker och felsträngar som hamnar i `Result::Err` (t.ex.
  `midi.rs`, `plugins.rs`) räknas inte som UI-text och finns inte i listan.
- En literal utan `å ä ö` kan inte skiljas från engelsk text maskinellt. Listan
  nedan är därför uppdelad på anropsform, inte på språk.

## Hårdkodade strängar — de tre efterfrågade anropsformerna

`ui.label("`, `ui.button("`, `.on_hover_text("` med literal. Inga av dem bär
meningstext: allt är symboler. (Notera att `ui.label`/`ui.button` i praktiken alltid
anropar `crate::i18n::t(...)` i den här koden.)

| Fil:rad | Anrop | Sträng | Bedömning |
|---|---|---|---|
| `src/ui/app/mixer.rs:1674` | `label` | `–` | symbol |
| `src/ui/app/mixer.rs:1677` | `label` | `–` | symbol |
| `src/ui/app/mixer.rs:1682` | `button` | `🗑` | symbol |
| `src/ui/app/mixer.rs:2649` | `button` | `🗑` | symbol |
| `src/ui/app/stretch.rs:546` | `button` | `🗑` | symbol |
| `src/ui/macros_modal.rs:447` | `button` | `⬆` | symbol |
| `src/ui/macros_modal.rs:450` | `button` | `⬇` | symbol |
| `src/ui/macros_modal.rs:453` | `button` | `✖` | symbol |
| `src/ui/macros_modal.rs:523` | `button` | `📁` | symbol |
| `src/ui/macros_modal.rs:533` | `button` | `📁` | symbol |

## Hårdkodade strängar — övriga widget- och hjälparanrop

Samma familj, samma problem: literal som går rakt in i egui. Bedömningen är min
läsning av strängen — `text` behöver översättas, `enhet/förkortning` och `symbol`
gör det troligen inte.

| Fil:rad | Anrop | Sträng | Bedömning |
|---|---|---|---|
| `src/ui/app/arranger.rs:685` | `hint_text` | `Select a spot on the timeline and ask me to add a part...` | text |
| `src/ui/app/browser.rs:537` | `collapsing` | `🎹 SYNTH PRESETS` | text |
| `src/ui/app/browser.rs:579` | `collapsing` | `🎵 GROOVE INSPIRATION LOOPS` | text |
| `src/ui/app/browser.rs:604` | `collapsing` | `🌊 SOUND FX & RISERS` | text |
| `src/ui/app/frame.rs:1431` | `small_button` | `✕` | symbol |
| `src/ui/app/midi.rs:492` | `hint_text` | `/sökväg/till/fil.mid` | exempel (sökväg) |
| `src/ui/app/mixer.rs:1530` | `text` | `st` | enhet/förkortning |
| `src/ui/app/mixer.rs:1535` | `text` | `ct` | enhet/förkortning |
| `src/ui/app/modals.rs:102` | `text` | `Ton` | text |
| `src/ui/app/modals.rs:452` | `collapsing` | `📋 OSC Adress-mappningar` | text |
| `src/ui/app/modals.rs:1406` | `small_button` | `✖` | symbol |
| `src/ui/app/modals.rs:1933` | `small_button` | `✖` | symbol |
| `src/ui/app/modals.rs:2046` | `small_button` | `✖` | symbol |
| `src/ui/app/modals.rs:2109` | `small_button` | `▲` | symbol |
| `src/ui/app/modals.rs:2112` | `small_button` | `▼` | symbol |
| `src/ui/app/modals.rs:2115` | `small_button` | `✖` | symbol |
| `src/ui/app/modals.rs:2526` | `text` | `L / R` | enhet/förkortning |
| `src/ui/app/modals.rs:2541` | `text` | `Threshold (dB)` | enhet/förkortning |
| `src/ui/app/modals.rs:2542` | `text` | `Ratio` | enhet/förkortning |
| `src/ui/app/modals.rs:2635` | `text` | `Gain (dB)` | enhet/förkortning |
| `src/ui/app/modals.rs:2636` | `text` | `Frekvens` | text |
| `src/ui/app/modals.rs:2648` | `text` | `Gain (dB)` | enhet/förkortning |
| `src/ui/app/modals.rs:2649` | `text` | `Frekvens` | text |
| `src/ui/app/modals.rs:2650` | `text` | `Q (Bredd)` | text |
| `src/ui/app/modals.rs:2657` | `text` | `Gain (dB)` | enhet/förkortning |
| `src/ui/app/modals.rs:2658` | `text` | `Frekvens` | text |
| `src/ui/app/modals.rs:2727` | `text` | `Sekunder` | text |
| `src/ui/app/modals.rs:2760` | `text` | `Sekunder` | text |
| `src/ui/app/piano_roll.rs:604` | `small_button` | `✖` | symbol |
| `src/ui/tuner_modal.rs:306` | `text` | `Cents` | enhet/förkortning |

## Familjer som scannen inte ser

Verifierade för hand; de syns inte i listan ovanför eftersom literalen inte är ett
direkt argument till ett widget-anrop.

| Var | Vad | Omfattning |
|---|---|---|
| `src/ui/app/frame.rs:16-31` | `view_name()` returnerar 13 vynamn som literaler; de renderas råa i `tstatus!` (`frame.rs:524`) och i `RichText::new(view_name(panel))` (`frame.rs:1422`). Nycklarna finns redan i tabellerna — det är uppslaget som fattas (`t(view_name(v))`). | 13 strängar |
| `src/ui/app/transport.rs:854,855,991` | Filvalsdialogens filter och titel går direkt till `rfd`: `"Stämning (.ascl)"`, `"Välj stämning"`, `"Välj groovemall"`. | 3 strängar |
| `src/ui/vocal_studio_view.rs:224` | `"🔴 SPELAR IN LIVE FRÅN MIKROFON..."` — den pausade grenen är `t()`-ad, den aktiva är det inte. | 1 sträng |
| `rotary_knob(...)`, 48 anrop | Rattetiketten är en literal: 34 i `mixer.rs`, 5 i `vocal_studio_view.rs`, 3 i `frame.rs`, 4 i `fx_rack_modal.rs`, 2 i `piano_roll.rs`. Språkliga exempel: `mixer.rs:2882 "TID (ms)"`, `:2898 "RUM"`, `:2899 "DÄMP"`, `:2915 "TON"`, `vocal_studio_view.rs:1127 "FORMANT"`, `piano_roll.rs:1401 "FILLS"`. Övriga är förkortningar (`VOL`, `PAN`, `MIX`, `GAIN`, `FREQ`) och kan lämnas. | 48 anrop |
| `src/ui/app/export.rs:492,738` | Kommentarstext som skrivs in i exporterade WAV-filer: `"Fruset spår (mellansteg i Sonix)"`, `"Bakat spår (mellansteg i Sonix)"`. Syns i andra program, inte i Sonix UI. | 2 strängar |

## Saknade översättningar

Resultat från `python3 scripts/i18n_audit.py` (tabellerna har 2149 nycklar var):

| Språk | Nycklar som saknas |
|---|---|
| en | 1 |
| da | 1 |
| no | 1 |
| de | 1 |
| es | 1 |
| fr | 1 |

Två distinkta nycklar, med olika hål:

| Nyckel | Saknas i | Används av |
|---|---|---|
| `⚠️ '{}' är en binär groove-fil — 103 av Abletons 219 mallar är det. Välj en XML-mall` | **en** | Ingen `t()`-referens i `src/` — posten finns bara i de fem andra tabellerna (troligen död nyckel). |
| `⚠️ '{}' är binär men innehåller ingen notlista att läsa` | **da, no, de, es, fr** | `src/ui/app/transport.rs:1010` via `tstatus!` |

Den andra används via `tstatus!`, den första har ingen referens alls i `src/`. Det är
därför de befintliga självtesterna i `src/i18n.rs:13440-13536` inte fångar dem: de
scannar bara `t(` och `tr(`. `tstatus!`-nycklar kontrolleras inte av något test i dag.

Nycklar som skickas till `t()`/`tr()`: **0 saknas i någon tabell** (1584 nycklar
kontrollerade).

## Förslag på styckning

1. `view_name()` i `frame.rs` — ett `t()`-omslag, 13 strängar, nycklarna finns.
2. Den saknade `tstatus!`-översättningen i `transport.rs:1010` — fem tabeller.
3. `Transport`-dialogerna (`transport.rs:854,855,991`) — tre strängar.
4. `modals.rs`-raderna med språklig text (`Ton`, `Frekvens`, `Sekunder`,
   `Q (Bredd)`, `Threshold (dB)`, `Ratio`) — en fil.
5. `browser.rs:537,579,604` + `modals.rs:452` — sektionsrubriker.
6. `vocal_studio_view.rs:224` och `export.rs:492,738` — en sträng vardera.
7. `rotary_knob`-etiketterna med svenska ord (`TID`, `RUM`, `DÄMP`, `TON`,
   `FORMANT`, `FILLS`) — resten är förkortningar och kan lämnas.
8. Symbolknapparna (`✕`, `🗑`, `⬆`, `⬇`, `📁`, `–`) — behöver ingen översättning,
   kan stängas som "avsiktligt".
