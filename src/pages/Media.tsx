import { useState } from "react";
import { media } from "../lib/tauri";
import type { MediaInfo, StreamInfo } from "../lib/tauri";
import { open } from "@tauri-apps/plugin-dialog";
import { formatSize, formatDuration, formatBitrate, formatFrameRate } from "../lib/format";

function StreamCard({ stream }: { stream: StreamInfo }) {
  const isVideo = stream.codec_type === "video";
  const isAudio = stream.codec_type === "audio";
  const isSub = stream.codec_type === "subtitle";
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
        #{stream.index} {typeLabel} — {stream.codec_name}
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
        {isAudio && stream.channels && <span>{stream.channels}ch</span>}
        {isAudio && stream.sample_rate && <span>{stream.sample_rate} Hz</span>}
        {stream.bit_rate && <span>{formatBitrate(stream.bit_rate)}</span>}
      </div>
    </div>
  );
}

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
        <div style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 20 }}>
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
          <div style={{ color: "#e53935", fontSize: 13, marginBottom: 16 }}>{error}</div>
        )}

        {info && (
          <>
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

            {videoStreams.length > 0 && (
              <div style={{ marginBottom: 16 }}>
                <h3 style={{ fontSize: 14, marginBottom: 8 }}>视频轨 ({videoStreams.length})</h3>
                {videoStreams.map((s) => <StreamCard key={s.index} stream={s} />)}
              </div>
            )}

            {audioStreams.length > 0 && (
              <div style={{ marginBottom: 16 }}>
                <h3 style={{ fontSize: 14, marginBottom: 8 }}>音频轨 ({audioStreams.length})</h3>
                {audioStreams.map((s) => <StreamCard key={s.index} stream={s} />)}
              </div>
            )}

            {subStreams.length > 0 && (
              <div style={{ marginBottom: 16 }}>
                <h3 style={{ fontSize: 14, marginBottom: 8 }}>字幕轨 ({subStreams.length})</h3>
                {subStreams.map((s) => <StreamCard key={s.index} stream={s} />)}
              </div>
            )}

            {otherStreams.length > 0 && (
              <div style={{ marginBottom: 16 }}>
                <h3 style={{ fontSize: 14, marginBottom: 8 }}>其他 ({otherStreams.length})</h3>
                {otherStreams.map((s) => <StreamCard key={s.index} stream={s} />)}
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
