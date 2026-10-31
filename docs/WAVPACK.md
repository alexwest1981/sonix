# WavPack i processen

`.wv` avkodades tidigare bara genom det externa `ffmpeg`-programmet: en process per fil, 44,1 kHz
påtvingat (ffmpeg-anropet i `wav_reader` omsamplar) och inget ljud alls på en maskin utan ffmpeg.
Nu avkodas WavPack **i processen** via `libwavpack`, som `dlopen`:as med `libloading` precis som
Opus (`src/audio/opus.rs`). Biblioteket är valfritt — binären startar utan det, `is_available()`
svarar, och `load_audio_pcm` faller tillbaka på ffmpeg om det saknas.

Avkodningen är förlustfri hela vägen: WavPack ger tillbaka originalets heltalsexempel, vi delar
bara med `2^(bits-1)` (samma skala som `load_wav_pcm` och ffmpeg använder). Flyttal (`MODE_FLOAT`)
passerar orörda. Resultatet är sampel-identiskt med ffmpegs egen f32-avkodning.

## Installation

Maskinen hade inget `libwavpack` (`/usr/lib` är tom på det), så biblioteket byggs från källa och
installeras i hemkatalogen — ingen `sudo`, inget i `/usr`.

```sh
curl -sSLO https://www.wavpack.com/wavpack-5.7.0.tar.bz2
tar xf wavpack-5.7.0.tar.bz2 && cd wavpack-5.7.0
./configure --prefix=$HOME/.local --enable-shared --disable-static
make -j8 && make install
```

Det ger `~/.local/lib/libwavpack.so.1` (och `.so`-symlänken) samt `~/.local/bin/wavpack` som kan
skapa egna testfiler. `--disable-shared` fungerar inte här: vi behöver `.so`, inte `.a`.

Modulen letar i den här ordningen, så både ett systempaket och hembygget hittas:

1. `libwavpack.so.1`
2. `libwavpack.so`
3. `$HOME/.local/lib/libwavpack.so.1`
4. `$HOME/.local/lib/libwavpack.so`

Absolutvägarna behövs eftersom `dlopen` inte söker i `~/.local/lib` utan `LD_LIBRARY_PATH`. Ett
systempaket (`pacman -S wavpack`, `apt install libwavpack1`) räcker också — då hittas sonamen direkt.

## Vad som faller tillbaka

- **Biblioteket saknas** → `wavpack::decode_file` returnerar `Err` som säger att WavPack inte är
  installerat och att ffmpeg används i stället; `load_audio_pcm` går vidare till ffmpeg.
- **Filen är trasig eller trunkerad** → `Err` som namnger felet (antal avkodade ramar av
  förväntade, eller libwavpacks egen feltext) och ffmpeg får sista ordet.
- **Endast `.wv`** tas här. WavPack i Matroska/andra containrar går fortfarande via symphonia/ffmpeg.
- **Flyttal med mer än 32 bitar** (dubbel precision) stöds inte och säger det.

## Mätning

Testerna i `src/audio/wavpack.rs` bygger fixtures med ffmpeg och jämför mot ffmpegs egen avkodning:
2 s 440 Hz-sinus vid 44,1 kHz ger 88 200 ramar, dominant 440 Hz, RMS-differens < 0,01 dB och
maximal sampeldifferens 0 (förlustfritt är förlustfritt). Samma sak för 24-bitars WavPack, och en
trunkerad respektive icke-WavPack-fil ger `Err` med orsak.
