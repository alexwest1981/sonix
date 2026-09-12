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
- **Tester:** 344 default / 390 med `--features plugin-host`, **0 varningar** i båda.
  CI fäller numera **alla** ben på varningar, inte bara Windows.
- **Senaste commit:** `fc5d830`
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
- **Cachebygget (release):** 1 min 233 ms / 4 min 972 ms / 10 min 2,54 s (debug-mätt).
  Byggs i arbetstråd och nycklas på buffertens identitet.
- **`write_with_ffmpeg` kodar från en temp-wav som `write_wav` skriver** — därför är
  Sonix egna exporter redan fria från främmande taggar, per konstruktion.
- **Frystabell:** ofruset 1,4 %/2,2 % → fruset 0,3 %/0,3 % (~4,7×).

## 4. Nästa steg, i ordning

1. **Importen analyserar varje stämma** → vågformer exakta från första bildrutan.
   *Detta är Alex' senaste önskemål och det som återstår av importflödet.*
   - Plats: `background_decode_stems` i `src/ui/app.rs` (importens EGEN avkodning —
     inte projektinläsningens, som ligger i `load_project_file`).
   - Efter avkodningen: `envelope_per_pixel` **en gång** per stämma.
   - Skicka med den ut med spårdatan (`PreloadedTrackData`) och sätt på spåret.
   - **Räkna en gång, rita sedan** — inte "rita finare ur samma grova data".
   - Läs strukturen FÖRST. Det är här jag stannade.
2. **Ljudet följer tempot** (`time_stretch`-kopplingen, se §3).
3. **Tonarten som faktisk tonart:** `song_key_scale` når i dag bara AI-kontexten
   (`refresh_ai_context`); `piano_roll_root_note` styr piano roll. Koppla skalan till
   piano roll så Dur/Moll betyder något, eller döp om kontrollen.
4. **`--clean-tags` för wav** (RIFF `LIST/INFO`) — samma sak som ID3 men andra chunks.
5. **Tempomarkering i taktlinjalen** vid tempobyten (syns bara i listan i dag).
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
