<p align="center">
  <img src="assets/sonix.png" alt="Sonix Studio" width="150">
</p>

> **Languages:** [🇬🇧 English](README.md) · [🇸🇪 Svenska](README_SV.md) · [🇩🇰 Dansk](README_DA.md) · [🇳🇴 Norsk](README_NB.md) · [🇩🇪 Deutsch](README_DE.md) · [🇪🇸 Español](README_ES.md) · [🇫🇷 Français](README_FR.md)

# 🍊 SONIX STUDIO — profesjonell Linux-DAW

En innebygd Linux-DAW skrevet i **Rust** og **egui**: kanalrack med trommer og synth, pianorull,
mikser med busser og sends, VST3/CLAP-vert, offline-eksport og en AI-assistent som skriver et beat
ut fra en setning. Ett binærprogram som bare lenker glibc og `libasound`.

![Arrangement](screenshots/01_tidslinje_arranger.png)

**Grensesnittet finnes på norsk** — og på svensk, dansk, tysk, spansk, fransk og engelsk. Bytt live
i appens språkmeny.

## Installation

| Distribution | Kommando |
| :--- | :--- |
| **Alle distribuerer** (ingen installation) | `chmod +x Sonix-*.AppImage && ./Sonix-*.AppImage` |
| Debian · Ubuntu · Mint · Pop! · Zorin · Kali | `sudo apt install ./sonix_<version>_amd64.deb` |
| Fedora · RHEL · Rocky · Alma · openSUSE | `sudo dnf install ./sonix-<version>-1.x86_64.rpm` |
| Arch · Omarchy · Manjaro · EndeavourOS | `yay -S sonix-bin` (AUR) |
| Alt andet | `.tar.gz` fra [Releases](https://github.com/alexwest1981/sonix/releases/latest) |

Krever x86-64 med glibc ≥ 2,28 og en lydserver (PipeWire, PulseAudio eller ALSA). ffmpeg er valgfrit
og kun nødvendig for MP3/OGG-eksport — WAV og FLAC virker alltid.

## Lisens

GPL-3.0-only. Bruk, bygg, endre og del fritt — men den som deler en endret Sonix, må publisere
kildekoden på samme vilkår. Plugins er ikke omfattet: de lastes som separate binærfiler.

Hele funksjonslisten, alle skjermbilder og utviklingsplanen finnes på **[engelsk](README.md)**.
Bidrag er velkomne — se [CONTRIBUTING.md](CONTRIBUTING.md).
