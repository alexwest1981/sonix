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
| **FL-pluggarna** (51 generatorer + 90 effekter) | **✅ 70 · ◐ 56 · ✗ 15** av 141 | `awk` på statuskolumnen i `FL-STUDIO-INVENTORY.md` §2 — och **namnen är kontrollerade mot installationen**: 51 + 90 mappar i `Plugins/Fruity`, noll skillnader mot inventariets 141 rader |
| — andel | **50 %** helt · **89 %** helt eller delvis · **11 %** saknas | 70 / (70+56) / 15 av 141 |
| **Ableton-enheterna** | 72: 13 instrument + 48 ljudeffekter + 11 MIDI-effekter | `ABLETON-INVENTORY.md` §2 |
| **Ableton gap-matris** | 6 områden där vi ligger före · 4 delvis · 8 efter | 18 rader i §8, räknade på dom-kolumnen |
| **Koden** | 119 filer · **110 858 rader** i `src/` · **931 `#[test]`** | `find` / `cat` / `grep -c` 2026-10-23 |

Ett räknefel i en inventarierubrik är rättat samma dag: Ableton sa **5 delvis** när matrisen har
4 — matrisen var rätt, summeringen var det inte. FL:s rubrik stämmer nu med tabellen (70/56/15).

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
| 1 | **Frasbank (`.fsc`/`.mid`) + Riff machine** — *◐ audition saknas* | *S* | **🎼 Frasbank** läser en mapp med `.mid`/`.midi` **och FL:s `.fsc`**, med **▶ lyssning** före infogning. Mätt mot FL:s egen bank: 609 av 609 filer läses, 5 456 noter; 120 poster med anslag 0 är pauser och infogas inte. Riff-generatorn fanns. Kvar: höra frasen innan den infogas |
| 2 | **Groove-pool** — *✅ hörbar och mappbytbar* | *S* | En mapp läses en gång via `GrooveLibrary::load_from_dir`, mallarna väljs ur en lista och hamnar i poolen. **▶** spelar en demorytm (kick, virvel, hi-hat) med mallens förskjutning och mängd, så mallen hörs innan den läggs på. 📂 väljer en annan mapp och 🔄 läser om **samma** mapp utan att pianorullen stängs |
| 3 | **Temafiler (JSON) med semantiska namn** | *M* | `Theme::` används på **909** ställen (räknat) — mekaniskt, men 909 rader |
| 4 | **Fokus/Tab mellan ytorna, zoom-kommandon, ledtrådar** — *✅* | *S–M* | **Tab/Shift+Tab** går genom alla 13 ytor, **Ctrl + +/−** zoomar pianorullens rutnät (Ctrl+0 nollställer), och de **kryptiska** knapparna har ledtråd: M/S på kanal, buss och grupp, chop-delarna, zonens papperskorg, mallens ✖, ▶ i webbläsaren, ✕/⬆ i promptrutan. Provat: ett varv återvänder, ingen yta dubbel, zoomen stannar vid sina ändar.  Abletons `Focus*` och `Zoom*` är adresserbara; 2 302 `sInfoText` mot våra 135 `on_hover_text` är den tydligaste hjälpklacken |

### Kräver ett nytt lager — inte en ratt

| # | Vad | Storlek | Lagret som måste byggas först |
| :--: | :--- | :---: | :--- |
| 1 | **MPE / per-not pitch bend** — *◐ live-bend klar* | *M* | Kanalen finns nu i `NoteOn`/`NoteOff` (`channel`), och `ChannelPitchBend` böjer bara rösterna på samma kanal; mastern (0) böjer allt. Kvar: spela in bend i tagningen (TakeNote bär ingen bend), och rita det i pianorullen. Tre prov i `synth::tests` + ett i MIDI-tolken |
| 2 | **VFX Key Mapper / Keyboard Splitter / Level Scaler** | *S–M* | kanalidentiteten finns nu (se ovan), så en not→not-omvandling är nästa steg — ingen ny modell behövs |
| 3 | **Kompning i spellistan (take lanes)** | *M* | MIDI-tagningarna finns; spellistans lanes kräver en lane-modell ovanpå regionerna |
| 4 | **Warp-lägena** (Beats/Tones/Texture/Complex) | *L* | fyra analysstrategier ovanpå WSOLA, inte fyra rattar på samma strategi |
| 5 | **Bärvågsingång i masterns kedja** | *M* | vocodern finns i sångstudion (`vocode`, 16 band, bärvåg som följer sången); som **effekt** kräver den en andra ingång. Sidokedja till plugins finns, inte till masterns kedja |
| 6 | **Skript-API** (pianorulle, utbyggnad) | *L* | en egen körningsyta. FL:s `Piano roll script reference.txt` och Abletons Max/Extensions är båda det; vår AI-väg är kommandon, inte skript |
| 7 | **Additiv/spektral syntes i full längd** (Harmor, Ogun, Sytrus 6-op) | *XL* | oscillatorbank per röst + spektral morfning. Morphine-kärnan (två källor) är byggd; resten är ett synthprojekt |
| 8 | **Control Room-matrisen** | *M* | monitor/cue/talkback per kanal. Utgångsbussarna finns (`ab715d5`), men matrisen kräver att varje kanal får egen signal |

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
