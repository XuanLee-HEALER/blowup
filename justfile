# blowup v2 — desktop app (Tauri + embedded HTTP) and standalone HTTP server
#
# Two runtime modes share the same blowup-core business logic:
#
#   Desktop  — `just dev` / `just build`
#              Tauri WebView + React frontend talks to blowup-core via
#              Tauri IPC `invoke()`. The same process also starts the
#              blowup-server axum router on 127.0.0.1:17690 so LAN
#              clients (iPad, iPhone, …) share the same DB + library +
#              torrent session + event stream.
#
#   Server   — `just dev-server` / `just build-server`
#              Headless blowup-server binary. Just HTTP + SSE, no
#              WebView, no libmpv, no child windows. Suitable for
#              NAS / Mac mini / Raspberry Pi deployment.

set windows-shell := ["pwsh", "-NoProfile", "-Command"]

# Show available recipes
default:
    @just --list

# ── Development ───────────────────────────────────────────────────

# Ensure runtime DLLs are in target/debug for dev mode
[windows]
_ensure-dev-dlls:
    if (-not (Test-Path "target\debug\libmpv-2.dll")) { Copy-Item "crates\tauri\lib\libmpv-2.dll" "target\debug\libmpv-2.dll" }

[macos]
_ensure-dev-dlls:

[linux]
_ensure-dev-dlls:

# Desktop dev — Tauri WebView + React + in-process HTTP server
dev: _ensure-dev-dlls
    bunx tauri dev

# Headless blowup-server dev — no WebView, no libmpv (env: BLOWUP_DATA_DIR, BLOWUP_SERVER_BIND)
dev-server:
    cargo run -p blowup-server

# Headless blowup-server dev with auto-restart (requires `cargo install cargo-watch`)
dev-server-watch:
    cargo watch -x 'run -p blowup-server'

# Start frontend only (Vite dev server)
dev-web:
    bun run dev

# ── Build ─────────────────────────────────────────────────────────

# Desktop production build (Tauri installer — bundles libmpv + frontend)
build:
    bunx tauri build

# Standalone server release binary (target/release/blowup-server[.exe])
build-server:
    cargo build -p blowup-server --release

# Frontend build only (Vite)
build-web:
    bun run build

# ── Run (release) ─────────────────────────────────────────────────

# Run the compiled blowup-server release binary (after `just build-server`)
run-server:
    cargo run -p blowup-server --release

# ── Quality ───────────────────────────────────────────────────────

# Run all checks (lint + typecheck + clippy + fmt + test)
check: lint typecheck clippy fmt-check test

# TypeScript type check
typecheck:
    bunx tsc --noEmit

# ESLint
lint:
    bunx eslint src/

# ESLint with auto-fix
lint-fix:
    bunx eslint src/ --fix

# Rust clippy (warnings as errors)
clippy:
    cargo clippy --workspace --tests -- -D warnings

# Rust format check
fmt-check:
    cargo fmt --all -- --check

# Rust format
fmt:
    cargo fmt --all

# ── Test ──────────────────────────────────────────────────────────

# Run Rust tests
test:
    cargo test --workspace

# Run a specific Rust test module inside blowup-core
test-mod mod:
    cargo test -p blowup-core --lib {{mod}} -- --nocapture

# ── Install ───────────────────────────────────────────────────────

# Build and install to /Applications (macOS)
[macos]
install: build
    #!/usr/bin/env bash
    set -euo pipefail
    DMG=$(find target/release/bundle/dmg -name "*.dmg" -maxdepth 1 | head -1)
    if [ -z "$DMG" ]; then echo "No .dmg found"; exit 1; fi
    MOUNT=$(hdiutil attach "$DMG" -nobrowse | grep "/Volumes" | awk -F'\t' '{print $NF}')
    APP=$(find "$MOUNT" -name "*.app" -maxdepth 1 | head -1)
    rm -rf "/Applications/$(basename "$APP")"
    cp -R "$APP" /Applications/
    hdiutil detach "$MOUNT" -quiet
    sudo xattr -rd com.apple.quarantine "/Applications/$(basename "$APP")"
    echo "Installed $(basename "$APP") to /Applications"

# ── Clean ─────────────────────────────────────────────────────────

# Clean ALL build and dev caches: Vite dev cache, frontend dist, cargo target
[windows]
clean:
    if (Test-Path dist) { Remove-Item -Recurse -Force dist }
    if (Test-Path node_modules/.vite) { Remove-Item -Recurse -Force node_modules/.vite }
    cargo clean

[macos]
clean:
    rm -rf dist node_modules/.vite
    cargo clean

[linux]
clean:
    rm -rf dist node_modules/.vite
    cargo clean

# Clean only Vite's dev cache (fixes stale HMR / multi-entry module graph)
[windows]
clean-vite:
    if (Test-Path node_modules/.vite) { Remove-Item -Recurse -Force node_modules/.vite }

[macos]
clean-vite:
    rm -rf node_modules/.vite

[linux]
clean-vite:
    rm -rf node_modules/.vite
