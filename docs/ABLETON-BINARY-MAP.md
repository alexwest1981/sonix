# Ableton Live 12.4.5 — vad binären och förinställningarna innehåller

Fortsättning på `ABLETON-INVENTORY.md` §10. Där stod det uttryckligen att binärerna lästes **som
strängar, inte disassemblerade**, och att `.adv`/`.adg` räknades men **inte tolkades**. Den här
filen stänger de två luckorna och lägger till en tredje (Python-lagret).

Allt nedan är mätt på den uppackade installationen i
`/mnt/fastnvme/hermes/cache/scratch/ableton` (6,2 GB, `innoextract` 1.13 byggd från git master —
systemets 1.9 klarar inte Inno Setup 6.4.0.1: *"Unexpected setup data version: 6.4.0.1 / Stream
error while parsing setup headers"*).

| Mätning | Verktyg |
| :--- | :--- |
| PE-huvud, sektioner, entropi, data directories | egen parser på råbytes (Python, stdlib) |
| Imports/exports | `objdump -p` |
| Klassnamn och algoritmer | RTTI (`.?AV…@@`) ur `strings`, 457 567 ASCII-rader |
| Presets | `gzip` + `xml.etree` |
| Python | `marshal` under CPython 3.11 (`~/.local/bin/python3.11`) |

---

## 1. `Ableton Live 12 Suite.exe`

| Fält | Värde |
| :--- | :--- |
| Storlek | **148 197 832 B** (141,3 MiB) |
| sha256 | `df7d1f2e58f294b2c4a411e021a094ccd17272609ffe94923ddd7b3f312cb02b` |
| Format | PE32+ (x86-64), Windows GUI-subsystem, 9 sektioner |
| Länkare | MSVC **14.44** |
| Tidsstämpel | 2026-08-19 10:35:42 UTC |
| Entry point | RVA `0x24e1c68`, i `.text` |
| Image base | `0x140000000`, `SizeOfImage` 0x08ff7000 |
| Debug | en post, PDB-sökväg `F:\Jenkins\live\output\Live\win_64_static\Release\Live.pdb` |
| Autentiseringskod | certificate-directory 10 696 B (ingen kontroll av vem som signerat — se §6) |

### 1.1 Sektioner och entropi

| Sektion | Virtuell storlek | Rå storlek | Entropi (Shannon, råbytes) | Flagg |
| :--- | ---: | ---: | ---: | :--- |
| `.text` | 0x6021296 | **100 799 488** | **6,535** | kod |
| `IPPCODE` | 0x1686cb | 1 476 608 | 6,307 | kod |
| `.rdata` | 0x20f85fc | **34 571 776** | **6,449** | data |
| `.data` | 0x852469 | 5 974 016 | 4,803 | data |
| `.pdata` | 0x484fb0 | 4 739 072 | 7,315 | data |
| `IPPDATA` | 0xd48 | 3 584 | 3,663 | data |
| `_RDATA` | 0x4d108 | 315 904 | 6,125 | data |
| `PyRuntim` | 0x28ac0 | 166 912 | **1,099** | data |
| `.rsrc` | 0x21c68 | 138 752 | 5,235 | data |

Summan av råstorlekarna är 148 186 112 B — filen är alltså inte trunkerad och inget ligger utanför
sektionerna.

### 1.2 Är den packad eller skyddad? **Nej.**

Fyra oberoende mätningar säger samma sak:

1. **Inga packarsektioner.** Sektionsnamnen är `.text/.rdata/.data/.pdata/_RDATA/.rsrc/` plus tre
   som tillhör Intel IPP och Python (`IPPCODE`, `IPPDATA`, `PyRuntim`). Ingen `UPX0/UPX1`,
   `.themida`, `.vmp0/.vmp1`, `.aspack`, `.nsp0`, `.enigma`.
2. **Inga packarsträngar.** `UPX!`, `Themida`, `VMProtect`, `WinLicense`, `ASProtect`, `MPRESS`:
   **0 träffar** i både ASCII- och UTF-16-dumpen. (En packad fil skulle dessutom ha noll läsbara
   strängar i `.text`; här finns 457 567.)
3. **Entropin är normal för MSVC-C++.** `.text` 6,535 och `.rdata` 6,449 ligger i det intervall en
   vanlig optimerad C++-binär ger. Krypterad/packad kod ligger på 7,9–8,0. Den enda sektionen över
   7,0 är `.pdata` (7,315), och det är **inte** kod: det är undantagstabellen, en array av
   `RUNTIME_FUNCTION`-poster som är nästan unik per funktion och därför hög-entropisk. Att peka på
   den som "packad" vore ett mätfel.
4. **Importtabellen är normal, inte strypt.** 56 DLL:er och 1 487 importerade funktioner, med
   `KERNEL32.dll` först. En packare brukar lämna 3–8 importer.

Entry point ligger i `.text`, inte i en stubbe utanför sektionerna — ytterligare ett skäl att
avskriva packning.

**Slutsats: binären är ren, ostrypt MSVC-kod.** Ingen anledning att gissa; den gick att läsa.

### 1.3 Vad importerna säger om motorerna

56 DLL:er. De som säger något om vad som finns inuti:

| Import | Vad det avslöjar |
| :--- | :--- |
| `onnxruntime_abl.dll`, `d3d11.dll` | ONNX-inferens på GPU (DirectML) — samma spår som `musicai.dll` |
| `REX Shared Library.dll` | Propellerhead/Reason REX-filformat, inläst direkt i processen |
| `USFLIB.dll` | Abletons egen ljudfilskatalog (namnet förekommer bara här) |
| `DSOUND.dll`, `WINMM.dll`, `AVRT.dll` | DirectSound/WaveOut direkt; **ASIO laddas dynamiskt** (ingen import) |
| `WebView2Loader.dll` | HTML-paneler via Edge-runtime |
| `libusb-1.0.dll` | USB-kontrollers (Push m.fl.) direkt, utan drivrutin |
| `OPENGL32.dll`, `d3d11.dll`, `dwmapi.dll` | tre renderingsvägar i samma binär |
| `MF.dll`, `MFPlat.DLL`, `MFReadWrite.dll`, `RTWorkQ.DLL` | Media Foundation — video och MP3/AAC-avkodning |
| `UIAutomationCore.DLL` | tillgänglighets-API:t (samma som AccessKit ersätter hos oss) |
| `dbghelp.dll`, `bcrypt.dll`, `CRYPT32.dll`, `WININET.dll` | kraschdumpar, licensiering, nedladdning |

**Det som INTE finns i importtabellen är lika intressant:** ingen `python3*.dll`, ingen `zlib1.dll`,
ingen `lzma`, ingen Qt, ingen `fftw`, ingen `libsndfile`. Allt det är **statiskt länkat** — därav
den 100 MB stora `.text`. Att Python är inbäddat statiskt syns på `Py_InitializeFromConfig` och 166
`PyObject_*`/`PyEval_*`-symboler i strängarna, och på att `Program/` inte innehåller någon
Python-DLL.

Exporttabellen har **1 734 poster** (RVA 0x826b590). Det är den yta Max for Live och
Extensions-processerna anropar.

### 1.4 Sektionen `PyRuntim` (166 912 B, entropi 1,099)

Låg entropi = mestadels nollor. Innehållet är 282 strängar, och de är **inte** användarkod utan
CPython-infrastruktur: `Py_Repr`, `__IOBase_closed`, `__abc_tpflags__`, `__abstractmethods__`,
`TextIOWrapper`, `WarningMessage`, `utf-8`, `list index out of range`. Det är tabellerna för
typslottar och undantag — den minimala Python-kärnan som behövs för att kunna starta ett skript
utan att länka in hela stdlib.

---

## 2. Enhets-DSP i binären

Två oberoende lager syns i strängarna:

* **6 087 unika RTTI-klassnamn** (`.?AV…@@`), dvs. riktiga C++-klasser med arv — inte etiketter.
* **382 klasser som slutar på `Processor` och börjar på `O`** — det är DSP-objekten i den graf som
  Live kör (`ORoarProcessor`, `OInstrumentWavetableProcessor`, `OHybridProcessor`,
  `OSpectralTimeProcessor`, `OVocoderProcessor`, `OGrainDelayProcessor`, `OTransmuteProcessor` …).

Det betyder att **DSP:n ligger i exe-filen**, inte i `Ableton Live Engine.dll` (28 MB, bara 809
RTTI-klasser, varav en handfull enhetsnamn). Engine.dll är motor- och schemalagret; ljudet ligger i
huvudbinären.

### 2.1 De 72 enheterna — vad som är bekräftat som klass

Namngivningen är `A<Namn>` för enhets-/UI-objektet, `O<Namn>Processor` för DSP:n, `L<Namn>` för
något tredje (sannolikt "legacy"/layout-varianten).

| Instrument | Klassbevis i exe |
| :--- | :--- |
| Operator | `AOperator`, **`OInstrumentOperatorProcessor`**, `OOperatorVoiceProcessor`, `OOperatorWavetableProcessor`, `AOperatorAlgorithmView`, `AOperatorGlobals` |
| Wavetable | `AInstrumentWavetable`, **`OInstrumentWavetableProcessor`** |
| Tension | `ATensionAdsrEnvelopeDisplay`, `ATensionFilterXyControl` (DSP-klass ej hittad under namnet) |
| Drift | `ADrift`, **`ODriftProcessor`**, `ADriftCyclingEnvelopeView` |
| Meld | `AInstrumentMeld`, **`OMeldProcessor`**, `AMeldModulationMatrixListView` |
| Sampler | `ASampler`, `OMultiSamplePlayerProcessor`, `OMultiSampleProcessor`, `OMultiSampleDecoderProcessor`, `OMultiSampleModProcessor` |
| Analog | `AUltraAnalog`, `AAasFilterTypeModule`, `OAasFilterTypeProcessor` (AAS-filtret) |
| Simpler | `ASimpler`, `OSimplerSubOscProcessor`, `OSimplerLfoControlProcessor` |
| Collision | `ACollision`, `OCollisionScaledModeProcessor`, `ACollisionResonator`, `ACollisionMallet` |
| Electric | **ingen RTTI-klass med namnet Electric hittad** |
| Impulse | `AInstrumentImpulse`, **`OInstrumentImpulseProcessor`** |
| External Instrument | ingen processor (den är en routing-nod) |
| Max Instrument | `OMxDeviceProcessor`, `OMxDControlSourceProcessor` |

Ljudeffekter (48): samtliga har `O*Processor` utom `Utility` (ingen klass alls) och `Spectrum`
(`OSpectrumAnalyzerProcessor` finns, men ingen `OSpectrumProcessor`). Bekräftade exempel:
`ORoarProcessor`, `OHybridProcessor`, `OEq8Processor`, `OFilterEQ3Processor`, `OChannelEqProcessor`,
`OCompressor2Processor`, `OGlueCompressorProcessor`, `OMultibandDynamicsProcessor`,
`OSaturatorProcessor`, `OPedalProcessor`, `ACabinet` (endast UI-klass, ingen `OCabinetProcessor`
hittad), `OOverdriveProcessor`, `OTubeProcessor` (Dynamic Tube), `OErosionProcessor`/`OErosion2Processor`,
`OReduxProcessor`/`ORedux2Processor`, `OVinylProcessor`, `OVocoderProcessor`,
`OSpectralTimeProcessor`, `OResonatorProcessor`, `OEnvelopeFollowerProcessor`, `OTunerProcessor`,
`OLooperProcessor`, `OBeatRepeatPlayerProcessor` + 3 syskon, `OGrainDelayProcessor`,
`OFilterDelayProcessor`, `OShifterProcessor`, `OAutoShiftProcessor`, `OAutoFilterProcessor` +
`OAutoFilter2Processor`, `OAutoPanProcessor`/`OAutoPan2Processor`, `OChorusProcessor`/`OChorus2Processor`,
`OPhaserProcessor`/`OPhaserNewProcessor`, `OFlangerProcessor`, `ODelayProcessor`/`ODelayXProcessor`,
`OEchoProcessor`, `OCompressor2Processor`, `OLimiterSampleProcessor`.

MIDI-effekter (11): `OMidiScaleProcessor`, `OMidiChordProcessor`, `OMidiVelocityProcessor`,
`OMidiArpeggiatorProcessor`, `OMidiRandomProcessor`, `OMidiNoteLengthProcessor`,
`OMidiPitcherProcessor`. Note Echo har ingen egen processor med det namnet; närmast är
`OMidiDelayProcessor`/`OMidiDelayProcessorBase` — jag har inte kunnat bevisa att det *är* Note Echo.
`MPE Control` och `Envelope MIDI` har ingen egen processor; MPE-kedjan ligger i stället som
`OMpeEncoderProcessor`, `OMpeDecoderProcessor`, `OMpeFilterProcessor`,
`OEffectiveMpePitchBendProcessor`, `OToAbletonLinkAudioProcessor`.

**Ärligt om frånvaron:** Electric (18 presets) och Utility (11) finns som enheter i biblioteket men
jag hittar dem inte som RTTI-klass. Två förklaringar är möjliga — antingen är de byggda utan RTTI
(inga virtuella funktioner i den del som exporteras) eller så ligger deras DSP i `Engine.dll`. Jag
har inte kunnat avgöra vilket, och påstår därför inget.

### 2.2 Strängar som avslöjar algoritmerna

**Filtret: Cytomic.** Sökvägen i binären är
`F:\Jenkins\live\shared\dsp\BlocksLib\include\ableton\blocks\devices\auto_filter2\filters\AutoFilter2Cytomic.hpp`,
och RTTI:n innehåller den generiska mallen och dess topologier:

```
.?AV?$FilterHandlerBiquadTmpl@Uf64x2@Cytomic_Filters@@U?$SallenK_MS2_SD_MIX@...@@
.?AV?$FilterHandlerBiquadTmpl@Uf64x2@Cytomic_Filters@@U?$StateV_OSR_MD@...@@
.?AU?$LadderT_PRD_SD_MIX@Uf64x2@Cytomic_Filters@@@Cytomic_Filters@@
.?AU?$SallenK_SMP_MD_HP@Uf64x2@Cytomic_Filters@@@Cytomic_Filters@@
```

Läses som: **Sallen-Key (MS2, SMP), State Variable (OSR) och Ladder (PRD)**, var och en i
varianterna `_SD` (single/standard) och `_MD` (multi/oversamplad) samt `_HP`/`_MIX` för uttaget.
`Uf64x2` = SIMD med två `double`. Gränssnittsetiketterna för valen finns också ordagrant:
`LP 12dB MS2`, `HP 12dB MS2`, `BP 12dB OSR`.

**Tidssträckning: zplane.** Två rader, ordagrant:

```
[aufTAKT] V4 Beat Tracking by zplane.development
Elastique Pro V3 Time Stretching and
```

plus byggsökvägen `C:\jenkins3\workspace\aufTAKT_aufTAKT-Windows_4.2.2@5\extern\Algorithms\src\Fft.cpp`
och klassen `CElastiqueProCore`. Alltså: **élastique Pro v3** (sträckning) + **aufTAKT 4.2.2**
(transient-/beatdetektering) — inte egen DSP. Det förklarar varför Abletons warp klarar transienter
som vår WSOLA inte gör (se `src/audio/warp.rs`, som ärligt säger samma sak).

**Warp-lägena** står som strängar: `Beats`, `Repitch`, `Complex Pro`, `Tones`, `Complex`, `Texture`.
Python-API:t räknar upp sju (`rex` som sjunde) — se §4.3.

**Mättnad och vågformning (Roar).** Tolv shaper-lägen (`Stage1_Shaper_Type` har intervallet 0..11)
och etikettlistan i binären lyder, i den ordning den ligger i strängblocket:

```
Soft Sine · Analog Clip · Waveshaper · Digital Clip · Sinoid Fold · Hard Curve · Bass Shaper …
```

Fler etiketter i samma block: `Hard Clip`, `No Clip`, `Post Clip Mode`, `WS Damp`, `WS Linearity`,
`WS Period`. **Osäkerhet:** att `Stage1_Shaper_Type="3"` i en preset betyder just "Digital Clip"
kräver att indexordningen i `rdata` läses, och det har jag inte gjort. Jag citerar listan, inte
mappningen.

**Reverb.** Hybrid Reverbs parametrar syns som etiketter: `Diffusion High Band Frequency`,
`Diffusion Low Band Frequency`, `Diff. Lo Freq`, `Decay Time`, `Density`, `Stereo Image`,
`Reflect Level`, `Sparse`, `Chorus Amount`, `Chorus Rate` — dvs. ett diffusionnät + separat
frekvensbandsuppdelad svans, plus faltning (`AHybridImpulseResponseHandler`,
`AHybridImpulseResponseView`, `AHybridRoutingDisplay`). Två motorer i samma enhet: algoritmisk och
IR.

**Oversampling.** `HiQualityOversampling`, `sInfoTextHeaderHiQualityOversampling`,
`sManeuverTextEq8Oversample` ("Access to Eq8's oversample value") — oversampling är ett
förstklassigt begrepp i både masterkedjan och EQ Eight.

**Sampler-interpolation.** Fyra nivåer, ordagrant: `No Interpolation`, `Good Interpolation`,
`Normal Interpolation`, `Best Interpolation`.

**FFT.** Egen sektion `IPPCODE` (1,47 MB) = Intel Integrated Performance Primitives statiskt
länkat, plus `CzplfFFT_IPP`, `CzplfFFTGen`, `CzplfFFT_If` (zplanes FFT-omslag).

**Max-motorn.** `OMxDeviceProcessor`, `OMxDControlSourceProcessor`, `OMxDemuxProcessor`,
`OMxDMidiAdapterProcessor`, `OMxDRemoteDelayProcessor` — Max-enheter är förstklassiga noder i
samma graf som de inbyggda, inte en sidoprocess.

---

## 3. Förinställningarna — vad som faktiskt står i dem

### 3.1 Formatet

`.adv` och `.adg` är **gzip-komprimerad XML**. Exempel (Roar, 3 208 B på disk → 41 418 B XML, 13:1):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<Ableton MajorVersion="5" MinorVersion="12.0_12402" SchemaChangeCount="1"
        Creator="Ableton Live 12.4d1" Revision="42bc084cd8152e4fb737ef3fefd04b2bea5320e7">
```

Schemats byggstenar, räknade i `Acid Distortion.adv` (91 parametrar):

| Element | Betydelse |
| :--- | :--- |
| `<Parameter>/<Manual Value="…"/>` | parameterns **faktiska värde** — det här är siffran |
| `<MidiControllerRange><Min/><Max/>` | parameterns intervall, i samma enhet som värdet |
| `<AutomationTarget Id="…"><LockEnvelope/></AutomationTarget>` | automationsmål |
| `<ModulationTarget Id="…"/>` | modulationsmål (för LFO/envelope) |
| `<LomId/>` | Live Object Model-id |
| `<MidiCCOnOffThresholds>` | på/av-trösklar för MIDI-mappning |
| `<ModulationSourceCount>`, `ModulationSources_*` | enhetens egna modulationskällor |

Värdena är **råa**, inte procent: frekvenser i Hz, gain i linjär amplitud (0,8767 = −1,14 dB),
`_Type` som heltal in i en enum.

### 3.2 Enhet 1 — **Roar** (multibandssaturator, 64 presets i `Devices/Audio Effects/Roar`)

Varför den: §8 säger att Sonix har "saturator" men inte multiband/mid-side/routing, och Roar är
Live 12:s svar på hela den frågan. Preset: `Acid Distortion.adv`, **91 parametrar**. Tre block,
ordagrant ur filen:

```xml
<Stage1_Shaper_Type>  0..11    = 3
<Stage1_Shaper_Amount> 0..1    = 0.2142857164
<Stage1_Shaper_Bias>  -1..1    = -0.2857142687
<Stage1_Filter_Type>  0..7     = 0
<Stage1_Filter_Frequency> 20..20000 = 668.097107
<Stage1_Filter_Resonance> 0..1 = 0.5
<Stage1_Filter_Morph> 0..1     = 0
<Stage1_Filter_PeakGain> 0.251..3.98 = 1
<Stage1_Filter_PreOn>          = true
```

```xml
<Input_LowMidCrossover>  20..5000   = 199.999985
<Input_MidHighCrossover> 200..18000 = 1999.99976
<Input_ToneAmount>  -1..1     = 0.1746032238
<Input_ToneFrequency> 80..2000 = 410.186371
<Input_Blend> 0..1            = 0            <!-- 0 = serial, 1 = parallel -->
```

```xml
<Feedback_FeedbackAmount> 0..1    = 0
<Feedback_FeedbackTimeMode> 0..4  = 0
<Feedback_FeedbackFrequency> 50..18000 = 49.9999962
<Feedback_FeedbackBandwidth> 0.5..9 = 0.5
<Feedback_FeedbackGateOn>        = true
<Output_CompressionAmount> 0..1  = 0.1507936567
<GlobalModulationAmount> 0..2    = 1.203125
<Output_DryWet> 0..1             = 0.7421875
```

Det som går att läsa ut utan att gissa:

* **Tre identiska stag** (`Stage1/2/3`), varje stag = shaper + filter + eget `PreOn`. Det är
  multibandet: stagen matas av två delningsfrekvenser i `Input_*`.
* **Filtret har `Morph` och `PeakGain`** utöver frekvens/resonans — samma Cytomic-topologier som i
  §2.2, med `Type` 0..7.
* **Feedback-loopen är enheten i enheten**: bandbredd 0,5–9 (oktav/enhet), egen frekvens, gate,
  invertering och fyra tidslägen.
* **Enheten har egen modulation inbyggd**: 2 LFO:er, envelope, noise, alla med `SmoothingAmount`.

### 3.3 Enhet 2 — **Wavetable** (271 presets)

Varför den: §8 listar ingen wavetabellmotor alls hos Sonix. Preset:
`Ambient & Evolving/Drifting Ambient Pad.adv`:

```xml
<Voice_Oscillator2_Pitch_Transpose> -24..24 = -7
<Voice_Oscillator2_Pitch_Detune> -0.5..0.5  = 0
<Voice_Oscillator2_Wavetables_WavePosition> 0..1 = 0.2095238119
<Voice_Oscillator2_Effects_Effect1> -1..1   = 0
<Voice_Oscillator2_Effects_Effect2> 0..1    = 0
```

```xml
<Voice_Filter1_Type> 0..4        = 0
<Voice_Filter1_CircuitLpHp> 0..4 = 0
<Voice_Filter1_CircuitBpNoMo> 0..1 = 0
<Voice_Filter1_Slope> 0..1       = 1
<Voice_Filter1_Frequency> 20..20480 = 10017.2666
<Voice_Filter1_Resonance> 0..1.25 = 0.6051587462
<Voice_Filter1_Drive> 0..24       = 0
<Voice_Filter1_Morph> 0..1        = 0
```

```xml
<Voice_Modulators_AmpEnvelope_Times_Attack> 0..20 = 4.61762476
<Voice_Modulators_AmpEnvelope_Slopes_Decay> -1..1 = 0.5
<Voice_Modulators_AmpEnvelope_Sustain> 0..1 = 0.5011875629
<Voice_Modulators_AmpEnvelope_LoopMode> 0..2 = 0
<Voice_Modulators_Envelope2_Values_Peak> 0..1 = 0.9740259647
```

Det här är den enda platsen där ett *filter med `Morph` + två circuit-val* syns i en preset — och
det bekräftar att Wavetable har två filter med varsin routing (`serial/parallel/split`, §4.3),
`Slope` som ett på/av (12 eller 24 dB), drive upp till 24 dB och resonans till 1,25.

### 3.4 Enhet 3 — **Drum Rack** (`Acuff Kit.adg`, 145 rack i `Racks/Drum Racks`)

Varför den: det är den struktur `SessionDrummer` saknar — pads, choke-grupper och nästlade kedjor.
98 261 B gzip → **1 501 103 B** XML, 2 076 parametrar, 172 `FileRef`.

```
GroupDevicePreset
  Device
    InstrumentGroupDevice
  PresetRef
    FilePresetRef → "Racks/Drum Racks/Acoustic/Acuff Kit.adg"
  BranchPresets
    InstrumentBranchPreset          ← rackets yttre kedja
      DevicePresets
        GroupDevicePreset
          Device  DrumGroupDevice   ← trumracket
          BranchPresets
            DrumBranchPreset × 16   ← en per pad, var och en med
              Device → OriginalSimpler (ett Simpler per pad, med egen Sample/FileRef)
          ReturnBranchPresets
            AudioEffectBranchPreset × 5   ← de fem returkanalerna
      MixerPreset
        AbletonDevicePreset → AudioBranchMixerDevice
      BranchSelectorRange (Min/Max/CrossfadeMin/CrossfadeMax)
      SessionViewBranchWidth = 55
```

Läses som: **ett rack är en graf av `GroupDevicePreset`**, där varje gren (`BranchPreset`) har egen
kedja, egen mixer och eget urval. Trumracket har 16 grenar (pads) som var och en bär ett helt
Simpler-instrument — det är därför ett Drum Rack i praktiken är 16 instrument i en enhet.
`Acuff Kit` har 172 `FileRef` poster, alltså 172 samplingar refererade med relativ sökväg
(`RelativePathType=5`, `LivePackName=Core Library`).

### 3.5 Antal per enhet (mätt, inte citerat)

| Katalog | `.adv` |
| :--- | ---: |
| `Devices/Instruments/Operator` | 331 |
| `Devices/Instruments/Wavetable` | 270 |
| `Devices/Instruments/Tension` | 107 |
| `Devices/Instruments/Drift` | 89 |
| `Devices/Instruments/Meld` | 84 |
| `Devices/Instruments/Sampler` | 76 |
| `Devices/Audio Effects/Reverb` | 79 |
| `Devices/Audio Effects/Hybrid Reverb` | 72 |
| `Devices/Audio Effects/Roar` | 64 |
| `Devices/Audio Effects/EQ Eight` | 62 |
| `Devices/Instruments/Analog` | 58 |
| `Devices/Audio Effects/Echo` | 48 |
| `Devices/Audio Effects/Delay` | 33 |
| `Devices/Instruments/Simpler` | 38 |
| `Devices/Instruments/Collision` | 33 |
| `Devices/Audio Effects/Vocoder` | 7 |
| `Devices/Audio Effects/Spectral Time` | 6 |
| `Devices/Audio Effects/Tuner` | 0 |

Summan i hela `Core Library` är **2 557 `.adv`** och **1 072 `.adg`** — samma tal som
`docs/ableton-files.md`. Notera att siffrorna ligger **en under** §2 i inventeringen (Wavetable 271,
Operator 332, Roar 65): inventeringen räknade även kopian i `Core Library/Defaults/`, som här är
utesluten. Båda är rätt, de mäter olika mängder.

---

## 4. Python-lagret

### 4.1 Vad som finns och vad det är

| Katalog | `.pyc` | Innehåll |
| :--- | ---: | :--- |
| `Resources/Python` | 888 | `abl.live` (licensiering, loggning, felrapportering), `abl.webconnector`, `lib` (CPython-stdlib), `site-packages` |
| `Resources/MIDI Remote Scripts` | 1 372 | 149 kontrollerytor + `_Framework` (59), `ableton` (224), `Push2` (104), `pushbase` (61), `Move` (46) |
| **Summa under `Resources`** | **2 260** | |
| `Program/Push/python/live_model` | 64 | **Live Object Model som källkod** |

Magiskt tal är `a7 0d 0d 0a` = **CPython 3.11**. Det betyder att `marshal.loads` räcker: ingen
dekompilering behövs för att läsa ut moduldocsträngar, klassnamn, attribut och enum-värden. Allt
nedan är läst så.

### 4.2 Live Object Model — 60 moduler, en per objekt

`Program/Push/python/live_model/Live/` innehåller 60 moduler. De viktigaste, med sina klasser:

```
Song        Song, View, GrooveInfo, MidiPortInfo, LiveMidiMapping, ControlSurfaceSlot,
            SessionRecordStatus, Quantization, RecordingQuantization, MetronomeRhythm,
            CaptureMode, CaptureDestination, InsertionResult, DeletionResult
Track       Track, View, DeviceInsertMode, MonitoringState, DuplicateDeviceResult
Clip        Clip, MidiClip, AudioClip, WarpMode, GridQuantization, PlayingStatus, LaunchMode
Device      Device, DeviceType
RackDevice  RackDevice, MacroMapping
DeviceParameter  DeviceParameter, AutomationState, ParameterState, BooleanDisplay, ValueHost
```

Modellerna är byggda av fyra byggstenar: `ValueProperty`, `ListProperty`, `ReferenceProperty`,
`VectorProperty` — alltså exakt den uppdelning som behövs för att generera bindningar. Det finns
också **en modul per enhet som har egna parametrar**:

```
AutoFilter2Device  AutoShiftDevice  CompressorDevice  DriftDevice  DrumCellDevice
DrumRackDevice     Eq8Device        ExternalAudioEffectDevice     ExternalInstrumentDevice
GateDevice         GlueCompressorDevice  HybridReverbDevice      MeldDevice
MultibandDynamicsDevice  PluginDevice  ResonatorDevice           RoarDevice
SamplerDevice      SimplerDevice   TransmuteDevice  TunerDevice  WavetableDevice  MaxDevice
```

21 enhetsmoduler. Det är den mest konkreta bekräftelsen på vilka enheter som har **eget API-ansikte**
— inte bara ljud.

### 4.3 Enumnamn — ordagrant ur `.pyc`

```python
# Clip.WarpMode
beats, tones, texture, repitch, complex, complex_pro, rex

# WavetableDevice.EffectMode
none, frequency_modulation, sync_and_pulse_width, warp_and_fold

# WavetableDevice.UnisonMode
none, classic, slow_shimmer, fast_shimmer, phase_sync, position_spread, random_note

# WavetableDevice.FilterRouting
serial, parallel, split

# Sample.SlicingStyle
transient, beat, region, manual

# Sample.TransientLoopMode
off, forward, alternate

# SimplerDevice.PlaybackMode
classic, one_shot, slicing
```

Det här är samma sak som `docs/ableton-command-surface.md` bara kunde se som menytext — men här är
det **enum-medlemmar**, dvs. de värden en klient faktiskt skickar.

`WavetableDevice`-attributen (lästa ur klassens `co_names`): `oscillator_1_wavetable_index`,
`oscillator_1_wavetable_category`, `oscillator_1_wavetables`, `oscillator_1_effect_mode`,
`oscillator_2_*`, `unison_mode`, `unison_voice_count`, `filter_routing`, `mono_poly`,
`poly_voices`, `modulation_sources`, `modulatable_parameters`,
`get_modulation_target_id_for_id`.

### 4.4 Kontrollerytorna

`_Framework/ControlSurface.pyc` ger två klasser, `ControlSurface` och `OptimizedControlSurface`, och
importerar `Live`, `ableton.v2.base`, `ButtonElement`, `MessageScheduler`,
`PhysicalDisplayElement`, `Profile`. `DeviceComponent.pyc` importerar `best_of_parameter_bank`,
`parameter_banks`, `device_to_appoint`, `select_and_appoint_device`. Det är ett **komplett
komponentramverk**, inte 149 fristående skript: varje yta ärver `ControlSurface` och får banker,
LED-ringar, display och sysex-schemaläggning gratis. Det förklarar varför Ableton kan skeppa 149
ytor och vi har en.

---

## 5. Vad Sonix saknar ur Ableton

Kolumnen "Sonix" är kontrollerad med `grep`/`ls` i `/home/alex/Projects/sonix/src` (19 filer på
toppnivå, 74 i `src/audio`, `src/ui` oräknat). "S/M/L" är min uppskattning av arbetet för att
stänga luckan, inte en mätning — det står i kolumnen för osäkerhet.

| Komponent | Bevis: Ableton har den | Sonix i dag | Storlek | Osäkerhet |
| :--- | :--- | :--- | :---: | :--- |
| **Wavetable-syntes** | `OInstrumentWavetableProcessor`; 270 presets; `EffectMode` med `sync_and_pulse_width`/`warp_and_fold`; `UnisonMode` med 7 lägen; `FilterRouting` serial/parallel/split; två filter med `Morph` | `Waveform`-enumen i `src/audio/command.rs` har Sine/Saw/Square/Triangle/Additive/Pluck/Guitar/Bowed/SixOp/Ogun — **ingen tabelluppslagning med position** | **L** | låg: motor + 7 unisonlägen + 4 effektlägen + tabellformat |
| **Multibandssaturator (Roar)** | `ORoarProcessor`; 3 stag × (12 shaper-lägen + 8 filtertyper med `Morph`), feedback-loop med bandbredd/gate, inbyggd LFO/envelope/noise | `Saturator` i `src/audio/master_fx/color.rs` har `drive`, `tone`, `mix` — tre rattar, ett band | **M** | låg |
| **Cytomic-filtertopologierna** | `SallenK_MS2/MD/SD`, `StateV_OSR`, `LadderT_PRD` med `Uf64x2`; etiketter `LP 12dB MS2`, `BP 12dB OSR` | `src/audio/filter.rs` (biquad + `process_lowpass`), `builtin_fx/biquad.rs`, `builtin_fx/filters.rs` (Butterworth 24/48/72 dB), `builtin_fx/mod.rs` `moog` (ladder) | **M** | medel: vi har ladder, men inte Sallen-Key/OSR-varianterna med drive i loopen |
| **Fysisk modellering (Tension/Collision)** | `ACollisionMallet`, `ACollisionResonator`, `ACollisionNoise`, `ACollisionScaledModeProcessor`; 107 + 33 presets | Karplus-Strong (`Waveform::Pluck`) och `Bowed` i `synth.rs` | **L** | låg: modal resonator + mallet-excitation är en egen motor |
| **Spektrala effekter** | `OSpectralTimeProcessor`, `OResonatorProcessor`, `OVocoderProcessor`; 6 + 8 + 7 presets | `src/audio/stft.rs` (236 rader), `spectral_edit.rs` (331), `builtin_fx` `spectralcomp` (STFT-kompression, 768 sampels latens), `SpectralFormant`/`SpectralSweep` i `synth.rs` | **M** | medel: byggstenarna finns, spektral *time* (frys/sträck) gör det inte |
| **Hybrid Reverb (2 motorer)** | `OHybridProcessor` + `AHybridImpulseResponseHandler`; etiketter `Diffusion`, `Density`, `Diff. Lo Freq`, `Chorus Rate`; 72 presets | `master_fx/color.rs` `Convolver` + `Reverb` (Schroeder, comb+allpass) | **M** | låg: diffusionnätet är det som saknas |
| **Warp via élastique/aufTAKT** | `Elastique Pro V3`, `CElastiqueProCore`, `aufTAKT 4.2.2`; 7 lägen i `WarpMode` | `src/audio/warp.rs` (5 egna lägen: Fast/Tone/Smooth/Complex/Transient) + `stretch.rs` 2 340 rader WSOLA | **L** | låg: kräver licens eller egen transientstyrd WSOLA — `warp.rs` förklarar redan varför vi inte lånar namnen |
| **Python-API (Live Object Model)** | 60 moduler i `live_model`, 2 260 `.pyc`, `ValueProperty`/`ListProperty`-modell | `src/audio/script.rs` (302 rader), `project_script.rs` | **L** | låg: detta är Abletons moat; en `Live`-modul med `Song`/`Track`/`Clip`/`Device` är månader |
| **Max for Live** | `OMxDeviceProcessor` m.fl.; 25 378 resursfiler; 864 `.mxe64` | `src/audio/patcher.rs` (781 rader, intern graf), JSFX-motor | **L** | låg: vi har en graf, inte ett ekosystem |
| **Kontrollerytor (149)** | 1 372 `.pyc`, `_Framework` med `ControlSurface`/`DeviceComponent`, banker och display | `hardware_control.rs` (498 rader), MCU + MIDI-learn | **M** | låg för 18 vanliga ytor, hög för resten |
| **MIDI-effekter (11)** | `OMidiScaleProcessor`, `OMidiChordProcessor`, `OMidiArpeggiator/NoteLength/Pitcher/Delay/Random/Velocity` | ackord/arpeggio i synten, `Scheduler`/`SchedulesChord`, tärning | **M** | låg: saknas som separat effekt med egen kedja |
| **Mikrotonalt (152 stämningar)** | `.ascl` i `Core Library/Tunings`; `ScaleAndTuningTableProcessor`, `TuningSystemToMpeProcessor` | `src/audio/tuning.rs` (242 rader) läser `.ascl`, en väg not→frekvens | **S** | låg — **befintligt**, luckan är bara fler stämningar i paketet |
| **Groove-pool** | 220 mallar i 88 grupper | `src/groove.rs` (745 rader), `parse_agr` läser XML-mallarna, mätt mot Swing 16ths 52 | **S** | låg — **befintligt**, kvar är de 103 binära mallarna |
| **Temafiler (18 `.ask`)** | 276 färgposter / 222 namn per tema | `src/ui/theme.rs`, en inbyggd palett | **M** | låg: `Theme::` används på 805 ställen (ur §10) |
| **Lektioner** | 1 016 filer, 6 kategorier | inget | **M** | låg |
| **Video-export** | `VideoExportMMF.dll`, Media Foundation-importer | `src/audio/video.rs` | **S** | okänd: har inte läst `video.rs` |
| **AI på GPU (DirectML)** | `onnxruntime_abl.dll` + `DirectML.dll` importerade | `neural_separator.rs`, `stem_separator.rs`, `ai_features.rs` (ONNX/Demucs) | **S** | låg — **vi ligger före** (§8) |

**Räkning:** 12 luckor att stänga (varav 3 redan är stängda: mikrotonalitet, groove, AI), 3 som är
M och billiga, 5 som är L. Den enda L-luckan som ger oproportionerligt mycket är **wavetable** —
den är en motor, inte ett ekosystem, och 270 presets visar hur mycket yta den fyller.

---

## 6. Gränser för den här mätningen

Det som **inte** gick att avgöra, och där jag inte gissar:

* **Varje enskild DSP-algoritm.** Jag kan citera att Cytomic-mallen `SallenK_MS2_MD_MIX` finns och
  att etiketten `LP 12dB MS2` finns. Jag kan inte säga vilken koefficient eller vilken
  överföringsfunktion Live använder — det kräver disassemblering av motsvarande funktion, och
  RTTI:n pekar bara ut klassen.
* **Mappningen enum-index → etikett.** `Stage1_Shaper_Type` är 0..11 och etikettlistan har tolv
  namn, men ordningen i `rdata` har jag inte läst. Inget i den här filen påstår att `3` = "Digital
  Clip".
* **Electric och Utility finns inte som RTTI-klass.** Jag vet inte om det beror på avsaknad av
  virtuella funktioner eller på att DSP:n ligger i `Engine.dll`. Jag har inte öppnat
  `Engine.dll` utöver att räkna dess RTTI (809 klasser).
* **Signeringscertifikatet** (10 696 B i certificate-directory) har jag inte verifierat. Att
  posten finns betyder inte att signaturen är giltig — kopian kommer från en patcher-release, så
  den kan vara bruten.
* **Presets utanför de tre enheterna.** Jag läste 3 av 3 629. Antalen per enhet är räknade för alla,
  men parameterblocken är citerade ur Roar, Wavetable och ett Drum Rack.
* **`.amxd` (Max-enheter)** har jag inte öppnat alls. De är ett eget format.
