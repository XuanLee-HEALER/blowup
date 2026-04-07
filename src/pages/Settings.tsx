import { useState, useEffect } from "react";
import type { ReactNode } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { TextInput } from "../components/ui/TextInput";
import { Button } from "../components/ui/Button";
import { config, dataIO, type AppConfig, type MusicTrack } from "../lib/tauri";

const LANG_OPTIONS = [
  { value: "zh", label: "中文 (zh)" },
  { value: "en", label: "English (en)" },
  { value: "ja", label: "日本語 (ja)" },
];

export default function Settings() {
  const [cfg, setCfg] = useState<AppConfig | null>(null);
  const [showKey, setShowKey] = useState(false);
  const [cachePath, setCachePath] = useState("");

  useEffect(() => {
    config.get().then(setCfg);
    config.getCachePath().then(setCachePath);
  }, []);

  const update = (mutate: (draft: AppConfig) => void) => {
    setCfg((prev) => {
      if (!prev) return null;
      const next: AppConfig = JSON.parse(JSON.stringify(prev));
      mutate(next);
      config.save(next).catch(console.error);
      return next;
    });
  };

  const pickDir = async () => {
    const dir = await open({ directory: true, multiple: false });
    if (typeof dir === "string") {
      update((c) => { c.library.root_dir = dir; });
    }
  };

  if (!cfg) {
    return (
      <div style={{ padding: "2rem", color: "var(--color-label-tertiary)", fontSize: "0.82rem" }}>
        加载中...
      </div>
    );
  }

  const updatePlaylist = (newPlaylist: MusicTrack[]) => {
    update((c) => { c.music.playlist = newPlaylist; });
  };

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
              onBlur={(e) => { const v = e.currentTarget.value; update((c) => { c.tmdb.api_key = v; }); }}
            />
            <Button onClick={() => setShowKey((v) => !v)}>
              {showKey ? "隐藏" : "显示"}
            </Button>
          </div>
        </Field>
      </Section>

      <Section title="OpenSubtitles">
        <Field label="API Key">
          <TextInput
            type="password"
            defaultValue={cfg.opensubtitles.api_key}
            placeholder="可选"
            onBlur={(e) => { const v = e.currentTarget.value; update((c) => { c.opensubtitles.api_key = v; }); }}
          />
        </Field>
      </Section>

      <Section title="字幕">
        <Field label="默认语言">
          <select
            value={cfg.subtitle.default_lang}
            onChange={(e) => update((c) => { c.subtitle.default_lang = e.target.value; })}
            style={{
              background: "var(--color-bg-control)",
              border: "1px solid var(--color-separator)",
              borderRadius: 8,
              padding: "0 0.75rem",
              height: 34,
              color: "var(--color-label-primary)",
              fontSize: "0.85rem",
              fontFamily: "inherit",
              colorScheme: "dark",
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
        {(["alass", "ffmpeg", "player"] as const).map((tool) => (
          <Field key={tool} label={tool}>
            <TextInput
              defaultValue={cfg.tools[tool]}
              placeholder={tool}
              onBlur={(e) => { const v = e.currentTarget.value; update((c) => { c.tools[tool] = v; }); }}
            />
          </Field>
        ))}
      </Section>

      <Section title="下载">
        <Field label="最大并发数">
          <TextInput
            type="number"
            defaultValue={String(cfg.download?.max_concurrent ?? 3)}
            onBlur={(e) => {
              const v = parseInt(e.currentTarget.value, 10);
              if (!isNaN(v) && v >= 1 && v <= 10) { const n = v; update((c) => { c.download.max_concurrent = n; }); }
            }}
            style={{ width: 80 }}
          />
        </Field>
        <Field label="启用 DHT">
          <input
            type="checkbox"
            checked={cfg.download?.enable_dht ?? true}
            onChange={(e) => update((c) => { c.download.enable_dht = e.target.checked; })}
            style={{ accentColor: "var(--color-accent)", cursor: "pointer" }}
          />
        </Field>
        <Field label="会话持久化">
          <input
            type="checkbox"
            checked={cfg.download?.persist_session ?? false}
            onChange={(e) => update((c) => { c.download.persist_session = e.target.checked; })}
            style={{ accentColor: "var(--color-accent)", cursor: "pointer" }}
          />
        </Field>
      </Section>

      <Section title="库目录">
        <Field label="本地库根目录">
          <div style={{ display: "flex", gap: "0.5rem" }}>
            <TextInput value={cfg.library.root_dir} readOnly style={{ flex: 1 }} onChange={() => {}} />
            <Button onClick={pickDir}>选择...</Button>
          </div>
        </Field>
      </Section>

      <Section title="搜索">
        <Field label="请求间隔（秒）">
          <TextInput
            type="number"
            defaultValue={String(cfg.search.rate_limit_secs)}
            onBlur={(e) => {
              const v = parseInt(e.currentTarget.value, 10);
              if (!isNaN(v) && v >= 0) { const n = v; update((c) => { c.search.rate_limit_secs = n; }); }
            }}
            style={{ width: 80 }}
          />
        </Field>
      </Section>

      <Section title="缓存">
        <Field label="缓存文件路径">
          <TextInput
            value={cachePath}
            readOnly
            style={{ flex: 1, color: "var(--color-label-tertiary)" }}
            onChange={() => {}}
          />
        </Field>
        <Field label="最大缓存条目">
          <TextInput
            type="number"
            defaultValue={String(cfg.cache?.max_entries ?? 200)}
            onBlur={(e) => {
              const v = parseInt(e.currentTarget.value, 10);
              if (!isNaN(v) && v > 0) { const n = v; update((c) => { c.cache.max_entries = n; }); }
            }}
            style={{ width: 100 }}
          />
        </Field>
      </Section>

      <Section title="背景音乐">
        <Field label="启用">
          <input
            type="checkbox"
            checked={!!cfg.music?.enabled}
            onChange={(e) => update((c) => { c.music.enabled = e.target.checked; })}
            style={{ accentColor: "var(--color-accent)", cursor: "pointer" }}
          />
        </Field>
        <Field label="播放模式">
          <select
            value={cfg.music?.mode ?? "sequential"}
            onChange={(e) => {
              const mode = e.target.value === "random" ? "random" as const : "sequential" as const;
              update((c) => { c.music.mode = mode; });
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
                  onBlur={(e) => {
                    const v = e.currentTarget.value;
                    const pl = [...(cfg.music?.playlist ?? [])];
                    pl[i] = { ...pl[i], name: v };
                    updatePlaylist(pl);
                  }}
                />
                <TextInput
                  placeholder="文件路径或 URL"
                  defaultValue={track.src}
                  style={{ flex: 1 }}
                  onBlur={(e) => {
                    const v = e.currentTarget.value;
                    const pl = [...(cfg.music?.playlist ?? [])];
                    pl[i] = { ...pl[i], src: v };
                    updatePlaylist(pl);
                  }}
                />
                <button
                  onClick={() => {
                    const pl = (cfg.music?.playlist ?? []).filter((_, idx) => idx !== i);
                    updatePlaylist(pl);
                  }}
                  style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.8rem" }}
                >
                  ✕
                </button>
              </div>
            ))}
            <button
              onClick={() => {
                const pl = [...(cfg.music?.playlist ?? []), { name: "", src: "" }];
                updatePlaylist(pl);
              }}
              style={{ background: "none", border: "1px dashed var(--color-separator)", borderRadius: 5, padding: "0.3rem 0.75rem", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit", marginTop: "0.25rem" }}
            >
              + 添加曲目
            </button>
          </div>
        </Field>
      </Section>

      <Section title="云同步">
        <Field label="Endpoint">
          <TextInput
            defaultValue={cfg.sync?.endpoint ?? ""}
            placeholder="http://192.168.1.x:3900"
            onBlur={(e) => { const v = e.currentTarget.value; update((c) => { c.sync.endpoint = v; }); }}
          />
        </Field>
        <Field label="Bucket">
          <TextInput
            defaultValue={cfg.sync?.bucket ?? ""}
            placeholder="blowup"
            onBlur={(e) => { const v = e.currentTarget.value; update((c) => { c.sync.bucket = v; }); }}
          />
        </Field>
        <Field label="Access Key">
          <TextInput
            type="password"
            defaultValue={cfg.sync?.access_key ?? ""}
            onBlur={(e) => { const v = e.currentTarget.value; update((c) => { c.sync.access_key = v; }); }}
          />
        </Field>
        <Field label="Secret Key">
          <TextInput
            type="password"
            defaultValue={cfg.sync?.secret_key ?? ""}
            onBlur={(e) => { const v = e.currentTarget.value; update((c) => { c.sync.secret_key = v; }); }}
          />
        </Field>
        <Field label="连接测试">
          <Button onClick={async () => {
            try {
              const msg = await dataIO.testS3Connection();
              alert(msg);
            } catch (e) { alert("连接失败: " + e); }
          }}>测试连接</Button>
        </Field>
      </Section>

      <Section title="数据管理">
        <DataIORow
          label="知识库"
          onLocal={async (dir) => {
            if (dir === "export") {
              const path = await save({ defaultPath: "blowup-knowledge-base.json", filters: [{ name: "JSON", extensions: ["json"] }] });
              if (path) { await dataIO.exportKnowledgeBase(path); alert("知识库导出成功"); }
            } else {
              const path = await open({ filters: [{ name: "JSON", extensions: ["json"] }] });
              if (path) {
                const msg = await dataIO.importKnowledgeBase(path as string);
                alert(msg);
                config.get().then(setCfg);
              }
            }
          }}
          onCloud={async (dir) => {
            if (dir === "export") {
              await dataIO.exportKnowledgeBaseS3();
              alert("知识库已导出到云端");
            } else {
              const msg = await dataIO.importKnowledgeBaseS3();
              alert(msg);
              config.get().then(setCfg);
            }
          }}
        />
        <DataIORow
          label="配置文件"
          onLocal={async (dir) => {
            if (dir === "export") {
              const path = await save({ defaultPath: "blowup-config.toml", filters: [{ name: "TOML", extensions: ["toml"] }] });
              if (path) { await config.exportConfig(path); alert("配置导出成功"); }
            } else {
              const path = await open({ filters: [{ name: "TOML", extensions: ["toml"] }] });
              if (path) {
                await config.importConfig(path as string);
                config.get().then(setCfg);
                alert("配置导入成功，部分设置需重启生效");
              }
            }
          }}
          onCloud={async (dir) => {
            if (dir === "export") {
              await dataIO.exportConfigS3();
              alert("配置已导出到云端");
            } else {
              await dataIO.importConfigS3();
              config.get().then(setCfg);
              alert("云端配置导入成功，部分设置需重启生效");
            }
          }}
        />
      </Section>

    </div>
  );
}

function DataIORow({ label, onLocal, onCloud }: {
  label: string;
  onLocal: (dir: "export" | "import") => Promise<void>;
  onCloud: (dir: "export" | "import") => Promise<void>;
}) {
  const [pending, setPending] = useState<"export" | "import" | null>(null);

  const execute = async (target: "local" | "cloud") => {
    const dir = pending!;
    setPending(null);
    try {
      if (target === "local") await onLocal(dir);
      else await onCloud(dir);
    } catch (e) {
      alert(`${target === "cloud" ? "云端" : ""}${dir === "export" ? "导出" : "导入"}失败: ${e}`);
    }
  };

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
      <span style={{ width: 120, flexShrink: 0, fontSize: "0.82rem", color: "var(--color-label-secondary)" }}>
        {label}
      </span>
      <div style={{ flex: 1 }}>
        {pending === null ? (
          <div style={{ display: "flex", gap: "0.5rem" }}>
            <Button onClick={() => setPending("export")}>导出</Button>
            <Button onClick={() => setPending("import")}>导入</Button>
          </div>
        ) : (
          <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
            <span style={{ fontSize: "0.78rem", color: "var(--color-label-tertiary)" }}>
              {pending === "export" ? "导出到" : "导入自"}:
            </span>
            <Button onClick={() => execute("local")}>本地文件</Button>
            <Button onClick={() => execute("cloud")}>云端</Button>
            <button
              onClick={() => setPending(null)}
              style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.8rem" }}
            >
              取消
            </button>
          </div>
        )}
      </div>
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
