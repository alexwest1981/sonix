#!/usr/bin/env bash
#
# Sonix Studio – installerare
#
# Två lägen:
#
#   1) GUIDED INSTALL (standard) – för nya användare.
#      Varje steg bekräftas först, precis som i en installerare:
#
#          bash install.sh
#
#   2) REFRESH – bygger om och uppdaterar binär + startmenypost.
#      Används efter en kodändring så du kan starta direkt från startmenyn:
#
#          bash install.sh --refresh
#
#   Lägg till --yes för att bekräfta alla steg automatiskt (ex. CI / egna scripts):
#
#          bash install.sh --refresh --yes
#
# Beroenden som installeras (per distribution) finns dokumenterade i README.

set -euo pipefail

APP_NAME="Sonix Studio"
APP_EXE="sonix"
REPO_URL="https://github.com/alexwest1981/sonix.git"
ICON_SRC_REL="assets/sonix.png"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

IN_REPO=0
if [[ -f "$SCRIPT_DIR/Cargo.toml" ]]; then
    IN_REPO=1
fi

REFRESH=0
YES=0
for arg in "$@"; do
    case "$arg" in
        --refresh) REFRESH=1 ;;
        --yes) YES=1 ;;
        -h|--help)
            sed -n '2,24p' "${BASH_SOURCE[0]}"
            exit 0
            ;;
        *) echo "Okänt argument: $arg (se --help)" >&2; exit 1 ;;
    esac
done

say()   { printf '\n\033[1m%s\033[0m\n' "$*"; }
info()  { printf '   %s\n' "$*"; }
step()  { printf '\n──────────────────────────────────────────────\n'; printf '◉  %s\n' "$1"; printf '──────────────────────────────────────────────\n'; }

confirm() {
    # $1 = fråga. Returnerar 0 (ja) eller 1 (nej). Enter = nej (säkert val).
    if [[ "$YES" -eq 1 ]]; then
        return 0
    fi
    local answer
    printf '\n\033[1m❓ %s\033[0m  [j/N]: ' "$1"
    read -r answer
    case "$answer" in
        j|J|ja|Ja|y|Y|yes|Yes|YES) return 0 ;;
        *) return 1 ;;
    esac
}

HOME_BIN="${HOME}/.cargo/bin"

detect_pm() {
    if command -v pacman >/dev/null 2>&1; then echo "pacman"
    elif command -v apt-get >/dev/null 2>&1; then echo "apt"
    elif command -v dnf >/dev/null 2>&1; then echo "dnf"
    else echo "unknown"; fi
}

ensure_rust() {
    if command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1; then
        return 0
    fi
    export PATH="${HOME}/.cargo/bin:${PATH}"
    if command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1; then
        return 0
    fi
    if command -v rustup >/dev/null 2>&1; then
        if confirm "Rustup finns – installera verktygskedjan (rustup default stable) nu?"; then
            rustup default stable
            export PATH="${HOME}/.cargo/bin:${PATH}"
        fi
    elif confirm "Installera Rust via rustup (stable) nu?"; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        export PATH="${HOME}/.cargo/bin:${PATH}"
    fi
    if ! command -v cargo >/dev/null 2>&1; then
        echo "   ⚠️  Rust krävs för att bygga. Avbryter." >&2
        exit 1
    fi
}

step_deps() {
    step "Systemberoenden"
    local pm
    pm="$(detect_pm)"
    case "$pm" in
        pacman)
            if confirm "Installera byggverktyg via pacman (base-devel, alsa-lib, zenity, unzip, rustup)?"; then
                sudo pacman -S --needed base-devel alsa-lib zenity unzip rustup
            fi
            ensure_rust
            ;;
        apt)
            if confirm "Installera byggverktyg via apt (build-essential, libasound2-dev, zenity, unzip, curl)?"; then
                sudo apt-get update
                sudo apt-get install -y build-essential libasound2-dev zenity unzip curl
            fi
            ensure_rust
            ;;
        dnf)
            if confirm "Installera byggverktyg via dnf (gcc-c++, alsa-lib-devel, zenity, unzip, curl)?"; then
                sudo dnf install -y gcc-c++ alsa-lib-devel zenity unzip curl
            fi
            ensure_rust
            ;;
        unknown)
            echo "   ⚠️  Okänd distribution – hoppar över beroendeinstallation."
            echo "   Installera Rust + en C-kompilator + ALSA-utvecklingsbibliotek manuellt."
            ensure_rust
            ;;
    esac
    if ! command -v cargo >/dev/null 2>&1; then
        echo "   ⚠️  cargo saknas fortfarande." >&2
        exit 1
    fi
}

step_source() {
    step "Källkod"
    if [[ "$IN_REPO" -eq 1 ]]; then
        info "Körs redan inuti en Sonix-klon: $SCRIPT_DIR"
        return 0
    fi

    local default_dir="${HOME}/Projects/sonix"
    if confirm "Klona Sonix till '${default_dir}'? (skriv 'n' för att hoppa över om du redan har en klon)"; then
        mkdir -p "$(dirname "$default_dir")"
        git clone "$REPO_URL" "$default_dir"
        cd "$default_dir"
        SCRIPT_DIR="$default_dir"
    else
        echo "   ⚠️  Ingen källkod – kan inte bygga. Avbryter." >&2
        exit 1
    fi
}

step_build() {
    step "Kompilera (release-bygge – kan ta några minuter första gången)"
    if confirm "Bygg Sonix nu (cargo build --release)?"; then
        (cd "$SCRIPT_DIR" && cargo build --release)
    fi
    if [[ ! -x "$SCRIPT_DIR/target/release/${APP_EXE}" ]]; then
        echo "   ⚠️  Bygget misslyckades (ingen körbar fil skapades). Avbryter." >&2
        exit 1
    fi
}

step_install_binary() {
    step "Installera binär → ~/.cargo/bin"
    if confirm "Installera '${APP_EXE}' till '${HOME_BIN}'?"; then
        mkdir -p "$HOME_BIN"
        install -m 0755 "$SCRIPT_DIR/target/release/${APP_EXE}" "$HOME_BIN/${APP_EXE}"
        info "Installerad: ${HOME_BIN}/${APP_EXE}"
    fi
}

step_menu() {
    step "Startmenypost (applikationsmenyn)"
    if ! confirm "Skapa menypost '${APP_NAME}' i startmenyn?"; then
        return 0
    fi

    mkdir -p "${HOME}/.local/share/applications"
    mkdir -p "${HOME}/.local/share/icons/hicolor/256x256/apps"

    # Kopiera ikonen så menyposten fungerar även om klonen flyttas/raderas.
    if [[ -f "$SCRIPT_DIR/$ICON_SRC_REL" ]]; then
        install -m 0644 "$SCRIPT_DIR/$ICON_SRC_REL" \
            "${HOME}/.local/share/icons/hicolor/256x256/apps/${APP_EXE}.png"
    fi

    local desktop_file="${HOME}/.local/share/applications/${APP_EXE}.desktop"
    cat > "$desktop_file" <<EOF
[Desktop Entry]
Name=${APP_NAME}
Comment=Native Linux DAW (Rust/egui)
Exec=${HOME_BIN}/${APP_EXE}
Icon=${APP_EXE}
Terminal=false
Type=Application
Categories=Audio;AudioVideo;Music;
StartupWMClass=${APP_EXE}-daw
EOF

    info "Skapad: ${desktop_file}"
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "${HOME}/.local/share/applications" >/dev/null 2>&1 || true
        info "Desktop-databas uppdaterad."
    fi
}

run_app() {
    step "Starta Sonix"
    if confirm "Starta Sonix nu?"; then
        exec "${HOME_BIN}/${APP_EXE}"
    else
        say "Klart! Starta när du vill via startmenyn (${APP_NAME}) eller kommandot '${APP_EXE}'."
    fi
}

say "🍊 ${APP_NAME} – installerare"
info "Detta utför alla steg från README med bekräftelse per steg."

if [[ "$REFRESH" -eq 1 ]]; then
    step "Refresh-läge: bygger om & uppdaterar binär + startmenypost"
    if [[ "$IN_REPO" -ne 1 ]]; then
        echo "   ⚠️  --refresh måste köras inifrån en Sonix-klon." >&2
        exit 1
    fi
    step_build
    step_install_binary
    step_menu
    say "✅ Klar! Sonix är uppdaterad och startbar från startmenyn."
    exit 0
fi

say "Vi går igenom stegen nedan. Tryck 'j' för ja (eller 'n'/Enter för att hoppa över)."

step_deps
step_source
step_build
step_install_binary
step_menu
say "✅ Installation klar."
info "📂 Sample-paket (valfritt): lägg egna WAV-paket under '~/Music/Sonix/Sample_Packs' så syns de i Ljudbiblioteket."
info "   Sonix skapar katalogerna själv vid första start."
run_app
