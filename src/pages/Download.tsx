import { useState, useEffect, useCallback, useRef } from "react";
import { download } from "../lib/tauri";
import type { DownloadRecord, TorrentFileInfo } from "../lib/tauri";
import { formatSize } from "../lib/format";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

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
    case "completed": return "var(--color-success)";
    case "failed": return "var(--color-danger)";
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

  // Speed calc: bytes diff over 2s event interval
  const speed = isActive && prevBytes > 0
    ? Math.max(0, record.progress_bytes - prevBytes) / 2
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
          {(record.status === "completed" || record.status === "failed") && (
            <button onClick={onRedownload} style={{
              background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
              padding: "0.2rem 0.5rem", color: "var(--color-label-secondary)", cursor: "pointer", fontSize: "0.7rem", fontFamily: "inherit",
            }}>重新下载</button>
          )}
          <button onClick={onDelete} style={{
            background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
            padding: "0.2rem 0.5rem", color: "var(--color-danger)", cursor: "pointer", fontSize: "0.7rem", fontFamily: "inherit",
          }}>删除</button>
        </div>
      </div>

      {/* Progress bar */}
      {(isActive || record.status === "paused") && record.total_bytes > 0 && (
        <div>
          <div style={{
            height: 4, borderRadius: 2, background: "var(--color-separator)", overflow: "hidden",
          }}>
            <div style={{
              height: "100%", borderRadius: 2,
              background: isActive ? "var(--color-accent)" : "var(--color-label-tertiary)",
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
        <div style={{ fontSize: "0.7rem", color: "var(--color-danger)", marginTop: "0.25rem" }}>
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
          <button onClick={onConfirm} style={{ background: "var(--color-danger)", border: "none", borderRadius: 6, padding: "0.35rem 1rem", color: "#fff", fontWeight: 600, cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>删除</button>
        </div>
      </div>
    </div>
  );
}

export default function Download() {
  const [downloads, setDownloads] = useState<DownloadRecord[]>([]);
  const prevBytesRef = useRef<Map<number, number>>(new Map());
  const [deleteTarget, setDeleteTarget] = useState<DownloadRecord | null>(null);
  // Redownload file-pick modal state
  const [redownloadTarget, setRedownloadTarget] = useState<DownloadRecord | null>(null);
  const [redownloadFiles, setRedownloadFiles] = useState<TorrentFileInfo[]>([]);
  const [redownloadExisting, setRedownloadExisting] = useState<Set<string>>(new Set());
  const [redownloadSelected, setRedownloadSelected] = useState<Set<number>>(new Set());
  const [redownloadFetching, setRedownloadFetching] = useState(false);
  const [redownloadSubmitting, setRedownloadSubmitting] = useState(false);
  const [overwriteConfirm, setOverwriteConfirm] = useState(false);

  const refresh = useCallback(async () => {
    const list = await download.listDownloads();
    setDownloads((prev) => {
      // Snapshot current bytes as prev for speed calc
      const map = new Map<number, number>();
      for (const d of prev) { map.set(d.id, d.progress_bytes); }
      prevBytesRef.current = map;
      return list;
    });
  }, []);

  useEffect(() => {
    download.listDownloads().then(setDownloads);
  }, []);

  // Re-fetch on backend events (replaces polling)
  useBackendEvent(BackendEvent.DOWNLOADS_CHANGED, refresh);

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

  // Step 1: fetch torrent files + existing files on disk
  const handleRedownload = async (record: DownloadRecord) => {
    setRedownloadFetching(true);
    setRedownloadTarget(record);
    try {
      const [files, existing] = await Promise.all([
        download.getTorrentFiles(record.target),
        download.listExistingFiles(record.id),
      ]);
      setRedownloadFiles(files);
      setRedownloadExisting(new Set(existing));
      setRedownloadSelected(new Set(files.map((f) => f.index)));
    } catch (e) {
      console.error("fetch torrent files failed:", e);
      setRedownloadTarget(null);
    } finally {
      setRedownloadFetching(false);
    }
  };

  // Step 2: confirm and start redownload
  const handleRedownloadConfirm = async () => {
    if (!redownloadTarget) return;

    // Check if any selected files already exist
    const selectedNames = redownloadFiles
      .filter((f) => redownloadSelected.has(f.index))
      .map((f) => f.name);
    const conflicts = selectedNames.filter((n) => redownloadExisting.has(n));

    if (conflicts.length > 0 && !overwriteConfirm) {
      setOverwriteConfirm(true);
      return;
    }

    setRedownloadSubmitting(true);
    try {
      await download.redownload(redownloadTarget.id, [...redownloadSelected]);
      closeRedownloadModal();
      refresh();
    } catch (e) {
      console.error("redownload failed:", e);
      closeRedownloadModal();
    }
  };

  const closeRedownloadModal = () => {
    setRedownloadTarget(null);
    setRedownloadFiles([]);
    setRedownloadExisting(new Set());
    setRedownloadSelected(new Set());
    setRedownloadSubmitting(false);
    setOverwriteConfirm(false);
    setRedownloadFetching(false);
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
              prevBytes={prevBytesRef.current.get(d.id) ?? 0}
              onPause={() => handlePause(d.id)}
              onResume={() => handleResume(d.id)}
              onDelete={() => handleDelete(d)}
              onRedownload={() => handleRedownload(d)}
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
            onRedownload={() => handleRedownload(d)}
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

      {/* Redownload file selection modal */}
      {redownloadTarget && (
        <div
          onClick={() => !redownloadSubmitting && !redownloadFetching && closeRedownloadModal()}
          style={{
            position: "fixed", inset: 0, background: "rgba(0,0,0,0.5)",
            display: "flex", alignItems: "center", justifyContent: "center", zIndex: 1000,
          }}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              background: "var(--color-bg-primary)", borderRadius: 12, padding: 24,
              width: 520, maxHeight: "70vh", display: "flex", flexDirection: "column",
            }}
          >
            {redownloadFetching ? (
              <div style={{ textAlign: "center", padding: "2rem 0", color: "var(--color-label-secondary)", fontSize: "0.85rem" }}>
                正在获取种子文件列表...
              </div>
            ) : overwriteConfirm ? (
              /* Overwrite confirmation */
              <>
                <h3 style={{ margin: "0 0 8px", fontSize: "0.95rem", fontWeight: 700 }}>文件已存在</h3>
                <p style={{ margin: "0 0 12px", fontSize: "0.82rem", color: "var(--color-label-secondary)", lineHeight: 1.5 }}>
                  以下文件已存在于库目录中，继续下载将覆盖现有文件：
                </p>
                <div style={{ flex: 1, overflowY: "auto", marginBottom: 16 }}>
                  {redownloadFiles
                    .filter((f) => redownloadSelected.has(f.index) && redownloadExisting.has(f.name))
                    .map((f) => (
                      <div key={f.index} style={{ fontSize: "0.82rem", padding: "4px 0", color: "var(--color-danger)" }}>
                        {f.name}
                      </div>
                    ))}
                </div>
                <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
                  <button
                    onClick={closeRedownloadModal}
                    style={{
                      background: "var(--color-bg-control)", border: "1px solid var(--color-separator)",
                      borderRadius: 6, padding: "6px 16px", color: "var(--color-label-primary)",
                      cursor: "pointer", fontSize: 13,
                    }}
                  >取消</button>
                  <button
                    onClick={handleRedownloadConfirm}
                    disabled={redownloadSubmitting}
                    style={{
                      background: "var(--color-danger)", border: "none", borderRadius: 6,
                      padding: "6px 16px", color: "#fff", cursor: "pointer", fontSize: 13,
                    }}
                  >{redownloadSubmitting ? "开始中..." : "覆盖下载"}</button>
                </div>
              </>
            ) : (
              /* File selection */
              <>
                <h3 style={{ margin: "0 0 4px" }}>选择下载文件</h3>
                <p style={{ margin: "0 0 12px", fontSize: 12, color: "var(--color-label-secondary)" }}>
                  重新下载: {redownloadTarget.title} · 共 {redownloadFiles.length} 个文件
                </p>

                <div style={{ flex: 1, overflowY: "auto", marginBottom: 16 }}>
                  {redownloadFiles.map((f) => {
                    const exists = redownloadExisting.has(f.name);
                    return (
                      <label
                        key={f.index}
                        style={{
                          display: "flex", alignItems: "center", gap: 8,
                          padding: "6px 0", borderBottom: "1px solid var(--color-separator)",
                          fontSize: 13, cursor: "pointer",
                        }}
                      >
                        <input
                          type="checkbox"
                          checked={redownloadSelected.has(f.index)}
                          onChange={() => {
                            setRedownloadSelected((prev) => {
                              const next = new Set(prev);
                              if (next.has(f.index)) next.delete(f.index);
                              else next.add(f.index);
                              return next;
                            });
                          }}
                        />
                        <span style={{
                          flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap",
                          color: exists ? "var(--color-danger)" : undefined,
                        }}>
                          {f.name}{exists ? " (已存在)" : ""}
                        </span>
                        <span style={{ color: "var(--color-label-tertiary)", flexShrink: 0 }}>
                          {formatSize(f.size)}
                        </span>
                      </label>
                    );
                  })}
                </div>

                <div style={{ display: "flex", gap: 8 }}>
                  <button
                    onClick={() => {
                      if (redownloadSelected.size === redownloadFiles.length) setRedownloadSelected(new Set());
                      else setRedownloadSelected(new Set(redownloadFiles.map((f) => f.index)));
                    }}
                    style={{
                      background: "var(--color-bg-control)", border: "1px solid var(--color-separator)",
                      borderRadius: 6, padding: "6px 16px", color: "var(--color-label-primary)",
                      cursor: "pointer", fontSize: 13,
                    }}
                  >
                    {redownloadSelected.size === redownloadFiles.length ? "取消全选" : "全选"}
                  </button>
                  <div style={{ flex: 1 }} />
                  <button
                    onClick={closeRedownloadModal}
                    style={{
                      background: "var(--color-bg-control)", border: "1px solid var(--color-separator)",
                      borderRadius: 6, padding: "6px 16px", color: "var(--color-label-primary)",
                      cursor: "pointer", fontSize: 13,
                    }}
                  >取消</button>
                  <button
                    onClick={handleRedownloadConfirm}
                    disabled={redownloadSelected.size === 0 || redownloadSubmitting}
                    style={{
                      background: redownloadSelected.size === 0 ? "var(--color-bg-control)" : "var(--color-accent)",
                      color: redownloadSelected.size === 0 ? "var(--color-label-tertiary)" : "#fff",
                      border: "none", borderRadius: 6, padding: "6px 16px",
                      cursor: redownloadSelected.size === 0 ? "not-allowed" : "pointer", fontSize: 13,
                    }}
                  >
                    {redownloadSubmitting ? "开始中..." : `确认下载 (${redownloadSelected.size})`}
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
