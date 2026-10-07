#!/usr/bin/env bash
# Linux packaging: build the release binary and produce
#   target/package/clyra-<version>-linux-<arch>.tar.gz
# containing the binary, the .desktop entry, and the icon, plus an install.sh
# that drops them into ~/.local (XDG) paths.
#
# Usage: scripts/package-linux.sh
# Env:   PROFILE=debug for a fast unoptimized package (CI smoke); default release.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v cargo >/dev/null 2>&1 || PATH="$HOME/.cargo/bin:$PATH"
PROFILE="${PROFILE:-release}"
ARCH="$(uname -m)"
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | sed 's/.*"\(.*\)".*/\1/')"
OUT_DIR="$ROOT/target/package"
STAGE="$OUT_DIR/clyra-$VERSION-linux-$ARCH"
TARBALL="$STAGE.tar.gz"

cd "$ROOT"
if [[ "$PROFILE" == "release" ]]; then
  cargo build --release --locked -p clyra
  BIN="$ROOT/target/release/clyra"
else
  cargo build -p clyra
  BIN="$ROOT/target/debug/clyra"
fi

rm -rf "$STAGE" "$TARBALL"
mkdir -p "$STAGE"
install -m 755 "$BIN" "$STAGE/clyra"
install -m 644 "$ROOT/dist/clyra.desktop" "$STAGE/clyra.desktop"
install -m 644 "$ROOT/dist/clyra.png" "$STAGE/clyra.png"
mkdir -p "$STAGE/licenses/fonts"
cp "$ROOT/LICENSE" "$ROOT/THIRD_PARTY_NOTICES.md" "$STAGE/"
cp "$ROOT/crates/ui/assets/fonts/licenses/"* "$STAGE/licenses/fonts/"

cat >"$STAGE/install.sh" <<'INSTALL'
#!/usr/bin/env bash
# Install Clyra into ~/.local (no root needed).
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
install -Dm755 "$HERE/clyra" "$HOME/.local/bin/clyra"
install -Dm644 "$HERE/clyra.desktop" "$HOME/.local/share/applications/clyra.desktop"
# Desktop launchers do not necessarily inherit ~/.local/bin in PATH.
sed -i "s|^Exec=.*|Exec=\"$HOME/.local/bin/clyra\" %u|; s|^TryExec=.*|TryExec=$HOME/.local/bin/clyra|" "$HOME/.local/share/applications/clyra.desktop"
install -Dm644 "$HERE/clyra.png" "$HOME/.local/share/icons/hicolor/1024x1024/apps/clyra.png"
mkdir -p "$HOME/.local/share/clyra/licenses/fonts"
cp "$HERE/LICENSE" "$HERE/THIRD_PARTY_NOTICES.md" "$HOME/.local/share/clyra/"
cp "$HERE/licenses/fonts/"* "$HOME/.local/share/clyra/licenses/fonts/"
command -v update-desktop-database >/dev/null 2>&1 \
  && update-desktop-database "$HOME/.local/share/applications" || true
echo "Installed. Make sure ~/.local/bin is on your PATH."
INSTALL
chmod 755 "$STAGE/install.sh"

tar -czf "$TARBALL" -C "$OUT_DIR" "$(basename "$STAGE")"

# Native Debian/Ubuntu installer, including the actual linked library requirements.
if command -v dpkg-deb >/dev/null && command -v dpkg-shlibdeps >/dev/null; then
  DEB_ARCH="$(dpkg --print-architecture)"
  DEB_ROOT="$OUT_DIR/deb-$ARCH"
  rm -rf "$DEB_ROOT"
  mkdir -p "$DEB_ROOT/DEBIAN" "$DEB_ROOT/usr/bin" "$DEB_ROOT/usr/share/applications" \
    "$DEB_ROOT/usr/share/icons/hicolor/1024x1024/apps" "$DEB_ROOT/usr/share/doc/clyra/licenses/fonts"
  install -m 755 "$BIN" "$DEB_ROOT/usr/bin/clyra"
  cp "$STAGE/clyra.desktop" "$DEB_ROOT/usr/share/applications/"
  cp "$STAGE/clyra.png" "$DEB_ROOT/usr/share/icons/hicolor/1024x1024/apps/"
  cp "$STAGE/LICENSE" "$STAGE/THIRD_PARTY_NOTICES.md" "$DEB_ROOT/usr/share/doc/clyra/"
  cp "$STAGE/licenses/fonts/"* "$DEB_ROOT/usr/share/doc/clyra/licenses/fonts/"
  mkdir -p "$DEB_ROOT/debian"
  printf 'Source: clyra\nSection: devel\nPriority: optional\nMaintainer: Clyra <noreply@localhost>\n\nPackage: clyra\nArchitecture: any\nDescription: Coding agent workspace\n' >"$DEB_ROOT/debian/control"
  # The WebKit helper is embedded in the Rust executable and extracted at runtime.
  # Its shared libraries are invisible when inspecting only the main executable.
  HELPER="$(find "$(dirname "$BIN")/build" -type f -name clyra-webkit -print -quit)"
  test -n "$HELPER" || { echo 'Compiled Linux browser helper not found'; exit 1; }
  DEPS="$(cd "$DEB_ROOT" && dpkg-shlibdeps -O -eusr/bin/clyra -e"$HELPER" | sed -n 's/^shlibs:Depends=//p')"
  test -n "$DEPS" || { echo 'Could not resolve Linux runtime dependencies'; exit 1; }
  rm -rf "$DEB_ROOT/debian"
  printf 'Package: clyra\nVersion: %s\nArchitecture: %s\nMaintainer: Clyra <noreply@localhost>\nSection: devel\nPriority: optional\nDepends: %s\nDescription: Coding agent workspace\n A desktop workspace for coding agents and autonomous Dots.\n' \
    "$VERSION" "$DEB_ARCH" "$DEPS, libvulkan1" >"$DEB_ROOT/DEBIAN/control"
  dpkg-deb --root-owner-group --build "$DEB_ROOT" "$OUT_DIR/clyra-$VERSION-linux-$ARCH.deb"
  rm -rf "$DEB_ROOT"
fi
rm -rf "$STAGE"
echo "packaged: $TARBALL"
tar -tzf "$TARBALL"
