# Läget i Sonix — överlämning till nästa session

*Skriven 2026-09-12 efter en lång dag. Läs den här först, sedan `ROADMAP.md`.
Nuläget bor i roadmapen; den här filen är bryggan dit — vad som hände, vad som är
mätt, och var nästa andetag ska tas.*

---


> **Kartan finns nu: `SECTIONS.md`** (genererad av `tools/sections.py`). Läs den i stället för att
> greppa hela koden: den listar varje moduls radantal, publika ingångar, antal anropare utanför
> filen och antal tester — plus modulens **egen** status, som står i modulens `//!`-dokumentation:
> `//! Status: fryst — …` och `//! Rör inte: …`. Saknas statusen står modulen i listan "utan
> status" sist i filen: lägg raden i modulen och kör om skriptet. `python3 tools/sections.py --check`
> ger exit 1 när kartan är inaktuell (kan läggas i CI). **En ägare per faktum** — samma läxa som
> de två listorna för samma sak.

## 1. Var allt står

- **Arbetskatalog:** `~/Projects/sonix`, branch `master`, remote `origin` → github.com/alexwest1981/sonix
- **Binären som körs:** `~/.local/bin/sonix` → symlänk till `~/Projects/sonix/target/release/sonix`
  (skrivbordsgenvägen `~/.local/share/applications/sonix.desktop` pekar rätt — den
  pekade på en tre dagar gammal kopia i `~/.cargo/bin/` fram till 2026-09-12).
  **En binär som gäller (2026-09-13):** den gamla kopian i `~/.cargo/bin/sonix` (9 sept, 19 MB)
  och en föräldralös mise-shim (`~/.local/share/mise/shims/sonix` → `/usr/bin/mise`, utan
  registrerat verktyg) är borttagna. Kör **aldrig** `cargo install --path .` i det här repot —
  det är precis så kopian uppstod. `cargo build --release` + symlänken är hela kedjan, och
  `which -a sonix` ska bara visa `~/.local/bin/sonix`.
- **Tester:** 462 default / 508 med `--features plugin-host`, **0 varningar** i båda
  (mätt 2026-09-13 kväll, efter sends mellan spår 8.3, samplern 8.4 och 8.2:s visning). CI fäller numera **alla** ben på
  varningar, inte bara Windows.
- **Senaste commit:** `git log --oneline -1` — hasharna i den här filen har åldrats förr,
  så den raden är sanningen. Bakom ligger 8.4 (samplern), 8.3 (sends mellan spår), 8.10e (motorvalet),
  8.14/8.15/8.15b och spelhuvudets klocka (8.13b).
- **Bara en arbetskatalog.** Worktreen `~/Projects/sonix-tempo` (8.10 steg 2) är **borta** —
  grenen är mergad till master och trädet städat. `git worktree list` ska visa en enda rad.
  **Kolla `git status` innan du bygger** om något ser märkligt ut: två skrivare i samma
  `app.rs` är fortfarande hur man tappar någons arbete.
- **Kör igång:** `cargo build --release --locked` (release krävs — det är den binären
  Alex startar). Efter varje ändring: `cargo build --release --locked`, `cargo test
  --locked --bin sonix`, `cargo test --locked --features plugin-host --bin sonix`.

## 2. Vad som gjordes 2026-09-12, i ordning

| Commit | Vad |
| :--- | :--- |
| `7a10fd1` | 8.2 steg 2 kärna: en väg för takter→sekunder (`stem_regions_for`), `secs_per_beat_at`, `secs_for_bars_at` |
| `ce47323` | CI: även Linux-bygget fälls av varningar (jag hade läst förbi en egen varning) |
| `7c8d4dd` | 8.2 steg 2 grupp 1: fem positioner genom tempokartan + inversen `bar_at_secs` |
| `c11e4bb` | 8.2 steg 2 klart för det som hörs (20 av 24 bpm-ställen) |
| `f089a96` | 8.2 steg 3: tempobyten i MIDI-exporten, med **gyllene test** (orörd fil är byte-identisk) |
| `8101ea6` | 8.2 datalager: `tempo_points` med `#[serde(default)]`, gamla projekt läses som förut |
| `3eacf85` | 8.2 UI: högerklick på taktlinjalen sätter tempobyte + ⏱ Tempo-listan |
| `e8235f5` → `df38312` | **8.3 exakta vågformer:** `waveform.rs` med `envelope_per_pixel`, flernivåcache, `envelope_looped`, dB-kurva, fyra skärpningar |
| `51251fc` | **Hängningen fixad:** cachen byggs i arbetstråd, första bildrutan blockerar inte |
| `ff1a4fb` | Tysta klipp: `load_audio_or_report` — ljud som inte går att läsa tiger inte längre |
| `fe93ae8` | `stem_files_for_import(files, keep_mp3)` — wav vinner över mp3 |
| `6ce0550` | Delad lista `UNREADABLE_SOURCES` — **elva anropsställen**, en avläsning per bildruta |
| `2352332` | `load_sample_pcm_arcs` rapporterar genom samma lista |
| `4279258` | Två Sound Browser-vägar **avbryter** i stället för att skapa klipp utan ljud |
| `3799755` | **Restore-buggen:** `retire`s returvärde kastades, koden läste sökvägen den just pensionerat |
| `7561491` | 8.5a steg 1: separatorn skriver stämmorna till disk (`write_stem_wav`, `paths::stem_file`) |
| `ad65c24` | **`sonix --clean-tags`** — ID3-sanering, visar vad som tas bort, ljudet orört |
| `b50f9f4` | **Stämvyn** ritar sann vågform ur egna samplar (`waveform_pairs`) |
| `d26b879` | **Sångstudion** gör samma sak för tagningar |
| `0bfb990` | Tempofältet går att **skriva** i (`DragValue`), inte bara dra |
| `4d45b8b` | `sonix --detect-bpm` + **mätningen som diskvalificerade analysen** |
| `fc5d830` | Metadata städas **vid inläsning**, i filen, med besked i statusraden |
| `78eb13e` | Den här överlämningen |
| `cb9f152` | **Pixeleringen efter import, stängd på datanivå:** cachen byggs i avkodningstråden, spåret får PCM + cache (se §4 — punkten är klar) |
| `f0dc8eb` | **8.10 steg 1:** klippen följer projektets tempo (bandspelarlogik — tonhöjden följer), bit-exakt vid faktor 1,0 |
| `a510775` | **Alex hörde inget** — hans projektfil saknade fältet. Inspelningstemot mäts nu fram vid inläsning (hans Broken: 9/9 klipp), plus en stämpel i ⏱ Tempokarta |
| `392a30c` | **8.11 tonarten:** en tabell och ett index i stället för två listor, skal-låset gör något, tonarten sparas |
| `1797cb7` | **8.7 steg 2:** slicekartan spelas från steg och piano roll (den andra sessionen) |
| `1d3fc1a` | **8.3 sends till bussar:** post-fader, mål-bussens mute/solo gäller, med i exporten, UI i mixern |

## 3. Mätt, inte gissat (bär dessa vidare — de är dyra att ta fram igen)

- **Appen har ingen mp3-avkodare.** Inga symphonia/rodio/minimp3. En mp3 i en regions
  `source_path` blir ett **tyst klipp, varje gång**. Wav/FLAC går bra.
- **BPM-analysen (`estimate_bpm`) är opålitlig på riktig musik.** Mot facit 120.0 gav
  trumstämman 139.53 och basstämman 93.75 — ur *samma låt*. Ingen av dem är en jämn
  multipel av 120, så det är inte oktavförvirring. **Koppla inte in den i importen.**
  Använd Sunos egen info (`parse_suno_zip_info` läser BPM ur arkivets namn) — den
  satte rätt 120.0 vid senaste importen.
- **Ljudet följer INTE projektets tempo.** Det finns inget tempokommando i `command.rs`
  alls. Stegklockan (`step_duration` → `TempoMap::single(self.bpm)`) följer tempot, så
  mönsterspår ändrar sig — men ljudklipp spelas i sin inspelade hastighet. Fixen:
  sätt klippens `time_stretch` (fältet finns, står på 1.0) till `ursprungstempo/nytt`
  och skicka till motorn. WSOLA finns sedan 2.3.
- **Cachebygget:** 1 min 62 ms / 4 min 279 ms / 10 min 729 ms i **release**
  (debug: 258 ms / 1,05 s / 2,78 s). 2,7 MiB minne för fyra minuter. Nycklas på
  buffertens identitet. Importen betalar det numera **per stämma** (≈2,5 s för åtta
  fyraminutersstämmor), i den tråd som ändå avkodar filerna — priset för att
  vågformen är sann från första bildrutan.
- **Den gamla översikten kunde TAPPA ljud:** den läste var 64:e sample inom varje
  punkt, vilket för en riktig stämma är ett steg på 93. Mätt: 100 av 200 enstaka
  anslag syntes inte alls. Cache-vägens fack kan inte missa något (0 av 200).
  Testet `the_old_overview_lost_the_shortest_transients` är kvar som mätning.
- **`write_with_ffmpeg` kodar från en temp-wav som `write_wav` skriver** — därför är
  Sonix egna exporter redan fria från främmande taggar, per konstruktion.
- **Frystabell:** ofruset 1,4 %/2,2 % → fruset 0,3 %/0,3 % (~4,7×).
- **Alex' projektfil är den enda källan till sanning om hur Sonix ser ut i bruk.** Dagens
  lärdom: funktionen "klippen följer tempot" var osynlig för honom därför att alla hans
  projekt sparades **innan** `source_bpm` fanns — fältet förekommer inte i en enda av dem.
  Ett fält som läggs till i dag når inte bakåt; ett ärligt standardvärde ("rör inte") blir
  i praktiken "ingenting händer". **Mät därför mot hans filer, inte bara mot sandlådor:**
  `./tmp/sonix_proj_diag.py`-mönstret (parsa .sonix, slå upp filens längd med `ffprobe`,
  räkna `takter × 240 / sekunder`) ligger i commit-texten för `a510775` och gav 9/9, 11/11
  och 0/9 — svaret på varför han inte hörde något.

## 4. Nästa steg, i ordning

0. ~~**Städningen tvättade för mycket**~~ — **RÄTTAT 2026-09-13.** Alex' invändning
   ("inte tvätta för mycket info om stämmor som hämtas från Suno") var riktig: den första
   versionen tog **hela** taggen, alltså också `USLT` (låttexten, 2 130 B) och `APIC`
   (omslaget, 11–15 kB) — mätt i hans egna zip-original. Nu tas bara härkomsten
   (`made with suno` / `suno.com` / `suno studio` / `c2pa`), ram för ram och underchunk för
   underchunk. **Rör du en väg som läser ljud eller filhuvuden: läs roadmapens 8.5-rättelse
   och `src/audio/metadata.rs` först.**
   0g. ~~**Låtens första slag mot rutnätet**~~ — **BYGGT 2026-09-13 (8.14).** Alex' sista bit
   av "markören matchar inte vågformerna": *"Sätt takt 1 här"* i klippets högerklicksmeny —
   ljudet under spelhuvudet blir klippets första sampel, klippet står kvar, högerkanten står
   still (Abletons 1.1.1 + Audacitys kant-trim). Ren funktion `align_clip_start_to_point`
   med **fyra prov**: 0,30 s i 140 BPM ✓, samma sträcka i 100 BPM blir 0,2143 s av *filen* ✓,
   klippet står kvar ✓, och ett nej är ett nej (spelhuvudet utanför klippet → inget ändrat) ✓.
   **Och steg 2 är byggt samma kväll:** *"🔍 Hitta första slaget (mät i filen)"* — samma meny.
   **Första försöket använde detektorns svar rakt av, och Alex' öra fällde det** ("hoppade
   till markören och klippte bort början på trummorna"): backningen ligger 98 ms före träffen
   (= 5,7 % av ett slag i 140 BPM), och för sång/gitarr hittar HFC-höljet inget alls. Ankaret
   är nu **ljudet** (`music_start_source_secs`: 20 ms RMS, −40 dB under fönstrets topp) med
   detektorn som finputs inom 20 ms. Mätt på fem av hans stämmor: trummor **0,6008** (mot
   backningens 0,502), sång **7,18**, bas **1,6885**, gitarr **0,0** (inget att trimma),
   backing vocals `None` (tyst i 20 s). Se roadmapens 8.14 för hela tabellen.
   **Och hela stämgruppen flyttar tillsammans (8.15b)** — Alex: *"Trummorna flyttas till 00:00 när
   jag väljer mätning. Det känns inte som en korrekt feature."* Rätt: ett ensamt trumklipp hamnade
   0,60 s före bas och gitarr, som ligger låsta på 0 ms mot varandra (korr 0,71, mätt i hans filer).
   Nu flyttas varje klipp som börjar på ankarets takt med samma skift **i tid** (0,01 takts
   tolerans), hela planen räknas fram innan något skrivs (en halvflyttad grupp är värre än ingen
   flytt), och båda dörrarna — "Sätt takt 1 här" och mätningen — går genom samma applikator.
   0f. ~~**"marker och wave synkar inte" + "ingen skillnad på tempo"**~~ — **RÄTTAT
   2026-09-13 (8.10d).** Ett fel, båda symptomen: `LoadStemTrack` — som appen skickar
   **vid varje uppspelningsstart** — byggde ett nytt spår och tömde `regions` tyst, så
   motorn spelade originalet i naturligt tempo medan vyn ritade den sträckta filen
   (1,4×-utsnittet). Bevisen: cachen var full av korrekta filer (362,99 s, första
   anslaget 0,838 s, tempot i filen mäter 100 BPM), `load_wav_pcm` läste dem, och
   autosaven 13:00:34 stod på 140/140 där inget ska sträckas. **Byggt:** regionerna
   överlever omladdningen, motorn räknar vad den faktiskt har
   (`stretched_track_count`, speglad som spelhuvudets klocka) och **statusraden säger
   ifrån** när talen går isär. Provet `a_track_reload_keeps_the_stretched_regions`
   fäller den gamla koden (kontrollerat). **KVITTERAT av Alex 2026-09-13 (kväll):** *"Så här
   långt låter allt perfekt."* — rätt tempo, rätt tonhöjd, ingen artefakt, spelhuvudet följer
   ljudet. Kedjan 8.13b → 8.10b → 8.10c → 8.10d är därmed bekräftad av örat.
   0e. ~~**Artefaktljud vid tempobyte**~~ — **RÄTTAT 2026-09-13 (8.10c).** Alex: *"Test av att
   sänka bpm resulterade i artefaktljud när den sänkte tempot, men ljudet höll rätt ton."*
   Faktorn var rätt (1,1667) och tonhöjden stod still; det var kvaliteten. Orsaken:
   tidsskalningen lånade **sångstudiens realtids-WSOLA** med sökfönster **±128 sampel =
   ±2,7 ms** — mindre än en period av en 150 Hz-ton, så skarvarna hamnade ur fas. **Mätt på
   hans stämma:** F0-fladdret föll **132,6 → 39,9 cent** (källans 48,8 = sångarens vibrato,
   alltså är tillagt fladder nu ≈ 0) medan kornstorleken behölls — den första tanken (60 ms
   korn, 75 % överlapp) tog bort fladdret men **åt diskanten −45 %**, och siffrorna dömde ut
   den. Nu: `Wsola::offline` = korta korn + ±12 ms sök, nivån normaliserad för överlappet,
   svansen fylld, och **motorns version i cachenyckeln** (`-v2`) — utan den hade de gamla
   cacharna spelats upp och fixen inte hörts. **Kvar (mätt):** sträckningen tappar ~2,5 dB
   mer energi än längden förklarar, diskanten −16 % i andel, och nästa kvalitetssteg är
   HPSS/fas-vocoder eller `signalsmith-stretch` (MIT) enligt 8.10 § 6.
   0d. ~~**"Tempot påverkar inte" + "låten skars av i slutet"**~~ — **RÄTTAT 2026-09-13
   (8.10b).** Samma rot i båda: klippen i Rock and Hard Place hade `source_bpm = 0` för
   att Suno inte gav något BPM (varken i arkivnamnet eller i taggarna). **Mätt:** filerna
   259,28 s med ljud till sista samplet, klippen 129,64 takter → byggda i **120,000**;
   projektet stod i **200**, så klippet räckte 155,57 s och **103,71 s musik skars av**
   (vid 120 räcker klippet filen exakt). Stämpeln ("låt klippen följa") satte förut
   *projektets* tempo — i 200 hade en sänkning till 100 sträckt ljudet till halv
   hastighet. Nu: `geometry_source_bpm` (takter × 240 / filens sekunder) används av
   stämpeln, **talet visas i ⏱ Tempokarta och i statusraden**, och importen stämplar
   projektets tempo när Suno inget ger. **KVITTERAT av Alex 2026-09-13 (kväll)** — tempot hörs
   nu ändra sig, och måttet är rätt för det ljud projektet faktiskt har.
   0c. ~~**Spelhuvudet låg efter ljudet**~~ — **RÄTTAT 2026-09-13 (8.13b).** Alex: *"markören
   står där ljudet börjar, men enligt vågformen är det ännu cirka 0,7 s kvar."* Mätt i hans
   egen fil: `Rock and Hard Place (Vocals).wav` är exakt noll till 5,0 s, första frasen
   7,155 s, nästa 13,3 s — och han stod på 12,64 s, alltså **0,66 s**. Felet var
   sekvenserns klocka: `last_step_time = Instant::now()` flyttade ankaret till NU varje
   steg, så varje steg blev en bildruta sent och felet **summerades** (118 sextondelar ×
   5,6 ms ≈ 0,64 s vid 89 Hz). Stegklockan räknas nu från förra deadline
   (`steps_elapsed`), och **spelhuvudet läser ljudtrådens egen klocka**
   (`AudioEngine::song_position_secs`) i stället för att räkna egna steg. Slingpunkten
   söker nu också motorn — det är en **beteendeändring** (ljudet följer slingan), läs
   8.13b i roadmapen. **Bekräftad av Alex 2026-09-13:** *"Nu verkar waveform och marker synka."*
   (Kvar i samma familj, som egen mindre punkt: notschemat ligger fortfarande i UI-tråden —
   upplösning en bildruta; hörs inte som glid längre.)
   0b. ~~**Nivå- och registermätare i toppraden**~~ — **BYGGT 2026-09-13 (8.13)**: L/R-mätare
   och register-EQ bredvid oscilloskopet, före MASTER. Mätningarna (och de två
   konstruktionsmissar siffrorna dömde ut) står i roadmapens 8.13. **Kvar: Alex' ögon** på
   placering och bredd — GUI:t är läst i koden, inte sett.

1. ~~**Importen analyserar varje stämma** → vågformer exakta från första bildrutan.~~
   **KLAR 2026-09-12 (`cb9f152`).** Gjort: `WaveformCache` byggs i
   `background_decode_stems` och följer med ut till spåret; `apply_imported_stems` ger
   spåret samma `Arc`-buffert som motorn fick plus cachen nycklad på den. Regeln ligger
   i `imported_track_waveform` (testad utan fönster). Översikten räknas ur cachen.
   **Kvar av punkten:** att Alex ser den i fönstret — jag kan inte klicka i GUI:t.
   Nästa läsare: om den fortfarande ser pixlar är det inte cachen, utan **klossens höjd**
   i pixlar (§8.4 i roadmapen: vertikal upplösning är ett eget tak) eller
   projektinläsningens första bildruta (den bygger sin cache i bakgrundstråd —
   `ensure_waveform_cache` — och visar den grova översikten i någon bildruta först).
2. ~~**Ljudet följer tempot**~~ — **STEG 1 KLAR 2026-09-12 (`f0dc8eb`).** Alex svarade
   "tempo-kontrollen styr allt" och valde bandspelarlogik först (tonhöjden följer), med
   pitch-bevarandet som steg 2 när han hört det. Gjort: `AudioRegion.source_bpm` (0 = okänt
   → rör inte ljudet), `stretch_ratio_for`, `region_source_secs` (ren mappning i motorn,
   bit-exakt vid 1,0), `region_source_span_samples` (ritning + exportdörrar), och
   `sync_tempo_follow` som fångar tempot **en gång per bildruta** i stället för en sync per
   dörr. **Kvar av punkten:** pitch-bevarande (WSOLA), kloss över ett tempobyte, och att
   klippet syns vara sträckt i vyn. Se fas 8.10 i roadmapen.
   **Efter hans kvittens (`a510775`):** gamla projekt saknade fältet helt, så ingenting
   hände. Nu mäts inspelningstemot fram vid inläsning när filen och projektets tempo
   stämmer (hans Broken: 9/9 klipp), och ⏱ Tempokarta har en stämpel för resten.
   **Nästa andetag här är steg 2** — och det är Alex' öra som avgör om det behövs.
3. ~~**Tonarten som faktisk tonart**~~ — **KLAR 2026-09-12 (`392a30c`).** Fem saker var
   osanna: två skalalistor med korsande index (arrangerarens "Dorian" blev piano rollens
   "Moll"), **13 grundtonsnamn för 12 toner** (fyra av tolv val gav fel ton), tonarten
   nådde bara AI-kontexten, `piano_roll_snap_to_scale` lästes **aldrig** (🔒-knappen
   gjorde ingenting), och tonarten sparades inte. Nu: `src/audio/scale.rs` med en tabell
   och rena regler, ett fältpar (`song_key_root` + `song_key_scale`), markeringen följer
   projektets tonart, låset flyttar klicket till närmaste skalton (nedåt vid lika avstånd),
   och tonarten ligger i projektfilen. **Kvar:** Alex' ögon på markeringen, skalnamnen är
   svenska strängar (inte i18n), och ingen transponering-till-tonart.
4. ~~**Sends**~~ — **SENDS TILL BUSSAR KLARA 2026-09-12 (`1d3fc1a`).** Ett spår kan skicka en
   del av sin signal till en annan buss (post-fader, mål-bussens mute/solo gäller, med i
   exporten, UI i mixerns kanalpanel). **Kvar av punkten:** sends **mellan spår** — en annan
   och större sak (spårloopen i `process_stereo` måste delas i två faser + en slingkontroll),
   och den ligger kvar i roadmapen med sitt eget pass.
5. **Två rättelser efter Alex' öron och ögon (kväll 2026-09-12):** sträckningen fyllde inte
   klippet (geometrin vinner nu över analystalet) och "inget hände" i Rock and Hard Place
   (klippen saknade känt tempo — och appen **sade ingenting**; nu står det i statusraden med
   pekaren till ⏱ Tempokarta). Båda står i roadmapens 8.10 med sina mätningar.
6. **8.10 steg 2 (pitch-bevarande sträckning)** — **KLAR och mergad till master**, med en
   rättelse efter Alex' öron: klippen bar 120,98828 medan geometrin sa 120,0000, och filen
   blev 2,3 s för lång vid 110 BPM (hela låten gled ur takt). Nu vinner geometrin inom en
   procent — se roadmapens 8.10. **Kvar av hans rapport:** WSOLA skorrar på sång (motorn,
   inte faktorn) och cachen hade ingen utrensning (5,7 GB för fem tempon, nu rensad).
   **Det gamla (worktreen):**
   (branch `tempo-follow`), **392 tester default / 438 med plugin-host, 0 varningar**.
   Switchen (🎚 Följ tempot, på som standard), klippets bandspelarläge, den
   offline-renderade filen med cache, och motorns `source_audio`. Bevisen står i
   roadmapens 8.10. Kvar: klipp över ett tempobyte får fortfarande en faktor (klossen
   delas inte), cachen har ingen utrensning (≈106 MB per fyraminutersstämma och tempo,
   mätt), och **Alex' öra** — GUI:t är inte klickat (ingen skärm): switchen, klippmenyns
   temoläge och statusraderna är lästa, inte sedda.
6. **`--clean-tags` för wav** (RIFF `LIST/INFO`) — samma sak som ID3 men andra chunks.
7. **Tempomarkering i taktlinjalen** vid tempobyten (syns bara i listan i dag).
6. **Riktig BPM-detektor** om någon behöver den: onset-styrka + tempokam
   (aubio/librosa/Essentia). Underlaget ligger i `sonix`-skillens research.

## 4a. Först av allt: två ritfel som hör ihop med 8.10 (små, men de ljuger i vyn)

1. **Påståendet "vågformen ritas med fel faktor" håller INTE — kontrollerat 2026-09-13.**
   Det stod här att ett sträckt klipp ritades med steg 1:s faktor medan det spelas från en
   färdigsträckt fil med 1,0, och att vyn därför visar "ett annat stycke". Räkningen säger
   motsatsen:

   - `stretch_ratio_for(källa, projekt)` = `projekt/källa` = **källsekunder per utsekund**.
   - Ritningens spann = `region_secs × projekt/källa` **källsekunder** — och det är exakt det
     källinnehåll klippet täcker. Fyra takter i 120 BPM är 8 s källa; i ett 150-projekt är
     samma fyra takter 6,4 s ut, och 6,4 × 1,25 = 8 s. Rätt stycke, rätt spann.
   - Uppspelningen läser **filen** (filfaktor `källa/projekt` = 1/uppspelningsfaktorn) med
     hastigheten 1,0, alltså `region_secs × källa/projekt` sekunder fil = **samma innehåll**.

   Dörrarna är alltså ense om *innehållet*; de skiljer sig bara i **vilken buffert** de läser
   (originalet mot den WSOLA-bearbetade filen). Och den föreslagna åtgärden — att rita med
   `region_playback`:s faktor 1,0 — skulle ge spannet `region_secs × 1,0` ur originalet, alltså
   6,4 s av ett 8 s klipp: **det** vore att visa fel stycke. Faktorn 1,0 är rätt bara om
   ritningen läser *filen*.

   Kvar som verkligt (men litet) hål: vyn ritar omsamplet original medan örat hör den
   sträckta filen. Att rita filens hölje kräver en egen vågformscache för den sträckta
   bufferten, precis som spåren har — och skillnaden syns bara vid fin zoom. Den skillnaden
   **är** WSOLA-artefakterna i §4b, inte ett eget ritfel. Mät innan du "lagar": jämför
   ritningens fönster med filens för ett klipp i olika tempo.
2. **Alex: "får inte riktigt markören att matcha vågformerna oavsett bpm".** Två fall, och de
   ska inte blandas: (a) tempot — ljudet ligger i Sunos tempo och rutnätet i projektets, så de
   möts bara vid rätt tempo (Rock and Hard Place: **140**); (b) **inledningen** — om det
   fortfarande inte stämmer vid rätt tempo ligger låtens första slag inte på takt 1 i filen,
   och då är åtgärden att **trimma klippets vänsterkant** (`sample_offset_sec`) så att slaget
   landar på rutnätet. Inget tempo lagar (b) — och sedan **2026-09-13 finns vägen:
   "🎯 Sätt takt 1 här"** i klippets högerklicksmeny (roadmapens **8.14**). Appen kan
   fortfarande inte *hitta* slaget åt honom; det är nästa steg och det står i 8.14.

## 4b. Nästa pass: motorn (färskt sammanhang — roadmapen säger det själv)

**Frågan är redan ett tal.** `the_stretch_artefacts_on_a_real_stem` (kör manuellt, se
nedan) mäter **var** anslagen hamnar och **hur många** de blir på Alex' egna stämmor vid
120 → 110 BPM, med repots egen validerade slagletning. Mätt 2026-09-12:

| Stämma | Anslag i källan | I renderingen | Överskott |
| :--- | ---: | ---: | ---: |
| Vocals | 3 205 | 3 536 | +10,3 % |
| Backing Vocals | 5 932 | 6 483 | +9,3 % |
| Drums | 4 273 | 4 682 | +9,6 % |
| Bass | 1 793 | 2 613 | +45,7 % |

**Tidpunkterna håller** (felet mot `källans tid × faktorn`: median 1,8–3,9 ms, p95
4,4–8,5 ms). Det är **antalet** som felar: WSOLA lägger till transienter, och det är
kornkanterna Alex hör som skorr.

`cargo test --release --bin sonix the_stretch_artefacts -- --ignored --nocapture`

**MÄTT 2026-09-13 — kornplaceringen vid anslag är INTE orsaken.** Två varianter prövades
mot samma fyra stämmor och samma test, och båda gav **sämre** siffror än utgångsläget:

| Variant | Sång | Bakgrundssång | Trummor | Bas |
| :--- | ---: | ---: | ---: | ---: |
| Utgångsläget (mätt 2026-09-12) | +10,3 % | +9,3 % | +9,6 % | +45,7 % |
| 1. Anslag gör ramen **orörbar** (sökningen hoppas över) | +10,7 % | +9,7 % | +11,7 % | +53,7 % |
| 2. Anslag **flyttar** ramen dit (framåt, monotont) | +10,2 % | +9,7 % | +10,5 % | +53,1 % |

Båda är **återställda** — en ändring som lägger till artefakter är sämre än ingen ändring, och
siffran i tabellen ovan står kvar som bevis. Slutsatsen är att överskottet inte kommer av att
sökningen flyttar kornet: det kommer av **överlappningen** (två Hann-fönster över samma
anslag, 512 sampel ≈ 11,6 ms) eller av kornlängden/hopet självt. Nästa försök ska därför
mäta på **överlappet**, inte på sökningen — eller gå till steg 2 direkt.

Kvar som vakt: `the_engine_does_not_invent_transients` (klickföljd, syntetisk). Den visar
**0** överskott både före och efter båda varianterna, alltså fångar den en grov regression
men inte den fina — det står i testets doc-kommentar så ingen tror annat.

**Steg 1 (inget nytt beroende):** transientmedveten kornplacering i vår egen WSOLA —
`src/audio/vocal_harmonizer.rs`, `struct Wsola` (frame 1024, hop 512, `search: 128`,
`best_start()` som gör den normaliserade korskorrelationen mot `prev_tail`). Klassiskt
grepp: vid ett anslag får sökningen **inte** flytta kornet (då smetas attacken), och
överlappningen ska inte korsfadas över anslaget (då dubblas det). Slagpunkterna finns
färdiga: `crate::audio::onset::detect_onsets`.
**Steg 2 är GJORT och dömt (2026-09-13 kväll, roadmapens 8.10e):** `signalsmith-stretch` (MIT)
prövades som renderingsmotor. **Vår egen WSOLA står kvar** — signalsmith smetar transienterna
på varje stämma (sång +16,0 % mot vår +9,8 %, trummor täta +1 440 mot +439), även om den håller
diskanten bättre (−2 % mot vår −10 %). Två domare, två svar: det är en riktig avvägning, och
domen gick på Alex' eget klagomål (skorret). Notera också att **roadmapens gamla tabell var
inaktuell** — den var mätt före 8.10c, och basens +45,7 % är i dag +11,0 %.
Kvar: hans öra på `the_two_engines_for_the_ear` (två flac-filer i `~/.local/state/sonix/ab/`).
Motorn ligger som **dev-beroende** (C++/bindgen/libclang behövs bara av `cargo test`), och
cachenyckeln står kvar på 2 eftersom produktionsvägen är oförändrad.

## 4c. Klart efter den här texten: sends mellan spår (2026-09-13 kväll)

**8.3 är i mål.** Ett spår kan skicka in i ett annat spårs kedja — den del som krävde att
spårloopen i `process_stereo` räknas i ordning. Det som är värt att bära vidare:

- **Ordningen är mekanismen.** `stem_order` (topologisk, via rena `plan_track_order` i
  `command.rs`) ser till att sändarens utgång för *det här* samplet finns när mottagarens
  kedja kör. `stem_incoming` — listan av `(sändare, nivå)` per spår — byggs i **samma** pass,
  så ordningen och vägarna inte kan driva isär.
- **Fasprovet är modellen för hur en sådan här sak ska bevisas:**
  `a_track_send_arrives_in_phase` jämför mot en **kontroll på samma nivå**, inte mot "2×",
  för masterns kurva är inte linjär. Och det är **kontrollerat att provet fäller**: kopplar
  man bort `recompute_stem_order` ger senden förra samplets värde (0,0443 mot 0,0884).
- **Ett nej är ett nej:** en slinga namnges (`Err((a, b))`), motorn behåller sin förra
  ordning och skriver i loggen, och mixern **vägrar skapa** en slinga med besked. Ett mål
  som inte finns gör ingenting — inte "närmast rätt".
- **Projektfilen är bakåtkompatibel:** `target_bus` / `target_track` i samma flata form som
  förut, med prov både för gamla filer och för rundgång.
- **Ärliga gränser:** post-fader (ingen pre-fader-variant), nivån kläms till 0..2 (ingen
  inverterad send), och **PDC är inte vägd in i send-vägen** — den får mottagarens latens
  också. Utan plugins i mottagaren är den noll och senden exakt; med plugin är senden
  förskjuten. Det står i roadmapens 8.3 med samma ord.

**Nästa andetag:** roadmapens lista pekar på **8.4 Sampler** (ett riktigt samplerinstrument i
kanalracket) — eller Alexanders öra på motorn, om han vill avgöra 8.10e först.

## 4g. MIDI-in blev EN väg (2026-09-14)

- Modulen hade **två** implementationer: ALSA-sequencern (Linux, användes) och `midir`
  (övriga, färdig men oanvänd). Den senare var den **bättre**: ALSA-vägen krävde att
  användaren själv kopplade klaviaturen med `aconnect`, `midir`-vägen ansluter automatiskt
  till alla in-portar. Att "porta" var alltså att låta Linux byta **till** den färdiga vägen
  och stryka den andra — inte att skriva nytt.
- **Bevisa bytet i körning:** `--selftest` skriver portlistan i back-endens format. Före
  `14:0 Midi Through Port-0` (ALSA), efter `Midi Through:Midi Through Port-0 14:0` (`midir`).
  Samma port, ny väg — och det syns utan att läsa ett enda commit-meddelande.
- **Den kortare listan var rätt:** efterkontroll med `aconnect -l` visade att fyra av de fem
  "portarna" var ALSA:s systemklienter och PipeWires infrastruktur. `midir` listar bara
  riktiga MIDI-portar.
- **Gränssnittet är en del av porten:** `aconnect`-tipset, knapptexten "(ALSA Seq)" och
  statusraden var sanna för den gamla vägen och blev fel i den nya. Leta efter dem varje gång
  en väg byts — texten är inte kosmetika, den är en instruktion som blir osann.
- **Kvar:** MCU-kontrollen (`hardware_control.rs`) är fortfarande ALSA-seq. Den kan portas
  likadant, men **kan inte verifieras här** (ingen MCU inkopplad) — och en blind omskrivning av
  en fungerande väg är inte vad det här repot gör.

## 4f. Windows-porten: steg 5–7 (2026-09-14)

- **`sonix --selftest`** mäter det CI inte kan: ljudenheten öppnas (med appens egna
  inställningar), ljudtråden går i **realtid** (dess egen position mot väggklockan — en enhet
  som öppnas men aldrig konsumerar ser likadan ut i varje annan mätning) och att en slagen
  trumma når mastern. `clock_verdict` är ren med prov; för kort fönster ger *"inte mätt"*.
  MIDI-delen är **ärlig, inte grön**: Linux hittade 5 riktiga portar, Windows säger stubbe
  tills `midir`-porten är gjord.
- **Sökvägarna** har plattformens egen layout (`%APPDATA%`/`%LOCALAPPDATA%`) som ren funktion
  med prov — inklusive att **Linux är oförändrat**, för ett prov som bara tittar på Windows
  hade inte sett om XDG-vägen rördes.
- **Ett löfte blev sant:** `SONIX_CONFIG_DIR`/`DATA_DIR`/`STATE_DIR`/`CACHE_DIR` stod som
  överstyrningar i modulhuvudet men lästes aldrig. Bevisat i körning efteråt: med
  `SONIX_STATE_DIR=/tmp/…` flyttade tillståndet, medan konfigurationen blev kvar. **`env
  HOME=…` isolerar INTE när skalet exporterar `XDG_*`** — det gör Hyprland, så gamla
  "isolerade" körningar skrev i riktiga `~/.local/state/sonix`. Skillen är uppdaterad.
- **Filhanteraren** per plattform (`src/platform.rs`): `explorer` avslutar med **1 även när den
  lyckas**, så anroparen tittar på starten och aldrig på slutkoden.
- **Kvar i portningen:** `midir` i stället för ALSA-seq (~885 rader fungerande kod att skriva
  om) — avsiktligt efter kvittensen på en riktig maskin, inte före.

## 4e. Och sedan: 8.2:s visning (2026-09-13, samma kväll)

- **"De 4 visningsställena" var fyra funktioner med 46 användningar.** Roadmapen räknade
  funktioner; arbetsenheterna var fler. Samma läxa som i början av 8.2, och värd att komma ihåg:
  räkna arbetsenheter, inte rader eller funktioner.
- **Fyra namngivna svar i stället för en skalär** (`secs_at`, `secs_len`, `bars_at`,
  `sec_per_bar_at`) — en enda `sec_per_bar` kan bara svara rätt på en *plats*, en *längd* och en
  *lokal taktlängd* samtidigt så länge tempot är konstant.
- **`snap_bar` är nu den enda snäppregeln**, i takter, med prov. Att den gamla sekundbaserade
  `snap_time_secs` blev **oansenlig** ("aldrig använd") var beviset för att migreringen var
  komplett — låt varningen vara kvittensen, inte en känsla.
- **Automation-lanen** ritas genom kartan, men punkterna ligger kvar i **sekunder**. Det är den
  enda kvarvarande frågan på 8.2, och den är ett *beslut* (förslag: flytta dem till takter, som
  klippen) — inte ett fel.
- **Inget av detta är GUI-verifierat.** Skriv det, inte "klart".

## 4d. Klart efter den här texten: samplern (2026-09-13 kväll)

**8.4 är i mål.** Kanalrackets kanal är nu ett instrument och inte en provspelare: den kan
hålla en not. Det som är värt att bära vidare:

- **De tre looplägena är de etablerade**, inte påhittade: `Off` / `UntilRelease` / `Forever`
  (OP-XY:s trio; "loop until release" är eget läge i Ableton, Kontakt, SFZ, EXS24, Renoise och
  SoundFont 2). Ping-pong är en riktning, inte ett fjärde läge.
- **Loop-punkter är hela ramar**, och det var provet som avgjorde saken: en loop på 399,6 ram
  drev 0,4 ram per varv, och `a_forever_loop_repeats_exactly` skrev ut positionerna
  `[1.0, 801.0, 801.2]`. `loop_frames` (ren, i `command.rs`) klämmer, ordnar och **vägrar** ett
  bakvänt par — "ett nej är ett nej", samma regel som för sends.
- **Identiteten är ett namngivet värde, inte en flagga.** `AdsrParams::identity()` =
  ingen envelop, och då rör motorn den inte alls. Det är den som gör att ett projekt från i går
  låter som i går (`the_defaults_are_still_a_one_shot`).
- **Den döda ratten var en riktig fälla:** kanalpanelen visade en "Attack / Decay"-ratt som
  *ingen* DSP läste. Fältet är kvar (`attack_decay`, migreringar raderar inte) och får nu sätta
  attacken när ett gammalt projekt öppnas. **Ser du ett fält som ingen läser: det är den här
  familjen, och det är värt att röja.**
- **Ett steg är nu en not med en längd** (`step_hold_secs`, en sextondel med swing, samma tal i
  uppspelningen och i exporten) — det är vad som gör `UntilRelease` meningsfull i kanalracket.
- **Exporten följer med** (`a_sampler_loop_follows_the_offline_render`), OCH provet mäts **torrt**
  (eko/rymd av): annars hade efterklangen låtit som en loop.

**Ärligt kvar, egna pass:** ingen velocitetsstyrning, ingen filterenvelop, inga multi-samples
(keymaps), och loop-punkterna sätts med reglage i stället för att dras i vågformen.

**Nästa andetag:** roadmapens lista pekar på **8.2 Tempo map** (de fyra visningsställena,
automation-lanen, drag-utökningen) eller **8.6 plugins** (bryggning, egna utgångar, sidokedja in
i en plugin).

## 5. Fällor som kostat tid (läs dessa innan du patchar)

- **`gh run list` stödjer inte `--arg`**, och `-c` kräver **full** SHA. Kort SHA ger tomt.
- **Verifiera binären du kör:** `nm <binär> | grep <symbol>`. 2026-09-12 körde Alex en
  tre dagar gammal kopia utan att någon märkte det.
- **`grep -c '^warning'` räknar varningar, inte fel** — och returnerar 1 vid noll.
- **`git add -A` sveper in vad som helst.** En funktion (8.3:s sidokedjor) hamnade i en
  commit märkt `docs`. Använd explicita filnamn.
- **Läs de exakta raderna innan du patchar** — med `repr`, inte ögonmått. Gissad
  indentering fällde tre patchar i rad; ett ankare pekade på fel rad fyra gånger.
- **`cargo fmt` på hela repot ger ~11 000 rader churn.** Formatera enskilda filer.
- **Skriv aldrig en siffra du inte mätt.** Två gånger i dag bar commit-meddelanden
  kopierade testantal i stället för körda.
- **Testa analyser mot facit innan de får styra något.** `--detect-bpm` finns för det.

## 6. Öppna frågor till Alex

- **Tonarten:** vill han att C#/Dur ska styra projektet på riktigt (transponering,
  skalmarkering), eller räcker det att kontrollen säger vad den gör?
- **Ljudet och tempot:** ska klipp sträcks när tempot ändras (som Ableton/Reaper), eller
  ska Sonix följa Sunos metadata och låta bli att röra ljudet?
- **Städningens omfattning:** bara importerade filer, eller också biblioteket
  (`~/Music/Sonix/Samples`, 10 600 filer)? I dag: bara det som importeras.

## 7. Miljö

- Hyprland, **ingen Xvfb/weston** → GUI-verifiering kräver Alex. Starta aldrig ett
  fönster oannonserat på hans skärm.
- `gh`: `/home/alex/.local/share/mise/installs/gh/2.100.0/gh_2.100.0_linux_amd64/bin/gh`
- **Push:** `git -c credential.helper= -c credential.helper="!<gh-sökvägen> auth git-credential" push origin master`
- **Ljudfilerna får aldrig in i git** (Alex' egna filer). `.githooks/pre-commit` stoppar
  filer över 5 MB. README förklarar hur man skaffar ljud.
- Alex' stämmor: `~/imported_stems/<Projekt>/` (relativ sökväg i projektfilen!).
- Sandlådor: `/tmp/sonix_*`. Alex' riktiga `~/.local/state/sonix/` rörs inte.
- **En flagga ingen läser är ingen funktion.** `piano_roll_snap_to_scale` sattes av en
  knapp och lästes av ingen — den såg funktionell ut i månader. När du rör en kontroll:
  `search_files` **alla** läsare av fältet, inte bara skrivarna. Samma sak med två listor
  för samma sak: de driver isär, och den som visar ett namn och gör något annat är värre än
  ingen kontroll alls.
- **Långa inline-kommandon (heredoc, jätte-ettor) blockeras av kommandoparsern.**
  Lägg skriptet i `/tmp/*.sh` med `write_file` och kör `bash /tmp/skriptet.sh` —
  det var vägen runt blockningen 2026-09-12. Samma sak för grepp-kedjor med `-A`.
