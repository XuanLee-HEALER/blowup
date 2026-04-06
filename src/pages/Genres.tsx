import { useState, useEffect, useCallback, useMemo, useRef } from "react";
import { library } from "../lib/tauri";
import type { GenreTreeNode, GenreDetail } from "../lib/tauri";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Components } from "react-markdown";

// ── Tree Node ────────────────────────────────────────────────────

function GenreNode({ node, depth, selectedId, onSelect, onAddChild }: {
  node: GenreTreeNode; depth: number; selectedId: number | undefined;
  onSelect: (id: number) => void; onAddChild: (parentId: number) => void;
}) {
  const [expanded, setExpanded] = useState(true);
  const hasChildren = node.children.length > 0;

  return (
    <div>
      <div
        style={{ display: "flex", alignItems: "center", paddingLeft: `${depth * 14 + 6}px`, paddingRight: 6, borderRadius: 5, background: selectedId === node.id ? "var(--color-bg-elevated)" : "transparent", cursor: "pointer" }}
        onMouseEnter={(e) => {
          if (selectedId !== node.id) (e.currentTarget as HTMLDivElement).style.background = "rgba(255,255,255,0.04)";
          const btn = (e.currentTarget as HTMLDivElement).querySelector<HTMLButtonElement>(".add-child");
          if (btn) btn.style.display = "inline";
        }}
        onMouseLeave={(e) => {
          if (selectedId !== node.id) (e.currentTarget as HTMLDivElement).style.background = "transparent";
          const btn = (e.currentTarget as HTMLDivElement).querySelector<HTMLButtonElement>(".add-child");
          if (btn) btn.style.display = "none";
        }}
      >
        <span onClick={() => hasChildren && setExpanded(!expanded)}
          style={{ width: 14, textAlign: "center", color: "var(--color-label-quaternary)", fontSize: "0.65rem", flexShrink: 0, userSelect: "none" }}>
          {hasChildren ? (expanded ? "▾" : "▸") : " "}
        </span>
        <span onClick={() => onSelect(node.id)} style={{ flex: 1, padding: "0.4rem 0.25rem", fontSize: "0.82rem" }}>
          {node.name}
          {node.film_count > 0 && <span style={{ marginLeft: "0.35rem", fontSize: "0.65rem", color: "var(--color-label-quaternary)" }}>{node.film_count}</span>}
        </span>
        <button className="add-child" onClick={(e) => { e.stopPropagation(); onAddChild(node.id); }}
          style={{ display: "none", background: "none", border: "none", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.7rem", padding: "0.2rem 0.3rem", fontFamily: "inherit" }}>
          +
        </button>
      </div>
      {expanded && node.children.map((child) => (
        <GenreNode key={child.id} node={child} depth={depth + 1} selectedId={selectedId} onSelect={onSelect} onAddChild={onAddChild} />
      ))}
    </div>
  );
}

// ── Outline (extracted from wiki markdown headings) ──────────────

function Outline({ content, containerRef }: { content: string; containerRef: React.RefObject<HTMLDivElement | null> }) {
  const headings = useMemo(() => {
    const lines = content.split("\n");
    const result: { level: number; text: string; id: string }[] = [];
    for (const line of lines) {
      const match = line.match(/^(#{1,4})\s+(.+)/);
      if (match) {
        const text = match[2].trim();
        const id = "heading-" + text.replace(/\s+/g, "-");
        result.push({ level: match[1].length, text, id });
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
          style={{
            paddingLeft: `${(h.level - 1) * 12}px`,
            color: "var(--color-label-secondary)",
            cursor: "pointer",
            whiteSpace: "nowrap",
            overflow: "hidden",
            textOverflow: "ellipsis",
          }}
          onMouseEnter={(e) => { e.currentTarget.style.color = "var(--color-accent)"; }}
          onMouseLeave={(e) => { e.currentTarget.style.color = "var(--color-label-secondary)"; }}
        >
          {h.text}
        </div>
      ))}
    </nav>
  );
}

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

// ── Wiki Preview ─────────────────────────────────────────────────

function WikiPreview({ content }: { content: string }) {
  if (!content) {
    return <p style={{ color: "var(--color-label-quaternary)", fontStyle: "italic", fontSize: "0.82rem" }}>（暂无内容）</p>;
  }
  return (
    <div style={{ padding: "0 1rem" }}>
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
        {content}
      </ReactMarkdown>
    </div>
  );
}

// ── Genre Detail View (new layout) ──────────────────────────────

function GenreDetailView({ genre, wikiContent, onWikiChange, onWikiSave, onDelete, onNavigatePerson }: {
  genre: GenreDetail; wikiContent: string;
  onWikiChange: (v: string) => void; onWikiSave: () => void; onDelete: () => void;
  onNavigatePerson: (id: number) => void;
}) {
  const [editing, setEditing] = useState(false);
  const contentRef = useRef<HTMLDivElement>(null);

  return (
    <div style={{ display: "flex", flexDirection: "column", height: "100%" }}>
      {/* Header: centered title + period */}
      <div style={{ textAlign: "center", padding: "1.5rem 0 1rem", borderBottom: "1px solid var(--color-separator)" }}>
        <h2 style={{ margin: 0, fontSize: "1.5rem", fontWeight: 700, letterSpacing: "-0.03em" }}>
          {genre.name}
        </h2>
        {genre.period && (
          <p style={{ margin: "0.3rem 0 0", fontSize: "0.8rem", color: "var(--color-label-tertiary)" }}>
            {genre.period}
          </p>
        )}
        {genre.description && (
          <p style={{ margin: "0.4rem 0 0", fontSize: "0.78rem", color: "var(--color-label-secondary)", maxWidth: 500, marginInline: "auto" }}>
            {genre.description}
          </p>
        )}
      </div>

      {/* Body: wiki content + outline */}
      <div ref={contentRef} style={{ flex: 1, display: "flex", overflow: "hidden" }}>
        {/* Main content area */}
        <div style={{ flex: 1, overflowY: "auto", padding: "2rem 0" }}>
         <div style={{ maxWidth: 680, margin: "0 auto", padding: "0 3rem" }}>
          {/* Edit/Preview toggle */}
          <div style={{ display: "flex", gap: "0.25rem", marginBottom: "1rem", justifyContent: "flex-end" }}>
            {(["preview", "edit"] as const).map((mode) => (
              <button
                key={mode}
                onClick={() => setEditing(mode === "edit")}
                style={{
                  background: "none", border: "none",
                  borderBottom: (editing ? "edit" : "preview") === mode ? "1px solid var(--color-accent)" : "1px solid transparent",
                  padding: "0.2rem 0.6rem 0.3rem",
                  cursor: "pointer", fontSize: "0.72rem",
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
              删除流派
            </button>
          </div>

          {editing ? (
            <textarea
              value={wikiContent}
              onChange={(e) => onWikiChange(e.target.value)}
              onBlur={onWikiSave}
              placeholder="支持 Markdown 格式..."
              style={{
                width: "100%", minHeight: 400, resize: "vertical", outline: "none",
                background: "var(--color-bg-elevated)",
                border: "1px solid var(--color-separator)",
                borderRadius: 6, padding: "0.75rem",
                color: "var(--color-label-primary)",
                fontSize: "0.8rem", fontFamily: "monospace",
                lineHeight: 1.65, boxSizing: "border-box",
              }}
            />
          ) : (
            <WikiPreview content={wikiContent} />
          )}
         </div>
        </div>

        {/* Right outline sidebar */}
        {!editing && wikiContent && (
          <div style={{
            width: 180, flexShrink: 0,
            borderLeft: "1px solid var(--color-separator)",
            padding: "1.25rem 0.75rem",
            overflowY: "auto",
          }}>
            <Outline content={wikiContent} containerRef={contentRef} />
          </div>
        )}
      </div>

      {/* Footer: associated people + films */}
      <div style={{ borderTop: "1px solid var(--color-separator)", padding: "0.75rem 1.5rem" }}>
        {/* People row */}
        <div style={{ marginBottom: genre.films.length > 0 ? "0.6rem" : 0 }}>
          <span style={{ fontSize: "0.68rem", color: "var(--color-label-quaternary)", textTransform: "uppercase", letterSpacing: "0.06em", marginRight: "0.75rem" }}>
            关联影人
          </span>
          {genre.people.length === 0 ? (
            <span style={{ fontSize: "0.75rem", color: "var(--color-label-quaternary)" }}>暂无</span>
          ) : (
            genre.people.map((p, i) => (
              <span key={p.id}>
                <span
                  onClick={() => onNavigatePerson(p.id)}
                  style={{
                    fontSize: "0.78rem", color: "var(--color-accent)",
                    cursor: "pointer", textDecoration: "none",
                  }}
                  onMouseEnter={(e) => { e.currentTarget.style.textDecoration = "underline"; }}
                  onMouseLeave={(e) => { e.currentTarget.style.textDecoration = "none"; }}
                >
                  {p.name}
                </span>
                {i < genre.people.length - 1 && (
                  <span style={{ color: "var(--color-label-quaternary)", margin: "0 0.35rem" }}>·</span>
                )}
              </span>
            ))
          )}
        </div>

        {/* Films row */}
        {genre.films.length > 0 && (
          <div>
            <span style={{ fontSize: "0.68rem", color: "var(--color-label-quaternary)", textTransform: "uppercase", letterSpacing: "0.06em", marginRight: "0.75rem" }}>
              收录电影
            </span>
            {genre.films.map((f, i) => (
              <span key={f.id}>
                <span style={{ fontSize: "0.78rem", color: "var(--color-label-secondary)" }}>
                  《{f.title}》
                  {f.year && <span style={{ fontSize: "0.68rem", color: "var(--color-label-quaternary)" }}>({f.year})</span>}
                </span>
                {i < genre.films.length - 1 && (
                  <span style={{ color: "var(--color-separator)", margin: "0 0.25rem" }}>·</span>
                )}
              </span>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

// ── Add Genre Modal ──────────────────────────────────────────────

function AddGenreModal({ parentId, onClose, onAdded }: { parentId?: number; onClose: () => void; onAdded: () => void }) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [period, setPeriod] = useState("");
  const iStyle: React.CSSProperties = { background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 5, padding: "0.4rem 0.6rem", color: "var(--color-label-primary)", fontSize: "0.82rem", fontFamily: "inherit", outline: "none" };

  const save = async () => {
    if (!name.trim()) return;
    await library.createGenre(name.trim(), parentId, description.trim() || undefined, period.trim() || undefined);
    onAdded();
  };

  type FieldDef = { label: string; value: string; onChange: (v: string) => void; placeholder: string; autoFocus?: boolean };
  const fields: FieldDef[] = [
    { label: "名称", value: name, onChange: setName, placeholder: "流派名称", autoFocus: true },
    { label: "简介", value: description, onChange: setDescription, placeholder: "简短描述（可选）" },
    { label: "年代区间", value: period, onChange: setPeriod, placeholder: "如 1945-1965（可选）" },
  ];

  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.55)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 100 }} onClick={onClose}>
      <div style={{ background: "var(--color-bg-secondary)", border: "1px solid var(--color-separator)", borderRadius: 10, padding: "1.5rem", width: 360, display: "flex", flexDirection: "column", gap: "0.75rem" }} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ margin: 0, fontSize: "0.95rem", fontWeight: 700 }}>{parentId ? "添加子流派" : "添加流派"}</h3>
        {fields.map(({ label, value, onChange, placeholder, autoFocus }) => (
          <div key={label} style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
            <label style={{ fontSize: "0.72rem", color: "var(--color-label-tertiary)" }}>{label}</label>
            <input value={value} onChange={(e) => onChange(e.target.value)} placeholder={placeholder} autoFocus={autoFocus} style={iStyle} />
          </div>
        ))}
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem" }}>
          <button onClick={onClose} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>取消</button>
          <button onClick={save} style={{ background: "var(--color-accent)", border: "none", borderRadius: 6, padding: "0.3rem 0.9rem", color: "#0B1628", fontWeight: 600, cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>添加</button>
        </div>
      </div>
    </div>
  );
}

// ── Main Page ────────────────────────────────────────────────────

export default function Genres() {
  const [tree, setTree] = useState<GenreTreeNode[]>([]);
  const [selected, setSelected] = useState<GenreDetail | null>(null);
  const [wikiContent, setWikiContent] = useState("");
  const [showAddModal, setShowAddModal] = useState(false);
  const [addParentId, setAddParentId] = useState<number | undefined>();

  const loadTree = useCallback(() => { library.listGenresTree().then(setTree).catch(console.error); }, []);
  const loadGenre = useCallback((id: number) => {
    library.getGenre(id).then((g) => { setSelected(g); setWikiContent(g.wiki_content); }).catch(console.error);
  }, []);

  useEffect(() => { loadTree(); }, [loadTree]);

  const handleNavigatePerson = useCallback((_personId: number) => {
    // TODO: navigate to /people and select this person
    // For now, this is a placeholder for cross-page navigation
  }, []);

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      {/* Left sidebar: genre tree */}
      <div style={{ width: 260, flexShrink: 0, borderRight: "1px solid var(--color-separator)", display: "flex", flexDirection: "column", overflow: "hidden" }}>
        <div style={{ padding: "1.2rem 1rem 0.75rem", borderBottom: "1px solid var(--color-separator)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h1 style={{ margin: 0, fontSize: "1.1rem", fontWeight: 700 }}>流派</h1>
          <button onClick={() => { setAddParentId(undefined); setShowAddModal(true); }}
            style={{ background: "var(--color-accent-soft)", border: "1px solid var(--color-accent)", borderRadius: 5, padding: "0.2rem 0.55rem", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>
            + 添加
          </button>
        </div>
        <div style={{ flex: 1, overflowY: "auto", padding: "0.5rem" }}>
          {tree.length === 0 ? (
            <p style={{ padding: "1rem", color: "var(--color-label-tertiary)", fontSize: "0.8rem" }}>知识库中暂无流派</p>
          ) : tree.map((node) => (
            <GenreNode key={node.id} node={node} depth={0} selectedId={selected?.id}
              onSelect={loadGenre}
              onAddChild={(pid) => { setAddParentId(pid); setShowAddModal(true); }}
            />
          ))}
        </div>
      </div>

      {/* Right: detail view */}
      <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>
        {!selected ? (
          <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center" }}>
            <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>选择左侧流派查看详情</p>
          </div>
        ) : (
          <GenreDetailView
            genre={selected} wikiContent={wikiContent}
            onWikiChange={setWikiContent}
            onWikiSave={async () => { if (selected) await library.updateGenreWiki(selected.id, wikiContent).catch(console.error); }}
            onDelete={async () => { await library.deleteGenre(selected.id); setSelected(null); loadTree(); }}
            onNavigatePerson={handleNavigatePerson}
          />
        )}
      </div>

      {showAddModal && (
        <AddGenreModal parentId={addParentId} onClose={() => setShowAddModal(false)}
          onAdded={() => { loadTree(); setShowAddModal(false); }} />
      )}
    </div>
  );
}
