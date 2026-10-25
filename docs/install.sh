#!/bin/sh
# kizn installer for macOS and Linux.
#
#   curl -fsSL https://kizn.app/install.sh | sh
#   curl -fsSL https://kizn.app/install.sh | sh -s -- --version 0.1.0-rc.1
#   curl -fsSL https://kizn.app/install.sh | sh -s -- --uninstall [--purge]
#
# Downloads a release from GitHub, checks its SHA-256 against the release's
# SHA256SUMS, and installs for this user only (no sudo):
#   Linux  ~/.local/bin/kizn (AppImage), a launcher entry and an icon
#   macOS  ~/Applications/kizn.app
#
# Environment: KIZN_VERSION (same as --version), KIZN_RELEASE_BASE (a mirror
# of the GitHub releases URL layout; the tests use it).

set -eu

BASE="${KIZN_RELEASE_BASE:-https://github.com/robertjdominguez/kizn/releases}"
VERSION="${KIZN_VERSION:-}"
UNINSTALL=0
PURGE=0

say() { printf '%s\n' "$*"; }
die() {
  printf 'kizn install: %s\n' "$*" >&2
  exit 1
}

usage() {
  say "Usage: install.sh [--version X.Y.Z] [--uninstall [--purge]]"
  say "  --version X   install that version (default: the latest release)"
  say "  --uninstall   remove kizn; your data stays"
  say "  --purge       with --uninstall, also delete your kizn data and config"
}

while [ $# -gt 0 ]; do
  case "$1" in
    --version) [ $# -ge 2 ] || die "--version needs a value"; VERSION="$2"; shift 2 ;;
    --version=*) VERSION="${1#*=}"; shift ;;
    --uninstall) UNINSTALL=1; shift ;;
    --purge) PURGE=1; shift ;;
    -h | --help) usage; exit 0 ;;
    *) die "unknown option '$1' (try --help)" ;;
  esac
done
[ "$PURGE" = 0 ] || [ "$UNINSTALL" = 1 ] || die "--purge only goes with --uninstall"
VERSION="${VERSION#v}"

# ---- where things go ---------------------------------------------------------

[ -n "${HOME:-}" ] || die "HOME isn't set"
case "$(uname -s)" in
  Linux)
    OS=linux
    case "$(uname -m)" in
      x86_64 | amd64) ;;
      *) die "kizn has no Linux $(uname -m) build yet (x86_64 only)" ;;
    esac
    ASSET=kizn-linux-x86_64.AppImage
    share="${XDG_DATA_HOME:-$HOME/.local/share}"
    APP="$HOME/.local/bin/kizn"
    LAUNCHER="$share/applications/kizn.desktop"
    ICON="$share/icons/hicolor/128x128/apps/kizn.png"
    DATA="$share/kizn"
    CONFIG="${XDG_CONFIG_HOME:-$HOME/.config}/kizn"
    ;;
  Darwin)
    OS=macos
    ASSET=kizn-macos-universal.app.tar.gz
    APP="$HOME/Applications/kizn.app"
    LAUNCHER=""
    ICON=""
    DATA="$HOME/Library/Application Support/kizn"
    CONFIG="$DATA"
    ;;
  *) die "this script is for macOS and Linux; on Windows use install.ps1 (see https://kizn.app)" ;;
esac

# ---- uninstall ---------------------------------------------------------------

if [ "$UNINSTALL" = 1 ]; then
  if [ -e "$APP" ]; then
    rm -rf "$APP"
    say "Removed $APP"
  else
    say "kizn isn't installed at $APP"
  fi
  for f in "$LAUNCHER" "$ICON"; do
    if [ -n "$f" ] && [ -e "$f" ]; then
      rm -f "$f"
      say "Removed $f"
    fi
  done
  if [ "$PURGE" = 1 ]; then
    for d in "$DATA" "$CONFIG"; do
      if [ -e "$d" ]; then
        rm -rf "$d"
        say "Deleted your kizn data: $d"
      fi
    done
  else
    say "Your kizn data is still in $DATA"
    [ "$CONFIG" = "$DATA" ] || say "and your config in $CONFIG"
    say "To delete it too: --uninstall --purge"
  fi
  exit 0
fi

# ---- download and verify -----------------------------------------------------

if command -v curl >/dev/null 2>&1; then
  fetch() { curl -fsSL --retry 2 -o "$2" "$1"; }
elif command -v wget >/dev/null 2>&1; then
  fetch() { wget -q -O "$2" "$1"; }
else
  die "needs curl or wget"
fi

if command -v sha256sum >/dev/null 2>&1; then
  sha256() { sha256sum "$1" | cut -d' ' -f1; }
elif command -v shasum >/dev/null 2>&1; then
  sha256() { shasum -a 256 "$1" | cut -d' ' -f1; }
else
  die "needs sha256sum or shasum to verify the download"
fi

if [ -n "$VERSION" ]; then
  url="$BASE/download/v$VERSION"
else
  url="$BASE/latest/download"
fi

tmp="$(mktemp -d 2>/dev/null || mktemp -d -t kizn)"
trap 'rm -rf "$tmp"' EXIT INT TERM

get() {
  fetch "$url/$1" "$tmp/$1" || die "couldn't download $url/$1${VERSION:+ (is v$VERSION a kizn release?)}"
}

get SHA256SUMS
verify() {
  want="$(awk -v n="$1" '$2 == n || $2 == "*" n { print $1 }' "$tmp/SHA256SUMS")"
  got="$(sha256 "$tmp/$1")"
  if [ -z "$want" ] || [ "$want" != "$got" ]; then
    die "the download failed verification: $1 doesn't match the release's SHA256SUMS (expected ${want:-no entry}, got $got). Nothing was installed."
  fi
}

files="VERSION $ASSET"
[ "$OS" = linux ] && files="$files kizn-icon.png"
for f in $files; do
  get "$f"
  verify "$f"
done
say "✓ checksum verified"
version="$(tr -d ' \r\n' <"$tmp/VERSION")"
say "Installing kizn v$version"

# ---- install -----------------------------------------------------------------

if [ "$OS" = linux ]; then
  mkdir -p "$(dirname "$APP")" "$(dirname "$LAUNCHER")" "$(dirname "$ICON")"
  cp "$tmp/$ASSET" "$APP.tmp"
  chmod 755 "$APP.tmp"
  mv -f "$APP.tmp" "$APP"
  cp "$tmp/kizn-icon.png" "$ICON"
  cat >"$LAUNCHER" <<EOF
[Desktop Entry]
Type=Application
Name=kizn
Comment=Learn something properly
Exec="$APP" %U
Icon=$ICON
Categories=Education;
Terminal=false
EOF
  command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$(dirname "$LAUNCHER")" >/dev/null 2>&1 || true
else
  mkdir -p "$tmp/unpacked" "$(dirname "$APP")"
  tar -xzf "$tmp/$ASSET" -C "$tmp/unpacked"
  [ -d "$tmp/unpacked/kizn.app" ] || die "the release's macOS archive has no kizn.app"
  rm -rf "$APP"
  mv "$tmp/unpacked/kizn.app" "$APP"
  # curl sets no quarantine flag; clear one in case the archive came by browser.
  xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true
fi

say "kizn v$version is installed: $APP"
if [ "$OS" = linux ]; then
  say "It's in your launcher as kizn."
else
  say "Open it from Launchpad or Spotlight (kizn)."
fi
say "Your data will live in $DATA"
[ "$CONFIG" = "$DATA" ] || say "and your config in $CONFIG"
if [ "$OS" = linux ]; then
  case ":$PATH:" in
    *":$HOME/.local/bin:"*) ;;
    *) say "Note: ~/.local/bin isn't on your PATH, so 'kizn' won't run from a terminal (the launcher works)." ;;
  esac
fi
