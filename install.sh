#!/usr/bin/env bash
#
# Sonix Studio – installer
#
# Two modes:
#
#   1) GUIDED INSTALL (default) – for new users.
#      Each step is confirmed first, just like in an installer:
#
#          bash install.sh
#
#   2) REFRESH – rebuilds and updates the binary + start menu entry.
#      Use after a code change so you can start straight from the menu:
#
#          bash install.sh --refresh
#
#   Add --yes to confirm every step automatically (e.g. CI / your own scripts):
#
#          bash install.sh --refresh --yes
#
# Dependencies installed (per distribution) are documented in the README.

set -euo pipefail

APP_NAME="Sonix Studio"
APP_EXE="sonix"
REPO_URL="https://github.com/alexwest1981/sonix.git"
ICON_SRC_REL="assets/sonix.png"
CONFIG_DIR="${HOME}/.config/sonix"

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
            sed -n '2,26p' "${BASH_SOURCE[0]}"
            exit 0
            ;;
        *) echo "Unknown argument: $arg (see --help)" >&2; exit 1 ;;
    esac
done

say()   { printf '\n\033[1m%s\033[0m\n' "$*"; }
info()  { printf '   %s\n' "$*"; }
step()  { printf '\n──────────────────────────────────────────────\n'; printf '◉  %s\n' "$1"; printf '──────────────────────────────────────────────\n'; }

confirm() {
    # $1 = question. Returns 0 (yes) or 1 (no). Enter = no (safe choice).
    if [[ "$YES" -eq 1 ]]; then
        return 0
    fi
    local answer
    printf '\n\033[1m❓ %s\033[0m  [y/N]: ' "$1"
    read -r answer
    case "$answer" in
        y|Y|yes|Yes|YES) return 0 ;;
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

rust_toolchain_ready() {
    command -v cargo >/dev/null 2>&1 && cargo --version >/dev/null 2>&1
}

ensure_rust() {
    # Verify cargo can actually run – on rustup systems the cargo/rustc shims
    # exist in PATH even when no toolchain is installed, so `command -v` alone
    # is not enough (it would fail later with "rustup could not choose a
    # version of cargo...").
    if rust_toolchain_ready; then
        return 0
    fi
    export PATH="${HOME}/.cargo/bin:${PATH}"
    if rust_toolchain_ready; then
        return 0
    fi
    if command -v rustup >/dev/null 2>&1; then
        if confirm "rustup is present but no toolchain is selected – install 'stable' now (rustup default stable)?"; then
            rustup default stable
            export PATH="${HOME}/.cargo/bin:${PATH}"
        fi
    elif confirm "Install Rust via rustup (stable) now?"; then
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        export PATH="${HOME}/.cargo/bin:${PATH}"
    fi
    if ! rust_toolchain_ready; then
        echo "   ⚠️  Rust/toolchain is missing or not configured. Aborting." >&2
        exit 1
    fi
}

has_cmd() { command -v "$1" >/dev/null 2>&1; }

cc_ok() { has_cmd cc || has_cmd gcc; }

# The ALSA development library is required to compile the audio engine (cpal).
alsa_dev_ok() {
    has_cmd pkg-config || return 1
    pkg-config --exists alsa 2>/dev/null
}

# Packages from official package sources. Groups: build dependencies (must be
# present to compile) and tools (zenity/unzip are needed by the app at runtime).
build_pkgs_for() {
    case "$1" in
        pacman) echo "base-devel alsa-lib" ;;
        apt)    echo "build-essential libasound2-dev" ;;
        dnf)    echo "gcc-c++ alsa-lib-devel" ;;
    esac
}
tool_pkgs_for() {
    echo "zenity unzip curl git"
}

pm_install() {
    local pm="$1"
    local pkgs="$2"
    case "$pm" in
        pacman) sudo pacman -S --needed $pkgs ;;
        apt)    sudo apt-get install -y $pkgs ;;
        dnf)    sudo dnf install -y $pkgs ;;
        *)      return 1 ;;
    esac
}

step_deps() {
    step "System dependencies"
    local pm
    pm="$(detect_pm)"

    # Which build dependencies are actually missing?
    local missing_build=0
    local missing_tools=0
    if ! cc_ok || ! has_cmd make || ! alsa_dev_ok; then
        missing_build=1
    fi
    local t
    for t in zenity unzip curl git; do
        if ! has_cmd "$t"; then
            missing_tools=1
            break
        fi
    done

    if [[ "$pm" == "apt" ]] && { [[ "$missing_build" -eq 1 || "$missing_tools" -eq 1 ]]; }; then
        sudo apt-get update
    fi

    case "$pm" in
        pacman|apt|dnf)
            if [[ "$missing_build" -eq 1 ]]; then
                info "Missing build dependencies: $(build_pkgs_for "$pm")"
                if confirm "Install missing build dependencies via '${pm}' (official package sources)?"; then
                    pm_install "$pm" "$(build_pkgs_for "$pm")"
                else
                    info "   Skipped – the build may fail if they are missing."
                fi
            else
                info "Build tools (C compiler, make, ALSA-dev) are already present."
            fi

            if [[ "$missing_tools" -eq 1 ]]; then
                info "Missing tools: $(tool_pkgs_for)"
                if confirm "Install missing tools via '${pm}' (official package sources)?"; then
                    pm_install "$pm" "$(tool_pkgs_for)"
                else
                    info "   Skipped – zenity/unzip are needed by the app at runtime."
                fi
            else
                info "Tools (zenity, unzip, curl, git) are already present."
            fi
            ;;
        unknown)
            echo "   ⚠️  Unknown distribution – cannot install automatically."
            echo "   Install manually from official channels: Rust + C compiler + ALSA development library (+ zenity, unzip)."
            ;;
    esac

    # Rust is installed via rustup (official channel: rust-lang.org), never via
    # third-party PPAs. Verifies that cargo can actually run.
    ensure_rust

    if ! cc_ok || ! alsa_dev_ok; then
        echo "   ⚠️  C compiler or ALSA-dev is still missing – the build may fail." >&2
    fi
}

step_language() {
    step "Interface language"
    say "Select the Sonix interface language. You can always switch later from the 🌐 menu in the app."

    local choice
    if [[ "$YES" -eq 1 ]]; then
        choice=1
    else
        echo "  1) English"
        echo "  2) Svenska"
        echo "  3) Dansk"
        echo "  4) Norsk (bokmål)"
        echo "  5) Deutsch"
        echo "  6) Español"
        echo "  7) Français"
        printf '\n\033[1m❓ Choice [1-7, default 1]: \033[0m'
        read -r choice
    fi

    local code
    case "$choice" in
        1|"") code="en" ;;
        2) code="sv" ;;
        3) code="da" ;;
        4) code="no" ;;
        5) code="de" ;;
        6) code="es" ;;
        7) code="fr" ;;
        *)
            echo "   ⚠️  Invalid choice – keeping English." >&2
            code="en" ;;
    esac

    mkdir -p "$CONFIG_DIR"
    cat > "$CONFIG_DIR/config.json" <<EOF
{
  "language": "$code"
}
EOF
    info "Saved language preference: $CONFIG_DIR/config.json"
}

step_source() {
    step "Source code"
    if [[ "$IN_REPO" -eq 1 ]]; then
        info "Already running inside a Sonix clone: $SCRIPT_DIR"
        return 0
    fi

    local default_dir="${HOME}/Projects/sonix"
    if confirm "Clone Sonix into '${default_dir}'? (type 'n' to skip if you already have a clone)"; then
        mkdir -p "$(dirname "$default_dir")"
        git clone "$REPO_URL" "$default_dir"
        cd "$default_dir"
        SCRIPT_DIR="$default_dir"
    else
        echo "   ⚠️  No source code – cannot build. Aborting." >&2
        exit 1
    fi
}

step_build() {
    step "Compile (release build – may take a few minutes the first time)"
    if confirm "Build Sonix now (cargo build --release)?"; then
        (cd "$SCRIPT_DIR" && cargo build --release)
    fi
    if [[ ! -x "$SCRIPT_DIR/target/release/${APP_EXE}" ]]; then
        echo "   ⚠️  The build failed (no executable was created). Aborting." >&2
        exit 1
    fi
}

step_install_binary() {
    step "Install binary → ~/.cargo/bin"
    if confirm "Install '${APP_EXE}' into '${HOME_BIN}'?"; then
        mkdir -p "$HOME_BIN"
        install -m 0755 "$SCRIPT_DIR/target/release/${APP_EXE}" "$HOME_BIN/${APP_EXE}"
        info "Installed: ${HOME_BIN}/${APP_EXE}"
    fi
}

step_menu() {
    step "Start menu entry (application menu)"
    if ! confirm "Create start menu entry '${APP_NAME}'?"; then
        return 0
    fi

    mkdir -p "${HOME}/.local/share/applications"

    # Copy the icon into the hicolor theme so the menu entry works even if the
    # clone is moved or deleted. Generate the common sizes when ImageMagick is
    # available; otherwise install the source PNG at its native size.
    if [[ -f "$SCRIPT_DIR/$ICON_SRC_REL" ]]; then
        local installed_any=0
        for size in 512 256 128 64 48 32; do
            local icon_dir="${HOME}/.local/share/icons/hicolor/${size}x${size}/apps"
            mkdir -p "$icon_dir"
            if command -v magick >/dev/null 2>&1; then
                magick "$SCRIPT_DIR/$ICON_SRC_REL" -resize "${size}x${size}" \
                    "$icon_dir/${APP_EXE}.png" && installed_any=1
            fi
        done
        if [[ "$installed_any" -eq 0 ]]; then
            mkdir -p "${HOME}/.local/share/icons/hicolor/512x512/apps"
            install -m 0644 "$SCRIPT_DIR/$ICON_SRC_REL" \
                "${HOME}/.local/share/icons/hicolor/512x512/apps/${APP_EXE}.png"
        fi
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

    info "Created: ${desktop_file}"
    if command -v update-desktop-database >/dev/null 2>&1; then
        update-desktop-database "${HOME}/.local/share/applications" >/dev/null 2>&1 || true
        info "Desktop database updated."
    fi
}

run_app() {
    step "Start Sonix"
    if confirm "Start Sonix now?"; then
        exec "${HOME_BIN}/${APP_EXE}"
    else
        say "Done! Start whenever you like from the start menu (${APP_NAME}) or with the command '${APP_EXE}'."
    fi
}

say "🍊 ${APP_NAME} – installer"
info "This performs every step from the README with confirmation at each step."

if [[ "$REFRESH" -eq 1 ]]; then
    step "Refresh mode: rebuilding & updating binary + start menu entry"
    if [[ "$IN_REPO" -ne 1 ]]; then
        echo "   ⚠️  --refresh must be run from inside a Sonix clone." >&2
        exit 1
    fi
    step_build
    step_install_binary
    step_menu
    say "✅ Done! Sonix is updated and can be started from the start menu."
    exit 0
fi

say "We will go through the steps below. Press 'y' for yes (or 'n'/Enter to skip)."

step_language
step_deps
step_source
step_build
step_install_binary
step_menu
say "✅ Installation complete."
info "📂 Sample packs (optional): place your own WAV packs under '~/Music/Sonix/Sample_Packs' and they will appear in the Sound Library."
info "   Sonix creates these folders itself on first start."
run_app
