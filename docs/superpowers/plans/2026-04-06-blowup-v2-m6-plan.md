# Blowup v2 M6: Media Player Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add media playback integration (mpv or system default) with a media info probe tool, play buttons in the Library page, and a dedicated Media tools page.

**Architecture:** New `open_in_player` command launches configured player (mpv by default, fallback to system default). New `probe_media_detail` command returns structured media info from ffprobe. Media page provides a file picker + detailed stream info display. Library page gains play buttons on linked files. Settings page gains player path field.

**Tech Stack:** mpv (configurable external player), ffprobe via existing `FfmpegTool` wrapper, React 19 + TypeScript

---

## Critical patterns

- **Player strategy:** Try configured `tools.player` first (default "mpv"). If not found, fall back to system default (`cmd /c start` on Windows, `open` on macOS, `xdg-open` on Linux).
- **No embedded player:** We launch an external process — no IPC, no in-app video rendering.
- **Existing probe_media** returns raw JSON. New `probe_media_detail` returns structured `MediaInfo` with parsed streams.
- **Settings page:** Tool fields use array mapping `["aria2c", "alass", "ffmpeg"].map(...)` — just add `"player"` to the array.

## File structure

| Action | File | Purpose |
|--------|------|---------|
| Modify | `src-tauri/src/config.rs` | Add `player` field to ToolsConfig |
| Modify | `src-tauri/src/commands/media.rs` | Add MediaInfo/StreamInfo types, `open_in_player`, `probe_media_detail` |
| Modify | `src-tauri/src/lib.rs` | Register 2 new commands |
| Modify | `src/lib/tauri.ts` | Add MediaInfo type + media wrappers, update AppConfig |
| Modify | `src/pages/Settings.tsx` | Add "player" to tools array |
| Modify | `src/pages/Library.tsx` | Add play button to linked files |
| Create | `src/pages/Media.tsx` | Media tools page with probe + play |
| Modify | `src/App.tsx` | Enable /media route |

---

### Task 1: Config + backend commands + register

**Files:**
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/commands/media.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add `player` to ToolsConfig in config.rs**

Read `src-tauri/src/config.rs`. Find `ToolsConfig` struct (around line 22) and add the `player` field:

```rust
#[derive(Debug, Deserialize, Serialize)]
pub struct ToolsConfig {
    #[serde(default = "default_aria2c")]
    pub aria2c: String,
    #[serde(default = "default_alass")]
    pub alass: String,
    #[serde(default = "default_ffmpeg")]
    pub ffmpeg: String,
    #[serde(default = "default_player")]
    pub player: String,
}
```

Add the default function near the other default functions:

```rust
fn default_player() -> String {
    "mpv".to_string()
}
```

Update the test `default_config_has_sane_values` to include:
```rust
assert_eq!(cfg.tools.player, "mpv");
```

- [ ] **Step 2: Add types and commands to media.rs**

Read `src-tauri/src/commands/media.rs`. Add imports and new types + commands after the existing `probe_media` command:

```rust
use serde::Serialize;
use crate::config::load_config;

// ── Types ────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct MediaInfo {
    pub file_path: String,
    pub file_size: Option<i64>,
    pub duration_secs: Option<f64>,
    pub format_name: Option<String>,
    pub bit_rate: Option<i64>,
    pub streams: Vec<StreamInfo>,
}

#[derive(Serialize)]
pub struct StreamInfo {
    pub index: i64,
    pub codec_type: String,
    pub codec_name: String,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub frame_rate: Option<String>,
    pub bit_rate: Option<i64>,
    pub channels: Option<i64>,
    pub sample_rate: Option<String>,
    pub language: Option<String>,
    pub title: Option<String>,
}

// ── Commands ─────────────────────────────────────────────────────

#[tauri::command]
pub async fn probe_media_detail(file_path: String) -> Result<MediaInfo, String> {
    let args: Vec<String> = vec![
        "-v", "quiet", "-print_format", "json", "-show_format", "-show_streams", "--", &file_path,
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();

    let (stdout, _) = FfmpegTool::Ffprobe
        .exec_with_options(None::<&str>, Some(args))
        .await
        .map_err(|e| e.to_string())?;

    let json: serde_json::Value =
        serde_json::from_str(&stdout).map_err(|e| format!("ffprobe parse error: {}", e))?;

    let format = &json["format"];
    let file_size = format["size"].as_str().and_then(|s| s.parse().ok());
    let duration_secs = format["duration"].as_str().and_then(|s| s.parse().ok());
    let format_name = format["format_name"].as_str().map(String::from);
    let bit_rate = format["bit_rate"].as_str().and_then(|s| s.parse().ok());

    let mut streams = Vec::new();
    if let Some(arr) = json["streams"].as_array() {
        for s in arr {
            streams.push(StreamInfo {
                index: s["index"].as_i64().unwrap_or(0),
                codec_type: s["codec_type"].as_str().unwrap_or("unknown").to_string(),
                codec_name: s["codec_name"].as_str().unwrap_or("unknown").to_string(),
                width: s["width"].as_i64(),
                height: s["height"].as_i64(),
                frame_rate: s["r_frame_rate"].as_str().map(String::from),
                bit_rate: s["bit_rate"].as_str().and_then(|s| s.parse().ok()),
                channels: s["channels"].as_i64(),
                sample_rate: s["sample_rate"].as_str().map(String::from),
                language: s["tags"]["language"].as_str().map(String::from),
                title: s["tags"]["title"].as_str().map(String::from),
            });
        }
    }

    Ok(MediaInfo {
        file_path,
        file_size,
        duration_secs,
        format_name,
        bit_rate,
        streams,
    })
}

#[tauri::command]
pub async fn open_in_player(file_path: String) -> Result<(), String> {
    let config = load_config();
    let player = config.tools.player.clone();

    if !player.is_empty() && which::which(&player).is_ok() {
        std::process::Command::new(&player)
            .arg(&file_path)
            .spawn()
            .map_err(|e| format!("启动播放器失败: {}", e))?;
        return Ok(());
    }

    open_with_system_default(&file_path)
}

fn open_with_system_default(file_path: &str) -> Result<(), String> {
    #[cfg(target_family = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/c", "start", "", file_path])
            .spawn()
            .map_err(|e| format!("打开文件失败: {}", e))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(file_path)
            .spawn()
            .map_err(|e| format!("打开文件失败: {}", e))?;
    }
    #[cfg(all(target_family = "unix", not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(file_path)
            .spawn()
            .map_err(|e| format!("打开文件失败: {}", e))?;
    }
    Ok(())
}
```

- [ ] **Step 3: Register commands in lib.rs**

Add after `commands::media::probe_media`:

```rust
commands::media::probe_media_detail,
commands::media::open_in_player,
```

- [ ] **Step 4: Run tests**

Run: `cd src-tauri && cargo test 2>&1`
Expected: All tests pass (~69 + updated config test)

- [ ] **Step 5: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src-tauri/src/config.rs src-tauri/src/commands/media.rs src-tauri/src/lib.rs
git commit -m "feat: add open_in_player and probe_media_detail commands"
```

---

### Task 2: tauri.ts + Settings.tsx updates

**Files:**
- Modify: `src/lib/tauri.ts`
- Modify: `src/pages/Settings.tsx`

- [ ] **Step 1: Update AppConfig and add types in tauri.ts**

Read `src/lib/tauri.ts`. Update the `AppConfig` interface — add `player` to the `tools` field:

```typescript
export interface AppConfig {
  tools: { aria2c: string; alass: string; ffmpeg: string; player: string };
  // ... rest stays the same
}
```

Add these types after `SubtitleStreamInfo`:

```typescript
export interface MediaInfo {
  file_path: string;
  file_size: number | null;
  duration_secs: number | null;
  format_name: string | null;
  bit_rate: number | null;
  streams: StreamInfo[];
}

export interface StreamInfo {
  index: number;
  codec_type: string;
  codec_name: string;
  width: number | null;
  height: number | null;
  frame_rate: string | null;
  bit_rate: number | null;
  channels: number | null;
  sample_rate: string | null;
  language: string | null;
  title: string | null;
}
```

Add a new export object after `subtitle`:

```typescript
export const media = {
  probeDetail: (filePath: string) =>
    invoke<MediaInfo>("probe_media_detail", { filePath }),
  openInPlayer: (filePath: string) =>
    invoke<void>("open_in_player", { filePath }),
};
```

- [ ] **Step 2: Add player field to Settings.tsx**

Read `src/pages/Settings.tsx`. Find the tools array (around line 109):

```typescript
{(["aria2c", "alass", "ffmpeg"] as const).map((tool) => (
```

Change to:

```typescript
{(["aria2c", "alass", "ffmpeg", "player"] as const).map((tool) => (
```

- [ ] **Step 3: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors

- [ ] **Step 4: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/lib/tauri.ts src/pages/Settings.tsx
git commit -m "feat: add media types, wrappers, and player config field"
```

---

### Task 3: Library page — play button

**Files:**
- Modify: `src/pages/Library.tsx`

- [ ] **Step 1: Read Library.tsx**

Read `src/pages/Library.tsx`. Find the `FilmDetailView` component, specifically where linked items render their action buttons (the "取消关联" and "移除" buttons).

- [ ] **Step 2: Add import**

Add `media` to the imports from tauri:

```typescript
import { library, media } from "../lib/tauri";
```

- [ ] **Step 3: Add play button**

In `FilmDetailView`, inside the `linkedItems.map(...)` render, find the action buttons div (the one with "取消关联" and "移除"). Add a play button BEFORE the "取消关联" button:

```tsx
<button
  onClick={() => media.openInPlayer(item.file_path)}
  style={{
    background: "var(--color-accent)",
    border: "none",
    borderRadius: 4,
    padding: "2px 8px",
    color: "#fff",
    cursor: "pointer",
    fontSize: 12,
  }}
>
  ▶ 播放
</button>
```

- [ ] **Step 4: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors

- [ ] **Step 5: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/pages/Library.tsx
git commit -m "feat: add play button to Library linked files"
```

---

### Task 4: Media page UI

**Files:**
- Create: `src/pages/Media.tsx`

The Media page provides a file picker, probe display, and play button.

- [ ] **Step 1: Create Media.tsx**

```tsx
import { useState } from "react";
import { media } from "../lib/tauri";
import type { MediaInfo, StreamInfo } from "../lib/tauri";
import { open } from "@tauri-apps/plugin-dialog";

// ── Helpers ──────────────────────────────────────────────────────

function formatSize(bytes: number | null): string {
  if (!bytes) return "—";
  if (bytes >= 1e9) return (bytes / 1e9).toFixed(2) + " GB";
  if (bytes >= 1e6) return (bytes / 1e6).toFixed(1) + " MB";
  return (bytes / 1e3).toFixed(0) + " KB";
}

function formatDuration(secs: number | null): string {
  if (!secs) return "—";
  const h = Math.floor(secs / 3600);
  const m = Math.floor((secs % 3600) / 60);
  const s = Math.floor(secs % 60);
  return h > 0 ? `${h}h${m}m${s}s` : `${m}m${s}s`;
}

function formatBitrate(bps: number | null): string {
  if (!bps) return "—";
  if (bps >= 1e6) return (bps / 1e6).toFixed(1) + " Mbps";
  return (bps / 1e3).toFixed(0) + " kbps";
}

function formatFrameRate(fr: string | null): string {
  if (!fr) return "—";
  const parts = fr.split("/");
  if (parts.length === 2) {
    const fps = parseFloat(parts[0]) / parseFloat(parts[1]);
    return fps.toFixed(2) + " fps";
  }
  return fr + " fps";
}

// ── Stream Card ──────────────────────────────────────────────────

function StreamCard({ stream }: { stream: StreamInfo }) {
  const isVideo = stream.codec_type === "video";
  const isAudio = stream.codec_type === "audio";
  const isSub = stream.codec_type === "subtitle";

  const icon = isVideo ? "🎬" : isAudio ? "🔊" : isSub ? "💬" : "📦";
  const typeLabel = isVideo ? "视频轨" : isAudio ? "音频轨" : isSub ? "字幕轨" : stream.codec_type;

  return (
    <div
      style={{
        background: "var(--color-bg-control)",
        borderRadius: 8,
        padding: 12,
        marginBottom: 8,
        fontSize: 13,
      }}
    >
      <div style={{ fontWeight: 500, marginBottom: 6 }}>
        {icon} #{stream.index} {typeLabel} — {stream.codec_name}
        {stream.language && ` (${stream.language})`}
        {stream.title && ` "${stream.title}"`}
      </div>
      <div
        style={{
          display: "flex",
          gap: 16,
          color: "var(--color-label-secondary)",
          fontSize: 12,
          flexWrap: "wrap",
        }}
      >
        {isVideo && stream.width && stream.height && (
          <span>{stream.width}x{stream.height}</span>
        )}
        {isVideo && <span>{formatFrameRate(stream.frame_rate)}</span>}
        {isAudio && stream.channels && (
          <span>{stream.channels}ch</span>
        )}
        {isAudio && stream.sample_rate && (
          <span>{stream.sample_rate} Hz</span>
        )}
        {stream.bit_rate && <span>{formatBitrate(stream.bit_rate)}</span>}
      </div>
    </div>
  );
}

// ── Main Page ────────────────────────────────────────────────────

export default function Media() {
  const [filePath, setFilePath] = useState("");
  const [info, setInfo] = useState<MediaInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState("");

  const handlePickFile = async () => {
    const path = await open({
      multiple: false,
      filters: [
        {
          name: "Media",
          extensions: [
            "mp4", "mkv", "avi", "mov", "ts", "webm", "m4v", "flv", "wmv",
            "mp3", "flac", "wav", "aac", "ogg", "m4a",
          ],
        },
      ],
    });
    if (!path) return;
    setFilePath(path as string);
    setInfo(null);
    setError("");
  };

  const handleProbe = async () => {
    if (!filePath) return;
    setLoading(true);
    setError("");
    try {
      const result = await media.probeDetail(filePath);
      setInfo(result);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const handlePlay = async () => {
    if (!filePath) return;
    try {
      await media.openInPlayer(filePath);
    } catch (e) {
      setError(String(e));
    }
  };

  const fileName = filePath ? filePath.split(/[/\\]/).pop() : "";

  const videoStreams = info?.streams.filter((s) => s.codec_type === "video") ?? [];
  const audioStreams = info?.streams.filter((s) => s.codec_type === "audio") ?? [];
  const subStreams = info?.streams.filter((s) => s.codec_type === "subtitle") ?? [];
  const otherStreams = info?.streams.filter(
    (s) => !["video", "audio", "subtitle"].includes(s.codec_type)
  ) ?? [];

  return (
    <div style={{ height: "100%", overflowY: "auto", padding: 24 }}>
      <h2 style={{ margin: "0 0 20px", fontSize: 18 }}>媒体工具</h2>

      <div style={{ maxWidth: 640 }}>
        {/* File picker + actions */}
        <div
          style={{
            display: "flex",
            gap: 8,
            alignItems: "center",
            marginBottom: 20,
          }}
        >
          <button
            onClick={handlePickFile}
            style={{
              background: "var(--color-bg-control)",
              border: "1px solid var(--color-separator)",
              borderRadius: 6,
              padding: "6px 16px",
              color: "var(--color-label-primary)",
              cursor: "pointer",
              fontSize: 13,
            }}
          >
            选择文件
          </button>
          <div
            style={{
              flex: 1,
              fontSize: 13,
              color: filePath ? "var(--color-label-primary)" : "var(--color-label-tertiary)",
              overflow: "hidden",
              textOverflow: "ellipsis",
              whiteSpace: "nowrap",
            }}
          >
            {fileName || "未选择文件"}
          </div>
          <button
            onClick={handleProbe}
            disabled={!filePath || loading}
            style={{
              background: "var(--color-accent)",
              color: "#fff",
              border: "none",
              borderRadius: 6,
              padding: "6px 16px",
              cursor: !filePath || loading ? "not-allowed" : "pointer",
              fontSize: 13,
              opacity: !filePath || loading ? 0.5 : 1,
            }}
          >
            {loading ? "探测中..." : "探测"}
          </button>
          <button
            onClick={handlePlay}
            disabled={!filePath}
            style={{
              background: "var(--color-bg-control)",
              border: "1px solid var(--color-separator)",
              borderRadius: 6,
              padding: "6px 16px",
              color: "var(--color-label-primary)",
              cursor: !filePath ? "not-allowed" : "pointer",
              fontSize: 13,
              opacity: !filePath ? 0.5 : 1,
            }}
          >
            ▶ 播放
          </button>
        </div>

        {error && (
          <div style={{ color: "#e53935", fontSize: 13, marginBottom: 16 }}>
            {error}
          </div>
        )}

        {info && (
          <>
            {/* File info */}
            <div
              style={{
                background: "var(--color-bg-control)",
                borderRadius: 10,
                padding: 16,
                marginBottom: 16,
                fontSize: 13,
              }}
            >
              <h3 style={{ margin: "0 0 10px", fontSize: 14 }}>文件信息</h3>
              <div
                style={{
                  display: "grid",
                  gridTemplateColumns: "1fr 1fr",
                  gap: 8,
                  color: "var(--color-label-secondary)",
                }}
              >
                <div>格式: {info.format_name ?? "—"}</div>
                <div>大小: {formatSize(info.file_size)}</div>
                <div>时长: {formatDuration(info.duration_secs)}</div>
                <div>比特率: {formatBitrate(info.bit_rate)}</div>
              </div>
            </div>

            {/* Streams by type */}
            {videoStreams.length > 0 && (
              <div style={{ marginBottom: 16 }}>
                <h3 style={{ fontSize: 14, marginBottom: 8 }}>
                  视频轨 ({videoStreams.length})
                </h3>
                {videoStreams.map((s) => (
                  <StreamCard key={s.index} stream={s} />
                ))}
              </div>
            )}

            {audioStreams.length > 0 && (
              <div style={{ marginBottom: 16 }}>
                <h3 style={{ fontSize: 14, marginBottom: 8 }}>
                  音频轨 ({audioStreams.length})
                </h3>
                {audioStreams.map((s) => (
                  <StreamCard key={s.index} stream={s} />
                ))}
              </div>
            )}

            {subStreams.length > 0 && (
              <div style={{ marginBottom: 16 }}>
                <h3 style={{ fontSize: 14, marginBottom: 8 }}>
                  字幕轨 ({subStreams.length})
                </h3>
                {subStreams.map((s) => (
                  <StreamCard key={s.index} stream={s} />
                ))}
              </div>
            )}

            {otherStreams.length > 0 && (
              <div style={{ marginBottom: 16 }}>
                <h3 style={{ fontSize: 14, marginBottom: 8 }}>
                  其他 ({otherStreams.length})
                </h3>
                {otherStreams.map((s) => (
                  <StreamCard key={s.index} stream={s} />
                ))}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors

- [ ] **Step 3: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/pages/Media.tsx
git commit -m "feat: add Media tools page with probe detail and player launch"
```

---

### Task 5: App.tsx unlock /media + final build

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Read App.tsx**

- [ ] **Step 2: Make changes**

1. Add import: `import Media from "./pages/Media";`
2. In NAV_SECTIONS, find `{ icon: "▶", label: "媒体", path: "/media", disabled: true }` and remove `disabled: true`.
3. In Routes, replace `/media` Placeholder with: `<Route path="/media" element={<Media />} />`
4. Do NOT add /media to KB_PATHS.

- [ ] **Step 3: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`

- [ ] **Step 4: Full frontend build**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npm run build 2>&1`

- [ ] **Step 5: Full Rust test suite**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup/src-tauri && cargo test 2>&1`

- [ ] **Step 6: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/App.tsx
git commit -m "feat: unlock /media route — M6 complete"
```

---

## Self-review checklist

- [x] **Spec coverage:** Player launch (mpv + fallback), media probe detail, play button in Library, Media tools page, Settings player config
- [x] **No placeholders:** Every task has complete code
- [x] **Type consistency:** `MediaInfo`/`StreamInfo` match across Rust and TS
- [x] **Config consistency:** `tools.player` field in ToolsConfig, AppConfig, and Settings array
- [x] **Platform handling:** `open_with_system_default` covers Windows/macOS/Linux
- [x] **No KB_PATHS change:** /media is a "工具" route
