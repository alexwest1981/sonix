# Stämmor — vilka modeller som finns och hur man provar dem

Separationen har två vägar: appens egen spektrala DSP-separator (fyra stämmor, ingen hämtning),
och en neural väg via ONNX. Den här filen handlar om den neurala: **katalogen, kontrakten och hur
man avgör vilken modell som är bäst på en viss låt.**

Katalogen bor i `src/audio/neural_separator.rs` (`MODELS`). Att lägga till en modell är att lägga
till en post där — ingen ny kod, så länge exporten passar någon av de två formerna nedan.

## Katalogen

| id | stämmor | storlek | licens | publicerad SDR |
| :--- | :--- | ---: | :--- | :--- |
| `htdemucs` | sång, trummor, bas, instrument | 166 MB | MIT | MUSDB18-HQ 9,0 dB medel |
| `htdemucs_6s` | + gitarr, piano | 136 MB | MIT | gitarr 3,1 / piano 1,6 dB (MoisesDB) |
| `bs_roformer_sw6` | 6: bas, trummor, other, sång, gitarr, piano | 353 MB | kod MIT, **vikter utan angiven licens** | gitarr 9,1 / piano 7,8 dB (MVSEP Multisong) |
| `mel_roformer` | sång + rest | 464 MB | MIT | sång 12,1 dB (MUSDB18-HQ) |
| `scnet` | sång, trummor, bas, instrument | 13 MB | MIT | MUSDB18-HQ 9,0 dB |

SDR-talen är hämtade ur modellkorten. De är mätta på **olika dataset** (MUSDB18-HQ, MoisesDB,
MVSEP Multisong) och är därför inte rad för rad jämförbara — jämför bara modeller inom samma rad i
tabellen ovan, eller mät själv (nedan).

Den enda modell som faktiskt separerar **gitarr och piano** är `bs_roformer_sw6`; `htdemucs_6s` har
gitarr- och pianohuvuden som i praktiken lämnar kvar materialet i `other`.

## Två format, tre layouter

**Vågform** (`ModelFormat::Waveform`) — Demucs-familjen. `mix [1,2,T]` in, `stems [1,S,2,T]` ut.
Appen klipper 7,8 s block med 1 s överlapp och normaliserar som `apply_model`.

**Spektrogram** (`ModelFormat::Spectrogram`) — RoFormer-familjen och SCNet är exporterade *mellan*
sin STFT och iSTFT. Appen gör fönstringen (`src/audio/stft.rs`) och måste använda exakt de
parametrar exporten är tracad med, annars blir resultatet brus utan felmeddelande:

| id | layout | n_fft | hop | fönster | T | skalning |
| :--- | :--- | ---: | ---: | :--- | ---: | ---: |
| `bs_roformer_sw6` | `RealImag` (`spec_real`/`spec_imag`) | 2048 | 512 | Hann | 345 | 1,0 |
| `mel_roformer` | `PackedSingle` (`mix_spec`) | 2048 | 441 | Hann | 801 | 1,0 |
| `scnet` | `PackedChannels` (`mix_spec`, ut `stems_spec`) | 4096 | 1024 | **inget** | 476 | 1/√4096 |

Layouterna:

- `RealImag` — två tensorer `[1,2,bins,T]`; svaret är `[1,S,2,bins,T]` i två tensorer.
- `PackedChannels` — en tensor `[1,4,bins,T]` (L-re, L-im, R-re, R-im); svaret `[1,S*4,bins,T]`.
- `PackedSingle` — samma packning in, men modellen ger **en** stämma. Appen lägger sången på
  sångplatsen och räknar ut resten som `mix − sång`, så en sångmodell lämnar aldrig tystnad efter
  sig.

## Prova vilken som är bäst

1. Stämmor-vyn → välj modell i listan (`✓` = hämtad). Knappen hämtar den till
   `~/.local/share/sonix/models/` (`.part` + atomiskt namnbyte, storleken kontrolleras mot
   `Content-Length`).
2. Separera samma låt med två modeller och lyssna på gitarr/pianostämmorna — det är där de skiljer
   sig.

**Mät i stället för att tycka.** `SONIX_NEURAL_SMOKE=<id>` kör modellen på en syntetisk mix och
kontrollerar det som måste stämma innan något annat spelar roll: att stämmorna summerar tillbaka
till källan (0,5–1,7 av källans RMS) och att modellen gav stämmor alls.

```bash
SONIX_NEURAL_SMOKE=scnet cargo test --release --features neural --bin sonix \
    a_real_model_separates_a_synthetic_mix -- --nocapture
```

Uppmätt på den här maskinen (CPU, 4 s syntetisk mix, inklusive modellens laddning):

| modell | stämmor | summering av källan | tid |
| :--- | ---: | ---: | ---: |
| `htdemucs` | 4 | 0,997 | 10,2 s |
| `htdemucs_6s` | 6 | 0,998 | 8,9 s |
| `scnet` | 4 | 0,863 | 3,2 s |
| `bs_roformer_sw6` | 6 | 0,929 | 27,7 s |
| `mel_roformer` | 2 | 1,000 | 16,8 s |

Summan 1,000 för `mel_roformer` är väntad: resten räknas som `mix − sång`, så kontrollen mäter
fönstringen (att iSTFT ger tillbaka det STFT:n skickade) och inte sångkvaliteten. För de andra är
summan modellens egen återskapning.

## Gränser

- **Ett fel i STFT-parametrarna syns inte som ett fel.** Modellen kör, ger ett spektrogram och
  appen gör ljud av det — men fel `n_fft`, fel fönster eller fel skalning ger grumlig eller tyst
  separation. Ändra bara parametrar mot en publicerad I/O-beskrivning, och kör smoke-provet efteråt.
- **RoFormer är dyrare än Demucs.** Ingen native-CPU-siffra är publicerad för `bs_roformer_sw6`;
  räkna med klart längre tid än `htdemucs` (mätningen ovan gäller SCNet).
- **`bs_roformer_sw6`: vikterna är re-hostade utan angiven licens.** Koden är MIT, vikterna är det
  inte dokumenterat att de är. Städa innan en release som utlovar modellen.
- **Modellerna skeppas inte** med installationen; varje modell hämtas för sig.
- **Fler exporter finns men passar inte våra layouter.** `xycld/BS-RoFormer-ONNX` (sång 12,9 dB,
  166 MB) tar rank-3-indata och kräver MSS:s egen förprocessor — den går inte att köra med
  `mix_spec`-layouten och är därför inte med i katalogen. Samma sak gäller MVSEP:s stängda
  modeller. En modell läggs till först när dess I/O-beskrivning stämmer med en av layouterna och
  smoke-provet är grönt.
- **Sångmodellerna ger två stämmor**, inte fyra. Resten är ett pålägg (`mix − sång`), inte en
  separat separation — gitarr och piano ligger kvar där.

## Mätt på en 4 s stereomix (i7-8700K, CPU, en kärna)

| Modell | Stämmor | Tid | Vad den är bäst på |
|---|---|---|---|
| `scnet` | 4 | 3,1 s | snabbast; 13 MB |
| `htdemucs` | 4 | 12,2 s | standard, MUSDB18-HQ 9,0 dB |
| `htdemucs_6s` | 6 | 12,2 s | gitarr/piano svaga (3,1 / 1,6 dB) |
| `mel_roformer` | 2 (sång + rest) | 20,8 s | bäst på sång, 12,1 dB |
| `bs_roformer_sw6` | 6 | 30,7 s | bäst på gitarr/piano (9,1 / 7,8 dB) |

**ONNX Runtime laddas vid körning** (0.9.213): `libonnxruntime.so` öppnas med `dlopen` i stället för
att länkas, så binären startar utan den och samma paket kan köra på en maskin med äldre glibc.
Filen letas upp i den här ordningen: `ORT_DYLIB_PATH`, bredvid binären, `~/.local/share/sonix/models/`.
Versionen måste vara **1.23.x** — 1.22 laddar men kraschar vid avslut ("double free or corruption"),
mätt på samma fil och samma modell. Saknas filen faller separationen tillbaka på DSP och säger var
den ska ligga.

`neural` är **på som standard** sedan 0.9.207: förut var det opt-in, och resultatet var en
modellväljare som tyst föll tillbaka på DSP i varje bygge som inte angav featuren — man valde en
modell och fick något annat. Ett bygge som måste vara beroendefritt stänger av den:
`cargo build --no-default-features --features plugin-host`, och då **säger** separationen det
("Byggd utan neural-stöd") i stället för att påstå att modellen saknas.

## Köra en modell utan fönster

```bash
sonix --separate låt.wav ut-mapp [modell-id]
```

Skriver en wav per stämma och rapporten visar modellfilens sökväg, tiden, `neural: true/false` och
rms per stämma. Frågan "fungerar modellen?" ska gå att svara på utan att titta på ett fönster.

## Varför släppta binärer inte kan separera

ONNX Runtimes färdiga bibliotek (som `ort` hämtar under bygget) är byggda mot nyare glibc: de
refererar `__libc_single_threaded` (2.32) och `__isoc23_strtol`/`__strtoull`/`__strtoll` (2.38).
Sonix släpps med glibc-golv 2.28 (manylinux_2_28), och länkningen faller där:

```
rust-lld: error: undefined symbol: __libc_single_threaded
rust-lld: error: undefined symbol: __isoc23_strtol
```

Därför bygger release-jobben **utan** `neural` (`--no-default-features`), och en binär som kan
separera byggs lokalt med standardkommandot:

```bash
cargo build --release
```

Binären säger vilket den är: `--separate` skriver `neural: true` eller `neural: false` med orsaken
("Byggd utan neural-stöd"). Vägen runt väggen, om släppen ska kunna separera, är `ort`s
`load-dynamic`: `dlopen` av `libonnxruntime.so` vid körning i stället för länkning vid bygge.
