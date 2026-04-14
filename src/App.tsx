// src/App.tsx
import { useEffect, useState, useCallback } from "react";
import { Routes, Route, Navigate } from "react-router-dom";
import { AppLayout } from "./layout/AppLayout";
import { LibrarySpace } from "./spaces/LibrarySpace";
import { DiscoverSpace } from "./spaces/DiscoverSpace";
import { KnowledgeSpace } from "./spaces/KnowledgeSpace";
import { SettingsOverlay } from "./spaces/SettingsOverlay";
import { MusicPlayer } from "./components/MusicPlayer";
import { config, type MusicTrack } from "./lib/tauri";
import { useBackendEvent, BackendEvent } from "./lib/useBackendEvent";

// Music-player state lives at the App root so the floating widget
// stays mounted across space switches.
export default function App() {
  const [musicEnabled, setMusicEnabled] = useState(false);
  const [musicMode, setMusicMode] = useState<"sequential" | "random">("sequential");
  const [musicPlaylist, setMusicPlaylist] = useState<MusicTrack[]>([]);

  const loadMusicConfig = useCallback(() => {
    config
      .get()
      .then((cfg) => {
        if (cfg.music) {
          setMusicEnabled(!!cfg.music.enabled);
          setMusicMode(cfg.music.mode === "random" ? "random" : "sequential");
          setMusicPlaylist(cfg.music.playlist ?? []);
        }
      })
      .catch((e) => console.error("[blowup] config.get failed:", e));
  }, []);

  useEffect(() => {
    console.log("[blowup] App mounted", performance.now().toFixed(0) + "ms");
    loadMusicConfig();
  }, [loadMusicConfig]);

  useBackendEvent(BackendEvent.CONFIG_CHANGED, loadMusicConfig);

  return (
    <>
      <Routes>
        <Route element={<AppLayout />}>
          <Route index element={<Navigate to="/library" replace />} />
          <Route path="library" element={<LibrarySpace />} />
          <Route path="library/:movieId" element={<LibrarySpace />} />
          <Route path="discover" element={<DiscoverSpace />} />
          <Route path="discover/downloads" element={<DiscoverSpace />} />
          <Route path="knowledge" element={<KnowledgeSpace />} />
          <Route path="knowledge/graph" element={<KnowledgeSpace />} />
          <Route path="knowledge/edit/:entryId" element={<KnowledgeSpace />} />
          <Route path="settings" element={<SettingsOverlay />} />
          <Route path="*" element={<Navigate to="/library" replace />} />
        </Route>
      </Routes>

      <MusicPlayer active enabled={musicEnabled} mode={musicMode} playlist={musicPlaylist} />
    </>
  );
}
