#!/bin/sh
# Clyra (native) headless installer.
#
#   curl -fsSL https://clyra-cli.xyz/install.sh | sh
#
# Installs the self-contained native binary (no runtime deps) to
# ~/.clyra/app, puts `clyra` on PATH, and runs it as a local-only
# systemd user service that survives reboots. Signing in is optional and
# enables sync after a restart. Re-running
# upgrades in place; ~/.clyra state is preserved.
#
# The binary ships with production endpoints baked in: no CLYRA_EDGE_URL or
# client-id configuration needed. Overrides (if any) go in ~/.clyra/env.
set -eu

BASE="${CLYRA_BASE_URL:-https://clyra-cli.xyz}"

# --- platform ---------------------------------------------------------------
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
  Linux) plat=linux ;;
  Darwin)
    echo "clyra install: on macOS, download the desktop app instead:" >&2
    echo "  $BASE/releases/latest.txt → $BASE/releases/clyra-<version>-macos-arm64.dmg" >&2
    exit 1
    ;;
  *)
    echo "clyra install: unsupported OS '$os' — only Linux for now." >&2
    exit 1
    ;;
esac
case "$arch" in
  x86_64 | amd64) arch=x86_64 ;;
  aarch64 | arm64) arch=aarch64 ;;
  *)
    echo "clyra install: unsupported architecture '$arch'." >&2
    exit 1
    ;;
esac

# --- download ----------------------------------------------------------------
ver="$(curl -fsSL "$BASE/releases/latest.txt" | tr -d '[:space:]')"
[ -n "$ver" ] || { echo "clyra install: could not resolve latest version" >&2; exit 1; }
file="clyra-$ver-$plat-$arch.tar.gz"
data_root="$HOME/.clyra"
app_root="$data_root/app"
dest="$app_root/$ver"

if [ -x "$dest/clyra" ]; then
  echo "clyra $ver already downloaded — relinking."
else
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' EXIT
  echo "downloading clyra $ver ($plat-$arch)…"
  curl -fSL --progress-bar "$BASE/releases/$file" -o "$tmp/$file"
  mkdir -p "$dest"
  tar -xzf "$tmp/$file" -C "$dest" --strip-components=1
fi

ln -sfn "$dest" "$app_root/current"
mkdir -p "$HOME/.local/bin"
ln -sf "$app_root/current/clyra" "$HOME/.local/bin/clyra"

# --- service -----------------------------------------------------------------
# The daemon is useful before auth: without a saved session it serves the local
# profile. Login only changes which profile the next daemon start selects.

service=manual
if command -v systemctl >/dev/null 2>&1 && [ -n "${XDG_RUNTIME_DIR:-}" ]; then
  mkdir -p "$HOME/.config/systemd/user"
  cat >"$HOME/.config/systemd/user/clyra.service" <<'UNIT'
[Unit]
Description=Clyra native headless engine
After=network-online.target
StartLimitIntervalSec=60
StartLimitBurst=5

[Service]
ExecStart=%h/.clyra/app/current/clyra headless
Restart=on-failure
RestartSec=5
EnvironmentFile=-%h/.clyra/env

[Install]
WantedBy=default.target
UNIT
  systemctl --user daemon-reload
  systemctl --user enable clyra
  systemctl --user restart clyra
  service=running
  # Keep the user manager (and the engine) running without an active login.
  loginctl enable-linger "$USER" 2>/dev/null \
    || sudo -n loginctl enable-linger "$USER" 2>/dev/null \
    || echo "warn: could not enable linger — the engine stops when you log out (run: sudo loginctl enable-linger $USER)"
else
  echo "warn: systemd user session not available — run the engine manually with: clyra headless"
fi

# --- agent CLIs ---------------------------------------------------------------
command -v claude >/dev/null 2>&1 || \
  echo "note: Claude Code CLI not found — install it with: curl -fsSL https://claude.ai/install.sh | bash"

case ":$PATH:" in
  *":$HOME/.local/bin:"*) path_hint="" ;;
  *) path_hint=' (add ~/.local/bin to your PATH)' ;;
esac

echo ""
echo "✓ clyra $ver installed$path_hint"
echo ""
case "$service" in
  running)
    echo "the engine is running with the new version (local-only unless sync is enabled)."
    echo "  systemctl --user status clyra    check the service"
    echo ""
    echo "optional sync (local sessions stay local):"
    echo "  systemctl --user stop clyra"
    echo "  clyra login"
    echo "  systemctl --user restart clyra"
    ;;
  manual)
    echo "next: run the local-only engine with \`clyra headless\`."
    echo "optional sync: run \`clyra login\` before starting the engine."
    ;;
esac
