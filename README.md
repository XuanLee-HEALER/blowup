# blowup

> [中文版本点此链接](./README_zh.md)

![Version](https://img.shields.io/badge/Version-2.0.7-blue?style=for-the-badge) ![License](https://img.shields.io/badge/License-MIT-darkgreen?style=for-the-badge)

> **Blow-Up [Michelangelo Antonioni, 1966]**: A fashion photographer unknowingly captures a death on film after following two lovers in a park.
>
> The best movie I've seen so far.

---

**blowup** is an all-in-one desktop app for classic film enthusiasts — a personal knowledge base combined with a complete viewing workflow including discovery, download, subtitle management, and media playback.

Search and discover films via TMDB, build your own knowledge graph (entries + tags + relations), fetch torrents from YTS, manage subtitles, probe and play media with the built-in mpv player — all in one place.

Built with **Tauri v2** (Rust backend + SQLite) and **React 19** (TypeScript frontend). The entire toolchain is open-source.

**Platform support:** macOS (Apple Silicon), Windows, and Linux.

### Install

**macOS (Homebrew):**

```bash
brew tap XuanLee-HEALER/tap
brew install --cask blowup
```

**Other platforms:** Download from [GitHub Releases](https://github.com/XuanLee-HEALER/blowup/releases/latest).

## Workflow: From Discovery to Playback

A typical workflow in blowup, end to end:

**1. Discover** — Open the **Search** page. Enter a film title or browse by genre/year/rating via TMDB. Click a result to see details, cast, and available torrents.

**2. Download** — Pick a torrent quality and select which files to download. The **Download** page tracks progress in real-time. When complete, the film appears in your **Library** automatically.

**3. Subtitles** — Go to the **Darkroom**. Search for subtitles via OpenSubtitles or ASSRT (Shooter). Embedded subtitle tracks are auto-extracted on download. Use the built-in aligner to sync subtitles against the audio.

**4. Play** — Back in the **Library**, select your subtitle files — configure color, font size, and vertical position for each. Multiple subtitles display simultaneously (e.g. Chinese bottom + English top). Hit play to watch with the built-in mpv player.

**5. Knowledge** — Record what you learn in the **Wiki**. Create entries for directors, films, genres — anything. Link them with custom relations. The **Graph** page visualizes your growing knowledge network.

### Pages

| Page | Description |
|------|-------------|
| **Search** | Search and discover films via TMDB with filters (year, genre, rating, sort) |
| **Wiki** | Unified knowledge base — every concept is an entry with tags and relations |
| **Graph** | D3 force-directed knowledge graph with directed edges and multi-link support |
| **Library** | Film library — director tree view, TMDB enrichment, poster/credits/overview, multi-subtitle overlay |
| **Download** | Download manager — YTS torrent search, file selection, pause/resume |
| **Darkroom** | Subtitle tools + media probe — extraction, alignment, time shift, audio waveform |
| **Settings** | Configuration for API keys, tool paths, sync, music player |

### Architecture

3-crate Rust workspace — see [`docs/REFACTOR.md`](./docs/REFACTOR.md) for the rationale.

- **`blowup-core`** — pure business logic (torrent, subtitle, tmdb, library, ...). Zero Tauri/HTTP coupling.
- **`blowup-server`** — `axum` HTTP wrapper around `blowup-core`. Runs headless, or in-process inside the Tauri app on `127.0.0.1:17690` so a LAN iPad can share the same DB + library + torrent session.
- **`blowup-tauri`** — desktop adapter (Tauri commands + embedded mpv player + native windows).

Data flow is **event-driven**: backend mutations emit domain events (`downloads:changed`, `library:changed`, `entries:changed`, `config:changed`, `tasks:changed`), frontend listens and re-fetches — no polling.

Two independent data systems:
- **Knowledge Base** (SQLite): unified entry model with tags and open-ended relations
- **Film Library** (file system + JSON index): `{root}/{director}/{tmdb_id}/` directories, in-memory index with lazy TMDB enrichment

### Building from source

```bash
# Prerequisites: Bun, Rust 1.80+, platform build tools for Tauri v2
# macOS: brew install mpv (for libmpv)
# Linux: sudo apt install libmpv-dev libwebkit2gtk-4.1-dev

bun install
just dev          # Desktop dev (Tauri + Vite, hot reload)
just dev-server   # Headless HTTP server only (no WebView, no libmpv)
just build        # Tauri installer
just build-server # Standalone server release binary
just check        # Lint + typecheck + clippy + fmt + test
```

### Runtime dependencies

| Tool | Required for | Install |
|------|-------------|---------|
| `ffmpeg` + `ffprobe` | Subtitle extraction, media probe, library scan | `brew install ffmpeg` / `choco install ffmpeg` |

Subtitle alignment is built-in (no external `alass` binary needed). Tool paths are auto-detected on startup.

### Configuration

Settings are stored at `{APP_DATA_DIR}/config.toml`. Configure via the Settings page.

| Key | Type | Description |
|-----|------|-------------|
| `tools.ffmpeg` | path | Path to ffmpeg binary |
| `tmdb.api_key` | string | TMDB API key (free at themoviedb.org) |
| `opensubtitles.api_key` | string | OpenSubtitles API key (optional) |
| `assrt.token` | string | ASSRT (Shooter) token (optional) |
| `subtitle.default_lang` | string | Default subtitle language (default: zh) |
| `library.root_dir` | path | Film library directory (default: ~/Movies/blowup) |
| `download.max_concurrent` | number | Max concurrent downloads (default: 3) |

## Legal

- **Film resources** come from [YTS/YIFY](https://yts.mx), which indexes public-domain films whose copyrights have expired.
- **All integrated tools** — ffmpeg, mpv — are open-source projects under their own licenses.
- **blowup does not store, host, or distribute** any copyrighted content. It is a client-side tool that helps users organize and view films they have legally obtained.
- Users are responsible for complying with the laws and regulations of their own jurisdiction.

## License

MIT — see [LICENSE](./LICENSE.txt).
