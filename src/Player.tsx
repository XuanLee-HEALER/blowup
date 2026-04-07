import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface PlayerState {
  playing: boolean;
  position: number;
  duration: number;
  volume: number;
  paused: boolean;
  title: string;
}

function formatTime(seconds: number): string {
  if (!seconds || !isFinite(seconds)) return "0:00";
  const h = Math.floor(seconds / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const s = Math.floor(seconds % 60);
  if (h > 0) return `${h}:${m.toString().padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export function Player() {
  const [state, setState] = useState<PlayerState>({
    playing: false, position: 0, duration: 0, volume: 100, paused: true, title: "",
  });
  const [showControls, setShowControls] = useState(true);
  const [seeking, setSeeking] = useState(false);
  const [seekPos, setSeekPos] = useState(0);
  const hideTimer = useRef<number | null>(null);

  // Listen to player-state events from Rust
  useEffect(() => {
    const unlisten = listen<PlayerState>("player-state", (event) => {
      if (!seeking) {
        setState(event.payload);
      }
    });
    return () => { unlisten.then((f) => f()); };
  }, [seeking]);

  // Auto-hide controls after 3 seconds of inactivity
  const resetHideTimer = useCallback(() => {
    setShowControls(true);
    if (hideTimer.current) clearTimeout(hideTimer.current);
    hideTimer.current = window.setTimeout(() => {
      if (!seeking) setShowControls(false);
    }, 3000);
  }, [seeking]);

  useEffect(() => {
    const handleMove = () => resetHideTimer();
    window.addEventListener("mousemove", handleMove);
    // Initial hide timer starts via mousemove
    return () => {
      window.removeEventListener("mousemove", handleMove);
      if (hideTimer.current) clearTimeout(hideTimer.current);
    };
  }, [resetHideTimer]);

  const togglePlayPause = () => invoke("cmd_player_play_pause").catch(console.error);
  const seek = (pos: number) => invoke("cmd_player_seek", { position: pos }).catch(console.error);
  const setVolume = (vol: number) => invoke("cmd_player_set_volume", { volume: vol }).catch(console.error);
  const toggleFullscreen = () => invoke("cmd_player_toggle_fullscreen").catch(console.error);
  const closePlayer = () => invoke("cmd_close_player").catch(console.error);

  // Keyboard shortcuts
  useEffect(() => {
    const handleKey = (e: KeyboardEvent) => {
      switch (e.key) {
        case " ": e.preventDefault(); togglePlayPause(); break;
        case "ArrowLeft": invoke("cmd_player_seek_relative", { offset: -5 }); break;
        case "ArrowRight": invoke("cmd_player_seek_relative", { offset: 5 }); break;
        case "ArrowUp": e.preventDefault(); setVolume(Math.min(100, state.volume + 5)); break;
        case "ArrowDown": e.preventDefault(); setVolume(Math.max(0, state.volume - 5)); break;
        case "f": toggleFullscreen(); break;
        case "Escape": closePlayer(); break;
      }
    };
    window.addEventListener("keydown", handleKey);
    return () => window.removeEventListener("keydown", handleKey);
  }, [state.volume]);

  const progress = state.duration > 0 ? ((seeking ? seekPos : state.position) / state.duration) * 100 : 0;

  return (
    <div
      style={{
        position: "fixed", inset: 0,
        background: "rgba(0,0,0,0.01)", // Near-transparent but captures mouse events
        display: "flex", flexDirection: "column",
        cursor: showControls ? "default" : "none",
        userSelect: "none",
      }}
      onMouseMove={resetHideTimer}
    >
      {/* Transparent spacer — lets mpv video show through */}
      <div style={{ flex: 1 }} onClick={togglePlayPause} onDoubleClick={toggleFullscreen} />

      {/* Controls overlay */}
      <div
        style={{
          background: "linear-gradient(transparent, rgba(0,0,0,0.85))",
          padding: "2rem 1.2rem 1rem",
          opacity: showControls ? 1 : 0,
          transition: "opacity 0.3s ease",
          pointerEvents: showControls ? "auto" : "none",
        }}
      >
        {/* Title */}
        <div style={{ fontSize: "0.8rem", color: "rgba(255,255,255,0.7)", marginBottom: "0.5rem", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
          {state.title}
        </div>

        {/* Progress bar */}
        <div
          style={{ height: 4, background: "rgba(255,255,255,0.2)", borderRadius: 2, cursor: "pointer", marginBottom: "0.6rem", position: "relative" }}
          onMouseDown={(e) => {
            const rect = e.currentTarget.getBoundingClientRect();
            const ratio = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
            const pos = ratio * state.duration;
            setSeeking(true);
            setSeekPos(pos);

            const handleMouseMove = (ev: MouseEvent) => {
              const r = Math.max(0, Math.min(1, (ev.clientX - rect.left) / rect.width));
              setSeekPos(r * state.duration);
            };
            const handleMouseUp = (ev: MouseEvent) => {
              const r = Math.max(0, Math.min(1, (ev.clientX - rect.left) / rect.width));
              seek(r * state.duration);
              setSeeking(false);
              window.removeEventListener("mousemove", handleMouseMove);
              window.removeEventListener("mouseup", handleMouseUp);
            };
            window.addEventListener("mousemove", handleMouseMove);
            window.addEventListener("mouseup", handleMouseUp);
          }}
        >
          <div style={{
            height: "100%", borderRadius: 2, background: "#fff",
            width: `${progress}%`, transition: seeking ? "none" : "width 0.2s linear",
          }} />
        </div>

        {/* Controls row */}
        <div style={{ display: "flex", alignItems: "center", gap: "1rem" }}>
          {/* Play/Pause */}
          <button onClick={togglePlayPause} style={btnStyle}>
            {state.paused ? "▶" : "⏸"}
          </button>

          {/* Time */}
          <span style={{ fontSize: "0.75rem", color: "rgba(255,255,255,0.8)", minWidth: 100 }}>
            {formatTime(seeking ? seekPos : state.position)} / {formatTime(state.duration)}
          </span>

          <div style={{ flex: 1 }} />

          {/* Volume */}
          <div style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
            <button onClick={() => setVolume(state.volume > 0 ? 0 : 100)} style={btnStyle}>
              {state.volume === 0 ? "🔇" : "🔊"}
            </button>
            <input
              type="range" min="0" max="100" value={state.volume}
              onChange={(e) => setVolume(Number(e.target.value))}
              style={{ width: 80, accentColor: "#fff" }}
            />
          </div>

          {/* Fullscreen */}
          <button onClick={toggleFullscreen} style={btnStyle}>⛶</button>

          {/* Close */}
          <button onClick={closePlayer} style={btnStyle}>✕</button>
        </div>
      </div>
    </div>
  );
}

const btnStyle: React.CSSProperties = {
  background: "none", border: "none", color: "#fff",
  fontSize: "1.1rem", cursor: "pointer", padding: "0.2rem",
  opacity: 0.9, lineHeight: 1,
};
