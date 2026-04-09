# blowup

> [中文版本点此链接](./README_zh.md)

![Version](https://img.shields.io/badge/Version-2.0.4-blue?style=for-the-badge) ![License](https://img.shields.io/badge/License-MIT-darkgreen?style=for-the-badge)

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

### Pages

| Page | Description |
|------|-------------|
| **Search** | Search and discover films via TMDB with filters (year, genre, rating, sort) |
| **Wiki** | Unified knowledge base — every concept is an entry with tags and relations |
| **Graph** | D3 force-directed knowledge graph with directed edges and multi-link support |
| **Library** | Film library — director tree view, TMDB enrichment, poster/credits/overview |
| **Download** | Download manager — YTS torrent search, file selection, pause/resume |
| **Darkroom** | Subtitle tools + media probe — extraction, alignment, time shift, audio waveform |
| **Settings** | Configuration for API keys, tool paths, sync, music player |

### Architecture

Data flow is **event-driven**: backend mutations emit domain events (`downloads:changed`, `library:changed`, `entries:changed`, `config:changed`), frontend listens and re-fetches — no polling.

Two independent data systems:
- **Knowledge Base** (SQLite): unified entry model with tags and open-ended relations
- **Film Library** (file system + JSON index): `{root}/{director}/{tmdb_id}/` directories, in-memory index with lazy TMDB enrichment

### Building from source

```bash
# Prerequisites: Bun, Rust 1.80+, platform build tools for Tauri v2
# macOS: brew install mpv (for libmpv)
# Linux: sudo apt install libmpv-dev libwebkit2gtk-4.1-dev

bun install
just dev    # Development mode
just build  # Production build
just check  # Lint + typecheck + test
```

### Runtime dependencies

| Tool | Required for | Install |
|------|-------------|---------|
| `ffmpeg` + `ffprobe` | Subtitle extraction, media probe, library scan | `brew install ffmpeg` / `choco install ffmpeg` |
| `alass` / `alass-cli` | Subtitle alignment (optional) | `brew install alass` / [GitHub releases](https://github.com/kaegi/alass/releases) |

Tool paths are auto-detected on startup from PATH and well-known directories.

### Configuration

Settings are stored at `{APP_DATA_DIR}/config.toml`. Configure via the Settings page.

| Key | Type | Description |
|-----|------|-------------|
| `tools.alass` | path | Path to alass binary |
| `tools.ffmpeg` | path | Path to ffmpeg binary |
| `tmdb.api_key` | string | TMDB API key (free at themoviedb.org) |
| `opensubtitles.api_key` | string | OpenSubtitles API key (optional) |
| `subtitle.default_lang` | string | Default subtitle language (default: zh) |
| `library.root_dir` | path | Film library directory (default: ~/Movies/blowup) |
| `download.max_concurrent` | number | Max concurrent downloads (default: 3) |

## Legal

- **Film resources** come from [YTS/YIFY](https://yts.mx), which indexes public-domain films whose copyrights have expired.
- **All integrated tools** — ffmpeg, alass, mpv — are open-source projects under their own licenses.
- **blowup does not store, host, or distribute** any copyrighted content. It is a client-side tool that helps users organize and view films they have legally obtained.
- Users are responsible for complying with the laws and regulations of their own jurisdiction.

## License

MIT — see [LICENSE](./LICENSE.txt).
