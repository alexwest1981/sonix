#!/usr/bin/env python3
"""Flyttar hela item från en stor modulfil till en ny — mekaniskt och kontrollerat.

Skrivet för delningen av `src/ui/app.rs` 2026-09-14 (23 119 rader → en rot på 2 112
och sexton undermoduler). Verktyget finns kvar därför att nästa fil är samma problem:
`update` är 1 154 rader och `render_playlist_arranger` 2 580 — och en sådan flytt ska
inte göras för hand.

    python3 tools/module_split.py <målfil> method|item|impl|mod [--from FIL] [--header TEXT]
                                  [--unwrap] namn...

- `method`: en metod inne i ett `impl`-block (indrag 4) → skrivs in i ett eget
  `impl <typ> { … }` i målfilen, avindenterad 4 steg.
- `item`: ett toppnivå-item (indrag 0, även `impl X {`) → skrivs rakt av.
- `impl`: matchar hela rubriken, t.ex. `Default for TrackEq` (annars delar två impls namn).
- `mod`: en `mod x { … }`-modul; med `--unwrap` skalas omslaget av (för en `tests.rs`).
- `namn@2`: välj den andra förekomsten, när två item heter exakt likadant.

Tre kontroller, alla **före** något skrivs (ett tyst fel här är dyrare än ett avbrott):

1. Varje namn måste finnas exakt en gång (eller nr `@n`).
2. Varje utbrutet block måste vara internt **klammerbalanserat**. Balansen räknas på
   hela blocket, inte rad för rad: en strängliteral som fortsätter över en radbrytning
   ser ut som kod om man tittar på raden ensam, och det felet klippte en metod en rad
   för tidigt (mätt: `render_effects_mixer_rack`, 2026-09-14).
3. Ingen icke-glue-rad får tappas eller dubbleras — en multiset-jämförelse av källfilen
   före mot (källfilen + målfilen) efter. En flytt ska vara en flytt.

Rör ingenting om en kontroll faller: filerna skrivs först när alla tre är gröna.
"""
from __future__ import annotations

import re
import sys
from collections import Counter
from pathlib import Path

# Radtyper som flytten själv lägger till eller tar bort: rubriker, mod-rader,
# re-exports och omslaget kring de utbrutna metoderna.
GLUE = re.compile(
    r"^(use super::\*;|//!.*|mod [a-z_]+;|mod [a-z_]+ \{|pub(\(crate\))? use [a-z_]+::\*;|"
    r"impl [A-Za-z0-9_]+ \{|\}|#\[cfg\(test\)\])$"
)


def blank(s: str) -> str:
    """Samma längd, men bara radbrytningar kvar — radstrukturen får inte ändras."""
    return "".join("\n" if ch == "\n" else " " for ch in s)


def mask(text: str) -> str:
    """Samma längd som `text`, men kommentarer och strängar utbytta mot mellanslag.

    Klammerräkningen får inte luras av `{` inuti en sträng, en råsträng över flera rader
    eller en kommentar. Radbrytningarna är kvar, så antalet rader är oförändrat.
    """
    out = []
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            while i < n and text[i] != "\n":
                out.append(" ")
                i += 1
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            j = text.find("*/", i + 2)
            j = n if j < 0 else j + 2
            out.append(blank(text[i:j]))
            i = j
            continue
        m = re.match(r'r(#*)"', text[i:]) if c == "r" else None
        if m:
            close = '"' + m.group(1)
            j = text.find(close, i + len(m.group(0)))
            j = n if j < 0 else j + len(close)
            out.append(blank(text[i:j]))
            i = j
            continue
        if c == '"':
            # En strängliteral får innehålla en rå radbrytning (och `\` + radbrytning).
            # Den slutar bara på ett oescapet `"` — att sluta vid radbrytningen tappar
            # en riktig `{` längre ned i filen.
            j = i + 1
            while j < n:
                if text[j] == "\\":
                    j += 2
                    continue
                if text[j] == '"':
                    j += 1
                    break
                j += 1
            out.append(blank(text[i:j]))
            i = j
            continue
        if c == "'":
            m2 = re.match(r"'(\\\\.|[^\\\\'])'", text[i:], re.S)
            if m2:
                out.append(" " * len(m2.group(0)))
                i += len(m2.group(0))
                continue
            out.append(" ")
            i += 1
            continue
        out.append(c)
        i += 1
    return "".join(out)


def extent(masked: list[str], start: int) -> int:
    """Sista raden (0-indexerad) för det item som börjar på `start`."""
    depth = 0
    opened = False
    for j in range(start, len(masked)):
        line = masked[j]
        depth += line.count("{") - line.count("}")
        if "{" in line:
            opened = True
        if opened and depth <= 0:
            return j  # blocket öppnades och stängdes (även en enrads-funktion)
        if not opened and depth <= 0 and line.rstrip().endswith(";"):
            return j  # item utan klammerpar, t.ex. en `static`
    raise SystemExit(f"kunde inte hitta slutet på item som börjar rad {start + 1}")


def attached_start(lines: list[str], sig: int) -> int:
    """Dokumentations- och attributrader som hör till itemet på rad `sig`."""
    k = sig - 1
    while k >= 0:
        s = lines[k].strip()
        if s.startswith(("///", "#[", "//!", "#!")):
            k -= 1
            continue
        break
    return k + 1


def find(lines: list[str], masked: list[str], name: str, kind: str, path: Path) -> tuple[int, int]:
    nth = 1
    nth_given = False
    if "@" in name:
        name, _, idx = name.partition("@")
        nth = int(idx)
        nth_given = True
    if kind == "method":
        pat = re.compile(rf"^    (?:pub(?:\(crate\))? )?(?:const )?fn {re.escape(name)}\b")
    elif kind == "impl":
        # Hela rubriken måste stämma: `impl AutomationLane {` och `impl Default for TrackEq {`
        # delar ord, så ett löst namn vore tvetydigt.
        pat = re.compile(rf"^impl {re.escape(name)} \{{")
    else:
        pat = re.compile(
            rf"^(?:pub(?:\(crate\))? )?(?:async )?(?:struct|enum|fn|mod|const|static|trait|type) "
            rf"{re.escape(name)}\b"
        )
    hits = [i for i, l in enumerate(masked) if pat.match(l)]
    if nth_given:
        if len(hits) < nth:
            raise SystemExit(f"'{name}' ({kind}) gav {len(hits)} träffar i {path} — begärde nr {nth}")
    elif len(hits) != 1:
        raise SystemExit(f"'{name}' ({kind}) gav {len(hits)} träffar i {path} — kräver exakt 1")
    sig = hits[nth - 1]
    return attached_start(lines, sig), extent(masked, sig)


def check_nothing_lost(before: list[str], after_text: str, targets: list[str]) -> None:
    """Varje icke-glue-rad i originalet ska finnas kvar exakt lika många gånger."""
    after: list[str] = list(after_text.splitlines())
    for t in targets:
        after += t.splitlines()
    orig = Counter(l.strip() for l in before if l.strip() and not GLUE.match(l.strip()))
    now = Counter(l.strip() for l in after if l.strip() and not GLUE.match(l.strip()))
    lost = orig - now
    extra = now - orig
    if lost:
        print(f"  ✗ TAPPADE rader ({sum(lost.values())}):")
        for l, c in list(lost.items())[:10]:
            print(f"      {c}× {l[:100]}")
        raise SystemExit("avbryter: text har försvunnit — inget skrivet")
    if extra:
        print(f"  ! nya rader utanför glue ({sum(extra.values())}) — väntat när målfilen redan fanns")
    print(f"  ✓ ingen rad tappad ({sum(orig.values())} rader före, {sum(now.values())} efter)")


def main() -> int:
    args = sys.argv[1:]
    if len(args) < 3:
        raise SystemExit(__doc__)
    target = Path(args[0])
    kind = args[1]
    rest = args[2:]
    source = Path("src/ui/app.rs")
    header = None
    unwrap = False
    names: list[str] = []
    i = 0
    while i < len(rest):
        if rest[i] == "--from":
            source = Path(rest[i + 1])
            i += 2
        elif rest[i] == "--header":
            header = rest[i + 1]
            i += 2
        elif rest[i] == "--unwrap":
            unwrap = True
            i += 1
        else:
            names.append(rest[i])
            i += 1

    text = source.read_text(encoding="utf-8")
    lines = text.splitlines()
    masked = mask(text).splitlines()
    assert len(lines) == len(masked), "masken ändrade radantalet"

    before = list(lines)
    picked = []
    for name in names:
        s, e = find(lines, masked, name, kind, source)
        picked.append((s, e, name))
    picked.sort()

    blocks = []
    for s, e, name in picked:
        block = lines[s : e + 1]
        if unwrap:
            idx = next((i for i, l in enumerate(block) if l.strip().startswith("mod ")), None)
            if idx is None:
                raise SystemExit(f"{name}: hittade ingen `mod`-rad att skala av")
            inner = block[idx + 1 :]
            if not inner or inner[-1].strip() != "}":
                raise SystemExit(f"{name}: omslaget slutar inte med `}}` — vägrar gissa")
            block = inner[:-1]
        if kind == "method":
            block = [(l[4:] if l.startswith("    ") else l) for l in block]
        blocks.append((block, name))

    # --- räkna fram båda filernas nya innehåll ---
    if target.exists():
        body = target.read_text(encoding="utf-8").rstrip("\n").splitlines() + [""]
    else:
        body = ([header.rstrip("\n"), ""] if header else []) + ["use super::*;", ""]
    for block, name in blocks:
        if kind == "method":
            body.append("impl SonixApp {")
            body += block
            body.append("}")
        else:
            body += block
        body.append("")
    target_after = "\n".join(body) + "\n"

    for s, e, name in sorted(picked, reverse=True):
        del lines[s : e + 1]
        while s < len(lines) and lines[s].strip() == "" and s > 0 and lines[s - 1].strip() == "":
            del lines[s]
    source_after = "\n".join(lines) + "\n"

    # --- kontrollera INNAN något skrivs ---
    for block, name in blocks:
        hela = mask("\n".join(block))
        bal = hela.count("{") - hela.count("}")
        if bal != 0:
            raise SystemExit(
                f"✗ '{name}': det utbrutna blocket är obalanserat ({bal:+d} klammer) — "
                "räckvidden blev fel. Inget skrivet."
            )
    check_nothing_lost(before, source_after, [target_after])

    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(target_after, encoding="utf-8")
    source.write_text(source_after, encoding="utf-8")
    print(f"{target}: {len(names)} item flyttade ({kind}); {source} {len(before)} → {len(lines)} rader")
    print(
        "  kom ihåg: `mod {0};` (+ `pub use {0}::*;` om någon utanför modulen nämner ett item)\n"
        "           i föräldern, och `pub(crate)` på det en syskonmodul läser — kompilatorn\n"
        "           räknar upp varje ställe som saknar det.".format(target.stem)
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
