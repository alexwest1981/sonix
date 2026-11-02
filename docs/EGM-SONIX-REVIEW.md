# EGM-genomgång av Sonix — 10 konkreta förbättringar

Underlag: fem fristående granskare (färsk kontext, ingen tidigare chatt) har läst koden ur fem vinklar —
Suno-flödet, samplingsfrekvens/tempo, UI-layout, struktur/CI och realtidsprestanda. Varje påstående nedan
är **kontrollerat i källkoden** av mig efteråt; siffrorna är granskarnas mätningar på den här maskinen
(release, `--no-default-features`, 128-frame block @ 48 kHz = **2,667 ms budget**).

Baslinje vid genomgången: `b1b9cce`, 195 `.rs`-filer / 166 860 rader, testsviten 1 377 prov varav
**1 355 gröna, 2 röda** (båda videoexporten, lastkänsliga — gröna enskilt). Bygget tar 2 m 14 s kallt.

Rangordningen är "vad som biter användaren först", inte "vad som är lättast".

---

## 1. 🔴 Neural separation spelar 147 cent för lågt på allt som inte är 44,1 kHz

**Symtom:** en importerad 48 kHz-fil (eller 96 kHz) låter som en smurf efter separation — samma klass av
fel som det jag nyss fixade för Suno, men i en annan ände av kedjan.

**Mekanism:** `neural_separator.rs:608` resamplar **in** till `MODEL_SAMPLE_RATE = 44_100` (`:26`) och
resamplar aldrig tillbaka, men `stem_separator.rs:355` rapporterar anroparens frekvens. 48 kHz-material
spelas då i 44 100/48 000 = **0,919×** (147 cent lågt, 8,8 % längre); 96 kHz blir **0,459×**.
`write_stems_to_dir` skriver samma felaktiga frekvens i WAV-huvudena, så filen på disk är också fel.

**Minsta rätta fix:** när `used_neural`, sätt `SeparationResult.sample_rate = MODEL_SAMPLE_RATE` (data och
etikett stämmer då), eller resamplea varje stämma tillbaka med `resample_sinc(…, MODEL_SAMPLE_RATE, sample_rate)`.
Kommentaren i `resample.rs:6-7` lovar redan "in and out" — den ut-resamplingen finns inte.

**Kontrollerat:** ja. **Insats:** en kväll, två rader plus ett prov som mäter frekvensen på utstämman.

---

## 2. 🔴 Pitchdetektorn körs varje bildruta och håller mikrofonens mutex

**Symtom:** mic-monitor hackar, inspelade tagningar får hål, UI:t hackar — så fort en mic-ström finns.

**Mekanism:** `ui/app/modals.rs:2011` anropar `detect_live_pitch_hz()` obetingat varje frame →
`recorder.rs:1019` tar `analysis_buffer.lock()` → autokorrelationen (`recorder.rs:590`) är O(fönster×872
laggar), och mic-callbacken håller **samma lås** under hela sitt block (`recorder.rs:527`).

**Mätt:** **4,30 ms per anrop** (48 kHz, 16 384 prov) = **1,6× blockbudgeten**, varje frame.

**Minsta fix:** kopiera bufferten under låset, analysera utanför det, fönster 1 024, kör i 10 Hz.

**Kontrollerat:** ja (anropet finns på den rad granskaren pekar ut). **Insats:** en kväll.

---

## 3. 🔴 En heap-allokering per sample i stämmixningen

**Symtom:** sporadiska klick/dropouts med många spår. `malloc` är inte realtidssäker.

**Mekanism:** `synth/process.rs:480` bygger `key_taps: Vec<(f32,f32)>` med `collect()` **inuti**
`process_stereo` — två buffertar längre ned i samma fil står det uttryckligen att inget allokeras per sample.

**Mätt:** **1,00 allokering/sample = 48 000/s**. 32 spår: medelblock 0,290 ms, **värsta 4,289 ms**
(> 2,667), **2 av 1 500 block över budget**. Med `Vec`:en flyttad till ett återanvänt fält: 0 allokeringar,
medel 0,252 ms, värsta 1,39 ms, **0 av 1 500 över**.

**Minsta fix:** `self.key_taps` med `take`/`clear`/`extend`/restore — samma mönster som `track_out_l`.

**Kontrollerat:** ja (`:480`). **Insats:** en timme, plus ett prov som räknar allokeringar.

---

## 4. 🔴 Instrumentladdning gör fil-I/O och VM-kompilering på ljudtråden

**Symtom:** ett hårt klick och ~17 ms tystnad när man byter instrument medan det spelar.

**Mekanism:** `synth/commands.rs:504` → `jsfx/processor.rs:47` (`std::fs::read` + `Jsfx::compile`) körs i
kommando-dräneringen, dvs på ljudtråden.

**Mätt:** **17,3–18,6 ms** för en 60 kB JSFX = **6,5 missade buffertar**.

**Minsta fix:** bygg på UI-tråden och lämna över färdigt objekt, precis som `SetTrackPlugin` redan gör.

**Kontrollerat:** ja. **Insats:** en kväll.

---

## 5. 🔴 Ett uppmätt tempo på 0,0 skriver över projektets BPM — och klipper regionen till 32 %

**Symtom:** efter separation kan en region bli en tredjedel så lång som ljudet, utan förvarning.

**Mekanism:** `stem_separator.rs:476` sätter `self.bpm = result.bpm` utan `> 0.0`-vakt. `estimate_bpm`
returnerar 0,0 för material utan tydlig puls (pad, a cappella). `export_separated_stems` bygger då takter
ur `TempoMap::single(self.bpm.max(40.0))` (`export.rs:66`) = 40 BPM, medan uppspelningen använder
`tempo_map()` = orörd 126 BPM → **0,317×** längd.

**Minsta fix:** `if result.bpm > 0.0` och gå genom `set_project_bpm`, så `tempo_points` förblir enda källan.

**Kontrollerat:** ja. **Insats:** en timme.

---

## 6. 🔴 Suno-kedjans tempo mäts aldrig — och "fel" tempo används tyst

**Symtom:** stämmorna hamnar i otakt mot resten; ingen varning.

**Mekanism (två fel som göra samma sak):**
- `detect_bpm_from_file` (`stem_separator.rs:999`) anropar **`load_wav_pcm`**, som kräver RIFF/WAVE-huvud.
  Suno levererar **mp3**, så anropet faller alltid, `.ok()` sväljer felet och `unwrap_or(self.bpm)`
  (`ai_actions.rs:966`) använder projektets tempo. Kontrollerat i koden: ja.
- Stämmor → arrangemang sätter `source_bpm: self.bpm` (`export.rs:84`) i stället för det uppmätta
  `stem_project.bpm`. Vid 125→128 blir det **1,4 s drift per minut**.

**Minsta fix:** använd `load_audio_pcm` (den läser mp3) i `detect_bpm_from_file`, och skicka
`stem_project.bpm` som `source_bpm` i exporten. Mätningen ska dessutom skrivas ut i panelen före export.

**Insats:** en timme för båda; ett prov som matar en 44,1 kHz mp3 genom tempomätaren.

---

## 7. 🔴 Suno-panelen ritar samma block två gånger

**Symtom:** två förloppsindikatorer, två "stämmor klara"-rader, två exportknappar som gör **olika** saker,
och Audio/Inspo-listan dyker upp två gånger — sidan hoppar när man väljer.

**Mekanism:** `suno_panel.rs:520-554` och `:558-600` är samma block kopierat; `suno_pick_list(ui)` anropas
både `:437` och `:555` (**kontrollerat: 2 anrop**). `:548` exporterar till tidslinjen, `:585` importerar
till bordet med `bpm = 0.0` och rensar `stem_files`.

**Minsta fix:** radera det andra blocket och det andra anropet.

**Insats:** fem minuter.

---

## 8. 🟡 Det man skriver före inloggning kastas bort

**Symtom:** man skriver låttext och stilar i panelen, loggar in — och fälten är tomma.

**Mekanism:** panelen bjuder in till arbete innan anslutning (`suno_panel.rs:199-208`, reservschema), men vid
`Connected` ersätts formuläret helt: `self.suno_form = fields_from_schema(...)` (`ai_actions.rs:1037`).
`fields_from_schema` (`suno_form.rs:57`) bygger nya fält utan sammanslagning.

**Minsta fix:** flytta över gamla `value` per fältnamn när schemat byts.

**Insats:** en timme.

---

## 9. 🟡 Kreditgrinden är inkonsekvent: lyssning kostar utan bekräftelse, och samma låt kan kosta två krediter

**Symtom:** "🎧 Lyssna" drar en kredit direkt på klick (två tagningar = två krediter för att jämföra), medan
biblioteket kräver ett uttryckligt **✅ Bekräfta — 1 kredit** för exakt samma anrop. Lyssnar man sedan och
importerar samma låt debiteras man igen, eftersom `save_song` alltid skapar en ny mapp (`suno.rs:1499`).

**Mekanism:** `ai_actions.rs:896` (lyssning) mot `suno_panel.rs:706-715` (bibliotekets grind), och
`suno_import_confirmed` (`:984`) som hämtar på nytt utan att se om filen redan finns.

**Minsta fix:** återanvänd `suno_credit_song`-grinden för lyssning, och återanvänd redan hämtad fil
(`suno_last` / sparad sökväg) i stället för att ladda ner igen.

**Insats:** en kväll.

---

## 10. 🟡 Trånga rader klipps utan att kunna scrollas

**Symtom:** i dockad kanalpanel syns M/S, rattar, namn och ~4 av 16 stepknappar — resten är avskuret, och det
går inte att scrolla i sidled. Lägger man till fler patterns växer radan förbi högerkanten och de sista
knapparna går inte att klicka.

**Mekanism:** stepraden är `ui.horizontal` (`mixer.rs:1076`) med 200 px namn (`:1100`) och 16×26 px steg
(`:1163`) ≈ 980 px, medan dockan är ≥320 px och bara scrollar vertikalt (`frame.rs:1797`, `:1826`).
Patternradan är icke-brytande `horizontal` på två ställen (`mixer.rs:1003-1018`, `frame.rs:1531-1561`).

**Minsta fix:** `ScrollArea::horizontal()` runt stepraden; `horizontal_wrapped` i rackets patternrad och egen
horisontell scroll i verktygsraden.

**Insats:** en timme.

---

## Utanför de tio (värda att veta)

- **CI är grönt av fel skäl:** ingen workflow installerar ffmpeg, så `fixture()` (`video_engine.rs:323`)
  skriver "skipping" och returnerar `None` — **17 videoprov rapporterar ok genom att hoppa över**. Samtidigt
  är det neurala jobbet `continue-on-error: true` fast `neural` är default. Kontrollerat i koden: ja.
- **18 600 rader UI utan ett enda prov:** `mixer.rs` 4 610, `modals.rs` 4 486, `arranger.rs` 3 705,
  `app.rs` 1 765 (0 `#[test]`, men 275 ändringar på 60 dagar).
- **En binär, ingen `lib.rs`:** varje ändring kompilerar om allt (2 m 14 s) och proven kan inte delas upp.
- **Åtta riktiga stretch-prov är `#[ignore]`:** tempovägen användaren känner har ingen grind.

## Så här är listan kontrollerad

Varje `fil:rad` ovan är uppslagen i koden efter granskningen: dubbelanropet (2 st), `load_wav_pcm` i
`detect_bpm_from_file`, `source_bpm: self.bpm`, `MODEL_SAMPLE_RATE`-resamplingen in, `detect_live_pitch_hz`
i frame-loopen, `key_taps`-allokeringen i `process_stereo` samt ffmpeg-skippen i `fixture()`. Siffrorna i
#2, #3 och #4 är granskarnas mätningar på den här maskinen; de är inte återupprepade av mig.
