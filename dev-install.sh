#!/usr/bin/env bash
# Bygger om den binär som startmenyn kör.
#
# `~/.local/share/applications/sonix.desktop` kör `~/.local/bin/sonix`, som är en länk till
# `target/release/sonix`. Alltså: menyn visar alltid senaste *bygget* — men bara om release
# faktiskt byggdes. Ett `cargo build` (debug) eller `cargo test` rör inte den binären, och
# 2026-11-05 fick Alex en version från 19:12 att testa en fix som gjorts 22:0x.
#
# Kör det här innan någon ombeds prova något i menyn.
set -euo pipefail
cd "$(dirname "$0")"
cargo build --release 2>&1 | grep -E '^(error|warning)' -A 6 && exit 1 || true
printf 'menyns binär: %s  (%s)\n' \
    "$(readlink -f "$HOME/.local/bin/sonix")" \
    "$(date -r target/release/sonix '+%Y-%m-%d %H:%M')"
