# Windows-plugins och Native Instruments i Sonix

Det här är vägen till **Kontakt 8, Massive X, Reaktor, Absynth** och de andra Native
Instruments-biblioteken på Linux — och till alla andra Windows-VST2/VST3.

Sonix hostar dem **inte** själv: `yabridge` gör om en Windows-plugin till en Linux-VST3 som ser ut
som vilken `.vst3` som helst, och Sonix laddar den som en vanlig VST3 (in-process eller i sandboxen).
Ingen extra kod i Sonix behövs för själva ljudet.

## 1. Installera Native Access under Wine

[`selimbucher/native-instruments`](https://github.com/selimbucher/native-instruments) (`ni-wine`)
sätter upp Native Access 2 i prefixet `~/.wine-ni`:

```sh
yay -S ni-wine            # Arch
native-access             # första starten skapar prefixet och installerar Native Access
```

Därifrån installeras produkterna som på Windows — du behöver ditt eget NI-konto. (Repot innehåller
ingen NI-programvara och kan inte ge åtkomst till någon; det är bara installationsskript.)

## 2. Brygga dem

`ni-wine` kräver yabridges **utvecklingsbygge** för Wine 10+:

```sh
yay -S yabridge-wine10-git yabridgectl-wine10-git

yabridgectl add "$HOME/.wine-ni/drive_c/Program Files/Common Files/VST3"
yabridgectl sync
```

`yabridgectl sync` lägger de färdiga `.so`-filerna i `~/.vst3` (och `~/.vst` för VST2).

## 3. Sonix hittar dem

Vid skanning läses **`~/.config/yabridgectl/config.toml`** — det är yabridge som vet var
Windows-mapparna ligger, så Sonix gissar inte. Dessutom läggs `~/.vst3`, `~/.vst` och
Wine-prefixens `Common Files/VST3` + `Common Files/VST` till automatiskt, inklusive `~/.wine-ni`.

Mätt: en config med två `plugin_dirs` (varav en i `~/.wine-ni`) ger exakt de två sökvägarna, och en
tom config ger inga påhittade.

## 4. Kör dem i sandboxen

Wine-plugins kan krascha hårt. Lägg dem i **sandboxen** (separat process, delat minne för ljudet) i
stället för in-process: `🧪 Ladda in i sandbox`. Mätt mot en riktig VST3 (Surge XT Effects):
backend `VST3 (sandbox)`, topp 0,437446, RMS 0,181184 — och en trasig sökväg faller tillbaka
in-process på 12 ms med orsaken i loggen.

## Vad sonix **inte** gör

- **Ingen VST2-mätning**: koden routar `.vst`/`.dll` genom sandboxen, men maskinen har ingen
  VST2-binär att mäta mot. Yabridge-VST2 syns i listan och laddas in-process.
- **Ingen NI-auktorisering**: Native Access sköter login och licenser inne i Wine-prefixet.
- **Inga NI-bibliotek medföljer**: Kontakt-biblioteken ägs av användaren, precis som på Windows.
