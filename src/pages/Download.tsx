import { useState, useEffect, useCallback } from "react";
import { download } from "../lib/tauri";
import type { DownloadRecord } from "../lib/tauri";
import { formatSize } from "../lib/format";

// ── Helpers ──────────────────────────────────────────────────────

function statusLabel(s: string) {
  switch (s) {
    case "downloading": return "下载中";
    case "paused": return "已暂停";
    case "completed": return "已完成";
    case "failed": return "失败";
    default: return "等待中";
  }
}

function statusColor(s: string) {
  switch (s) {
    case "downloading": return "var(--color-accent)";
    case "paused": return "var(--color-label-secondary)";
    case "completed": return "#66bb6a";
    case "failed": return "#e57373";
    default: return "var(--color-label-tertiary)";
  }
}

// ── Download Row ─────────────────────────────────────────────────

function DownloadRow({
  record,
  prevBytes,
  onPause,
  onResume,
  onDelete,
  onRedownload,
}: {
  record: DownloadRecord;
  prevBytes: number;
  onPause: () => void;
  onResume: () => void;
  onDelete: () => void;
  onRedownload: () => void;
}) {
  const isActive = record.status === "downloading";
  const progress = record.total_bytes > 0
    ? Math.min(100, (record.progress_bytes / record.total_bytes) * 100)
    : 0;

  // Speed calc: bytes diff over 3s polling interval
  const speed = isActive && prevBytes > 0
    ? Math.max(0, record.progress_bytes - prevBytes) / 3
    : 0;

  return (
    <div style={{
      padding: "0.75rem 0",
      borderBottom: "1px solid var(--color-separator)",
    }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "center", marginBottom: "0.3rem" }}>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: "0.85rem", fontWeight: 500, whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}>
            {record.title}
          </div>
          <div style={{ fontSize: "0.7rem", color: "var(--color-label-tertiary)", marginTop: "0.1rem" }}>
            {record.director && <span>{record.director} · </span>}
            {record.quality && <span>{record.quality} · </span>}
            <span style={{ color: statusColor(record.status) }}>{statusLabel(record.status)}</span>
          </div>
        </div>
        <div style={{ display: "flex", gap: "0.4rem", flexShrink: 0, marginLeft: "0.5rem" }}>
          {isActive && (
            <button onClick={onPause} style={{
              background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
              padding: "0.2rem 0.5rem", color: "var(--color-label-secondary)", cursor: "pointer", fontSize: "0.7rem", fontFamily: "inherit",
            }}>暂停</button>
          )}
          {record.status === "paused" && (
            <button onClick={onResume} style={{
              background: "none", border: "1px solid var(--color-accent)", borderRadius: 4,
              padding: "0.2rem 0.5rem", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.7rem", fontFamily: "inherit",
            }}>继续</button>
          )}
          {record.status === "completed" && (
            <button onClick={onRedownload} style={{
              background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
              padding: "0.2rem 0.5rem", color: "var(--color-label-secondary)", cursor: "pointer", fontSize: "0.7rem", fontFamily: "inherit",
            }}>重新下载</button>
          )}
          <button onClick={onDelete} style={{
            background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
            padding: "0.2rem 0.5rem", color: "#e57373", cursor: "pointer", fontSize: "0.7rem", fontFamily: "inherit",
          }}>删除</button>
        </div>
      </div>

      {/* Progress bar */}
      {isActive && record.total_bytes > 0 && (
        <div>
          <div style={{
            height: 4, borderRadius: 2, background: "var(--color-separator)", overflow: "hidden",
          }}>
            <div style={{
              height: "100%", borderRadius: 2, background: "var(--color-accent)",
              width: `${progress}%`, transition: "width 0.5s ease",
            }} />
          </div>
          <div style={{ display: "flex", justifyContent: "space-between", fontSize: "0.65rem", color: "var(--color-label-quaternary)", marginTop: "0.2rem" }}>
            <span>{formatSize(record.progress_bytes)} / {formatSize(record.total_bytes)}</span>
            <span>{progress.toFixed(1)}%{speed > 0 ? ` · ${formatSize(speed)}/s` : ""}</span>
          </div>
        </div>
      )}

      {/* Error message */}
      {record.error_message && (
        <div style={{ fontSize: "0.7rem", color: "#e57373", marginTop: "0.25rem" }}>
          {record.error_message}
        </div>
      )}
    </div>
  );
}

// ── Main Page ────────────────────────────────────────────────────

function DeleteConfirmModal({ title, onConfirm, onCancel }: {
  title: string; onConfirm: () => void; onCancel: () => void;
}) {
  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.5)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000 }} onClick={onCancel}>
      <div onClick={(e) => e.stopPropagation()} style={{ background: "var(--color-bg-primary)", borderRadius: 10, padding: "1.5rem", width: 360 }}>
        <h3 style={{ margin: "0 0 0.75rem", fontSize: "0.95rem", fontWeight: 700 }}>确认删除</h3>
        <p style={{ margin: "0 0 1rem", fontSize: "0.82rem", color: "var(--color-label-secondary)", lineHeight: 1.5 }}>
          将永久删除「{title}」的下载记录及所有已下载文件（包括种子文件），此操作不可恢复。
        </p>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem" }}>
          <button onClick={onCancel} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>取消</button>
          <button onClick={onConfirm} style={{ background: "#e57373", border: "none", borderRadius: 6, padding: "0.35rem 1rem", color: "#fff", fontWeight: 600, cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>删除</button>
        </div>
      </div>
    </div>
  );
}

export default function Download() {
  const [downloads, setDownloads] = useState<DownloadRecord[]>([]);
  const [prevBytes, setPrevBytes] = useState<Map<number, number>>(new Map());
  const [deleteTarget, setDeleteTarget] = useState<DownloadRecord | null>(null);

  const refresh = useCallback(async () => {
    const list = await download.listDownloads();
    setPrevBytes((prev) => {
      // Build new map from current downloads before updating
      const map = new Map<number, number>();
      for (const [k, v] of prev) { map.set(k, v); }
      return map;
    });
    setDownloads((prev) => {
      // Save current bytes as prev for next refresh
      const map = new Map<number, number>();
      for (const d of prev) { map.set(d.id, d.progress_bytes); }
      setPrevBytes(map);
      return list;
    });
  }, []);

  useEffect(() => {
    download.listDownloads().then(setDownloads);
  }, []);

  // Auto-refresh if active downloads
  useEffect(() => {
    const hasActive = downloads.some((d) => d.status === "downloading");
    if (!hasActive) return;
    const timer = setInterval(refresh, 3000);
    return () => clearInterval(timer);
  }, [downloads, refresh]);

  const handlePause = async (id: number) => {
    await download.pauseDownload(id);
    refresh();
  };

  const handleResume = async (id: number) => {
    await download.resumeDownload(id);
    refresh();
  };

  const handleDelete = async (record: DownloadRecord) => {
    const isActive = record.status === "downloading" || record.status === "paused" || record.status === "pending";
    if (isActive) {
      // Active task: confirm before deleting (will remove files)
      setDeleteTarget(record);
    } else {
      // History: just remove DB record
      await download.deleteDownload(record.id);
      refresh();
    }
  };

  const handleDeleteConfirm = async () => {
    if (!deleteTarget) return;
    await download.deleteDownload(deleteTarget.id);
    setDeleteTarget(null);
    refresh();
  };

  const handleRedownload = async (id: number) => {
    try {
      await download.redownload(id);
      refresh();
    } catch (e) {
      console.error("redownload failed:", e);
    }
  };

  const active = downloads.filter((d) => d.status === "downloading" || d.status === "paused" || d.status === "pending");
  const history = downloads.filter((d) => d.status === "completed" || d.status === "failed");

  return (
    <div style={{ flex: 1, overflowY: "auto", padding: "1.4rem 1.75rem 3rem" }}>
      <h1 style={{ fontSize: "1.6rem", fontWeight: 700, letterSpacing: "-0.035em", marginBottom: "1.5rem" }}>
        下载
      </h1>

      {/* Active downloads */}
      {active.length > 0 && (
        <div style={{ marginBottom: "2rem" }}>
          <p style={{
            margin: "0 0 0.5rem", fontSize: "0.7rem", color: "var(--color-label-quaternary)",
            letterSpacing: "0.08em", textTransform: "uppercase",
          }}>进行中</p>
          {active.map((d) => (
            <DownloadRow
              key={d.id}
              record={d}
              prevBytes={prevBytes.get(d.id) ?? 0}
              onPause={() => handlePause(d.id)}
              onResume={() => handleResume(d.id)}
              onDelete={() => handleDelete(d)}
              onRedownload={() => handleRedownload(d.id)}
            />
          ))}
        </div>
      )}

      {/* History */}
      <div>
        <p style={{
          margin: "0 0 0.5rem", fontSize: "0.7rem", color: "var(--color-label-quaternary)",
          letterSpacing: "0.08em", textTransform: "uppercase",
        }}>历史记录</p>
        {history.length === 0 && (
          <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>暂无下载记录</p>
        )}
        {history.map((d) => (
          <DownloadRow
            key={d.id}
            record={d}
            prevBytes={0}
            onPause={() => {}}
            onResume={() => {}}
            onDelete={() => handleDelete(d)}
            onRedownload={() => handleRedownload(d.id)}
          />
        ))}
      </div>

      {deleteTarget && (
        <DeleteConfirmModal
          title={deleteTarget.title}
          onConfirm={handleDeleteConfirm}
          onCancel={() => setDeleteTarget(null)}
        />
      )}
    </div>
  );
}
