# Additiv och spektral syntes i full längd

**Status: plan.** Den här filen finns för att nästa pass ska börja i rätt ände i stället för
att uppfinna omfattningen. Inget av det nedan är byggt utom det som står under *det som finns*.

## Det som finns i dag

- `Waveform::Additive` med **16 övertoner** (`HARMONIC_COUNT`), statiska nivåer som sätts med
  dragstänger. Ingen tid, ingen egen envelopp per partial.
- `Waveform::Inharmonic` — partialerna stäms om, men nivåerna är fortfarande statiska.
- `Waveform::Bowed`/`Guitar`/`Pluck` — strängmodell, inte additiv.
- `audio::spectral_image` — en PNG blir **16 statiska partialnivåer** (Harmors bildväg).
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
| 1 | 64 partialer, tabellbaserad oscillator | `realtime_bench` utan tappade buffertar vid 64 partialer, 16 röster |
| 2 | Envelopp per partial | Ett anslag med bara partial 1 långsam och partial 8 snabb ska ge **avtagande spektrum** — mätt som centroid som faller över 500 ms |
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
