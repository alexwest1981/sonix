#!/usr/bin/env bash
# AppImage: en fil, inget rot-lösen, fungerar på varje glibc-distro från bygg-Imagen.
#
#   bash packaging/build-appimage.sh            # bygger dist/Sonix-<version>-x86_64.AppImage
#
# Kräver att `target/release/sonix` redan är byggd (CI gör det före).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

ID="io.github.alexwest1981.Sonix"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
ARCH="${ARCH:-x86_64}"
TOOLS="${TOOLS:-$ROOT/.appimage-tools}"
APPDIR="$ROOT/dist/Sonix.AppDir"

mkdir -p "$TOOLS" dist "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/512x512/apps" \
         "$APPDIR/usr/share/metainfo"

# linuxdeploy + appimagetool: hämtas en gång, ligger kvar i .appimage-tools.
fetch() {
    local url="$1" out="$TOOLS/$(basename "$1")"
    [[ -x "$out" ]] && return 0
    echo "hämtar $(basename "$url")"
    curl -fsSL "$url" -o "$out"
    chmod +x "$out"
}
fetch "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-${ARCH}.AppImage"
fetch "https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-${ARCH}.AppImage"

install -m 755 target/release/sonix "$APPDIR/usr/bin/sonix"
install -m 644 "packaging/$ID.desktop" "$APPDIR/usr/share/applications/$ID.desktop"
install -m 644 assets/sonix.png "$APPDIR/usr/share/icons/hicolor/512x512/apps/$ID.png"
install -m 644 "packaging/$ID.metainfo.xml" "$APPDIR/usr/share/metainfo/$ID.metainfo.xml"
# AppImage-specen vill ha desktop-filen och ikonen i roten också.
cp "packaging/$ID.desktop" "$APPDIR/$ID.desktop"
cp assets/sonix.png "$APPDIR/$ID.png"

# --appimage-extract-and-run: containrar (CI) har ingen FUSE.
export ARCH
export APPIMAGE_EXTRACT_AND_RUN=1
"$TOOLS/linuxdeploy-${ARCH}.AppImage" --appdir "$APPDIR" \
    --desktop-file "$APPDIR/$ID.desktop" \
    --icon-file "$APPDIR/$ID.png"

OUT="dist/Sonix-${VERSION}-${ARCH}.AppImage"
"$TOOLS/appimagetool-${ARCH}.AppImage" "$APPDIR" "$OUT"
(cd dist && sha256sum "$(basename "$OUT")" > "$(basename "$OUT").sha256")
echo "klar: $OUT"
