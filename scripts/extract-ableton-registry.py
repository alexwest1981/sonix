#!/usr/bin/env python3
"""Extract Ableton Live's processor registry from the PE binary.

Every processor is registered by the same three-instruction sequence:

    lea rdx, [rip+disp32]   ; RTTI type descriptor for the class
    lea rcx, [rip+disp32]   ; the name as a C string
    call  <factory>

so one pass over .text yields (name, type_info address, factory address) for every device,
without disassembling anything. The output is the map in docs/ABLETON-DSP-MAP.md.

Usage:  python3 scripts/extract-ableton-registry.py <path to Ableton Live Suite.exe> [out.tsv]
Requires an unpacked installation (innoextract) - this script reads the file, it does not
modify it.
"""
import struct

import sys
EXE = sys.argv[1] if len(sys.argv) > 1 else "Ableton Live 12 Suite.exe"
OUT = sys.argv[2] if len(sys.argv) > 2 else "ableton_registry.tsv"
IMAGE_BASE = 0x140000000

def sections(path):
    with open(path, "rb") as f:
        f.seek(0x3C)
        pe = struct.unpack("<I", f.read(4))[0]
        f.seek(pe + 6)
        nsec = struct.unpack("<H", f.read(2))[0]
        f.seek(pe + 20)
        opt_size = struct.unpack("<H", f.read(2))[0]
        f.seek(pe + 24 + opt_size)
        out = []
        for _ in range(nsec):
            raw = f.read(40)
            name = raw[:8].rstrip(b"\0").decode(errors="replace")
            vsize, vma, rsize, roff = struct.unpack("<IIII", raw[8:24])
            out.append((name, vma, vsize, roff, rsize))
        return out

data = open(EXE, "rb").read()
secs = sections(EXE)

def va_to_off(va):
    rva = va - IMAGE_BASE
    for name, vma, vsize, roff, rsize in secs:
        if vma <= rva < vma + rsize:
            return roff + (rva - vma)
    return None

def cstr(va, limit=48):
    off = va_to_off(va)
    if off is None:
        return None
    end = data.find(b"\0", off, off + limit)
    if end < 0:
        return None
    raw = data[off:end]
    if not raw or len(raw) < 3:
        return None
    try:
        text = raw.decode("ascii")
    except UnicodeDecodeError:
        return None
    if not all(32 <= ord(c) < 127 for c in text):
        return None
    return text

text = next(s for s in secs if s[0] == ".text")
_, text_rva, text_size, text_off, _ = text
blob = data[text_off:text_off + text_size]
base = IMAGE_BASE + text_rva

entries = []
n = len(blob)
for i in range(n - 22):
    # Registreringen är `lea rdx,<typ>` FÖRE `lea rcx,<namn>`, sedan `call <fabrik>`
    # (sett i utskriften kring 0x1434e07e0), inte tvärtom.
    if blob[i] == 0x48 and blob[i + 1] == 0x8D and blob[i + 2] == 0x15:
        disp = struct.unpack_from("<i", blob, i + 3)[0]
        type_va = base + i + 7 + disp
        j = i + 7
        found = False
        while j < i + 21:
            if blob[j] == 0x48 and blob[j + 1] == 0x8D and blob[j + 2] == 0x0D:
                disp2 = struct.unpack_from("<i", blob, j + 3)[0]
                name_va = base + j + 7 + disp2
                name = cstr(name_va)
                if not name:
                    break
                # call rel32 right after
                k = j + 7
                if blob[k] == 0xE8:
                    call_va = base + k + 5 + struct.unpack_from("<i", blob, k + 1)[0]
                    entries.append((name, type_va, call_va, base + i))
                    found = True
                break
            j += 1
        if not found:
            continue

print(f"registry entries: {len(entries)}")
for name, type_va, call_va, at in entries[:40]:
    print(f"  {name:<32} type=0x{type_va:x} factory=0x{call_va:x} at=0x{at:x}")

with open(OUT, "w") as out:
    for name, type_va, call_va, at in entries:
        out.write(f"{name}\t0x{type_va:x}\t0x{call_va:x}\t0x{at:x}\n")
print(f"written to {OUT}")
