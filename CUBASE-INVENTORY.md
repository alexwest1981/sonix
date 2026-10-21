# Cubase Pro 15.0.30 — inventering ur installationsfilen

Allt här är mätt på `~/Downloads/Steinberg Cubase Pro 15.0.30`, inte läst på webben.
Datum: 21 oktober 2026.

⚠️ **`Activator/Activation Manager Unlocker b14.exe` är en crack. Den har inte körts,
inte packats upp och inte analyserats.** Samma regel som för FL:s `FL Keygen.exe` och
Abletons `Patcher-Keygen`: bara `Setup/` undersöks.

---

## 1. Vad paketet är

| Egenskap | Värde |
|---|---|
| Ytterkatalog | 1,1 GB, tre nivåer (zip-innehåll uppackat) |
| Installationsformat | **MSI** (WiX 5 — `Wix5FirewallException` i tabellerna) |
| Huvudfil | `Cubase15.msi`, **730 607 616 B** (697 MiB) |
| Startprogram | `Setup.exe`, 2 441 232 B — bara en launcher; payloaden ligger i MSI:n |
| Payload | 11 CAB-filer, packade upp till **1,70 GiB** |
| Filposter | **2 171** (MSI:ns `File`-tabell), alla närvarande i CAB:en |
| Innehåll utanför MSI | 3 `.vstsound`, 210 MB (REVerence, VST Amp Rack, VST Bass Amp) |

MSI:n läses med `msitools` 0.106 (`msiinfo`/`msiextract`). Verktyget installerades inte
systembrett utan packades upp i scratch-katalogen och kördes därifrån — ingen root.

Sökvägarna i [`docs/cubase-files.md`](docs/cubase-files.md) är byggda ur MSI:ns tre
tabeller: `File` (2171 rader) → `Component` (405) → `Directory` (405). Alltså Cubase
egna installationsvägar, inte gissade.

---

## 2. Filförteckning

**2 171 poster, 1,70 GiB.** Hela listan står i `docs/cubase-files.md`, utan urval.

| Rot | Poster |
|---|---|
| `PFiles64/` (Program Files) | 1 986 |
| `Appdata/` | 148 |
| `Fonts/` | 34 |
| `CFiles64/`, `CommApp/` | 3 |

Vanligaste ändelser: `.xml` 1 390, `.png` 220, `.txt` 163, `.js` 80, `.wav` 49,
`.dll` 45, `.cpr` 38 (projektmallar), `.ttf` 25, `.qm` 21 (översättningar),
`.json` 16, `.otf` 16, `.ico` 9, `.mod` 9, `.xsl` 8, `.pdf` 8.

---

## 3. Kommandoytan

Två oberoende källor, båda ur filerna:

**a) 1 148 unika kommandon** ur Eucon-appsetten (`Appdata/Steinberg/Eucon/Appsets/*/*.xml`).
Cubase skriver sina menyval som `<Command>*/Meny/Val</Command>`, alltså med hierarkin kvar.
Fördelning per rot:

| Rot | Kommandon |
|---|---|
| `Key Commands` | 956 |
| `Control Room` | 50 |
| `Automation Controls` | 46 |
| `Left Edit` | 21 |
| `Transport` | 18 |
| `Right Edit` | 17 |
| `pidCommand` | 12 |
| `Project` | 8 |
| `ADR Controls` | 7 |
| `System` | 6 |

**b) 21 527 strängar** ur `Cubase15.exe` (129,2 MB), sorterade i fack:
3 085 etiketter, 177 meningar, 722 feltexter, 690 formatmallar, 16 853 tekniska.

Båda står i [`docs/cubase-command-surface.md`](docs/cubase-command-surface.md).

---

## 4. Komponenterna — 42 PE-filer i 36 poster

Cubase 15 är inte en monolit: huvudprogrammet är 129 MB och resten är moduler som laddas
vid behov. De tio största:

| Storlek | Komponent | Vad den gör (namnet, inte gissat) |
|---|---|---|
| 132,2 MB | `stemseparation.dll` | stämmseparering (§7) |
| 110,1 MB | `Drum Machine.bundle` | trummaskin |
| 102,8 MB | `ScoringEngine/ScoringEngine.dll` | notskrift |
| 101,2 MB | `Sampler Track.dll` | sampler-spår |
| 100,3 MB | `Modulation FX.bundle` | modulations-effekter |
| 20,1 MB | `admeditor.dll` | ADM (Dolby Atmos) |
| 17,0 MB | `vstconnect.dll` | VST Connect (fjärrinspelning) |
| 10,1 MB | `scu/scu.exe` | separat process |
| 9,7 MB | `audio2chords.dll` | ljud → ackord |
| 9,1 MB | `audioalignment.dll` | ljudinpassning (tajming) |

Övriga: `AAFFilter`, `araservice`, `AudioCodecs`, `AudioVideoConverter.bundle`,
`baios`, `DAWproject.dll` (import/export av DAWproject-format), `euconadapter`,
`goproremoteski`, `headtracking.dll`, `hostchannelaccess`, `hubservice`,
`mediaservice`, `OMFFilter`, `REX Shared Library` + `rex-support`, `Sampler Track.srf`,
`stepdesigner`, `usagelogger`, `VideoCodec-Apple-ProRes`, `videoengine`,
`vst2xscanner`, `VSTPlugManager`, `vstscanner`, `vstscannermaster`, `WebView2Loader`,
`webview`.

Noterbart i `Externals/`: **FFmpeg 8.0.1** och **SoXR** (samplingskonvertering) — de
skeppar alltså med två öppna projekt i stället för att skriva eget.

---

## 5. VST3-paketen (8)

`Cubase Plug-in Set.vst3` (106,9 MB binär + 145,8 MB `AudioPlugs.srf`),
`vintage-plugins.vst3`, `CurveEQ.vst3`, `ModScripter.vst3`, `LoopMash FX.vst3`,
`vstambiconverter.vst3`, `VST Connect.vst3`, `VST Renderer.vst3`.

---

## 6. Innehåll

| Slag | Antal | Storlek |
|---|---|---|
| `.vstsound` (i MSI) | 5 | 222 MB |
| `.vstsound` (utanför MSI) | 3 | 210 MB |
| Förval (XML) | **610** | — |
| Projektmallar (`.cpr`) | 38 | — |
| Hjälp-/metadata-XML | 1 390 | — |

Förvalen per kategori — det är Cubases MIDI-verktyg som syns tydligast:

| Kategori | Förval |
|---|---|
| Project Logical Editor | 130 |
| Logical Edit | 117 |
| Arpache SX | 88 |
| Chorder | 62 |
| Input Transformer | 28 |
| MIDIEcho | 28 |
| Auto LFO | 25 |
| Transformer | 21 |
| Pattern Arp | 20 |
| StepDesigner | 16 |

**247 av 610 förval är Logical Editor** — Cubases programmerbara MIDI-transformator.
Det är deras svar på "jag vill ändra alla noter enligt en regel", och det finns inte hos oss.

---

## 7. Stämmsepareringen — deras och vår

`Components/stemseparation.dll` är **132,2 MB** och innehåller:
`onnx` 165 gånger, `ONNX` 9, `UVR` 2. Alltså en **inlärd modell** (ONNX) som de kör
lokalt, samma familj som Demucs/UVR — inte ett filter.

Vår separator (`src/audio/stem_separator.rs`) är en **filterbank**: fyra enpolsfilter
och en transientgrind, ~200 rader, ingen modell. Den är snabb och deterministisk, men
den kan inte separera två källor som ligger i samma frekvensband. Cubases kan.
`neural_separator.rs` finns som väg för en ONNX-modell, men ingen modell skeppas med oss.

**Buggen som rapporterades** ("enbart ohörbart brus" vid import och separering) var två fel:
avkodningen av 24- och 32-bitars WAV (WAVE_FORMAT_EXTENSIBLE föll i 8-bitsfallet) och att
stämmorna normaliserades var för sig till 0,92 (fyra stycken summerar till 3,7 → klippning).
Båda är fixade och mätta; se commit `7329040`.

---

## 8. Gränssnittet — mätt mot vårt

| Egenskap | Cubase 15 | Sonix |
|---|---|---|
| Kommandon | 1 148 (Eucon-namn, hierarki) | ~200 genvägar |
| Kategorier i kommandoträdet | 10 | — |
| Strängar i binären | 21 527 | — |
| Fönsterytor | MixConsole + projektfönster + separata editrar | 12 vyer + 5 paneler (Ctrl+1…5) |
| Kontrollerytor | Eucon-appset för Artist/Hybrid/S5MC m.fl. | 0 |
| Programmerbar MIDI-transform | 247 förval i Logical Editor | — |
| Notskrift | ScoringEngine (103 MB) | — |
| Stämmseparering | ONNX-modell | filterbank |
| Fjärrinspelning | VST Connect | — |
| Video | videoengine + ProRes-codec | — |

---

## 9. Gap-matris — vad vi saknar, rangordnat

Underlaget för prioriteringen (vad de 610 förvalen faktiskt gör) står i
[`CUBASE-NEEDS.md`](CUBASE-NEEDS.md). Kort: förvalen är mest **massoperationer på noter och
event** — därav att transponering och anslagsändring byggdes först.

| # | Vad | Storlek | Varför |
|---|---|---|---|
| 1 | **Stämmseparering med modell** (ONNX) | L | vår filterbank kan inte separera källor i samma band; vägen finns i `neural_separator.rs` men ingen modell |
| 2 | **Logical Editor** — regler på noter/event | L | 247 förval hos dem; vi har ingen regel motor |
| 3 | **Chord Track** | M | vi har ackordgenerator, inte ett spår som andra spår följer |
| 4 | **Arranger Track** (låtsektioner) | M | vi har markörer, inte sektioner som kan flyttas om |
| 5 | **VariAudio** — pitch/tajming per ton i tagningen | L | kräver tonsegmentering; vi har global pitch och stretch |
| 6 | **Notskrift** | XL | 103 MB motor hos dem |
| 7 | **Control Room** (50 kommandon: monitor, cue, talkback) | M | vi har master ut, ingen monitorsektion |
| 8 | **MediaBay** — sökbar innehållsdatabas | M | vi har filbläddrare |
| 9 | **Sampler Control** (sampler-spår med zoning) | L | vi har sampler-röst, inte zon-redigering |
| 10 | **Render in Place / Direct Offline Processing** | M | vi har bounce, inte kedjor per klipp |
| 11 | **VST Connect** | XL | fjärrinspelning över nät |
| 12 | **Eucon/kontrollerytor** | M | 0 hos oss |
| 13 | **Video** | M | vi har videoengine? nej — video saknas |
| 14 | **Projektmallar** | S | 38 hos dem, 0 hos oss |
| 15 | **DAWproject-import/export** | S | öppet format, `DAWproject.dll` hos dem |

---

## 10. Vad som inte är mätt

- **`Activator/Activation Manager Unlocker b14.exe`** — crack, ej körd, ej analyserad.
- **Hur stämmsepareringen låter.** Modellen ligger i en DLL vi inte kan köra; vi vet att
  den är ONNX, inte hur bra den är. Ingen jämförelse i dB är gjord.
- **Gränssnittet i drift.** Ingen skärm i sessionen: alla UI-jämförelser är ur filerna
  (kommandon, etiketter, XML), inte från en körande Cubase.
- **`.vstsound`-innehållet.** Filerna är räknade och storleksmätta, inte uppackade — deras
  interna format är inte dokumenterat här.
- **De 16 853 tekniska strängarna** i `docs/cubase-command-surface.md` är utelämnade som
  fack; de innehåller kodfragment och C++-mallar, inte UI-text.
