#!/usr/bin/env python3
"""i18n-audit för Sonix: hittar nycklar som saknas i någon av de sex tabellerna
och literaler i UI-koden som aldrig går genom t().

Svenska är nyckelspråket: `t("…")` slår upp samma sträng i en/da/no/de/es/fr.
Kör från repo-roten:  python3 scripts/i18n_audit.py
"""
import re
import sys
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
LANGS = ["en", "da", "no", "de", "es", "fr"]

# --- lexer: skilj strängar från kommentarer, hantera "…", r#"…"#, '"' ---------
RAW = re.compile(r'(?:br|r)(#*)"')


def skip(src, i):
    """Om i börjar en sträng/kommentar: returnera dess slut, annars None."""
    m = RAW.match(src, i)
    if m:
        end = src.find('"' + "#" * len(m.group(1)), m.end())
        return len(src) if end == -1 else end + 1 + len(m.group(1))
    if src.startswith("//", i):
        j = src.find("\n", i)
        return len(src) if j == -1 else j
    if src.startswith("/*", i):
        j = src.find("*/", i + 2)
        return len(src) if j == -1 else j + 2
    if src[i] == "'" and i + 2 < len(src):          # '"' och '\'' är tecken, inte strängar
        if src[i + 1] == "\\":
            j = i + 2
            while j < len(src) and src[j] != "'":
                j += 1
            return j + 1
        if src[i + 2] == "'":
            return i + 3
    if src[i] == '"':
        j = i + 1
        while j < len(src):
            if src[j] == "\\":
                j += 2
                continue
            if src[j] == '"':
                return j + 1
            j += 1
        return len(src)
    return None


def blank_comments(src):
    out, i = [], 0
    while i < len(src):
        e = skip(src, i)
        if e is None:
            out.append(src[i])
        elif src.startswith(("//", "/*"), i):
            out.append("".join("\n" if c == "\n" else " " for c in src[i:e]))
        else:
            out.append(src[i:e])
        i = e if e is not None else i + 1
    return "".join(out)


def literals(src):
    """(start, slut, innehåll, text före) för varje strängliteral."""
    i = 0
    while i < len(src):
        m = RAW.match(src, i)
        if m or src[i] == '"':
            e = skip(src, i)
            body = src[m.end():e - 1 - len(m.group(1))] if m else src[i + 1:e - 1]
            yield i, e, body, src[max(0, i - 300):i]
            i = e
        else:
            i += 1


UNESC = {"n": "\n", "t": "\t", "r": "\r", "0": "\0", "\\": "\\", '"': '"', "'": "'"}


def unescape(s):
    def rep(m):
        g = m.group(1)
        if g.startswith("u{"):
            return chr(int(g[2:-1], 16))
        return UNESC.get(g, g)
    return re.sub(r"\\(u\{[0-9a-fA-F]+\}|.)", rep, s, flags=re.S)


# --- tabellerna ------------------------------------------------------------
def tables():
    src = (ROOT / "src/i18n.rs").read_text()
    pair = re.compile(
        r'"((?:[^"\\]|\\.)*)"\s*=>\s*(?:\{\s*)?"((?:[^"\\]|\\.)*)"\s*\}?\s*,?', re.S
    )
    out = {}
    for lang in LANGS:
        start = src.find(f"fn tr_{lang}(key: &str) -> Option<&'static str>")
        end = src.find("_ => return None,", start)
        if start == -1 or end == -1:
            sys.exit(f"hittade inte tabellen tr_{lang}")
        out[lang] = {unescape(k): unescape(v) for k, v in pair.findall(src[start:end])}
    return out


def missing_in_tables(tbl):
    keys = set().union(*(set(t) for t in tbl.values()))
    return {lang: sorted(keys - set(tbl[lang])) for lang in LANGS}


# --- nycklar som skickas till t()/tr() -------------------------------------
CALL = re.compile(r'(?:\b(?:t|tr|translate|translate_owned)\s*!?\s*\(\s*|\.tr\s*\(\s*)"((?:[^"\\]|\\.)*)"')


def call_keys():
    used = {}
    for p in sorted((ROOT / "src").rglob("*.rs")):
        if p.name == "i18n.rs":
            continue
        src = blank_comments(p.read_text())
        for m in CALL.finditer(src):
            key = unescape(m.group(1))
            if any(c.isalpha() for c in key):
                used.setdefault(key, str(p.relative_to(ROOT)))
    return used


# --- literaler i UI-anrop --------------------------------------------------
WIDGETS = {
    "label", "button", "heading", "small_button", "checkbox", "radio_value", "selectable_label",
    "selectable_value", "menu_button", "on_hover_text", "on_hover_ui", "colored_label", "strong",
    "monospace", "text_edit_singleline", "text_edit_multiline", "toggle_value", "hyperlink",
    "collapsing", "hint_text", "text", "subheading",
}
CALLEE = re.compile(r"([A-Za-z_][A-Za-z0-9_]*)\s*!?\s*\(\s*$")
I18N_WRAP = re.compile(r"(?:\b(?:t|tr|translate|translate_owned|tstatus)\s*!?\s*\(\s*|\.tr\s*\(\s*)$")


def hardcoded():
    rows = []
    for p in sorted((ROOT / "src/ui").rglob("*.rs")):
        if p.name == "tests.rs":
            continue
        raw = p.read_text()
        cut = raw.find("#[cfg(test)]")           # testmoduler bär inte UI-text
        if cut != -1:
            raw = raw[:cut]
        src = blank_comments(raw)
        for start, _, body, pre in literals(src):
            m = CALLEE.search(pre)
            if not body.strip() or not m or m.group(1) not in WIDGETS or I18N_WRAP.search(pre):
                continue
            rows.append((str(p.relative_to(ROOT)), src.count("\n", 0, start) + 1, m.group(1), body))
    return rows


def main():
    tbl = tables()
    print("== Tabeller ==")
    for lang in LANGS:
        print(f"  {lang}: {len(tbl[lang])} nycklar")

    print("\n== Saknade översättningar (nyckel finns i en tabell men inte i alla) ==")
    miss = missing_in_tables(tbl)
    total = 0
    for lang, keys in miss.items():
        total += len(keys)
        if keys:
            print(f"  {lang}: {len(keys)} saknas")
            for k in keys:
                print(f"    {k!r}")
        else:
            print(f"  {lang}: 0 saknas")
    print(f"  summa: {total}")

    print("\n== Nycklar i t()/tr() som ingen tabell har ==")
    used = call_keys()
    untranslated = [k for k in used if not any(k in tbl[lang] for lang in LANGS)]
    for k in sorted(untranslated):
        print(f"  {used[k]}: {k!r}")
    print(f"  summa: {len(untranslated)} (av {len(used)} nycklar i t()/tr())")

    print("\n== Hårdkodade literaler i UI-anrop (inte via t()) ==")
    rows = hardcoded()
    for f, line, callee, lit in rows:
        print(f"  {f}:{line}\t{callee}\t{lit!r}")
    print(f"  summa: {len(rows)}")

    # Självkontroll: fångar en trasig tabell-regex eller ett tomt källträd.
    assert all(len(t) > 2000 for t in tbl.values()), "tabellerna ser avhuggna ut"
    assert len(used) > 1000, "hittade nästan inga t()-nycklar"


if __name__ == "__main__":
    main()
