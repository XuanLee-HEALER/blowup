import { useState, useEffect, useCallback } from "react";
import { library } from "../lib/tauri";
import type { PersonSummary, PersonDetail } from "../lib/tauri";
import { WikiDetailView } from "../components/WikiDetailView";

const ROLE_LABELS: Record<string, string> = {
  director: "导演", cinematographer: "摄影", composer: "音乐",
  editor: "剪辑", screenwriter: "编剧", producer: "制片", actor: "演员",
};
const PRIMARY_ROLES = Object.keys(ROLE_LABELS);

// ── Shared modal primitives ───────────────────────────────────────
const inputStyle: React.CSSProperties = {
  background: "var(--color-bg-elevated)", border: "1px solid var(--color-separator)",
  borderRadius: 5, padding: "0.4rem 0.6rem", color: "var(--color-label-primary)",
  fontSize: "0.82rem", fontFamily: "inherit", outline: "none", width: "100%", boxSizing: "border-box",
};

function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: React.ReactNode }) {
  return (
    <div style={{ position: "fixed", inset: 0, background: "rgba(0,0,0,0.55)", display: "flex", alignItems: "center", justifyContent: "center", zIndex: 100 }} onClick={onClose}>
      <div style={{ background: "var(--color-bg-secondary)", border: "1px solid var(--color-separator)", borderRadius: 10, padding: "1.5rem", width: 360, display: "flex", flexDirection: "column", gap: "0.75rem" }} onClick={(e) => e.stopPropagation()}>
        <h3 style={{ margin: 0, fontSize: "0.95rem", fontWeight: 700 }}>{title}</h3>
        {children}
      </div>
    </div>
  );
}

function ModalField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ display: "flex", flexDirection: "column", gap: "0.25rem" }}>
      <label style={{ fontSize: "0.72rem", color: "var(--color-label-tertiary)" }}>{label}</label>
      {children}
    </div>
  );
}

function ModalActions({ onCancel, onConfirm, confirmLabel }: { onCancel: () => void; onConfirm: () => void; confirmLabel: string }) {
  return (
    <div style={{ display: "flex", justifyContent: "flex-end", gap: "0.5rem" }}>
      <button onClick={onCancel} style={{ background: "none", border: "none", color: "var(--color-label-tertiary)", cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>取消</button>
      <button onClick={onConfirm} style={{ background: "var(--color-accent)", border: "none", borderRadius: 6, padding: "0.3rem 0.9rem", color: "#0B1628", fontWeight: 600, cursor: "pointer", fontSize: "0.8rem", fontFamily: "inherit" }}>{confirmLabel}</button>
    </div>
  );
}

// ── Add Person Modal ──────────────────────────────────────────────
function AddPersonModal({ onClose, onAdded }: { onClose: () => void; onAdded: () => void }) {
  const [name, setName] = useState("");
  const [role, setRole] = useState("director");
  const [tmdbId, setTmdbId] = useState("");

  const save = async () => {
    if (!name.trim()) return;
    await library.createPerson(name.trim(), role, tmdbId ? parseInt(tmdbId) : undefined);
    onAdded();
  };

  return (
    <Modal title="添加影人" onClose={onClose}>
      <ModalField label="姓名">
        <input value={name} onChange={(e) => setName(e.target.value)} placeholder="影人姓名" autoFocus style={inputStyle} />
      </ModalField>
      <ModalField label="主要角色">
        <select value={role} onChange={(e) => setRole(e.target.value)} style={{ ...inputStyle, cursor: "pointer" }}>
          {PRIMARY_ROLES.map((r) => <option key={r} value={r}>{ROLE_LABELS[r]}</option>)}
        </select>
      </ModalField>
      <ModalField label="TMDB ID（可选）">
        <input value={tmdbId} onChange={(e) => setTmdbId(e.target.value)} placeholder="如 19429" style={inputStyle} />
      </ModalField>
      <ModalActions onCancel={onClose} onConfirm={save} confirmLabel="添加" />
    </Modal>
  );
}

// ── Add Relation Modal ────────────────────────────────────────────
function AddRelationModal({ fromId, onClose, onAdded }: { fromId: number; onClose: () => void; onAdded: () => void }) {
  const [allPeople, setAllPeople] = useState<PersonSummary[]>([]);
  const [toId, setToId] = useState<number | null>(null);
  const [relType, setRelType] = useState("influenced");

  useEffect(() => { library.listPeople().then(setAllPeople); }, []);

  const save = async () => {
    if (!toId) return;
    await library.addPersonRelation(fromId, toId, relType);
    onAdded();
  };

  return (
    <Modal title="添加影人关系" onClose={onClose}>
      <ModalField label="关联影人">
        <select value={toId ?? ""} onChange={(e) => setToId(parseInt(e.target.value))} style={{ ...inputStyle, cursor: "pointer" }}>
          <option value="">请选择...</option>
          {allPeople.filter((p) => p.id !== fromId).map((p) => (
            <option key={p.id} value={p.id}>{p.name}</option>
          ))}
        </select>
      </ModalField>
      <ModalField label="关系类型">
        <select value={relType} onChange={(e) => setRelType(e.target.value)} style={{ ...inputStyle, cursor: "pointer" }}>
          <option value="influenced">影响（→）</option>
          <option value="contemporary">同时期（↔）</option>
          <option value="collaborated">合作（↔）</option>
        </select>
      </ModalField>
      <ModalActions onCancel={onClose} onConfirm={save} confirmLabel="添加" />
    </Modal>
  );
}

// ── Person detail (using shared WikiDetailView) ──────────────────
function PersonDetailView({
  person, wikiContent, onWikiChange, onWikiSave, onDelete, onRelationAdded,
}: {
  person: PersonDetail; wikiContent: string;
  onWikiChange: (v: string) => void; onWikiSave: () => void;
  onDelete: () => void; onRelationAdded: () => void;
}) {
  const [showRelModal, setShowRelModal] = useState(false);

  const subtitle = [
    ROLE_LABELS[person.primary_role] ?? person.primary_role,
    person.nationality,
    person.born_date ? `出生：${person.born_date}` : null,
  ].filter(Boolean).join("  ·  ");

  const footer = (
    <>
      {/* Films */}
      <div style={{ marginBottom: person.relations.length > 0 ? "0.6rem" : 0 }}>
        <span style={{ fontSize: "0.68rem", color: "var(--color-label-quaternary)", textTransform: "uppercase", letterSpacing: "0.06em", marginRight: "0.75rem" }}>作品</span>
        {person.films.length === 0 ? (
          <span style={{ fontSize: "0.75rem", color: "var(--color-label-quaternary)" }}>暂无</span>
        ) : person.films.map((f, i) => (
          <span key={f.film_id}>
            <span style={{ fontSize: "0.78rem", color: "var(--color-label-secondary)" }}>
              《{f.title}》{f.year && <span style={{ fontSize: "0.68rem", color: "var(--color-label-quaternary)" }}>({f.year})</span>}
              <span style={{ fontSize: "0.65rem", color: "var(--color-label-quaternary)" }}> {f.role}</span>
            </span>
            {i < person.films.length - 1 && <span style={{ color: "var(--color-separator)", margin: "0 0.25rem" }}>·</span>}
          </span>
        ))}
      </div>

      {/* Relations */}
      <div style={{ display: "flex", alignItems: "center", gap: "0.5rem", flexWrap: "wrap" }}>
        <span style={{ fontSize: "0.68rem", color: "var(--color-label-quaternary)", textTransform: "uppercase", letterSpacing: "0.06em" }}>关系</span>
        {person.relations.length === 0 ? (
          <span style={{ fontSize: "0.75rem", color: "var(--color-label-quaternary)" }}>暂无</span>
        ) : person.relations.map((r) => (
          <span key={`${r.direction}-${r.target_id}-${r.relation_type}`} style={{ fontSize: "0.75rem", color: "var(--color-label-secondary)" }}>
            <span style={{ color: "var(--color-label-quaternary)", fontSize: "0.68rem" }}>
              {r.direction === "to" ? "→" : r.direction === "from" ? "←" : "↔"}
            </span>{" "}
            <span style={{ color: "var(--color-accent)" }}>{r.target_name}</span>
          </span>
        ))}
        <button onClick={() => setShowRelModal(true)} style={{ background: "none", border: "none", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.68rem", fontFamily: "inherit" }}>+ 添加</button>
      </div>

      {showRelModal && (
        <AddRelationModal fromId={person.id} onClose={() => setShowRelModal(false)}
          onAdded={() => { onRelationAdded(); setShowRelModal(false); }} />
      )}
    </>
  );

  return (
    <WikiDetailView
      title={person.name}
      subtitle={subtitle}
      wikiContent={wikiContent}
      onWikiChange={onWikiChange}
      onWikiSave={onWikiSave}
      onDelete={onDelete}
      deleteLabel="删除影人"
      footer={footer}
    />
  );
}

// ── Main page ─────────────────────────────────────────────────────
export default function People() {
  const [people, setPeople] = useState<PersonSummary[]>([]);
  const [selected, setSelected] = useState<PersonDetail | null>(null);
  const [wikiContent, setWikiContent] = useState("");
  const [showAddModal, setShowAddModal] = useState(false);

  const loadPeople = useCallback(() => { library.listPeople().then(setPeople).catch(console.error); }, []);
  const loadPerson = useCallback((id: number) => {
    library.getPerson(id).then((p) => { setSelected(p); setWikiContent(p.wiki_content); }).catch(console.error);
  }, []);

  useEffect(() => { loadPeople(); }, [loadPeople]);

  const saveWiki = async () => {
    if (!selected) return;
    await library.updatePersonWiki(selected.id, wikiContent).catch(console.error);
  };

  return (
    <div style={{ display: "flex", height: "100%", overflow: "hidden" }}>
      {/* Left list */}
      <div style={{ width: 260, flexShrink: 0, borderRight: "1px solid var(--color-separator)", display: "flex", flexDirection: "column", overflow: "hidden" }}>
        <div style={{ padding: "1.2rem 1rem 0.75rem", borderBottom: "1px solid var(--color-separator)", display: "flex", justifyContent: "space-between", alignItems: "center" }}>
          <h1 style={{ margin: 0, fontSize: "1.1rem", fontWeight: 700 }}>影人</h1>
          <button onClick={() => setShowAddModal(true)}
            style={{ background: "var(--color-accent-soft)", border: "1px solid var(--color-accent)", borderRadius: 5, padding: "0.2rem 0.55rem", color: "var(--color-accent)", cursor: "pointer", fontSize: "0.75rem", fontFamily: "inherit" }}>
            + 添加
          </button>
        </div>
        <div style={{ flex: 1, overflowY: "auto", padding: "0.5rem" }}>
          {people.length === 0 ? (
            <p style={{ padding: "1rem", color: "var(--color-label-tertiary)", fontSize: "0.8rem" }}>知识库中暂无影人</p>
          ) : people.map((p) => (
            <div key={p.id}
              onClick={() => loadPerson(p.id)}
              style={{ padding: "0.45rem 0.75rem", borderRadius: 5, cursor: "pointer", background: selected?.id === p.id ? "var(--color-bg-elevated)" : "transparent" }}
              onMouseEnter={(e) => { if (selected?.id !== p.id) (e.currentTarget as HTMLDivElement).style.background = "rgba(255,255,255,0.04)"; }}
              onMouseLeave={(e) => { if (selected?.id !== p.id) (e.currentTarget as HTMLDivElement).style.background = "transparent"; }}
            >
              <div style={{ fontSize: "0.82rem" }}>{p.name}</div>
              <div style={{ fontSize: "0.68rem", color: "var(--color-label-quaternary)" }}>
                {ROLE_LABELS[p.primary_role] ?? p.primary_role}
                {p.film_count > 0 && ` · ${p.film_count} 部`}
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Right detail */}
      <div style={{ flex: 1, display: "flex", flexDirection: "column", overflow: "hidden" }}>
        {!selected ? (
          <div style={{ flex: 1, display: "flex", alignItems: "center", justifyContent: "center" }}>
            <p style={{ color: "var(--color-label-tertiary)", fontSize: "0.85rem" }}>选择左侧影人查看详情</p>
          </div>
        ) : (
          <PersonDetailView
            person={selected} wikiContent={wikiContent}
            onWikiChange={setWikiContent}
            onWikiSave={saveWiki}
            onDelete={async () => { await library.deletePerson(selected.id); setSelected(null); loadPeople(); }}
            onRelationAdded={() => loadPerson(selected.id)}
          />
        )}
      </div>

      {showAddModal && (
        <AddPersonModal onClose={() => setShowAddModal(false)}
          onAdded={() => { loadPeople(); setShowAddModal(false); }} />
      )}
    </div>
  );
}
