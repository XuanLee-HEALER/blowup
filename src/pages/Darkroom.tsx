import { useState, useEffect, useCallback, useRef } from "react";
import { library, subtitle, media, config } from "../lib/tauri";
import type { IndexEntry, MediaInfo } from "../lib/tauri";
import { TextInput } from "../components/ui/TextInput";
import { formatSize, formatDuration, formatBitrate, formatFrameRate } from "../lib/format";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

const VIDEO_EXTS = ["mp4", "mkv", "avi", "mov", "ts", "flv", "wmv", "webm", "m4v"];
const SUB_EXTS = ["srt", "ass", "sub", "idx", "vtt"];
const getExt = (f: string) => f.split(".").pop()?.toLowerCase() ?? "";

// ── Types ────────────────────────────────────────────────────────

interface StatusMsg { ok: boolean; msg: string }

// ── Shared small components ──────────────────────────────────────

function StatusBadge({ status }: { status: StatusMsg | null }) {
  if (!status) return null;
  return (
    <span style={{ fontSize: "0.72rem", color: status.ok ? "var(--color-success)" : "var(--color-danger)" }}>
      {status.ok ? "✓ " : "✗ "}{status.msg}
    </span>
  );
}

function ActionButton({ label, onClick, disabled, accent }: {
  label: string; onClick: () => void; disabled?: boolean; accent?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      style={{
        background: accent ? "var(--color-accent)" : "none",
        border: accent ? "none" : "1px solid var(--color-separator)",
        borderRadius: 4,
        padding: "0.2rem 0.5rem",
        color: accent ? "#fff" : "var(--color-label-secondary)",
        cursor: disabled ? "not-allowed" : "pointer",
        fontSize: "0.72rem",
        fontFamily: "inherit",
        fontWeight: accent ? 600 : 400,
        opacity: disabled ? 0.5 : 1,
      }}
    >
      {label}
    </button>
  );
}

function MoreMenu({ items }: { items: { label: string; onClick: () => void; danger?: boolean }[] }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  return (
    <div ref={ref} style={{ position: "relative" }}>
      <button
        onClick={() => setOpen(!open)}
        style={{
          background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
          padding: "0.2rem 0.35rem", cursor: "pointer", fontSize: "0.72rem",
          color: "var(--color-label-tertiary)", fontFamily: "inherit", lineHeight: 1,
        }}
      >⋮</button>
      {open && (
        <div style={{
          position: "absolute", right: 0, top: "100%", marginTop: 4, zIndex: 50,
          background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)",
          borderRadius: 6, padding: "4px 0", boxShadow: "0 4px 16px rgba(0,0,0,0.35)",
          minWidth: 120,
        }}>
          {items.map((item, i) => (
            <div
              key={i}
              onClick={() => { item.onClick(); setOpen(false); }}
              style={{
                padding: "6px 14px", fontSize: "0.78rem", cursor: "pointer",
                color: item.danger ? "var(--color-danger)" : "var(--color-label-primary)",
              }}
              onMouseEnter={(e) => (e.currentTarget.style.background = item.danger ? "var(--color-danger-soft)" : "var(--color-hover)")}
              onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
            >
              {item.label}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

// ── Video resource row ──────────────────────────────────────────

function VideoRow({ file, rootPath, onStatusChange }: {
  file: string; rootPath: string; onStatusChange: (s: StatusMsg) => void;
}) {
  const fullPath = `${rootPath}/${file}`;
  const [probeInfo, setProbeInfo] = useState<MediaInfo | null>(null);
  const [probing, setProbing] = useState(false);

  const handlePlay = async () => {
    try { await media.openInPlayer(fullPath); }
    catch (e) { onStatusChange({ ok: false, msg: `播放失败: ${e}` }); }
  };

  const handleProbe = async () => {
    setProbing(true);
    try {
      const info = await media.probeDetail(fullPath);
      setProbeInfo(info);
    } catch (e) { onStatusChange({ ok: false, msg: `探测失败: ${e}` }); }
    finally { setProbing(false); }
  };

  const handleExtractSubtitle = async () => {
    try {
      const streams = await subtitle.listStreams(fullPath);
      if (streams.length === 0) {
        onStatusChange({ ok: false, msg: "未找到内嵌字幕轨" });
        return;
      }
      await subtitle.extract(fullPath, streams[0].index);
      onStatusChange({ ok: true, msg: "字幕提取成功" });
    } catch (e) { onStatusChange({ ok: false, msg: `提取失败: ${e}` }); }
  };

  return (
    <div style={{ marginBottom: "0.4rem" }}>
      <div style={{
        display: "flex", alignItems: "center", gap: "0.4rem",
        padding: "0.5rem 0.75rem", background: "var(--color-bg-secondary)",
        border: "1px solid var(--color-separator)", borderRadius: 6,
      }}>
        <span style={{ flex: 1, fontSize: "0.82rem", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {file}
        </span>
        <ActionButton label="▶ 播放" onClick={handlePlay} accent />
        <ActionButton label={probing ? "探测中…" : "探测"} onClick={handleProbe} disabled={probing} />
        <MoreMenu items={[
          { label: "提取字幕轨", onClick: handleExtractSubtitle },
        ]} />
      </div>
      {probeInfo && <ProbeDetail info={probeInfo} />}
    </div>
  );
}

function ProbeDetail({ info }: { info: MediaInfo }) {
  return (
    <div style={{ padding: "0.5rem 0.75rem 0.25rem 1.5rem", fontSize: "0.75rem", color: "var(--color-label-secondary)" }}>
      <div style={{ display: "flex", gap: "1rem", flexWrap: "wrap", marginBottom: "0.3rem" }}>
        <span>格式: {info.format_name ?? "—"}</span>
        <span>大小: {formatSize(info.file_size)}</span>
        <span>时长: {formatDuration(info.duration_secs)}</span>
        <span>码率: {formatBitrate(info.bit_rate)}</span>
      </div>
      {info.streams.map((s) => (
        <div key={s.index} style={{ fontSize: "0.7rem", color: "var(--color-label-tertiary)", marginBottom: "0.15rem" }}>
          #{s.index} {s.codec_type} — {s.codec_name}
          {s.codec_type === "video" && s.width && s.height && ` ${s.width}x${s.height}`}
          {s.codec_type === "video" && ` ${formatFrameRate(s.frame_rate)}`}
          {s.codec_type === "audio" && s.channels && ` ${s.channels}ch`}
          {s.codec_type === "audio" && s.sample_rate && ` ${s.sample_rate}Hz`}
          {s.language && ` (${s.language})`}
          {s.title && ` "${s.title}"`}
        </div>
      ))}
    </div>
  );
}

// ── Subtitle resource row ───────────────────────────────────────

function SubtitleRow({ file, rootPath, videoFile, onStatusChange, onRefresh }: {
  file: string; rootPath: string; videoFile: string | null;
  onStatusChange: (s: StatusMsg) => void; onRefresh: () => void;
}) {
  const fullPath = `${rootPath}/${file}`;
  const [shifting, setShifting] = useState(false);
  const [offsetMs, setOffsetMs] = useState(0);
  const [showShift, setShowShift] = useState(false);

  const handleAlign = async () => {
    if (!videoFile) {
      onStatusChange({ ok: false, msg: "目录中没有视频文件，无法对齐" });
      return;
    }
    const videoPath = `${rootPath}/${videoFile}`;
    try {
      await subtitle.align(videoPath, fullPath);
      onStatusChange({ ok: true, msg: `${file} 对齐完成` });
    } catch (e) { onStatusChange({ ok: false, msg: `对齐失败: ${e}` }); }
  };

  const handleShift = async () => {
    if (offsetMs === 0) return;
    setShifting(true);
    try {
      await subtitle.shift(fullPath, offsetMs);
      onStatusChange({ ok: true, msg: `${file} 偏移 ${offsetMs > 0 ? "+" : ""}${offsetMs}ms 完成` });
      setShowShift(false);
      setOffsetMs(0);
    } catch (e) { onStatusChange({ ok: false, msg: `偏移失败: ${e}` }); }
    finally { setShifting(false); }
  };

  const handleDelete = async () => {
    try {
      await library.deleteLibraryResource(fullPath);
      onStatusChange({ ok: true, msg: `已删除 ${file}` });
      onRefresh();
    } catch (e) { onStatusChange({ ok: false, msg: `删除失败: ${e}` }); }
  };

  return (
    <div style={{ marginBottom: "0.3rem" }}>
      <div style={{
        display: "flex", alignItems: "center", gap: "0.4rem",
        padding: "0.4rem 0.75rem", background: "var(--color-bg-secondary)",
        border: "1px solid var(--color-separator)", borderRadius: 6,
      }}>
        <span style={{ flex: 1, fontSize: "0.82rem", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {file}
        </span>
        <ActionButton label="对齐" onClick={handleAlign} disabled={!videoFile} />
        <MoreMenu items={[
          { label: "时间偏移", onClick: () => setShowShift(!showShift) },
          { label: "删除", onClick: handleDelete, danger: true },
        ]} />
      </div>
      {showShift && (
        <div style={{ display: "flex", alignItems: "center", gap: "0.4rem", padding: "0.35rem 0.75rem 0.35rem 1.5rem" }}>
          <input
            type="number"
            value={offsetMs}
            onChange={(e) => setOffsetMs(Number(e.target.value))}
            style={{
              width: 80, padding: "0.2rem 0.4rem", background: "var(--color-bg-elevated)",
              border: "1px solid var(--color-separator)", borderRadius: 4,
              color: "var(--color-label-primary)", fontSize: "0.75rem", fontFamily: "inherit",
            }}
          />
          <span style={{ fontSize: "0.7rem", color: "var(--color-label-tertiary)" }}>ms</span>
          {[-1000, -500, 500, 1000].map((v) => (
            <button key={v} onClick={() => setOffsetMs((p) => p + v)} style={{
              background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
              padding: "0.1rem 0.3rem", fontSize: "0.65rem", color: "var(--color-label-tertiary)",
              cursor: "pointer", fontFamily: "inherit",
            }}>{v > 0 ? `+${v}` : v}</button>
          ))}
          <ActionButton label={shifting ? "处理中…" : "应用"} onClick={handleShift} disabled={offsetMs === 0 || shifting} accent />
        </div>
      )}
    </div>
  );
}

// ── Workspace panel (right side) ────────────────────────────────

function WorkspacePanel({ entry, rootDir }: { entry: IndexEntry; rootDir: string }) {
  const rootPath = `${rootDir}/${entry.path}`;
  const [status, setStatus] = useState<StatusMsg | null>(null);
  const [files, setFiles] = useState(entry.files);
  const [fetchingLang, setFetchingLang] = useState("zh");
  const [fetchingSub, setFetchingSub] = useState(false);

  // Refresh file list from index
  const refreshFiles = useCallback(async () => {
    await library.refreshIndexEntry(entry.tmdb_id);
    const entries = await library.listIndexEntries();
    const updated = entries.find((e) => e.tmdb_id === entry.tmdb_id);
    if (updated) setFiles(updated.files);
  }, [entry.tmdb_id]);

  const videoFiles = files.filter((f) => VIDEO_EXTS.includes(getExt(f)));
  const subtitleFiles = files.filter((f) => SUB_EXTS.includes(getExt(f)));
  const otherFiles = files.filter((f) => !VIDEO_EXTS.includes(getExt(f)) && !SUB_EXTS.includes(getExt(f)));
  const primaryVideo = videoFiles[0] ?? null;

  const handleFetchSubtitle = async () => {
    if (!primaryVideo) {
      setStatus({ ok: false, msg: "无视频文件，无法搜索字幕" });
      return;
    }
    setFetchingSub(true);
    setStatus(null);
    try {
      await subtitle.fetch(`${rootPath}/${primaryVideo}`, fetchingLang);
      setStatus({ ok: true, msg: "字幕下载成功" });
      await refreshFiles();
    } catch (e) { setStatus({ ok: false, msg: `字幕搜索失败: ${e}` }); }
    finally { setFetchingSub(false); }
  };

  const handleExtractAllSubs = async () => {
    if (!primaryVideo) return;
    setStatus(null);
    try {
      const streams = await subtitle.listStreams(`${rootPath}/${primaryVideo}`);
      if (streams.length === 0) {
        setStatus({ ok: false, msg: "未找到内嵌字幕轨" });
        return;
      }
      for (const s of streams) {
        await subtitle.extract(`${rootPath}/${primaryVideo}`, s.index);
      }
      setStatus({ ok: true, msg: `提取了 ${streams.length} 条字幕轨` });
      await refreshFiles();
    } catch (e) { setStatus({ ok: false, msg: `提取失败: ${e}` }); }
  };

  return (
    <div>
      {/* Header */}
      <div style={{ marginBottom: "1.2rem" }}>
        <h2 style={{ margin: "0 0 0.2rem", fontSize: "1.1rem", fontWeight: 700, letterSpacing: "-0.02em" }}>
          {entry.title}
        </h2>
        <div style={{ fontSize: "0.78rem", color: "var(--color-label-tertiary)" }}>
          {entry.director_display} · {entry.year ?? "—"}
        </div>
        <div style={{ fontSize: "0.7rem", color: "var(--color-label-quaternary)", marginTop: "0.2rem" }}>
          {rootPath}
        </div>
      </div>

      {/* Status */}
      {status && (
        <div style={{ marginBottom: "0.75rem" }}>
          <StatusBadge status={status} />
        </div>
      )}

      {/* Video section */}
      <SectionHeader>视频</SectionHeader>
      {videoFiles.length === 0 ? (
        <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.82rem", marginBottom: "1rem" }}>无视频文件</p>
      ) : (
        <div style={{ marginBottom: "1rem" }}>
          {videoFiles.map((f) => (
            <VideoRow key={f} file={f} rootPath={rootPath} onStatusChange={setStatus} />
          ))}
        </div>
      )}

      {/* Subtitle section */}
      <SectionHeader>字幕</SectionHeader>
      {subtitleFiles.length === 0 ? (
        <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.82rem", marginBottom: "0.5rem" }}>无字幕文件</p>
      ) : (
        <div style={{ marginBottom: "0.5rem" }}>
          {subtitleFiles.map((f) => (
            <SubtitleRow
              key={f}
              file={f}
              rootPath={rootPath}
              videoFile={primaryVideo}
              onStatusChange={setStatus}
              onRefresh={refreshFiles}
            />
          ))}
        </div>
      )}

      {/* Subtitle actions */}
      <div style={{ display: "flex", gap: "0.4rem", alignItems: "center", marginBottom: "1.2rem" }}>
        <ActionButton label={fetchingSub ? "搜索中…" : "+ 获取字幕"} onClick={handleFetchSubtitle} disabled={!primaryVideo || fetchingSub} />
        <select
          value={fetchingLang}
          onChange={(e) => setFetchingLang(e.target.value)}
          style={{
            padding: "0.2rem 0.3rem", background: "var(--color-bg-elevated)",
            border: "1px solid var(--color-separator)", borderRadius: 4,
            color: "var(--color-label-secondary)", fontSize: "0.72rem", fontFamily: "inherit",
          }}
        >
          <option value="zh">中文</option>
          <option value="en">English</option>
          <option value="ja">日本語</option>
          <option value="ko">한국어</option>
          <option value="fr">Français</option>
        </select>
        <ActionButton label="+ 从视频提取" onClick={handleExtractAllSubs} disabled={!primaryVideo} />
      </div>

      {/* Other files */}
      {otherFiles.length > 0 && (
        <>
          <SectionHeader>其他文件</SectionHeader>
          <div style={{ marginBottom: "1rem" }}>
            {otherFiles.map((f) => (
              <div key={f} style={{
                padding: "0.3rem 0.75rem", fontSize: "0.78rem",
                color: "var(--color-label-tertiary)",
              }}>{f}</div>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <p style={{
      margin: "0 0 0.5rem", fontSize: "0.7rem", color: "var(--color-label-quaternary)",
      letterSpacing: "0.08em", textTransform: "uppercase",
    }}>
      {children}
    </p>
  );
}

// ── Main Darkroom page ──────────────────────────────────────────

export default function Darkroom() {
  const [directorMap, setDirectorMap] = useState<Record<string, IndexEntry[]>>({});
  const [selectedDirector, setSelectedDirector] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<IndexEntry | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<IndexEntry[] | null>(null);
  const [rootDir, setRootDir] = useState("");

  const refreshDirectorMap = useCallback(() => {
    library.listIndexByDirector().then(setDirectorMap);
  }, []);

  useEffect(() => {
    refreshDirectorMap();
    config.get().then((c) => setRootDir(c.library.root_dir));
  }, [refreshDirectorMap]);

  useBackendEvent(BackendEvent.LIBRARY_CHANGED, refreshDirectorMap);

  const doSearch = async () => {
    if (!searchQuery.trim()) { setSearchResults(null); return; }
    const results = await library.searchIndex(searchQuery.trim());
    setSearchResults(results);
  };

  const directors = Object.keys(directorMap).sort();

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      {/* Left: film selector */}
      <div style={{ width: 240, flexShrink: 0, borderRight: "1px solid var(--color-separator)", display: "flex", flexDirection: "column", overflow: "hidden" }}>
        <div style={{ padding: "1.4rem 1rem 0" }}>
          <h1 style={{ fontSize: "1.3rem", fontWeight: 700, letterSpacing: "-0.035em", marginBottom: "0.8rem" }}>
            暗房
          </h1>
          <TextInput
            leadingIcon="⌕"
            placeholder="搜索影片…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            onKeyDown={(e) => { if (e.key === "Enter" && !e.nativeEvent.isComposing) doSearch(); }}
            style={{ marginBottom: "0.5rem" }}
          />
          {searchResults && (
            <div style={{ marginBottom: "0.5rem" }}>
              <button onClick={() => { setSearchResults(null); setSearchQuery(""); }} style={{
                background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
                padding: "0.2rem 0.5rem", color: "var(--color-label-tertiary)", cursor: "pointer",
                fontSize: "0.65rem", fontFamily: "inherit",
              }}>清除搜索</button>
            </div>
          )}
        </div>

        <div style={{ height: 1, background: "var(--color-separator)", margin: "0 1rem" }} />

        <div style={{ flex: 1, overflowY: "auto", padding: "0.5rem 0", userSelect: "none" }}>
          {searchResults ? (
            searchResults.map((e) => (
              <div
                key={e.tmdb_id}
                onClick={() => setSelectedEntry(e)}
                style={{
                  padding: "0.5rem 1rem", cursor: "pointer", fontSize: "0.82rem",
                  background: selectedEntry?.tmdb_id === e.tmdb_id ? "var(--color-bg-elevated)" : "transparent",
                }}
              >
                <div style={{ fontWeight: 500 }}>{e.title}</div>
                <div style={{ fontSize: "0.7rem", color: "var(--color-label-tertiary)" }}>
                  {e.director_display} · {e.year ?? "—"}
                </div>
              </div>
            ))
          ) : (
            directors.map((dir) => (
              <div key={dir}>
                <div
                  onClick={() => { setSelectedDirector(selectedDirector === dir ? null : dir); }}
                  style={{
                    padding: "0.45rem 1rem", cursor: "pointer", fontSize: "0.82rem",
                    fontWeight: 600, display: "flex", justifyContent: "space-between",
                    background: selectedDirector === dir ? "var(--color-bg-elevated)" : "transparent",
                  }}
                >
                  <span>{dir}</span>
                  <span style={{ fontSize: "0.7rem", color: "var(--color-label-quaternary)" }}>
                    {directorMap[dir].length}
                  </span>
                </div>
                {selectedDirector === dir && directorMap[dir].map((e) => (
                  <div
                    key={e.tmdb_id}
                    onClick={() => setSelectedEntry(e)}
                    style={{
                      padding: "0.35rem 1rem 0.35rem 1.8rem", cursor: "pointer", fontSize: "0.78rem",
                      background: selectedEntry?.tmdb_id === e.tmdb_id ? "var(--color-hover)" : "transparent",
                    }}
                  >
                    <span>{e.title}</span>
                    <span style={{ marginLeft: "0.4rem", fontSize: "0.68rem", color: "var(--color-label-quaternary)" }}>
                      {e.year ?? ""}
                    </span>
                  </div>
                ))}
              </div>
            ))
          )}
          {!searchResults && directors.length === 0 && (
            <p style={{ padding: "1rem", color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>
              暂无影片。通过搜索页下载电影后即可使用暗房。
            </p>
          )}
        </div>
      </div>

      {/* Right: workspace */}
      <div style={{ flex: 1, overflowY: "auto", padding: "1.4rem 1.75rem" }}>
        {selectedEntry && rootDir ? (
          <WorkspacePanel key={selectedEntry.tmdb_id} entry={selectedEntry} rootDir={rootDir} />
        ) : (
          <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>
            选择一部电影开始工作
          </div>
        )}
      </div>
    </div>
  );
}
