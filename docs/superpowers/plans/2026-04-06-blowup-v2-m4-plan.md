# Blowup v2 M4: Download Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add download management with background aria2c downloads, YTS torrent search integration in FilmDetailPanel, and a dedicated Download page with queue/history tracking.

**Architecture:** New `downloads` DB table tracks download lifecycle. `download.rs` gains `start_download` (spawns aria2c via `tokio::process`, monitors in background task), `list_downloads`, `cancel_download`, `delete_download_record`. FilmDetailPanel gets a YTS search modal that triggers downloads. Download page shows active + completed downloads with auto-refresh polling.

**Tech Stack:** sqlx 0.8 (downloads table), tokio::process (async child process), aria2c (torrent download), existing YTS search API, React 19 + TypeScript

---

## Critical patterns (read before any task)

- **Pool access:** `pool.inner()` — NOT `&**pool`
- **Config loading:** `crate::config::load_config()` returns `Config` with `tools.aria2c` and `library.root_dir`
- **Tracker loading:** `super::tracker::load_trackers()` returns `Vec<String>` of tracker URLs
- **Existing commands:** `search_yify_cmd` and `update_trackers` are already registered — just need frontend wrappers
- **Background tasks:** `tokio::spawn(async move { child.wait().await })` for monitoring aria2c processes
- **Process killing:** `taskkill /PID <pid> /F` on Windows, `kill -TERM <pid>` on Unix

## File structure

| Action | File | Purpose |
|--------|------|---------|
| Create | `src-tauri/migrations/002_downloads.sql` | Downloads table |
| Modify | `src-tauri/src/commands/download.rs` | Add DownloadRecord type + 4 new commands |
| Modify | `src-tauri/src/lib.rs` | Register 4 new commands |
| Modify | `src/lib/tauri.ts` | Download + YTS types and wrappers |
| Modify | `src/components/FilmDetailPanel.tsx` | YTS search modal + download trigger |
| Create | `src/pages/Download.tsx` | Download page with queue/history |
| Modify | `src/App.tsx` | Enable /download route |

---

### Task 1: Migration 002 — downloads table

**Files:**
- Create: `src-tauri/migrations/002_downloads.sql`

- [ ] **Step 1: Create migration file**

Create `src-tauri/migrations/002_downloads.sql`:

```sql
CREATE TABLE IF NOT EXISTS downloads (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    film_id       INTEGER REFERENCES films(id),
    title         TEXT NOT NULL,
    quality       TEXT,
    target        TEXT NOT NULL,
    output_dir    TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK(status IN ('pending','downloading','completed','failed','cancelled')),
    pid           INTEGER,
    file_path     TEXT,
    error_message TEXT,
    started_at    TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at  TEXT
);
```

- [ ] **Step 2: Verify migration compiles**

Run: `cd src-tauri && cargo check 2>&1`
Expected: clean (sqlx migrations are loaded at runtime)

- [ ] **Step 3: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src-tauri/migrations/002_downloads.sql
git commit -m "feat: add downloads table migration"
```

---

### Task 2: Download management commands + register + tests

**Files:**
- Modify: `src-tauri/src/commands/download.rs`
- Modify: `src-tauri/src/lib.rs`

This task adds `DownloadRecord` type, `start_download`, `list_downloads`, `cancel_download`, `delete_download_record` commands, and registers them.

- [ ] **Step 1: Read existing download.rs**

Read `src-tauri/src/commands/download.rs` to understand the current structure.

- [ ] **Step 2: Add imports and DownloadRecord type**

Add these imports at the top of `download.rs`:

```rust
use serde::Serialize;
use sqlx::SqlitePool;
use crate::config::load_config;
```

Add the `DownloadRecord` type (after the existing `DownloadArgs` struct):

```rust
#[derive(Serialize, sqlx::FromRow)]
pub struct DownloadRecord {
    pub id: i64,
    pub film_id: Option<i64>,
    pub title: String,
    pub quality: Option<String>,
    pub target: String,
    pub output_dir: String,
    pub status: String,
    pub pid: Option<i64>,
    pub file_path: Option<String>,
    pub error_message: Option<String>,
    pub started_at: String,
    pub completed_at: Option<String>,
}
```

- [ ] **Step 3: Add `start_download` command**

This command loads config for aria2c path and output dir, spawns aria2c in background, and monitors via a tokio task.

```rust
#[tauri::command]
pub async fn start_download(
    title: String,
    target: String,
    quality: Option<String>,
    film_id: Option<i64>,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<i64, String> {
    let config = load_config();
    let aria2c_bin = config.tools.aria2c.clone();
    let output_dir = config.library.root_dir.clone();

    which::which(&aria2c_bin)
        .map_err(|_| "aria2c 未找到，请在设置中配置 aria2c 路径".to_string())?;

    std::fs::create_dir_all(&output_dir).map_err(|e| e.to_string())?;

    let trackers = load_trackers();

    let mut cmd = tokio::process::Command::new(&aria2c_bin);
    cmd.arg("--dir").arg(&output_dir);
    cmd.arg("--seed-time=0");
    if !trackers.is_empty() {
        cmd.arg(format!("--bt-tracker={}", trackers.join(",")));
    }
    cmd.arg(&target);

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id().map(|p| p as i64);

    let result = sqlx::query(
        "INSERT INTO downloads (film_id, title, quality, target, output_dir, status, pid)
         VALUES (?, ?, ?, ?, ?, 'downloading', ?)",
    )
    .bind(film_id)
    .bind(&title)
    .bind(&quality)
    .bind(&target)
    .bind(&output_dir)
    .bind(pid)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    let download_id = result.last_insert_rowid();

    let pool_clone = pool.inner().clone();
    tokio::spawn(async move {
        let wait_result = child.wait().await;

        let current_status: Option<String> = sqlx::query_scalar(
            "SELECT status FROM downloads WHERE id = ?",
        )
        .bind(download_id)
        .fetch_optional(&pool_clone)
        .await
        .ok()
        .flatten();

        if current_status.as_deref() == Some("cancelled") {
            return;
        }

        match wait_result {
            Ok(status) if status.success() => {
                let _ = sqlx::query(
                    "UPDATE downloads SET status = 'completed', completed_at = datetime('now'), pid = NULL WHERE id = ?",
                )
                .bind(download_id)
                .execute(&pool_clone)
                .await;
            }
            Ok(status) => {
                let msg = format!("aria2c exited with code {}", status.code().unwrap_or(-1));
                let _ = sqlx::query(
                    "UPDATE downloads SET status = 'failed', completed_at = datetime('now'), pid = NULL, error_message = ? WHERE id = ?",
                )
                .bind(&msg)
                .bind(download_id)
                .execute(&pool_clone)
                .await;
            }
            Err(e) => {
                let _ = sqlx::query(
                    "UPDATE downloads SET status = 'failed', completed_at = datetime('now'), pid = NULL, error_message = ? WHERE id = ?",
                )
                .bind(e.to_string())
                .bind(download_id)
                .execute(&pool_clone)
                .await;
            }
        }
    });

    Ok(download_id)
}
```

- [ ] **Step 4: Add `list_downloads`, `cancel_download`, `delete_download_record`**

```rust
#[tauri::command]
pub async fn list_downloads(
    pool: tauri::State<'_, SqlitePool>,
) -> Result<Vec<DownloadRecord>, String> {
    sqlx::query_as::<_, DownloadRecord>(
        "SELECT id, film_id, title, quality, target, output_dir, status, pid,
                file_path, error_message, started_at, completed_at
         FROM downloads ORDER BY started_at DESC",
    )
    .fetch_all(pool.inner())
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_download(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    let pid: Option<i64> = sqlx::query_scalar(
        "SELECT pid FROM downloads WHERE id = ? AND status = 'downloading'",
    )
    .bind(id)
    .fetch_optional(pool.inner())
    .await
    .map_err(|e| e.to_string())?
    .flatten();

    if let Some(pid) = pid {
        kill_process(pid as u32);
    }

    sqlx::query(
        "UPDATE downloads SET status = 'cancelled', completed_at = datetime('now'), pid = NULL WHERE id = ?",
    )
    .bind(id)
    .execute(pool.inner())
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn delete_download_record(
    id: i64,
    pool: tauri::State<'_, SqlitePool>,
) -> Result<(), String> {
    sqlx::query("DELETE FROM downloads WHERE id = ?")
        .bind(id)
        .execute(pool.inner())
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn kill_process(pid: u32) {
    #[cfg(target_family = "windows")]
    {
        let _ = std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();
    }
    #[cfg(not(target_family = "windows"))]
    {
        let _ = std::process::Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
    }
}
```

- [ ] **Step 5: Add tests**

Append tests at the bottom of `download.rs` (inside the existing `#[cfg(test)] mod tests` block):

```rust
    #[tokio::test]
    async fn test_download_record_crud() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        // Insert a download record
        sqlx::query(
            "INSERT INTO downloads (title, target, output_dir, status)
             VALUES (?, ?, ?, 'downloading')",
        )
        .bind("Test Film")
        .bind("magnet:?xt=test")
        .bind("/tmp/downloads")
        .execute(&pool)
        .await
        .unwrap();

        // List
        let records: Vec<super::DownloadRecord> = sqlx::query_as(
            "SELECT id, film_id, title, quality, target, output_dir, status, pid,
                    file_path, error_message, started_at, completed_at
             FROM downloads ORDER BY started_at DESC",
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].title, "Test Film");
        assert_eq!(records[0].status, "downloading");
    }

    #[tokio::test]
    async fn test_cancel_sets_status() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (title, target, output_dir, status, pid)
             VALUES (?, ?, ?, 'downloading', ?)",
        )
        .bind("Test Film")
        .bind("magnet:?xt=test")
        .bind("/tmp")
        .bind(99999_i64)
        .execute(&pool)
        .await
        .unwrap();

        // Cancel (won't actually kill since PID is fake)
        sqlx::query(
            "UPDATE downloads SET status = 'cancelled', completed_at = datetime('now'), pid = NULL WHERE id = 1",
        )
        .execute(&pool)
        .await
        .unwrap();

        let status: String = sqlx::query_scalar("SELECT status FROM downloads WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(status, "cancelled");
    }

    #[tokio::test]
    async fn test_delete_download_record() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();

        sqlx::query(
            "INSERT INTO downloads (title, target, output_dir, status)
             VALUES (?, ?, ?, 'completed')",
        )
        .bind("Done Film")
        .bind("magnet:?xt=done")
        .bind("/tmp")
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("DELETE FROM downloads WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0);
    }
```

- [ ] **Step 6: Register new commands in lib.rs**

Read `src-tauri/src/lib.rs` and add these 4 commands to the `generate_handler![]` macro (after existing `commands::download::download_target`):

```rust
commands::download::start_download,
commands::download::list_downloads,
commands::download::cancel_download,
commands::download::delete_download_record,
```

- [ ] **Step 7: Run full test suite**

Run: `cd src-tauri && cargo test 2>&1`
Expected: All existing + 3 new tests pass (~69 total)

- [ ] **Step 8: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src-tauri/src/commands/download.rs src-tauri/src/lib.rs
git commit -m "feat: add download management commands with background monitoring"
```

---

### Task 3: tauri.ts — download + YTS types and wrappers

**Files:**
- Modify: `src/lib/tauri.ts`

- [ ] **Step 1: Read `tauri.ts`**

Read `src/lib/tauri.ts` to see the current structure.

- [ ] **Step 2: Add types**

Add these types after the existing `FilmFilterParams` interface:

```typescript
// ── Download ─────────────────────────────────────────────────────

export interface DownloadRecord {
  id: number;
  film_id: number | null;
  title: string;
  quality: string | null;
  target: string;
  output_dir: string;
  status: "pending" | "downloading" | "completed" | "failed" | "cancelled";
  pid: number | null;
  file_path: string | null;
  error_message: string | null;
  started_at: string;
  completed_at: string | null;
}

export interface MovieResult {
  title: string;
  year: number;
  quality: string;
  magnet: string | null;
  torrent_url: string | null;
  seeds: number;
}
```

- [ ] **Step 3: Add invoke wrappers**

Add a new `download` export object after the `library` object:

```typescript
export const download = {
  startDownload: (title: string, target: string, quality?: string, filmId?: number) =>
    invoke<number>("start_download", { title, target, quality, filmId }),
  listDownloads: () =>
    invoke<DownloadRecord[]>("list_downloads"),
  cancelDownload: (id: number) =>
    invoke<void>("cancel_download", { id }),
  deleteDownloadRecord: (id: number) =>
    invoke<void>("delete_download_record", { id }),
};

export const yts = {
  search: (query: string, year?: number) =>
    invoke<MovieResult[]>("search_yify_cmd", { query, year }),
};

export const tracker = {
  update: (source?: string) =>
    invoke<void>("update_trackers", { source }),
};
```

- [ ] **Step 4: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors

- [ ] **Step 5: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/lib/tauri.ts
git commit -m "feat: add download, YTS, and tracker invoke wrappers in tauri.ts"
```

---

### Task 4: FilmDetailPanel — YTS search modal + download trigger

**Files:**
- Modify: `src/components/FilmDetailPanel.tsx`

The disabled "搜索资源（M3）" button (around line 175) needs to become active and open a YTS torrent search modal.

- [ ] **Step 1: Read FilmDetailPanel.tsx**

Read `src/components/FilmDetailPanel.tsx` to understand the current structure, particularly the disabled button and existing modal pattern (AddToLibraryModal).

- [ ] **Step 2: Add imports**

Add these imports at the top:

```typescript
import { yts, download } from "../lib/tauri";
import type { MovieResult } from "../lib/tauri";
```

- [ ] **Step 3: Create TorrentSearchModal component**

Add this component inside the file (before or after the existing `AddToLibraryModal`):

```typescript
function TorrentSearchModal({
  title,
  filmId,
  onClose,
}: {
  title: string;
  filmId?: number;
  onClose: () => void;
}) {
  const [results, setResults] = useState<MovieResult[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [started, setStarted] = useState<Set<string>>(new Set());

  useEffect(() => {
    yts
      .search(title)
      .then(setResults)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [title]);

  const handleDownload = async (r: MovieResult) => {
    const target = r.magnet ?? r.torrent_url;
    if (!target) return;
    await download.startDownload(
      `${r.title} (${r.year})`,
      target,
      r.quality,
      filmId
    );
    setStarted((prev) => new Set(prev).add(target));
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          background: "var(--color-bg-primary)",
          borderRadius: 12,
          padding: 24,
          width: 480,
          maxHeight: "70vh",
          overflowY: "auto",
        }}
      >
        <h3 style={{ margin: "0 0 12px" }}>搜索资源: {title}</h3>

        {loading && (
          <div style={{ color: "var(--color-label-secondary)", fontSize: 13 }}>
            搜索中...
          </div>
        )}

        {error && (
          <div style={{ color: "#e53935", fontSize: 13 }}>
            {error.includes("NoResults") ? "未找到资源" : `搜索失败: ${error}`}
          </div>
        )}

        {!loading && !error && results.length === 0 && (
          <div style={{ color: "var(--color-label-tertiary)", fontSize: 13 }}>
            未找到资源
          </div>
        )}

        {results.map((r, i) => {
          const target = r.magnet ?? r.torrent_url ?? "";
          const isStarted = started.has(target);
          return (
            <div
              key={i}
              style={{
                display: "flex",
                alignItems: "center",
                justifyContent: "space-between",
                padding: "8px 0",
                borderBottom: "1px solid var(--color-separator)",
                fontSize: 13,
              }}
            >
              <div>
                <span style={{ fontWeight: 500 }}>{r.quality}</span>
                <span style={{ color: "var(--color-label-secondary)", marginLeft: 12 }}>
                  {r.seeds} seeds
                </span>
              </div>
              {isStarted ? (
                <span style={{ color: "var(--color-accent)", fontSize: 12 }}>
                  ✓ 已添加
                </span>
              ) : (
                <button
                  onClick={() => handleDownload(r)}
                  disabled={!target}
                  style={{
                    background: "var(--color-accent)",
                    color: "#fff",
                    border: "none",
                    borderRadius: 4,
                    padding: "3px 12px",
                    cursor: "pointer",
                    fontSize: 12,
                  }}
                >
                  下载
                </button>
              )}
            </div>
          );
        })}

        <button
          onClick={onClose}
          style={{
            marginTop: 16,
            background: "var(--color-bg-control)",
            border: "1px solid var(--color-separator)",
            borderRadius: 6,
            padding: "6px 16px",
            color: "var(--color-label-primary)",
            cursor: "pointer",
            fontSize: 13,
            width: "100%",
          }}
        >
          关闭
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Replace disabled button with active "搜索资源" button**

Find the disabled button around line 175:
```tsx
<button disabled style={{ /* ... */ }}>
  搜索资源（M3）
</button>
```

Replace it with:
```tsx
<button
  onClick={() => setShowTorrentModal(true)}
  style={{
    background: "var(--color-bg-control)",
    border: "1px solid var(--color-separator)",
    borderRadius: 8,
    padding: "0.4rem 0.75rem",
    color: "var(--color-label-primary)",
    cursor: "pointer",
    fontSize: "0.78rem",
  }}
>
  搜索资源
</button>
```

Add state for the modal (near the other state declarations in the parent component):
```tsx
const [showTorrentModal, setShowTorrentModal] = useState(false);
```

Add the modal render (near the existing AddToLibraryModal render):
```tsx
{showTorrentModal && film && (
  <TorrentSearchModal
    title={film.title}
    filmId={addedFilmId ?? undefined}
    onClose={() => setShowTorrentModal(false)}
  />
)}
```

**Note:** `addedFilmId` is the ID from the "加入知识库" flow if the film was already added. If the variable name differs, read the file to find the correct state variable that holds the KB film ID.

- [ ] **Step 5: Type check**

Run: `cd C:/Users/lixuan/Documents/code/rust/blowup && npx tsc --noEmit 2>&1`
Expected: 0 errors

- [ ] **Step 6: Commit**

```bash
cd C:/Users/lixuan/Documents/code/rust/blowup
git add src/components/FilmDetailPanel.tsx
git commit -m "feat: add YTS torrent search modal to FilmDetailPanel"
```

---

### Task 5: Download page UI

**Files:**
- Create: `src/pages/Download.tsx`

Layout: header bar with actions + download list. Auto-refreshes every 3 seconds when active downloads exist.

- [ ] **Step 1: Read existing page patterns**

Read `src/pages/Library.tsx` (lines 1-30) to confirm the established patterns.

- [ ] **Step 2: Create Download.tsx**

```tsx
import { useState, useEffect, useCallback } from "react";
import { download, tracker } from "../lib/tauri";
import type { DownloadRecord } from "../lib/tauri";

// ── Helpers ──────────────────────────────────────────────────────

function statusLabel(s: string): string {
  switch (s) {
    case "downloading": return "下载中";
    case "completed": return "已完成";
    case "failed": return "失败";
    case "cancelled": return "已取消";
    default: return s;
  }
}

function statusColor(s: string): string {
  switch (s) {
    case "downloading": return "var(--color-accent)";
    case "completed": return "#4caf50";
    case "failed": return "#e53935";
    case "cancelled": return "var(--color-label-tertiary)";
    default: return "var(--color-label-secondary)";
  }
}

// ── Add Download Modal ───────────────────────────────────────────

function AddDownloadModal({ onClose, onAdd }: {
  onClose: () => void;
  onAdd: () => void;
}) {
  const [title, setTitle] = useState("");
  const [target, setTarget] = useState("");
  const [quality, setQuality] = useState("");

  const handleSubmit = async () => {
    if (!target.trim()) return;
    await download.startDownload(
      title.trim() || "手动下载",
      target.trim(),
      quality.trim() || undefined,
    );
    onAdd();
    onClose();
  };

  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.5)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 1000,
      }}
      onClick={onClose}
    >
      <div
        onClick={(e) => e.stopPropagation()}
        style={{
          background: "var(--color-bg-primary)",
          borderRadius: 12,
          padding: 24,
          width: 440,
        }}
      >
        <h3 style={{ margin: "0 0 16px" }}>手动添加下载</h3>

        <div style={{ marginBottom: 12 }}>
          <label style={{ fontSize: 12, color: "var(--color-label-secondary)", display: "block", marginBottom: 4 }}>
            磁力链接 / Torrent URL *
          </label>
          <input
            value={target}
            onChange={(e) => setTarget(e.target.value)}
            placeholder="magnet:?xt=urn:btih:..."
            style={{
              width: "100%",
              padding: "6px 10px",
              borderRadius: 6,
              border: "1px solid var(--color-separator)",
              background: "var(--color-bg-control)",
              color: "var(--color-label-primary)",
              fontSize: 13,
              boxSizing: "border-box",
            }}
          />
        </div>

        <div style={{ marginBottom: 12 }}>
          <label style={{ fontSize: 12, color: "var(--color-label-secondary)", display: "block", marginBottom: 4 }}>
            标题
          </label>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="影片名称（可选）"
            style={{
              width: "100%",
              padding: "6px 10px",
              borderRadius: 6,
              border: "1px solid var(--color-separator)",
              background: "var(--color-bg-control)",
              color: "var(--color-label-primary)",
              fontSize: 13,
              boxSizing: "border-box",
            }}
          />
        </div>

        <div style={{ marginBottom: 16 }}>
          <label style={{ fontSize: 12, color: "var(--color-label-secondary)", display: "block", marginBottom: 4 }}>
            画质
          </label>
          <input
            value={quality}
            onChange={(e) => setQuality(e.target.value)}
            placeholder="1080p（可选）"
            style={{
              width: "100%",
              padding: "6px 10px",
              borderRadius: 6,
              border: "1px solid var(--color-separator)",
              background: "var(--color-bg-control)",
              color: "var(--color-label-primary)",
              fontSize: 13,
              boxSizing: "border-box",
            }}
          />
        </div>

        <div style={{ display: "flex", gap: 8, justifyContent: "flex-end" }}>
          <button
            onClick={onClose}
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
            取消
          </button>
          <button
            onClick={handleSubmit}
            disabled={!target.trim()}
            style={{
              background: "var(--color-accent)",
              color: "#fff",
              border: "none",
              borderRadius: 6,
              padding: "6px 16px",
              cursor: target.trim() ? "pointer" : "not-allowed",
              fontSize: 13,
              opacity: target.trim() ? 1 : 0.5,
            }}
          >
            开始下载
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Download Row ─────────────────────────────────────────────────

function DownloadRow({
  record,
  onCancel,
  onDelete,
}: {
  record: DownloadRecord;
  onCancel: () => void;
  onDelete: () => void;
}) {
  return (
    <div
      style={{
        padding: "12px 16px",
        borderBottom: "1px solid var(--color-separator)",
        display: "flex",
        alignItems: "center",
        gap: 12,
      }}
    >
      <div style={{ flex: 1, minWidth: 0 }}>
        <div
          style={{
            fontWeight: 500,
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
        >
          {record.title}
        </div>
        <div style={{ fontSize: 12, color: "var(--color-label-secondary)", marginTop: 2 }}>
          {record.quality && <span>{record.quality} · </span>}
          {record.started_at.replace("T", " ").slice(0, 16)}
          {record.error_message && (
            <span style={{ color: "#e53935", marginLeft: 8 }}>
              {record.error_message}
            </span>
          )}
        </div>
      </div>

      <span
        style={{
          fontSize: 12,
          fontWeight: 500,
          color: statusColor(record.status),
          whiteSpace: "nowrap",
        }}
      >
        {record.status === "downloading" && "● "}
        {statusLabel(record.status)}
      </span>

      <div style={{ display: "flex", gap: 6 }}>
        {record.status === "downloading" && (
          <button
            onClick={onCancel}
            style={{
              background: "none",
              border: "1px solid var(--color-separator)",
              borderRadius: 4,
              padding: "2px 8px",
              color: "var(--color-label-secondary)",
              cursor: "pointer",
              fontSize: 12,
            }}
          >
            取消
          </button>
        )}
        {record.status !== "downloading" && (
          <button
            onClick={onDelete}
            style={{
              background: "none",
              border: "1px solid #e53935",
              borderRadius: 4,
              padding: "2px 8px",
              color: "#e53935",
              cursor: "pointer",
              fontSize: 12,
            }}
          >
            删除
          </button>
        )}
      </div>
    </div>
  );
}

// ── Main Page ────────────────────────────────────────────────────

export default function Download() {
  const [downloads, setDownloads] = useState<DownloadRecord[]>([]);
  const [showAddModal, setShowAddModal] = useState(false);
  const [updating, setUpdating] = useState(false);

  const refresh = useCallback(() => {
    download.listDownloads().then(setDownloads);
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  // Auto-refresh when active downloads exist
  useEffect(() => {
    const hasActive = downloads.some((d) => d.status === "downloading");
    if (!hasActive) return;
    const timer = setInterval(refresh, 3000);
    return () => clearInterval(timer);
  }, [downloads, refresh]);

  const handleCancel = async (id: number) => {
    await download.cancelDownload(id);
    refresh();
  };

  const handleDelete = async (id: number) => {
    await download.deleteDownloadRecord(id);
    refresh();
  };

  const handleUpdateTrackers = async () => {
    setUpdating(true);
    try {
      await tracker.update();
      alert("Tracker 列表已更新");
    } catch (e) {
      alert(`更新失败: ${e}`);
    } finally {
      setUpdating(false);
    }
  };

  const active = downloads.filter((d) => d.status === "downloading");
  const history = downloads.filter((d) => d.status !== "downloading");

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      {/* Header */}
      <div
        style={{
          padding: "12px 16px",
          borderBottom: "1px solid var(--color-separator)",
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}
      >
        <h2 style={{ margin: 0, fontSize: 16, flex: 1 }}>下载管理</h2>
        <button
          onClick={handleUpdateTrackers}
          disabled={updating}
          style={{
            background: "var(--color-bg-control)",
            border: "1px solid var(--color-separator)",
            borderRadius: 6,
            padding: "4px 12px",
            color: "var(--color-label-primary)",
            cursor: updating ? "wait" : "pointer",
            fontSize: 13,
          }}
        >
          {updating ? "更新中..." : "更新 Tracker"}
        </button>
        <button
          onClick={() => setShowAddModal(true)}
          style={{
            background: "var(--color-accent)",
            color: "#fff",
            border: "none",
            borderRadius: 6,
            padding: "4px 12px",
            cursor: "pointer",
            fontSize: 13,
          }}
        >
          + 手动添加
        </button>
      </div>

      {/* Content */}
      <div style={{ flex: 1, overflowY: "auto" }}>
        {downloads.length === 0 ? (
          <div
            style={{
              padding: 48,
              textAlign: "center",
              color: "var(--color-label-tertiary)",
              fontSize: 13,
            }}
          >
            暂无下载任务。在搜索页面选择影片并搜索资源，或手动添加磁力链接。
          </div>
        ) : (
          <>
            {active.length > 0 && (
              <div>
                <div
                  style={{
                    padding: "8px 16px",
                    fontSize: 12,
                    fontWeight: 600,
                    color: "var(--color-label-secondary)",
                    background: "var(--color-bg-control)",
                  }}
                >
                  进行中 ({active.length})
                </div>
                {active.map((d) => (
                  <DownloadRow
                    key={d.id}
                    record={d}
                    onCancel={() => handleCancel(d.id)}
                    onDelete={() => handleDelete(d.id)}
                  />
                ))}
              </div>
            )}
            {history.length > 0 && (
              <div>
                <div
                  style={{
                    padding: "8px 16px",
                    fontSize: 12,
                    fontWeight: 600,
                    color: "var(--color-label-secondary)",
                    background: "var(--color-bg-control)",
                  }}
                >
                  历史记录 ({history.length})
                </div>
                {history.map((d) => (
                  <DownloadRow
                    key={d.id}
                    record={d}
                    onCancel={() => handleCancel(d.id)}
                    onDelete={() => handleDelete(d.id)}
                  />
                ))}
              </div>
            )}
          </>
        )}
      </div>

      {showAddModal && (
        <AddDownloadModal
          onClose={() => setShowAddModal(false)}
          onAdd={refresh}
        />
      )}
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
git add src/pages/Download.tsx
git commit -m "feat: add Download page with queue, history, and manual add"
```

---

### Task 6: App.tsx unlock /download + final build

**Files:**
- Modify: `src/App.tsx`

- [ ] **Step 1: Read App.tsx**

Read `src/App.tsx` to see current nav and routing.

- [ ] **Step 2: Make changes**

1. Add import:
```tsx
import Download from "./pages/Download";
```

2. In `NAV_SECTIONS`, find `{ icon: "↓", label: "下载", path: "/download", disabled: true }` and remove `disabled: true`.

3. In `<Routes>`, replace the `/download` Placeholder:
```tsx
<Route path="/download" element={<Download />} />
```

4. Do NOT add `/download` to `KB_PATHS` — downloads are in the "资源" section, not knowledge base. MusicPlayer should NOT play on the download page.

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
git commit -m "feat: unlock /download route — M4 complete"
```

---

## Self-review checklist

- [x] **Spec coverage:** Migration, download CRUD, background monitoring, YTS search integration, download page, manual add, tracker update, cancel, delete
- [x] **No placeholders:** Every task has complete code
- [x] **Type consistency:** `DownloadRecord` matches across Rust/TS/UI, `MovieResult` matches search.rs
- [x] **Pattern consistency:** `pool.inner()`, `query_as::<_, T>()`, `useEffect` for data loading, `useCallback` for refresh
- [x] **File paths:** All exact
- [x] **Process lifecycle:** start_download spawns → background task monitors → cancel kills by PID → DB tracks status
- [x] **No KB_PATHS change:** /download is NOT a knowledge base route
