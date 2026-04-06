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
