# Designgranskning av Sonix — 2026-11-03

**Underlag:** 39 skärmdumpar från `f744e10` (`--capture-screenshots`, 1920×1200, mörkt tema,
headless X). Tio av dem granskades visuellt; designsystemet räknades dessutom ur källkoden, för en
synmodell ser glyfer, inte tokens.

Metod: `/design-review` (rubrik + allvarlighetsgrader), två oberoende granskare — en visuell, en som
mäter i källan — och varje siffra nedan är kontrollerad av mig i trädet.

## Fynd

### 🔴 1. 197+ ikoner ritas som ◻

De fyra vanligaste ikonerna i appen finns **inte i någon av de fyra typsnitt** epaint levererar:

| Glyf | Förekomster | I fonten? |
|---|---|---|
| 🎛 (FX-rack) | 57 | nej |
| 🎚 (mixer/nivå) | 41 | nej |
| 🥁 (trummor) | 41 | nej |
| 🎙 (sång) | 35 | nej |
| 🤖 🧩 🧪 🧹 🪟 ✕ ⧉ | 6–13 var | nej |

Mätt genom att parsa cmap i `Ubuntu-Light.ttf` (1 194 tecken), `NotoEmoji-Regular.ttf` (887),
`emoji-icon-font.ttf` (654) och `Hack-Regular.ttf` (1 548) — ingen av dem täcker U+1F399–1F9F9.
Epaint ritar då ersättningstecknet `◻`. Kontroll: 🎵 🎲 ⚙ ✂ 🔊 🎹 **finns**, så det är inte
emoji i allmänhet som saknas — det är just de här kodpunkterna.

**Fix:** en täckande emoji-TTF i `assets/fonts/`, registrerad efter `Ubuntu-Light` i
`FontDefinitions`. En rad löser alla 46 glyfer.

### 🔴 2. Registermätaren går inte att läsa

`register_eq` ritar **sex** etiketter (SUB · BAS · L-MID · MID · DISK · TOPP ur
`spectrum::REGISTERS`) i en widget som är **128×22 px** (`frame.rs:1625`). Slotbredd
≈ (128−4)/6 ≈ 20,7 px; "L-MID" i 10,5 px är ~27 px. Dessutom är `label_h = 8.0`
(`widgets.rs:706`) mindre än fontens 10,5 px, så texten ligger i staplarna. Det syns som
`SUB BAS…MID…ISK…OPP` i stills.

**Fix:** `label_h = 12.0` och tre etiketter (SUB / MID / TOPP) — eller widgeten till 176 px.

### 🔴 3. Hjälpguiden kollapsar till en remsa

`modals.rs:102–104` sätter `ui.set_width(ui.available_width())` och
`ui.set_height(ui.available_height())` **inuti** en `ui.horizontal`. Där är tillgängligt mått
obundet horisontellt och noll höjd, så manualen blir skärmbred och några tiotal px hög. Det är
appens enda dokumentation (F1).

**Fix:** `allocate_ui(Vec2::new(660.0, 520.0), …)` kring innehållet.

### 🔴 4. 21 av 68 accentfyllda kontroller har oläsbar etikett

Räknat: 68 kontroller med mättad fyllning, 21 med etikett under 4,5:1. Tre orsaker, alla mätta:
nära-vit text på fyllning (grön **1,96:1**, orange 2,06, gul 1,44, cyan 1,79, lila 2,87, röd 2,86 —
min beräkning, samma storleksordning som granskarens 1,90–2,92), fyra ställen *utan* färg (egui:s
grå 180), och `readable_on()` vars tröskel 0,45 ger vit text där svart hade gett 6,5–10:1.

**Fix:** `Theme::label_on(fill)` som väljer `bg` eller `text` vid luminans 0,1791 — då hamnar varje
accent på 5,7–13:1.

### 🟠 5. Typografin har i praktiken två steg

833 storleksanrop fördelade på fem tokens: `FONT_S` **587**, `FONT_XS` 176, `FONT_M` 39, `FONT_L`
25, `FONT_XL` 6. Steget S→M är 1,16× (under 1,2 där två steg läses som olika), och 92 % av texten
ligger på två storlekar. `theme.rs:596–600` skriver dessutom om talen som literaler i stället för att
läsa `FONT_*`, så en ändrad token flyttar inte all text som ärver `TextStyle::Body`.

**Fix:** 11,5 / 14 / 17,5 / 22 (förhållanden 1,22 · 1,25 · 1,26) och `FontId::proportional(Theme::FONT_S)`
i de fem raderna.

### 🟠 6. Accenterna betyder inte längre det paletten säger

`theme.rs:24–27` dokumenterar en semantik (orange = aktivt, grön = ok, gul = varning, lila =
modulering). I praktiken är grön primär CTA 22 gånger, gul är primärknapp i AI-vyn, och orange är
passiv rubrikfärg i varje modal. När varningsfärgen är huvudåtgärden slutar färgen vägleda blicken.

**Fix:** en accentfylld knapp per vy; rubriker i `text`/`text_muted`.

### 🟠 7. Modalerna delar ingen ram

**42** handbyggda `egui::Window` i 23 filer. Nio upprepar titeln som rubrik inne i fönstret, 21 gör
det inte. Stängknappen sitter i fyra positioner. Ingen scrim någonstans — ytan bakom ser klickbar ut.

**Fix:** `modal_frame(ctx, title, |ui| …)` med titelrad, X höger, primär åtgärd höger i foten, och
`from_black_alpha(140)` före fönstret.

### 🟡 8. Radier och luft är literaler

Rundningar: minst 10 värden (0 · 0,5 · 1 · 1,5 · 2 · 3 · 4 · 6 · 8 · 9); 2, 3 och 4 är alla
"standard" och ser nästan likadana ut. `add_space`: **196** anrop i `src/ui/app` över **9** värden
(2 · 3 · 4 · 6 · 8 · 10 · 12 · 16 · 20) — 3 och 10 bryter 2-px-griden utan att markera något.

**Fix:** `RADIUS_S/M/L` = 2/4/6 + `PILL`, och `GAP_XS/S/M/L` som `apply_style` också läser.

### 🟡 9. i18n: 240 franska strängar är ≥1,5× den svenska de mättes för

2622 nycklar × 7 tabeller. Snitt mot svenska: tyska 1,127×, spanska 1,172×, franska 1,196×; över
1,5×: 108 / 174 / **240**. Skyddet finns på få ställen (`truncate()` används 7 gånger i hela
`src/ui`) medan 30 textknappar sitter på fast `min_size`.

**Fix:** `min_size` ur `galley.size().x` på de 30, och ett prov som faller när en tabell växer >1,6×
för nycklar ≤24 tecken.

## Styrkor

- **Färgdisciplinen är ovanligt hög**: 29 semantiska tokens, 24 `from_rgb`-konstruktioner varav 22 är
  härledda ur en token — 2 hittar på en färg.
- **Kontrasten är mätt i koden**, inte påstådd: `contrast_tests` prövar text, muted och åtta accenter
  mot tre ytor i båda lägena. Mörkt läge klarar allt (lägsta accent 5,07:1).
- **Mätning före ritning där det är svårast**: spårnamnet mäts mot sin plats i stället för att
  klippas, flikraden räknar bredd ur `layout_no_wrap`.
- **Ythierarkin håller**: `bg_deep / bg / panel / header` ger lager utan skuggor, och modaler lyfts
  korrekt mot arrangören.

## Mest hävstång

**Emoji-typsnittet** (fynd 1) — 46 trasiga ikoner blir 0, och det är det enda fyndet som syns som
*fel* i stället för som *svag*. Därefter **en accent per vy**: en regel, inte en omritning.

---

# Runda 2 (2026-11-04, `9383d59`)

39 färska skärmdumpar; tio granskade. Runda 1:s fynd var **ofixade** — den här rundan skulle
bekräfta eller avvisa dem, och leta där runda 1 inte tittade.

## Bekräftat, med mått

| Fynd | Mått i den här rundan |
|---|---|
| Registermätaren är oläsbar | sex etiketter i `Vec2::new(128.0, 22.0)` → slot **20,7 px**, "L-MID" ~26 px vid 10,5 px. Raden läses som **ett** ord |
| F1-manualen kollapsar | fönstret mäter 1919 × **~210 px**, kapitelmenyn (230 px) syns inte alls, brödtexten är **en** rad som börjar mitt i ett ord och klipps i båda ändar |
| Typskalans två steg | 834 storleksanrop, **587** på 12,5 px + **177** på 10,5 px = 92 %; steget 12,5 → 14,5 är **1,16** |

## Nya fynd

- 🟠 **"DJ FILTER & CRUSH XY PAD" skriver bara filter** (`channel_rack.rs:1418-1419`: X → cutoff,
  Y → resonans). Ingen crush. Ingen axel är märkt i ytan.
- 🟠 **"Mina inspelade ljud" har ingen tom-state** (`vocal_studio_view.rs:1222`): en tom svart yta
  där resultatet ska visas.
- 🟠 **Verktygsskärmarna är toppankrade**: sista innehållsrad i ett 1200 px-fönster lämnar
  **48–75 %** tomt (05: 75 %, 11: 70 %, 13: 62 %, 09: 48 %).
- 🟠 **Stäeditorn: tre av sex "knappar" i huvudraden är bara text** — primärhandlingen
  (ISOLERA STÄMMA) är den enda som inte ser ut som en knapp.
- 🟡 **Vocoder-reglagen visar bandindex, inte nivå** (`fx_rack_modal.rs:1145-1170`: `.text("{i}")`
  med `.show_value(false)`), så "0" läses som värdet noll.
- 🟡 **Stem-separatorn visar projektets tempo som om det vore filens** (`stem_view.rs:55`), och
  med noll decimaler där transporten visar en.

## Tillämpbara patchar (räknade, inte gissade)

| Fynd | Ändringar | Risk |
|---|---|---|
| **Ikonfonten** | 1 ny funktion + 2 anropsrader + 3 typsnittsfiler | **`set_fonts` måste ligga i `CreationContext`, inte i `update`** — `apply_style` körs varje bildruta. Repot har ingen `FontDefinitions` alls i `src/`, så det är en ny funktion, inte en rad. Tre filer behövs (Symbols2 + Math + NotoEmoji, monokrom — NotoColorEmoji är CBDT och kan inte rastreras av ab_glyph). U+FF0B finns i ingen font → byt mot ASCII `+` i `fx_mixer_rack.rs:310` |
| **Accentkontrast** | 31 sites i 13 filer + tröskeln i `readable_on` (0,45 → 0,1791) | Alla "aktiva" knappar byter från vit till svart text i mörkt läge (2,1 → 9,0:1). Rätt, men det syns |
| **Typskalans fyra steg** | 44 sites i 20 filer | `FONT_S` +12 % träffar 587 sites; flikraden och den 210 px spårkolumnen kan klippas. `arranger.rs:989` blir död kod (mikspårets `FONT_M`-gren) |
| **Radier & luft** | 197 radier + 334 `add_space` i 34 filer, 95 byter värde | 3,0 → 4,0 (35 sites) gör chips och småknappar märkbart rundare — den enda ändringen som syns på en skärmdump |

**Rekommenderad ordning:** ikonfonten (oberoende, störst synlig vinst, rör inga tal) → accent­kontrasten
(en rad + mekaniska edits, mätbar) → radier/luft (mekaniskt, jämför skärmdumpar) → typskalans fyra
beslut (kräver en titt på flikraden).
