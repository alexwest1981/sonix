# EGM-genomgång 2 av Sonix — optimering, refaktorisering och struktur

Underlag: fem fristående granskare (färsk kontext, ingen tidigare chatt) har läst koden ur fem vinklar —
GUI-trådens per-frame-arbete, ljudtrådens realtidssäkerhet, monolituppdelning, bygg/test-arkitektur och
dold dubbelräkning. Varje påstående nedan är **kontrollerat i källkoden av mig efteråt**; siffror märkta
*uppmätt* kommer från en körning på den här maskinen, inte från en granskare.

Baslinje: `b3827da`, 195 `.rs`-filer / 167 756 rader (126 759 icke-test), **573 paket** i `Cargo.lock`,
1 467 `#[test]`, 17 `#[ignore]`, en binär utan `src/lib.rs`. `SonixApp` har **415 fält** och det finns
**250 `impl SonixApp`-block** i icke-testfiler. Sju funktioner är ≥800 rader.

---

## 1. 🔴 Release-vägen byggde inte alls

**Symtom:** `cargo build --no-default-features` faller på `error[E0433]: cannot find resample in audio`.
CI har varit röd på **alla tre jobben** sedan 13:14 i dag (körningar `41411346040`, `41451248906`,
`41463193209`) — inklusive release-jobbet, som är det som gör binärerna.

**Mekanism:** `src/audio/mod.rs` gated: `pub mod resample;` bakom `feature = "neural"` med motiveringen
att bara den neurala separatorn omsamplar. Men `neural_separator::stems_at_rate` (dit fixen för 147 cent
flyttade omsamplingen) kompileras **obetingat** och anropar `crate::audio::resample::resample_sinc`.
Modulen är ren Rust utan ONNX-beroende — gaten var fel, inte anropet.

**Fix:** tagit bort gaten. `cargo check` är grön för `--no-default-features`, `plugin-host`, `jack` och
default. Releasebygget verifierat separat.

**Lärdom:** den förra rundans egen fix (#1, samplingsfrekvensen) införde felet, och ingen av de 1 368
gröna testerna kunde se det — de byggs med default-features. CI:s `--no-default-features`-jobb såg det,
men ingen läste körningen.

---

## 2. 🟡 Vågformen ritades för hela regionens bredd — varje bildruta

**Symtom:** långa klipp kostar lika mycket oavsett hur mycket som syns; värst inzoomat.

**Mekanism:** `arranger.rs:1891` (`draw_start_x = r_rect.min.x`) och `:1922` (`cols` ur hela bredden)
ritar en `line_segment` per bildpunkt utan att någonsin fråga `ui.clip_rect()`. En region i
`~/Music/Sonix/Projects/Broken.sonix` är **127 takter**; vid standardzoom (60 px/takt) är den
**7 620 px bred**, och filen har nio sådana regioner = **68 580 linjesegment per bildruta**, 2,06 M/s vid
30 fps. Vid 4× zoom blir det 274 000 per bildruta. Den gamla reservvägen (`waveform_peaks`, rad 2059)
hade samma fel.

**Mätt i appens egen fasmätare** (`logs/frame_spikes.log`, 1280×800, defaultzoom, startbildrutor):
`arbetsyta` låg på **0,8–1,0 ms**. Klippningen är alltså en **skalningsfix**, inte beviset på att
defaultvyn hackar: vinsten växer med zoom och antal klipp, och uteblir när allt redan syns.

**Fix:** regionen klipps mot `ui.clip_rect()`; bara det synliga spannet skickas till `envelope_at` med
samma bildpunkter per pixel (kolumnantalet följer den synliga bredden). Slingade regioner lämnas orörda
(deras bildpunkter följer upprepningen), och zoomregimen räknas fortfarande på **hela** regionen, så
klippningen kan inte byta representation vid en skrollkant.

**Kvar att se med ögat:** appen mappar inget fönster under Xvfb här, så skrollkanten är verifierad med
beräkning + kompilering, inte med skärmbild. En manuell titt vid 4× zoom och halvt synligt klipp räcker.

---

## 3. 🟡 Ljudtråden skannar alla regioner för varje sample

`synth/process.rs:569-619`: för varje spår, i varje sample, itereras **alla** regioner med
tidsfönsterjämförelse och utan `break`. 24 spår × 100 klipp ≈ 2 400 jämförelser per sample ≈
115 M/s, varav 1–2 träffar. CPU skalar med antal klipp, inte med antal aktiva klipp.

**Minsta fix:** sorterad starttid och en cursor per spår som bara flyttas framåt. Cursorn måste
invalidieras av `SetStemTrackRegions` (och allt som byter regionlista), annars spelar regioner som
redigeras under uppspelning fel. **Insats:** en kväll inklusive prov.

---

## 4. 🟡 PDC-max räknas om varje sample

`synth/process.rs:421-448`: `stem_tracks.iter().map(...latency_frames()).max()` plus
`pdc_bus.set_delay()` ligger i `process_stereo`, dvs en gång per sample — ~1 M virtuella anrop/s.
`set_delay` tömmer dessutom sin `VecDeque` när värdet ändras.

**Minsta fix:** räkna om på blockstart (eller på dirty-flagga satt av `SetPlugin`,
`SetPluginLatencyOffset`, pitchbyte och manuellt offset). **Insats:** en kväll; kräver att varje
kommando som kan ändra latensen sätter flaggan, annars blir kompensationen stående.

---

## 5. 🟡 `regions.clone()` per spår per bildruta

`arranger.rs:1715` kopierar varje spårs regionlista — inklusive `waveform_peaks` (256 f32 per klipp)
och två `String`-fält — bara för att läsa. Granskaren kallade fixen "tre rader". Så är det inte:
samma loop muterar `self` på fyra ställen (`click_region`, `push_undo`, `selected_regions`,
`item_groups`), och det är därför kopian finns.

**Minsta fix:** flytta de fyra mutationerna till efter loopen (samma mönster som `split_action` redan
använder), sedan låna slicen. **Insats:** en kväll, ingen beteendeändring.

---

## 6. 🟢 Mixer-digesten bygger en ägd kopia varje bildruta

`timeline.rs:1111` → `mixer.rs:554` → `project.rs:1185`: `mixer_state_digest()` anropar
`non_track_sound()` som klonar `plugin_automation`, `eq_nodes` och bygger `Vec<PedalSound>` med
`id.to_string()` — 30–60 gånger/s, utan att något ändrats. Hashen är billig; kopian är onödig.
**Minsta fix:** hasha lånade slices direkt i stället för att först bygga `NonTrackSound`.
**Insats:** en timme. Liten vinst, men den växer med antalet automationskurvor.

---

## 7. 🟡 Monoliterna: 4 656 rader `mixer.rs` var fyra ytor i en fil

`mixer.rs` blandade kanalracket, effektmixern, synthracket och mixerns logik. Utdelat i den här rundan:

| Fil | Rader | Innehåll |
|---|---:|---|
| `ui/app/mixer.rs` | 998 | patterns, automation, spektrum, audition, snapshot |
| `ui/app/channel_rack.rs` | 1 563 | `render_channel_rack` + kanalremsor |
| `ui/app/fx_mixer_rack.rs` | 1 095 | `render_effects_mixer_rack`, EQ-block, master |
| `ui/app/synth_rack.rs` | 1 018 | oscillator, filter, ADSR, graf |

Två privata metoder blev `pub(crate)` (`add_transmitter_track`, `mix_snapshot`) eftersom de nu anropas
från syskonmoduler. Allt annat är flyttat oförändrat; kompilatorn är hela verifieringen.

**Kvar, i ordning (mätt arbetsmängd, inte gissning):**
1. `modals.rs` 4 486 rader / 34 dialoger → `modal_stems.rs`, `modal_settings.rs`, `modal_project.rs`,
   `modal_creative.rs`, `modal_panels.rs`. Noll prov i filen — bara kompilering och skärmbildsläget fångar fel.
2. `arranger.rs:1224 draw_timeline_lanes` = **2 120 rader** → rita, hit-test och actions i tre filer.
3. `piano_roll.rs` regelverk (`apply_rule`, `snap_grid_to_chord`, 10 prov) → `ui/app/rules.rs`.
4. `project.rs` persistenstmodell (`Saved*` + konverteringar, 9 prov) → `ui/app/saved.rs`.

Dela **inte**: `state.rs` (modell + 43 prov), `i18n.rs` (strängdata), `frame.rs` och `ui/app.rs` (rot).
Sju filer är redan under 1 000 rader.

---

## 8. 🟡 Bygg- och testarkitekturen

*Uppmätt:* `cargo check --no-default-features` efter att ha rört **en** fil: **8,4–9,0 s**. Samma siffra
oavsett om filen är `engine.rs`, `i18n.rs` eller en UI-fil — en crate, ingen modulvis inkrementalism.
Den här rundans uppdelning ändrade inte den siffran (8,7 s för `channel_rack.rs`), vilket är väntat:
fler filer i samma crate ger kortare filer, inte kortare bygge.

*Verifierat statiskt:* 1 467 prov, 17 `#[ignore]`, 46 `cfg(feature = …)`. CI bygger `""` och
`plugin-host`; **`jack` och `python` förekommer noll gånger** i `.github/workflows/`. De 20 ignorerna
körs aldrig automatiskt.

**Minsta åtgärder:** två `cargo check --features jack` / `--features python` i CI (inte fyra nya
testjobb), och ett nattligt `cargo test --release -- --ignored`. `lib.rs` införs först om
`cargo build --timings` visar att länkningen är flaskhalsen — annars blir det bara dubbelkompilering.

---

## 9. 🟢 Kall biblioteksskanning: 139,7 s för 10 630 samplar

*Uppmätt* med tom cache: `🎵 Ljudbibliotek skannat: 10630 samplar på 139.7 s`. Med cache är starten
omedelbar (`Ljudbibliotek läst från cache: 10628 samplar`). Fabrikssamplarna genereras om vid varje
fullskanning men tar **0,5 s** (`🧪 DSP klart på 0.5s`) — granskarens 99 s var en summa av buffertlängder,
inte en mätning.

**Minsta fix om det blir ett problem:** skippa `save_to_file` när filen redan finns med rätt storlek.
Inte prioriterat: skanningen är bakgrundsarbete och cachen träffar normalt.

---

## Granskarpåståenden som inte höll

| Påstående | Verifiering | Beslut |
|---|---|---|
| "Mickingången räknar sin/cos 48 000 gånger/s" (`recorder.rs:542`) | `set_cutoff` och `set_freq` returnerar tidigt när värdet är oförändrat (`recorder.rs:314`, `:464`) | **Kontestat** — ingen fix |
| "`format!` sker före dbg-gaten i ljudtråden" (`process.rs:1001`) | Byggs en gång per 48 000 frames = **1/s**, inte per sample | **Kontestat** — ingen fix |
| "Fabrikssamplarna tar 99 s" | Uppmätt **0,5 s** | **Kontestat** |
| "`render_add_track_modal` finns två gånger" | `modals.rs:30` är en tunn `impl`-metod som delegerar till den fria funktionen | **Kontestat** |
| "i18n: 105 ns mot 36 ns för HashMap" | Gick inte att mäta i den här rundan (ingen isolerad bench); 2 603 anropsställen är däremot räknade | **Ej bedömt** — `t()` returnerar `&'static str`, svenska går genom no-op |
| "`regions.clone()`-fixen är tre rader" | Loopen muterar `self` på fyra ställen | **Korrigerat** — en kväll, inte tre rader |
| "`TempoMap` byggs om per steg" | Sant, men bara när `tempo_points` är icke-tom; tom lista ger `TempoMap::single` utan clone | **Nedgraderat** till 🟢 |

---

## Status efter rundan

| Post | Utförd | Bevis |
|---|---|---|
| Release-vägen (`--no-default-features`) | ✅ | `cargo check` grön för alla fyra feature-kombinationer; releasebygge kört |
| Vågformsklippning mot vyn | ✅ | kompilerar, 1 369 prov gröna; kantfall kvar för ögat |
| `mixer.rs` delad i fyra filer | ✅ | 4 656 → 998 + 1 563 + 1 095 + 1 018 rader |
| Mickingångens lås över hela blocket | ✅ | samlas i scratch, läggs in under ett memcpy-långt lås |
| Tagning >28 s reallokerade i callbacken | ✅ | fem minuter reserveras på UI-tråden vid start |
| Regionskanning med cursor | ✅ | sorterad lista + cursor; seek bakåt nollställer; nytt prov |
| PDC-cache | ✅ | räknas om var 64:e sample, tvingas direkt av varje kommando |
| `piano_roll` regelverk → `rules.rs` | ✅ | 193 rader ut; proven körs kvar i `piano_roll` (1 369 gröna) |
| `modals.rs` delad i fem filer | ✅ | 4 484 → 1 088 + 763 + 500 + 1 046 + 500 + 622 rader, 39/39 funktioner |
| CI: jack/python | ✅ | nytt `feature-checks`-jobb med `-D warnings` |
| CI: nattliga ignore-prov | ⛔ | de 20 är mätningar mot filer på disk; ett CI-jobb skulle falla varje natt |
| `regions.clone()` bort | ⏳ | kräver att fyra mutationer flyttas ut ur loopen |
| `arranger.rs`-delning (`draw_timeline_lanes` 2 120 rader) | ⏳ | nästa strukturella pass |

Prov efter åtgärderna: **1 369 gröna, 0 röda, 20 ignorerade** (19,4 s) i basbenet — ett nytt prov för
regionkursorns seek — och **1 437 gröna** i plugin-host-benet. `cargo check --no-default-features`
0 varningar, `cargo build --release --locked --no-default-features` 1 m 41 s. Realtidsmätningen
(`audio::realtime_bench`, 8 kanaler + 3 spår): värsta block **0,07 ms vid 64 frames** mot 1,45 ms
budget (5,2 %), 0,26 ms vid 512 frames (2,2 %). Den mäter inte 100-klipps-fallet; cursorfixen vilar på
funktionsproven, inte på den siffran.

**CI efter rundan:** alla fem jobb gröna (körning `41496326702`), inklusive `--no-default-features`,
Windows-bygget, neural-benet, release-paketen och rökproven på debian:12, ubuntu:22.04/24.04,
fedora:41 och rockylinux:9. Plugin-host-benet föll först i `apt`-steget (spegel-timeout, inte koden);
omkörningen av det jobbet gick grönt.

---

## Så här är listan kontrollerad

Varje `fil:rad` är uppslagen i koden: `resample`-gaten och CI-loggen, `draw_start_x`/`cols`/`zoom_regime`
i arranger, regionsloopen i `process.rs`, PDC-blocket, `regions.clone()`, `non_track_sound()` i
mixer-digesten samt `set_cutoff`/`set_freq`/`dbg_log`. Siffrorna för prov, filer, rader, fält och
`impl`-block är räknade i den här sessionen. Körtiderna för `cargo check` är tagna med `date +%s%N`
runt kommandot på den här maskinen.
