#!/usr/bin/env bash
set -euo pipefail

REPO="linuxmobile/hibiki"
APP="hibiki"
BRANCH="${HIBIKI_BRANCH:-main}"

VERSION="${HIBIKI_VERSION:-latest}"
MODE="user"
INSTALL_DESKTOP=1
UNINSTALL=0
USE_APPIMAGE=0
BIN_DIR="${HIBIKI_BIN_DIR:-}"
SUDO=""

# ---------------------------------------------------------------------------
# Pretty output
# ---------------------------------------------------------------------------
if [ -t 1 ]; then
  C_RESET=$'\033[0m'; C_BOLD=$'\033[1m'; C_RED=$'\033[31m'
  C_GREEN=$'\033[32m'; C_YELLOW=$'\033[33m'; C_BLUE=$'\033[34m'
else
  C_RESET=""; C_BOLD=""; C_RED=""; C_GREEN=""; C_YELLOW=""; C_BLUE=""
fi

info()  { printf '%s==>%s %s\n' "$C_BLUE$C_BOLD" "$C_RESET" "$*"; }
ok()    { printf '%s==>%s %s\n' "$C_GREEN$C_BOLD" "$C_RESET" "$*"; }
warn()  { printf '%swarning:%s %s\n' "$C_YELLOW$C_BOLD" "$C_RESET" "$*" >&2; }
die()   { printf '%serror:%s %s\n' "$C_RED$C_BOLD" "$C_RESET" "$*" >&2; exit 1; }

usage() {
  cat <<'EOF'
Hibiki installer for Linux (Wayland).

Usage:
  install.sh [options]

Options:
  --version <tag|latest>   Release to install (default: latest)
  --appimage               Install the self-contained AppImage instead of the
                           raw binary (recommended where gtk4-layer-shell is
                           not packaged, e.g. Debian 12 / Ubuntu 24.04)
  --bin-dir <dir>          Where to place the `hibiki` binary (default: ~/.local/bin)
  --system                 Install to /usr/local (uses sudo)
  --no-desktop             Do not install the .desktop entry / icon
  --uninstall              Remove a previously installed copy
  -h, --help               Show this help

Environment variables:
  HIBIKI_VERSION   Same as --version
  HIBIKI_BIN_DIR   Same as --bin-dir
  HIBIKI_BRANCH    Git branch to fetch the .desktop/icon from (default: main)
EOF
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
while [ $# -gt 0 ]; do
  case "$1" in
    --version)      VERSION="${2:?--version needs a value}"; shift 2 ;;
    --version=*)    VERSION="${1#*=}"; shift ;;
    --appimage)     USE_APPIMAGE=1; shift ;;
    --bin-dir)      BIN_DIR="${2:?--bin-dir needs a value}"; shift 2 ;;
    --bin-dir=*)    BIN_DIR="${1#*=}"; shift ;;
    --system)       MODE="system"; shift ;;
    --no-desktop)   INSTALL_DESKTOP=0; shift ;;
    --uninstall)    UNINSTALL=1; shift ;;
    -h|--help)      usage; exit 0 ;;
    *) die "unknown option: $1 (try --help)" ;;
  esac
done

# ---------------------------------------------------------------------------
# Layout / privileges
# ---------------------------------------------------------------------------
if [ "$MODE" = "system" ]; then
  BIN_DIR="${BIN_DIR:-/usr/local/bin}"
  SHARE_DIR="/usr/local/share"
  if [ "$(id -u)" -ne 0 ]; then
    command -v sudo >/dev/null 2>&1 || die "system install needs root or sudo"
    SUDO="sudo"
  fi
else
  BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
  SHARE_DIR="${XDG_DATA_HOME:-$HOME/.local/share}"
fi

APP_DESKTOP_DIR="$SHARE_DIR/applications"
APP_ICON_DIR="$SHARE_DIR/icons/hicolor/scalable/apps"
APP_METAINFO_DIR="$SHARE_DIR/metainfo"

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
need_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

download() {
  curl -fL --retry 3 --connect-timeout 20 -o "$2" "$1"
}

detect_asset_arch() {
  case "$(uname -m)" in
    x86_64|amd64)   echo "x86_64" ;;
    aarch64|arm64)  echo "aarch64" ;;
    *) die "unsupported architecture: $(uname -m) (only x86_64 and aarch64 builds are published)" ;;
  esac
}

resolve_version() {
  case "$VERSION" in
    latest)
      local tag
      tag="$(curl -fsSL -o /dev/null -w '%{url_effective}' \
            "https://github.com/$REPO/releases/latest" 2>/dev/null | sed 's#.*/tag/##')"
      [ -n "$tag" ] && [ "$tag" != "$VERSION" ] || die "could not resolve the latest release"
      printf '%s' "$tag"
      ;;
    v*)  printf '%s' "$VERSION" ;;
    *)   printf 'v%s' "$VERSION" ;;
  esac
}

# ---------------------------------------------------------------------------
# Uninstall
# ---------------------------------------------------------------------------
if [ "$UNINSTALL" = "1" ]; then
  info "Uninstalling $APP"
  rm -f "$BIN_DIR/$APP"
  rm -f "$APP_DESKTOP_DIR/$APP.desktop"
  rm -f "$APP_ICON_DIR/$APP.svg"
  rm -f "$APP_METAINFO_DIR/$APP.metainfo.xml"
  ok "Removed $APP (configuration in ~/.config/$APP was left untouched)"
  exit 0
fi

# ---------------------------------------------------------------------------
# Preflight
# ---------------------------------------------------------------------------
[ "$(uname -s)" = "Linux" ] || die "Hibiki only runs on Linux"

need_cmd curl
need_cmd uname

ASSET_ARCH="$(detect_asset_arch)"
TAG="$(resolve_version)"
if [ "$USE_APPIMAGE" = "1" ]; then
  info "Installing $APP $TAG ($ASSET_ARCH, AppImage)"
else
  info "Installing $APP $TAG ($ASSET_ARCH, native binary)"
fi

# ---------------------------------------------------------------------------
# Download
# ---------------------------------------------------------------------------
TMP_DIR="$(mktemp -d)"
cleanup() { rm -rf "$TMP_DIR"; }
trap cleanup EXIT

TAG_NOV="${TAG#v}"

if [ "$USE_APPIMAGE" = "1" ]; then
  ASSET_NAMES=("$APP-$TAG_NOV-$ASSET_ARCH.AppImage")
  DEST="$TMP_DIR/$APP"
else
  ASSET_NAMES=(
    "$APP-linux-$ASSET_ARCH"
    "keystroke-linux-$ASSET_ARCH"
    "$APP-$ASSET_ARCH-unknown-linux-gnu"
  )
  DEST="$TMP_DIR/$APP"
fi

found=0
for name in "${ASSET_NAMES[@]}"; do
  url="https://github.com/$REPO/releases/download/$TAG/$name"
  info "Downloading $url"
  if download "$url" "$DEST"; then
    found=1
    break
  fi
done
[ "$found" = "1" ] || die "no $([ "$USE_APPIMAGE" = 1 ] && echo AppImage || echo binary) found for $ASSET_ARCH in release $TAG"

chmod 0755 "$DEST"

if command -v file >/dev/null 2>&1; then
  file_type="$(file "$DEST" 2>/dev/null || true)"
  case "$file_type" in
    *ELF*) ;;
    *) die "downloaded file is not a Linux executable" ;;
  esac
fi

# ---------------------------------------------------------------------------
# glibc check
# ---------------------------------------------------------------------------
check_glibc() {
  command -v getconf >/dev/null 2>&1 || return 0
  local host req sorted smallest
  host="$(getconf GNU_LIBC_VERSION 2>/dev/null | awk '{print $2}')"
  [ -n "$host" ] || return 0
  req="$(grep -aoE 'GLIBC_[0-9]+\.[0-9]+' "$1" 2>/dev/null | sort -V -u | tail -1 | sed 's/GLIBC_//')"
  [ -n "$req" ] || return 0
  sorted="$(printf '%s\n%s\n' "$host" "$req" | sort -V)"
  smallest="${sorted%%$'\n'*}"
  if [ "$smallest" != "$req" ]; then
    warn "this build needs glibc >= $req but this system has $host; it may fail to start"
  fi
}

if [ "$USE_APPIMAGE" != "1" ]; then
  check_glibc "$DEST"
fi

# ---------------------------------------------------------------------------
# Install
# ---------------------------------------------------------------------------
info "Installing to $BIN_DIR/$APP"
$SUDO install -d "$BIN_DIR"
$SUDO install -m 0755 "$DEST" "$BIN_DIR/$APP"

# ---------------------------------------------------------------------------
# Desktop entry + icon
# ---------------------------------------------------------------------------
if [ "$INSTALL_DESKTOP" = "1" ]; then
  info "Installing desktop entry and icon"
  $SUDO install -d "$APP_DESKTOP_DIR" "$APP_ICON_DIR"

  tmp_desktop="$TMP_DIR/$APP.desktop"
  cat >"$tmp_desktop" <<EOF
[Desktop Entry]
Name=Hibiki
Comment=Elevating the tactile dialogue. A high-fidelity visual and auditory companion that gives your keystrokes a modern resonance.
Exec=$BIN_DIR/$APP
Icon=$APP
Type=Application
Categories=Utility;
Keywords=keystroke;keyboard;screencast;
EOF
  $SUDO install -m 0644 "$tmp_desktop" "$APP_DESKTOP_DIR/$APP.desktop"

  icon_url="https://raw.githubusercontent.com/$REPO/$BRANCH/assets/$APP.svg"
  if download "$icon_url" "$TMP_DIR/$APP.svg"; then
    $SUDO install -m 0644 "$TMP_DIR/$APP.svg" "$APP_ICON_DIR/$APP.svg"
  else
    warn "could not download $APP.svg; skipping icon"
  fi

  meta_url="https://raw.githubusercontent.com/$REPO/$BRANCH/assets/$APP.metainfo.xml"
  if download "$meta_url" "$TMP_DIR/$APP.metainfo.xml"; then
    $SUDO install -Dm0644 "$TMP_DIR/$APP.metainfo.xml" "$APP_METAINFO_DIR/$APP.metainfo.xml"
  else
    warn "could not download $APP.metainfo.xml; skipping AppStream metadata"
  fi

  if [ "$MODE" = "user" ]; then
    command -v update-desktop-database >/dev/null 2>&1 && \
      update-desktop-database "$APP_DESKTOP_DIR" >/dev/null 2>&1 || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 && \
      gtk-update-icon-cache -q -t -f "$SHARE_DIR/icons/hicolor" >/dev/null 2>&1 || true
  fi
fi

# ---------------------------------------------------------------------------
# Runtime dependencies
# ---------------------------------------------------------------------------
if [ "$USE_APPIMAGE" != "1" ]; then
  LD_CACHE="$(ldconfig -p 2>/dev/null || true)"
  lib_present() {
    case "$LD_CACHE" in
      *"$1"*) return 0 ;;
      *)      return 1 ;;
    esac
  }

  LIBS=(
    libgtk-4.so.1
    libgtk4-layer-shell.so.0
    libxkbcommon.so.0
    libdbus-1.so.3
    libasound.so.2
    libfontconfig.so.1
  )

  missing_libs=()
  for lib in "${LIBS[@]}"; do
    lib_present "$lib" || missing_libs+=("$lib")
  done

  if [ "${#missing_libs[@]}" -gt 0 ]; then
    warn "missing runtime libraries: ${missing_libs[*]}"

    declare -A PKG_PACMAN=(
      [libgtk-4.so.1]=gtk4 [libgtk4-layer-shell.so.0]=gtk4-layer-shell
      [libxkbcommon.so.0]=libxkbcommon [libdbus-1.so.3]=dbus
      [libasound.so.2]=alsa-lib [libfontconfig.so.1]=fontconfig
    )
    declare -A PKG_APT=(
      [libgtk-4.so.1]=libgtk-4-1 [libgtk4-layer-shell.so.0]=libgtk4-layer-shell0
      [libxkbcommon.so.0]=libxkbcommon0 [libdbus-1.so.3]=libdbus-1-3
      [libasound.so.2]=libasound2 [libfontconfig.so.1]=libfontconfig1
    )
    declare -A PKG_DNF=(
      [libgtk-4.so.1]=gtk4 [libgtk4-layer-shell.so.0]=gtk4-layer-shell
      [libxkbcommon.so.0]=libxkbcommon [libdbus-1.so.3]=dbus-libs
      [libasound.so.2]=alsa-lib [libfontconfig.so.1]=fontconfig
    )

    if grep -qi '^ID=ubuntu' /etc/os-release 2>/dev/null; then
      PKG_APT[libasound.so.2]=libasound2t64
    fi

    mgr=""; pkgs=()
    if command -v pacman >/dev/null 2>&1; then
      mgr="sudo pacman -S"
      for lib in "${missing_libs[@]}"; do pkgs+=("${PKG_PACMAN[$lib]:-$lib}"); done
    elif command -v apt-get >/dev/null 2>&1; then
      mgr="sudo apt install"
      for lib in "${missing_libs[@]}"; do pkgs+=("${PKG_APT[$lib]:-$lib}"); done
    elif command -v dnf >/dev/null 2>&1; then
      mgr="sudo dnf install"
      for lib in "${missing_libs[@]}"; do pkgs+=("${PKG_DNF[$lib]:-$lib}"); done
    fi

    if [ -n "$mgr" ]; then
      printf '    install them with:\n\n      %s%s %s%s\n\n' "$C_BOLD" "$mgr" "${pkgs[*]}" "$C_RESET"
      case " ${missing_libs[*]} " in
        *" libgtk4-layer-shell.so.0 "*)
          printf '    %sgtk4-layer-shell is not packaged on older Debian/Ubuntu; use the AppImage instead:%s\n\n      %scurl -fsSL https://raw.githubusercontent.com/%s/%s/install.sh | bash -s -- --appimage%s\n\n' \
            "$C_YELLOW" "$C_RESET" "$C_BOLD" "$REPO" "$BRANCH" "$C_RESET"
          ;;
      esac
    else
      printf '    install the packages that provide: %s\n\n' "${missing_libs[*]}"
    fi
  fi
fi

# ---------------------------------------------------------------------------
# Post-install notes
# ---------------------------------------------------------------------------
echo
ok "Hibiki $TAG installed to $BIN_DIR/$APP"

case ":$PATH:" in
  *":$BIN_DIR:"*) ;;
  *)
    warn "$BIN_DIR is not in your PATH"
    printf '    add this to your shell profile:\n\n      %sexport PATH="%s:$PATH"%s\n\n' "$C_BOLD" "$BIN_DIR" "$C_RESET"
    ;;
esac

if [ "$USE_APPIMAGE" = "1" ]; then
  if [ ! -e /dev/fuse ]; then
    warn "/dev/fuse not found; run the AppImage with --appimage-extract-and-run, or install fuse"
  fi
fi

cat <<EOF
${C_BOLD}Next steps${C_RESET}
  1. Hibiki reads keyboards from /dev/input, so your user must be in the
     'input' group:

       sudo usermod -aG input "\$USER"   ${C_YELLOW}# log out and back in afterwards${C_RESET}

  2. Start it with:  ${C_BOLD}$APP${C_RESET}   (or from your application launcher)
  3. Toggle modes:   ${C_BOLD}$APP --toggle${C_RESET}

EOF
