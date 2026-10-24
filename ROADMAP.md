# 🗺️ Sonix Studio — Roadmap

> **Sanningen om vad som är gjort och kvar.** Uppdaterad **2026-10-23** mot `master`.
> Siffrorna är räknade ur filerna — kommandot står vid varje tabell — inte gissade och inte
> ärvda. Historik och bevis per punkt: [`tools.md`](tools.md) (P1–P27) och git.
> Den tidigare 3 000-radiga roadmapen ligger kvar i git (`git show 65af62b:ROADMAP.md`); den
> läses inte längre som sanning, för den slutade stämma i oktober.

**Läsordning:** den här filen för läget · [`FL-STUDIO-INVENTORY.md`](FL-STUDIO-INVENTORY.md) och
[`ABLETON-INVENTORY.md`](ABLETON-INVENTORY.md) för vad de två DAW:erna har ·
[`SOUNDTRAP-INVENTORY.md`](SOUNDTRAP-INVENTORY.md) för vad en webb-DAW har som vi inte har (samarbete,
molndelning, innehåll) · [`UX-AUDIT.md`](UX-AUDIT.md) för hantverket (typografi, färg, ikoner, mätt) · `docs/` för de
kompletta referenslistorna (filposter, menyrader, parameterordförråd) · `README.md` för vad
gränssnittet lovar.

---

## 📊 Status i siffror

| Mått | Räknat | Så räknades det |
| :--- | ---: | :--- |
| **FL-pluggarna** (51 generatorer + 90 effekter) | **✅ 70 · ◐ 65 · ✗ 0** av 141 (6 ej tillämpliga) | `awk` på statuskolumnen i `FL-STUDIO-INVENTORY.md` §2 — och **namnen är kontrollerade mot installationen**: 51 + 90 mappar i `Plugins/Fruity`, noll skillnader mot inventariets 141 rader |
| — andel | **52 %** helt · **100 %** helt eller delvis · **0 %** saknas | 70 / (70+65) / 0 av 135 tillämpliga |
| **Ableton-enheterna** | 72: 13 instrument + 48 ljudeffekter + 11 MIDI-effekter | `ABLETON-INVENTORY.md` §2 |
| **Ableton gap-matris** | 6 områden där vi ligger före · 4 delvis · 8 efter | 18 rader i §8, räknade på dom-kolumnen |
| **Koden** | 119 filer · **110 858 rader** i `src/` · **931 `#[test]`** | `find` / `cat` / `grep -c` 2026-10-23 |

Ett räknefel i en inventarierubrik är rättat samma dag: Ableton sa **5 delvis** när matrisen har
4 — matrisen var rätt, summeringen var det inte. FL:s rubrik stämmer nu med tabellen (70/64/7).

**Testtalet är antalet `#[test]`-funktioner i trädet, inte en körd svit.** Inventariernas
644/693 gäller en äldre commit; den siffran ska mätas om med `cargo test` innan den citeras igen.

---

## ✅ Klart senast — det som byggdes efter att roadmapen slutade följa med

| Vad | Commit | Vad som finns | Beviset |
| :--- | :--- | :--- | :--- |
| **Korsning** (FL: `Create crossfade`) | `0e0a094` | `create_crossfades()` + knapparna `🔀 Skapa korsning` och `✚ Korsa överlappande klipp`; `declick()` lägger 3 ms golv på varje regionkant | korsningen = överlappet, kapat vid halva regionen; regioner som möts utan överlapp får ingen; tre prov |
| **Kanalanteckningar** (FL: NoteBook) | `3df4001` | `PlaylistTrack::notes`, ett fält per kanal i kanalinställningarna | `channel_notes_are_empty_in_old_files_and_survive_a_round_trip` |
| **Morphine-kärnan** | `3bff962` | `instruments::morph_spectrum` + `Preset::Morph`: två additiva operatorkällor och ett läge 0–1 mellan dem | exakt vid 0 och 1, linjärt emellan (provat); fyra källor och de 199 förvalen saknas |
| **Capture MIDI** | `04322e1` + `038d5f5` | `note_capture::NoteCapture` + knappen `⏺ Fånga nyss spelade noter` — varje levande not fångas med tid sedan den första, räknad till taktens 16 steg | knappen är avstängd tills något fångats och tipset visar antalet; frasen viks in i **en** takt (`shortcut` i modulen) |
| **Gruppbakning** (Ableton: `Bounce Group In Place`) | `444629f` | `bounce_group_to_new` — gruppen solas med samma regel som live-solon (`group_member_mask`), ljudspåren i gruppen får sina buffertar tömda, resultatet hamnar på ett nytt spår och källorna tystas | ✅ i Ableton-inventariet §5, alla tre bounce-kommandona |
| **Spegling** (FL: `Invert`) | `1c66284` + `d63fd97` | `↕ Spegla tonhöjder` kring C4 (MIDI 60) | gäller **urvalet**, inte hela tagningen — rättat i `d63fd97` |
| **Join / Fit** (Ableton) | `b8f56e2` | `⧉ Slå ihop dubbletter` (två noter på samma position och ton blir den starkaste) och `⇥ Tryck ihop frasen ×2` | — |
| **Nudge** (Ableton) | `65af62b` | `-1/16` / `+1/16` skjuter urvalet i tid, ett sextondelssteg i taget; mixerns gränser har egen nudge sedan 8.7 | — |
| **ADSR + VFX Envelope som källa** | `1acd7d6` + `68c31ce` | `ModSource::Adsr` (attack/decay/sustain/release i sekunder, triggas av noter) och ADSR-rattarna i modulationspanelen; källorna är nu LFO, envelop, anslag, tonhöjd, ADSR och spänning → volym, panorering, pitch och cutoff | version 0.9.1 |

**Notlängder finns inte i branchen.** `TakeNote` bär `pos`, `key`, `velocity` och `probability` —
ingen längd — och `feat/note-lengths` har noll commits ovanpå `master`. Alltså står den under
`Kvar`, inte här.

---

**Byggt efter roadmappens senaste revidering:** notlängder (`TakeNote.length_steps`), `Legato`/`Chop`/`Join`, artikulation per not, formelstyrning (`src/formula.rs`), Capture MIDI, strängrösten (dämpning i återkopplingen + kropp), Transmitter, A/B-jämförelse, take-lanes i kompen, i18n-revisionen, `Consolidate` (`82c611d`), `Flatten` (`b4a23ea`), MusicXML-exporten (`485410d`), MPE-kanalen (`0223f59`), groove-biblioteket (`b6b6624`), frasbanken (`1f2e4e6`) och **score-loggen** (`dump_score_log` — noterna utlagda över de takter de tar, med prov på både stegräkningen och pulsomräkningen).

## ⬜ Kvar

### Går att bygga vid datorn nu

| # | Vad | Storlek | Varför den ligger här |
| :--: | :--- | :---: | :--- |
| 1 | **Frasbank (`.fsc`/`.mid`) + Riff machine** — *✅* | *S* | **🎼 Frasbank** läser en mapp med `.mid`/`.midi` **och FL:s `.fsc`**, med **▶ lyssning** före infogning. Mätt mot FL:s egen bank: 609 av 609 filer läses, 5 456 noter; 120 poster med anslag 0 är pauser och infogas inte. Riff-generatorn fanns. ▶ lyssnar på frasen före infogning (0.9.8). |
| 2 | **Groove-pool** — *✅ hörbar och mappbytbar* | *S* | En mapp läses en gång via `GrooveLibrary::load_from_dir`, mallarna väljs ur en lista och hamnar i poolen. **▶** spelar en demorytm (kick, virvel, hi-hat) med mallens förskjutning och mängd, så mallen hörs innan den läggs på. 📂 väljer en annan mapp och 🔄 läser om **samma** mapp utan att pianorullen stängs |
| 3 | **Temafiler (JSON) med semantiska namn** | *M* | `Theme::` används på **909** ställen (räknat) — mekaniskt, men 909 rader |
| 4 | **Fokus/Tab mellan ytorna, zoom-kommandon, ledtrådar** — *✅* | *S–M* | **Tab/Shift+Tab** går genom alla 13 ytor, **Ctrl + +/−** zoomar pianorullens rutnät (Ctrl+0 nollställer), och de **kryptiska** knapparna har ledtråd: M/S på kanal, buss och grupp, chop-delarna, zonens papperskorg, mallens ✖, ▶ i webbläsaren, ✕/⬆ i promptrutan. Provat: ett varv återvänder, ingen yta dubbel, zoomen stannar vid sina ändar.  Abletons `Focus*` och `Zoom*` är adresserbara; 2 302 `sInfoText` mot våra 135 `on_hover_text` är den tydligaste hjälpklacken |

### Kräver ett nytt lager — inte en ratt

| # | Vad | Storlek | Lagret som måste byggas först |
| :--: | :--- | :---: | :--- |
| 1 | **MPE / per-not pitch bend** — *✅* | *M* | Kanalen finns i `NoteOn`/`NoteOff` (`channel`), `ChannelPitchBend` böjer bara rösterna på samma kanal (mastern 0 böjer allt), och `BendDelayed` lägger böjningen samplenoggrant. Inspelade bend hamnar i patternets kurva, ritas i **BEND**-fältet under velocity (dra, dubbelklicka, högerklicka), följer med i projektfilen och i exporten. Mätt: en bend fördröjd 100 samples gäller först efter 100 samples; en punkt på 2,5 i ett steg ger 9 000 samples fördröjning vid 10 000/steg. Kvar: MPE-zonens egen kurva per ton (bend är kanalvis, som i MPE-standarden) |
| 2 | **VFX Key Mapper / Keyboard Splitter / Level Scaler** — *✅* | *S–M* | zonerna har kanal, transponering, anslagsfaktor, kurva och nu **not → not**: `snap_scale` kvantiserar zonen till en skala (`snap_to_scale`, närmaste ton högst sex halvtoner bort, grundton från projektet). Prov: C# i C-dur → C, F# → F, E orörd, C# i C#-dur orörd. |
| 3 | **Kompning (take lanes)** — *✅* | *M* | Varje inspelningsförsök blir en **lane** i stället för att skriva över det förra: `Pattern.alt_takes` + `active_take`, **🧵 Tagningar** i menyn väljer vilken som spelas, och kvantisering/transponering gäller den valda. Prov: lane 2 spelas och redigeras utan att lane 1 rörs. **Kompning:** `Pattern.comp` väljer lane per steg (✂ i 🧵-fönstret: välj steg, klicka lanen), täckningen är implicit fram till nästa komp, och `comped_take()` lägger ihop noterna — spel och export använder den. Prov: steg 1 ur lane 1 + steg 9 ur lane 2 ger båda noterna; ett komp mitt i delar utan omräkning. **Komp per takt (0.9.45):** `CompSlice.bar` (`None` = varje takt, `Some(bar)` = en takt), valt med *Takt* i 🧵-fönstret; ett taktkomp vinner över ett globalt på samma steg, och exporten har en tagning per takt (`PatternSnap.bar_takes`). **Bugg fixad på vägen:** i ett flertaktsblock skickades hela blockets stegindex (0–63) till en tagning som är en takt, så ingen tagning spelade i takt 2–4 i filen — prov: `a_take_note_plays_in_the_second_bar_of_a_block`. Kvar: lane-vyn i spellistan |
| 4 | **Warp-lägena** — *✅* | *L* | Fem lägen i `audio::warp::WarpMode`, var och ett med eget korn och sökvidd; `Transient` kopierar anslagen orörda. **Mätt** (klickrad 2×, ren 220 Hz): Fast topp **0,413** med förhandsläckage **1,65**, Complex och Transient topp **0,900** = källans egen topp, Transient crest **82,5**. **Per klipp (0.9.44):** `AudioRegion.warp_mode` (serde-default `Complex`, så äldre filer låter som förut och cachade filer förblir giltiga), väljs i klippets högerklicksmeny; **läget ligger i sträckningens nyckel** — annars spelar ett lägebyte den gamla filen — och nya klipp ärver det globala läget. Prov: `the_warp_mode_is_part_of_the_stretch_key` |
| 5 | **Bärvågsingång i masterns kedja** — *✅* | *M* | vocodern finns i sångstudion (`vocode`, 16 band, bärvåg som följer sången); som **effekt** kräver den en andra ingång. **0.9.51:** `MasterVocoder` i masterns kedja med **rackets synt som bärvåg** och **stammarna som modulator** (16 logaritmiska band, 100 Hz–8 kHz). **Mätt:** tyst modulator → **0,00000**, stam på 400 Hz → 0,026 — bärvågen öppnas av modulatorns spektrum, inte av en ratt. Modulatorn tas ur `stem_mix`, inte ur mastern: syntens egen reverb-/delaysvans öppnade vocodern annars vocodern finns i sångstudion (`vocode`, 16 band, bärvåg som följer sången); som **effekt** kräver den en andra ingång. Sidokedja till plugins finns, inte till masterns kedja |
| 6 | **Skript-API** (pianorulle, utbyggnad) | *L* | en egen körningsyta. FL:s `Piano roll script reference.txt` och Abletons Max/Extensions är båda det; vår AI-väg är kommandon, inte skript |
| 7 | **Additiv/spektral syntes i full längd** (Harmor, Ogun, Sytrus 6-op) — *◐ Sytrus: matris, enveloper och filter klara* | *XL* | **`ADDITIVE-SYNTH.md`** har fyra steg med mått per steg. **Steg 1 klart:** 64 partialer i en delad tabell (en läsning per sampel i stället för en `sin()` per partial). **Mätt:** oscillatorn **241× realtid**, hela rösten **30× realtid** vid 16 röster (en `sin()` per partial: 3,4×). Prov: tabellen matchar den direkta summan (< 2e-3) och 64:e partialen hörs. **Steg 2 klart:** egen attack/decay/sustain per partial, räknad var 32:a sampel. **Mätt:** centroid **4,30 → 1,03** över 500 ms (partial 8 dör på 0,25 s medan partial 1 står kvar); värsta fallet 16 röster à 64 partialer med kontur går i **2,0× realtid** (utan kontur 240×). **Steg 3 klart:** spektralt filter över partialindex (`SpectralFormant`). **Mätt:** centroid **4,59 → 8,69** när formanten flyttas från partial 2 till 9, grundtonen **110,0 Hz** i båda. **Steg 4 klart:** tidsaxeln (`SpectralSweep`) glider formanten per röst; **mätt:** två kurvor ger samma centroid vid anslaget (4,61/4,63) men olika vid 0,5 s (6,42/4,59). **Bandbegränsning:** tabellen finns nu i ett exemplar per partialgräns och noten väljer oktavband, så en hög not inte viker partialer ner i hörbart område — **mätt** på C7 med alla 64 dragstänger: partial 20 vek till 6,1 kHz med **0,51** av grundtonens amplitud före, **0,0045** efter (−41 dB), och grundtonen blev **6× starkare** när bandet normaliseras över sina egna partialer. **Patchen sparas (0.9.41):** röstens hela patch ligger i projektfilens `synth`-block med `serde(default)` — förut tappades dragstänger, konturer, formant, sub, unison och FM vid sparning, och en återöppnad fil lät som standardpatchen. **En partial i taget (0.9.41):** *Partial* i SPEKTRAL ENVELO väljer 0 = alla (mallen med lutning) eller en enskild partial, och A/D/S skriver då ett undantag som ligger ovanpå mallen — prov: partial 5 får egen kontur medan partial 1 och 4 följer mallen. **Kvar:** Sytrus 6-op, godtycklig tidskurva/bild |
| 8 | **Control Room** — *✅* | *M* | `audio::control_room::ControlRoom` sitter **efter** masterns FX-kedja: nivå 0–2×, **DIM** (−20 dB), **MONO** och **TYST**, med egen ratt i mixern. **Talkback (0.9.50):** en 🎙-knapp tar micken till monitorerna i stället för in i mastern och lägger den **efter** control room-steget — DIM dämpar musiken men inte pratet, och den som lyssnar på en cue-buss hör det över bussen. **Mätt:** dimmad monitor, micken av 0,0099 mot på 0,25; ringen fylls av mikrofoninspelningen, så utan den är talkback tyst. Exportmotorn får aldrig kommandot, så monitorläget kan inte hamna i en fil (prov: tystad motor ger 0,0, färsk motor låter). **Cue-sends A/B (0.9.43):** varje spår har CUE A/CUE B (post-fader, som en send på en konsol), monitorerna väljer MIX/A/B i control room, och bussen hörs bara där — exportmotorn har alltid `Main`, så en cue kan aldrig hamna i en fil. **Mätt:** 400 Hz till cue A, 1 200 Hz utan send — mixen bär båda (0,272/0,269), cue A bär 400 Hz (0,277) och läcker 1 200 Hz på 0,0018 (**0,7 %**, −43 dB); cue B är tyst. Sends följer med i projektfilen. Kvar: talkback per kanal (kräver mikrofoningång i cue-vägen) |

### Går inte att bygga här — och varför

| Vad | Skäl |
| :--- | :--- |
| **FL Studio Mobile** (344 filer, `.flms`-program) | mobilappens rack och instrument; annan plattform och annan motor |
| **ReWire** | proprietärt och avvecklat; FL och Ableton har det kvar som bakåtkompatibilitet (`ReWire.dll`, `ReWired_x64.dll`) |
| **Razer Chroma** | tangentbords-RGB via en Windows-driver; det finns inget att svara på i en Linux-DAW |
| **ZGameEditor Visualizer** | 1 423 `.ilv`/`.zla`-förval och en GPU-scen — ett eget projekt, inte en effekt |
| **Max for Live / Extensions** | Abletons moat: licensierad runtime och SDK |
| **Lektioner** (1 016 filer hos Ableton) | kursmaterial, inte kod. Tio egna vore rimligt; 1 016 är det inte |

**Väntar på en maskin, inte på kod:** Windows-porten (7.1, pausad — Alex har ingen laptop) och
kvittensen av en **riktig** yabridge-brygga (7.3/4.6). CLAP-, VST3- och VST2-vägarna är byggda,
processar ljud med PDC, sparar state och är gröna mot mock-modulerna; det som saknas är en riktig
plugin, en riktig skärm och en riktig enhet.

---

## 📌 Tre saker att veta innan man räknar vidare

1. **FL-inventariets §10 är skriven 21 okt och har tre rader som hunnit bli byggda:**
   modulationskällor (`ModSource` bär sex källor → fyra mål), Granulizer (`Granular` i masterkedjan)
   och vocodern (`vocode` i sångstudion). Den här filens `Kvar` gäller, inte den listan.
2. **Statusarna ✅/◐/✗ är på funktionsnivå.** "✅ Fruity Limiter" betyder att vi har en limiter med
   tak och release — inte att kurvan är identisk med FL:s.
3. **En punkt flyttas från `Kvar` till `Klart` bara med ett prov eller en mätning.** Att knappen
   finns är inte att den fungerar; att den är byggd är inte att den är klickad i en riktig skärm.
