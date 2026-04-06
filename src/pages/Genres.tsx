// src/pages/Genres.tsx
import { useState, useEffect, useCallback } from "react";
import { library } from "../lib/tauri";
import type { GenreTreeNode, GenreDetail } from "../lib/tauri";
import { WikiEditor } from "../components/WikiEditor";

function SectionTitle({ children }: { children: React.ReactNode }) {
  return (
    <p style={{ margin: "1.25rem 0 0.5rem", fontSize: "0.72rem", color: "var(--color-label-quaternary)", textTransform: "uppercase", letterSpacing: "0.06em", fontWeight: 600 }}>
      {children}
    </p>
  );
}

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

function GenreDetailView({ genre, wikiContent, onWikiChange, onWikiSave, onDelete }: {
  genre: GenreDetail; wikiContent: string;
  onWikiChange: (v: string) => void; onWikiSave: () => void; onDelete: () => void;
}) {
  return (
    <div style={{ maxWidth: 680 }}>
      <div style={{ display: "flex", justifyContent: "space-between", alignItems: "flex-start", marginBottom: "0.25rem" }}>
        <div>
          <h2 style={{ margin: 0, fontSize: "1.4rem", fontWeight: 700, letterSpacing: "-0.03em" }}>{genre.name}</h2>
          {genre.period && <p style={{ margin: "0.15rem 0 0", fontSize: "0.75rem", color: "var(--color-label-tertiary)" }}>{genre.period}</p>}
        </div>
        <button onClick={onDelete} style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>删除</button>
      </div>

      <SectionTitle>简介 Wiki</SectionTitle>
      <WikiEditor value={wikiContent} onChange={onWikiChange} onSave={onWikiSave} />

      <SectionTitle>关联影人</SectionTitle>
      {genre.people.length === 0 ? (
        <p style={{ fontSize: "0.78rem", color: "var(--color-label-quaternary)" }}>暂无关联影人</p>
      ) : (
        <div style={{ display: "flex", flexWrap: "wrap", gap: "0.4rem" }}>
          {genre.people.map((p) => (
            <span key={p.id} style={{ background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 4, padding: "0.2rem 0.5rem", fontSize: "0.75rem", color: "var(--color-label-secondary)" }}>{p.name}</span>
          ))}
        </div>
      )}

      <SectionTitle>收录电影</SectionTitle>
      {genre.films.length === 0 ? (
        <p style={{ fontSize: "0.78rem", color: "var(--color-label-quaternary)" }}>暂无收录电影</p>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "0.3rem" }}>
          {genre.films.map((f) => (
            <div key={f.id} style={{ display: "flex", alignItems: "center", gap: "0.5rem", fontSize: "0.8rem" }}>
              <span>{f.title}</span>
              {f.year && <span style={{ color: "var(--color-label-quaternary)" }}>({f.year})</span>}
              {f.tmdb_rating && <span style={{ color: "var(--color-accent)", fontSize: "0.72rem" }}>★ {f.tmdb_rating.toFixed(1)}</span>}
            </div>
          ))}
        </div>
      )}

      {genre.children.length > 0 && (
        <>
          <SectionTitle>子流派</SectionTitle>
          <div style={{ display: "flex", flexWrap: "wrap", gap: "0.4rem" }}>
            {genre.children.map((c) => (
              <span key={c.id} style={{ background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 4, padding: "0.2rem 0.5rem", fontSize: "0.75rem", color: "var(--color-label-secondary)" }}>
                {c.name} ({c.film_count})
              </span>
            ))}
          </div>
        </>
      )}
    </div>
  );
}

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

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
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

      <div style={{ flex: 1, overflowY: "auto", padding: "1.5rem" }}>
        {!selected ? (
          <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>选择左侧流派查看详情</p>
        ) : (
          <GenreDetailView
            genre={selected} wikiContent={wikiContent}
            onWikiChange={setWikiContent}
            onWikiSave={async () => { if (selected) await library.updateGenreWiki(selected.id, wikiContent).catch(console.error); }}
            onDelete={async () => { await library.deleteGenre(selected.id); setSelected(null); loadTree(); }}
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
