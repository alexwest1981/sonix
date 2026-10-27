<p align="center">
  <img src="assets/sonix.png" alt="Sonix Studio" width="150">
</p>

> **Languages:** [🇬🇧 English](README.md) · [🇸🇪 Svenska](README_SV.md) · [🇩🇰 Dansk](README_DA.md) · [🇳🇴 Norsk](README_NB.md) · [🇩🇪 Deutsch](README_DE.md) · [🇪🇸 Español](README_ES.md) · [🇫🇷 Français](README_FR.md)

# 🍊 SONIX STUDIO — STAN professionnelle pour Linux

Un STAN (DAW) natif pour Linux écrit en **Rust** et **egui** : rack de canaux avec batteries et
synthés, piano roll, mixeur avec bus et départs, hébergement VST3/CLAP, export hors ligne et un
assistant IA qui écrit un beat à partir d'une phrase. Un seul binaire, lié uniquement à glibc et
`libasound`.

![Arrangement](screenshots/01_tidslinje_arranger.png)

**L'interface est en français** — ainsi qu'en suédois, danois, norvégien, allemand, espagnol et
anglais. Le changement se fait à chaud depuis le menu de langue de l'application.

## Installation

| Distribution | Commande |
| :--- | :--- |
| **Toute distribution** (sans rien installer) | `chmod +x Sonix-*.AppImage && ./Sonix-*.AppImage` |
| Debian · Ubuntu · Mint · Pop! · Zorin · Kali | `sudo apt install ./sonix_<version>_amd64.deb` |
| Fedora · RHEL · Rocky · Alma · openSUSE | `sudo dnf install ./sonix-<version>-1.x86_64.rpm` |
| Arch · Omarchy · Manjaro · EndeavourOS | `yay -S sonix-bin` (AUR) |
| Tout le reste | `.tar.gz` depuis les [Releases](https://github.com/alexwest1981/sonix/releases/latest) |

Nécessite x86-64 avec glibc ≥ 2.28 et un serveur audio (PipeWire, PulseAudio ou ALSA). ffmpeg est
facultatif et ne sert qu'à l'export MP3/OGG — WAV et FLAC fonctionnent toujours.

## Licence

GPL-3.0-only. Utilisation, compilation, modification et partage libres — mais quiconque partage une
Sonix modifiée doit publier le code source aux mêmes conditions. Les plugins ne sont pas concernés :
ils sont chargés comme binaires séparés.

La liste complète des fonctions, toutes les captures et la feuille de route sont en
**[anglais](README.md)**. Les contributions sont bienvenues — voir [CONTRIBUTING.md](CONTRIBUTING.md).
