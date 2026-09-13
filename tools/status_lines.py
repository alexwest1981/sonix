#!/usr/bin/env python3
"""Lägger en `//! Status:`-rad i modulens dokumentation (kartan i SECTIONS.md läser den).

Körs en gång: varje modul äger sin egen status. Skriptet rör bara filer som saknar raden.
"""
from pathlib import Path

ROOT = Path("/home/alex/Projects/sonix/src")

STATUS = {
    "ui/app.rs": (
        "byggs — den stora ytan (UI + tillstånd). Kvar här: 8.7:s slice-UI, "
        "8.10-vyns sträckt-märke, 8.4-samplern, 8.2:s fyra visningsställen",
        "två sessioner har krockat i den här filen; kolla `git status` och mtime före varje skrivning",
    ),
    "audio/stretch.rs": (
        "byggs (8.10 steg 2) — motorn, valideringen och beslutet är klara och testade. "
        "Kvar: klipp över ett tempobyte, cachen har ingen utrensning, vyn visar inte att klippet är sträckt",
        "`check_rendered` är 8.5-regeln i siffror — en rendering som inte godkänns får aldrig spelas",
    ),
    "audio/tempo.rs": (
        "stabil — tempokartan och `region_source_secs`",
        "bit-exakt vid faktor 1,0; ändra bara med ett test som visar samma sak",
    ),
    "audio/vocal_harmonizer.rs": (
        "stabil (motorn) — WSOLA, formantbevarande skift och sångstudiens röster",
        "`Wsola` är sträckningens motor; anslag kommer in via `set_onsets`, motorn gissar dem aldrig",
    ),
    "audio/onset.rs": (
        "stabil — slagletning och slicekarta (8.7 steg 1 + 2)",
        "mät på en jämn ton först: en detektor är en mätning, och 4 ms enpolsfilter + centrerad tröskel är den enda variant som ger noll falska slag",
    ),
    "audio/synth.rs": (
        "stabil — motorn: mixer, bussar, sidokedjor (8.3) och sends",
        "sends mellan spår kräver att spårloopen i `process_stereo` delas i två faser + slingkontroll",
    ),
    "audio/exporter.rs": (
        "stabil — offline-rendering och export",
        "exporten måste ge **samma ljud som högtalarna**; samma väg som uppspelningen, inte en parallell",
    ),
    "audio/waveform.rs": (
        "stabil — exakt hölje och flernivåcache (8.3)",
        "representationen ska bytas vid zoomtrösklar, aldrig göras finare kolumn för kolumn",
    ),
    "paths.rs": (
        "fryst — enda modulen som får bygga sökvägar",
        "nya sökvägar läggs här, aldrig hos anroparen",
    ),
    "i18n.rs": (
        "fryst — nycklar på engelska, texter på svenska",
        "nya strängar går via `t()`/`tstatus!`, aldrig som literaler i UI:t",
    ),
    "audio/smf.rs": (
        "stabil — MIDI-export med tempobyten (8.2 steg 3)",
        "gyllene test: en orörd fil ska vara byte-identisk",
    ),
    "audio/stem_separator.rs": (
        "stabil (8.5a) — separatorn skriver stämmorna till disk och minns var de ligger",
        "8.5-regeln: en väg som skapar ett klipp eller en fil får aldrig hitta på ljud — säg fel och avbryt",
    ),
    "audio/realtime_bench.rs": (
        "stabil — realtidsmätningen",
        "release tillåter högst 2 missade block av 120; en spik är ett fel, inte brus",
    ),
    "audio/plugin_host_live.rs": (
        "stabil (4.x) — CLAP-värden i egen process",
        "en plugin i uppspelningsvägen får aldrig kunna tysta eller krascha motorn",
    ),
    "audio/plugin_vst3.rs": (
        "stabil (4.6a) — VST3-värden, en utbuss",
        "",
    ),
    "audio/recorder.rs": (
        "stabil — inspelning",
        "",
    ),
    "audio/factory_samples.rs": (
        "stabil — fabriksbiblioteket och `merge_library`",
        "",
    ),
    "audio/scale.rs": (
        "stabil (8.11) — en tabell och ett index för tonarten",
        "en tabell, ett index, en test som räknar namnen — två listor för samma sak driver isär",
    ),
    "audio/command.rs": (
        "byggs — kommando-protokollet (AI-vägen)",
        "nya kommandon ska gå genom `Command` så att agenten och UI:t gör samma sak",
    ),
}

changed = []
for rel, (status, keep) in STATUS.items():
    p = ROOT / rel
    if not p.exists():
        print(f"  ✗ {rel} finns inte")
        continue
    lines = p.read_text(encoding="utf-8").splitlines(keepends=True)
    if any(l.strip().lower().startswith("//! status:") for l in lines[:40]):
        print(f"  – {rel} har redan status")
        continue
    # Sista raden i den inledande //!-blocket (eller rad 0 om filen inte börjar med //!)
    last = -1
    for i, l in enumerate(lines):
        if l.lstrip().startswith("//!"):
            last = i
        elif last >= 0:
            break
    insert_at = last + 1 if last >= 0 else 0
    block = [f"//! Status: {status}\n"]
    if keep:
        block.append(f"//! Rör inte: {keep}\n")
    lines[insert_at:insert_at] = block
    p.write_text("".join(lines), encoding="utf-8")
    changed.append(rel)

print(f"status satt i {len(changed)} moduler")
