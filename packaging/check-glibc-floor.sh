#!/usr/bin/env bash
# **Grinden för "fungerar på alla distrar".** Läser vilka GLIBC-symbolversioner binären
# begär och faller om någon är nyare än golvet. Utan den här grinden är en release bara
# verifierad på den maskin den råkade byggas på — mätt: en binär byggd på Arch begärde
# GLIBC_2.44 och startade därför inte ens på Ubuntu 24.04.
#
#   bash packaging/check-glibc-floor.sh dist/sonix 2.28
set -euo pipefail

BIN="${1:?sökväg till binären}"
FLOOR="${2:-2.28}"

worst="$(objdump -T "$BIN" | grep -o 'GLIBC_[0-9.]*' | sed 's/GLIBC_//' | sort -V | tail -1)"
if [[ -z "$worst" ]]; then
    echo "kunde inte läsa GLIBC-krav ur $BIN" >&2
    exit 1
fi
newest="$(printf '%s\n%s\n' "$worst" "$FLOOR" | sort -V | tail -1)"
echo "binären begär GLIBC_$worst, golvet är $FLOOR"
if [[ "$newest" != "$FLOOR" ]]; then
    echo "FEL: binären kräver glibc $worst — den startar inte på äldre distrar (Debian 12 = 2.36, RHEL 8 = 2.28)." >&2
    exit 1
fi
echo "OK: högst GLIBC_$worst"
