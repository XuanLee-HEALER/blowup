# blowup

> [中文版本点此链接](./README_zh.md)

![Version](https://img.shields.io/badge/Version-2.0.0-blue?style=for-the-badge) ![License](https://img.shields.io/badge/License-MIT-darkgreen?style=for-the-badge)

> **Blow-Up [Michelangelo Antonioni, 1966]**: A fashion photographer unknowingly captures a death on film after following two lovers in a park.
>
> The best movie I've seen so far.

---

**blowup** is a desktop application (and CLI tool) for the Chinese film-watching pipeline: TMDB discovery, torrent search & download, subtitle management, knowledge base curation, and media playback.

## v2 Desktop App

v2 is a full-featured **Tauri v2** desktop application with a React frontend. It wraps all CLI functionality in a native desktop GUI and adds a personal film knowledge base.

### Pages

| Page | Description |
|------|-------------|
| **Search** | Search and discover films via TMDB with filters (year, genre, rating, sort) |
| **People** | Knowledge base: directors, actors, crew — filmographies, wiki, relations |
| **Genres** | Knowledge base: hierarchical genre tree with film/person associations |
| **Graph** | D3 force-directed relationship graph of people and films |
| **My Library** | Local file management — scan directories, link files to films, stats |
| **Downloads** | Download manager — YTS torrent search, aria2c integration, queue tracking |
| **Subtitles** | Subtitle tools — OpenSubtitles fetch, alass alignment, extraction, time shift |
| **Media** | Media probe (ffprobe) and external player launch (mpv / system default) |
| **Settings** | Configuration for API keys, tool paths, music player |

### Building from source

```bash
# Prerequisites: Node.js 20+, Rust 1.80+, platform build tools for Tauri v2

# Install frontend dependencies
npm install

# Development mode
npm run tauri dev

# Production build
npm run tauri build
```

### Runtime dependencies

| Tool | Required for | Install |
|------|-------------|---------|
| `aria2c` | Downloads | `brew install aria2` / `apt install aria2` / `choco install aria2` |
| `alass` / `alass-cli` | Subtitle alignment | `brew install alass` / [GitHub releases](https://github.com/kaegi/alass/releases) |
| `ffmpeg` + `ffprobe` | Subtitle extraction, media probe | `brew install ffmpeg` / `choco install ffmpeg` |
| `mpv` (optional) | Media playback | `brew install mpv` / `choco install mpv` |

### Configuration

Settings are stored at `~/.config/blowup/config.toml`. Configure via the Settings page or:

```bash
# TMDB API key (free at themoviedb.org)
# Set via Settings page → TMDB section
```

| Key | Type | Description |
|-----|------|-------------|
| `tools.aria2c` | path | Path to aria2c binary |
| `tools.alass` | path | Path to alass binary |
| `tools.ffmpeg` | path | Path to ffmpeg binary |
| `tools.player` | path | Path to media player (default: mpv) |
| `tmdb.api_key` | string | TMDB API key |
| `opensubtitles.api_key` | string | OpenSubtitles API key (optional) |
| `subtitle.default_lang` | string | Default subtitle language (default: zh) |
| `library.root_dir` | path | Download/library directory (default: ~/Movies/blowup) |

---

## v1 CLI Tool

The original CLI is still available on crates.io:

```bash
cargo install blowup
```

### CLI Commands

| Command | Description |
|---------|-------------|
| `blowup search` | Search YIFY for movie torrents by title and year |
| `blowup download` | Download a torrent or magnet link via aria2c |
| `blowup info` | Look up movie details via TMDB |
| `blowup sub fetch` | Download subtitles from OpenSubtitles |
| `blowup sub align` | Auto-sync subtitle timing using alass |
| `blowup sub extract` | Extract embedded subtitle streams |
| `blowup sub list` | List subtitle streams in a video file |
| `blowup sub shift` | Shift subtitle timestamps by N milliseconds |
| `blowup tracker update` | Update local tracker list |
| `blowup config` | Read and write configuration |

### Quick Start (CLI)

```bash
# 1. Configure
blowup config set tmdb.api_key YOUR_TMDB_KEY
blowup config set tools.aria2c /usr/local/bin/aria2c

# 2. Find a movie
blowup info "Blow-Up" --year 1966

# 3. Search and download
blowup search "Blow-Up" --year 1966
blowup download "magnet:?xt=..." --output-dir ~/Movies

# 4. Subtitles
blowup sub fetch ~/Movies/Blow-Up.mp4 --lang zh
blowup sub align ~/Movies/Blow-Up.mp4 ~/Movies/Blow-Up.zh.srt
```

## License

MIT — see [LICENSE](./LICENSE.txt).
