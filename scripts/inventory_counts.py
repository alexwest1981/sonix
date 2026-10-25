#!/usr/bin/env python3
"""Räkna statusraden i gap-inventarierna. Körs före varje commit som rör en tabell.

    python3 scripts/inventory_counts.py            # visa
    python3 scripts/inventory_counts.py --write    # skriv om "Status mot Sonix i dag"-raden

Bara plugin-tabellen räknas (avsnittet "De ... pluggarna"); kommandoytan och filändelserna
är egna tabeller med egna frågor. Summan skrivs aldrig för hand — den drev en gång en rad bakom.
"""
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MARKS = ("✅", "◐", "✗", "—")


def counts(text: str, section: str) -> dict[str, int]:
    tally = dict.fromkeys(MARKS, 0)
    inside = False
    for line in text.splitlines():
        # Bara "## "-rubriken byter avsnitt; underrubriker ligger inuti.
        if line.startswith("## "):
            inside = section in line
        cells = [c.strip() for c in line.strip().strip("|").split("|")]
        # Statusen är cellens första tecken: många rader har noten i samma cell.
        mark = cells[2][:1] if len(cells) >= 3 else ""
        if inside and line.startswith("|") and mark in tally:
            tally[mark] += 1
    return tally


def main() -> int:
    files = sorted(ROOT.glob("*INVENTORY.md"))
    write = "--write" in sys.argv
    wrote = 0
    for path in files:
        text = path.read_text()
        t = counts(text, "pluggarna")
        comparable = t["✅"] + t["◐"] + t["✗"]
        if not comparable:
            print(f"{path.name}: ingen plugin-tabell")
            continue
        pct = round(100 * t["✅"] / comparable)
        print(f"{path.name}: ✅ {t['✅']} · ◐ {t['◐']} · ✗ {t['✗']} · — {t['—']}  ({pct} % av {comparable})")
        # Kommandoytan har ingen statusrad, men den driver lika lätt — visa den varje gång.
        cmd = counts(text, "Kommandoytan")
        if sum(cmd.values()):
            print(f"  kommandoytan: ✅ {cmd['✅']} · ◐ {cmd['◐']} · ✗ {cmd['✗']}")
        if not write:
            continue
        new = (
            f"Status mot Sonix i dag: **✅ {t['✅']}** har en motsvarighet, **◐ {t['◐']}** delvis, "
            f"**✗ {t['✗']}** saknas, **— {t['—']}** ej tillämpliga (**{pct} %** av de {comparable} som går att "
            "jämföra). Siffrorna räknas ur tabellerna, inte ur minnet."
        )
        text, n = re.subn(r"Status mot Sonix i dag:[^\n]*", new, text, count=1)
        if n:
            path.write_text(text)
            wrote += 1
    if write and not wrote:
        print("ingen statusrad att skriva om", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
