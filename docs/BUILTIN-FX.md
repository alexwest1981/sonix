# Sonix egna effekter (`builtin:`)

Tio effekter som är **vår kod**, inte någon annans fil. De ligger i `src/audio/builtin_fx/`,
registreras i en tabell och laddas på samma väg som en plugin:

```
builtin:filter72 · builtin:tilt · builtin:moog · builtin:phaserotate · builtin:bassmanager
builtin:ringmod · builtin:pitch · builtin:spectralcomp · builtin:inflator · builtin:align
```

De syns i plugin-listan som formatet **Sonix (inbyggd)**, med kategori, och dyker upp i varje
kanals effektkedja med automatable parametrar — ingen fil att skanna, ingen sandlåda, ingen
`--features plugin-host`.

## Varför egen kod och inte REAPERs filer

JSFX-motorn (`docs/JSFX.md`) kan *köra* REAPERs 256 effekter, men de filerna är REAPERs och
skeppas aldrig med Sonix. De här nio är skrivna här: ingen rad är kopierad, ingen port med
attributionshuvud, inget beroende till `InstallData/Effects`. Algoritmerna (Butterworth, ladder,
STFT-kompression, tvåtaps-pitch) är allmängods; implementationen är vår och står under Sonix
GPL-3.0-only.

## Effekterna

| Id | Vad den gör | Mätt i proven |
| :--- | :--- | :--- |
| `filter72` | Butterworth låg-/högpass, 24/48/72 dB per oktav | 24 dB: −49 dB två oktaver över gränsen; 72 dB minst 30 dB brantare |
| `tilt` | Lutning kring en vridpunkt | +6 dB vid 700 Hz: basen ned, diskanten upp; 0 dB är oförändrat |
| `moog` | Fyrpolig ladder med mättnad i återkopplingen | ändlig och under topp 8 vid full resonans och 24 dB drive |
| `phaserotate` | Andra ordningens allpass | 180° vid gränsfrekvensen, nivån orörd |
| `bassmanager` | Mono under Hz + hylla + fas, LR4-delning | motfas 50 Hz försvinner; 1 kHz orörd; nivåneutral med mono av |
| `ringmod` | Multiplikation med egen oscillator | likström in → bärvågen ut, RMS 0,707 |
| `pitch` | Realtid ±12 semitoner, två korsande läspunkter | +12 ger 880 Hz ur 440 Hz; 0 semitoner är bitidentisk passthrough |
| `spectralcomp` | STFT-kompression, frekvensberoende tröskel, nedåt/uppåt | ratio 1 rekonstruerar insignalen; 19,5 dB över tröskeln ned 14,6 dB vid ratio 4 |
| `inflator` | Udda ordningens vågformning | toppfaktorn sjunker, toppen stannar under 1 |
| `align` | Sampelnoggrann fördröjning + polaritet per kanal | 1 ms vid 48 kHz = sampel 48; polariteten vänder tecknet |

`spectralcomp` arbetar i 1024-samplers fönster med 75 % överlapp och rapporterar **768 sampel**
latens, så PDC:n kompenserar den som vilken plugin som helst.

## Bygga en ny

1. Skriv en struct med `impl Dsp` (`process`, `set`, `reset`, eventuellt `latency`).
2. Lägg en `EffectDef` med id, namn, kategori och parametrar (`ParamDef::knob`/`step`).
3. Lägg den i `defs()` i `mod.rs`.

Parametrarna *är* gränssnittet: namn, intervall och standardvärde räcker för rattarna, och
`set(index, value)` säger vilken ratt som vrids. Ingen UI-kod per effekt.

Effekten har ingen egen GUI, så **GUI-knappen öppnar en Rattar-ruta** med en slider per
parameter. Den läser värdet ur instansen och skriver tillbaka till samma handtag som ljudtråden
läser — inget lås på ljudtråden, bara en atomär per parameter.

Prov: `cargo test --bin sonix builtin_fx` — varje effekt laddas, tar alla sina parametrar och
lämnar ändligt ljud; de akustiska egenskaperna prövas i samma svit.
