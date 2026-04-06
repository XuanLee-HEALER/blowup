import { useState } from "react";
import { subtitle } from "../lib/tauri";
import type { SubtitleStreamInfo } from "../lib/tauri";
import { open } from "@tauri-apps/plugin-dialog";

const cardStyle: React.CSSProperties = {
  background: "var(--color-bg-control)",
  borderRadius: 10,
  padding: 20,
  marginBottom: 16,
};

const labelStyle: React.CSSProperties = {
  fontSize: 12,
  color: "var(--color-label-secondary)",
  display: "block",
  marginBottom: 4,
};

const inputStyle: React.CSSProperties = {
  width: "100%",
  padding: "6px 10px",
  borderRadius: 6,
  border: "1px solid var(--color-separator)",
  background: "var(--color-bg-primary)",
  color: "var(--color-label-primary)",
  fontSize: 13,
  boxSizing: "border-box" as const,
};

const btnStyle: React.CSSProperties = {
  background: "var(--color-accent)",
  color: "#fff",
  border: "none",
  borderRadius: 6,
  padding: "6px 16px",
  cursor: "pointer",
  fontSize: 13,
};

const btnSecondaryStyle: React.CSSProperties = {
  background: "var(--color-bg-primary)",
  border: "1px solid var(--color-separator)",
  borderRadius: 6,
  padding: "6px 16px",
  color: "var(--color-label-primary)",
  cursor: "pointer",
  fontSize: 13,
};

const fileRowStyle: React.CSSProperties = {
  display: "flex",
  gap: 8,
  alignItems: "center",
  marginBottom: 10,
};

const VIDEO_FILTERS = [
  { name: "Video", extensions: ["mp4", "mkv", "avi", "mov", "ts", "webm", "m4v", "flv", "wmv"] },
];

const SRT_FILTERS = [
  { name: "Subtitle", extensions: ["srt", "ass", "ssa", "sub", "vtt"] },
];

function FilePickerRow({
  label,
  value,
  onChange,
  filters,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  filters: { name: string; extensions: string[] }[];
}) {
  const handlePick = async () => {
    const path = await open({ multiple: false, filters });
    if (path) onChange(path as string);
  };
  const fileName = value ? value.split(/[/\\]/).pop() : "";
  return (
    <div style={fileRowStyle}>
      <label style={{ ...labelStyle, marginBottom: 0, minWidth: 60 }}>{label}</label>
      <div
        style={{
          flex: 1,
          fontSize: 13,
          color: value ? "var(--color-label-primary)" : "var(--color-label-tertiary)",
          overflow: "hidden",
          textOverflow: "ellipsis",
          whiteSpace: "nowrap",
        }}
      >
        {fileName || "未选择"}
      </div>
      <button onClick={handlePick} style={btnSecondaryStyle}>
        选择
      </button>
    </div>
  );
}

function StatusMsg({ status }: { status: { ok: boolean; msg: string } | null }) {
  if (!status) return null;
  return (
    <div style={{ marginTop: 10, fontSize: 13, color: status.ok ? "#4caf50" : "#e53935" }}>
      {status.ok ? "✓ " : "✗ "}{status.msg}
    </div>
  );
}

function FetchSection() {
  const [video, setVideo] = useState("");
  const [lang, setLang] = useState("zh");
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; msg: string } | null>(null);

  const handleFetch = async () => {
    if (!video) return;
    setLoading(true);
    setStatus(null);
    try {
      await subtitle.fetch(video, lang);
      setStatus({ ok: true, msg: "字幕下载成功" });
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={cardStyle}>
      <h3 style={{ margin: "0 0 12px", fontSize: 15 }}>搜索字幕</h3>
      <p style={{ fontSize: 12, color: "var(--color-label-tertiary)", margin: "0 0 12px" }}>
        从 OpenSubtitles 搜索并下载字幕，保存到视频文件同目录
      </p>
      <FilePickerRow label="视频" value={video} onChange={setVideo} filters={VIDEO_FILTERS} />
      <div style={fileRowStyle}>
        <label style={{ ...labelStyle, marginBottom: 0, minWidth: 60 }}>语言</label>
        <select value={lang} onChange={(e) => setLang(e.target.value)} style={{ ...inputStyle, width: "auto", flex: 1 }}>
          <option value="zh">中文</option>
          <option value="en">English</option>
          <option value="ja">日本語</option>
          <option value="ko">한국어</option>
          <option value="fr">Français</option>
          <option value="de">Deutsch</option>
          <option value="es">Español</option>
        </select>
      </div>
      <button onClick={handleFetch} disabled={!video || loading} style={{ ...btnStyle, opacity: !video || loading ? 0.5 : 1 }}>
        {loading ? "搜索中..." : "搜索并下载"}
      </button>
      <StatusMsg status={status} />
    </div>
  );
}

function AlignSection() {
  const [video, setVideo] = useState("");
  const [srt, setSrt] = useState("");
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; msg: string } | null>(null);

  const handleAlign = async () => {
    if (!video || !srt) return;
    setLoading(true);
    setStatus(null);
    try {
      await subtitle.align(video, srt);
      setStatus({ ok: true, msg: "字幕对齐完成" });
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={cardStyle}>
      <h3 style={{ margin: "0 0 12px", fontSize: 15 }}>对齐字幕</h3>
      <p style={{ fontSize: 12, color: "var(--color-label-tertiary)", margin: "0 0 12px" }}>
        使用 alass 自动对齐字幕时间轴（需要安装 alass）
      </p>
      <FilePickerRow label="视频" value={video} onChange={setVideo} filters={VIDEO_FILTERS} />
      <FilePickerRow label="字幕" value={srt} onChange={setSrt} filters={SRT_FILTERS} />
      <button onClick={handleAlign} disabled={!video || !srt || loading} style={{ ...btnStyle, opacity: !video || !srt || loading ? 0.5 : 1 }}>
        {loading ? "对齐中..." : "开始对齐"}
      </button>
      <StatusMsg status={status} />
    </div>
  );
}

function ExtractSection() {
  const [video, setVideo] = useState("");
  const [streams, setStreams] = useState<SubtitleStreamInfo[]>([]);
  const [selectedStream, setSelectedStream] = useState<number | undefined>();
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; msg: string } | null>(null);

  const handleListStreams = async () => {
    if (!video) return;
    setLoading(true);
    setStatus(null);
    setStreams([]);
    setSelectedStream(undefined);
    try {
      const result = await subtitle.listStreams(video);
      setStreams(result);
      if (result.length === 0) {
        setStatus({ ok: false, msg: "未找到字幕轨道" });
      } else {
        setSelectedStream(result[0].index);
      }
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  const handleExtract = async () => {
    if (!video) return;
    setLoading(true);
    setStatus(null);
    try {
      await subtitle.extract(video, selectedStream);
      setStatus({ ok: true, msg: "字幕提取成功" });
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={cardStyle}>
      <h3 style={{ margin: "0 0 12px", fontSize: 15 }}>提取字幕</h3>
      <p style={{ fontSize: 12, color: "var(--color-label-tertiary)", margin: "0 0 12px" }}>
        从视频文件中提取内嵌字幕轨道为 SRT 文件（需要 ffmpeg）
      </p>
      <FilePickerRow label="视频" value={video} onChange={(v) => { setVideo(v); setStreams([]); setSelectedStream(undefined); setStatus(null); }} filters={VIDEO_FILTERS} />
      <div style={{ display: "flex", gap: 8, marginBottom: 10 }}>
        <button onClick={handleListStreams} disabled={!video || loading} style={{ ...btnSecondaryStyle, opacity: !video || loading ? 0.5 : 1 }}>
          列出字幕轨
        </button>
      </div>
      {streams.length > 0 && (
        <div style={{ marginBottom: 10 }}>
          <label style={labelStyle}>选择轨道</label>
          <select value={selectedStream ?? ""} onChange={(e) => setSelectedStream(Number(e.target.value))} style={{ ...inputStyle, width: "auto" }}>
            {streams.map((s) => (
              <option key={s.index} value={s.index}>
                #{s.index} — {s.codec_name} {s.language ? `(${s.language})` : ""} {s.title ?? ""}
              </option>
            ))}
          </select>
        </div>
      )}
      {streams.length > 0 && (
        <button onClick={handleExtract} disabled={loading} style={{ ...btnStyle, opacity: loading ? 0.5 : 1 }}>
          {loading ? "提取中..." : "提取"}
        </button>
      )}
      <StatusMsg status={status} />
    </div>
  );
}

function ShiftSection() {
  const [srt, setSrt] = useState("");
  const [offsetMs, setOffsetMs] = useState(0);
  const [loading, setLoading] = useState(false);
  const [status, setStatus] = useState<{ ok: boolean; msg: string } | null>(null);

  const handleShift = async () => {
    if (!srt || offsetMs === 0) return;
    setLoading(true);
    setStatus(null);
    try {
      await subtitle.shift(srt, offsetMs);
      setStatus({ ok: true, msg: `偏移 ${offsetMs > 0 ? "+" : ""}${offsetMs}ms 完成` });
    } catch (e) {
      setStatus({ ok: false, msg: String(e) });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div style={cardStyle}>
      <h3 style={{ margin: "0 0 12px", fontSize: 15 }}>时间偏移</h3>
      <p style={{ fontSize: 12, color: "var(--color-label-tertiary)", margin: "0 0 12px" }}>
        手动调整 SRT 字幕时间轴（正数延后，负数提前）
      </p>
      <FilePickerRow label="字幕" value={srt} onChange={setSrt} filters={SRT_FILTERS} />
      <div style={fileRowStyle}>
        <label style={{ ...labelStyle, marginBottom: 0, minWidth: 60 }}>偏移量</label>
        <input type="number" value={offsetMs} onChange={(e) => setOffsetMs(Number(e.target.value))} style={{ ...inputStyle, width: 120, flex: "none" }} />
        <span style={{ fontSize: 12, color: "var(--color-label-secondary)" }}>毫秒</span>
      </div>
      <div style={{ display: "flex", gap: 8, marginBottom: 4 }}>
        {[-5000, -1000, -500, 500, 1000, 5000].map((v) => (
          <button key={v} onClick={() => setOffsetMs((prev) => prev + v)} style={{ ...btnSecondaryStyle, padding: "2px 8px", fontSize: 11 }}>
            {v > 0 ? `+${v}` : v}
          </button>
        ))}
      </div>
      <button onClick={handleShift} disabled={!srt || offsetMs === 0 || loading} style={{ ...btnStyle, marginTop: 8, opacity: !srt || offsetMs === 0 || loading ? 0.5 : 1 }}>
        {loading ? "处理中..." : "应用偏移"}
      </button>
      <StatusMsg status={status} />
    </div>
  );
}

export default function Subtitle() {
  return (
    <div style={{ height: "100%", overflowY: "auto", padding: 24 }}>
      <h2 style={{ margin: "0 0 20px", fontSize: 18 }}>字幕工具</h2>
      <div style={{ maxWidth: 600 }}>
        <FetchSection />
        <AlignSection />
        <ExtractSection />
        <ShiftSection />
      </div>
    </div>
  );
}
