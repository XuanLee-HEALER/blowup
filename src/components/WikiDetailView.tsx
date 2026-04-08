import { useState, useRef, useMemo } from "react";
import type { ReactNode } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Components } from "react-markdown";

// ── Markdown custom components ───────────────────────────────────

const mdComponents: Components = {
  h1: ({ children, ...props }) => {
    const id = "heading-" + String(children).replace(/\s+/g, "-");
    return <h1 data-heading-id={id} style={{ fontSize: "1.3rem", fontWeight: 700, margin: "2rem 0 0.75rem", color: "var(--color-label-primary)", letterSpacing: "-0.02em" }} {...props}>{children}</h1>;
  },
  h2: ({ children, ...props }) => {
    const id = "heading-" + String(children).replace(/\s+/g, "-");
    return <h2 data-heading-id={id} style={{ fontSize: "1.1rem", fontWeight: 600, margin: "2rem 0 0.6rem", paddingBottom: "0.35rem", borderBottom: "1px solid var(--color-separator)", color: "var(--color-label-primary)" }} {...props}>{children}</h2>;
  },
  h3: ({ children, ...props }) => {
    const id = "heading-" + String(children).replace(/\s+/g, "-");
    return <h3 data-heading-id={id} style={{ fontSize: "0.95rem", fontWeight: 600, margin: "1.5rem 0 0.5rem", color: "var(--color-label-primary)" }} {...props}>{children}</h3>;
  },
  p: ({ children, ...props }) => (
    <p style={{ margin: "0.6rem 0", lineHeight: 1.85, color: "var(--color-label-secondary)", fontSize: "0.82rem" }} {...props}>{children}</p>
  ),
  ul: ({ children, ...props }) => (
    <ul style={{ margin: "0.5rem 0", paddingLeft: "1.5rem", lineHeight: 1.85 }} {...props}>{children}</ul>
  ),
  ol: ({ children, ...props }) => (
    <ol style={{ margin: "0.5rem 0", paddingLeft: "1.5rem", lineHeight: 1.85 }} {...props}>{children}</ol>
  ),
  li: ({ children, ...props }) => (
    <li style={{ margin: "0.3rem 0", fontSize: "0.82rem", color: "var(--color-label-secondary)" }} {...props}>{children}</li>
  ),
  strong: ({ children, ...props }) => (
    <strong style={{ color: "var(--color-label-primary)", fontWeight: 600 }} {...props}>{children}</strong>
  ),
  blockquote: ({ children, ...props }) => (
    <blockquote style={{ margin: "0.75rem 0", paddingLeft: "1rem", borderLeft: "3px solid var(--color-accent)", color: "var(--color-label-tertiary)", fontStyle: "italic" }} {...props}>{children}</blockquote>
  ),
  hr: (props) => (
    <hr style={{ border: "none", borderTop: "1px solid var(--color-separator)", margin: "1.5rem 0" }} {...props} />
  ),
  a: ({ children, href, ...props }) => (
    <a href={href} style={{ color: "var(--color-accent)", textDecoration: "none" }} {...props}>{children}</a>
  ),
};

// ── Outline ──────────────────────────────────────────────────────

function Outline({ content, containerRef }: { content: string; containerRef: React.RefObject<HTMLDivElement | null> }) {
  const headings = useMemo(() => {
    const result: { level: number; text: string; id: string }[] = [];
    for (const line of content.split("\n")) {
      const match = line.match(/^(#{1,4})\s+(.+)/);
      if (match) {
        const text = match[2].trim();
        result.push({ level: match[1].length, text, id: "heading-" + text.replace(/\s+/g, "-") });
      }
    }
    return result;
  }, [content]);

  if (headings.length === 0) return null;

  const handleClick = (id: string) => {
    const el = containerRef.current?.querySelector(`[data-heading-id="${id}"]`);
    if (el) el.scrollIntoView({ behavior: "smooth", block: "start" });
  };

  return (
    <nav style={{ fontSize: "0.75rem", lineHeight: 2 }}>
      <p style={{ margin: "0 0 0.5rem", fontSize: "0.68rem", color: "var(--color-label-quaternary)", textTransform: "uppercase", letterSpacing: "0.06em", fontWeight: 600 }}>
        目录
      </p>
      {headings.map((h, i) => (
        <div
          key={i}
          onClick={() => handleClick(h.id)}
          style={{ paddingLeft: `${(h.level - 1) * 12}px`, color: "var(--color-label-secondary)", cursor: "pointer", whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis" }}
          onMouseEnter={(e) => { e.currentTarget.style.color = "var(--color-accent)"; }}
          onMouseLeave={(e) => { e.currentTarget.style.color = "var(--color-label-secondary)"; }}
        >
          {h.text}
        </div>
      ))}
    </nav>
  );
}

// ── Wiki Preview ─────────────────────────────────────────────────

function WikiPreview({ content }: { content: string }) {
  if (!content) return <p style={{ color: "var(--color-label-quaternary)", fontStyle: "italic", fontSize: "0.82rem" }}>（暂无内容）</p>;
  return (
    <div style={{ padding: "0 1rem" }}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>{content}</ReactMarkdown>
    </div>
  );
}

// ── WikiDetailView (shared layout) ──────────────────────────────

export interface WikiDetailViewProps {
  /** Centered title (string or ReactNode for editable titles) */
  title: ReactNode;
  /** Subtitle below title (e.g. period, role) */
  subtitle?: string;
  /** Short description below subtitle */
  description?: string;
  /** Wiki markdown content */
  wikiContent: string;
  onWikiChange: (v: string) => void;
  onWikiSave: () => void;
  /** Delete action */
  onDelete: () => void;
  deleteLabel?: string;
  /** Footer sections rendered below wiki */
  footer?: ReactNode;
}

export function WikiDetailView({
  title, subtitle, description,
  wikiContent, onWikiChange, onWikiSave,
  onDelete, deleteLabel = "删除",
  footer,
}: WikiDetailViewProps) {
  const [editing, setEditing] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      {/* Header */}
      <div style={{ textAlign: "center", padding: "1.5rem 0 1rem", borderBottom: "1px solid var(--color-separator)" }}>
        <h2 style={{ margin: 0, fontSize: "1.5rem", fontWeight: 700, letterSpacing: "-0.03em" }}>{title}</h2>
        {subtitle && <p style={{ margin: "0.3rem 0 0", fontSize: "0.8rem", color: "var(--color-label-tertiary)" }}>{subtitle}</p>}
        {description && <p style={{ margin: "0.4rem 0 0", fontSize: "0.78rem", color: "var(--color-label-secondary)", maxWidth: 500, marginInline: "auto" }}>{description}</p>}
      </div>

      {/* Body: content + outline */}
      <div ref={contentRef} style={{ flex: 1, display: "flex", overflow: "hidden" }}>
        <div style={{ flex: 1, overflowY: "auto", padding: "2rem 0" }}>
          <div style={{ width: "60%", margin: "0 auto" }}>
            {/* Toggle + delete */}
            <div style={{ display: "flex", gap: "0.25rem", marginBottom: "1rem", justifyContent: "flex-end" }}>
              {(["preview", "edit"] as const).map((mode) => (
                <button
                  key={mode}
                  onClick={() => setEditing(mode === "edit")}
                  style={{
                    background: "none", border: "none",
                    borderBottom: (editing ? "edit" : "preview") === mode ? "1px solid var(--color-accent)" : "1px solid transparent",
                    padding: "0.2rem 0.6rem 0.3rem", cursor: "pointer", fontSize: "0.72rem",
                    color: (editing ? "edit" : "preview") === mode ? "var(--color-accent)" : "var(--color-label-tertiary)",
                    fontFamily: "inherit",
                  }}
                >
                  {mode === "preview" ? "预览" : "编辑"}
                </button>
              ))}
              <button onClick={onDelete} style={{
                background: "none", border: "none", color: "var(--color-label-quaternary)",
                cursor: "pointer", fontSize: "0.68rem", fontFamily: "inherit", marginLeft: "auto",
              }}>
                {deleteLabel}
              </button>
            </div>

            {editing ? (
              <textarea
                value={wikiContent}
                onChange={(e) => onWikiChange(e.target.value)}
                onBlur={onWikiSave}
                placeholder="支持 Markdown 格式..."
                style={{
                  width: "100%", height: "calc(100vh - 220px)", resize: "none", outline: "none",
                  background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)",
                  borderRadius: 6, padding: "0.75rem", color: "var(--color-label-primary)",
                  fontSize: "0.8rem", fontFamily: "monospace", lineHeight: 1.65, boxSizing: "border-box",
                }}
              />
            ) : (
              <WikiPreview content={wikiContent} />
            )}
          </div>
        </div>

        {/* Outline sidebar — always occupies space, content hidden when editing */}
        {wikiContent && (
          <div style={{ width: 180, flexShrink: 0, borderLeft: "1px solid var(--color-separator)", padding: "1.25rem 0.75rem", overflowY: "auto" }}>
            {!editing && <Outline content={wikiContent} containerRef={contentRef} />}
          </div>
        )}
      </div>

      {/* Footer */}
      {footer && (
        <div style={{ borderTop: "1px solid var(--color-separator)", padding: "0.75rem 1.5rem" }}>
          {footer}
        </div>
      )}
    </div>
  );
}
