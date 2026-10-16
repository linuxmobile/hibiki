#!/usr/bin/env bash
set -euo pipefail

APP=hibiki
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"

ARCH="${APPIMAGE_ARCH:-$(uname -m)}"
case "$ARCH" in
  x86_64|amd64)   LD_ARCH=x86_64 ;;
  aarch64|arm64)  LD_ARCH=aarch64 ;;
  *) echo "error: unsupported architecture: $ARCH" >&2; exit 1 ;;
esac

VERSION="${VERSION:-$(sed -n 's/^version *= *"\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)}"
BINARY="${BINARY:-$ROOT/target/release/$APP}"
OUTDIR="${OUTDIR:-$ROOT/dist}"
TOOLS_DIR="${TOOLS_DIR:-$ROOT/.appimage-tools}"

for tool in curl install realpath; do
  command -v "$tool" >/dev/null 2>&1 || { echo "error: missing required command: $tool" >&2; exit 1; }
done
[ -x "$BINARY" ] || { echo "error: binary not found: $BINARY (run 'cargo build --release' first)" >&2; exit 1; }

# Normalise to absolute paths
mkdir -p "$TOOLS_DIR/bin" "$OUTDIR"
OUTDIR="$(cd "$OUTDIR" && pwd)"
TOOLS_DIR="$(cd "$TOOLS_DIR" && pwd)"
APPDIR="${APPDIR:-$OUTDIR/$APP.AppDir}"
OUTPUT="${OUTPUT:-$OUTDIR/$APP-$VERSION-$ARCH.AppImage}"

LD="$TOOLS_DIR/linuxdeploy-$LD_ARCH.AppImage"
PLUGIN="$TOOLS_DIR/linuxdeploy-plugin-gtk.sh"

if [ ! -f "$LD" ]; then
  echo "==> Downloading linuxdeploy ($LD_ARCH)"
  curl -fL --retry 3 -o "$LD" \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-$LD_ARCH.AppImage"
fi
if [ ! -f "$PLUGIN" ]; then
  echo "==> Downloading linuxdeploy-plugin-gtk"
  curl -fL --retry 3 -o "$PLUGIN" \
    "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh"
fi
chmod +x "$LD" "$PLUGIN"
ln -sf "$PLUGIN" "$TOOLS_DIR/bin/linuxdeploy-plugin-gtk"

# --- Assemble the AppDir -----------------------------------------------------
echo "==> Assembling AppDir at $APPDIR"
rm -rf "$APPDIR"
install -Dm755 "$BINARY" "$APPDIR/usr/bin/$APP"
install -Dm644 "$ROOT/assets/$APP.desktop" "$APPDIR/usr/share/applications/$APP.desktop"
install -Dm644 "$ROOT/assets/$APP.metainfo.xml" "$APPDIR/usr/share/metainfo/$APP.metainfo.xml"
install -Dm644 "$ROOT/assets/$APP.svg" "$APPDIR/usr/share/icons/hicolor/scalable/apps/$APP.svg"
install -Dm644 "$ROOT/assets/$APP.svg" "$APPDIR/$APP.svg"

install -d "$APPDIR/apprun-hooks"
cat > "$APPDIR/apprun-hooks/zz-$APP.sh" <<'EOF'
unset GDK_BACKEND
unset GTK_THEME
EOF

# --- Run linuxdeploy ---------------------------------------------------------
echo "==> Bundling libraries and building AppImage"
export ARCH="$LD_ARCH"
export VERSION="$VERSION"
export DEPLOY_GTK_VERSION=4
export PATH="$TOOLS_DIR/bin:$PATH"
export APPIMAGE_EXTRACT_AND_RUN=1

cd "$OUTDIR"
"$LD" --appdir "$APPDIR" --plugin gtk --output appimage \
  --desktop-file "$APPDIR/usr/share/applications/$APP.desktop" \
  --icon-file "$APPDIR/$APP.svg"

for f in "$OUTDIR"/*.AppImage; do
  [ -e "$f" ] || continue
  [ "$f" = "$OUTPUT" ] && continue
  mv -f "$f" "$OUTPUT"
done

[ -x "$OUTPUT" ] || { echo "error: AppImage was not produced" >&2; exit 1; }
echo "==> AppImage ready: $OUTPUT"
