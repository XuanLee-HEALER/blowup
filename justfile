# blowup v2 — Tauri desktop app for film management

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

# Start Tauri dev server (frontend + backend hot reload)
dev: _ensure-dev-dlls
    bunx tauri dev

# Start frontend only (Vite dev server)
dev-web:
    bun run dev

# ── Build ─────────────────────────────────────────────────────────

# Production build (Tauri installer)
build:
    bunx tauri build

# Frontend build only (Vite)
build-web:
    bun run build

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

# Run a specific Rust test module
test-mod mod:
    cargo test -p blowup-tauri --lib {{mod}} -- --nocapture

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

# Clean build artifacts
clean:
    rm -rf dist
    cargo clean
