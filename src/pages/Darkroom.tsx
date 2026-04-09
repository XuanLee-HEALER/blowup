import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { library, subtitle, media, audio, config } from "../lib/tauri";
import type { IndexEntry, FileMediaInfo, SubtitleSearchResult } from "../lib/tauri";
import { TextInput } from "../components/ui/TextInput";
import { formatSize, formatDuration, formatBitrate, formatFrameRate } from "../lib/format";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

const VIDEO_EXTS = ["mp4", "mkv", "avi", "mov", "ts", "flv", "wmv", "webm", "m4v"];
const SUB_EXTS = ["srt", "ass", "sub", "idx", "vtt"];
const AUDIO_EXTS = ["mp3", "aac", "flac", "opus", "m4a", "wav", "ogg", "ac3", "dts", "mka"];
const getExt = (f: string) => f.split(".").pop()?.toLowerCase() ?? "";
const getStem = (f: string) => f.replace(/\.[^.]+$/, "");

function openWaveformWindow(filePath: string) {
  invoke("open_waveform_window", { filePath }).catch(console.error);
}

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

const AUDIO_FORMATS = [
  { label: "MP3", value: "mp3" },
  { label: "AAC", value: "aac" },
  { label: "FLAC", value: "flac" },
  { label: "Opus", value: "opus" },
  { label: "原始", value: "copy" },
];

function VideoRow({ file, rootPath, tmdbId, cachedInfo, highlighted, onHover, onStatusChange, onRefresh }: {
  file: string; rootPath: string; tmdbId: number; cachedInfo?: FileMediaInfo;
  highlighted: boolean; onHover: (file: string | null) => void;
  onStatusChange: (s: StatusMsg) => void; onRefresh: () => void;
}) {
  const fullPath = `${rootPath}/${file}`;
  const [probeInfo, setProbeInfo] = useState<FileMediaInfo | null>(cachedInfo ?? null);
  const [probing, setProbing] = useState(false);
  const [showExtractAudio, setShowExtractAudio] = useState(false);
  const [extracting, setExtracting] = useState(false);

  const handlePlay = async () => {
    try { await media.openInPlayer(fullPath); }
    catch (e) { onStatusChange({ ok: false, msg: `播放失败: ${e}` }); }
  };

  const handleProbe = async () => {
    setProbing(true);
    try {
      const info = await media.probeAndCache(tmdbId, file);
      setProbeInfo(info);
    } catch (e) { onStatusChange({ ok: false, msg: `获取媒体信息失败: ${e}` }); }
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
      onRefresh();
    } catch (e) { onStatusChange({ ok: false, msg: `提取失败: ${e}` }); }
  };

  const handleExtractAudio = async (format: string) => {
    setExtracting(true);
    setShowExtractAudio(false);
    try {
      await audio.extract(fullPath, 0, format);
      onStatusChange({ ok: true, msg: `音轨提取成功 (${format})` });
      onRefresh();
    } catch (e) { onStatusChange({ ok: false, msg: `音轨提取失败: ${e}` }); }
    finally { setExtracting(false); }
  };

  return (
    <div
      style={{ marginBottom: "0.4rem" }}
      onMouseEnter={() => onHover(file)}
      onMouseLeave={() => onHover(null)}
    >
      <div style={{
        display: "flex", alignItems: "center", gap: "0.4rem",
        padding: "0.5rem 0.75rem",
        background: highlighted ? "var(--color-accent-soft)" : "var(--color-bg-secondary)",
        border: "1px solid var(--color-separator)", borderRadius: 6,
        boxShadow: highlighted ? "inset 3px 0 0 var(--color-accent)" : undefined,
        transition: "background 0.15s",
      }}>
        <span style={{ flex: 1, fontSize: "0.82rem", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {file}
        </span>
        <ActionButton label="▶ 播放" onClick={handlePlay} accent />
        <ActionButton label={probing ? "获取中…" : "媒体信息"} onClick={handleProbe} disabled={probing} />
        <MoreMenu items={[
          { label: "提取字幕轨", onClick: handleExtractSubtitle },
          { label: extracting ? "提取中…" : "提取音轨", onClick: () => setShowExtractAudio(true) },
        ]} />
      </div>
      {showExtractAudio && (
        <div
          style={{
            position: "fixed", inset: 0, zIndex: 100,
            background: "rgba(0,0,0,0.4)",
            display: "flex", alignItems: "center", justifyContent: "center",
          }}
          onClick={() => setShowExtractAudio(false)}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              background: "var(--color-bg-elevated)", borderRadius: 10,
              border: "1px solid var(--color-separator)",
              boxShadow: "0 8px 32px rgba(0,0,0,0.4)",
              padding: "1.2rem 1.5rem", minWidth: 260,
            }}
          >
            <div style={{ fontSize: "0.85rem", fontWeight: 600, marginBottom: "0.75rem" }}>
              选择输出格式
            </div>
            <div style={{ display: "flex", flexDirection: "column", gap: "0.4rem" }}>
              {AUDIO_FORMATS.map((fmt) => (
                <button
                  key={fmt.value}
                  onClick={() => handleExtractAudio(fmt.value)}
                  style={{
                    background: "var(--color-bg-control)",
                    border: "1px solid var(--color-separator)", borderRadius: 6,
                    padding: "0.45rem 0.75rem", fontSize: "0.82rem",
                    color: "var(--color-label-primary)",
                    cursor: "pointer", fontFamily: "inherit", textAlign: "left",
                  }}
                  onMouseEnter={(e) => (e.currentTarget.style.background = "var(--color-hover)")}
                  onMouseLeave={(e) => (e.currentTarget.style.background = "var(--color-bg-control)")}
                >{fmt.label}</button>
              ))}
            </div>
            <div style={{ marginTop: "0.75rem", textAlign: "right" }}>
              <button
                onClick={() => setShowExtractAudio(false)}
                style={{
                  background: "none", border: "none", fontSize: "0.78rem",
                  color: "var(--color-label-tertiary)", cursor: "pointer", fontFamily: "inherit",
                }}
              >取消</button>
            </div>
          </div>
        </div>
      )}
      {probeInfo && <ProbeDetail info={probeInfo} />}
    </div>
  );
}

function ProbeDetail({ info }: { info: FileMediaInfo }) {
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

function SubtitleRow({ file, rootPath, audioFiles, alignedFiles, onStatusChange, onRefresh }: {
  file: string; rootPath: string; audioFiles: string[];
  alignedFiles: string[];
  onStatusChange: (s: StatusMsg) => void; onRefresh: () => void;
}) {
  const fullPath = `${rootPath}/${file}`;
  const [shifting, setShifting] = useState(false);
  const [offsetMs, setOffsetMs] = useState(0);
  const [showShift, setShowShift] = useState(false);
  const [showAlignModal, setShowAlignModal] = useState(false);
  // align states: "idle" | "loading" | "success" | "error"
  const [alignState, setAlignState] = useState<"idle" | "loading" | "success" | "error">("idle");
  const [alignMsg, setAlignMsg] = useState("");
  const [showAlignBubble, setShowAlignBubble] = useState(false);
  const [bubbleExpanded, setBubbleExpanded] = useState(false);
  const bubbleRef = useRef<HTMLDivElement>(null);

  const handleAlignConfirm = (audioFile: string) => {
    setShowAlignModal(false);
    setAlignState("loading");
    setAlignMsg("");
    setShowAlignBubble(false);
    subtitle.alignToAudio(fullPath, `${rootPath}/${audioFile}`)
      .then((result) => {
        setAlignState("success");
        setAlignMsg(result.summary);
        onRefresh();
      })
      .catch((e) => {
        setAlignState("error");
        setAlignMsg(`${e}`);
      });
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
      {/* Main row */}
      <div style={{
        display: "flex", alignItems: "center", gap: "0.4rem",
        padding: "0.4rem 0.75rem", background: "var(--color-bg-secondary)",
        border: "1px solid var(--color-separator)", borderRadius: 6,
      }}>
        <span style={{ flex: 1, fontSize: "0.82rem", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {file}
        </span>
        {/* Align button — all states self-contained */}
        <div style={{ position: "relative", display: "inline-flex", alignItems: "center" }}>
          <button
            disabled={alignState === "loading"}
            onClick={() => {
              if (alignState === "success" || alignState === "error") {
                setShowAlignBubble(!showAlignBubble);
              } else {
                setShowAlignModal(true);
              }
            }}
            style={{
              position: "relative",
              background: alignState === "loading" ? "none" : "none",
              border: "1px solid var(--color-separator)", borderRadius: 4,
              padding: "0.2rem 0.5rem",
              color: alignState === "loading" ? "var(--color-label-quaternary)" : "var(--color-label-secondary)",
              cursor: alignState === "loading" ? "not-allowed" : "pointer",
              fontSize: "0.72rem", fontFamily: "inherit",
              opacity: alignState === "loading" ? 0.6 : 1,
              animation: alignState === "loading" ? "pulse 1.2s infinite" : "none",
            }}
          >
            {alignState === "loading" ? "对齐中…" : "对齐"}
            {/* Status dot */}
            {(alignState === "success" || alignState === "error") && (
              <span style={{
                position: "absolute", top: -2, right: -2,
                width: 7, height: 7, borderRadius: "50%",
                background: alignState === "success" ? "var(--color-success)" : "var(--color-danger)",
              }} />
            )}
          </button>
          {/* Bubble tooltip — click bubble to expand, click outside to close */}
          {showAlignBubble && (alignState === "success" || alignState === "error") && (
            <>
              {/* Invisible overlay to catch outside clicks */}
              <div
                onClick={() => { setShowAlignBubble(false); setBubbleExpanded(false); setAlignState("idle"); }}
                style={{ position: "fixed", inset: 0, zIndex: 49 }}
              />
              <div
                ref={bubbleRef}
                onClick={(e) => { e.stopPropagation(); setBubbleExpanded(!bubbleExpanded); }}
                style={{
                  position: "absolute", top: "100%", right: 0, marginTop: 4, zIndex: 50,
                  background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)",
                  borderRadius: 6, padding: "6px 10px",
                  boxShadow: "0 4px 16px rgba(0,0,0,0.35)",
                  fontSize: "0.72rem", cursor: "pointer",
                  color: alignState === "success" ? "var(--color-success)" : "var(--color-danger)",
                  ...(bubbleExpanded
                    ? { whiteSpace: "pre-wrap", width: 360, wordBreak: "break-word" as const }
                    : { whiteSpace: "nowrap", maxWidth: 220, overflow: "hidden", textOverflow: "ellipsis" }),
                }}
              >
                {alignMsg}
              </div>
            </>
          )}
        </div>
        <MoreMenu items={[
          { label: "查看", onClick: () => subtitle.openViewer(fullPath) },
          { label: "时间偏移", onClick: () => setShowShift(!showShift) },
          { label: "删除", onClick: handleDelete, danger: true },
        ]} />
      </div>

      {/* Align modal — select audio file */}
      {showAlignModal && (
        <div
          style={{
            position: "fixed", inset: 0, zIndex: 100,
            background: "rgba(0,0,0,0.4)",
            display: "flex", alignItems: "center", justifyContent: "center",
          }}
          onClick={() => setShowAlignModal(false)}
        >
          <div
            onClick={(e) => e.stopPropagation()}
            style={{
              background: "var(--color-bg-elevated)", borderRadius: 10,
              border: "1px solid var(--color-separator)",
              boxShadow: "0 8px 32px rgba(0,0,0,0.4)",
              padding: "1.2rem 1.5rem", minWidth: 300, maxWidth: 420,
            }}
          >
            <div style={{ fontSize: "0.85rem", fontWeight: 600, marginBottom: "0.75rem" }}>
              选择对齐目标音频
            </div>
            {audioFiles.length === 0 ? (
              <div style={{ fontSize: "0.82rem", color: "var(--color-label-tertiary)", padding: "0.5rem 0" }}>
                当前目录下没有音频文件。请先从视频中提取音轨。
              </div>
            ) : (
              <div style={{ display: "flex", flexDirection: "column", gap: "0.35rem" }}>
                {audioFiles.map((af) => (
                  <button
                    key={af}
                    onClick={() => handleAlignConfirm(af)}
                    style={{
                      background: "var(--color-bg-control)",
                      border: "1px solid var(--color-separator)", borderRadius: 6,
                      padding: "0.45rem 0.75rem", fontSize: "0.82rem",
                      color: "var(--color-label-primary)",
                      cursor: "pointer", fontFamily: "inherit", textAlign: "left",
                    }}
                    onMouseEnter={(e) => (e.currentTarget.style.background = "var(--color-hover)")}
                    onMouseLeave={(e) => (e.currentTarget.style.background = "var(--color-bg-control)")}
                  >{af}</button>
                ))}
              </div>
            )}
            <div style={{ marginTop: "0.75rem", textAlign: "right" }}>
              <button
                onClick={() => setShowAlignModal(false)}
                style={{
                  background: "none", border: "none", fontSize: "0.78rem",
                  color: "var(--color-label-tertiary)", cursor: "pointer", fontFamily: "inherit",
                }}
              >取消</button>
            </div>
          </div>
        </div>
      )}

      {/* Shift panel */}
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

      {/* Aligned child files */}
      {alignedFiles.map((af) => (
        <div key={af} style={{
          display: "flex", alignItems: "center", gap: "0.4rem",
          padding: "0.3rem 0.75rem 0.3rem 1.8rem",
          marginTop: "0.15rem",
          background: "var(--color-bg-secondary)",
          border: "1px solid var(--color-separator)", borderRadius: 4,
          opacity: 0.85,
        }}>
          <span style={{ fontSize: "0.65rem", color: "var(--color-accent)", marginRight: "0.2rem" }}>↳</span>
          <span style={{ flex: 1, fontSize: "0.78rem", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap", color: "var(--color-label-secondary)" }}>
            {af}
          </span>
          <ActionButton label="查看" onClick={() => subtitle.openViewer(`${rootPath}/${af}`)} />
          <button
            onClick={async () => {
              try {
                await library.deleteLibraryResource(`${rootPath}/${af}`);
                onRefresh();
              } catch (e) { onStatusChange({ ok: false, msg: `删除失败: ${e}` }); }
            }}
            style={{
              background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
              padding: "0.2rem 0.5rem", cursor: "pointer", fontSize: "0.72rem",
              color: "var(--color-danger)", fontFamily: "inherit",
            }}
          >删除</button>
        </div>
      ))}
    </div>
  );
}

// ── Audio resource row ─────────────────────────────────────────

function AudioRow({ file, rootPath, highlighted, onHover, onStatusChange, onRefresh }: {
  file: string; rootPath: string;
  highlighted: boolean; onHover: (file: string | null) => void;
  onStatusChange: (s: StatusMsg) => void; onRefresh: () => void;
}) {
  const fullPath = `${rootPath}/${file}`;

  const handlePlay = () => openWaveformWindow(fullPath);

  const handleDelete = async () => {
    try {
      await library.deleteLibraryResource(fullPath);
      onStatusChange({ ok: true, msg: `已删除 ${file}` });
      onRefresh();
    } catch (e) { onStatusChange({ ok: false, msg: `删除失败: ${e}` }); }
  };

  return (
    <div
      style={{ marginBottom: "0.3rem" }}
      onMouseEnter={() => onHover(file)}
      onMouseLeave={() => onHover(null)}
    >
      <div style={{
        display: "flex", alignItems: "center", gap: "0.4rem",
        padding: "0.4rem 0.75rem",
        background: highlighted ? "var(--color-accent-soft)" : "var(--color-bg-secondary)",
        border: "1px solid var(--color-separator)", borderRadius: 6,
        boxShadow: highlighted ? "inset 3px 0 0 var(--color-accent)" : undefined,
        transition: "background 0.15s",
      }}>
        <span style={{ flex: 1, fontSize: "0.82rem", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {file}
        </span>
        <ActionButton label="▶ 播放" onClick={handlePlay} accent />
        <button
          onClick={handleDelete}
          style={{
            background: "none", border: "1px solid var(--color-separator)", borderRadius: 4,
            padding: "0.2rem 0.5rem", cursor: "pointer", fontSize: "0.72rem",
            color: "var(--color-danger)", fontFamily: "inherit",
          }}
        >删除</button>
      </div>
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
  const [subResults, setSubResults] = useState<SubtitleSearchResult[] | null>(null);
  const [downloadingSub, setDownloadingSub] = useState<string | null>(null);
  const [hoveredVideo, setHoveredVideo] = useState<string | null>(null);
  const [hoveredAudio, setHoveredAudio] = useState<string | null>(null);

  // Refresh file list from index
  const refreshFiles = useCallback(async () => {
    await library.refreshIndexEntry(entry.tmdb_id);
    const entries = await library.listIndexEntries();
    const updated = entries.find((e) => e.tmdb_id === entry.tmdb_id);
    if (updated) setFiles(updated.files);
  }, [entry.tmdb_id]);

  const videoFiles = files.filter((f) => VIDEO_EXTS.includes(getExt(f)));
  const allSubtitleFiles = files.filter((f) => SUB_EXTS.includes(getExt(f)));
  // Split subtitles into primary (non-aligned) and aligned child files
  const isAligned = (f: string) => f.includes(".aligned.");
  const subtitleFiles = allSubtitleFiles.filter((f) => !isAligned(f));
  // Map each primary subtitle to its aligned children
  const getAlignedFiles = (parentFile: string) => {
    const stem = getStem(parentFile); // e.g. "sub.zh" from "sub.zh.srt"
    return allSubtitleFiles.filter((f) => isAligned(f) && f.startsWith(stem + ".aligned."));
  };
  const audioFiles = files.filter((f) => AUDIO_EXTS.includes(getExt(f)));
  const otherFiles = files.filter((f) =>
    !VIDEO_EXTS.includes(getExt(f)) && !SUB_EXTS.includes(getExt(f)) && !AUDIO_EXTS.includes(getExt(f))
  );
  const primaryVideo = videoFiles[0] ?? null;

  // Association: video stem matches audio filename prefix "{stem}_audio_"
  const isAudioLinkedToVideo = (audioFile: string, videoFile: string) =>
    audioFile.startsWith(`${getStem(videoFile)}_audio_`);

  const isVideoHighlighted = (videoFile: string) => {
    if (hoveredAudio && isAudioLinkedToVideo(hoveredAudio, videoFile)) return true;
    return false;
  };

  const isAudioHighlighted = (audioFile: string) => {
    if (hoveredVideo && isAudioLinkedToVideo(audioFile, hoveredVideo)) return true;
    return false;
  };

  const handleSearchSubtitle = async () => {
    if (!primaryVideo) {
      setStatus({ ok: false, msg: "无视频文件，无法搜索字幕" });
      return;
    }
    setFetchingSub(true);
    setStatus(null);
    setSubResults(null);
    try {
      const results = await subtitle.search(
        `${rootPath}/${primaryVideo}`, fetchingLang,
        entry.title, entry.year ?? undefined, entry.tmdb_id,
      );
      if (results.length === 0) {
        setStatus({ ok: false, msg: "未找到字幕" });
      } else {
        setSubResults(results);
      }
    } catch (e) { setStatus({ ok: false, msg: `字幕搜索失败: ${e}` }); }
    finally { setFetchingSub(false); }
  };

  const handleDownloadSubtitle = async (downloadId: string) => {
    if (!primaryVideo) return;
    setDownloadingSub(downloadId);
    try {
      await subtitle.download(`${rootPath}/${primaryVideo}`, fetchingLang, downloadId);
      setStatus({ ok: true, msg: "字幕下载成功" });
      setSubResults(null);
      await refreshFiles();
    } catch (e) { setStatus({ ok: false, msg: `字幕下载失败: ${e}` }); }
    finally { setDownloadingSub(null); }
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
            <VideoRow
              key={f} file={f} rootPath={rootPath} tmdbId={entry.tmdb_id}
              cachedInfo={entry.media_info?.[f]}
              highlighted={isVideoHighlighted(f)}
              onHover={setHoveredVideo}
              onStatusChange={setStatus}
              onRefresh={refreshFiles}
            />
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
              audioFiles={audioFiles}
              alignedFiles={getAlignedFiles(f)}
              onStatusChange={setStatus}
              onRefresh={refreshFiles}
            />
          ))}
        </div>
      )}

      {/* Subtitle actions */}
      <div style={{ display: "flex", gap: "0.4rem", alignItems: "center", marginBottom: subResults ? "0.5rem" : "1.2rem" }}>
        <ActionButton label={fetchingSub ? "搜索中…" : "+ 搜索字幕"} onClick={handleSearchSubtitle} disabled={!primaryVideo || fetchingSub} />
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

      {/* Subtitle search results */}
      {subResults && (
        <div style={{ marginBottom: "1.2rem" }}>
          {subResults.map((r) => (
            <div
              key={r.download_id}
              style={{
                display: "flex", alignItems: "center", gap: "0.5rem",
                padding: "0.4rem 0.75rem", marginBottom: "0.25rem",
                background: "var(--color-bg-secondary)",
                border: "1px solid var(--color-separator)", borderRadius: 6,
              }}
            >
              <span style={{
                fontSize: "0.6rem", fontWeight: 600, padding: "0.1rem 0.3rem",
                borderRadius: 3, flexShrink: 0,
                background: r.source === "assrt" ? "rgba(255,149,0,0.15)" : "rgba(100,149,237,0.15)",
                color: r.source === "assrt" ? "rgb(255,149,0)" : "rgb(100,149,237)",
              }}>
                {r.source === "assrt" ? "ASSRT" : "OS"}
              </span>
              <span style={{ flex: 1, fontSize: "0.78rem", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {r.title}
              </span>
              {r.language && (
                <span style={{ fontSize: "0.65rem", color: "var(--color-label-quaternary)", flexShrink: 0 }}>
                  {r.language}
                </span>
              )}
              {r.download_count != null && (
                <span style={{ fontSize: "0.68rem", color: "var(--color-label-quaternary)", flexShrink: 0 }}>
                  {r.download_count} 次
                </span>
              )}
              <ActionButton
                label={downloadingSub === r.download_id ? "下载中…" : "下载"}
                onClick={() => handleDownloadSubtitle(r.download_id)}
                disabled={downloadingSub !== null}
                accent
              />
            </div>
          ))}
          <button
            onClick={() => setSubResults(null)}
            style={{
              background: "none", border: "none", fontSize: "0.7rem",
              color: "var(--color-label-tertiary)", cursor: "pointer",
              fontFamily: "inherit", marginTop: "0.2rem",
            }}
          >关闭结果</button>
        </div>
      )}

      {/* Audio section */}
      {audioFiles.length > 0 && (
        <>
          <SectionHeader>音频</SectionHeader>
          <div style={{ marginBottom: "1rem" }}>
            {audioFiles.map((f) => (
              <AudioRow
                key={f} file={f} rootPath={rootPath}
                highlighted={isAudioHighlighted(f)}
                onHover={setHoveredAudio}
                onStatusChange={setStatus}
                onRefresh={refreshFiles}
              />
            ))}
          </div>
        </>
      )}

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
      <style>{`@keyframes pulse { 0%,100% { opacity: 1; } 50% { opacity: 0.4; } }`}</style>
      {/* Left: film selector */}
      <div style={{ width: 240, flexShrink: 0, borderRight: "1px solid var(--color-separator)", display: "flex", flexDirection: "column", overflow: "hidden", userSelect: "none", WebkitUserSelect: "none" }}>
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

        <div style={{ flex: 1, overflowY: "auto", padding: "0.5rem 0", userSelect: "none", WebkitUserSelect: "none" }}>
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
