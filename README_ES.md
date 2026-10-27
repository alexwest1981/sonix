<p align="center">
  <img src="assets/sonix.png" alt="Sonix Studio" width="150">
</p>

> **Languages:** [🇬🇧 English](README.md) · [🇸🇪 Svenska](README_SV.md) · [🇩🇰 Dansk](README_DA.md) · [🇳🇴 Norsk](README_NB.md) · [🇩🇪 Deutsch](README_DE.md) · [🇪🇸 Español](README_ES.md) · [🇫🇷 Français](README_FR.md)

# 🍊 SONIX STUDIO — DAW profesional para Linux

Una DAW nativa para Linux escrita en **Rust** y **egui**: rack de canales con baterías y sintes,
piano roll, mezclador con buses y envíos, alojamiento VST3/CLAP, exportación offline y un asistente
de IA que escribe un beat a partir de una frase. Un solo binario que solo enlaza glibc y `libasound`.

![Arreglo](screenshots/01_tidslinje_arranger.png)

**La interfaz está en español** — y también en sueco, danés, noruego, alemán, francés e inglés.
Se cambia en vivo desde el menú de idioma de la aplicación.

## Instalación

| Distribución | Comando |
| :--- | :--- |
| **Cualquier distribución** (sin instalar nada) | `chmod +x Sonix-*.AppImage && ./Sonix-*.AppImage` |
| Debian · Ubuntu · Mint · Pop! · Zorin · Kali | `sudo apt install ./sonix_<version>_amd64.deb` |
| Fedora · RHEL · Rocky · Alma · openSUSE | `sudo dnf install ./sonix-<version>-1.x86_64.rpm` |
| Arch · Omarchy · Manjaro · EndeavourOS | `yay -S sonix-bin` (AUR) |
| Todo lo demás | `.tar.gz` desde [Releases](https://github.com/alexwest1981/sonix/releases/latest) |

Requiere x86-64 con glibc ≥ 2.28 y un servidor de audio (PipeWire, PulseAudio o ALSA). ffmpeg es
opcional y solo hace falta para exportar MP3/OGG — WAV y FLAC funcionan siempre.

## Licencia

GPL-3.0-only. Puedes usarlo, compilarlo, modificarlo y compartirlo libremente — pero quien comparta
una Sonix modificada debe publicar el código fuente bajo las mismas condiciones. Los plugins no se
ven afectados: se cargan como binarios separados.

La lista completa de funciones, todas las capturas y el plan de desarrollo están en
**[inglés](README.md)**. Las contribuciones son bienvenidas — mira [CONTRIBUTING.md](CONTRIBUTING.md).
