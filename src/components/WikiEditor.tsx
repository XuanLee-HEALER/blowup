// src/components/WikiEditor.tsx
import { useState } from "react";
import { marked } from "marked";
import DOMPurify from "dompurify";

// Configure marked to NOT render raw HTML (prevents XSS)
marked.use({ breaks: true });

interface WikiEditorProps {
  value: string;
  onChange: (v: string) => void;
  onSave: () => void;
  minHeight?: number;
}

export function WikiEditor({ value, onChange, onSave, minHeight = 200 }: WikiEditorProps) {
  const [tab, setTab] = useState<"write" | "preview">("write");

  return (
    <div>
      <div style={{ display: "flex", gap: "0.25rem", marginBottom: "0.5rem" }}>
        {(["write", "preview"] as const).map((t) => (
          <button
            key={t}
            onClick={() => setTab(t)}
            style={{
              background: "none", border: "none",
              borderBottom: tab === t ? "1px solid var(--color-accent)" : "1px solid transparent",
              padding: "0.2rem 0.6rem 0.3rem",
              cursor: "pointer", fontSize: "0.75rem",
              color: tab === t ? "var(--color-accent)" : "var(--color-label-tertiary)",
              fontFamily: "inherit",
            }}
          >
            {t === "write" ? "编辑" : "预览"}
          </button>
        ))}
      </div>

      {tab === "write" ? (
        <textarea
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onBlur={onSave}
          placeholder="支持 Markdown 格式…"
          style={{
            width: "100%", minHeight, resize: "vertical", outline: "none",
            background: "var(--color-bg-elevated)",
            border: "1px solid var(--color-separator)",
            borderRadius: 6, padding: "0.75rem",
            color: "var(--color-label-primary)",
            fontSize: "0.8rem", fontFamily: "monospace",
            lineHeight: 1.65, boxSizing: "border-box",
          }}
        />
      ) : (
        <div
          dangerouslySetInnerHTML={{
            __html: DOMPurify.sanitize(marked.parse(value || "_（暂无内容）_") as string),
          }}
          style={{
            minHeight, overflowY: "auto",
            background: "var(--color-bg-elevated)",
            border: "1px solid var(--color-separator)",
            borderRadius: 6, padding: "0.75rem",
            fontSize: "0.8rem", lineHeight: 1.65,
            color: "var(--color-label-secondary)",
          }}
        />
      )}
    </div>
  );
}
