# Additiv och spektral syntes i full längd

**Status: steg 1–2 byggda (tabelloscillator med 64 partialer, egen kontur per partial);
steg 3–4 kvar.** Den här filen finns för att nästa pass ska börja i rätt ände i stället för
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
| 3 | Spektralt filter | En formantförskjutning flyttar centroid men **inte** grundtonen (mätt i Hz) |
| 4 | Tidsaxel | Samma not med kurva A och kurva B ska ge olika centroid vid 0,5 s men samma vid 0 s |

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
