#!/usr/bin/env bash
#
# Spear Launcher — smart installer
# =================================
# Detects your distro, checks (and optionally installs) system dependencies,
# ensures Rust is available, then builds and installs Spear.

set -euo pipefail

# ──────────────────────────────────────────────
#  ANSI colours & helpers
# ──────────────────────────────────────────────
R="\033[1;31m"   G="\033[1;32m"   Y="\033[1;33m"
B="\033[1;34m"   M="\033[1;35m"   C="\033[1;36m"
W="\033[1;37m"   D="\033[2m"      NC="\033[0m"

info()  { echo -e " ${B}•${NC} $*"; }
ok()    { echo -e " ${G}✓${NC} $*"; }
warn()  { echo -e " ${Y}⚠${NC} $*"; }
err()   { echo -e " ${R}✗${NC} $*"; }
header(){ echo -e "\n${M}━━━ $* ━━━${NC}\n"; }
bullet(){ echo -e " ${D}·${NC} $*"; }

# ──────────────────────────────────────────────
#  Spinner — runs a command with a tiny animation
# ──────────────────────────────────────────────
spinner() {
  local msg="$1"  label="$2"
  shift 2
  local pid=
  # background spinner
  (
    tput civis 2>/dev/null || true
    local s=('⣾' '⣽' '⣻' '⢿' '⡿' '⣟' '⣯' '⣷')
    while kill -0 "$pid" 2>/dev/null; do
      for c in "${s[@]}"; do
        printf "\r ${C}%s${NC} ${msg}${D} %s${NC}   " "$c" "$label"
        sleep 0.08
      done
    done
    tput cnorm 2>/dev/null || true
  ) &
  local spinpid=$!
  # real command
  "$@" &
  pid=$!
  wait "$pid" 2>/dev/null && true
  local rc=$?
  kill "$spinpid" 2>/dev/null || true
  wait "$spinpid" 2>/dev/null || true
  printf "\r"
  return $rc
}

# ──────────────────────────────────────────────
#  Banner
# ──────────────────────────────────────────────
banner() {
  echo -e "${M}"
  cat << "EOF"

       ╔══════════════════╗
       ║  ⚔  S P E A R  ⚔  ║
       ╚══════════════════╝

EOF
  echo -e "${NC}"
  echo -e "${W}  Spear Launcher${NC} — Raycast-like launcher for GNOME"
  echo -e "${D}  Installer v0.1${NC}\n"
}

# ──────────────────────────────────────────────
#  Distro detection
# ──────────────────────────────────────────────
detect_distro() {
  if [[ -f /etc/os-release ]]; then
    local id id_like
    id=$(  grep '^ID='            /etc/os-release 2>/dev/null | cut -d= -f2 | tr -d '"' || true)
    id_like=$(grep '^ID_LIKE='    /etc/os-release 2>/dev/null | cut -d= -f2 | tr -d '"' || true)
    [[ "$id_like" == *"rhel"*  || "$id" == "fedora"    ]] && echo "fedora" && return
    [[ "$id" == "ubuntu" || "$id" == "debian" || "$id_like" == *"debian"* ]] && echo "debian" && return
    [[ "$id" == "arch"   || "$id_like" == *"arch"*     ]] && echo "arch"   && return
    [[ "$id" == "opensuse"* || "$id_like" == *"suse"* ]] && echo "suse"   && return
  fi
  echo "unknown"
}

# ──────────────────────────────────────────────
#  Dependency maps
# ──────────────────────────────────────────────
# Build-only — match the README precisely
DEPS_BUILD_fedora=(gcc pkg-config gtk4-devel libadwaita-devel)
DEPS_BUILD_debian=(build-essential pkg-config libgtk-4-dev libadwaita-1-dev)
DEPS_BUILD_arch=(base-devel gtk4 libadwaita)
DEPS_BUILD_suse=(gcc pkg-config gtk4-devel libadwaita-devel)

# Optional run-time
DEPS_RUNTIME_fedora=(gtk4-layer-shell)
DEPS_RUNTIME_debian=(libgtk4-layer-shell0)
DEPS_RUNTIME_arch=(gtk4-layer-shell)
DEPS_RUNTIME_suse=(gtk4-layer-shell)

PKG_QUERY_fedora() { rpm -q "$1" &>/dev/null; }
PKG_QUERY_debian() { dpkg -s "$1" &>/dev/null; }
PKG_QUERY_arch()   { pacman -Qi "$1" &>/dev/null; }
PKG_QUERY_suse()   { rpm -q "$1" &>/dev/null; }

PKG_INSTALL_fedora="sudo dnf install -y"
PKG_INSTALL_debian="sudo apt-get install -y"
PKG_INSTALL_arch="sudo pacman -S --needed --noconfirm"
PKG_INSTALL_suse="sudo zypper install -y"

# ──────────────────────────────────────────────
#  Toolbox install (keeps host clean)
# ──────────────────────────────────────────────
toolbox_install() {
  local distro=$1
  local container="spear-build"

  header "Installing via Toolbox"

  # Ensure the container exists
  if ! toolbox list --containers 2>/dev/null | grep -q "$container"; then
    info "Creating toolbox container '${container}'…"
    toolbox create --container "$container" -y 2>/dev/null || {
      # older toolbox versions
      toolbox create --container "$container" <<< "y" 2>/dev/null || true
    }
  fi

  # Install build deps inside container (toolbox runs as user, needs sudo)
  local cmd_var="PKG_INSTALL_${distro}"
  local install_cmd="${!cmd_var}"

  info "Installing build dependencies inside container…"
  build_arr="DEPS_BUILD_${distro}[@]"
  local deps=("${!build_arr}")
  toolbox run --container "$container" $install_cmd "${deps[@]}"

  # Install Rust inside container if missing (home is shared, so check full path)
  info "Ensuring Rust is available inside container…"
  toolbox run --container "$container" bash -c 'export PATH="$HOME/.cargo/bin:$PATH"; command -v cargo &>/dev/null || curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y &>/dev/null'

  # Build & install — toolbox shares home, so pwd resolves to same path inside
  info "Building Spear inside container…"
  local project_dir
  project_dir="$(pwd)"
  toolbox run --container "$container" bash -c "
    export PATH=\"\$HOME/.cargo/bin:\$PATH\"
    cd '$project_dir' && cargo run --bin install
  "

  ok "Installation via toolbox complete."
}

# ──────────────────────────────────────────────
#  Run the Rust installer binary natively
# ──────────────────────────────────────────────
run_install_bin() {
  header "Building & installing Spear"

  local extra_args=()
  if [[ -n "${SHORTCUT:-}" ]]; then
    extra_args+=("$SHORTCUT")
  fi

  echo -e " ${D}Compiling in release mode…${NC}"
  if ! cargo run --bin install "${extra_args[@]}"; then
    err "Installation failed."
    exit 1
  fi
}

# ──────────────────────────────────────────────
#  PATH hint
# ──────────────────────────────────────────────
path_hint() {
  local bin_dir="$HOME/.local/bin"
  if [[ ":$PATH:" != *":$bin_dir:"* ]]; then
    echo ""
    warn "$bin_dir is not in your PATH."
    bullet "Add this to your shell profile (~/.bashrc / ~/.zshrc):"
    echo -e "   ${D}export PATH=\"\$HOME/.local/bin:\$PATH\"${NC}"
  fi
}

# ──────────────────────────────────────────────
#  CLI flags
# ──────────────────────────────────────────────
usage() {
  echo "Usage: $0 [options] [shortcut]"
  echo ""
  echo "Options:"
  echo "  --help         Show this help"
  echo "  --shortcut S   Keybinding (default: <Alt>space)"
  echo "  --skip-deps    Skip system dependency check"
  exit 0
}

SKIP_DEPS=false
SHORTCUT=
while [[ $# -gt 0 ]]; do
  case "$1" in
    --help) usage ;;
    --shortcut) shift; SHORTCUT="$1" ;;
    --skip-deps) SKIP_DEPS=true ;;
    --*) warn "Unknown flag: $1"; usage ;;
    *) SHORTCUT="$1" ;;
  esac
  shift
done

# ──────────────────────────────────────────────
#  Check if toolbox is available
# ──────────────────────────────────────────────
has_toolbox() { command -v toolbox &>/dev/null; }

# ──────────────────────────────────────────────
#  Main
# ──────────────────────────────────────────────
main() {
  banner

  # ── Distro ──
  header "System"
  local distro
  distro=$(detect_distro)
  if [[ "$distro" == "unknown" ]]; then
    warn "Could not detect distribution. You may need to install build dependencies manually."
    bullet "See README.md for details."
    echo ""
  else
    ok "Detected: ${W}$distro${NC}"
  fi

  # ── Toolbox availability ──
  if has_toolbox; then
    ok "Toolbox available — can build in a clean sandbox."
  fi

  # ── System dependencies ──
  if [[ "$SKIP_DEPS" == false && "$distro" != "unknown" ]]; then
    header "Build dependencies"
    pkg_arr="DEPS_BUILD_${distro}[@]"
    local build_pkgs=("${!pkg_arr}")

    local -a missing_build=()
    for pkg in "${build_pkgs[@]}"; do
      qfn="PKG_QUERY_${distro}"
      if $qfn "$pkg" &>/dev/null; then
        ok "$pkg"
      else
        missing_build+=("$pkg")
      fi
    done

    if [[ ${#missing_build[@]} -gt 0 ]]; then
      local cmd_var="PKG_INSTALL_${distro}"
      local install_cmd="${!cmd_var}"
      warn "Missing build packages: ${Y}${missing_build[*]}${NC}"
      echo ""

      if has_toolbox; then
        echo -e " ${W}?${NC} How to proceed?"
        echo -e "   ${W}1${NC}) Install natively with sudo"
        echo -e "   ${W}2${NC}) Use Toolbox (keeps system clean)${D} (recommended)${NC}"
        echo -e "   ${W}3${NC}) Skip — build anyway (may fail)"
        echo ""
        echo -ne " ${W}▸${NC} "
        read -r ans
        case "$ans" in
          3|s|skip) warn "Skipping. Build may fail if libraries are missing." ;;
          2|t|toolbox) toolbox_install "$distro"; path_hint; echo ""; ok "Done."; exit 0 ;;
          *)
            spinner "Installing build packages…" "" $install_cmd "${missing_build[@]}"
            echo ""
            ok "Build dependencies installed." ;;
        esac
      else
        echo -e " ${W}?${NC} Install missing build dependencies? (requires sudo) [Y/n] "
        read -r ans
        case "$ans" in
          n|N|no|NO)
            warn "Skipping dependency installation. Build may fail if libraries are missing." ;;
          *)
            spinner "Installing build packages…" "" $install_cmd "${missing_build[@]}"
            echo ""
            ok "Build dependencies installed." ;;
        esac
      fi
    else
      ok "All build dependencies satisfied."
    fi

    # Optional runtime deps
    header "Runtime dependencies (optional)"
    runtime_arr="DEPS_RUNTIME_${distro}[@]"
    local runtime_pkgs=("${!runtime_arr}")
    local -a missing_runtime=()
    for pkg in "${runtime_pkgs[@]}"; do
      qfn="PKG_QUERY_${distro}"
      if $qfn "$pkg" &>/dev/null; then
        ok "$pkg"
      else
        missing_runtime+=("$pkg")
      fi
    done

    if [[ ${#missing_runtime[@]} -gt 0 ]]; then
      echo ""
      echo -e " ${D}Optional packages for Wayland layer-shell support:${NC}"
      bullet "Provides better integration on Wayland sessions."
      local rcmd_var="PKG_INSTALL_${distro}"
      local install_cmd="${!rcmd_var}"
      echo ""
      echo -e " ${W}?${NC} Install runtime optional packages? [y/N] "
      read -r ans
      case "$ans" in
        y|Y|yes|YES)
          spinner "Installing runtime packages…" "" $install_cmd "${missing_runtime[@]}"
          echo ""
          ok "Runtime packages installed." ;;
        *)
          warn "Skipping. Layer-shell support will be unavailable on Wayland." ;;
      esac
    else
      ok "All runtime dependencies satisfied."
    fi

  elif [[ "$SKIP_DEPS" == false ]]; then
    warn "Unknown distro — skipping automatic dependency check."
    bullet "Make sure GTK4, Libadwaita, and pkg-config are installed."
  else
    info "Dependency check skipped (--skip-deps)."
  fi

  # ── Rust ──
  if ! command -v cargo &>/dev/null; then
    header "Rust toolchain"
    warn "Rust / Cargo is not installed."
    echo ""

    if has_toolbox; then
      echo -e " ${W}?${NC} How to proceed?"
      echo -e "   ${W}1${NC}) Install Rust via rustup on host"
      echo -e "   ${W}2${NC}) Use Toolbox (keeps system clean)${D} (recommended)${NC}"
      echo ""
      echo -ne " ${W}▸${NC} "
      read -r ans
      case "$ans" in
        2|t|toolbox) toolbox_install "$distro"; path_hint; echo ""; ok "Done."; exit 0 ;;
        *)
          spinner "Downloading rustup…" "" \
            bash -c "$(curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs)" -y --no-modify-path
          source "$HOME/.cargo/env"
          ok "Rust installed: $(cargo version)" ;;
      esac
    else
      echo -e " ${W}?${NC} Install Rust via rustup? [Y/n] "
      read -r ans
      case "$ans" in
        n|N|no|NO)
          err "Rust is required. Please install it from https://rustup.rs/ and re-run."
          exit 1 ;;
        *)
          spinner "Downloading rustup…" "" \
            bash -c "$(curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs)" -y --no-modify-path
          source "$HOME/.cargo/env"
          ok "Rust installed: $(cargo version)" ;;
      esac
    fi
  else
    header "Rust toolchain"
    local ver
    ver=$(cargo version 2>/dev/null | cut -d' ' -f2)
    ok "Rust ${ver} found at $(command -v cargo)"
  fi

  # ── Build & install ──
  run_install_bin

  # ── Post-install hints ──
  path_hint
  echo ""
  echo -e "${G}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo -e "${W}  Spear is ready!${NC}"
  echo ""
  echo -e "  ${D}Run:${NC}  ${W}spear --daemon${NC}"
  echo -e "  ${D}Key:${NC}  ${SHORTCUT:-<Alt>space}"
  echo ""
  echo -e "  ${D}Quit:${NC} ${W}spear --quit${NC}"
  echo -e "${G}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
  echo ""
}

main
