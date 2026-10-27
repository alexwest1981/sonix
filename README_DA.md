<p align="center">
  <img src="assets/sonix.png" alt="Sonix Studio" width="150">
</p>

> **Languages:** [🇬🇧 English](README.md) · [🇸🇪 Svenska](README_SV.md) · [🇩🇰 Dansk](README_DA.md) · [🇳🇴 Norsk](README_NB.md) · [🇩🇪 Deutsch](README_DE.md) · [🇪🇸 Español](README_ES.md) · [🇫🇷 Français](README_FR.md)

# 🍊 SONIX STUDIO — professionel Linux-DAW

En native Linux-DAW skrevet i **Rust** og **egui**: kanalrack med trommer og synth, pianorulle,
mixer med busser og sends, VST3/CLAP-hosting, offline-eksport og en AI-assistent der skriver et
beat ud fra en sætning. Ét binært program, som kun linker glibc og `libasound`.

![Arrangement](screenshots/01_tidslinje_arranger.png)

**Grænsefladen fås på dansk** — og på svensk, norsk, tysk, spansk, fransk og engelsk. Skift live i
appens sprogmenu.

## Installation

| Distribution | Kommando |
| :--- | :--- |
| **Alle distribuerer** (ingen installation) | `chmod +x Sonix-*.AppImage && ./Sonix-*.AppImage` |
| Debian · Ubuntu · Mint · Pop! · Zorin · Kali | `sudo apt install ./sonix_<version>_amd64.deb` |
| Fedora · RHEL · Rocky · Alma · openSUSE | `sudo dnf install ./sonix-<version>-1.x86_64.rpm` |
| Arch · Omarchy · Manjaro · EndeavourOS | `yay -S sonix-bin` (AUR) |
| Alt andet | `.tar.gz` fra [Releases](https://github.com/alexwest1981/sonix/releases/latest) |

Kræver x86-64 med glibc ≥ 2,28 og en lydserver (PipeWire, PulseAudio eller ALSA). ffmpeg er valgfrit
og kun nødvendigt for MP3/OGG-eksport — WAV og FLAC virker altid.

## Licens

GPL-3.0-only. Brug, byg, ændr og del frit — men den der deler en ændret Sonix, skal udgive
kildekoden på samme vilkår. Plugins er ikke omfattet: de indlæses som separate binære filer.

Hele funktionslisten, alle skærmbilleder og udviklingsplanen findes på **[engelsk](README.md)**.
Bidrag er velkomne — se [CONTRIBUTING.md](CONTRIBUTING.md).
