// src/components/MusicPlayer.tsx
import { useEffect, useRef, useState, useCallback } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { MusicTrack } from "../lib/tauri";

interface MusicPlayerProps {
  enabled: boolean;
  mode: "sequential" | "random";
  playlist: MusicTrack[];
  active: boolean;
}

export function MusicPlayer({ enabled, mode, playlist, active }: MusicPlayerProps) {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [currentIndex, setCurrentIndex] = useState(0);
  const [isPlaying, setIsPlaying] = useState(false);
  const [progress, setProgress] = useState(0);
  const [duration, setDuration] = useState(0);

  const getTrackSrc = useCallback((track: MusicTrack): string => {
    if (track.src.startsWith("http://") || track.src.startsWith("https://")) return track.src;
    return convertFileSrc(track.src);
  }, []);

  useEffect(() => {
    audioRef.current = new Audio();
    return () => { audioRef.current?.pause(); audioRef.current = null; };
  }, []);

  useEffect(() => {
    if (!audioRef.current) return;
    if (!active || !enabled || playlist.length === 0) {
      audioRef.current.pause();
      // Defer state update to avoid synchronous setState in effect body
      queueMicrotask(() => setIsPlaying(false));
    }
  }, [active, enabled, playlist.length]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;

    const onEnded = () => {
      if (playlist.length === 0) return;
      if (mode === "random") {
        setCurrentIndex(Math.floor(Math.random() * playlist.length));
      } else {
        setCurrentIndex((i) => (i + 1) % playlist.length);
      }
    };
    const onTimeUpdate = () => { setProgress(audio.currentTime); setDuration(audio.duration || 0); };
    const onPlay = () => setIsPlaying(true);
    const onPause = () => setIsPlaying(false);

    audio.addEventListener("ended", onEnded);
    audio.addEventListener("timeupdate", onTimeUpdate);
    audio.addEventListener("play", onPlay);
    audio.addEventListener("pause", onPause);
    return () => {
      audio.removeEventListener("ended", onEnded);
      audio.removeEventListener("timeupdate", onTimeUpdate);
      audio.removeEventListener("play", onPlay);
      audio.removeEventListener("pause", onPause);
    };
  }, [mode, playlist.length]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio || !active || !enabled || playlist.length === 0) return;
    const track = playlist[currentIndex];
    if (!track) return;
    const wasSrc = audio.src;
    const newSrc = getTrackSrc(track);
    if (wasSrc !== newSrc) {
      audio.src = newSrc;
      audio.play().then(() => setIsPlaying(true)).catch(() => {});
    }
  }, [currentIndex, active, enabled, playlist, getTrackSrc]);

  if (!enabled || playlist.length === 0 || !active) return null;

  const currentTrack = playlist[currentIndex];
  const fmt = (s: number) => `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
  const next = () => setCurrentIndex(mode === "random" ? Math.floor(Math.random() * playlist.length) : (currentIndex + 1) % playlist.length);
  const togglePlay = () => {
    const a = audioRef.current;
    if (!a) return;
    if (isPlaying) { a.pause(); } else { a.play().catch(() => {}); }
  };

  const btnStyle: React.CSSProperties = { background: "none", border: "none", color: "var(--color-label-secondary)", cursor: "pointer", fontSize: "0.8rem", padding: "0.1rem 0.2rem", lineHeight: 1 };

  return (
    <div style={{ position: "fixed", bottom: "1.25rem", right: "1.25rem", zIndex: 50, background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 10, padding: "0.6rem 0.85rem", width: 240, boxShadow: "0 4px 20px rgba(0,0,0,0.4)" }}>
      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem", marginBottom: "0.4rem" }}>
        <span style={{ fontSize: "0.78rem", color: "var(--color-accent)" }}>♪</span>
        <span style={{ flex: 1, fontSize: "0.75rem", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis", color: "var(--color-label-secondary)" }}>
          {currentTrack?.name ?? "未知曲目"}
        </span>
        <button onClick={togglePlay} style={btnStyle}>{isPlaying ? "⏸" : "▶"}</button>
        <button onClick={next} style={btnStyle}>⏭</button>
        {mode === "random" && <span style={{ fontSize: "0.65rem", color: "var(--color-label-quaternary)" }}>🔀</span>}
      </div>
      <div style={{ display: "flex", alignItems: "center", gap: "0.4rem" }}>
        <div style={{ flex: 1, height: 3, background: "var(--color-bg-secondary)", borderRadius: 2, overflow: "hidden", cursor: "pointer" }}
          onClick={(e) => {
            const a = audioRef.current;
            if (!a || !duration) return;
            const rect = e.currentTarget.getBoundingClientRect();
            a.currentTime = ((e.clientX - rect.left) / rect.width) * duration;
          }}>
          <div style={{ height: "100%", width: duration > 0 ? `${(progress / duration) * 100}%` : "0%", background: "var(--color-accent)", borderRadius: 2, transition: "width 0.5s linear" }} />
        </div>
        <span style={{ fontSize: "0.62rem", color: "var(--color-label-quaternary)", whiteSpace: "nowrap" }}>
          {fmt(progress)} / {fmt(duration)}
        </span>
      </div>
    </div>
  );
}
