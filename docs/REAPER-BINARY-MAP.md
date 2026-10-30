# REAPER 7.83 (linux-x86_64) — karta över binären, och vad Sonix saknar

Allt nedan är mätt på artefakten i `/mnt/fastnvme/hermes/cache/scratch/reaper783/REAPER`
(135 MB utpackat), inte läst på reaper.fm. Varje påstående har en mätning bakom sig:
symbolnamn, strängträffar, sektionsstorlekar eller antal. Där något bara är en sträng
säger texten uttryckligen **sträng** — en sträng är inte ett bevis på att koden körs på
Linux. Där versioner eller kurvor inte går att läsa ur en strippad binär står det att det
inte går att avgöra.

Mätkommandon: `file`, `readelf -SW/-d/-sW --dyn-syms`, `size`, `nm -D`, `strings -n N | grep -c`,
`ldd`. `reaper` har sha256 början `aae1412e452b83b540d546431ae77a28`.

---

## 1. Binäröversikt

| Fil | Byte | Format | Strippad | Dynamiska beroenden (NEEDED) |
|---|---:|---|---|---|
| `reaper` | 12 881 232 | ELF64 LSB, ET_EXEC, x86-64, interp `/lib64/ld-linux-x86-64.so.2`, GNU/Linux 2.6.32, BuildID `77423b12…` | **ja** — ingen `.symtab` (`readelf -SW \| grep -c symtab` = 0) | 7: `libpthread.so.0`, `libdl.so.2`, `libasound.so.2`, `libstdc++.so.6`, `libm.so.6`, `libgcc_s.so.1`, `libc.so.6` |
| `libSwell.so` | 790 248 | ELF64 DYN, BuildID `464dfa1c…` | ja | X11-delen: `libX11`, `libXi`, `libfontconfig`, `libGL`, `libfreetype`, `libxcb`, `libXext`, `libpthread`, `libdl`, `libstdc++`, `libc` |
| `Plugins/elastique3.so` | 1 691 880 | ELF64 DYN, BuildID `86b70c9b…` | ja | `libdl`, `libm`, `libgcc_s`, `libc` |
| `Plugins/reaper_midi.so` | 2 100 120 | ELF64 DYN | ja | `libdl`, `libstdc++`, `libm`, `libgcc_s`, `libc` |
| `Plugins/reaper_video.so` | 2 040 176 | ELF64 DYN | ja | ovan + `libGL.so.1`, `libGLdispatch`, `libGLX`, `libX11`, `libxcb` |
| `Plugins/reaper_ogg.so` | 1 113 272 | ELF64 DYN | ja | bas (dl/stdc++/m/gcc_s/c) |
| `Plugins/jsfx.so` | 662 704 | ELF64 DYN | ja | bas |
| `Plugins/reaper_opus.so` | 679 512 | ELF64 DYN | ja | bas |
| `Plugins/reaper_explorer.so` | 643 408 | ELF64 DYN | ja | bas |
| `Plugins/reaper_csurf.so` | 490 832 | ELF64 DYN | ja | bas |
| `Plugins/reaper_flac.so` | 392 232 | ELF64 DYN | ja | bas |
| `Plugins/reaper_wavpack.so` | 355 528 | ELF64 DYN | ja | bas |
| `Plugins/reaper_mp3dec.so` | 286 424 | ELF64 DYN | ja | bas |
| `Plugins/reaper_wave.so` | 269 656 | ELF64 DYN | ja | bas |
| `Plugins/reaper_host_x86_64` | 207 928 | ELF64 **EXEC** (eget program, inte .so) | ja | `libpthread`, `libdl`, `libasound`, `libstdc++`, `libm`, `libgcc_s`, `libc` |
| `Plugins/rubberband.so` | 184 784 | ELF64 DYN | ja | bas |
| `Plugins/reaper_ddp.so` | 136 840 | ELF64 DYN | ja | bas |
| `Plugins/reaper_cd.so` | 96 696 | ELF64 DYN | ja | bas |
| `Plugins/soundtouch.so` | 51 560 | ELF64 DYN | ja | bas |

Storleksfördelning i huvudbinären (`size`): `.text` 12 671 652 B, `.data` 203 552 B,
`.bss` 730 624 B. Alltså ~98 % kod — motorn är monolitisk och allt "inbyggt" ligger i
`reaper`, inte i `Plugins/`.

Exportytan är medvetet minimal: `.dynsym` 344 poster, 99 definierade symboler
(`nm -D --defined-only`), varav **62 är `nseel_asm_*`** och 7 är `eel_*`. Resten är
`SWELLAppMain`, `SWELLAPI_GetFunc`, `_start`, `_init` och libc/libstdc++-svansar. Inga
`reaper_*`-API-symboler exporteras — ReaScript-API:t skeppas som en **tabell**, inte som
länkade symboler (se §4).

Varje `Plugins/*.so` (utom `elastique3.so`, `rubberband.so`, `soundtouch.so`, som är rena
DSP-bibliotek) exporterar exakt två symboler: `ReaperPluginEntry` + `SWELL_dllMain`
(mätt: 2 träffar var). De tre DSP-biblioteken exporterar 0 av dessa — de laddas av
värden, de är inte värden.

---

## 2. Inbyggd DSP

### 2.1 Tidssträckning och pitch — tre tredjepartsmotorer, alla separat laddade

**élastique (zplane)** — `Plugins/elastique3.so`, 1 691 880 B, deps bara dl/m/gcc_s/c.
Bevis att det är zplanes kod och inte "något som heter elastique":

- exporterade inträden: `CreateInstance_E3`, `CreateInstance_E3D` (`nm -D --defined-only`)
- RTTI/klassnamn i strängtabellen: `CElastiqueV3`, `CElastiqueProV3`, `CElastiqueProCore`,
  `CElastiqueEffV3Core`, `CElastiqueEffV3mobileCore`, `CElastiqueSOLOV3`,
  `CElastiqueV3Direct`, `ElastiqueV3If`, `ElastiqueProV3If`
- PSOLA-syntes: `PSOLASynthForElastiqueProV3`, `CPSOLASynthesis`
- egen intern omsampling/pitchspårning: `CHQResample::ProcessData`, `CPitchTrack::Process`,
  `CFFTSplitV3::ProcessData`, `zplVecLibGetVersion` (zplanes vektorbibliotek)
- i huvudbinären: `ElastiqueShifter`, `ElastiqueShifter_V3`, `ElastiqueShifter_v1_depr`,
  `\/elastique.so`, `lastique Pro by zplane.development` (strängen är `élastique Pro by
  zplane.development` — `strings` skriver inte ut UTF-8-ledbyten), `…lastique %d.%d.%d Soloist`

Alltså: **Pro + Efficient + Soloist**, med PSOLA för monofont material. Exakt
versionsnummer går inte att läsa (strippad, versionssträngen byggs av `%d.%d.%d` vid
körning). Tredjepart, licensierad — inte öppen källkod.

**Rubber Band** — `Plugins/rubberband.so`, 184 784 B.
`RubberBandStretcher::Impl::analyseChunk/available/calculateIncrements/calculateStretch/
consumeChannel/formantShiftChunk/getIncrements`, namnrymden `RubberBand::Resamplers::D_Speex`
(alltså **Speex-resamplern** inuti), `RubberBand::FFT::D_Cross`, `StretchCalculator`,
`CompoundAudioCurve`, `PercussiveAudioCurve`, `SpectralDifferenceAudioCurve`.
I huvudbinären: `Rubber Band Library`, `Rubber Band Library audio timestretcher by
Breakfast Quay`. Formant-skiftning finns som funktion (`formantShiftChunk`).
Version: **går inte att avgöra** — ingen versionssträng i den strippade .so:n.

**SoundTouch** — `Plugins/soundtouch.so`, 51 560 B.
`soundtouch::SoundTouch`, `TDStretch`, `TDStretchSSE`, `RateTransposer`, `FIRFilter`,
`FIRFilterSSE`, `InterpolateCubic/Shannon/LinearInteger/LinearFloat`, `FIFOSampleBuffer`,
samt strängen `SoundTouch Version: %s` (printf — siffran syns inte) och
`http://www.surina.net/soundtouch/`. Alltså Olli Parviainens bibliotek, med SSE-vägar.
Lägst kvalitet av de tre, men billigast.

**Vilken motor används när?** Huvudbinären innehåller alla tre shifter-klasserna
(`ElastiqueShifter`, `ElastiqueShifter_V3`, `ElastiqueShifter_v1_depr`) plus
`Loading SoundTouch...`, `soundtouch`, `SoundTouch`, `Rubber Band`, `Monophonic`.
Valet är ett projekttillstånd (`Pitch shifter parameter:` finns som sträng), inte hårdkodat.
Vilken som är default **går inte att avgöra** ur binären.

### 2.2 Omsampling — r8brain free, statiskt inlänkad i `reaper`

- manglat C++-symbolnamn i strängtabellen: `N3r8b13CDSPResamplerE` (Itanium-ABI för
  `r8b::CDSPResampler`)
- upphovssträng: `Includes r8brain free designed by Aleksey Vaneev of Voxengo`
- användarvändliga val: `r8brain free (highest quality, fast)`, `Dots and sinc`,
  `playresamplemode`, `projrenderresample`
- internt gränssnitt: `REAPER_Resample_Interface` (sträng), RTTI `12SysResampler`
- träffar för `r8brain|resampl` per modul: `reaper` 30+ strängar, `elastique3.so` 90
  (dess egen `CHQResample`), `rubberband.so` 13 (egen `Resampler`), `jsfx.so`,
  `reaper_{flac,mp3dec,ogg,opus,wavpack,wave,video}.so` 1 var (kodekarnas egen
  omsampling, inte projektets)

Alltså: två kvalitetslägen — r8brain (polyfas, hög kvalitet) och en sinc-baserad
("Dots and sinc"). Båda i `reaper`, ingen .so. Tredjepartskod (Voxengo, fri licens).

### 2.3 Mediekodekar — in-process, en .so per familj

Klassnamn i strängtabellen ger en exakt lista. **Källor (`PCM_source_*`, 22 st):**
`PCM_source_wavefile`, `_wav_metadata`, `_flac`, `_flac_metadata`, `_mp3`, `_mp3_metadata`,
`_ogg`, `_ogg_metadata`, `_opus`, `_wavpack`, `_video_base`, `_ffmpeg_metadata`,
`_midifile`, `_midi_metadata`, `_syxfile`, `_media`, `_fx`, `_sd2`, `_simplewrapper`,
`_rpp_metadata`, `_peaktransfer_t`, `_transfer_t`.

**Sänkor (`PCM_sink_*`, 10 st):** `_wavefile`, `_flac`, `_mp3lame`, `_vorbis`, `_opus`,
`_wavpack`, `_midifile`, `_raw`, `_iso`, `_dummy`.

| Format | Modul (byte) | Bevis |
|---|---|---|
| WAV / RF64 / AIFF / BWF | `reaper_wave.so` 269 656 | `19PCM_source_wavefile`, `23PCM_source_wav_metadata`, `AIF;AIFF`, `AIFF is not 8/16/20/24/32/64 bit`, `API_wave__createMetadataSource` |
| FLAC | `reaper_flac.so` 392 232 | `FLAC__STREAM_DECODER_*` (7 olika felkoder), `API_flac__createMetadataSource`, `PCM_sink_flac` |
| MP3 | `reaper_mp3dec.so` 286 424 | `14PCM_source_mp3`, `16PCM_sink_mp3lame`, `get_lame_version`, `LAME ?.??`, `lame_close` → **LAME** för encode |
| Ogg Vorbis | `reaper_ogg.so` 1 113 272 | `VorbisDecoder`, `VorbisEncoder`, `PCM_sink_vorbis`, `API_CreateVorbisDecoder/Encoder` |
| Opus | `reaper_opus.so` 679 512 | `14OggOpusEncoder`, `15PCM_source_opus`, `CVBR %.0fkbps%s OGG Opus`, `Hard-CBR …`, `Error parsing OPUS: file contains more than 8 channels` |
| WavPack | `reaper_wavpack.so` 355 528 | `18PCM_source_wavpack`, `16PCM_sink_wavpack`, `not compatible with this version of WavPack file!` |
| CD-audio (CDDA) | `reaper_cd.so` 96 696 | `Burn CD`, `Burn CD image after render`, `Audio CD Image (CUE/BIN format)`, `Eject CD when done` |
| DDP (mastering-lev.) | `reaper_ddp.so` 136 840 | `DDP 2.00`, `10PCMSinkDDP`, `12PCMSourceDDP`, `Corrupted DDPMS file`, `Corrupted PQDESCR file`, CD-TEXT/ISRC-hantering |
| Video | `reaper_video.so` 2 040 176 | se nedan |
| Media Explorer | `reaper_explorer.so` 643 408 | `Media Explorer`, `Apply preview pitch/rate to inserted media item`, `Auto-stop preview when dragging media`, preview-databas + metadata-cache |

**Video är den tyngsta enskilda komponenten efter huvudbinären.** Bevis:
`FFMPEGDECODER`, `ffmpegStreamDecoderBase`, `FFMPEGDecodeStream_54` … `_63` (tio
libavformat-huvudversioner, 54–63), `FFMPEGVideoEncoder_54` … `_63` (alltså **både**
avkodning och inkodning), `avcodec_alloc_context3`, `avcodec_receive_packet`,
`PCM_source_ffmpeg_metadata`, kodeksträngen `h264`, samt GL-beroenden i `ldd` (`libGL`,
`libGLX`, `libGLdispatch`).

Att ffmpeg laddas **vid körning**, inte länkas: `nm -D -u Plugins/reaper_video.so` ger
109 odefinierade symboler och **noll** `av*`/`sws*`. De enda spåren är strängar. Exakt
dlopen-namn syns inte (endast `/codec/libavcodec_plugin.so` som sökvägssträng).

Ovanpå avkodningen ligger en **videoprocessor med eget språk**: strängar `//@param`,
`//@gmem=`, `//@gmem=jsfx_to_video`, `input_info(input, w, h[,srctime, wet, parm1, ...])`,
samt GLSL-fragment (`gl_FragColor=vec4(rgb.x,rgb.y,rgb.z,1.0);`, `gl_TexCoord[0].x`) och
`// Color Peeker (Monitoring FX only -- interactive in video window)`. Alltså: video-FX
skrivs som shaders i en JSFX-liknande ram, kompileras och körs på GPU.

### 2.4 Dither

13 träffar på `dither` i `reaper`. Vad som faktiskt går att belägga:

- flaggbitmask, inte en algoritm: `RENDER_DITHER : &1=dither, &2=noise shaping,
  &4=dither stems, &8=noise shaping on stems, &16=disable all`
- `DITHER 3 \t# 1=dither, 2=noise shaping, 3=both` (projektfil-/RPP-nyckel)
- lägena: `Dither`, `Dither...`, `Noise shaping`, `Dither enabled`, `Dither master`,
  `Dither stems`
- interna nycklar: `projrenderdither`, `convertdither`, `2=old_unsupported/dither`

Alltså: dither + noise shaping, separat för master och stems, och även i "convert"
(källkonvertering). **Vilka shaping-kurvor som erbjuds går inte att avgöra** — inga
strängar som `Lipshitz`, `F-weighted`, `E-weighted` eller `Modified E-weighted` finns i
binären (0 träffar). Att kurvorna är flera är sannolikt men inte belagt.

### 2.5 Synt och instrument — inget nativt

Noll träffar på `ReaSynth`, `ReaSamplomatic`, `ReaPitch`, `ReaEQ`, `ReaComp`, `ReaGate`,
`ReaXcomp`, `ReaFir`, `ReaVerbate` som klassnamn (exaktmatchning = 0 för samtliga;
`ReaTune` ger 1 träff, `ReaEQ` 9 träffar — alla i hjälptexter/stränglistor, inte som
klasser). I stället finns en **migreringstabell** som mappar gamla VST2-namn till JSFX:

```
reastream.  reasurround.  reasurround2.  reasyndr.  reasynth.  reatune.  reaverb.
reaverbate.  reavocode.  reavoice.  reaxcomp.  realimit.  Automap
3=reasyndr.vst.so
```

följd av `Scanning VST plugins...` och `~/.vst`. Slutsatsen är mätbar, inte gissad:
**ReaPlugs är JSFX, inte nativa** — de 256 filerna i `InstallData/Effects` är hela
instrument- och effektbiblioteket, och binären innehåller bara *namnen* för att kunna
migrera gamla projekt. Undantaget är elastiques PSOLA/`CPitchTrack`, som är DSP men inte
ett instrument.

---

## 3. Pluginhosting — vad som faktiskt är med

Antal strängträffar i `reaper` (`strings -n 5 | grep -cE …`), plus det specifika beviset:

| Format | Träffar | Bevis | Körs på Linux? |
|---|---:|---|---|
| **LV2** | **151** | `LV2Ctx`, `Add system LV2 paths`, `Directories to scan for LV2 plug-ins…`, `%d plug-ins scanned [%d lv2 bundles] [%d UIs] [%d presets] [%d failed]`, URI:er `lv2plug.in/ns/ext/atom#…`, `lv2plug.in/ns/lv2core#sampleRate`, `kxstudio.sf.net/ns/lv2ext/external-ui#Host`, `ardour.org/lv2/midnam#interface`, `harrisonconsoles.com/lv2/inlinedisplay#interface` | **ja** |
| **CLAP** | 71 | `clap_entry`, `clap_get_reaper_context`, `~/.clap`, `CLAP_PATH`, `%CLAP_PATH%`, `Add system CLAP paths`, `Export CLAP preset (.clappreset/.*)`, `clap.audio-ports`, `clap.audio-ports-config`, `clap.latency`, `clap.gui`, `clap.context-menu/1`, `clap.gain-adjustment-metering/0`, `reaper-clap-linux-x86_64.ini` | **ja** |
| **VST3** | 39 | `VST3_Host`, `VST3_Message`, `VST3Frame`, `VST3_EventList`, `VST3_MemStream`, `VST3_ParamChanges`, `VST3_AttributeList`, `VST3_ContextMenu`, `dummyVST3View`, `.vstpreset` (läs/skriv/fel), `/usr/local/share/vst3/presets/%s/%s/`, `Adjust Track Volume (via VST3)` m.fl. | **ja** |
| **VST2** | 4 (`VST2`), +1 `VSTPluginMain` | `VSTPluginMain` (entry point), `Scanning VST plugins...`, `~/.vst`, `// VST plugins: see http://www.cockos.com/reaper/sdk/vst/vst_ext.php`, namnprefix `VST:`, `VST2:`, `VST2i:`, `WARNING - The following VST2 plug-ins were not available, but matching VST3 versions were found…` | **ja** (få strängar — implementationen är intern, vilket är väntat) |
| **JS/JSFX** | 24 | `jsfx.so` (662 704 B), `JS:`-prefix i FX-namn, `get_eel_funcdesc`, `eel_callcode64`, `EEL_Editor`, `EEL_LICE_gfx_standalone`, **53 unika `gfx_*`-funktioner** dokumenterade i `jsfx.so` | **ja** |
| **DX (DirectX)** | 2 | endast `Scanning DirectX FX...` och `DX:` inuti en doc-sträng | **nej** — död kod/sträng på Linux, ingen DirectX-API-yta |
| **AU (AudioUnit)** | 1 | endast `AU:` inuti samma doc-sträng (`…VST3:,VST2:,VST:,AU:,JS:, or DX:`) | **nej** |

**LV2-värden är egen kod, inte lilv**: `grep -c 'lilv|serd|sord|suil'` = 0 träffar, men
151 `lv2`-träffar inklusive egen klass `LV2Ctx` och egen bundle-skannerare. (Slutledning
från frånvaron av lilv/serd/sord — inte bevisat att ingen annan LV2-lib används.)

**Bit-bridge / separat process finns**: `Plugins/reaper_host_x86_64` är ett **eget
program** (ELF EXEC, 207 928 B) som laddar plugins utanför huvudprocessen. Bevis i
huvudbinären: `#BRIDGE#`, `#BRIDGE#%s`, `bridged:`, `Bridged plug-in may be open in top
level window.`, `SWELL_CreateXBridgeWindow`, `Separate process`, `Plugins/reaper_host_x86_64`,
`/reaper-vstbridge32.ini`, samt brygg-gränssnitten `Bridge_IComponent`,
`Bridge_IEditController`, `Bridge_IPluginFactory`, `Bridge_IPlugView`. Versionen står i
klartext: `7.83/linux-x86_64`.

---

## 4. ReaScript och skriptspråk

**API-ytan: 732 funktioner.** Mätt som `grep -c '^def RPR_' Plugins/reaper_python.py`
(filen är 4 966 rader och är den genererade bindningen; 733 `^def`, varav en är
`RPR_ValidatePtr`-hjälparens syskon — 732 är de faktiska `RPR_`-funktionerna).
Samma tabell nås från EEL2 och Lua; antalet är alltså API:ts storlek, inte Pythons.

| Språk | Status | Bevis |
|---|---|---|
| **Lua** | **inbäddad, 5.4.6** | `$LuaVersion: Lua 5.4.6  Copyright (C) 1994-2023 Lua.org, PUC-Rio $$LuaAuthors: R. Ierusalimschy, L. H. de Figueiredo, W. Celes $` (Lua:s egen identitetssträng — finns bara om källan är medkompilerad). Plus runtime-felsträngar: `stack traceback`, `C stack overflow`, `attempt to compare`, `invalid escape`, `LUA_PATH`, `__index`. Inga exporterade `lua_*`-symboler (`nm -D \| grep -c ' lua'` = **0**) → statiskt länkad och strippad. "ReaScript/EEL2 och ReaScript/Lua stöds alltid" står i preferenstexten. |
| **EEL2** | inbäddad, egen kompilator | `nseel_asm_xor_op`, `nseel_asm_sqr`, `nseel_asm_notequal`, `nseel_asm_above`, `nseel_asm_assign` … **62 exporterade `nseel_asm_*`** + `eel_callcode64`, `eel_callcode64_fast`, `eel_enterfp`, `eel_leavefp`, `eel_gmem_attach`, `eel_gmem_getptr`, `eel_fft_register`, `_asm_generic1parm…`, `_asm_megabuf`, `_asm_gmegabuf`. Asm-blocken betyder kodgenerering till native x86, inte en tolk. Samma kompilator ligger i `jsfx.so` (separat kopia: `eel_callcode64`, `get_eel_funcdesc`, `EEL_Editor`). |
| **Python** | **värd, inte inbäddad** | `Can't initialize Python`, `Can't initialize Python interpreter`, `Custom path to Python dll directory:`, `Changing Python libraries may require closing and restarting REAPER.`, `After installing Python, REAPER may detect the Python dynamic library automatically.`, `Download Python`, `13Python_Editor`, `9PythonAPI`. Ingen `libpython`-sträng och inget NEEDED → `dlopen` vid körning mot användarens Python. |
| **JS** | **finns inte som eget språk** | `JS:` i FX-namn betyder JSFX, dvs EEL2 (§2.5). Inga `javascript`/`V8`/`QuickJS`-strängar. |

Dokumentationen genereras ur binären själv: strängarna `ReaScript/EEL2 API`,
`ReaScript/Python API`, `<div class="e_func">`, `<div class="p_func">`,
`<a name="eel_list">` är HTML-mallar som bygger API-referensen i appen. Det är därför
API-namnen finns som strängar i binären över huvud taget.

---

## 5. Kontrollsytor och protokoll

`reaper_csurf.so` (490 832 B) exporterar `ReaperPluginEntry` och innehåller
**nio** kontrollyte-implementationer som klassnamn:

`CSurf_MCU`, `CSurf_BCF2k`, `CSurf_BabyHUI`, `CSurf_FaderPort`, `CSurf_FaderPort_FP2`,
`CSurf_Osc` (+ `createlocaloschandler`, `destroylocaloschandler`,
`CSurf_OnOscControlMessage2`, `GetOscCfgDir`), plus enheter/etiketter:
`Mackie Control Universal`, `Mackie Control Extender`, `Mackie Control Extended`,
`HUI (partial)`, `Behringer BCF2000 (using preset 1)`, `Frontier AlphaTrack`,
`Frontier TranzPort`, `PreSonus FaderPort`, `mcu_mcp`, `MCUEX`, `BCF2K`, `FADERPORT`,
`FADERPORT2`.

- **OSC**: `reaper/osc` (protokoll-id), `OSC: listening to port %d`, `Error: can't create
  OSC listener`, `OSC (Open Sound Control)`, `OSC Config File Warning`;
  79 `OSC`-träffar i `reaper`; konfigfilen `Plugins/Default.ReaperOSC` är 24 744 B
  med adressmönster (`/action`, `/track`, …) som användaren kan skriva om.
- **MIDI-hårdvara**: `reaper_midi.so` är den största pluginmodulen (2 100 120 B) med
  `midi_Input_jack`, `midi_Output_jack` (JACK MIDI), `midi_insts` (instrumentdefinitioner,
  `.ins`), `midi_inlinehand`, `Sequencer data contains more than 255 bytes…`.
- **Ljudbackend**: ALSA (`libasound` NEEDED, `ALSA: error setting input device
  samplerate`), JACK (`audioStreamer_JACK`, `JACK`, `jack_activate`, `jack_client`,
  `Auto-connect jack audio channels to hardware`), `Dummy Audio`, samt
  `audioStreamer_linux_asiosim` (ASIO-emulering ovanpå ALSA).
- **ReaRoute** (mellan-applikationsljud, som ett virtuellt ljudkort):
  `rearoute_in`, `rearoute_out`, `rearoute_loopback`, `rearoute_loopback_delay`,
  `ReaRoute %d`, `Audio ReaRoute Output`, `ReaRoute Input`.
- **Webbgränssnitt**: `reaper_www_root` (sträng i binären) + katalogen
  `Plugins/reaper_www_root/` med 7 filer, 234 175 B (`index.html` 57 687 B,
  `fancier.html` 128 417 B, `main.js` 13 463 B, `basic.html`, `more_me.html`,
  `lyrics.html`, `click.html`) och strängen `Web Interface Pages`. Alltså en HTTP-server
  i huvudbinären som serverar en JS-UI (som sedan talar med OSC/ReaScript).

---

## 6. Vad Sonix saknar ur REAPERs binär

Mätt mot `/home/alex/Projects/sonix` (grep i `src/**/*.rs` och `Cargo.toml`).
Storlek: **S** ≈ dagar, **M** ≈ veckor, **L** ≈ månader. Osäkerhet står där jag inte kan
avgöra saken ur binären eller ur repot.

| Komponent | Bevis för att REAPER har den | Sonix idag (mätning) | Storlek | Osäkerhet |
|---|---|---|---|---|
| **élastique Pro/Efficient/Soloist** (pitch/stretch med PSOLA) | `elastique3.so` 1 691 880 B; `CreateInstance_E3`, `CElastiqueProV3`, `PSOLASynthForElastiqueProV3`, `ElastiqueShifter_V3` i `reaper` | `src/audio/stretch.rs` = egen **WSOLA** (`vocal_harmonizer::Wsola`), `src/audio/warp.rs` 5 lägen som bara är olika WSOLA-parametrar (`wsola_params`); `signalsmith-stretch` endast dev-dependency. Grep: `elastique` 0 träffar | **L** (licens) / **M** (om köps in) | Kan inte avgöra ur binären vilka kvalitetslägen som är default |
| **Rubber Band** (formantbevarande, Speex-resampler) | `rubberband.so` 184 784 B; `RubberBandStretcher::Impl::formantShiftChunk`, `RubberBand::Resamplers::D_Speex`; sträng `Rubber Band Library audio timestretcher by Breakfast Quay` | `rubberband` 0 träffar, ingen dep i `Cargo.toml` | **S** (länka lib) / **M** (hosta dess lägen) | Version okänd ur binären |
| **SoundTouch** (billig stretch) | `soundtouch.so` 51 560 B; `soundtouch::TDStretchSSE`, `SoundTouch Version: %s` | 0 träffar | **S** | — |
| **r8brain-class omsampling** | `N3r8b13CDSPResamplerE`, `Includes r8brain free designed by Aleksey Vaneev of Voxengo`, `r8brain free (highest quality, fast)`, `Dots and sinc`, `REAPER_Resample_Interface` | Endast linjär interpolation: `resample_linear` i `neural_separator.rs:418` och `ui/fx_rack_modal.rs:1940`; `\bsinc\b` ger **3** träffar, alla i `loudness.rs` (4× oversampling för true peak) | **M** | Om Sonix behöver hög kvalitet alls beror på om projekt-SR ≠ fil-SR tillåts — `wav_reader.rs` läser native SR utan konvertering |
| **In-process-kodekar** (FLAC, MP3, Vorbis, Opus, WavPack, AIFF) + **encoders** (LAME, Vorbis, Opus, FLAC, WavPack) | 22 `PCM_source_*` + 10 `PCM_sink_*`; `get_lame_version`, `FLAC__STREAM_DECODER_*`, `VorbisEncoder`, `OggOpusEncoder`, `PCM_sink_wavpack` | `wav_reader.rs:297 load_audio_pcm` → `load_via_ffmpeg`, dvs **extern `ffmpeg`-binär** (samma i `video.rs`). `Cargo.toml` saknar `symphonia`/`claxon`/`lewton`/`minimp3`. `opus` 0 träffar, `wavpack` 0 träffar | **M** (rena Rust-libs) | Opus saknas helt; ffmpeg-vägen kräver att ffmpeg finns på maskinen |
| **Videomotor** (avkoda + koda + GL + spellista) | `reaper_video.so` 2 040 176 B; `FFMPEGDecodeStream_54…_63`, `FFMPEGVideoEncoder_54…_63`, `avcodec_receive_packet`, GL-deps i `ldd` | `src/audio/video.rs` 263 rader: endast `probe()` och `frame_rgb()` (en bildruta i taget via ffmpeg-subprocess); `ui/video_view.rs` ritar. Ingen encode, ingen tidslinje, ingen FX-kedja | **L** | — |
| **Video processor-språk** (shader-DSL med `//@param`, `//@gmem`) | `//@param`, `//@gmem=jsfx_to_video`, `input_info(input, w, h[,srctime, wet, parm1,…])`, `gl_FragColor`, `gl_TexCoord[0].x` i `reaper_video.so` | 0 träffar på `//@param`/`gmem` i `src` | **L** | Kräver att Sonix först har en videomotor |
| **LV2-hosting** | 151 `lv2`-träffar: `LV2Ctx`, `lv2plug.in/ns/ext/atom#…`, `%d plug-ins scanned [%d lv2 bundles]`, egen skanner | `src/audio/plugin_host.rs` har `PluginFormat::Lv2` **som etikett** och `is_lv2_path()` **bara för att avvisa** — kommentaren säger rakt ut: "Sonix har ingen LV2-värd" | **M** | LV2 är ett litet API; M gäller full UI/state-hantering |
| **Bit-bridge för alla format** (separat process) | `Plugins/reaper_host_x86_64` (207 928 B, eget program), `#BRIDGE#`, `Bridge_IComponent/IEditController/IPluginFactory/IPlugView`, `Separate process` | `src/audio/plugin_sandbox.rs` 944 rader finns och används (`ui/app/plugins.rs:165 SandboxHost::new_audio`), men `load_processor` i `plugin_host_live.rs` routar bara **CLAP** dit (VST2/VST3 har egna moduler) | **M** | Okänt om VST2/VST3 redan går via sandbox — grep visar en anropsplats |
| **Lua + 732-funktioners ReaScript-API** | `$LuaVersion: Lua 5.4.6…`; 732 `RPR_`-funktioner i `reaper_python.py`; `ReaScript/Lua are always supported` | `rhai 1.27` (`Cargo.toml:67`), `src/audio/script.rs` 302 rader med **~10** registrerade funktioner (`count`, `pitch`, `velocity`, `set_pitch`, `transpose`, `load_midi`, `save_midi`, …), `project_script.rs` 1 `run()`. `lua` 0 riktiga träffar (3 falska: "evaluated"/"evaluation") | **S** att bädda in Lua, **L** för API-paritet | API-ytan är det dyra, inte språket |
| **EEL2 som projekt-skript** | `ReaScript/EEL2 API`; 62 `nseel_asm_*` + 7 `eel_*` i `reaper` | EEL2 finns redan i `src/audio/jsfx/` (engine/parser/processor) men är bara kopplad till JSFX-laddaren | **S** | Interpren finns; det är kopplingen och API:t som saknas |
| **Python-skript** | `Can't initialize Python`, `Custom path to Python dll directory:`, `9PythonAPI` | 0 träffar | **M** | Kräver `pyo3`/dlopen mot användarens Python |
| **DDP-export + CD-bränning (CUE/BIN)** | `reaper_ddp.so` 136 840 B (`DDP 2.00`, `Corrupted DDPMS file`, CD-TEXT/ISRC); `reaper_cd.so` 96 696 B (`Burn CD`, `Audio CD Image (CUE/BIN format)`) | 0 träffar | **M** | Bränning kräver `cdrecord`/`growisofs`-väg; DDP-exporten är ren filskrivning (S) |
| **JACK-backend + ReaRoute** | `audioStreamer_JACK`, `jack_activate`, `jack_client`, `Auto-connect jack audio channels…`; `rearoute_in/out/loopback`, `ReaRoute %d` | `Cargo.toml`: `cpal 0.15` + `alsa 0.9`; `jack` 0 träffar, `rearoute` 0 träffar | **M** | ReaRoute är REAPER-egen och går inte att återskapa binärt utan protokoll — M gäller JACK |
| **HTTP-server + webbgränssnitt** | `reaper_www_root` + 7 HTML/JS-filer (234 175 B), `Web Interface Pages` | Inga `TcpListener`/http-server i `src` (grep ger bara URL:er till AI-API:er) | **M** | — |
| **Media Explorer med preview/tempo-match/databas** | `reaper_explorer.so` 643 408 B; `Apply preview pitch/rate to inserted media item`, `Auto-stop preview…`, metadata-cache | `src/ui/app/browser.rs` finns (filläsare), men ingen preview med tempo-matchning hittad | **S/M** | **Osäkert** — jag har inte läst browser-koden i detalj, bara grepat |
| **.ins-instrumentdefinitioner för MIDI-hårdvara** | `reaper_midi.so` 2 100 120 B, `midi_insts`, `midi_Input_jack`, `midi_Output_jack` | `midir 0.11`, `midi_input.rs`/`midi_output.rs`; `.ins` 0 träffar | **S** | — |
| **Nativ synth/sampler** | **Nej** — REAPER har ingen (bevis: 0 klassnamn, migreringstabellen pekar på JSFX) | Sonix har egen synth (`src/audio/synth/`, `sf2.rs`) | — | Sonix ligger före här; ingen åtgärd |
| **Dither + noise shaping** | 13 `dither`-träffar, `RENDER_DITHER`-bitmask, `Noise shaping`, `Dither master/stems` | `src/audio/dither.rs` 295 rader: TPDF + första ordningens noise shaping (`with_noise_shaping`) | — | Sonix har det; gapet är eventuellt högre ordningars kurvor, men **REAPERs kurvor går inte att läsa ur binären** |

### Det som inte går att avgöra ur binären

- Exakta versioner av Rubber Band och SoundTouch (versionssträng byggs vid körning).
- Vilken stretch-motor som är default, och om élastique har fler lägen än Pro/Efficient/Soloist.
- Dither-kurvornas ordning och viktning (inga namnsträngar).
- Om LV2-värden använder något annat bibliotek än egen kod (bara frånvaro av lilv/serd/sord mätt).
- Vilket dlopen-namn `reaper_video.so` använder för ffmpeg (endast symbolnamn + en sökvägssträng).
