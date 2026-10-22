# VST3-värden: så prövas den mot ett riktigt plugin

Sonix laddar VST3 via sin egen värd (`src/audio/plugin_vst3.rs`) — ingen SDK länkas in i
bygget. För att veta att värden fungerar mot något annat än våra egna antaganden byggs
Steinbergs eget SDK-exempel och körs genom den.

Provet är opt-in: ett VST3-binär hör inte till repot, så utan `SONIX_VST3_PROBE` hoppar
det över.

## Bygg SDK-exemplet

```bash
mkdir -p ~/.hermes/cache/scratch/vst3 && cd ~/.hermes/cache/scratch/vst3
git clone --depth 1 --recursive --shallow-submodules \
  https://github.com/steinbergmedia/vst3sdk.git
cmake -S vst3sdk -B build -DCMAKE_BUILD_TYPE=Release \
  -DSMTG_ENABLE_VSTGUI_SUPPORT=OFF -DSMTG_CREATE_PLUGIN_LINK=OFF -DSMTG_BUILD_EXAMPLES=ON
cmake --build build --target again-sample-accurate -j"$(nproc)"
```

`SMTG_ENABLE_VSTGUI_SUPPORT=OFF` är det som gör bygget huvudlöst: exemplet `again` har ett
GUI, men GUI:t länkas bara in när VSTGUI är på, och en yta behövs inte för att mäta ljud.

Målet heter `again-sample-accurate` (det finns inget mål som bara heter `again`).
Efter bygget kör SDK:ts egen validator automatiskt: 112 prov, 0 fel.

## Kör provet

```bash
SONIX_VST3_PROBE=~/.hermes/cache/scratch/vst3/build/VST3/Release/again-sample-accurate.vst3/Contents/x86_64-linux/again-sample-accurate.so \
  cargo test --features plugin-host --bin sonix -- --nocapture real_sdk
```

Väntat utfall: `AGain Sample Accurate: parameter 0 = 0.0 → RMS 0.0000, 1.0 → RMS 0.3513`.
Förstärkarens Gain sänker alltså till tyst och lämnar unity vid 1,0 — mätt på utgången.

## Vad provet hittade

`setProcessing` returnerade **3 (`kNotImplemented`)** och värden avvisade pluginat. VST3:s
basklass `AudioEffect` lämnar metoden oimplementerad, och en plugin som inte behöver veta om
den är i drift är fortfarande spelbar. Värden accepterar nu `kResultOk` **och**
`kNotImplemented` där. Utan det riktiga pluginat hade felet varit osynligt: våra egna prov
använde aldrig en komponent som svarade 3.

Felmeddelandena bär nu också **vilket anrop som gav vilken kod** (`setProcessing gav 3`), i
stället för ett allmänt "kunde inte starta" — det var det som gjorde felet synligt.
