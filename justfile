set windows-shell := ["pwsh", "-NoLogo", "-Command"]

# List recipes
default:
    @just --list

# Install dependencies
install:
    pnpm install

# Start the desktop app in development mode
dev:
    pnpm tauri dev

# Check formatting, lint, type-check and test everything
check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace
    pnpm lint
    pnpm typecheck
    pnpm test
    node --test 'scripts/*.test.mjs'

# Build the web viewer's WebAssembly package (crates/web/pkg) with the wasm-bindgen CLI from mise.toml
web-wasm:
    #!/usr/bin/env bash
    set -euo pipefail
    # The CLI must be exactly the version the crate pins, or the generated bindings do not match.
    want=$(sed -n 's/^wasm-bindgen = "=\(.*\)"$/\1/p' crates/web/Cargo.toml)
    have=$(wasm-bindgen --version | cut -d' ' -f2)
    [ "$want" = "$have" ] || { echo "wasm-bindgen CLI is $have, crates/web needs $want: run mise install" >&2; exit 1; }
    cargo build -p specdeck-web --target wasm32-unknown-unknown --release --locked
    wasm-bindgen --target web --out-dir crates/web/pkg target/wasm32-unknown-unknown/release/specdeck_web.wasm

# Run the web viewer's WebAssembly tests in Node
web-test:
    cargo test -p specdeck-web --target wasm32-unknown-unknown --locked

# Build and check the web viewer: its WebAssembly core and tests, the page, and the service with the page built in
check-web: web-wasm web-test
    cargo clippy -p specdeck-web --target wasm32-unknown-unknown --all-targets -- -D warnings
    pnpm --dir app/ui build:web
    cargo clippy -p specdeck-signalling --all-targets --features web-viewer -- -D warnings

# Measure 20 viewers' effect on the host's terminal, alone so other work does not skew it
capacity:
    cargo test -p specdeck-core --test sharing twenty_viewers_under_continuous_output -- --ignored --nocapture

# Format the Rust code
fmt:
    cargo fmt --all

# Build installers for this platform (target/release/bundle)
package:
    pnpm tauri build

# Start the documentation site with live reload (http://localhost:4321/specdeck/)
docs:
    cd docs/site && pnpm install && pnpm dev

# Build the documentation site (docs/site/dist)
docs-build:
    cd docs/site && pnpm install --frozen-lockfile && pnpm build

# Make the how-to videos, or the ones named (docs/video/<name>/renders/); needs Chrome, Python 3 and ffmpeg
video *names="getting-started following diagnostics":
    #!/usr/bin/env bash
    set -euo pipefail
    cd docs/video
    pnpm install
    [ -x .venv/bin/python ] || python3 -m venv .venv
    .venv/bin/pip install --quiet -r requirements.txt
    for name in {{names}}; do
        (
            cd "$name"
            node capture.mjs
            ../.venv/bin/python ../lib/speak.py .
            node build.mjs
            pnpm exec hyperframes render --output "renders/specdeck-$name.mp4"
        )
    done

# Deploy signalling and coturn to Colima's local Kubernetes (deploy/k8s/local); needs `colima start --kubernetes`
local-up:
    #!/usr/bin/env bash
    set -euo pipefail
    test -f deploy/k8s/base/turn-secret.env || (umask 077 && echo "TURN_SECRET=$(openssl rand -hex 32)" > deploy/k8s/base/turn-secret.env)
    # The image's npm steps use your npm config when there is one, for networks that only allow a
    # company registry; it is a build secret, never stored in the image.
    npmrc=(); [ -f "$HOME/.npmrc" ] && npmrc=(--secret "id=npmrc,src=$HOME/.npmrc")
    # Written so an empty list also works in macOS's bash 3.2 under `set -u`.
    docker build ${npmrc[@]+"${npmrc[@]}"} -f docker/signalling.Dockerfile -t specdeck-signalling:local .
    scripts/local-kubectl.sh apply -k deploy/k8s/local
    # The image tag does not change between builds: restart to pick up the new image.
    scripts/local-kubectl.sh -n specdeck1-local rollout restart deployment/specdeck-signalling
    scripts/local-kubectl.sh -n specdeck1-local rollout status deployment/specdeck-signalling deployment/specdeck-turn --timeout=120s

# Remove the local Kubernetes deployment
local-down:
    scripts/local-kubectl.sh delete -k deploy/k8s/local --ignore-not-found

# Check that two peers connect through the local coturn with direct connections disabled
local-test: local-up
    #!/usr/bin/env bash
    set -euo pipefail
    scripts/local-kubectl.sh -n specdeck1-local port-forward svc/specdeck-signalling 18081:8080 >/dev/null &
    trap 'kill $! 2>/dev/null || true' EXIT
    until curl -sf http://127.0.0.1:18081/health >/dev/null; do sleep 0.25; done
    SPECDECK_SIGNALLING_URL=ws://127.0.0.1:18081/signal cargo test -p specdeck-core --test turn_relay -- --ignored --nocapture

# Local test setup: signalling and coturn on Colima's k3s, plus a host app and a viewer app (own settings each)
local-dev: local-up
    #!/usr/bin/env bash
    set -euo pipefail
    # Each background job gets its own process group, so Ctrl+C stops the whole tree under it
    # (tauri, vite, the apps), not just the job's first process. Background jobs then no longer
    # get /dev/null as input by themselves, so each is given it: one that reads the terminal stops.
    set -m
    stop() { trap - EXIT INT TERM; for pgid in $(jobs -p); do kill -TERM -- -"$pgid" 2>/dev/null || true; done; }
    trap stop EXIT
    trap 'exit 130' INT TERM
    scripts/local-kubectl.sh -n specdeck1-local port-forward svc/specdeck-signalling 18081:8080 >/dev/null </dev/null &
    until curl -sf http://127.0.0.1:18081/health >/dev/null; do sleep 0.25; done
    export SPECDECK_SIGNALLING_URL=ws://127.0.0.1:18081/signal
    mkdir -p .local-dev
    SPECDECK_PROFILE=host pnpm tauri dev </dev/null 2>&1 | tee .local-dev/host.log &
    # The viewer is a second instance of the same debug build, loading the host's dev server.
    until curl -sf http://localhost:5173 >/dev/null && pgrep -f target/debug/specdeck-app >/dev/null; do sleep 0.5; done
    SPECDECK_PROFILE=viewer target/debug/specdeck-app </dev/null 2>&1 | tee .local-dev/viewer.log &
    echo "SpecDeck (host) and SpecDeck (viewer) are starting. Close this with Ctrl+C."
    wait
