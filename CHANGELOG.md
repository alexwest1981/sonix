# Changelog

All notable changes to Sonix Studio are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.192] - 2026-10-28

**Ramarkering över flera spår och razor-duplicera.**

- Ramen går nu över så många spår man drar: spåret under pekaren blir ramens andra kant,
  och släppet markerar alla regioner som överlappar intervallet i alla spår däremellan.
- `Ctrl+Alt+D` duplicerar tidsmarkeringen: klipper ut området, gör plats genom att skjuta
  allt efter det och lägger kopian direkt efter — sampling-offset och vågform följer med.
- Prov: en beskuren kopia täcker exakt området, och en region utanför kopieras inte.

## [0.9.191] - 2026-10-28

**Tidsmarkering från regeln.** Shift+drag i regeln markerar tid i stället för att flytta
spelhuvudet, och området syns som ett cyan band i regeln. Samma markering som rammarkeringen
lämnar, så `Ctrl+Shift+X` (razor) arbetar på båda.

## [0.9.190] - 2026-10-28

**Ramarkering, ripple-redigering och razor.** De tre sätten att arbeta på ett *område* i
stället för på en region i taget.

- **Ramarkering**: drag som börjar i tom yta markerar regionerna man drar över, ritar en
  cyan ram och lämnar intervallet som tidsmarkering. Gäller spåret man drar i.
- **Ripple** (knapp i verktygsraden, av → spår → alla): borttagning stänger gapet och
  inklistring skjuter undan det som ligger efter, på samma spår eller alla.
- **Razor** (`Ctrl+Shift+X`): tar bort tidsmarkeringen — regioner helt innanför tas bort,
  de som sticker in kapas, och en region som spänner över hela området delas i två med
  korrekt sampling-offset och beskuren vågformsbild. `Escape` rensar markeringen.
- Prov: intervallurvalet tar bara regioner som överlappar (och klarar omvänd dragning),
  och ripple-knappen cyklar av → spår → alla.

## [0.9.189] - 2026-10-28

**Objektgrupper, Alt+drag-kopia, klistra på ursprunglig plats och ångra-historik.**
Resten av kopiera/gruppera-flödet, det som REAPER gör utan att man tänker på det.

- **Objektgrupper** (`Ctrl+G` på regioner, `Ctrl+Shift+G` löser upp): region-id → grupp-id i
  `item_groups`, så en grupp kan sträcka sig över spår. Ett klick på en grupperad region
  markerar hela gruppen, och en dragning flyttar hela gruppen. Grupperna sparas i projektet.
- **Alt+drag kopierar** i stället för att flytta: klonen får nya id:n, ärver grupptillhörighet
  och blir den man drar i.
- **Ctrl+Shift+V klistrar på ursprunglig plats** — klungan kommer tillbaka där den kopierades,
  inte vid spelhuvudet.
- **Ctrl+Alt+Z öppnar ångra-historiken**: stegen listas nyast överst och ett klick hoppar
  tillbaka dit (`undo_to`), i stället för tio Ctrl+Z.
- Prov: markeringen expanderar till hela objektgruppen och lämnar ogrupperade regioner i fred.

## [0.9.188] - 2026-10-28

**Hela markeringen flyttar med.** Multi-markeringen från 0.9.187 hade en halv sanning:
man kunde markera fem regioner men bara den man drog i rörde sig.

- `RegionDragState::multi_initial` fångar de övriga markerade regionernas starttakter när
  dragningen börjar, och Move-grenen lägger samma förskjutning på dem — räknad från
  starten, inte kumulativt per bildruta.
- Markeringen uppdateras före `push_undo`, så klungan som följer med är den som gäller
  efter klicket.
- Vid släpp synkas alla spår som var med i dragningen, inte bara det man höll i.
- `docs/REAPER-WORKFLOW-GAPS.md` uppdaterad: multi-flytten är ✅, kvar är rammarkering,
  objektgrupper och mappar med routing.

## [0.9.187] - 2026-10-28

**Markera flera spår och regioner — och kopiera dem tillsammans.** Flödet saknade
grunden: allt utgick från ett spår och en region, så Ctrl+C kopierade alltid en enda sak.

- `selected_tracks` och `selected_regions` som mängder. Den primära posten ligger kvar i
  `selected_timeline_track` / `selected_audio_region`, så de 47 ställen som redan läser
  dem fortsätter att fungera.
- Klick: Ctrl/Cmd växlar, Shift tar spannet, vanligt klick börjar om. Markeringen kan
  aldrig bli tom — `clicked_selection` är provad för alla fyra fallen.
- Ctrl+C kopierar alla markerade regioner som en klunga med spår- och taktavstånd; Ctrl+V
  klistrar in den vid spelhuvudet på det primära spåret. Utan regionmarkering kopieras
  hela spår, som klistras in efter det sista markerade.
- Ctrl+A markerar alla spår (när inget textfält äger tangenten). Ctrl+G grupperar de
  markerade spåren, Ctrl+Shift+G löser upp.
- `prune_selection` en gång per bildruta: borttagna spår och regioner städas ur
  markeringen i stället för att bli spöken.
- Spårhuvudet visar hela markeringen: primärt spår i sin färg, övriga i cyan ram.
- Nytt: `docs/REAPER-WORKFLOW-GAPS.md` — hela flödet mot REAPER, med vad som är kvar.

## [0.9.186] - 2026-10-28

**Korsning vid spelhuvudet och alla grupper på en gång.** De två sista raderna i FL-kommandoytan
som gick att bygga utan ett nytt urvalslager.

- **✚ Korsa vid spelhuvudet**: bara skarven närmast spelhuvudet korsas, inte hela spåret.
  `create_crossfade_at(bar)` väljer det överlappande paret vars skarv ligger närmast och
  använder samma regel som `create_crossfades` (halva regionen som tak). Prov: med två skarvar
  rörs bara den vid spelhuvudet, och utan överlapp händer ingenting.
- **📁 Fäll ihop/upp alla**: `set_all_groups_collapsed` fäller alla grupper och räknar bara dem
  som faktiskt ändrades, så statusraden inte säger "3" när två redan stod rätt.
- Inventariet uppdaterat: `Create crossfade`-raden och gruppraden stämmer nu med koden.

## [0.9.185] - 2026-10-28

**Spågrupperna sparas i projektet.** De fanns bara i minnet: en grupp och dess ihopfällda
läge försvann vid varje sparning, och ingen fil kunde säga vilket spår som hörde till vilken
grupp. Inventariet hade det som ✗ — nu är det stängt.

- `SonixProjectData.track_groups` och `SavedTrackData.group`, båda `#[serde(default)]` så en
  fil från före fältet öppnas som ogrupperad i stället för att vägra.
- Laddningen sätter tillbaka både grupplistan och varje spårs grupp, och gruppens
  ihopfällda läge följer med.
- Prov: en grupp med `collapsed` överlever JSON-rundturen, och projektfilen bär den.

## [0.9.184] - 2026-10-28

**Exportarbetaren flyttad till `audio::exporter` — och prövad med en riktig fil.** Trådvägen
från 0.9.182 var bara kompilerad; nu finns `render_files` där renderingskoden hör hemma, med
`ExportFile`/`ExportOutcome`, och ett prov som renderar en takt, skriver en WAV och läser
storleken tillbaka. Samma kod som UI-tråden använder — alltså är det den vägen som prövas.

- Ett tomt jobb är ett giltigt jobb: inga filer, inget fel, ingen panik.
- Framstegen räknas i samma atomär som `poll_render_job` läser, så tråden och UI:t delar en
  sanning i stället för två.

## [0.9.183] - 2026-10-28

**Export av spågrupper som egna filer.** Det sista av FL-kommandoytan som handlade om export:
gruppbakningen fanns, men ingen väg att få ut varje grupp som en fil.

- Fjärde valet i exportens omfång: **Spågrupper – en fil per grupp**. Varje grupp får en fil
  med bara sina spår hörda, via samma `isolate_group` som gruppbakningen använder — gruppens
  spår solas och övriga spårs tidslinjeljud töms, så filen bär gruppen och inget annat.
- Tomma grupper hoppas över, och en grupp vars namn blir tomt i filnamnet får `GruppN`.
- Gruppfilerna går genom samma bakgrundstråd som resten av exporten.

## [0.9.182] - 2026-10-28

**Exporten renderar i bakgrunden.** FL:s `Background rendering` var det sista i kommandoytan
som handlade om arbetsflöde i stället för om en effekt: förut frös fönstret medan filerna
skrevs, nu kan man jobba vidare och statusraden säger till när det är klart.

- Specarna byggs på UI-tråden (de är ren data), sedan tar en tråd över: `render_files` kör
  samma `build_offline_engine` + `render_project_offline` som förut, hoppar över tomma stammar,
  normaliserar mastern och skriver filerna.
- `RenderJob` bär tråden, en atomär räknare och antalet filer; `poll_render_job` räknar om
  förloppet varje bildruta och hämtar resultatet när tråden är klar. Panik i tråden blir
  "avslutades oväntat" i statusraden i stället för en evig progressbar.
- Startknappen är avstängd medan en export kör, så två exporter inte kan skriva samma fil.
- Beskeden vid klart/fel är desamma som den blockerande exporten gav, inklusive
  normaliseringsraden (LUFS, mätt nivå, tak).

## [0.9.181] - 2026-10-28

**Tre MIDI-verktyg på tagningen: arpeggio, rullning och choke.** De bor där Sonix faktiskt har
notlängder — i tagningen — i stället för i en effekt som saknar MIDI-ingång.

- **🎹 Arpeggio**: noter på samma slag blir en följd. Tonerna cyklas i tonhöjdsordning, `octaves`
  låter följden klättra, och varje steg får vald längd. Riktningen är samma ratt som
  strumningen (Ner = högsta först) — en riktning att lära sig, inte två.
- **🔁 Rulla ×4**: varje not med känd längd upprepas fyra gånger inom sin egen längd, en
  stutter som stannar där frasen slutar. Okänd längd hoppas över.
- **🔗 Choke**: en not som klingar när nästa börjar kapas vid nästas start. Basslines och mono
  leads blir rena, och noter med okänd längd rörs inte — att korta en not vars slut vi inte
  känner är att gissa.
- **Mätt:** ett treklang på 4 steg med 0,25-steg blir 16 noter (60, 64, 67, 60 …), oktaver
  klättrar 60→72→76, rullning ger 4 noter på 0/1/2/3 med 0,9 längd, och choke kapar 2,0 → 1,0
  medan okänd längd står kvar. 1162 prov gröna.

## [0.9.180] - 2026-10-28

**Spektral målning: dämpa ett ljud genom att måla på dess spektrogram.** Verktyget ligger i
spårets verktygsrad och är den interaktion som REAPERs `spectro_filter_paint` är — utan att en
rad är lånad.

- `audio::spectral_edit`: STFT 1024/256 med Hann, spektrogram i dBFS, mask per (bildruta, bin)
  och `apply` som gör masken till ljud igen. Vyn är högst 1024 × 256 rutor även för långt
  ljud, och raderna är vända så hög frekvens ligger överst.
- `Mask::apply` lägger en framruta på N sampel: utan den är de första ~768 samplen för svaga,
  eftersom överlapp-adderingen bara är exakt där alla fyra fönstren bidrar.
- `ui::spectral_editor`: spektrogram som textur, målning som RGBA-lager, pensel (1–12 rutor)
  och dämpning 0…−60 dB. `Verkställ` byter spårets buffer och synkar stammarna — bara om något
  målats.
- **Mätt:** tom mask rekonstruerar insignalen (< 1e-3), 1 kHz-målning dämpar 1 kHz ≥30 dB och
  lämnar 4 kHz inom 1,5 dB, och fönstret ritas i en headless egui-bildruta.

## [0.9.179] - 2026-10-28

**Elva egna effekter, byggda här.** `src/audio/builtin_fx/` är ett register i stället för nio
nya fält i mastern: en `EffectDef` bär id, namn, kategori och sina parametrar, och effekten
laddas på samma väg som en plugin (`builtin:<id>`) — alltså i varje kanals kedja, med
automatable parametrar, utan fil och utan `--features plugin-host`.

- **Filter 72**: Butterworth låg-/högpass med 24, 48 eller 72 dB per oktav. Mätt: 24 dB ger
  −49 dB två oktaver över gränsen, 72 dB minst 30 dB brantare.
- **Tilt**, **Stege 24** (fyrpolig ladder med mättnad) och **Fasrotation** (allpass, 180° vid
  gränsfrekvensen).
- **Bass Manager**: mono under vald frekvens med LR4-delning, hylla och fas. Motfas bas
  försvinner; diskanten rörs inte; delningen är nivåneutral med mono av.
- **Ringmodulator** och **Pitch** (±12 semitoner, två korsande läspunkter; 0 semitoner är
  bitidentisk passthrough).
- **Spektral kompressor**: STFT 1024/256, frekvensberoende tröskel (center + lutning), nedåt-
  och uppåtkoppling med mjuk knee, attack/release per bin, mix. 768 sampel latens som PDC:n
  kompenserar. Ratio 1 rekonstruerar insignalen.
- **Inflator**: udda ordningens vågformning; toppfaktorn sjunker och toppen stannar under 1.
- **Justera**: sampelnoggrann fördröjning och polaritet per kanal — fasjustering av två
  mikrofoner på samma källa. 1 ms vid 48 kHz hamnar på sampel 48.
- **Rattar-rutan**: en inbyggd effekt har ingen egen GUI, så GUI-knappen öppnar en egui-ruta
  med en slider per parameter (värdet läses ur instansen, skrivs till samma handtag som
  ljudtråden läser). `PluginCore::parameter_value` svarar `None` när en backend inte kan, och då
  ritas ingen ratt.
- **Kurva**: waveshaper med åtta punkter — rak kurva är passthrough, fallande inverterar,
  mättande sänker toppfaktorn. Punkterna är parametrar, så den ritas i Rattar-rutan.
- **Gain reduction-scope**: masterns kompression över tid (~8 s) i FX-racket, 0 dB överst och
  −24 dB nederst. Mätningen kommer ur `master_gain_reduction_db`, historiken är ringformad.
- **Formatsfilter** i plugin-listan har en **Sonix**-flik, och de inbyggda ligger i listan även
  när cachen laddas — de finns inte som fil och kan därför aldrig hittas av en diskskanning.
- `docs/BUILTIN-FX.md`: vad de gör, vad som är mätt, och hur en ny läggs till (tre steg).

## [0.9.178] - 2026-10-28

**JSFX kan läsa ljudfiler, och tabeller skrivs som de ska.** `file_open`/`file_riff`/`file_avail`/
`file_mem`/`file_close` är inte längre stubbar: en WAV som ligger bredvid `.jsfx`-filen (eller i
`~/.config/sonix/jsfx`) öppnas, kanaler och frekvens rapporteras, och proverna kopieras in i
JSFX-minnet. `mem_set_values(dest, v1, v2, …)` skriver verkligen tabellen — den var en tom stubbe,
och REAPERs filer använder den för att bygga sina tabeller.

- `Jsfx::compile_in(source, mapp, srate)`: filens mapp är känd redan när `@init` körs, vilket är när
  en sampler öppnar sina ljudfiler.
- `@slider` körs vid laddning (som REAPER) — och därför finns nu `set_midi_note_name`,
  `slider_show`/`slider_hide`/`slider_name` m.fl. som ärliga no-ops: de ändrar bara hur REAPER visar
  saker, men koden ska inte falla på att de anropas.
- Bara WAV avkodas; FLAC (DrNine m.fl.) svarar fortfarande "ingen fil" och bokförs i `stubs`.

## [0.9.177] - 2026-10-28

**JSFX går att lägga i effektkedjan.** En `.jsfx`-fil laddas nu där en CLAP/VST-plugin laddas:
`load_processor` känner igen ändelsen och ger en `PluginProcessor` (backend `"JSFX"`), med filens
sliders som parametrar (`set_parameter` kör `@slider`, `reset` kör om `@init`). Det fungerar i
standardbygget — JSFX är egen Rust-kod och kräver inte `--features plugin-host`.

- `PluginFormat::Jsfx` i plugin-listan: `.jsfx` klassas som *JSFX (REAPER)* i stället för som en
  VST2 som inte går att ladda.
- `~/.config/sonix/jsfx` ligger i sökningen, och `.jsfx` hittas av skanningen.
- `Jsfx::compile` kör `@slider` en gång vid laddning, som REAPER — utan det var en effekt tyst fram
  till första vridningen.
- `Jsfx::reset` nollställer minnet (sliderna behålls) och kör `@init` igen.
- `shortcut`: instrument tar ingen MIDI genom `PluginProcessor` (traitet har ingen notingång), så
  JSFX-instrument låter i `--jsfx-note` men är tysta i kedjan än.

## [0.9.176] - 2026-10-28

**REAPERs 256 JSFX-effekter och instrument kör i Sonix.** `src/audio/jsfx/` är en EEL2-motor
(lexer, parser, virtuell maskin) som tolkar filerna som de är i stället för att porta dem en och
en — samma val som gjorde att en `PatternClip` räckte för alla stämmor. Alla 256 filer i REAPER
7.83:s `InstallData/Effects` tolkas och kör sin `@init`; de fyra referenseffekterna mäter
1,5–9× realtid per instans.

- **Språket**: `^` är potens (xor skrivs `xor`/`~`), `x[]` = `x[x]`, `?:`-grenar får vara block
  och `:` får saknas, `while ( kropp; villkor )`, `loop (i = 0; n, kropp)`, `<? ?>`-generatorblock,
  `/* */`, `import`, punktade namnrymder (`this.x` → `rmsa.x`), `$pi`/`$x90`/`$'F'`.
- **Motorn**: `slider1..64` på plats 0–63 som i REAPER, eget `f64`-minne, egna funktioner med
  lokala namn, MIDI in/ut, `fft`/`fft_real`/`ifft_real` via `rustfft`, 50 M steg per påstående
  som skydd mot `while(1)`. En effekt som kastar fel **tystnar hela blocket** i stället för att
  elda 2 M steg per prov.
- **Ärligt om stubbarna**: `file_open`/`file_riff`/`file_mem` m.fl. svarar "ingen fil" och bokförs
  i `Jsfx::stubs`, så en effekt som tystnar går att skilja från en som inte kunde läsas. `@gfx`
  och `@serialize` hoppas över (UI:t är REAPERs, tillståndet är projektets).
- **Instrumenten**: `TolerableTines` låter direkt; `DrNine`, `JamSynth` och `SynthyMcSynthface`
  laddar samplingar som inte skeppas med REAPER (de är tysta där också); `ChamberOrgan` importerar
  sin stopptabell och tolkas, men stannar på MIDI-filtret.
- **CLI**: `--jsfx-list <mapp>`, `--jsfx-run <fil> <in.wav> <ut.wav> [slider=värde]`,
  `--jsfx-note <fil> [not]`, `--jsfx-dump <fil> [sektion]`.
- **Prov**: `SONIX_JSFX_DIR=… cargo test --bin sonix corpus_tests` kräver alla 256 filer och att
  minst fyra instrument ger ljud från en nedtryckt tangent.
- **Docs**: `docs/JSFX.md` med mätningarna, språkets hörn och vad som är stubbat.

## [0.9.175] - 2026-10-28

**Fem separationsmodeller att välja mellan.** Katalogen `neural_separator::MODELS` ersätter den
gamla 4/6-boolen: varje modell bär sina stämmor, sin nedladdnings-URL, sin licensnot, sin
storlek — och sitt **format**, för nu finns två.

- **Två sorters export.** Demucs-familjen tar vågform. RoFormer-familjen och SCNet är exporterade
  *mellan* sin STFT och iSTFT, så appen gör fönstringen själv (`audio::stft`, ny modul med
  STFT/iSTFT, Hann eller inget fönster, skalning per export). Rundturen `istft(stft(x))` är provad.
- **Katalogen:** `htdemucs` (4), `htdemucs_6s` (6), `bs_roformer_sw6` (6 — den enda öppna export
  där gitarr och piano faktiskt separeras: 9,1 / 7,8 dB mot 3,1 / 1,6), `mel_roformer`
  (sång 12,1 dB) och `scnet` (4 stämmor, 13 MB).
- **Stemmor-vyn** har en lista i stället för en kryssruta: modell, not om kvalitet/licens, `✓` när
  den är hämtad, och hämtning per modell med storlekskontroll (13 MB-modellen krävde förut 100 MB
  för att godkännas).
- **Sångmodeller ger sång + rest**: resten räknas som `mix − sång`, så en sångmodell lämnar aldrig
  tystnad efter sig.
- **Namnen läses ur sessionen** (`spec_real`/`spec_imag`/`mix_spec` var inte samma i alla exporter),
  och ett formfel rapporterar modellens egna in-/utdata i stället för bara "Invalid rank".

Uppmätt med `SONIX_NEURAL_SMOKE=<id>` på 4 s syntetisk mix (CPU): `htdemucs` 0,997 · `htdemucs_6s`
0,998 · `scnet` 0,863 · `bs_roformer_sw6` 0,929 · `mel_roformer` 1,000 av källans RMS — alla
kör genom vår inferens. `docs/STEM-SEPARATION.md` har tabellerna, kontrakten och gränserna.


## [0.9.174] - 2026-10-28

**AI:n får mäta.** Sex nya verktyg. Assistenten kan inte höra — så den läser appens egna mätvärden
och föreslår utifrån dem i stället för att gissa.

- **`analyze_mix`** mäter LUFS och topp per spår (spår för spår genom samma offline-render som
  exporten använder, `solo_track`) plus sektionerna. Ett nivåförslag kommer nu ur siffror.
- **`groove_report`** mäter spårets slag mot sextondelsrutnätet: median och värsta avvikelse i ms.
- **`master_to_target`** sätter närmaste loudness-preset, renderar, normaliserar på samma väg som
  exporten och rapporterar uppmätt LUFS före och efter samt topp.
- **`chop_channel`** slår upp kanalens ljud vid sina transients (samma detektor som mixerpanelen)
  och lägger en slice per sextondel i ett **tomt** mönster — ett komp som spelar rörs inte.
- **`rename_track`** döper spår, t.ex. stämmorna efter en separation.
- **`write_script`** kör Rhai mot den valda tagningen — mot en **kopia**, så ett fel halvvägs
  lämnar tagningen orörd.

Dessutom: kontexten till modellen innehåller nu spåren (pattern/ljud/klippa/tystad) och sektionerna,
inte bara kanalerna. `docs/AI-API.md` har hela verktygstabellen — den påstod felaktigt att
`set_channel_instrument` saknade utförare, och att `set_key` var byggt.


## [0.9.173] - 2026-10-27

**Tutti — och fixarna som kom av att använda appen.** Projektet kan lämnas till det delade
projektformatet och tas tillbaka igen. Vägen dit lagade det som mötte oss: ljudströmmen, tempot i
gamla filer, melodin som satt fast i rutnätet, och en MIDI-slinga som spelade av sig själv.

**Nyheter**

- **Tutti ut och in** (0.9.172/0.9.173): `sonix --export-tutti` skriver en tutti-folder som
  `tutti check` godkänner, och `sonix --import-tutti` gör en tutti-folder till ett `.sonix`-projekt.
  Rundtur export → import → export ger ett identiskt dokument; Sonix egna fält (warp, EQ,
  kompressor, swing, tonart) bärs i `se.alexwest1981.sonix` så en runda genom ett annat program
  inte tappar dem. `.sonix` är kvar som appens eget format.
- **AI:ns melodi blir en klippa på tidslinjen.** `write_notes` skapar en `PatternClip` med egna
  noter (position, anslag, notlängd) i stället för att tända rutor i mönstret. Klippan ritas och
  dras som en ljudregion, och en flytt rör bara startpositionen.
- **Högerklick på en not i pianorullen** (den sista punkten i "högerklick överallt"): radera,
  duplicera ett steg senare, anslag ±.
- **`sonix --version` / `-V`** skriver `Sonix Studio <version>` utan att öppna fönster eller
  ljudenhet — en rad att klistra in i en felrapport.
- **Flikar i toppraden**: grupperna (Skapa/Arrangera/Mixa/Verktyg) och vyerna är mappa-flikar med
  accentlinje på den valda, inte knappar.

**Fixar**

- **Midi Through är en slinga, inte ett instrument.** MIDI-in kopplade upp sig mot varje in-port,
  även ALSA:s loopback — och `--selftest` skickade sin testnot dit. En pågående Sonix hörde då sin
  egen testnot och spelade den. Slingor hoppas nu över, och testnoten går till en riktig utgång.
- **512 frames som standard** (0.9.168): 128 dog på en lastad maskin; en död ljudström byggs nu om
  i stället för att tiga, och felräknaren hör till strömmen som dog.
- **Stämmornas tempo kommer ur arkivet, aldrig ur 120** (0.9.169), och en gammal fil med 120 i
  klippen lagas vid inläsning (0.9.171).
- **Exporten säger rätt installationskommando**: ffmpeg-raden läses ur `/etc/os-release` (apt, dnf
  eller pacman) i stället för att alltid föreslå `pacman`.
- **Textrutan äger tangenterna.** Ett mellanslag i ett projektnamn startade uppspelningen; nu tiger
  transport- och arrangergenvägarna medan en textruta har fokus. En not som startats med musen i
  pianorullen släpps också när fliken lämnas i stället för att ligga kvar och spela.
- **Fasmätaren skriver vad bildrutan gjorde** när den sprack (0.9.170).


## [0.9.167] - 2026-10-27

**En död ljudström byggs om i stället för att tiga.** cpal 0.15 återhämtar sig inte från en
ALSA-ström som svarat `POLLERR`: `alsa::poll()` fortsätter svara fel, callbacken kallas aldrig
mer — men strömmen ser fortfarande `RUNNING` ut i `pactl`. Mätt: 40 000 fel/s, 1,5 GB logg, en
kärna bränd och noll ljud när play trycktes.

- **Hjärtslag per buffert** i ljudcallbacken. Står det stilla en sekund byggs strömmen om med
  samma `reconfigure` + `resync_engine_after_reconfigure` som ljudmodalen, och spelar vidare
  från samma position om transporten rullade. Cooldown 2 s så en enhet som vägrar starta inte
  byggs om varje bildruta.
- **Felloggen dämpad:** tre rader och sedan var 20 000:e fel, både i `err_fn` och i
  panikvägen — en död ström ska synas, inte äta disken.
- **Felräknaren hör till strömmen** som dog: `reconfigure` nollställer den, och vakthunden
  läser den innan omstarten, så statusraden säger den döda strömmens fel och inte appens total.

**AI:n lovar inte längre mer än rutnätet kan skriva.** `write_pattern` skriver en not per steg
utan anslag och längd; modellen kunde ändå be om trioler och legato som appen tyst kastade bort.

- Prompten säger vad som gäller (sextondelsrutnät, en not per steg, heltal), och `parse_notes`
  avvisar bråktal med ett begripligt fel i stället för "steget -1 ligger utanför".
- Varje stämma bär en `subdivision` (1/16 … 1/1); anropet avvisas om slagen ligger utanför den,
  med raden som visar vilka steg som bryter.
- `2.0` läses som 2 — JSON-bråktal föll tidigare igenom som fel.

## [0.9.165] - 2026-10-27

**AI:ns melodi blir ett objekt på tidslinjen.** Den skrev in i mönstrets rutnät, och där fanns
inget att greppa — ingen klippa att flytta, inget att spara. Nu skriver `write_notes` en
**klippa** (`PatternClip`) med egna noter: position, anslag och notlängd.

- **`PatternClip`** på spåret (`pattern_clips`): start och längd i `f32` takter som en
  ljudregion, `lane` mot kanalracket, och noterna i klippan. Sparas i projektfilen
  (`#[serde(default)]`, så gamla projekt öppnas som förut).
- **AI:n skriver noter, inte steg.** `write_notes` skapar klippan vid spelhuvudet på valt spår i
  stället för att tända rutor i rutnätet — notlängd och anslag följer med, vilket rutnätet inte
  kunde bära. Trummor ligger kvar i rutnätet; en klippa utan noter spelar mönstrets rutnät.
- **Klippan ritas och dras** i arrangemanget med snäpp och ångra. Flytten skriver bara
  `start_bar`, så noterna kan inte ändras av en förflyttning (prov i `state::clip_tests`).
- **Uppspelning:** en klippa spelar sin kanal, läser kanalens sampel vid varje slag (bytt ljud
  behåller noterna) och loopar sin egen längd.
- **Fyra döda AI-prov körs igen.** Tre prov i `ai_tools` hade förlorat sina `#[test]`-attribut i
  en tidigare commit och räknades aldrig — de ligger nu på rätt funktion, och `grid_of` känner
  igen de svenska pluralformerna ("åttondelar") som provet kräver.

Kvar till nästa pass: knappen som lägger klippan vid spelhuvudet, Sample-export (WAV + `.mid`),
och triolernas `pos`-schemaläggning — se `docs/AI-CLIPS.md`.


## [0.9.162] - 2026-10-26

**Sju språk, inte bara i appen.** Gränssnittet har kunnat sju språk sedan tidigare, men repot och
skrivbordsposten var engelska — och GitHub visar bara EN README, så en spansk eller tysk besökare
möttes av ett språk hen kanske inte läser.

- **`README_DA/NB/DE/ES/FR.md`** — korta README-filer på danska, norska, tyska, spanska och
  franska: vad det är, skärmdump, installationstabell, licens, och en länk till den engelska
  README:n för hela funktionslistan. Korta med flit: en full översättning av 400 rader ruttnar
  mellan releaserna, det här håller.
- **Språkrad i varje README** som länkar till alla sju, så ingen fil är en återvändsgränd.
- **Skrivbordsposten är översatt** (`Comment[sv/da/nb/de/es/fr]`, `GenericName[...]`,
  `Keywords[...]`) — appens namn och beskrivning syns nu på användarens språk i menyer och
  programstartare på alla distribuerade paket. `desktop-file-validate` godkänner den.
- **AppStream-metainfo** har `summary xml:lang` för samma sex språk, så mjukvaru-butiker visar en
  lokaliserad sammanfattning. `appstreamcli validate` godkänner den.
- **`packaging/check-docs-languages.sh`** är grinden: den faller när en README saknas, när
  språkraden inte länkar till alla, eller när .desktop/metainfo saknar ett språk. Körd i CI, och
  provad genom att ta bort en fil (den föll, som den ska).
- Release-tarballen tar med alla README-filer.

Manualen och CONTRIBUTING är kvar på engelska — det är sagt rakt ut i stället för halvöversatt.


## [0.9.161] - 2026-10-26

**Licensen är nu GPL-3.0-only.** Projektet var dubbellicensierat MIT OR Apache-2.0, vilket är
exakt motsatsen till vad som efterfrågades: båda licenserna tillåter vem som helst att ta koden,
stänga den och sälja den under ett annat namn. GPL-3.0 behåller det som var poängen — bidrag är
välkomna och ingen kan ta programmet och sälja en inlåst version.

- `LICENSE` är GPL-3.0-only, ordagrant ur SPDX (`GPL-3.0-only.txt`), med copyrightraden i README.
  `LICENSE-MIT` och `LICENSE-APACHE` är borta.
- **`CONTRIBUTING.md`** är ny: hur man bidrar, `git commit -s` (DCO) som enda pappersexercis,
  kodkonventionerna, och varför en tredjepartsplugin inte ärver programmets licens (CLAP/VST3 laddas
  som separata binärer vid körning).
- README (båda språken) förklarar licensen med vanliga ord: använd/byg/ändra/dela fritt, dela källkod
  när du delar en ändrad Sonix, inga CLA:er, plugins oberoende, och att **namnet** inte följer med
  koden — en fork bär eget namn och ikon. Samt att versioner till och med 0.9.160 behåller sina
  villkor, eftersom en licens inte är retroaktiv.
- `Cargo.toml`, `nfpm.yaml` (deb/rpm-licensfältet), AppStream-metainfo och release-tarballen pekar
  på den nya licensen; paketen skeppar `LICENSE` + `CONTRIBUTING.md` i stället för de två gamla
  filerna.

## [0.9.160] - 2026-10-26

**Fungerar på alla distributioner.** Tills i dag fanns binären bara som ett tarball byggt på en
Ubuntu-runner: glibc ≥ 2.35 för x86-64 och ≥ 2.39 för aarch64, vilket stänger ute Debian 11,
Ubuntu 20.04 och hela RHEL 8-serien — och en binär byggd på Arch begärde till och med GLIBC_2.44
och startade inte ens på Ubuntu 24.04. Nu byggs releasen i en **glibc 2.28**-container för båda
arkitekturerna, och flödet **stannar** om någon symbol begär nyare.

- **`packaging/check-glibc-floor.sh`** — grinden. Läser binärens GLIBC-symbolversioner och faller
  över golvet. Körs både i CI och i releaseflödet.
- **`packaging/verify-in-docker.sh`** — beviset. Installerar paketet i en riktig distribution och
  startar appen med Xvfb; ett nytt CI-jobb kör den på Debian 12, Ubuntu 22.04, Ubuntu 24.04,
  Fedora 41 och Rocky 9.
- **Fyra former av samma binär:** `.deb` (Debian, Ubuntu, Mint, Pop!…), `.rpm` (Fedora, RHEL,
  Rocky, Alma, openSUSE), **AppImage** (ingen installation alls) och tarballen som förut.
  `SHA256SUMS` ligger bredvid och alla filer attesteras med byggbevis.
- **`packaging/nfpm.yaml`** — paketberoendet deklareras per paketformat (`libasound2` för deb,
  `alsa-lib` för rpm) och `ffmpeg` som rekommendation, eftersom WAV/FLAC fungerar utan.
- **`packaging/io.github.alexwest1981.Sonix.{desktop,metainfo.xml}`** — desktoppost, ikon och
  AppStream-metadata som `appstreamcli validate` godkänner, så appen syns i programmenyer och
  mjukvaru-butiker med namn, ikon och skärmdumpar.
- README (båda språken) svarar nu på frågan en ny användare har: *vilken rad ska jag köra?* — en
  tabell per distribution, vad varje form installerar, och hur den tas bort.

## [0.9.159] - 2026-10-25

**aarch64-release.** Release-workflowen bygger nu båda arkitekturerna: x86_64 på `ubuntu-22.04`
och aarch64 på `ubuntu-24.04-arm`, som två jobb som laddar upp var sin artefakt och en tredje som
attesterar och publicerar dem tillsammans. Det var den sista varningen i OmaStore — utan en
arm-tillgång ser appen "inte installerbar" ut för aarch64-användare.

- `omastore.toml` deklarerar nu `linux.aarch64` med samma `{version}`-mönster.
- ARM-binären kräver glibc ≥ 2.39 (Ubuntu 24.04); x86_64 ligger kvar på ≥ 2.35.
- `.SRCINFO`/`PKGBUILD` deklarerar `alsa-lib` och `glibc`, så butiken kan kontrollera beroenden.

## [0.9.158] - 2026-10-25

Första releasen sedan 0.9.0 — **579 commits**. Den fullständiga listan står i
[CHANGELOG-DETAIL.md](CHANGELOG-DETAIL.md); det här är vad man märker.

- **Masterns FX-rack**: Love Philter (tre filter i serie), Luxeverb (FDN), Emphasis,
  Hyper Chorus, Maximus (tre band + master, solo per band, ritbara volymkurvor),
  16-bands vocoder med egna bandnivåer och Time Curve som bandspelarhastighet.
- **Tempot mäts ur ljudet** vid import och per klipp, och **tempokartan gäller överallt**:
  uppspelning, sökning, nedräkning och export. Klipp utan känt tempo behåller sina sekunder.
- **Edison, Newtone och Newtime** som redigerare på urval, med spektrum, tonhöjd och tid.
- **FLEX-fabriken**: åtta namngivna ljud som följer med appen.
- **38 skärmdumpar** i `screenshots/`, tagna mot den här versionen.

## [0.9.0] - 2026-09-11

First public release. Everything advertised in the UI is backed by the real
audio engine; the remaining gaps are documented honestly in
[README.md](README.md#-whats-left--how-far-from-real) and [ROADMAP.md](ROADMAP.md).

### Added

- **Audio engine** — real-time cpal/ALSA engine with a crash-safe callback;
  oscillator synth (Analog Alchemy), drum synthesis, per-voice filter and
  filter-envelope, master FX chain, delay/reverb, Remix FX (stutter/reverse),
  tape-stop, and modular patcher DSP with topological sorting.
- **Sequencer & arranger** — multitrack timeline with linear audio regions,
  slice tool, loop-dragging, 16-step Channel Rack, Piano Roll with MIDI
  keyboard recording, song-section arranger, and automation curves.
- **Mixer** — faders, real VU and gain-reduction meters, per-track 3-band
  EQ / compressor / sends / pitch, VCA groups and sub-mix buses.
- **Vocal Studio** — microphone recording, take lanes and comping, pitch
  editor, offline and real-time autotune, formant-preserving pitch shift,
  4-voice harmonizer, sampler, and WSOLA pitch-preserving time-stretch.
- **Generators** — chord matrix, beat/melody dice, Session Drummer, strobe
  tuner, and add-track templates.
- **AI** — local rule-based composer, external LLM support
  (OpenAI / Anthropic / OpenRouter / Ollama), audio generation APIs
  (OpenAI TTS / Stability), and project-context awareness.
- **Stem separation** — DSP separator plus optional neural HTDemucs via ONNX
  (`--features neural` with a user-supplied model), and Suno ZIP import.
- **Plugin hosting** (opt-in `--features plugin-host`) — in-process CLAP host,
  per-track audio processing with PDC, plugin state/preset save-load, CLAP GUI
  in its own X11 window, out-of-process sandbox with crash detection/restart
  and shared-memory audio transport, plus hand-rolled VST3 and VST2 ABI hosts
  with parameters, state and PDC.
- **Export & project I/O** — WAV/FLAC in-app, MP3/OGG/AAC via `ffmpeg`,
  offline full-project renderer with real samples and clean metadata, export
  presets, loudness normalization, and project save/load.
- **Hardware** — real ALSA MIDI (MCU) control and a UDP OSC server.
- **Localization** — 7 languages: Swedish, English, Danish, Norwegian, German,
  Spanish and French.
- **Documentation** — English and Swedish README, roadmap, tooling history and
  a full user manual.
- **Release infrastructure** — dual-licensed under MIT OR Apache-2.0, with
  GitHub Actions CI (default, `plugin-host` and `neural` builds).

### Fixed

- Tape-stop idle passthrough: no ring-buffer latency and no initial silence
  when the effect is disabled.
- Parallel-test race in the CLAP test fixture by giving each mock plugin
  instance its own state.
- Stale "not implemented" plugin/sandbox texts and the `.fst` preset
  rationale, which now state the real reason (the closed format cannot be
  decoded).

[Unreleased]: https://github.com/alexwest1981/sonix/compare/v0.9.175...HEAD
[0.9.175]: https://github.com/alexwest1981/sonix/compare/v0.9.174...v0.9.175
[0.9.174]: https://github.com/alexwest1981/sonix/compare/v0.9.173...v0.9.174
[0.9.173]: https://github.com/alexwest1981/sonix/compare/v0.9.167...v0.9.173
[0.9.167]: https://github.com/alexwest1981/sonix/compare/v0.9.165...v0.9.167
[0.9.159]: https://github.com/alexwest1981/sonix/compare/v0.9.158...v0.9.159
[0.9.158]: https://github.com/alexwest1981/sonix/compare/v0.9.0...v0.9.158
[0.9.0]: https://github.com/alexwest1981/sonix/releases/tag/v0.9.0
