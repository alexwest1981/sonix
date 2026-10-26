#!/usr/bin/env bash
# **Beviset, inte påståendet.** Installerar paketet i en riktig distribution och startar
# appen med Xvfb, så att det som mäts är paketet så som en ny användare får det.
#
#   bash packaging/verify-in-docker.sh            # debian:12 ubuntu:24.04 fedora:41 rockylinux:9
#   bash packaging/verify-in-docker.sh debian:11  # en enskild
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

IMAGES=("$@")
if [[ ${#IMAGES[@]} -eq 0 ]]; then
    IMAGES=(debian:12 ubuntu:24.04 fedora:41 rockylinux:9)
fi
DEB="$(ls -t dist/sonix_*.deb 2>/dev/null | head -1)"
RPM="$(ls -t dist/sonix-*.rpm 2>/dev/null | head -1)"
FAIL=0

for img in "${IMAGES[@]}"; do
    echo "=== $img"
    if ! out="$(docker run --rm -v "$ROOT/dist:/d:ro" "$img" bash -c '
        set -e
        if command -v apt-get >/dev/null; then
            apt-get update -qq >/dev/null 2>&1
            pkg="/d/$(ls /d | grep -m1 "^sonix_.*\.deb$")"
            DEBIAN_FRONTEND=noninteractive apt-get install -y -qq xvfb "$pkg" >/dev/null 2>&1
        else
            pkg="/d/$(ls /d | grep -m1 "^sonix-.*\.rpm$")"
            dnf install -y -q xorg-x11-server-Xvfb "$pkg" >/dev/null 2>&1
        fi
        command -v sonix >/dev/null || { echo "sonix finns inte i PATH"; exit 1; }
        mkdir -p /tmp/shots
        # Startar den alls? Xvfb + skärmdumpsläget ritar hela ytan utan ljud.
        timeout 90 xvfb-run -a sonix --capture-screenshots /tmp/shots >/tmp/run.log 2>&1 || true
        n=$(find /tmp/shots -name "*.png" | wc -l)
        echo "glibc $(ldd --version | head -1 | awk "{print \$NF}") · skärmdumpar: $n"
        [[ "$n" -ge 5 ]] || { echo "appen ritade inget"; tail -5 /tmp/run.log; exit 1; }
    ' 2>&1)"; then
        echo "$out" | tail -6
        echo "FEL i $img"
        FAIL=1
    else
        echo "$out" | tail -3
    fi
done
exit $FAIL
