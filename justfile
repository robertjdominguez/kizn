# kizn: every task is a recipe. `just` lists them.
# No recipe here calls a real model.

set shell := ["bash", "-euo", "pipefail", "-c"]
set windows-shell := ["bash.exe", "-euo", "pipefail", "-c"]

dev_dir := justfile_directory() / ".kizn-dev"
demo_dir := justfile_directory() / ".kizn-demo"
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

# Browser loop: bridge + Vite. `mock`: the fake model server. `paused`: hold bank writing. `tailnet`: serve on Tailscale
dev *args:
    #!/usr/bin/env bash
    set -euo pipefail
    mock=0; tailnet=0; paused=0; days=0
    for a in {{ args }}; do
      case "$a" in
        mock) mock=1 ;;
        tailnet) tailnet=1 ;;
        paused) paused=1 ;;
        days=*) days="${a#days=}" ;;
        *) echo "just dev: unknown option '$a' (mock, tailnet, paused, days=N)"; exit 2 ;;
      esac
    done
    # `just demo` runs this against its own data dir.
    data="${KIZN_DEV_DIR:-{{ dev_dir }}}"
    cargo build -q -p kizn-bridge
    mkdir -p "$data"
    pids=()
    cleanup() {
      trap - EXIT INT TERM
      kill "${pids[@]}" 2>/dev/null || true
      wait 2>/dev/null || true
      if [[ $tailnet == 1 ]]; then tailscale serve --https=443 off >/dev/null 2>&1 || true; echo "tailscale serve: off"; fi
    }
    trap cleanup EXIT INT TERM
    # mock: model traffic goes to the fake model server (127.0.0.1:7880), keys stay in memory.
    # paused: the bank writer holds still, so seeded states stay as seeded.
    # days=N: the clock runs N days ahead, to walk a week of practice.
    KIZN_DAYS_LATER=$days KIZN_BANK_PAUSED=$paused KIZN_MOCK=$mock KIZN_DATA_DIR="$data" KIZN_CONFIG_DIR="$data" "{{ bridge }}" &
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

# A sandbox with a mocked topic, ready to practise (./.kizn-demo; never touches .kizn-dev): fresh, tailnet, days=N
demo *args:
    #!/usr/bin/env bash
    set -euo pipefail
    fresh=0; pass=()
    for a in {{ args }}; do
      case "$a" in
        fresh) fresh=1 ;;
        *) pass+=("$a") ;;
      esac
    done
    if [[ $fresh == 1 ]]; then rm -rf "{{ demo_dir }}"; echo "demo: starting fresh"; fi
    if [[ ! -f "{{ demo_dir }}/kizn.db" ]]; then
      cargo build -q -p kizn-bridge
      mkdir -p "{{ demo_dir }}"
      sed 's|{{{{FAKE}}|http://127.0.0.1:7880|g' tests/fixtures/topic-with-bank/config.toml > "{{ demo_dir }}/config.toml"
      KIZN_DATA_DIR="{{ demo_dir }}" KIZN_CONFIG_DIR="{{ demo_dir }}" "{{ bridge }}" seed tests/fixtures/topic-with-bank/seed.sql
      echo "demo: seeded topic-with-bank"
    fi
    KIZN_DEV_DIR="{{ demo_dir }}" just dev mock "${pass[@]}"

# Remove the Tailscale serve mapping `just dev tailnet` adds (only that one)
dev-unserve:
    tailscale serve --https=443 off

# Window loop: the real Tauri window, IPC instead of the bridge
app:
    mkdir -p "{{ dev_dir }}"
    KIZN_DATA_DIR="{{ dev_dir }}" KIZN_CONFIG_DIR="{{ dev_dir }}" bunx tauri dev

# Unit tests, then both BDD layers
test:
    cargo test --workspace --exclude kizn-bdd --exclude kizn-install-tests
    just bdd
    just install-test

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

# Installers for this machine, unsigned (macOS is ad-hoc signed). Extra args go to `tauri build`.
# RPM and WiX (MSI) reject prerelease versions (0.1.0-rc.1), so prereleases skip .rpm and .msi.
bundle *args:
    #!/usr/bin/env bash
    set -euo pipefail
    version=$(sed -n 's/^  "version": "\(.*\)",$/\1/p' src-tauri/tauri.conf.json)
    extra=()
    if [[ "$version" == *-* ]]; then
      case "$(uname -s)" in
        Linux) extra=(--bundles appimage,deb) ;;
        Darwin) ;;
        *) extra=(--no-bundle) ;;
      esac
      echo "prerelease $version: ${extra[*]:-all bundles}"
    fi
    bunx tauri build {{ args }} ${extra[@]+"${extra[@]}"}

# @install scenarios: the real install scripts against a staged release (stubs, or KIZN_STAGE_ASSETS=<dir> of real ones)
install-test tag="":
    KIZN_BDD_TAG="{{ tag }}" cargo test -p kizn-install-tests --test install

# Copy this machine's bundles into <out> under the release's stable names
release-assets out="dist":
    #!/usr/bin/env bash
    set -euo pipefail
    out="{{ out }}"; mkdir -p "$out"
    first() { for f in "$@"; do [[ -e "$f" ]] && { echo "$f"; return; }; done; return 1; }
    case "$(uname -s)" in
      Linux)
        cp "$(first target/release/bundle/appimage/*.AppImage)" "$out/kizn-linux-x86_64.AppImage"
        cp src-tauri/icons/128x128.png "$out/kizn-icon.png"
        cp target/release/bundle/deb/*.deb "$out/"
        cp target/release/bundle/rpm/*.rpm "$out/" 2>/dev/null || echo "no .rpm (prereleases ship without one)"
        ;;
      Darwin)
        bundle=$(first target/universal-apple-darwin/release/bundle target/release/bundle)
        tar -czf "$out/kizn-macos-universal.app.tar.gz" -C "$bundle/macos" kizn.app
        cp "$bundle"/dmg/*.dmg "$out/"
        ;;
      *)
        exe=$(first target/release/kizn.exe)
        powershell -NoProfile -Command "Compress-Archive -Force -Path '$exe' -DestinationPath '$out/kizn-windows-x86_64.zip'"
        cp target/release/bundle/msi/*.msi "$out/" 2>/dev/null || echo "no .msi (prereleases ship without one)"
        ;;
    esac
    ls -l "$out"

# Write VERSION and SHA256SUMS for a release directory (after every OS's assets are in it)
release-sums dir="dist":
    #!/usr/bin/env bash
    set -euo pipefail
    cd "{{ dir }}"
    sed -n 's/^ *"version": *"\(.*\)",$/\1/p' "{{ justfile_directory() }}/src-tauri/tauri.conf.json" | head -1 > VERSION
    rm -f SHA256SUMS
    if command -v sha256sum >/dev/null; then sha256sum -- * > SHA256SUMS; else shasum -a 256 -- * > SHA256SUMS; fi
    cat VERSION SHA256SUMS

# Stage a fake release under <dir> and serve it, to try the install scripts by hand
stage-release dir version="0.1.0":
    cargo run -q -p kizn-install-tests --bin kizn-stage -- "{{ dir }}" "{{ version }}"

# Releasing, step 1: a PR that sets the version (main is protected). Merge it, then `just release <version>`
release-pr version:
    #!/usr/bin/env bash
    set -euo pipefail
    v="{{ version }}"; v="${v#v}"
    [[ "$v" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]] || { echo "not a version: $v (want X.Y.Z or X.Y.Z-rc.N)"; exit 2; }
    [[ -z "$(git status --porcelain)" ]] || { echo "the working tree isn't clean"; exit 1; }
    git fetch -q origin main
    git switch -q -c "release-v$v" origin/main
    sed -i.bak -E '/^\[workspace\.package\]/,/^\[/ s/^version = ".*"/version = "'"$v"'"/' Cargo.toml && rm Cargo.toml.bak
    sed -i.bak -E 's/^  "version": ".*",$/  "version": "'"$v"'",/' src-tauri/tauri.conf.json && rm src-tauri/tauri.conf.json.bak
    cargo update -q --workspace
    git add Cargo.toml Cargo.lock src-tauri/tauri.conf.json
    git commit -q -s -m "Release v$v"
    echo "Committed v$v on release-v$v."
    read -r -p "Push release-v$v and open its PR? [y/N] " a
    [[ "$a" == y || "$a" == Y ]] || { echo "Not pushed. Later: git push -u origin release-v$v && gh pr create"; exit 0; }
    git push -q -u origin "release-v$v"
    gh pr create --base main --head "release-v$v" --title "Release v$v" --body "$(printf '%s\n\n%s' \
      "**Before:** kizn's version was the previous release's." \
      "**After:** kizn is versioned $v. Once this merges, \`just release $v\` tags it and CI drafts the release.")"

# Releasing, step 2 (after the release PR merges): tag main's merge commit; CI builds and drafts the release
release version:
    #!/usr/bin/env bash
    set -euo pipefail
    v="{{ version }}"; v="${v#v}"
    git fetch -q --tags origin main
    git rev-parse -q --verify "refs/tags/v$v" >/dev/null && { echo "v$v is already tagged"; exit 1; }
    have=$(git show origin/main:src-tauri/tauri.conf.json | sed -n 's/^  "version": "\(.*\)",$/\1/p')
    [[ "$have" == "$v" ]] || { echo "main is at $have, not $v: merge the release PR first (just release-pr $v)"; exit 1; }
    commit=$(git rev-parse --short origin/main)
    read -r -p "Tag $commit (origin/main) as v$v and push the tag (CI drafts the release)? [y/N] " a
    [[ "$a" == y || "$a" == Y ]] || { echo "Not tagged."; exit 0; }
    git tag -a "v$v" -m "kizn v$v" origin/main
    git push -q origin "v$v"
    echo "Pushed v$v: the release workflow builds and drafts it."
# Load a named fixture (tests/fixtures/<name>) into the dev data dir; `just seed` lists them
seed fixture="":
    #!/usr/bin/env bash
    set -euo pipefail
    dir=tests/fixtures/{{ fixture }}
    if [[ -z "{{ fixture }}" || ! -d "$dir" ]]; then
      [[ -n "{{ fixture }}" ]] && echo "no fixture '{{ fixture }}'"
      echo "fixtures:"; for f in tests/fixtures/*/; do n=$(basename "$f"); echo "  $n  $(head -1 "$f/README" 2>/dev/null)"; done
      exit 1
    fi
    mkdir -p "{{ dev_dir }}"
    if [[ -f "$dir/config.toml" ]]; then
      # {{{{FAKE}} is the fake model server `just dev mock` runs.
      sed 's|{{{{FAKE}}|http://127.0.0.1:7880|g' "$dir/config.toml" > "{{ dev_dir }}/config.toml"
      echo "wrote {{ dev_dir }}/config.toml"
    fi
    if [[ -f "$dir/seed.sql" ]]; then
      cargo build -q -p kizn-bridge
      KIZN_DATA_DIR="{{ dev_dir }}" KIZN_CONFIG_DIR="{{ dev_dir }}" "{{ bridge }}" seed "$dir/seed.sql"
      echo "loaded $dir/seed.sql (reload the page)"
    fi
    echo "seeded {{ fixture }}: $(head -1 "$dir/README" 2>/dev/null)"

# Read-only views of the dev data: `db`, or `bank [topic]` (progress and attempts come later)
inspect what topic="":
    cargo build -q -p kizn-bridge
    KIZN_DATA_DIR="{{ dev_dir }}" KIZN_CONFIG_DIR="{{ dev_dir }}" "{{ bridge }}" inspect {{ what }} {{ topic }}

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
