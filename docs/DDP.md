# DDP-export (Red Book, bild utan bränning)

`sonix --export-ddp <fil.wav> <utmapp> [titel:start_sampel[:isrc] ...]` skriver en
DDP 2.00-bild — en mapp med filer som ett pressverk kan läsa. Ingenting bränns,
monteras eller skickas. `sonix --export-cue <fil.wav> <utmapp> [...]` skriver samma
program som `image.cue` + `image.bin` (CUE/BIN), formen REAPER:s `reaper_cd.so`
erbjuder som "Audio CD Image (CUE/BIN format)".

Källan måste vara **44,1 kHz** stereoljud. Är den 48 kHz avbryts exporten med ett
fel som namnger båda frekvenserna — vi samplar inte om i tysthet, eftersom ett
felaktigt antal sampel per sektor gör hela bilden oanvändbar hos pressverket.

## Filerna i bilden

| Fil | Innehåll |
|-----|----------|
| `DDPID` | Fast 128-byte-post. Börjar med magic `DDP 2.00`, därefter raderna `IMAGE_ID`, `TRACKS`, `SECTORS`, `SAMPLE_RATE`, `CHANNELS`, `BITS`; posten fylls ut med blanksteg till exakt 128 byte. |
| `DDPMap` | En rad per fil i bilden, i bildordning: `namn byteantal sektorer`. `DDPMap` listar också sig själv. |
| `PQDESCR` | En rad per spår: `TN=02 INDEX=01 ABS=MM:SS:FF ISRC=...`. Tiderna är absoluta sektortider (75 rutor per sekund). ISRC-raden är tom när spåret saknar ISRC. |
| `CDTEXT` | Skrivs **bara** när det finns metadata: `DISC TITLE=`, `DISC PERFORMER=` och `TRACK nn TITLE=` för spår med namn. Utan metadata skapas ingen fil — en CDTEXT med tomma fält är inte metadata. |
| `AUDIO.DAT` | 16-bitars little-endian PCM, 44,1 kHz, stereo. 588 sampelramar per sektor = 2352 byte. |

`image.bin` i CUE/BIN-varianten är byte-för-byte samma data som `AUDIO.DAT`;
`image.cue` pekar på den med `FILE "image.bin" BINARY` och en `TRACK nn AUDIO` +
`INDEX 01 MM:SS:FF` per spår.

## Uttoningsregeln (padding)

En CD-sektor rymmer alltid exakt 588 sampelramar (2352 byte). Antalet ramar i
källan är sällan en multipel av 588, så sista sektorn fylls ut med **digital tystnad**
(nollor) upp till nästa hela sektor:

```
sektorer = ceil(ramar / 588)
AUDIO.DAT = sektorer * 2352 byte
```

Exempel: 6 180 ramar (10,5 sektor) ger 11 sektorer = 25 872 byte, alltså 6 468 ramar
i filen varav de sista 288 är tystnad. `AUDIO.DAT` är därför alltid en jämn multipel
av 2352 byte, och antalet ramar efter uttoning är `sektorer * 588`.

Samma regel gäller för `DDPMap`s sektorantal: textfilerna är ASCII och deras
fotavtryck avrundas **uppåt** till hela sektorer i kartan, men bara `AUDIO.DAT`
är sektor-exakt på disk.

## Spårgränser och ISRC

- Spårlistan är `(namn, start_sampel)`. Första spåret måste börja på sampel 0.
- Varje start måste ligga på en sektorgräns (multipel av 588), annars avbryts
  exporten: en spårstart mitt i en sektor kan inte beskrivas i PQ.
- ISRC är exakt 12 tecken, versaler, A–Z och 0–9 (land 2 + registrant 3 + år 2 +
  beteckning 5). Gemener godtas och skrivs som versaler. För kort, för långt,
  blanksteg eller bindestreck avbryts exporten med ett fel som namnger fältet
  `ISRC` och citerar värdet.

## Vad som inte är verifierat

Filerna följer den form den här skrivaren producerar och läses tillbaka av testerna
i `src/audio/ddp.rs`. Textbeskrivningarna är ASCII-rader, inte CD-TEXT:ens 18-byte-pack,
och bilden är **inte** prövad mot ett pressverk eller mot ett kommersiellt
DDP-verktyg — det kräver extern programvara (t.ex. en DDP-validerare) som inte
finns i den här miljön.
