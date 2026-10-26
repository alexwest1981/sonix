#!/usr/bin/env bash
# **Beviset, inte påståendet.** Installerar paketet i en riktig distribution och mäter två saker:
# att beroendet löstes (ingen `not found` i ldd) och att binären inte begär nyare glibc än
# distributionen har. Kan Xvfb installeras startar appen också på riktigt och ska rita minst fem
# skärmdumpar.
#
#   bash packaging/verify-in-docker.sh                      # debian:12 ubuntu:24.04 fedora:41 rockylinux:9
#   bash packaging/verify-in-docker.sh debian:11            # en enskild
#
# **Debian 11 mäts statiskt, inte med Xvfb:** dess spegel ligger på archive.debian.org, så
# `apt install xvfb` ger 404. Paketet självt installeras och länkar rätt — det är den skillnaden
# skriptet ska visa, inte dölja.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

IMAGES=("$@")
if [[ ${#IMAGES[@]} -eq 0 ]]; then
    IMAGES=(debian:12 ubuntu:24.04 fedora:41 rockylinux:9)
fi
FAIL=0

for img in "${IMAGES[@]}"; do
    echo "=== $img"
    out="$(docker run --rm -v "$ROOT/dist:/d:ro" "$img" bash -c '
set -u
if command -v apt-get >/dev/null; then
    # **Debian 11 är EOL och spegeln flyttad.** Utan den här raden ger `apt update` 404 på
    # halva arkivet och paketet "installeras inte" av ett skäl som inte har med Sonix att göra.
    if [ -f /etc/debian_version ] && [ "$(cut -d. -f1 /etc/debian_version)" = "11" ]; then
        sed -i "s|deb.debian.org|archive.debian.org|g; s|security.debian.org|archive.debian.org|g" \
            /etc/apt/sources.list
        sed -i "/-security/d; /-updates/d" /etc/apt/sources.list
        APT_OPTS="-o Acquire::Check-Valid-Until=false"
    fi
    APT_OPTS="${APT_OPTS:-}"
    apt-get $APT_OPTS update -qq >/dev/null 2>&1
    pkg="/d/$(ls /d | grep -m1 "^sonix_.*\\.deb$")"
    DEBIAN_FRONTEND=noninteractive apt-get $APT_OPTS install -y -qq "$pkg" >/tmp/inst.log 2>&1
    true
else
    pkg="/d/$(ls /d | grep -m1 "^sonix-.*\\.rpm$")"
    dnf install -y -q "$pkg" >/tmp/inst.log 2>&1
    true
fi
glibc="$(ldd --version | head -1 | grep -o "[0-9][0-9.]*$")"
if ! command -v sonix >/dev/null; then
    echo "FEL: paketet installerades inte (glibc $glibc)"; tail -3 /tmp/inst.log; exit 1
fi
missing="$(ldd /usr/bin/sonix 2>/dev/null | grep -c "not found")"
need="$(objdump -T /usr/bin/sonix | grep -o "GLIBC_[0-9.]*" | sed "s/GLIBC_//" | sort -V | tail -1)"
echo "glibc $glibc · binären begär GLIBC_$need · saknade bibliotek: $missing"
[ "$missing" = "0" ] || { echo "FEL: $missing bibliotek saknas"; exit 1; }
[ "$(printf "%s\\n%s\\n" "$need" "$glibc" | sort -V | tail -1)" = "$glibc" ] || {
    echo "FEL: binären kräver glibc $need, distributionen har $glibc"; exit 1; }
have_x="${VERIFY_GUI:-1}"
if [ "$have_x" = "0" ]; then
    mkdir -p /tmp/shots
    timeout 90 xvfb-run -a sonix --capture-screenshots /tmp/shots >/tmp/run.log 2>&1
    n=$(find /tmp/shots -name "*.png" | wc -l)
    echo "startade med Xvfb · skärmdumpar: $n"
    [ "$n" -ge 5 ] || { echo "FEL: appen ritade inget"; tail -5 /tmp/run.log; exit 1; }
else
    # Startar den alls? Utan skärm dör en GUI-app i winit — men den ska dö *där* och inte i
    # länkaren. Det skiljer "fel distro" från "ingen skärm" utan att installera 120 paket.
    timeout 20 sonix >/tmp/run.log 2>&1
    if grep -qE "error while loading shared libraries|GLIBC_.*not found" /tmp/run.log; then
        echo "FEL: binären länkar inte på den här distributionen"; grep -E "not found|GLIBC" /tmp/run.log | head -3; exit 1
    fi
    echo "länkkontroll ok (ingen skärm här: Xvfb är opt-in med VERIFY_GUI=1)"
fi
' 2>&1)"
    rc=$?
    echo "$out" | tail -6
    if [[ $rc -ne 0 ]]; then FAIL=1; echo "FEL i $img"; fi
done
exit $FAIL
