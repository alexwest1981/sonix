# EGM-genomgång 3 av Sonix — gränssnittet

Underlag: fem fristående granskare (färsk kontext, ingen tidigare chatt) läser **ytan** i stället för
koden. Rundan 2 granskade motorn och struktur; den här rundan granskar det man ser, klickar på och
läser — lay-out, hierarki, arbetsflöde, kontrollernas uppriktighet och texten.

Mätsstocken är inte "snyggt". Den är **tre saker**: går det att göra jobbet utan att leta, stämmer
ytan med vad den lovar, och håller den när fönstret blir litet.

---

## 1. Bevisbasen (det som får citeras)

| Bevis | Var | Vad det är |
| :--- | :--- | :--- |
| **Skärmdumpar** | `screenshots/` i repot (38 st, 25 okt) · färsk fångst 2 nov i `/mnt/fastnvme/hermes/cache/scratch/sonix-gui/shot/` | Körda ur appen själv med `--capture-screenshots` under Xvfb, 1920×1200, mörkt tema |
| **Maskinbeskrivning av ytorna** | `gui-evidence.md` utanför repot (`/mnt/fastnvme/hermes/cache/scratch/sonix-gui/`) | Regioner med pixelrutor, varje läsbar etikett ordagrant, påstådda lay-outfel, siffror. **OCR-kvalitet — inte facit**, och **27 av 39 ytor** blev beskrivna: den lokala synmodellen kvotstoppade på de tolv sista dialogerna (Edison, Newtone, Newtime, visualizern, modulationsmatrisen, Suno, vocoder-/bandkurvorna, stämmomenyn m.fl.). Inget av rundans 20 fynd vilar på en rad ur filen — alla står på `fil:rad` |
| **Koden som ritar** | `src/ui/**` (64 filer, 58 556 rader) | Den enda källa som kan avgöra *varför* något ser ut som det gör |
| **Temats sanning** | `src/ui/theme.rs` | 29 semantiska palettnycklar, textskalan, `item_spacing`, `button_padding`, `interact_size`, kontrastprovet |
| **Texten** | `src/i18n.rs` (16 599 rader, 7 språk, svenska är nyckelspråk) + `scripts/i18n_audit.py` | Vad som är översatt och vad som är en literal som aldrig går genom `t()` |

### Uppmätt innan rundan (2 nov, `master` `4d8ac06`, 0.9.237)

| Mått | Värde | Så mättes det |
| :--- | ---: | :--- |
| Ytor (vyer) | **14** | `ViewMode::ALL` i `src/ui/app/state.rs` |
| Flikar ytorna ligger bakom | 4 (SKAPA, ARRANGERA, MIXA, VERKTYG) | `ROADMAP.md` §GUI |
| Minsta fönster | **780 × 540** | `with_min_inner_size` i `src/main.rs` |
| Textskala | **10,5 / 12,5 / 18** px (`Small` / `Body` = `Button` / `Heading`), monospace 12,5 | `style.text_styles`, `theme.rs:591` |
| `item_spacing` / `button_padding` / `interact_size.y` | 8×5 / 9×4 / **21 px** | `theme.rs:565-567` |
| Knappar i ytan | **585** | `ui.button(` + `Button::new(` + `ui.add(egui::Button` |
| Ledtrådar (`on_hover_text`) | **311** | samma sökning, `src/ui/` |
| Hårdkodade literaler i UI-anrop | **83** | `python3 scripts/i18n_audit.py` (dokumentet `I18N-GAPS.md` sa 40 vid `65af62b`) |
| Saknade nycklar i en tabell | **1** (en varningstext, `en`) | samma skript |
| Nycklar i `t()` som ingen tabell har | **0** (av 1 885) | samma skript |
| Prov i trädet | 1 468 `#[test]` | `grep -c` |

**Vad som inte är mätt, och därför inte får påstås:** färgkontrast i pixlarna (mörkt tema finns bara
som PNG; kontrastprovet i `theme.rs` räknar på palettens tal, inte på det som ritades), fönstrets
beteende vid 780×540 (**fångsten tvingar 1920×1200** — `--capture-screenshots` sätter fullskärm, så
minsta storlek är granskad ur koden, inte ur en bild), och svarstider (fångsten körs på llvmpipe, en
mjukvarurenderare — varje bildrutetid därifrån är oanvändbar som sanning om appen).

---

## 2. Vad varje granskare ska döma

Varje fynd ska ha **ett bevis** (en etikett citerad ur `gui-evidence.md` med ytans namn, eller
`fil:rad`), en **allvarlighetsgrad** (🔴 Blocker / 🟡 Major / 🟢 Nit) och **vad som ska mätas** för att
avgöra om det är åtgärdat. Ett fynd utan bevis är en åsikt och stryks.

### G1 — Visuellt hantverk
Hierarki (vad ögat hittar först på varje yta), justering mellan kolumner och rader, täthet, luft,
omväxling mellan textstorlekarna, färgens roller (29 nycklar — används de till samma sak överallt?),
ikonografi. Frågan: *ser man vad som är viktigt utan att läsa allt?*

### G2 — Arbetsflöde
Antal steg till de tio vanligaste sakerna: spela in, lägga ett beat, klippa, kvantisera, mixa, lägga
reverb, exportera låt, exportera stammar, byta tempo, ångra. Var letar ögat? Finns det döda ändar
(en dialog som inte går att stänga, en knapp som är avstängd utan att säga varför)? Är
`README.md`:s löfte sant mot ytan? Frågan: *hur många klick, och hur många av dem gissar man?*

### G3 — Geometri och lay-out
Avklippt text, överlapp, kolumner som inte linjerar, död yta, träffytor under 24 px, ytor som
kollapsar vid 780×540, scroll kontra trunkering (en etikett som slutar i `…` utan ledtråd är en
gåta). Frågan: *håller ytan i minsta fönster, och vad försvinner först?*

### G4 — Kontrollernas uppriktighet
Det här repots egen familj av fel: **en flagga ingen läser är ingen funktion** och **en etikett som
gissar är värre än ingen etikett**. Leta: reglage utan läsare, två listor för samma sak, en etikett
som lovar mer än kommandot gör, ett standardvärde som inte är rattens eget, avstängda kontroller utan
orsak i statusraden. Frågan: *ljuger någon kontroll?*

### G5 — Text, enheter och teman
Svenska som nyckelspråk men 83 hårdkodade literaler; blandade enheter (`%`, `dB`, `ct`, `st`, `Hz`,
`BPM`); skiftläge (`BROWSER` mot `Tidslinje` mot `EXPORT SONG`); siffror med decimalpunkt mot komma;
både ljust och mörkt tema. Frågan: *skulle en svensk musiker och en tysk stava samma sak?*

---

## 3. Så avgörs rundan

Ett fynd flyttas till åtgärd bara om det (a) har ett bevis som håller vid efterkontroll och (b) går
att mäta om. Tabellen är Elefantens (huvudkontextens) dom över de fem rapporterna, kontrollerad i
koden efteråt och **åtgärdad samma kväll**. **En granskares siffra är ett påstående, inte ett
kvitto** — där en siffra inte gick att reproducera står det, och två fynd föll.

| # | Granskare | Fynd | Allvar | Beslut | Mätt efteråt |
| :--: | :--- | :--- | :---: | :--- | :--- |
| 1 | G4 | **Masterns PAN** — `master_pan` skrevs till projektfilen och lästes av ingen ljudkod | 🔴 | **Åtgärdad** | `SetMasterPanWidth` → `finish_master`; prov: equal-power, energin 2,0 i varje läge |
| 2 | G4 | **Masterns BREDD** — fält, startvärde och ratt, ingen läsare, sparades inte ens | 🔴 | **Åtgärdad** | mid/sid i `finish_master`; prov: 0 → mono, 1 → orörd, 2 → dubbel sida |
| 3 | G4 | **Drivepedalens TON** — `stompbox_tone` hade ingen läsare | 🟡 | **Åtgärdad** | enpolsfilter efter mättningen, i DRIVE-grenen; prov: 8 kHz tappar ≥10 dB mot öppet filter |
| 4 | G4 | **"4x12 Cab"** — kryssrutan skrev ett fält ingen läste | 🟡 | **Borttagen** | fältet är borta ur `SonixApp`; bygget varnar inte längre |
| 5 | G3 | **Spårnamnet ritas utan mått** (`arranger.rs`) | 🟡 *(nedgraderat)* | **Åtgärdad** | **Granskaren sa att namnet rann över M/S/R — det gör det inte:** knapparna ligger på rad 2 (y+34) och namnet på rad 1 (y+20). Det verkliga felet var att inget mått fanns, så ett långt namn kunde rita ut i tidslinjen. Nu mäts texten mot namnfältet |
| 6 | G2 | **Tempot gick inte att skriva** — bara ▲/▼ i 1-steg | 🔴 | **Åtgärdad** | `DragValue` 40–240 med decimal: drag, klick-och-skriv, en decimal |
| 7 | G2 | **Tempot gick inte att ångra** — `bpm` saknades i `TimelineUndoSnapshot` | 🔴 | **Åtgärdad** | `bpm` + `tempo_points` i snapshotet; ångring tas när redigeringen börjar (ett drag = ett steg) |
| 8 | G3 | **Minsta fönstret gav ingen tidslinje** — 780 − 200 − 320 − 228 | 🔴 | **Åtgärdad** | minsta fönster **1024 × 600** och dockens `min_width` 320 → 280: 276 px tidslinje i värsta fallet |
| 9 | G2 | **Tre navigationslistor** gick isär (Tab i annan ordning än flikraden) | 🟡 | **Åtgärdad** | `VIEW_ORDER` borta, `VIEWS` är enda listan med etiketter, `ViewMode::ALL` har samma ordning; två prov vaktar (`the_tab_row_and_the_tab_key_walk_the_same_order`, `every_surface_has_exactly_one_tab`) |
| 10 | G4 | **Flik och panel hade olika namn** ("Stems" mot "Stämmor", "Trummor" mot "Session") | 🟡 | **Delvis** | ordningen och listan är nu en; de **korta** flik-etiketterna är kvar med flit (raden är 14 flikar bred), men panelrubriken vinner inte längre över fliken i ordning. Ett namn per yta är kvar som beslut |
| 11 | G1 | **Fem fyllnadsfärger i verktygsraden** | 🟡 | **Öppen** | kräver en designregel (≤2 fyllningar per rad). Att välja vilka två är smak, inte mätning |
| 12 | G3 | **`interact_size.y = 21`** — knappar under 24 px | 🟡 | **Öppen** | en rad att höja, men den flyttar lay-outen på alla 14 ytor; ska mätas med fångst före/efter |
| 13 | G5 | **Sifferformat** — `format!("{:.1}", bpm)` ger "124.0" i alla sju språk | 🟡 | **Öppen** | kräver **en** språkmedveten formatterare och en svepning; att fixa fem ställen lämnar appen halvöversatt |
| 14 | G5 | **Exportraden gick aldrig genom `t()`** | 🟡 | **Åtgärdad** | `💾 EXPORT SONG` / `💾 EXPORT PAT` i alla sex tabeller; auditen rapporterar 0 nycklar utan tabell (1 888 nycklar i `t()`) |
| 15 | G5 | **Ljust tema under AA för accenter** | 🟡 | **Åtgärdad** | **reproducerat:** orange 3,72:1, amber 3,68:1, gul 4,45:1 mot panel_alt. Nya värden ger ≥4,5; mörkt läge hade också röd 4,27 och lila 4,49 → rättade. Provet `light_mode_keeps_the_accents_readable_as_text` vaktar |
| 16 | G1 | **Textskalan var åtta storlekar i praktiken** | 🟡 | **Åtgärdad** | 11,0 / 13,5 / 20,0 / 9,0 flyttade till `Theme::FONT_XS/S/L`; kvar utanför skalan: inga |
| 17 | G4 | **AUTO-TUNE-ratten styr både `strength` och `speed`** | 🟡 | **Åtgärdad** | två rattar (HASTIGHET, STYRKA) och två fält i `VocalHarmonizer`; live-vägen tar dem separat |
| 18 | G4 | **Ctrl+R / Ctrl+T stod i ledtrådarna men var inte bundna** | 🟡 | **Åtgärdad** | bundna i kommandoblocket; ledtrådarna är nu sanna |
| 19 | G3 | **Kanalnamnet trunkerades utan ledtråd** | 🟡 | **Åtgärdad** | hover på namnfältet visar hela namnet |
| 20 | G3 | **`set_width(380/340)` utan scroll** — klippte vid 780 px | 🟡 | **Löst av #8** | vid nya minsta bredd (1024) ryms 380 + 340; ingen egen layoutändring behövdes |

**Sammanfattning:** 20 fynd prövade. **14 helt åtgärdade**, **1 nedgraderat och åtgärdat** (#5),
**1 löst av ett annat** (#20), **1 delvis** (#10), **3 öppna** (#11 fyllnadsfärger, #12
`interact_size`, #13 sifferformat) — alla tre för att de är beslut eller svepningar, inte buggfixar.

**Nytt fynd som rundan inte hade:** spårfärgerna används som **text** (spårnamnet i huvudet) och
ligger i mörkt läge på **3,2–4,5:1** — `track_magenta` 3,23, `track_rose` 3,24, `track_violet` 3,56,
`track_cyan` 3,94. Antingen ljusas elva färger upp (ändrar hela arrangörens utseende) eller så ritas
namnet i `text`-färgen och identiteten bärs av ikon och accentrad. **Inte gjort** — det är design.

**Vad rundan inte kunde döma:** tolv av de 39 dialogerna i maskinbeskrivningen (kvotstopp — ytan finns
i fångsten, textbeskrivningen saknas), färg och kontrast i pixlarna (mörkt tema fångades, ljust inte),
fönstret vid minsta storlek (fångsten tvingar fullskärm) och svarstider (llvmpipe).

**Sviten efter åtgärderna:** 1 378 prov, 0 röda, 20 ignorerade. Fångsten går att köra om:
`env -u WAYLAND_DISPLAY WINIT_UNIX_BACKEND=x11 xvfb-run -a -s "-screen 0 1920x1200x24" ./target/release/sonix --capture-screenshots <mapp>`
— **utan** de två variablerna tar winit Waylandsätet och öppnar fönstret på skrivbordet i stället.

## 4. Alex' eget ingripande under rundan (2 nov)

Mitt i granskningen: **"Varje rad i tidslinjen behöver gå att zooma per rad, inte alla på en gång …
och de behöver vara lite smalare, inte så höga."** Byggt samma kväll:

| Vad | Hur | Bevis |
| :--- | :--- | :--- |
| Radens standardhöjd | 120 → **96 px** (`arranger.rs`) | spårhuvudets innehåll slutar vid ~60 px, så 120 gav en tom nedre halva. **Mätt i fångsten 2 nov:** raden mäter ~88 px i bilden, innehållet ryms, inget namn rinner in i M/S/R |
| Zoom per rad | `PlaylistTrack::row_scale` (0,5–2,0), klämd i `row_height()` (44–400 px) | `a_row_stays_between_its_limits` |
| Styra den | **Ctrl+hjul över spårhuvudet** zoomar bara den raden, plus tre steg i spårets högerklicksmeny (smal/normal/hög) | ingen körning — den vägen är klickad för hand, inte mätt |
| Följer med filen | `SavedTrackData::row_scale` med `#[serde(default)]` | `an_old_file_gets_a_normal_row_and_a_new_one_keeps_its_zoom` |
| Lanes och block | radhöjden gäller spårhuvud, lane, patternblock och tagningarnas band — inte masterns rad | **1 371 prov gröna**; fyra video-prov föll under last (ffmpeg-timeout, 5 s) och var gröna i isolering |
