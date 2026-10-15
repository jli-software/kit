#!/bin/sh
# kit — Online-Installer für Linux und macOS
#
#   curl -fsSL https://raw.githubusercontent.com/jli-software/kit/master/install.sh | sh
#
# Optionen (Umgebungsvariablen):
#   KIT_VERSION=v0.10.0      bestimmte Version statt der neusten
#   KIT_INSTALL_DIR=~/bin    Zielordner (Default: ~/.local/bin)
set -eu

REPO="jli-software/kit"
INSTALL_DIR="${KIT_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${KIT_VERSION:-latest}"

say() { printf '%s\n' "$*"; }
fail() { printf 'Fehler: %s\n' "$*" >&2; exit 1; }

case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=macos ;;
  *) fail "Nicht unterstütztes System: $(uname -s) — unter Windows install.ps1 nehmen." ;;
esac

case "$(uname -m)" in
  x86_64 | amd64) arch=x64 ;;
  aarch64 | arm64) arch=arm64 ;;
  *) fail "Nicht unterstützte Architektur: $(uname -m)" ;;
esac

[ "$os" = macos ] && [ "$arch" = x64 ] && fail "Für Intel-Macs gibt es kein Binary — 'cargo install --git https://github.com/$REPO' nehmen."

asset="kit-$os-$arch"

if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL "$1" -o "$2"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -q "$1" -O "$2"; }
else
  fail "curl oder wget wird benötigt."
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

# Neuste Version über die API auflösen: die "latest/download"-Links cached
# GitHub pro Datei, kurz nach einem Release können sie gemischt alt/neu zeigen.
if [ "$VERSION" = latest ]; then
  fetch "https://api.github.com/repos/$REPO/releases/latest" "$tmp/latest.json" \
    || fail "Neuste Version nicht abrufbar."
  VERSION="$(sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' "$tmp/latest.json" | head -n 1)"
  [ -n "$VERSION" ] || fail "Neuste Version nicht gefunden."
fi
base="https://github.com/$REPO/releases/download/$VERSION"

say "kit ($VERSION) für $os-$arch wird geladen …"
fetch "$base/$asset" "$tmp/kit" || fail "Download fehlgeschlagen: $base/$asset"
fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS" || fail "Prüfsummen nicht gefunden."

expected="$(awk -v a="$asset" '$2 == a || $2 == "*" a { print $1 }' "$tmp/SHA256SUMS")"
if command -v sha256sum >/dev/null 2>&1; then
  actual="$(sha256sum "$tmp/kit" | awk '{ print $1 }')"
else
  actual="$(shasum -a 256 "$tmp/kit" | awk '{ print $1 }')"
fi
if [ -z "$expected" ] || [ "$expected" != "$actual" ]; then
  fail "Prüfsumme stimmt nicht — Installation abgebrochen."
fi

mkdir -p "$INSTALL_DIR"
chmod +x "$tmp/kit"
mv -f "$tmp/kit" "$INSTALL_DIR/kit"
say "Installiert: $INSTALL_DIR/kit"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *) say ""
     say "Hinweis: $INSTALL_DIR ist nicht im PATH. In ~/.bashrc bzw. ~/.zshrc ergänzen:"
     say "  export PATH=\"$INSTALL_DIR:\$PATH\"" ;;
esac

if [ ! -f "${CODEX_HOME:-$HOME/.codex}/auth.json" ]; then
  say ""
  say "Für den Codex-Provider (ChatGPT-Abo) einmal mit Codex CLI anmelden:"
  say "  codex login"
fi

# stdin umleiten: bei "curl | sh" ist stdin das Skript selbst
if "$INSTALL_DIR/kit" --version </dev/null >/dev/null 2>&1; then
  say ""
  say "Fertig — 'kit --help' zeigt alle Modi."
else
  fail "kit startet nicht — bitte als Issue melden: https://github.com/$REPO/issues"
fi
