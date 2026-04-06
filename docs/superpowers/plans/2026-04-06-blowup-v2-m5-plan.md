# Blowup v2 M5: Subtitle Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Subtitle tools page with UI for all 5 existing subtitle operations — fetch from OpenSubtitles, align with alass, extract embedded tracks, list streams, and manual time shift.

**Architecture:** Pure frontend milestone — all 5 Rust commands are already implemented and registered (`fetch_subtitle_cmd`, `align_subtitle_cmd`, `extract_subtitle_cmd`, `list_subtitle_streams_cmd`, `shift_subtitle_cmd`). M5 adds TypeScript invoke wrappers and a single Subtitle page with card-based sections for each tool, using Tauri file dialogs for file selection.

**Tech Stack:** React 19 + TypeScript, @tauri-apps/plugin-dialog (file picker), existing Tauri subtitle commands

---

## Critical patterns

- All 5 subtitle commands are **already registered** in `lib.rs` — no backend changes needed
- File dialogs: `import { open } from "@tauri-apps/plugin-dialog"`
- The `fetch_subtitle_cmd` takes `(video, lang, _api_key)` — the `_api_key` param exists but is unused (OpenSubtitles XML-RPC uses anonymous login). Pass empty string.
- The `align_subtitle_cmd` takes `(video, srt)` — both are file paths
- The `shift_subtitle_cmd` takes `(srt, offset_ms)` — offset is i64 in milliseconds (positive = delay, negative = advance)
- The `list_subtitle_streams_cmd` returns `Vec<SubtitleStreamInfo>` with fields: `index: u32`, `codec_name: String`, `duration: u32`, `language: Option<String>`, `title: Option<String>`

## File structure

| Action | File | Purpose |
|--------|------|---------|
| Modify | `src/lib/tauri.ts` | Add SubtitleStreamInfo type + subtitle invoke wrappers |
| Create | `src/pages/Subtitle.tsx` | Subtitle tools page with 4 tool sections |
| Modify | `src/App.tsx` | Enable /subtitle route |

---

### Task 1: tauri.ts — subtitle types and wrappers

**Files:**
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Read `tauri.ts`**

Read `src/lib/tauri.ts` to see current structure and where to add the new exports.

- [ ] **Step 2: Add SubtitleStreamInfo type**

Add after the existing `MovieResult` interface:

```typescript
export interface SubtitleStreamInfo {
  index: number;
  codec_name: string;
  duration: number;
  language: string | null;
  title: string | null;
}
```

- [ ] **Step 3: Add subtitle invoke wrappers**

Add a new export object after the `tracker` object:

```typescript
export const subtitle = {
  fetch: (video: string, lang: string) =>
    invoke<void>("fetch_subtitle_cmd", { video, lang, apiKey: "" }),
  align: (video: string, srt: string) =>
    invoke<void>("align_subtitle_cmd", { video, srt }),
  extract: (video: string, stream?: number) =>
    invoke<void>("extract_subtitle_cmd", { video, stream }),
  listStreams: (video: string) =>
    invoke<SubtitleStreamInfo[]>("list_subtitle_streams_cmd", { video }),
  shift: (srt: string, offsetMs: number) =>
    invoke<void>("shift_subtitle_cmd", { srt, offsetMs }),
};
```

- [ ] **Step 4: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors

- [ ] **Step 5: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/lib/tauri.ts
git commit -m "feat: add subtitle types and invoke wrappers in tauri.ts"
```

---

### Task 2: Subtitle page UI

**Files:**
- Create: `src/pages/Subtitle.tsx`

The page has 4 tool sections as cards, each with file picker inputs and action buttons. Layout is a scrollable single-column page.

**Sections:**
1. **搜索字幕** — pick video → select language → fetch from OpenSubtitles
2. **对齐字幕** — pick video + pick SRT → align with alass
3. **提取字幕** — pick video → list embedded streams → extract selected stream
4. **时间偏移** — pick SRT → enter offset in ms → apply shift

- [ ] **Step 1: Read existing page patterns**

Read `src/pages/Download.tsx` (lines 1-30) for the established page patterns.

- [ ] **Step 2: Create Subtitle.tsx**

```tsx
import { useState } from "react";
import { subtitle } from "../lib/tauri";
import type { SubtitleStreamInfo } from "../lib/tauri";
import { open } from "@tauri-apps/plugin-dialog";

// ── Shared styles ────────────────────────────────────────────────

const cardStyle: React.CSSProperties = {
  background: "var(--color-bg-control)",
  borderRadius: 10,
  padding: 20,
  marginBottom: 16,
};

const labelStyle: React.CSSProperties = {
  fontSize: 12,
  color: "var(--color-label-secondary)",
  display: "block",
  marginBottom: 4,
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "6px 10px",
  borderRadius: 6,
  border: "1px solid var(--color-separator)",
  background: "var(--color-bg-primary)",
  color: "var(--color-label-primary)",
  fontSize: 13,
  boxSizing: "border-box" as const,
};

const btnStyle: React.CSSProperties = {
  background: "var(--color-accent)",
  color: "#fff",
  border: "none",
  borderRadius: 6,
  padding: "6px 16px",
  cursor: "pointer",
  fontSize: 13,
};

const btnSecondaryStyle: React.CSSProperties = {
  background: "var(--color-bg-primary)",
  border: "1px solid var(--color-separator)",
  borderRadius: 6,
  padding: "6px 16px",
  color: "var(--color-label-primary)",
  cursor: "pointer",
  fontSize: 13,
};

const fileRowStyle: React.CSSProperties = {
  display: "flex",
  gap: 8,
  alignItems: "center",
  marginBottom: 10,
};

const VIDEO_FILTERS = [
  { name: "Video", extensions: ["mp4", "mkv", "avi", "mov", "ts", "webm", "m4v", "flv", "wmv"] },
];

const SRT_FILTERS = [
  { name: "Subtitle", extensions: ["srt", "ass", "ssa", "sub", "vtt"] },
];

// ── File Picker Row ──────────────────────────────────────────────

function FilePickerRow({
  label,
  value,
  onChange,
  filters,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  filters: { name: string; extensions: string[] }[];
}) {
  const handlePick = async () => {
    const path = await open({ multiple: false, filters });
    if (path) onChange(path as string);
  };
  const fileName = value ? value.split(/[/\\]/).pop() : "";
  return (
    <div style={fileRowStyle}>
      <label style={{ ...labelStyle, marginBottom: 0, minWidth: 60 }}>{label}</label>
      <div
        style={{
          flex: 1,
          fontSize: 13,
          color: value ? "var(--color-label-primary)" : "var(--color-label-tertiary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {fileName || "未选择"}
      </div>
      <button onClick={handlePick} style={btnSecondaryStyle}>
        选择
      </button>
    </div>
  );
}

// ── Status Message ───────────────────────────────────────────────

function StatusMsg({ status }: { status: { ok: boolean; msg: string } | null }) {
  if (!status) return null;
  return (
    <div
      style={{
        marginTop: 10,
        fontSize: 13,
        color: status.ok ? "#4caf50" : "#e53935",
      }}
    >
      {status.ok ? "✓ " : "✗ "}
      {status.msg}
    </div>
  );
}

// ── 1. Fetch Subtitle ────────────────────────────────────────────

function FetchSection() {
  const [video, setVideo] = useState("");
  const [lang, setLang] = useState("zh");
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; msg: string } | null>(null);

  const handleFetch = async () => {
    if (!video) return;
    setLoading(true);
    setStatus(null);
    try {
      await subtitle.fetch(video, lang);
      setStatus({ ok: true, msg: "字幕下载成功" });
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={cardStyle}>
      <h3 style={{ margin: "0 0 12px", fontSize: 15 }}>搜索字幕</h3>
      <p style={{ fontSize: 12, color: "var(--color-label-tertiary)", margin: "0 0 12px" }}>
        从 OpenSubtitles 搜索并下载字幕，保存到视频文件同目录
      </p>
      <FilePickerRow label="视频" value={video} onChange={setVideo} filters={VIDEO_FILTERS} />
      <div style={fileRowStyle}>
        <label style={{ ...labelStyle, marginBottom: 0, minWidth: 60 }}>语言</label>
        <select
          value={lang}
          onChange={(e) => setLang(e.target.value)}
          style={{ ...inputStyle, width: "auto", flex: 1 }}
        >
          <option value="zh">中文</option>
          <option value="en">English</option>
          <option value="ja">日本語</option>
          <option value="ko">한국어</option>
          <option value="fr">Français</option>
          <option value="de">Deutsch</option>
          <option value="es">Español</option>
        </select>
      </div>
      <button onClick={handleFetch} disabled={!video || loading} style={{ ...btnStyle, opacity: !video || loading ? 0.5 : 1 }}>
        {loading ? "搜索中..." : "搜索并下载"}
      </button>
      <StatusMsg status={status} />
    </div>
  );
}

// ── 2. Align Subtitle ────────────────────────────────────────────

function AlignSection() {
  const [video, setVideo] = useState("");
  const [srt, setSrt] = useState("");
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; msg: string } | null>(null);

  const handleAlign = async () => {
    if (!video || !srt) return;
    setLoading(true);
    setStatus(null);
    try {
      await subtitle.align(video, srt);
      setStatus({ ok: true, msg: "字幕对齐完成" });
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={cardStyle}>
      <h3 style={{ margin: "0 0 12px", fontSize: 15 }}>对齐字幕</h3>
      <p style={{ fontSize: 12, color: "var(--color-label-tertiary)", margin: "0 0 12px" }}>
        使用 alass 自动对齐字幕时间轴（需要安装 alass）
      </p>
      <FilePickerRow label="视频" value={video} onChange={setVideo} filters={VIDEO_FILTERS} />
      <FilePickerRow label="字幕" value={srt} onChange={setSrt} filters={SRT_FILTERS} />
      <button onClick={handleAlign} disabled={!video || !srt || loading} style={{ ...btnStyle, opacity: !video || !srt || loading ? 0.5 : 1 }}>
        {loading ? "对齐中..." : "开始对齐"}
      </button>
      <StatusMsg status={status} />
    </div>
  );
}

// ── 3. Extract Subtitle ──────────────────────────────────────────

function ExtractSection() {
  const [video, setVideo] = useState("");
  const [streams, setStreams] = useState<SubtitleStreamInfo[]>([]);
  const [selectedStream, setSelectedStream] = useState<number | undefined>();
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; msg: string } | null>(null);

  const handleListStreams = async () => {
    if (!video) return;
    setLoading(true);
    setStatus(null);
    setStreams([]);
    setSelectedStream(undefined);
    try {
      const result = await subtitle.listStreams(video);
      setStreams(result);
      if (result.length === 0) {
        setStatus({ ok: false, msg: "未找到字幕轨道" });
      } else {
        setSelectedStream(result[0].index);
      }
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  const handleExtract = async () => {
    if (!video) return;
    setLoading(true);
    setStatus(null);
    try {
      await subtitle.extract(video, selectedStream);
      setStatus({ ok: true, msg: "字幕提取成功" });
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={cardStyle}>
      <h3 style={{ margin: "0 0 12px", fontSize: 15 }}>提取字幕</h3>
      <p style={{ fontSize: 12, color: "var(--color-label-tertiary)", margin: "0 0 12px" }}>
        从视频文件中提取内嵌字幕轨道为 SRT 文件（需要 ffmpeg）
      </p>
      <FilePickerRow label="视频" value={video} onChange={(v) => { setVideo(v); setStreams([]); setSelectedStream(undefined); setStatus(null); }} filters={VIDEO_FILTERS} />
      <div style={{ display: "flex", gap: 8, marginBottom: 10 }}>
        <button onClick={handleListStreams} disabled={!video || loading} style={{ ...btnSecondaryStyle, opacity: !video || loading ? 0.5 : 1 }}>
          列出字幕轨
        </button>
      </div>

      {streams.length > 0 && (
        <div style={{ marginBottom: 10 }}>
          <label style={labelStyle}>选择轨道</label>
          <select
            value={selectedStream ?? ""}
            onChange={(e) => setSelectedStream(Number(e.target.value))}
            style={{ ...inputStyle, width: "auto" }}
          >
            {streams.map((s) => (
              <option key={s.index} value={s.index}>
                #{s.index} — {s.codec_name} {s.language ? `(${s.language})` : ""} {s.title ?? ""}
              </option>
            ))}
          </select>
        </div>
      )}

      {streams.length > 0 && (
        <button onClick={handleExtract} disabled={loading} style={{ ...btnStyle, opacity: loading ? 0.5 : 1 }}>
          {loading ? "提取中..." : "提取"}
        </button>
      )}
      <StatusMsg status={status} />
    </div>
  );
}

// ── 4. Shift Subtitle ────────────────────────────────────────────

function ShiftSection() {
  const [srt, setSrt] = useState("");
  const [offsetMs, setOffsetMs] = useState(0);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; msg: string } | null>(null);

  const handleShift = async () => {
    if (!srt || offsetMs === 0) return;
    setLoading(true);
    setStatus(null);
    try {
      await subtitle.shift(srt, offsetMs);
      setStatus({ ok: true, msg: `偏移 ${offsetMs > 0 ? "+" : ""}${offsetMs}ms 完成` });
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={cardStyle}>
      <h3 style={{ margin: "0 0 12px", fontSize: 15 }}>时间偏移</h3>
      <p style={{ fontSize: 12, color: "var(--color-label-tertiary)", margin: "0 0 12px" }}>
        手动调整 SRT 字幕时间轴（正数延后，负数提前）
      </p>
      <FilePickerRow label="字幕" value={srt} onChange={setSrt} filters={SRT_FILTERS} />
      <div style={fileRowStyle}>
        <label style={{ ...labelStyle, marginBottom: 0, minWidth: 60 }}>偏移量</label>
        <input
          type="number"
          value={offsetMs}
          onChange={(e) => setOffsetMs(Number(e.target.value))}
          style={{ ...inputStyle, width: 120, flex: "none" }}
        />
        <span style={{ fontSize: 12, color: "var(--color-label-secondary)" }}>毫秒</span>
      </div>
      <div style={{ display: "flex", gap: 8, marginBottom: 4 }}>
        {[-5000, -1000, -500, 500, 1000, 5000].map((v) => (
          <button
            key={v}
            onClick={() => setOffsetMs((prev) => prev + v)}
            style={{
              ...btnSecondaryStyle,
              padding: "2px 8px",
              fontSize: 11,
            }}
          >
            {v > 0 ? `+${v}` : v}
          </button>
        ))}
      </div>
      <button onClick={handleShift} disabled={!srt || offsetMs === 0 || loading} style={{ ...btnStyle, marginTop: 8, opacity: !srt || offsetMs === 0 || loading ? 0.5 : 1 }}>
        {loading ? "处理中..." : "应用偏移"}
      </button>
      <StatusMsg status={status} />
    </div>
  );
}

// ── Main Page ────────────────────────────────────────────────────

export default function Subtitle() {
  return (
    <div style={{ height: "100%", overflowY: "auto", padding: 24 }}>
      <h2 style={{ margin: "0 0 20px", fontSize: 18 }}>字幕工具</h2>
      <div style={{ maxWidth: 600 }}>
        <FetchSection />
        <AlignSection />
        <ExtractSection />
        <ShiftSection />
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors

- [ ] **Step 4: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/pages/Subtitle.tsx
git commit -m "feat: add Subtitle tools page with fetch, align, extract, and shift"
```

---

### Task 3: App.tsx unlock /subtitle + final build

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Read App.tsx**

Read `src/App.tsx`.

- [ ] **Step 2: Make changes**

1. Add import:
```tsx
import Subtitle from "./pages/Subtitle";
```

2. In `NAV_SECTIONS`, find `{ icon: "◷", label: "字幕", path: "/subtitle", disabled: true }` and remove `disabled: true`.

3. In `<Routes>`, replace the `/subtitle` Placeholder with:
```tsx
<Route path="/subtitle" element={<Subtitle />} />
```

4. Do NOT add `/subtitle` to `KB_PATHS` — subtitle tools are in the "工具" section, not knowledge base.

- [ ] **Step 3: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors

- [ ] **Step 4: Full frontend build**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npm run build 2>&1`
Expected: Build success

- [ ] **Step 5: Full Rust test suite**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup/src-tauri && cargo test 2>&1`
Expected: All tests pass (~69 tests)

- [ ] **Step 6: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/App.tsx
git commit -m "feat: unlock /subtitle route — M5 complete"
```

---

## Self-review checklist

- [x] **Spec coverage:** All 5 subtitle operations have UI — fetch, align, extract (with stream listing), shift
- [x] **No placeholders:** Every task has complete code
- [x] **Type consistency:** `SubtitleStreamInfo` matches Rust struct (index, codec_name, duration, language, title)
- [x] **No backend changes:** All 5 commands already registered — this is a pure frontend milestone
- [x] **No KB_PATHS change:** /subtitle is a "工具" route, not knowledge base
- [x] **invoke param names:** Match Rust command parameter names exactly (video, lang, srt, offsetMs, stream, apiKey)
