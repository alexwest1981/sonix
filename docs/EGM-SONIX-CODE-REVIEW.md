# EGM på koden — runda 4 (2026-11-03)

**Mål:** diffen `4d8ac06..f744e10` (29 filer, 1 199 tillagda rader) — allt sedan EGM-varv 2.
**Granskare:** tre färska kontexter (`codebase_details`, `structural`, `zoom_out`), EGM-personorna
lästa ur `agents/`, standarder ur `resources/document_standards/`.
**Regel:** koden är sanningen, dokumenten är påståenden. Varje fynd nedan är **kvitterat** mot
källan av mig efter granskningen — de som inte höll är märkta *motbevisat*.

## Kvittens

| # | Fynd | Allvar | Status |
|---|---|---|---|
| 1 | `write_script` gav en extern modell godtycklig filskrivning (`save_midi`) | 🔴 | **Åtgärdat** — `script::run_isolated` registrerar inte filfunktionerna; AI-vägen använder den. Prov: `a_model_script_cannot_touch_the_filesystem` |
| 2 | AI-anrop går inte att ångra, och CHANGELOG påstod att de gjorde det | 🔴 | **Åtgärdat** — snapshotet bär nu `channels` och `sections`, och dräneringen lägger **ett** steg per POST (bara om något gick igenom). Inte provtäckt: inget prov kan bygga en `SonixApp` |
| 3 | `isError` alltid false; ett avvisat anrop ser ut som ett lyckat | 🔴 | **Åtgärdat** — `ai_execute_call` → `AiReceipt { ok, text }` via en `Result`-väg genom alla 15 verktyg; `/v1/last` bär flaggan. Mätt mot körande app: `bpm 400` → `isError: true` |
| 4 | Kvittot var inte knutet till anropet — ett tidsutgånget svar kunde bli nästa anrops svar | 🔴 | **Åtgärdat** — `/v1/last` töms före POST. Prov: `a_stale_receipt_is_not_returned_as_the_answer` |
| 5 | Vakthunden mätte *fokus*, inte synlighet — en DAW i bakgrunden slutade återhämta ljudet | 🔴 | **Åtgärdat** — hoppar bara när fönstret är i bakgrunden **och** transporten står still |
| 6 | Vakthunden ger upp **utan att säga det** | 🔴 | **Åtgärdat** — verifierat att beskedet var oåtkomligt (en egen konstant kan aldrig enas med trappans fyra steg); nu sätts det där trappan tar slut, en gång (`audio_gave_up`) |
| 7 | Hängproben kunde peka ut fönstrets present när blockeringen låg i appens egen kod | 🟡 | **Åtgärdat** — `perf_frame_start()` nollställer fasen per bildruta; poison-säkra lås; raden säger `bildruta-start` när ingen fas hunnit köras |
| 8 | PAN/BREDD hörs bara tills projektet öppnas; `stereo_width` sparas inte alls | 🟡 | **Öppet** — verifierat: `stereo_width` finns inte i `project.rs`, `SetMasterPanWidth` sänds bara vid `reconfigure` |
| 9 | Radzoomens golv klippte kontrollerna; filens faktor klämdes inte | 🟡 | **Åtgärdat** — golv 44 → 56 px, `row_scale.clamp(0.5, 2.0)` vid inläsning, provet mäter rätt gräns |
| 10 | Ctrl+hjul har två ägare (global tidslinjezoom + radzoom) och skriver radhöjd mitt i bildrutan | 🟡 | **Öppet** |
| 11 | Avdelarlinjen räknar `num_tracks * 50.0` medan raderna är 56–400 px | 🟡 | **Öppet** |
| 12 | `write_drum_pattern` kräver `pattern` i schemat men utföraren läser det aldrig | 🟡 | **Öppet** |
| 13 | `write_pattern` är inte atomisk över lanes; `chop_channel` synkar inte till patternmodellen | 🟡 | **Öppet** |
| 14 | MCP-testet kontrollerar inte `Authorization` och kör aldrig `{"tools": […]}`-grenen | 🟡 | **Delvis** — kvittokorrelationen är provad; de två kvar |
| 15 | Dokumentens siffror (1 378 prov, textskalan, 276 px) stämmer inte med trädet | 🟢 | **Öppet** |

## Det som inte höll

- Inget fynd motbevisades. Ett påstående från runda 3 (`write_script`-sandlådan) visade sig vara
  sant bara mot *tagningen* — filsystemet var oskyddat, och MCP gjorde vägen anropbar utan en
  människa framför skriptrutan.

## Öppna fynd, i den ordning de bör tas

1. **PAN/BREDD i projektfilen** (8): `stereo_width` in i payloaden med `serde(default)`, båda in i
   `NonTrackSound`, och `SetMasterPanWidth` sänds vid inläsning.
2. **Läsrutt för en extern klient** (12): ett read-only `project_state` som återanvänder
   `ai_channel_context()` — och ta bort `pattern` ur schemat i stället för att låtsas.
3. **`chop_channel` synkar inte till patternmodellen** (13): stegen sätts men
   `sync_active_pattern_from_ui()` anropas aldrig, så slicarna försvinner vid patternbyte.
4. **MCP-testets täckning** (14): attrappen kontrollerar inte `Authorization`, och
   `{"tools": […]}`-grenen körs aldrig.
5. **Dokumentens siffror** (15).

---

# Runda 5 — `c93b11f..9383d59` (kvittoreformen och ångringen)

Samma tre granskare, nu på den kod som skrevs *för att åtgärda runda 4*. Två av fyndens allvar
kom av att fixen själv var ny.

| # | Fynd | Allvar | Status |
|---|---|---|---|
| 1 | Ångringen skrev kanalernas steg in i **det pattern som var valt nu**, inte det som hörde till snapshotet | 🔴 | **Åtgärdat** — `selected_pattern` i snapshotet, och `sync_active_pattern_from_ui()` borttagen ur `restore_snapshot` (den kunde bara skriva fel) |
| 2 | Läsverktyg (`analyze_mix`) gav ett tomt ångringssteg och **slängde redo-historiken**; ett halvt `write_pattern` gav inget steg alls | 🔴 | **Åtgärdat** — `ai_tools::mutates(tool)` styr steget, inte kvittots `ok`. Prov: `every_tool_is_classified_as_mutating_or_not` |
| 3 | `export_loudness_idx` låg utanför snapshotet | 🟡 | **Åtgärdat** — fältet tillagt |
| 4 | `set_channel_instrument` ändrar tio globala synthfält som ingen snapshot bär, och schemat kräver `channel` som utföraren aldrig läser | 🟡 | **Öppet** — kräver synth-inställningarna i `NonTrackSound`; modulhuvudet som påstod att verktyget saknade utförare är rättat |
| 5 | `plan_sections` lagrar bara en väntande plan; sektionerna läggs in av UI-knappen, som inte pushar något steg | 🟡 | **Öppet** |
| 6 | MCP-adaptern läste bara **första** kvittot, och gjorde ett kvitto utan `ok` till en framgång | 🟡 | **Åtgärdat** — alla kvitton läses, ett avvisat gör hela POST:en avvisad, och en post utan `ok` är ett fel. Två nya prov |
| 7 | Vaktprovet (`mixer_digest_covers_every_mixed_field`) mäter inte kanalerna, trots att det påstår att det fångar ett glömt snapshotfält | 🟡 | **Delvis** — kommentaren säger nu vad digesten täcker; digesten utökas inte, för den styr mixer-ångringens viloläge |
| 8 | `groove_report` ger `Err` för "inga slag hittades" medan `analyze_mix` ger `Ok` för "tyst" — två regler för samma sak | 🟢 | **Öppet** |
| 9 | `docs/AI-API.md` visade den gamla kvittoformen | 🟢 | **Åtgärdat** |

## Kvar att ta

1. Synth-inställningarna (10 fält) in i `NonTrackSound` + `LoadPreset` vid restore, så
   `set_channel_instrument` går att ångra — **och ta bort `channel` ur dess schema**, som i dag
   är ett argument utföraren struntar i.
2. Ett snapshot i `apply_pending_plan` (UI-knappen som lägger in sektionerna).
3. En regel för "mätningen hittade inget": antingen `Ok` i båda mätverktygen eller `Err` i båda.
