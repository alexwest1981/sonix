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
att mäta om. Tabellen nedan är Elefantens (huvudkontextens) dom över de fem granskarnas rapporter,
2 nov: varje rad är kontrollerad i koden efteråt. **En granskares siffra är ett påstående, inte ett
kvitto** — där en siffra inte gick att reproducera står det.

| # | Granskare | Fynd | Allvar | Beslut | Kontroll / mätt efteråt |
| :--: | :--- | :--- | :---: | :--- | :--- |
| 1 | G4 | **Masterns PAN** (`fx_mixer_rack.rs:394`) — `master_pan` skrivs till projektfilen och läses av ingen ljudkod; `AudioCommand::SetMasterPan` finns inte | 🔴 | **Åtgärdad** | `rg SetMasterPan src/audio` gav 0 träffar före; nu `SetMasterPanWidth` + prov på equal-power-förstärkningen (energin 2,0 i varje läge) |
| 2 | G4 | **Masterns BREDD** (`:395`) — `stereo_width` fanns exakt tre gånger (fält, init, ratt), ingen läsare, sparas inte ens | 🔴 | **Åtgärdad** | M/S-bredd i `finish_master`; prov: bredd 0 → mono, 2 → sidan förstärkt |
| 3 | G4 | **DRIVE-pedalens TON** (`:1056`) — `stompbox_tone` hade ingen läsare; bara DRIVE skickades | 🟡 | **Åtgärdad** | ett pol-lågpass efter mättningen; prov: 1 kHz kvar, 8 kHz dämpad |
| 4 | G4 | **"4x12 Cab"** (`:1057`) — `stompbox_cab` läses ingenstans; en högtalarsimulering är en IR, inte en kryssruta | 🟡 | **Borttagen** | en kryssruta som inte gör något är värre än ingen kryssruta |
| 5 | G3 | **Spårnamnet ritas över M/S/R/F** (`arranger.rs:977`) — `painter().text` utan klippning; namnfältet är 78 px, "1 Lead Vocals (Suno AI…)" är ~170 px | 🔴 | **Kvittat, ej åtgärdat** | kräver en textbreddmätning + trunkering; står först i kön |
| 6 | G2 | **Tempot går inte att skriva** (`frame.rs:1477-1480`) — bara ▲/▼ i 1-steg; 124→140 är 16 klick och decimaltempo går inte alls | 🔴 | **Kvittat, ej åtgärdat** | kräver ett tal-fält med fokus; kön |
| 7 | G2 | **Tempot går inte att ångra** (`timeline.rs:198`: `TimelineUndoSnapshot` saknar `bpm`; `frame.rs:1479` ändrar utan `push_undo`) | 🔴 | **Kvittat, ej åtgärdat** | samma pass som #6 |
| 8 | G3 | **Minsta fönstret ger ingen tidslinje** — browser 200 + dock min 320 + spårhuvud 228 (`arranger.rs:121`) mot 780 | 🔴 | **Kvittat** | granskaren sa 8 px, räkningen ger **32 px** utan marginal; båda betyder oanvändbar. Kräver ett beslut: krympa docken eller höj minsta bredd |
| 9 | G2 | **Tre navigationslistor** (`state.rs:45` `ALL`, `frame.rs:31` `VIEW_ORDER`, `frame.rs:1689` `VIEWS`) — Tab går i annan ordning än flikraden visar | 🟡 | **Kvittat** | en lista, en ordning; mekaniskt men rör tangentbordsordningen |
| 10 | G4 | **Flik och panel har olika namn** — fliken säger "🧠 Stems", panelen "🪄 Stämmor"; "🥁 Trummor" mot "🥁 Session" | 🟡 | **Kvittat** | en strängkälla; kolliderar med flikradens bredd (kortare etiketter behövs där) |
| 11 | G1 | **Fem fyllnadsfärger i verktygsraden** (`frame.rs:1726-1738`) | 🟡 | **Kvittat** | kräver en designregel (≤2 fyllningar per rad), inte en buggfix |
| 12 | G3 | **`interact_size.y = 21`** (`theme.rs:567`) — varje knapp under 24 px | 🟡 | **Kvittat** | höjningen är en rad, men flyttar lay-outen på alla 14 ytor; mäts med en fångst före/efter |
| 13 | G5 | **Sifferformat** — `format!("{:.1}", bpm)` ger "124.0" i alla språk | 🟡 | **Kvittat** | kräver en språkmedveten formatterare (99 `format!`-ställen) |
| 14 | G5 | **Exportraden går aldrig genom `t()`** — "💾 EXPORT SONG" är svensk/engelsk/spansk samtidigt | 🟡 | **Kvittat** | 4 literaler + 6 tabeller |
| 15 | G5 | **Ljust tema under AA för accenter** — granskaren räknade orange 4,10:1 och amber 4,05:1 på vitt (mörkt 8,33/11,20) | 🟡 | **Kvittat, ej reproducerat** | kontrastprovet prövar bara `text`/`text_muted`; siffran är räknad ur paletten, inte mätt i en bild |
| 16 | G1 | **Textskalan är åtta storlekar i praktiken** (11,0 / 13,5 / 20,0 / 9,0 utöver 10,5 / 12,5 / 18) | 🟡 | **Kvittat** | en skala i `theme.rs`, anropen utanför den ska bort |
| 17 | G4 | **AUTO-TUNE-ratten styr både `strength` och `speed`** (`transport.rs:2175`) | 🟡 | **Kvittat** | två rattar eller en årlig ledtråd — ett beslut |
| 18 | G4 | **Ctrl+R / Ctrl+T står i ledtrådarna men är inte bundna** (`frame.rs:1732/1735`) | 🟡 | **Kvittat** | en etikett som gissar; bind dem eller stryk texten |
| 19 | G3 | **Kanalnamn trunkeras utan ledtråd** (`channel_rack.rs:116-121`) | 🟡 | **Kvittat** | hover med hela namnet |
| 20 | G3 | **`set_width(380/340)` utan scroll** (`channel_rack.rs:1327/1402`) | 🟡 | **Kvittat** | klipper vid 780 px; ska mätas med `clip_rect` |

**Sammanfattning:** 20 fynd prövade, **4 åtgärdade** (alla fyra är kontroller som ljög — den familj
repot redan har en regel om), **16 kvittade och köade** i den ordning tabellen står. Noll fynd
underkändes: varje rad granskaren skrev gick att hitta i koden.

**Vad rundan inte kunde döma:** tolv av de 39 dialogerna i maskinbeskrivningen (kvotstopp — ytan finns i
fångsten, textbeskrivningen saknas), färg och kontrast i pixlarna (mörkt tema fångades, ljust inte),
fönstret vid 780×540 (fångsten tvingar fullskärm), och svarstider (llvmpipe). De tre står kvar som
mätpunkter, inte som slutsatser.

---

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
