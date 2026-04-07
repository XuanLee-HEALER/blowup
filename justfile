# blowup v2 — Tauri desktop app for film management

set windows-shell := ["pwsh", "-NoProfile", "-Command"]

# Show available recipes
default:
    @just --list

# ── Development ───────────────────────────────────────────────────

# Start Tauri dev server (frontend + backend hot reload)
dev:
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
    cd src-tauri && cargo clippy -- -D warnings

# Rust format check
fmt-check:
    cd src-tauri && cargo fmt -- --check

# Rust format
fmt:
    cd src-tauri && cargo fmt

# ── Test ──────────────────────────────────────────────────────────

# Run Rust tests
test:
    cd src-tauri && cargo test

# Run a specific Rust test module
test-mod mod:
    cd src-tauri && cargo test --lib {{mod}} -- --nocapture

# ── Clean ─────────────────────────────────────────────────────────

# Clean build artifacts
clean:
    rm -rf dist
    cd src-tauri && cargo clean
