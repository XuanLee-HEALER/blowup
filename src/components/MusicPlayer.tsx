// src/components/MusicPlayer.tsx
import { useEffect, useRef, useState, useCallback } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { ActionIcon, Box, Group, Paper, Progress, Text } from "@mantine/core";
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
    return () => {
      audioRef.current?.pause();
      audioRef.current = null;
    };
  }, []);

  useEffect(() => {
    if (!audioRef.current) return;
    if (!active || !enabled || playlist.length === 0) {
      audioRef.current.pause();
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
    const onTimeUpdate = () => {
      setProgress(audio.currentTime);
      setDuration(audio.duration || 0);
    };
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
      audio
        .play()
        .then(() => setIsPlaying(true))
        .catch(() => {});
    }
  }, [currentIndex, active, enabled, playlist, getTrackSrc]);

  if (!enabled || playlist.length === 0 || !active) return null;

  const currentTrack = playlist[currentIndex];
  const fmt = (s: number) =>
    `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;
  const next = () =>
    setCurrentIndex(
      mode === "random"
        ? Math.floor(Math.random() * playlist.length)
        : (currentIndex + 1) % playlist.length
    );
  const togglePlay = () => {
    const a = audioRef.current;
    if (!a) return;
    if (isPlaying) {
      a.pause();
    } else {
      a.play().catch(() => {});
    }
  };

  const percent = duration > 0 ? (progress / duration) * 100 : 0;

  return (
    <Paper
      withBorder
      shadow="lg"
      bg="var(--color-bg-elevated)"
      px="0.85rem"
      py="0.6rem"
      w={240}
      style={{
        position: "fixed",
        bottom: "1.25rem",
        right: "1.25rem",
        zIndex: 50,
        borderColor: "var(--color-separator)",
      }}
    >
      <Group gap="0.5rem" mb="0.4rem" wrap="nowrap" align="center">
        <Text fz="0.78rem" c="var(--color-accent)">
          ♪
        </Text>
        <Text fz="0.75rem" c="var(--color-label-secondary)" truncate style={{ flex: 1 }}>
          {currentTrack?.name ?? "未知曲目"}
        </Text>
        <ActionIcon variant="subtle" color="gray" size="sm" onClick={togglePlay}>
          {isPlaying ? "⏸" : "▶"}
        </ActionIcon>
        <ActionIcon variant="subtle" color="gray" size="sm" onClick={next}>
          ⏭
        </ActionIcon>
        {mode === "random" && (
          <Text fz="0.65rem" c="var(--color-label-quaternary)">
            🔀
          </Text>
        )}
      </Group>
      <Group gap="0.4rem" align="center">
        <Box
          style={{ flex: 1, cursor: "pointer" }}
          onClick={(e) => {
            const a = audioRef.current;
            if (!a || !duration) return;
            const rect = e.currentTarget.getBoundingClientRect();
            a.currentTime = ((e.clientX - rect.left) / rect.width) * duration;
          }}
        >
          <Progress
            value={percent}
            size="xs"
            transitionDuration={500}
            styles={{ section: { backgroundColor: "var(--color-accent)" } }}
          />
        </Box>
        <Text fz="0.62rem" c="var(--color-label-quaternary)" style={{ whiteSpace: "nowrap" }}>
          {fmt(progress)} / {fmt(duration)}
        </Text>
      </Group>
    </Paper>
  );
}
