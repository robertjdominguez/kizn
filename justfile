# kizn: every task is a recipe. `just` lists them.
# No recipe here calls a real model.

set shell := ["bash", "-euo", "pipefail", "-c"]
set windows-shell := ["bash.exe", "-euo", "pipefail", "-c"]

dev_dir := justfile_directory() / ".kizn-dev"
exe := if os() == "windows" { ".exe" } else { "" }
bridge := justfile_directory() / "target/debug/kizn-bridge" + exe
toolchain := `sed -n 's/^channel *= *"\(.*\)"/\1/p' rust-toolchain.toml`

default:
    @just --list

# Check the toolchain and system deps, install JS deps, fetch crates
setup:
    #!/usr/bin/env bash
    set -euo pipefail
    fail=0
    need() { command -v "$1" >/dev/null || { echo "✗ $1 not found: $2"; fail=1; }; }
    need cargo "install Rust {{ toolchain }} (https://rustup.rs)"
    need bun "https://bun.sh"
    need node "Node 20+ runs Playwright and WebdriverIO (https://nodejs.org)"
    if command -v rustup >/dev/null; then
      rustup toolchain install >/dev/null   # installs the toolchain rust-toolchain.toml pins
    fi
    if command -v rustc >/dev/null; then
      have=$(rustc --version | awk '{print $2}')
      if [[ "$have" != "{{ toolchain }}" ]]; then
        echo "✗ rustc $have, but rust-toolchain.toml pins {{ toolchain }}."
        echo "  With rustup this is automatic; otherwise install {{ toolchain }}."
        fail=1
      else
        echo "✓ rustc $have"
      fi
    fi
    if [[ "$(uname -s)" == Linux ]]; then
      for pc in webkit2gtk-4.1 javascriptcoregtk-4.1 libsoup-3.0 librsvg-2.0; do
        pkg-config --exists "$pc" 2>/dev/null || { echo "✗ $pc (Tauri needs it: see https://v2.tauri.app/start/prerequisites/)"; fail=1; }
      done
    fi
    [[ $fail == 0 ]] || exit 1
    bun install
    cargo fetch
    (cd tests/ui && bunx playwright install chromium)
    echo "✓ setup done. Next: just doctor, then just dev"

# What's installed, where data and config live, and the Tailscale operator
doctor:
    #!/usr/bin/env bash
    set -uo pipefail
    v() { printf "  %-16s " "$1"; if command -v "$1" >/dev/null; then "$@" 2>&1 | head -1; else echo "not found"; fi; }
    echo "tools"
    v rustc --version; v cargo --version; v bun --version; v node --version; v just --version
    printf "  %-16s %s\n" "tauri-cli" "$(bunx tauri --version 2>/dev/null || echo 'not installed (just setup)')"
    printf "  %-16s %s\n" "pinned rust" "{{ toolchain }}"
    if [[ "$(uname -s)" == Linux ]]; then
      echo "linux libraries"
      for pc in webkit2gtk-4.1 javascriptcoregtk-4.1 libsoup-3.0 librsvg-2.0; do
        printf "  %-22s %s\n" "$pc" "$(pkg-config --modversion "$pc" 2>/dev/null || echo missing)"
      done
    fi
    echo "smoke tests (just smoke)"
    printf "  %-16s %s\n" "tauri-driver" "$(command -v tauri-driver || echo 'not found: cargo install tauri-driver --locked')"
    if [[ "$(uname -s)" == Linux ]]; then
      printf "  %-16s %s\n" "WebKitWebDriver" "$(command -v WebKitWebDriver || echo 'not found (Debian/Ubuntu: webkit2gtk-driver)')"
      printf "  %-16s %s\n" "xvfb-run" "$(command -v xvfb-run || echo 'not found (only needed without a display)')"
    fi
    cargo build -q -p kizn-bridge
    echo "paths: the app"
    env -u KIZN_DATA_DIR -u KIZN_CONFIG_DIR "{{ bridge }}" paths | sed 's/^/  /'
    echo "paths: just dev"
    KIZN_DATA_DIR="{{ dev_dir }}" KIZN_CONFIG_DIR="{{ dev_dir }}" "{{ bridge }}" paths | sed 's/^/  /'
    if command -v tailscale >/dev/null; then
      op=$(tailscale debug prefs 2>/dev/null | sed -n 's/.*"OperatorUser": *"\([^"]*\)".*/\1/p')
      echo "tailscale"
      printf "  %-16s %s\n" "operator" "${op:-not set: sudo tailscale set --operator=\$USER (for just dev tailnet)}"
    fi

# Browser loop: bridge + Vite on localhost. `mock` (phase 2+), `tailnet` serves it on this host's Tailscale name
dev *args:
    #!/usr/bin/env bash
    set -euo pipefail
    mock=0; tailnet=0
    for a in {{ args }}; do
      case "$a" in
        mock) mock=1 ;;
        tailnet) tailnet=1 ;;
        *) echo "just dev: unknown option '$a' (mock, tailnet)"; exit 2 ;;
      esac
    done
    [[ $mock == 1 ]] && echo "note: the mock provider arrives in phase 2; 'mock' does nothing yet"
    cargo build -q -p kizn-bridge
    mkdir -p "{{ dev_dir }}"
    pids=()
    cleanup() {
      trap - EXIT INT TERM
      kill "${pids[@]}" 2>/dev/null || true
      wait 2>/dev/null || true
      if [[ $tailnet == 1 ]]; then tailscale serve --https=443 off >/dev/null 2>&1 || true; echo "tailscale serve: off"; fi
    }
    trap cleanup EXIT INT TERM
    KIZN_DATA_DIR="{{ dev_dir }}" KIZN_CONFIG_DIR="{{ dev_dir }}" "{{ bridge }}" &
    pids+=($!)
    if [[ $tailnet == 1 ]]; then
      # Tailnet only. Never `tailscale funnel`, which is public.
      tailscale serve --bg --https=443 http://127.0.0.1:1420 >/dev/null
      host=$(tailscale status --json | node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>console.log(JSON.parse(s).Self.DNSName.replace(/\.$/,"")))')
      echo "serving on https://$host (tailnet only)"
    fi
    # Vite runs directly (not via `bun run`) so killing its pid stops it.
    (cd ui && exec node_modules/.bin/vite) &
    pids+=($!)
    # Whichever stops first (or Ctrl-C) takes the other down with it.
    wait -n

# Remove the Tailscale serve mapping `just dev tailnet` adds (only that one)
dev-unserve:
    tailscale serve --https=443 off

# Window loop: the real Tauri window, IPC instead of the bridge
app:
    mkdir -p "{{ dev_dir }}"
    KIZN_DATA_DIR="{{ dev_dir }}" KIZN_CONFIG_DIR="{{ dev_dir }}" bunx tauri dev

# Unit tests, then both BDD layers
test:
    cargo test --workspace --exclude kizn-bdd
    just bdd

# BDD: @domain (cucumber-rs) then @ui (Playwright). Optional tag narrows both
bdd tag="":
    KIZN_BDD_TAG="{{ tag }}" cargo test -p kizn-bdd --test domain
    cargo build -q -p kizn-bridge --features test
    cd tests/ui && bunx bddgen && KIZN_BDD_TAG="{{ tag }}" bunx playwright test --pass-with-no-tests

# @smoke scenarios in the real webview via tauri-driver (Linux, Windows)
smoke tag="":
    #!/usr/bin/env bash
    set -euo pipefail
    case "$(uname -s)" in
      Darwin) echo "macOS has no WebDriver for WKWebView: check the window by hand (just app)"; exit 1 ;;
      Linux) native=WebKitWebDriver; hint="Debian/Ubuntu: apt install webkit2gtk-driver; Arch's webkit2gtk-4.1 doesn't ship it" ;;
      *) native=msedgedriver; hint="https://developer.microsoft.com/microsoft-edge/tools/webdriver/ (match your Edge version)" ;;
    esac
    driver=${TAURI_DRIVER:-$(command -v tauri-driver || echo "$HOME/.cargo/bin/tauri-driver")}
    [[ -x "$driver" ]] || { echo "✗ tauri-driver not found: cargo install tauri-driver --locked"; exit 1; }
    command -v "$native" >/dev/null || { echo "✗ $native not found ($hint)"; exit 1; }
    bunx tauri build --debug --no-bundle
    run=()
    if [[ "$(uname -s)" == Linux && -z "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]]; then
      command -v xvfb-run >/dev/null || { echo "✗ no display and no xvfb-run"; exit 1; }
      run=(xvfb-run -a)
    fi
    cd tests/smoke && TAURI_DRIVER="$driver" KIZN_BDD_TAG="{{ tag }}" "${run[@]}" bunx wdio run wdio.conf.ts

# clippy, rustfmt, biome, tsc
lint:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets --all-features -- -D warnings
    bunx biome check .
    cd ui && bunx tsc --noEmit
    cd tests/ui && bunx tsc --noEmit
    cd tests/smoke && bunx tsc --noEmit

# Format Rust and TS
fmt:
    cargo fmt --all
    bunx biome check --write .

# Regenerate ui/src/bindings from the Rust types
types:
    cargo test -q -p kizn-app export_bindings

# Release build for this machine (no installers)
build:
    bunx tauri build --no-bundle

# Installers for this machine (unsigned until phase 1)
bundle:
    bunx tauri build

# Read-only views of the dev data (db; bank, progress, attempts in later phases)
inspect what:
    cargo build -q -p kizn-bridge
    KIZN_DATA_DIR="{{ dev_dir }}" KIZN_CONFIG_DIR="{{ dev_dir }}" "{{ bridge }}" inspect {{ what }}

# Wipe the dev data directory (./.kizn-dev)
db-reset yes="":
    #!/usr/bin/env bash
    set -euo pipefail
    [[ -d "{{ dev_dir }}" ]] || { echo "nothing to reset: {{ dev_dir }} doesn't exist"; exit 0; }
    if [[ "{{ yes }}" != "--yes" ]]; then
      read -r -p "Delete {{ dev_dir }}? [y/N] " a
      [[ "$a" == y || "$a" == Y ]] || { echo "kept"; exit 0; }
    fi
    rm -rf "{{ dev_dir }}"
    echo "deleted {{ dev_dir }}"
