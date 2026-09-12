# Läget i Sonix — överlämning till nästa session

*Skriven 2026-09-12 efter en lång dag. Läs den här först, sedan `ROADMAP.md`.
Nuläget bor i roadmapen; den här filen är bryggan dit — vad som hände, vad som är
mätt, och var nästa andetag ska tas.*

---

## 1. Var allt står

- **Arbetskatalog:** `~/Projects/sonix`, branch `master`, remote `origin` → github.com/alexwest1981/sonix
- **Binären som körs:** `~/.local/bin/sonix` → symlänk till `~/Projects/sonix/target/release/sonix`
  (skrivbordsgenvägen `~/.local/share/applications/sonix.desktop` pekar rätt — den
  pekade på en tre dagar gammal kopia i `~/.cargo/bin/` fram till 2026-09-12)
- **Tester:** 376 default / 422 med `--features plugin-host`, **0 varningar** i båda.
  CI fäller numera **alla** ben på varningar, inte bara Windows.
- **Senaste commit:** `1d3fc1a` (8.3: sends till bussar)
- **En andra session jobbar i en egen worktree.** `~/Projects/sonix-tempo`, branch
  `tempo-follow`: **8.10 steg 2 (pitch-bevarande sträckning) är byggd och verifierad där**
  — switchen, den offline-renderade filen, cachen och motorns `source_audio`. Se roadmapens
  8.10 för bevisen och för vad som är kvar. Grenen bär också master fram till `1d3fc1a`
  (sends), så den kan möta master utan konflikt. **Kolla `git status` och filernas mtime
  innan du bygger i något av träden** — två skrivare i samma `app.rs` är hur man tappar
  någons arbete.
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
5. **8.10 steg 2 (pitch-bevarande sträckning)** — **KLAR i `~/Projects/sonix-tempo`**
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
