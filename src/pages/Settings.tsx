// src/pages/Settings.tsx
import { useState, useEffect } from "react";
import type { ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { TextInput } from "../components/ui/TextInput";
import { Button } from "../components/ui/Button";
import { config, type AppConfig } from "../lib/tauri";

const LANG_OPTIONS = [
  { value: "zh", label: "中文 (zh)" },
  { value: "en", label: "English (en)" },
  { value: "ja", label: "日本語 (ja)" },
];

export default function Settings() {
  const [cfg, setCfg]         = useState<AppConfig | null>(null);
  const [showKey, setShowKey] = useState(false);
  const [saving, setSaving]   = useState<string | null>(null);

  useEffect(() => {
    config.get().then(setCfg);
  }, []);

  const save = async (key: string, value: string) => {
    setSaving(key);
    try {
      await config.set(key, value);
      setCfg((prev) => {
        if (!prev) return prev;
        const [section, field] = key.split(".");
        return { ...prev, [section]: { ...(prev as AppConfig)[section as keyof AppConfig], [field]: value } };
      });
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(null);
    }
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") save("library.root_dir", dir);
  };

  if (!cfg) {
    return (
      <div style={{ padding: "2rem", color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>
        加载中…
      </div>
    );
  }

  return (
    <div style={{ flex: 1, overflowY: "auto", padding: "1.4rem 1.75rem 3rem" }}>
      <h1
        style={{
          fontSize: "1.6rem",
          fontWeight: 700,
          letterSpacing: "-0.035em",
          marginBottom: "2rem",
        }}
      >
        设置
      </h1>

      <Section title="TMDB">
        <Field label="API Key">
          <div style={{ display: "flex", gap: "0.5rem" }}>
            <TextInput
              type={showKey ? "text" : "password"}
              defaultValue={cfg.tmdb.api_key}
              placeholder="在 themoviedb.org 免费申请"
              style={{ flex: 1 }}
              onBlur={(e) => save("tmdb.api_key", e.currentTarget.value)}
            />
            <Button onClick={() => setShowKey((v) => !v)}>
              {showKey ? "隐藏" : "显示"}
            </Button>
          </div>
        </Field>
      </Section>

      <Section title="字幕">
        <Field label="默认语言">
          <select
            value={cfg.subtitle.default_lang}
            onChange={(e) => save("subtitle.default_lang", e.target.value)}
            style={{
              background: "var(--color-bg-control)",
              border: "1px solid var(--color-separator)",
              borderRadius: 8,
              padding: "0 0.75rem",
              height: 34,
              color: "var(--color-label-primary)",
              fontSize: "0.85rem",
              fontFamily: "inherit",
            }}
          >
            {LANG_OPTIONS.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        </Field>
      </Section>

      <Section title="工具路径">
        {(["aria2c", "alass", "ffmpeg", "player"] as const).map((tool) => (
          <Field key={tool} label={tool}>
            <TextInput
              defaultValue={cfg.tools[tool]}
              placeholder={tool}
              onBlur={(e) => save(`tools.${tool}`, e.currentTarget.value)}
            />
          </Field>
        ))}
      </Section>

      <Section title="库目录">
        <Field label="本地库根目录">
          <div style={{ display: "flex", gap: "0.5rem" }}>
            <TextInput
              value={cfg.library.root_dir}
              readOnly
              style={{ flex: 1 }}
              onChange={() => {}}
            />
            <Button onClick={pickDir}>选择…</Button>
          </div>
        </Field>
      </Section>

      <Section title="背景音乐">
        <Field label="启用背景音乐">
          <div style={{ display: "flex", flexDirection: "column", gap: "0.2rem" }}>
            <input
              type="checkbox"
              checked={!!cfg.music?.enabled}
              onChange={async (e) => {
                await config.set("music.enabled", e.target.checked ? "true" : "false");
                setCfg((prev) => prev ? { ...prev, music: { ...prev.music, enabled: e.target.checked } } : prev);
              }}
              style={{ accentColor: "var(--color-accent)", cursor: "pointer" }}
            />
            <span style={{ fontSize: "0.72rem", color: "var(--color-label-tertiary)" }}>在影人/流派/关系图页面播放</span>
          </div>
        </Field>
        <Field label="播放模式">
          <select
            value={cfg.music?.mode ?? "sequential"}
            onChange={async (e) => {
              await config.set("music.mode", e.target.value);
              const mode = e.target.value === "random" ? "random" as const : "sequential" as const;
              setCfg((prev) => prev ? { ...prev, music: { ...prev.music, mode } } : prev);
            }}
            style={{
              background: "var(--color-bg-control)",
              border: "1px solid var(--color-separator)",
              borderRadius: 8,
              padding: "0 0.75rem",
              height: 34,
              color: "var(--color-label-primary)",
              fontSize: "0.85rem",
              fontFamily: "inherit",
            }}
          >
            <option value="sequential">顺序播放</option>
            <option value="random">随机播放</option>
          </select>
        </Field>
        <Field label="播放列表">
          <div>
            {(cfg.music?.playlist ?? []).map((track, i) => (
              <div key={i} style={{ display: "flex", gap: "0.5rem", marginBottom: "0.4rem", alignItems: "center" }}>
                <TextInput
                  placeholder="曲目名称"
                  defaultValue={track.name}
                  style={{ width: 120, flexShrink: 0 }}
                  onBlur={async (e) => {
                    const newPlaylist = (cfg.music?.playlist ?? []).map((t, idx) =>
                      idx === i ? { ...t, name: e.currentTarget.value } : t
                    );
                    setCfg((prev) => prev ? { ...prev, music: { ...prev.music, playlist: newPlaylist } } : prev);
                    await config.setMusicPlaylist(newPlaylist);
                  }}
                />
                <TextInput
                  placeholder="文件路径或 URL"
                  defaultValue={track.src}
                  style={{ flex: 1 }}
                  onBlur={async (e) => {
                    const newPlaylist = (cfg.music?.playlist ?? []).map((t, idx) =>
                      idx === i ? { ...t, src: e.currentTarget.value } : t
                    );
                    setCfg((prev) => prev ? { ...prev, music: { ...prev.music, playlist: newPlaylist } } : prev);
                    await config.setMusicPlaylist(newPlaylist);
                  }}
                />
                <button
                  onClick={async () => {
                    const newPlaylist = (cfg.music?.playlist ?? []).filter((_, idx) => idx !== i);
                    setCfg((prev) => prev ? { ...prev, music: { ...prev.music, playlist: newPlaylist } } : prev);
                    await config.setMusicPlaylist(newPlaylist);
                  }}
                  style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.8rem" }}
                >
                  ✕
                </button>
              </div>
            ))}
            <button
              onClick={() => {
                setCfg((prev) => prev ? { ...prev, music: { ...prev.music, playlist: [...(prev.music?.playlist ?? []), { name: "", src: "" }] } } : prev);
              }}
              style={{ background: "none", border: "1px dashed var(--color-separator)", borderRadius: 5, padding: "0.3rem 0.75rem", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit", marginTop: "0.25rem" }}
            >
              + 添加曲目
            </button>
          </div>
        </Field>
      </Section>

      {saving && (
        <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.72rem", marginTop: "1rem" }}>
          保存中…
        </p>
      )}
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div style={{ marginBottom: "2rem" }}>
      <p
        style={{
          margin: "0 0 0.75rem",
          fontSize: "0.7rem",
          color: "var(--color-label-quaternary)",
          letterSpacing: "0.08em",
          textTransform: "uppercase",
        }}
      >
        {title}
      </p>
      <div
        style={{
          background: "var(--color-bg-secondary)",
          border: "1px solid var(--color-separator)",
          borderRadius: 10,
          padding: "0.1rem 1rem",
        }}
      >
        {children}
      </div>
    </div>
  );
}

function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div
      style={{
        display: "flex",
        alignItems: "center",
        gap: "1rem",
        padding: "0.65rem 0",
        borderBottom: "1px solid var(--color-separator)",
      }}
    >
      <span
        style={{
          width: 120,
          flexShrink: 0,
          fontSize: "0.82rem",
          color: "var(--color-label-secondary)",
        }}
      >
        {label}
      </span>
      <div style={{ flex: 1 }}>{children}</div>
    </div>
  );
}
