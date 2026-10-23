# UX-hantverk: Sonix mot Soundtrap

> Mätt 2026-10-23 på `master` (`bd44ded`). **Vår sida är mätt, deras är observerad.**
> Våra 30 skärmdumpar är tagna ur den riktiga appen (`sonix --capture-screenshots`, 1920×1080,
> demoläge) och ligger i [`screenshots/`](screenshots/). Soundtraps studio kräver inloggning, så
> deras kolumn är läst ur deras egna publicerade studiobilder — siffrorna där är **uppskattningar
> ur en bild, inte mätningar**. Allt om vår kod är räknat; kommandona står i §6.

## 1. Svaret i en rad

Av 13 hantverksaxlar är **1 i nivå, 6 delvis, 6 inte**. Men de sex som faller är inte sex problem:
de är **tre lager** — typografi, färg, ikoner — plus en hierarki som faller ut ur dem.
Det som känns hemsnickrat är alltså inte mängden kod (108 782 rader i `src/`), utan att varje
skärm får bestämma sina egna storlekar, färger och glyfer.

## 2. Axel för axel

| Axel | Sonix (mätt) | Soundtrap (observerat) | Dom |
| :--- | :--- | :--- | :---: |
| Typografisk skala | 22 storlekar, halvsteg (9,5 · 10,5 · 11,5 · 12,5 · 13,5) | ~3 storlekar, en familj, två vikter | ✗ |
| Färgsystem | 16 tokens i `theme.rs`, men **642 literala `from_rgb` → 362 unika färger i 22 nyanskluster**; 6 accenter som betyder olika saker | 1 accent (violett) + 2 innehållsfärger (röd, ockra), ~10 tokens | ✗ |
| Ikonografi | **88 distinkta emoji, 1 350 förekomster** i `src/ui`; emoji ligger även i i18n-nycklarna | en linjeikonfamilj, ~1,5 px, 16–20 px stora | ✗ |
| Komponentfamilj | 12 publika widgetfunktioner mot 235 `pub fn` i `src/ui`; 287 råa `ui.button(`; 5–6 kontrollhöjder i samma rad | en knappfamilj i två storlekar (28–32 px) | ✗ |
| Primär handling | orange är markering, aktivt steg, primär-CTA **och** avvisa-knapp; i återställningsdialogen är "Fortsätt utan att återställa" den enda fyllda knappen (`modals.rs:952`) | en fylld CTA per yta (Export), allt annat neutralt | ✗ |
| Lager och fokus | 29 `egui::Window::new`, **0 scrim/dim** — modaler flyter över en fullt upplyst bakgrund | modaler/paneler med egen elevation, bakgrunden dämpas | ✗ |
| Avståndsrytm | 9 `add_space`-värden, 4/6/8 dominerar; 4 px bas finns | 4/8-bas | ◐ |
| Panelgrid och linjering | spårrad 46 px; ytterkanter linjerar, inre innehåll glider (master saknar EQ-rad) | spårrader i linje med lanes, fast 3-zonslayout | ◐ |
| Hover-hjälp | 199 `on_hover_text` mot 287 knappar och 21 vyer | verktygstips på det mesta | ◐ |
| Tangentbord | 50 unika tangenter, 47 `key_pressed`-ställen, **ingen shortcut-tabell** (0 `consume_shortcut`/`KeyboardShortcut`) | okänt (webb), men dokumenterade genvägar | ◐ |
| Språk | svensk bas, men engelska etiketter kvar (EXPORT SONG, STEMS, SOUND BROWSER, SWING); 7 språk kompletta | engelska + lokalisering av marknadssidan | ◐ |
| Första start | demodata + 19 spårmallar + hjälpguide (F1); återställningsdialogen dyker upp i demoläget | låtmallar direkt i starten, guider | ◐ |
| Mörkt tema och grundpalett | `theme.rs` med 16 namngivna färger, mörka paneler genomgående | mörkt tema, ~6 gråsteg | ✅ |

**Räknat:** 1 ✅ · 6 ◐ · 6 ✗.

## 3. De tre lagren som ger intrycket

### 3.1 Typografin har ingen skala
22 storlekar, och de ligger på halvsteg (`10.5`, `11.5`, `12.5`) — det är ögat som justerar per skärm,
inte ett system som väljer. En halv pixel är inte en hierarki; det är 22 beslut som var för sig såg
rimliga ut.
**Fix:** fem steg (`10 · 11,5 · 13 · 16 · 20`) i `Theme` som `Type::SMALL/NORMAL/…`, mekanisk
ersättning av 22 värden. **Mät efter:** `rg -o 'size\(([0-9.]+)\)' src -r '$1' | sort -u | wc -l` → 5.

### 3.2 Färgen är inte ett system
16 namngivna färger i `theme.rs`, men **642 literala `Color32::from_rgb` ger 362 unika färger**,
varav 137 är varianter av samma blå nyans (210°). Sex accenter (cyan, orange, grön, gul, lila, röd)
används både som markering, status, spårfärg och primär handling — samma gul betyder "vald rad" i
en panel och "aktivt steg" i nästa.
**Fix:** färg bara via `Theme::` (noll literaler utanför `theme.rs`), och accentrollerna delas upp:
*markering* / *innehåll* / *status* / *primär*. **Mät efter:**
`rg -c 'Color32::from_rgb' src -g '*.rs' | awk -F: '{s+=$2} END{print s}'` → 0 utanför `theme.rs`.

### 3.3 Ikonerna är emoji
**88 distinkta emoji i 1 350 UI-strängar** — 🎹 48 gånger, 🎵 44, ⚡ 44, 🥁 40 … De ritas av egui:s
emoji-typsnitt, alltså med olika optisk storlek och vikt per glyf, och de sitter **inuti
i18n-nycklarna** (`"🛟 Osparat arbete hittades"`). Det är därför raderna i menyer och knappar inte
linjerar optiskt: det finns ingen ikonfamilj, bara text som råkar börja med en bild.
**Fix:** en `icon()`-hjälpare + ett eget glyfset, och emoji **strippas i renderingslagret** — inte i
nycklarna, för då måste 2 185 strängar × 7 språk göras om. **Mät efter:**
`rg -o '"[^"]*"' src/ui | rg -o '\p{Emoji_Presentation}' | sort -u | wc -l` → 0.

## 4. Fyra fynd som är UX-fel, inte smak

1. **Den loudaste knappen i återställningsdialogen är den som kastar arbetet.** "Fortsätt utan att
   återställa" är den enda fyllda (orange) knappen (`ui/app/modals.rs:952`); "Återställ" är en platt
   standardknapp. Den primära handlingen ska vara den som räddar arbetet.
2. **Modaler dämpar inte bakgrunden.** 29 `egui::Window::new` och **noll** scrim/dim i `src/ui`;
   tidslinjens cyan/magenta klipp konkurrerar med dialogens text (syns i `screenshots/01`).
3. **Språket är blandat.** "EXPORT SONG", "BATCH EXPORT", "STEMS", "SOUND BROWSER", "SWING",
   "Guide" står kvar på engelska i en svensk menyrad. Antingen översätts de eller så väljs engelska
   överallt — nu är det två system i samma rad.
4. **362 färger betyder att ingen kan säga vad en färg betyder.** När en skärm ska ändras finns ingen
   sanning att ändra i; man måste leta literaler.

## 5. Det som redan håller

- **Mörkt tema med namngivna panelfärger** — grunden finns, det är användningen som saknar disciplin.
- **4 px-bas i avstånden** (4/6/8 dominerar) — rytmen är nästan där.
- **7 kompletta språk** och 2 185 källsträngar: lokaliseringen är byggd, det är etiketterna som är kvar.
- **Skärmdumpsharnessen** (30 mål, `--capture-screenshots`) — de flesta projekt *kan inte* fotografera
  sin egen UI; det gör varje påstående i det här dokumentet kontrollerbart.
- **Informationstätheten** är en DAW-kvalitet, inte ett fel: Soundtrap är glesare för att de gör mindre.

## 6. Så mäts det om

```bash
rg -o 'size\(([0-9.]+)\)' src -r '$1' -g '*.rs' | sort -u | wc -l          # typografi: 22 → mål 5
rg --no-filename -c 'Color32::from_rgb' src -g '*.rs' | awk '{s+=$1} END{print s}'   # färg: 642 → mål 0 utanför theme.rs
rg -o '"[^"]*"' src/ui -g '*.rs' | rg -o '\p{Emoji_Presentation}' | sort -u | wc -l  # ikoner: 88 → mål 0
rg --no-filename -c 'egui::Window::new' src/ui -g '*.rs' | awk '{s+=$1} END{print s}' # modaler: 29
rg --no-filename -c 'on_hover_text' src -g '*.rs' | awk '{s+=$1} END{print s}'       # hjälp: 199
```

## 7. Vad jag inte mätte

- **Soundtraps siffror är ur bilder.** En av bilderna är ett foto av en laptop, så deras pixelmått är
  normaliserade uppskattningar. Riktningen är säker, decimalerna är det inte.
- **Inget klick-test.** Jag har inte kört appen och mätt tiden till "första låt" — den mätningen
  (steg, klick, sekunder) är nästa naturliga prov och säger mer om UX än någon färgräkning.
- **Kontrast är inte räknat per yta.** `Theme::TEXT_MUTED` används 248 gånger, ofta på 10 px text —
  misstanken om för låg kontrast är inte verifierad mot WCAG-tal här.
- **Emoji-räkningen är textbaserad.** Hur de faktiskt ritas (egui:s typsnitt, fallback-glyfer) är
  bedömt ur skärmdumparna, inte ur teckentabeller.
