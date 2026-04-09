import { useState, useEffect, useRef } from "react";
import { kb } from "../lib/tauri";
import type { EntrySummary, EntryDetail, RelationEntry } from "../lib/tauri";
import { WikiDetailView } from "../components/WikiDetailView";
import { Chip } from "../components/ui/Chip";
import { MODAL_OVERLAY, MODAL_CARD, INPUT, BTN_SECONDARY, BTN_ACCENT, LABEL, SECTION_HEADER } from "../lib/styles";

// ── Add Entry Modal ─────────────────────────────────────────────

function AddEntryModal({ onClose, onCreated }: { onClose: () => void; onCreated: (id: number) => void }) {
  const [name, setName] = useState("");

  const handleCreate = async () => {
    if (!name.trim()) return;
    const id = await kb.createEntry(name.trim());
    onCreated(id);
  };

  return (
    <div style={MODAL_OVERLAY} onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} style={{ ...MODAL_CARD, width: 360 }}>
        <h3 style={{ margin: "0 0 1rem", fontSize: "0.9rem" }}>新建条目</h3>
        <input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleCreate()}
          placeholder="条目名称"
          style={INPUT}
        />
        <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem", marginTop: "1rem" }}>
          <button onClick={onClose} style={BTN_SECONDARY}>取消</button>
          <button onClick={handleCreate} style={BTN_ACCENT}>创建</button>
        </div>
      </div>
    </div>
  );
}

// ── Add Relation Modal ──────────────────────────────────────────

function AddRelationModal({ currentId, entries, onClose, onAdded }: {
  currentId: number;
  entries: EntrySummary[];
  onClose: () => void;
  onAdded: () => void;
}) {
  const [targetId, setTargetId] = useState<number | "">("");
  const [relationType, setRelationType] = useState("");
  const [existingTypes, setExistingTypes] = useState<string[]>([]);

  useEffect(() => {
    kb.listRelationTypes().then(setExistingTypes).catch(() => {});
  }, []);

  const handleAdd = async () => {
    if (targetId === "" || !relationType.trim()) return;
    await kb.addRelation(currentId, targetId as number, relationType.trim());
    onAdded();
  };

  const candidates = entries.filter((e) => e.id !== currentId);

  return (
    <div style={MODAL_OVERLAY} onClick={onClose}>
      <div onClick={(e) => e.stopPropagation()} style={{ ...MODAL_CARD, width: 380 }}>
        <h3 style={{ margin: "0 0 1rem", fontSize: "0.9rem" }}>添加关系</h3>

        <label style={LABEL}>目标条目</label>
        <select
          value={targetId}
          onChange={(e) => setTargetId(e.target.value ? Number(e.target.value) : "")}
          style={{ ...INPUT, marginBottom: "0.75rem" }}
        >
          <option value="">选择...</option>
          {candidates.map((e) => <option key={e.id} value={e.id}>{e.name}</option>)}
        </select>

        <label style={LABEL}>关系类型</label>
        <input
          value={relationType}
          onChange={(e) => setRelationType(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && handleAdd()}
          placeholder="例: 影响、合作、出演..."
          list="relation-types"
          style={INPUT}
        />
        <datalist id="relation-types">
          {existingTypes.map((t) => <option key={t} value={t} />)}
        </datalist>

        <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem", marginTop: "1rem" }}>
          <button onClick={onClose} style={BTN_SECONDARY}>取消</button>
          <button onClick={handleAdd} style={BTN_ACCENT}>添加</button>
        </div>
      </div>
    </div>
  );
}

// ── Entry Detail View ───────────────────────────────────────────

function EntryDetailView({ entry, entries, onWikiChange, onDeleted, onUpdated }: {
  entry: EntryDetail;
  entries: EntrySummary[];
  onWikiChange: (wiki: string) => void;
  onDeleted: () => void;
  onUpdated: () => void;
}) {
  const [wiki, setWiki] = useState(entry.wiki);
  const [showRelModal, setShowRelModal] = useState(false);
  const [newTag, setNewTag] = useState("");
  const [editingName, setEditingName] = useState(false);
  const [name, setName] = useState(entry.name);

  const handleSaveWiki = async () => {
    await kb.updateEntryWiki(entry.id, wiki);
    onWikiChange(wiki);
  };

  const handleSaveName = async () => {
    if (name.trim() && name !== entry.name) {
      await kb.updateEntryName(entry.id, name.trim());
      onUpdated();
    }
    setEditingName(false);
  };

  const handleAddTag = async () => {
    const t = newTag.trim();
    if (!t) return;
    await kb.addTag(entry.id, t);
    setNewTag("");
    onUpdated();
  };

  const handleRemoveTag = async (tag: string) => {
    await kb.removeTag(entry.id, tag);
    onUpdated();
  };

  const handleDelete = async () => {
    await kb.deleteEntry(entry.id);
    onDeleted();
  };

  const handleRemoveRelation = async (id: number) => {
    await kb.removeRelation(id);
    onUpdated();
  };

  // Build title - editable on double-click
  const titleEl = editingName ? (
    <input
      autoFocus
      value={name}
      onChange={(e) => setName(e.target.value)}
      onBlur={handleSaveName}
      onKeyDown={(e) => e.key === "Enter" && handleSaveName()}
      style={{ fontSize: "1.5rem", fontWeight: 700, textAlign: "center", background: "transparent", border: "none", borderBottom: "1px solid var(--color-accent)", color: "var(--color-label-primary)", outline: "none", fontFamily: "inherit", width: "auto" }}
    />
  ) : (
    <span onDoubleClick={() => setEditingName(true)} style={{ cursor: "pointer" }} title="双击编辑名称">{entry.name}</span>
  );

  const footer = (
    <div style={{ display: "flex", flexDirection: "column", gap: "0.75rem" }}>
      {/* Tags */}
      <div>
        <p style={{ ...SECTION_HEADER, marginBottom: "0.4rem" }}>标签</p>
        <div style={{ display: "flex", flexWrap: "wrap", gap: "0.3rem", alignItems: "center" }}>
          {entry.tags.map((tag) => (
            <Chip key={tag} label={tag} onRemove={() => handleRemoveTag(tag)} />
          ))}
          <input
            value={newTag}
            onChange={(e) => setNewTag(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleAddTag()}
            placeholder="+ 标签"
            style={{ width: 80, padding: "0.15rem 0.4rem", background: "transparent", border: "1px solid var(--color-separator)", borderRadius: 100, fontSize: "0.72rem", color: "var(--color-label-secondary)", outline: "none", fontFamily: "inherit" }}
          />
        </div>
      </div>

      {/* Relations */}
      <div>
        <div style={{ display: "flex", alignItems: "center", gap: "0.5rem", marginBottom: "0.4rem" }}>
          <p style={SECTION_HEADER}>关系</p>
          <button
            onClick={() => setShowRelModal(true)}
            style={{ background: "none", border: "none", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.72rem", fontFamily: "inherit", padding: 0 }}
          >+ 添加</button>
        </div>
        {entry.relations.length === 0 ? (
          <p style={{ fontSize: "0.78rem", color: "var(--color-label-quaternary)", fontStyle: "italic", margin: 0 }}>暂无关系</p>
        ) : (
          <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
            {entry.relations.map((rel) => (
              <RelationRow key={rel.id} rel={rel} onRemove={() => handleRemoveRelation(rel.id)} />
            ))}
          </div>
        )}
      </div>

      {showRelModal && (
        <AddRelationModal
          currentId={entry.id}
          entries={entries}
          onClose={() => setShowRelModal(false)}
          onAdded={() => { setShowRelModal(false); onUpdated(); }}
        />
      )}
    </div>
  );

  return (
    <WikiDetailView
      title={titleEl}
      wikiContent={wiki}
      onWikiChange={setWiki}
      onWikiSave={handleSaveWiki}
      onDelete={handleDelete}
      deleteLabel="删除条目"
      footer={footer}
    />
  );
}

function RelationRow({ rel, onRemove }: { rel: RelationEntry; onRemove: () => void }) {
  const arrow = rel.direction === "to" ? "→" : "←";
  return (
    <div
      style={{ display: "flex", alignItems: "center", gap: "0.4rem", fontSize: "0.78rem", padding: "0.15rem 0" }}
      onMouseEnter={(e) => {
        const btn = e.currentTarget.querySelector<HTMLButtonElement>(".rel-remove");
        if (btn) btn.style.opacity = "1";
      }}
      onMouseLeave={(e) => {
        const btn = e.currentTarget.querySelector<HTMLButtonElement>(".rel-remove");
        if (btn) btn.style.opacity = "0";
      }}
    >
      <span style={{ color: "var(--color-label-quaternary)" }}>{arrow}</span>
      <span style={{ color: "var(--color-label-primary)" }}>{rel.target_name}</span>
      <span style={{ color: "var(--color-label-quaternary)", fontSize: "0.72rem" }}>({rel.relation_type})</span>
      <button
        className="rel-remove"
        onClick={onRemove}
        style={{ background: "none", border: "none", color: "var(--color-label-quaternary)", cursor: "pointer", fontSize: "0.65rem", fontFamily: "inherit", padding: "0 0.2rem", opacity: 0, transition: "opacity 0.15s" }}
      >✕</button>
    </div>
  );
}

// ── Main Wiki Page ──────────────────────────────────────────────

export default function Wiki() {
  const [entries, setEntries] = useState<EntrySummary[]>([]);
  const [allTags, setAllTags] = useState<string[]>([]);
  const [selectedTag, setSelectedTag] = useState<string | undefined>();
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedId, setSelectedId] = useState<number | undefined>();
  const [detail, setDetail] = useState<EntryDetail | null>(null);
  const [showAddModal, setShowAddModal] = useState(false);

  const refreshRef = useRef(0);
  const refresh = () => { refreshRef.current += 1; setRefreshKey(refreshRef.current); };
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    const q = searchQuery.trim() || undefined;
    kb.listEntries(q, selectedTag).then(setEntries);
  }, [searchQuery, selectedTag, refreshKey]);

  useEffect(() => {
    kb.listAllTags().then(setAllTags);
  }, [refreshKey]);

  useEffect(() => {
    if (selectedId != null) kb.getEntry(selectedId).then(setDetail);
  }, [selectedId, refreshKey]);

  const handleCreated = (id: number) => {
    setShowAddModal(false);
    setSelectedId(id);
    refresh();
  };

  const handleUpdated = () => {
    refresh();
  };

  const handleDeleted = () => {
    setSelectedId(undefined);
    setDetail(null);
    refresh();
  };

  return (
    <div style={{ display: "flex", height: "100%" }}>
      {/* Left Panel: tag bar + search + list */}
      <div style={{ width: 260, flexShrink: 0, borderRight: "1px solid var(--color-separator)", display: "flex", flexDirection: "column" }}>
        {/* Header */}
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "0.75rem 0.75rem 0" }}>
          <h2 style={{ margin: 0, fontSize: "0.9rem", fontWeight: 700 }}>Wiki</h2>
          <button
            onClick={() => setShowAddModal(true)}
            style={{ background: "none", border: "none", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.78rem", fontFamily: "inherit" }}
          >+ 添加</button>
        </div>

        {/* Tag bar */}
        {allTags.length > 0 && (
          <div style={{ display: "flex", flexWrap: "wrap", gap: "0.25rem", padding: "0.5rem 0.75rem 0" }}>
            {allTags.map((tag) => (
              <Chip
                key={tag}
                label={tag}
                active={selectedTag === tag}
                onClick={() => setSelectedTag(selectedTag === tag ? undefined : tag)}
              />
            ))}
          </div>
        )}

        {/* Search */}
        <div style={{ padding: "0.5rem 0.75rem" }}>
          <input
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="搜索..."
            style={{ width: "100%", padding: "0.4rem 0.5rem", background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)", borderRadius: 6, fontSize: "0.78rem", color: "var(--color-label-primary)", outline: "none", fontFamily: "inherit", boxSizing: "border-box" }}
          />
        </div>

        {/* Entry list */}
        <div style={{ flex: 1, overflowY: "auto" }}>
          {entries.map((entry) => (
            <div
              key={entry.id}
              onClick={() => setSelectedId(entry.id)}
              style={{
                padding: "0.5rem 0.75rem",
                cursor: "pointer",
                background: selectedId === entry.id ? "var(--color-bg-elevated)" : "transparent",
                borderRadius: 5,
                margin: "0 0.25rem",
              }}
              onMouseEnter={(e) => { if (selectedId !== entry.id) e.currentTarget.style.background = "var(--color-hover)"; }}
              onMouseLeave={(e) => { if (selectedId !== entry.id) e.currentTarget.style.background = "transparent"; }}
            >
              <div style={{ fontSize: "0.82rem", fontWeight: 500, color: "var(--color-label-primary)" }}>{entry.name}</div>
              {entry.tags.length > 0 && (
                <div style={{ display: "flex", gap: "0.2rem", marginTop: "0.2rem", flexWrap: "wrap" }}>
                  {entry.tags.map((t) => (
                    <span key={t} style={{ fontSize: "0.65rem", color: "var(--color-label-quaternary)", background: "var(--color-bg-control)", padding: "0.05rem 0.35rem", borderRadius: 100 }}>{t}</span>
                  ))}
                </div>
              )}
              <div style={{ fontSize: "0.65rem", color: "var(--color-label-quaternary)", marginTop: "0.15rem" }}>
                {entry.updated_at.slice(0, 16)}
              </div>
            </div>
          ))}
          {entries.length === 0 && (
            <p style={{ textAlign: "center", color: "var(--color-label-quaternary)", fontSize: "0.78rem", padding: "2rem 0" }}>
              {searchQuery || selectedTag ? "未找到匹配条目" : "暂无条目"}
            </p>
          )}
        </div>
      </div>

      {/* Right Panel: detail */}
      <div style={{ flex: 1, overflow: "hidden" }}>
        {detail ? (
          <EntryDetailView
            key={detail.id + "-" + detail.updated_at}
            entry={detail}
            entries={entries}
            onWikiChange={() => {}}
            onDeleted={handleDeleted}
            onUpdated={handleUpdated}
          />
        ) : (
          <div style={{ display: "flex", alignItems: "center", justifyContent: "center", height: "100%", color: "var(--color-label-quaternary)", fontSize: "0.82rem" }}>
            选择或创建一个条目
          </div>
        )}
      </div>

      {showAddModal && (
        <AddEntryModal onClose={() => setShowAddModal(false)} onCreated={handleCreated} />
      )}
    </div>
  );
}
