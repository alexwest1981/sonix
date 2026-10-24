# Additiv och spektral syntes i full längd

**Status: steg 1–4 byggda (tabelloscillator med 64 partialer, egen kontur per partial,
spektralt filter och tidsaxel); kvar är Sytrus 6-op och redigering per partial i UI:t.** Den här filen finns för att nästa pass ska börja i rätt ände i stället för
att uppfinna omfattningen.

## Det som finns i dag

- `AdditiveTable` — en delad tabell med summan av **64 partialer**, en tabelläsning per sampel.
  **Mätt** (release, i7-8700K, 16 röster, 1 s ljud): oscillatorn **241× realtid**, hela rösten
  med filter och envelop **30× realtid**; en `sin()` per partial i samma mätning gav 3,4×.
  Tabellen byggs om när dragstängerna ändras (0,2 ms) och bandbegränsas inte — en hög not kan
  vika partialer ovanför Nyquist.
- **Egen kontur per partial** (`partial_envs: [[attack, decay, sustain]; 64]`): varje partial
  kan ha sin egen attack, decay och sustain, så spektret kan mörkna med tiden. Nivåerna räknas
  om var 32:a sampel och rösten summerar partialerna per sampel (tabellen kan inte användas när
  konturerna skiljer sig). **Mätt:** centroid **4,3 → 1,03** över 500 ms när partial 1 står kvar
  och partial 8 dör på 0,25 s. Kostnaden i värsta fallet — 16 röster à 64 utdragna partialer med
  egen kontur — är **2,0× realtid**; utan kontur går samma patch i 240×.
- **Spektralt filter** (`SpectralFormant { center, width, amount }`): en Gaussisk klangform över
  **partialindex**, inte över Hz — därför flyttar den klangfärgen men inte tonhöjden. Nivåerna
  formas i `shaped_harmonics` innan tabellen byggs, så både den snabba och den långsamma vägen
  ser samma spektrum. **Mätt:** formant på partial 2 → centroid **4,59**, på partial 9 →
  **8,69**; grundtonen mäts till **110,0 Hz** i båda fallen. `amount = 0` lämnar ljudet bit-identiskt.
- **Tidsaxel** (`SpectralSweep { to, seconds }`): klangformen glider linjärt från formantens
  läge till `to` under `seconds`, räknat från anslaget och **per röst** — två toner som startar
  olika sent glider olika långt. **Mätt:** två kurvor som startar på partial 1 ger samma centroid
  vid anslaget (**4,61** mot **4,63**) men olika vid 0,5 s (**6,42** mot **4,59**). Det är en
  svepande formant, inte Harmors godtyckliga bild/'tidskurva' — den kurvan är kvar.
- `Waveform::Inharmonic` — partialerna stäms om, men nivåerna är fortfarande statiska.
- `Waveform::Bowed`/`Guitar`/`Pluck` — strängmodell, inte additiv.
- `audio::spectral_image` — en PNG blir **64 statiska partialnivåer** (Harmors bildväg).
- `audio::flex` — presets, inte syntes.
- Sytrus-vägen är **FM med 2 operatorer** (`FmParams`), inte 6.

Alltså: en additiv oscillator med 16 rattar, ingen spektral kontroll och ingen tid.

## Vad "full längd" betyder

Fyra saker, och de är oberoende nog att byggas i ordning:

1. **Fler partialer, med rimlig kostnad.** 16 → 64 partialer per röst. 64 sinus per röst × 16
   röster = 1 024 oscillatorer; det går i en tabellbaserad oscillator (wavetable per partial
   delas mellan rösterna), inte i 1 024 `sin()`-anrop per sampel. Mät först: Nordic-traktorn
   (Alex' maskin) ska klara 64 partialer utan att `realtime_bench` tappar buffertar.
2. **Egen amplitudenvelopp per partial** (Harmors kärna): varje partial får attack, decay och
   sustain. Det är detta som gör att ett anslag låter som ett anslag och inte som en orgel.
   Lagras som `[[f32; 3]; 64]` per röst — 768 byte, ryms i kommandot.
3. **Spektralt filter**: en kurva över partialindex (inte över Hz), så en formant kan flyttas
   utan att grundtonen ändras. Det är skillnaden mellan ett filter och en spektral form.
4. **Tidsaxeln** (Harmors `.fnv`): partialnivåerna får ändras över notens längd, inte bara
   genom en envelopp. Kräver att kurvan samplas per block, inte per not.

## Ordning och mått

| Steg | Vad | Mått som avgör om det är klart |
|---|---|---|
| 1 | 64 partialer, tabellbaserad oscillator — **✅** | **Mätt 2026-10-24:** 16 röster à 64 partialer, 1 s ljud i release: tabellen **241× realtid**, hela rösten **30× realtid**. En `sin()` per partial i samma mätning: 3,4×. Prov: tabellen matchar den direkta summan (97 faser, < 2e-3) och den 64:e partialen hörs (7 040 Hz ur 110 Hz). |
| 2 | Envelopp per partial — **✅** | **Mätt 2026-10-24:** `per_partial_envelopes_darken_the_spectrum` — partial 1 med full sustain, partial 8 med 0,25 s decay till 0: centroid **4,30 → 1,03** mellan 50–100 ms och 400–450 ms (Hann-fönster; FX avstängda, annars mäter provet delayens 300 ms-eko). Konturen är linjär attack → linjär decay → sustain; UI:t sätter `decay / (k+1)^lutning` så höga partialer dör först, och kommandot tar exakta värden per partial. |
| 3 | Spektralt filter — **✅** | **Mätt 2026-10-24:** `a_spectral_formant_moves_the_centroid_but_not_the_pitch` — formant på partial 2 ger centroid **4,59**, på partial 9 **8,69**, och `fundamental_hz` säger **110,0 Hz** i båda fallen. Formen är Gaussisk över partialindex (center, bredd i partialer, mängd 0–1); UI:t har tre reglage, kommandot `SetSpectralFormant`. |
| 4 | Tidsaxel — **✅** | **Mätt 2026-10-24:** `the_time_axis_moves_the_spectrum_after_the_attack` — kurva A (formant 1 → 9 på 1 s) och kurva B (1 → 2) ger centroid **4,61 / 4,63** vid anslaget och **6,42 / 4,59** vid 0,5 s. Per röst (tiden räknas från anslaget), så polyfoni inte delar svepfas. Formen är en linjär formantglidning; en godtycklig kurva/bild över tid är nästa steg om det behövs. |

## Mätt först: behövs tabellen?

Samma loop som `Waveform::Additive` gör (en `sin()` per partial och röst), 16 röster, 1 sekund
ljud i 48 kHz, mätt på Alex' i7-8700K:

| Partialer | Tid för 1 s ljud | Realtidsfaktor |
|---|---|---|
| 16 (i dag) | 0,131 s | **7,6×** |
| 32 | 0,256 s | **3,9×** |
| 64 | 0,491 s | **2,0×** |

Slutsatsen är att steg 1 **måste** vara tabellbaserat: 64 partialer med `sin()` per sampel tar
halva en kärna innan filter, enveloper och FX ens är räknade. En tabell per partial (delad
mellan rösterna) byter ut 64 `sin()` mot 64 tabelläsningar, och en läsning är billigare än en
transcendent funktion. Mät om efter bytet — samma tabell ska ge ≥ 10× vid 64 partialer.

**Utfall (mätt 2026-10-24, release, i7-8700K):** en gemensam tabell med partialsumman (inte en
tabell per partial — summan behöver bara en läsning) ger **241× realtid** för oscillatorn och
**30× realtid** för hela rösten med filter och envelop. Samma mätning med `sin()` per partial:
3,4×. Ombyggnaden tar 0,25 ms. Provet `the_additive_table_matches_the_direct_sum` vaktar att tabellen är samma summa,
och `the_sixty_fourth_partial_is_heard` att taket inte längre ligger vid 16.

## Steg 5: bandbegränsning (byggd 0.9.40)

Tabellen var en enda summa för hela klaviaturen, så en hög not vek partialer över Nyquist. En
partial vars frekvens passerar Nyquist hamnar på `sample_rate − f`, alltså *in* i det hörbara
området: C7 (2 093 Hz) med alla 64 dragstänger lade partial 20 (41,9 kHz) på 6 140 Hz.

- **En tabell per distinkt partialgräns**, inte per not. `partial_limit(band, sample_rate)` räknas
  ur bandets **lägsta** ton (`band_of(note) = note/12`, elva oktavband), så ingen not i bandet kan
  vika en partial ner i hörbart område. De sex låga banden har alla 64 partialer och delar därför
  tabell; arbetet per omdragning är summan av de distinkta gränserna, inte 11 × 64.
- **Normaliseringen flyttade till rösten** (`refresh_band_gain`): vikten räknas över bandets
  **formade** partialer (dragstänger × formant i basläget), samma vikt tabellen byggs av. Då
  tappar inte en hög not volym när bandet kortas, och tabellvägen och den långsamma vägen
  (`partial_sum`, konturer/tidsaxel) får samma volymregel.

**Mätt** (C7, MIDI 96, alla 64 dragstänger, 48 kHz, Hann-fönster): alias/grundton **0,511 före**,
**0,0045 efter** (−41 dB). Grundtonen blev **6× starkare** (0,00749 → 0,04354) eftersom vikten nu
räknas över bandets elva partialer i stället för alla 64. Provet `a_high_note_is_band_limited`
fälldes med flit genom att stänga av gränsen — då föll det på aliasnivån, inte på gränstalet.

**Provhygien som mätningen avslöjade:** elva band per tabellbygge gjorde prov som skapade en
tabell per sampel minuter långa. De använder nu en delad `default_table()` (`OnceLock`) — hela
sviten gick från 10+ minuter till under två.

## Vad som inte ska byggas

- Ingen ny motor vid sidan av `SynthEngine`: partialerna hör till `Waveform::Additive`, och
  rösterna, enveloperna och MPE-vägen är desamma.
- Ingen egen fil för partialkurvor. De hör i projektet, som `harmonics` gör i dag.
- Ingen spektral (FFT-baserad) omsyntes i steg 1–4. Den vägen är en annan, dyrare metod
  (analys + omsyntes per block) och hör till ett eget beslut.

## Sytrus 6-op

Det är en **annan** uppgift än partialerna: sex operatorer med egna enveloppper och en
kopplingsmatris (varje operator kan modulera varje). Byggs efter steg 1–2, för då finns
envelopperna som behövs. Kopplingsmatrisen är 6×6 nivåer plus per-operatorns vågform —
det är den delen som gör Sytrus till Sytrus.

## 6-op (Sytrus): matrisen

`audio::synth::six_op` är sex sinusoperatorer i en **6×6-matris** (`matrix[källa][mål]`), plus
förhållande och utgångsnivå per operator. `Waveform::SixOp` spelar den; `AudioCommand::SetSixOp`
byter hela patchen i **ett** kommando (48 enskilda hade kunnat ge halva patchar).

**Ordningen är regeln:** en källa med lägre index räknas först och läses direkt ur samplet; en
källa med samma eller högre index läses från **föregående** sampel. Det ger egen återkoppling
(sågtand ur en sinus) utan att bli implicit, och gör framåtriktade kopplingar möjliga.

**Mätt** (48 kHz, 9 600 sampel, 440 Hz):

| Prov | Mätning |
| :--- | :--- |
| `without_modulation_six_op_is_a_sine` | andra övertonen < 1 % av grundtonen |
| `modulation_puts_sidebands_where_fm_says` | modulering 0,5 × f₀ med index 2,0 ger sidoband på 220 och 660 Hz, båda > 20 % av bärvågen |
| `feedback_uses_the_previous_sample` | egen återkoppling (1,5) ger 2:a och 3:e övertonen |
| `the_six_op_matrix_is_audible_in_an_offline_render` | ren sinus 0,0078 mot kedja 0,0078 → **fälldes** genom att ta bort `SetSixOp` ur exporten; med den på plats ger kedjan övertoner |

### Envelop per operator

`SixOpParams.envs: [AdsrParams; 6]` sitter **före** matrisen: den formar både det som hörs och
det som modulerar. Standard är neutral (attack 0, sustain 1) — en patch som inte rört envelopen
låter som före 0.9.48. `SixOpVoice` äger sex `AdsrVoice` som `gate_on` vid anslag och `gate_off`
vid släpp.

**Mätt** (`each_operator_has_its_own_envelope`): decay 50 ms med sustain 0 tar grundtonen från
anslaget till under 20 % på 75 ms; samma envelop på en **modulator** tar sidobandet på 660 Hz
från hörbart till under 20 % — alltså är det envelopen som styr sidobanden, inte bara volymen.

### Filter per operator

`SixOpParams.filters: [OpFilter; 6]` (cutoff + resonans) sitter **efter** envelopen och **före**
matrisen, så en filtrerad modulator tar sina övertoner med sig in i ljudet i stället för att bara
ändra sin egen nivå. `cutoff >= 19 000` betyder av — filtret räknas då inte alls, och en patch
som inte rört det låter som förut.

**Mätt** (`the_operator_filter_takes_the_high_side_band_away`): bärvåg 440 Hz med sidoband på 220
och 660; lågpass 300 Hz på bärvågen dämpar 660 Hz till under 30 % och lämnar 220 Hz över 60 %.

`OpFilter` är en egen struct och inte `FilterParams`, eftersom sex `FilterParams` i en array inte
gick genom serde-vägen — två fält klarar sig själva.

**Kvar:** fler än de tre utgångslägena (sinus/kedja/sågtand).
