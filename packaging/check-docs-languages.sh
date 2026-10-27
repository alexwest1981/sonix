#!/usr/bin/env bash
# **Grinden för flerspråkigheten.** Appen finns på sju språk (en + sex), och då ska repot och
# skrivbordsposten göra det också — annars glider de isär vid nästa språk och ingen märker det.
#
#   bash packaging/check-docs-languages.sh
#
# Kontrollerar tre ytor: README-filerna, språkraden i varje README, och de lokala raderna i
# .desktop + AppStream-metainfo. Ett nytt språk läggs till på alla tre ställena eller inte alls.
set -uo pipefail
cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

DESKTOP=packaging/io.github.alexwest1981.Sonix.desktop
METAINFO=packaging/io.github.alexwest1981.Sonix.metainfo.xml
FAIL=0

# kod:fil — `en` är standard-README:n, resten är egna filer.
LANGS=("en:README.md" "sv:README_SV.md" "da:README_DA.md" "nb:README_NB.md" "de:README_DE.md" "es:README_ES.md" "fr:README_FR.md")

fail() { echo "FEL: $*" >&2; FAIL=1; }

for entry in "${LANGS[@]}"; do
    code="${entry%%:*}"
    file="${entry##*:}"

    [ -f "$file" ] || { fail "$code: $file saknas"; continue; }

    # Språkraden: varje README ska länka till alla andra, annars är den en återvändsgränd.
    for other in "${LANGS[@]}"; do
        other_file="${other##*:}"
        grep -q "($other_file)" "$file" || fail "$file länkar inte till $other_file"
    done

    # Skrivbordsposten och metainfo behöver bara de sex översättningarna (en är standard).
    if [ "$code" != "en" ]; then
        grep -q "^Comment\[$code\]=" "$DESKTOP" || fail "$DESKTOP saknar Comment[$code]"
        grep -q "^GenericName\[$code\]=" "$DESKTOP" || fail "$DESKTOP saknar GenericName[$code]"
        grep -q "^Keywords\[$code\]=" "$DESKTOP" || fail "$DESKTOP saknar Keywords[$code]"
        grep -q "<summary xml:lang=\"$code\">" "$METAINFO" || fail "$METAINFO saknar summary för $code"
    fi
done

if [ "$FAIL" -eq 0 ]; then
    echo "OK: ${#LANGS[@]} språk i README, .desktop och metainfo"
fi
exit "$FAIL"
