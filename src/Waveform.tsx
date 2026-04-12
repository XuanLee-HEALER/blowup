import { useState, useEffect, useRef, useCallback } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import WaveSurfer from "wavesurfer.js";
import { audio as audioApi, WAVEFORM_PEAKS_SAMPLE_RATE } from "./lib/tauri";

function formatTime(secs: number): string {
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function Waveform() {
  const containerRef = useRef<HTMLDivElement>(null);
  const audioRef = useRef<HTMLAudioElement>(null);
  const wsRef = useRef<WaveSurfer | null>(null);
  const [playing, setPlaying] = useState(false);
  const [currentTime, setCurrentTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [loading, setLoading] = useState(true);
  const [loadingStage, setLoadingStage] = useState("准备中...");
  const [error, setError] = useState<string | null>(null);

  const filePath = new URLSearchParams(window.location.search).get("file") ?? "";
  const parts = filePath.replace(/\\/g, "/").split("/");
  const fileName = parts[parts.length - 1] || filePath;
  const audioUrl = filePath ? convertFileSrc(filePath) : "";

  useEffect(() => {
    if (!filePath || !containerRef.current || !audioRef.current) return;

    let cancelled = false;
    let ws: WaveSurfer | null = null;

    async function init() {
      try {
        // Fetch pre-computed peaks from the Rust backend (ffmpeg
        // downsamples the audio to mono f32le @ 100 Hz and caches the
        // result next to the audio file). This replaces WaveSurfer's
        // built-in `decodeAudioData` path which would otherwise decode
        // the entire multi-hundred-MB AAC stream into PCM in RAM and
        // hang the window for 60+ seconds on multi-channel tracks.
        setLoadingStage("生成波形峰值…");
        const buf = await audioApi.getPeaks(filePath);
        if (cancelled) return;
        const samples = new Float32Array(buf);
        const totalSecs = samples.length / WAVEFORM_PEAKS_SAMPLE_RATE;

        setLoadingStage("渲染中…");
        ws = WaveSurfer.create({
          container: containerRef.current!,
          waveColor: "rgba(100, 149, 237, 0.5)",
          progressColor: "rgba(100, 149, 237, 0.9)",
          cursorColor: "#fff",
          cursorWidth: 1,
          height: 160,
          barWidth: 2,
          barGap: 1,
          barRadius: 2,
          normalize: true,
          media: audioRef.current!,
          peaks: [samples],
          duration: totalSecs,
        });

        ws.on("ready", () => {
          setDuration(ws!.getDuration() || totalSecs);
          setLoading(false);
        });
        ws.on("audioprocess", () => setCurrentTime(ws!.getCurrentTime()));
        ws.on("seeking", () => setCurrentTime(ws!.getCurrentTime()));
        ws.on("play", () => setPlaying(true));
        ws.on("pause", () => setPlaying(false));
        ws.on("finish", () => setPlaying(false));
        ws.on("error", (err: unknown) => {
          setError(String(err));
          setLoading(false);
        });

        // WaveSurfer with pre-computed peaks + media element is
        // "ready" synchronously after create() — some versions don't
        // fire the 'ready' event in that path. Force-clear loading.
        setDuration(totalSecs);
        setLoading(false);

        wsRef.current = ws;
      } catch (e) {
        if (!cancelled) {
          setError(String(e));
          setLoading(false);
        }
      }
    }
    init();

    return () => {
      cancelled = true;
      ws?.destroy();
      wsRef.current = null;
    };
  }, [filePath]);

  const togglePlay = useCallback(() => {
    wsRef.current?.playPause();
  }, []);

  const skipBy = useCallback((secs: number) => {
    const ws = wsRef.current;
    if (!ws) return;
    const t = Math.max(0, Math.min(ws.getDuration(), ws.getCurrentTime() + secs));
    ws.seekTo(t / ws.getDuration());
  }, []);

  if (!filePath) {
    return (
      <div style={styles.container}>
        <div style={styles.error}>未指定音频文件</div>
      </div>
    );
  }

  return (
    <div style={styles.container}>
      {/* Hidden audio element — wavesurfer uses it as media source */}
      <audio ref={audioRef} src={audioUrl} preload="auto" style={{ display: "none" }} />

      {/* Title bar */}
      <div style={styles.titleBar} data-tauri-drag-region>
        <span style={styles.fileName}>{fileName}</span>
        {duration > 0 && (
          <span style={styles.duration}>{formatTime(duration)}</span>
        )}
      </div>

      {/* Waveform */}
      <div style={styles.waveformArea}>
        {loading && <div style={styles.loading}>{loadingStage}</div>}
        {error && <div style={styles.error}>{error}</div>}
        <div ref={containerRef} style={{ width: "100%", opacity: loading ? 0 : 1 }} />
      </div>

      {/* Controls */}
      <div style={styles.controls}>
        <button onClick={() => skipBy(-5)} style={styles.btn} title="-5s">
          -5
        </button>
        <button onClick={togglePlay} style={styles.playBtn}>
          {playing ? "⏸" : "▶"}
        </button>
        <button onClick={() => skipBy(5)} style={styles.btn} title="+5s">
          +5
        </button>
        <span style={styles.time}>
          {formatTime(currentTime)} / {formatTime(duration)}
        </span>
      </div>
    </div>
  );
}

const styles: Record<string, React.CSSProperties> = {
  container: {
    display: "flex",
    flexDirection: "column",
    height: "100vh",
    background: "#1a1a1a",
    color: "#e0e0e0",
    fontFamily: "-apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif",
    userSelect: "none",
  },
  titleBar: {
    display: "flex",
    justifyContent: "space-between",
    alignItems: "center",
    padding: "10px 16px",
    fontSize: "0.82rem",
    borderBottom: "1px solid #333",
  },
  fileName: {
    fontWeight: 600,
    overflow: "hidden",
    textOverflow: "ellipsis",
    whiteSpace: "nowrap",
  },
  duration: {
    color: "#888",
    fontSize: "0.75rem",
    flexShrink: 0,
    marginLeft: 12,
  },
  waveformArea: {
    flex: 1,
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    padding: "0 16px",
    position: "relative",
  },
  loading: {
    position: "absolute",
    color: "#888",
    fontSize: "0.82rem",
  },
  error: {
    position: "absolute",
    color: "#e55",
    fontSize: "0.82rem",
  },
  controls: {
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
    gap: 12,
    padding: "12px 16px",
    borderTop: "1px solid #333",
  },
  btn: {
    background: "none",
    border: "1px solid #555",
    borderRadius: 4,
    color: "#ccc",
    padding: "4px 10px",
    cursor: "pointer",
    fontFamily: "inherit",
    fontSize: "0.75rem",
  },
  playBtn: {
    background: "rgba(100, 149, 237, 0.2)",
    border: "1px solid rgba(100, 149, 237, 0.5)",
    borderRadius: "50%",
    color: "#fff",
    width: 40,
    height: 40,
    cursor: "pointer",
    fontSize: "1rem",
    display: "flex",
    alignItems: "center",
    justifyContent: "center",
  },
  time: {
    fontSize: "0.75rem",
    color: "#888",
    minWidth: 90,
    textAlign: "center",
  },
};
