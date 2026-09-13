#!/usr/bin/env python3
"""Genererar SECTIONS.md — kartan över Sonix' delar.

Alex 2026-09-13: "Går det att köra en 'sektions hantering' av Sonix? Att en fil
meddelar vilka delar som gör vad, vilka delar som är klara och inte behöver röras,
vad som ännu behöver byggas, så slipper du läsa genom hela koden varje gång?"

Principen: **det som kan bli inaktuellt räknas fram, det som kräver omdöme skrivs
av en människa — och varje faktum har EN ägare.**

- Radantal, publika ingångar, anropare och tester: räknas ur koden här. Aldrig handskrivet.
- Status och "rör inte"-regeln: ägs av **modulen själv**, i dess `//!`-dokumentation:

      //! Status: fryst — 6.7:s fullständighetsvakt, rör bara vid projektformatet.
      //! Rör inte: anropas av elva ställen; ändra i så fall alla.

  En modul utan `//! Status:` listas som "saknar status" i slutet i stället för att
  gissas — kartan får inte ljuga om sin egen täckning.

Kör:  python3 tools/sections.py          (skriver SECTIONS.md)
      python3 tools/sections.py --check  (avslutar 1 om SECTIONS.md är inaktuell)
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC = ROOT / "src"
OUT = ROOT / "SECTIONS.md"

PUB_RE = re.compile(r"^\s*pub(?:\(crate\))?\s+(?:async\s+)?(?:fn|struct|enum|trait|const)\s+([A-Za-z_][A-Za-z0-9_]*)")
TEST_RE = re.compile(r"^\s*fn\s+([a-z0-9_]+)\s*\(")
IGNORE_RE = re.compile(r"^\s*#\[ignore")


def module_doc(text: str) -> list[str]:
    """Modulens `//!`-rader (utan markören), till och med första tomma icke-doc-raden."""
    rows: list[str] = []
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("//!"):
            rows.append(s[3:].strip())
        elif rows:
            break
    return rows


def status_of(doc: list[str]) -> tuple[str, str]:
    """(status, rör inte) ur modulens dokumentation. Tomma strängar när de saknas."""
    status = ""
    keep = ""
    for line in doc:
        low = line.lower()
        if low.startswith("status:"):
            status = line.split(":", 1)[1].strip()
        elif low.startswith(("rör inte:", "ror inte:")):
            keep = line.split(":", 1)[1].strip()
    return status, keep


def main() -> int:
    files = sorted(SRC.rglob("*.rs"))
    texts = {f: f.read_text(encoding="utf-8", errors="replace") for f in files}
    all_text = "\n".join(texts.values())
    rows = []
    missing = []
    for f in files:
        text = texts[f]
        rel = f.relative_to(ROOT).as_posix()
        lines = text.count("\n") + 1
        doc = module_doc(text)
        status, keep = status_of(doc)
        pubs = [m.group(1) for m in (PUB_RE.match(l) for l in text.splitlines()) if m]
        # Anropare: antalet ställen utanför filen som nämner modulens namn.
        stem = f.stem
        callers = max(0, all_text.count(f"{stem}::") - text.count(f"{stem}::"))
        tests = 0
        ignored = 0
        prev_ignore = False
        for line in text.splitlines():
            if IGNORE_RE.match(line):
                prev_ignore = True
            elif TEST_RE.match(line):
                if "#[test]" in text or True:
                    tests += 1
                    if prev_ignore:
                        ignored += 1
                    prev_ignore = False
        # Bara tester som faktiskt står under #[test] räknas (ovan är en räknare över
        # testfunktioner i filen; `#[test]`-attributet kan stå flera rader upp).
        tests = len(re.findall(r"#\[test\]", text))
        ignored = len(re.findall(r"#\[ignore\]", text))
        body = [
            l
            for l in doc
            if not l.lower().startswith(("status:", "rör inte:", "ror inte:"))
        ]
        summary = " ".join(body[:2])[:200]
        rows.append(
            {
                "rel": rel,
                "lines": lines,
                "status": status,
                "keep": keep,
                "pubs": pubs,
                "callers": callers,
                "tests": tests,
                "ignored": ignored,
                "summary": summary,
            }
        )
        if not status:
            missing.append(rel)

    total_lines = sum(r["lines"] for r in rows)
    with_status = [r for r in rows if r["status"]]

    out: list[str] = []
    out.append("# SECTIONS.md — vad varje del av Sonix gör, och om den får röras\n")
    out.append(
        "*Genererad av `tools/sections.py` — kör om den efter varje ändring: "
        "`python3 tools/sections.py`. Alla siffror är räknade ur koden; statusen ägs av "
        "modulens egen `//! Status:`-rad. Redigera inte det här dokumentet för hand.*\n"
    )
    out.append(
        f"**{len(rows)} moduler · {total_lines} rader kod · {len(with_status)} med status "
        f"· {len(missing)} utan.**\n"
    )
    out.append("## Så läser du kartan\n")
    out.append(
        "1. Leta upp modulen i tabellen. **Fryst/klart + många anropare = rör den inte** "
        "utan att läsa `Rör inte`-raden.\n"
        "2. `Anropare` är hur många ställen utanför filen som nämner modulen — ett mått på "
        "hur dyrt ett misstag är, inte på hur viktig den är.\n"
        "3. `Tester` är antalet `#[test]` i filen (`ign` = `#[ignore]`-körningar, som ofta "
        "är mätningar mot riktiga filer).\n"
        "4. Statusen kommer från modulens egen dokumentation — **en ägare per faktum**. "
        "Saknas den står modulen i listan sist.\n"
    )
    out.append("## Modulerna\n")
    out.append("| Modul | Rader | Status | Anropare | Tester | Vad den gör |")
    out.append("| :--- | ---: | :--- | ---: | :--- | :--- |")
    for r in sorted(rows, key=lambda r: (-r["lines"])):
        status = r["status"] or "—"
        tests = f"{r['tests']}" + (f" (+{r['ignored']} ign)" if r["ignored"] else "")
        out.append(
            f"| `{r['rel']}` | {r['lines']} | {status} | {r['callers']} | {tests} | {r['summary']} |"
        )
    out.append("")
    out.append("## Rör inte-regler och publika ingångar\n")
    for r in sorted(rows, key=lambda r: r["rel"]):
        if not (r["keep"] or r["pubs"]):
            continue
        out.append(f"### `{r['rel']}`")
        if r["status"]:
            out.append(f"- **Status:** {r['status']}")
        if r["keep"]:
            out.append(f"- **Rör inte:** {r['keep']}")
        if r["pubs"]:
            shown = ", ".join(f"`{p}`" for p in r["pubs"][:12])
            more = f" … (+{len(r['pubs']) - 12})" if len(r["pubs"]) > 12 else ""
            out.append(f"- **Publika ingångar:** {shown}{more}")
        out.append("")
    out.append("## Moduler utan status\n")
    if missing:
        out.append(
            "De här är inte klassade än. Lägg till en `//! Status:`-rad i modulen "
            "(och `//! Rör inte:` om den har en sådan regel) — kör sedan om skriptet.\n"
        )
        for rel in missing:
            out.append(f"- `{rel}`")
    else:
        out.append("Inga — varje modul säger själv vad den är.\n")
    out.append("")

    text = "\n".join(out)
    if "--check" in sys.argv:
        current = OUT.read_text(encoding="utf-8") if OUT.exists() else ""
        if current != text:
            print("SECTIONS.md är inaktuell — kör: python3 tools/sections.py")
            return 1
        print("SECTIONS.md är aktuell.")
        return 0
    OUT.write_text(text, encoding="utf-8")
    print(f"skrev {OUT.relative_to(ROOT)}: {len(rows)} moduler, {len(missing)} utan status")
    return 0


if __name__ == "__main__":
    sys.exit(main())
