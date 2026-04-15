import { useState, useEffect, useRef, useMemo } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Components } from "react-markdown";
import {
  ActionIcon,
  Anchor,
  Badge,
  Box,
  Button,
  Divider,
  Flex,
  Group,
  Modal,
  Pill,
  ScrollArea,
  Select,
  Stack,
  Tabs,
  Text,
  Textarea,
  TextInput,
  Title,
  UnstyledButton,
} from "@mantine/core";
import { kb } from "../lib/tauri";
import type { EntrySummary, EntryDetail, RelationEntry } from "../lib/tauri";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

// ── Markdown renderer ───────────────────────────────────────────

const mdComponents: Components = {
  h1: ({ children, ...props }) => {
    const id = "heading-" + String(children).replace(/\s+/g, "-");
    return (
      <h1
        data-heading-id={id}
        style={{
          fontSize: "1.3rem",
          fontWeight: 700,
          margin: "2rem 0 0.75rem",
          color: "var(--color-label-primary)",
          letterSpacing: "-0.02em",
        }}
        {...props}
      >
        {children}
      </h1>
    );
  },
  h2: ({ children, ...props }) => {
    const id = "heading-" + String(children).replace(/\s+/g, "-");
    return (
      <h2
        data-heading-id={id}
        style={{
          fontSize: "1.1rem",
          fontWeight: 600,
          margin: "2rem 0 0.6rem",
          paddingBottom: "0.35rem",
          borderBottom: "1px solid var(--color-separator)",
          color: "var(--color-label-primary)",
        }}
        {...props}
      >
        {children}
      </h2>
    );
  },
  h3: ({ children, ...props }) => {
    const id = "heading-" + String(children).replace(/\s+/g, "-");
    return (
      <h3
        data-heading-id={id}
        style={{
          fontSize: "0.95rem",
          fontWeight: 600,
          margin: "1.5rem 0 0.5rem",
          color: "var(--color-label-primary)",
        }}
        {...props}
      >
        {children}
      </h3>
    );
  },
  p: ({ children, ...props }) => (
    <p
      style={{
        margin: "0.6rem 0",
        lineHeight: 1.85,
        color: "var(--color-label-secondary)",
        fontSize: "0.82rem",
      }}
      {...props}
    >
      {children}
    </p>
  ),
  ul: ({ children, ...props }) => (
    <ul style={{ margin: "0.5rem 0", paddingLeft: "1.5rem", lineHeight: 1.85 }} {...props}>
      {children}
    </ul>
  ),
  ol: ({ children, ...props }) => (
    <ol style={{ margin: "0.5rem 0", paddingLeft: "1.5rem", lineHeight: 1.85 }} {...props}>
      {children}
    </ol>
  ),
  li: ({ children, ...props }) => (
    <li
      style={{
        margin: "0.3rem 0",
        fontSize: "0.82rem",
        color: "var(--color-label-secondary)",
      }}
      {...props}
    >
      {children}
    </li>
  ),
  strong: ({ children, ...props }) => (
    <strong style={{ color: "var(--color-label-primary)", fontWeight: 600 }} {...props}>
      {children}
    </strong>
  ),
  blockquote: ({ children, ...props }) => (
    <blockquote
      style={{
        margin: "0.75rem 0",
        paddingLeft: "1rem",
        borderLeft: "3px solid var(--color-accent)",
        color: "var(--color-label-tertiary)",
        fontStyle: "italic",
      }}
      {...props}
    >
      {children}
    </blockquote>
  ),
  hr: (props) => (
    <hr
      style={{
        border: "none",
        borderTop: "1px solid var(--color-separator)",
        margin: "1.5rem 0",
      }}
      {...props}
    />
  ),
  a: ({ children, href, ...props }) => (
    <a href={href} style={{ color: "var(--color-accent)", textDecoration: "none" }} {...props}>
      {children}
    </a>
  ),
};

function WikiPreview({ content }: { content: string }) {
  if (!content) {
    return (
      <Text c="var(--color-label-quaternary)" fs="italic" size="sm">
        （暂无内容）
      </Text>
    );
  }
  return (
    <Box px="1rem">
      <ReactMarkdown remarkPlugins={[remarkGfm]} components={mdComponents}>
        {content}
      </ReactMarkdown>
    </Box>
  );
}

// ── Outline (markdown headings sidebar) ─────────────────────────

function Outline({
  content,
  containerRef,
}: {
  content: string;
  containerRef: React.RefObject<HTMLDivElement | null>;
}) {
  const headings = useMemo(() => {
    const result: { level: number; text: string; id: string }[] = [];
    for (const line of content.split("\n")) {
      const match = line.match(/^(#{1,4})\s+(.+)/);
      if (match) {
        const text = match[2].trim();
        result.push({
          level: match[1].length,
          text,
          id: "heading-" + text.replace(/\s+/g, "-"),
        });
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
    <Stack gap={2}>
      <Text
        size="xs"
        tt="uppercase"
        c="var(--color-label-quaternary)"
        fw={600}
        mb="0.5rem"
        style={{ letterSpacing: "0.06em", fontSize: "0.68rem" }}
      >
        目录
      </Text>
      {headings.map((h, i) => (
        <Text
          key={i}
          size="xs"
          c="var(--color-label-secondary)"
          truncate
          onClick={() => handleClick(h.id)}
          style={{
            paddingLeft: `${(h.level - 1) * 12}px`,
            cursor: "pointer",
            lineHeight: 2,
          }}
        >
          {h.text}
        </Text>
      ))}
    </Stack>
  );
}

// ── Add Entry Modal ─────────────────────────────────────────────

function AddEntryModal({
  opened,
  onClose,
  onCreated,
}: {
  opened: boolean;
  onClose: () => void;
  onCreated: (id: number) => void;
}) {
  const [name, setName] = useState("");

  const handleCreate = async () => {
    if (!name.trim()) return;
    const id = await kb.createEntry(name.trim());
    setName("");
    onCreated(id);
  };

  return (
    <Modal opened={opened} onClose={onClose} title="新建条目" size="sm" centered>
      <Stack gap="md">
        <TextInput
          autoFocus
          value={name}
          onChange={(e) => setName(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && handleCreate()}
          placeholder="条目名称"
        />
        <Group justify="flex-end" gap="0.5rem">
          <Button variant="subtle" onClick={onClose}>
            取消
          </Button>
          <Button onClick={handleCreate}>创建</Button>
        </Group>
      </Stack>
    </Modal>
  );
}

// ── Add Relation Modal ──────────────────────────────────────────

function AddRelationModal({
  opened,
  currentId,
  entries,
  onClose,
  onAdded,
}: {
  opened: boolean;
  currentId: number;
  entries: EntrySummary[];
  onClose: () => void;
  onAdded: () => void;
}) {
  const [targetId, setTargetId] = useState<string | null>(null);
  const [relationType, setRelationType] = useState("");
  const [existingTypes, setExistingTypes] = useState<string[]>([]);

  useEffect(() => {
    if (opened) {
      kb.listRelationTypes().then(setExistingTypes).catch(() => {});
    }
  }, [opened]);

  const handleAdd = async () => {
    if (!targetId || !relationType.trim()) return;
    await kb.addRelation(currentId, Number(targetId), relationType.trim());
    setTargetId(null);
    setRelationType("");
    onAdded();
  };

  const candidates = entries
    .filter((e) => e.id !== currentId)
    .map((e) => ({ value: String(e.id), label: e.name }));

  return (
    <Modal opened={opened} onClose={onClose} title="添加关系" size="sm" centered>
      <Stack gap="md">
        <Select
          label="目标条目"
          placeholder="选择..."
          data={candidates}
          value={targetId}
          onChange={setTargetId}
          searchable
        />
        <TextInput
          label="关系类型"
          value={relationType}
          onChange={(e) => setRelationType(e.currentTarget.value)}
          onKeyDown={(e) => e.key === "Enter" && handleAdd()}
          placeholder="例: 影响、合作、出演..."
          list="relation-types"
        />
        <datalist id="relation-types">
          {existingTypes.map((t) => (
            <option key={t} value={t} />
          ))}
        </datalist>
        <Group justify="flex-end" gap="0.5rem">
          <Button variant="subtle" onClick={onClose}>
            取消
          </Button>
          <Button onClick={handleAdd}>添加</Button>
        </Group>
      </Stack>
    </Modal>
  );
}

// ── Entry Detail View ───────────────────────────────────────────

function EntryDetailView({
  entry,
  entries,
  onWikiChange,
  onDeleted,
  onUpdated,
}: {
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
  const [mode, setMode] = useState<"preview" | "edit">("preview");
  const [currentLine, setCurrentLine] = useState(1);
  const contentRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const stats = useMemo(() => {
    const lines = wiki.length === 0 ? 0 : wiki.split("\n").length;
    const chars = wiki.length;
    const headings = wiki.split("\n").filter((l) => /^#{1,6}\s/.test(l)).length;
    return { lines, chars, headings };
  }, [wiki]);

  const updateCursorLine = () => {
    const ta = textareaRef.current;
    if (!ta) return;
    const pos = ta.selectionStart;
    setCurrentLine(wiki.slice(0, pos).split("\n").length);
  };

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

  const titleEl = editingName ? (
    <TextInput
      autoFocus
      value={name}
      onChange={(e) => setName(e.currentTarget.value)}
      onBlur={handleSaveName}
      onKeyDown={(e) => e.key === "Enter" && handleSaveName()}
      variant="unstyled"
      size="xl"
      ta="center"
      styles={{
        input: {
          fontSize: "1.5rem",
          fontWeight: 700,
          textAlign: "center",
          borderBottom: "1px solid var(--color-accent)",
        },
      }}
    />
  ) : (
    <Text
      component="span"
      onDoubleClick={() => setEditingName(true)}
      style={{ cursor: "pointer" }}
      title="双击编辑名称"
    >
      {entry.name}
    </Text>
  );

  return (
    <Flex direction="column" style={{ flex: 1, minHeight: 0 }}>
      {/* Header — title only */}
      <Box
        ta="center"
        pt="1.5rem"
        pb="1rem"
        style={{
          borderBottom: "1px solid var(--color-separator)",
          flexShrink: 0,
        }}
      >
        <Title order={2} fz="1.5rem" fw={700} style={{ letterSpacing: "-0.03em" }}>
          {titleEl}
        </Title>
      </Box>

      {/* Body: left scroll column (content + tags/relations) + right outline.
          `minHeight: 0` on the Flex breaks default min-height:auto so the
          ScrollArea can actually overflow; `h="100%"` on the ScrollArea gives
          it a definite height to compute against. */}
      <Flex
        ref={contentRef}
        style={{ flex: 1, overflow: "hidden", minHeight: 0 }}
      >
        <ScrollArea h="100%" style={{ flex: 1, minHeight: 0 }}>
          <Box py="2rem" pb="3rem">
            <Box w="60%" mx="auto">
              <Group justify="flex-end" gap="0.25rem" mb="1rem">
                <Tabs
                  value={mode}
                  onChange={(v) => v && setMode(v as "preview" | "edit")}
                  variant="default"
                >
                  <Tabs.List>
                    <Tabs.Tab value="preview">预览</Tabs.Tab>
                    <Tabs.Tab value="edit">编辑</Tabs.Tab>
                  </Tabs.List>
                </Tabs>
                <Button
                  variant="subtle"
                  size="compact-xs"
                  color="gray"
                  onClick={handleDelete}
                  ml="auto"
                >
                  删除条目
                </Button>
              </Group>

              {mode === "edit" ? (
                <Textarea
                  ref={textareaRef}
                  value={wiki}
                  onChange={(e) => {
                    setWiki(e.currentTarget.value);
                    requestAnimationFrame(updateCursorLine);
                  }}
                  onBlur={handleSaveWiki}
                  onSelect={updateCursorLine}
                  onKeyUp={updateCursorLine}
                  onClick={updateCursorLine}
                  placeholder="支持 Markdown 格式..."
                  autosize
                  minRows={20}
                  maxRows={40}
                  styles={{
                    input: {
                      fontFamily: "monospace",
                      fontSize: "0.8rem",
                      lineHeight: 1.65,
                    },
                  }}
                />
              ) : (
                <WikiPreview content={wiki} />
              )}

              {/* Metadata: tags + relations, scrolls with content */}
              <Divider my="2rem" />

              <Box mb="1.5rem">
                <Text
                  size="xs"
                  tt="uppercase"
                  c="var(--color-label-quaternary)"
                  fw={600}
                  mb="0.4rem"
                  style={{ letterSpacing: "0.06em", fontSize: "0.7rem" }}
                >
                  标签
                </Text>
                <Group gap="0.3rem" wrap="wrap" align="center">
                  {entry.tags.map((tag) => (
                    <Pill
                      key={tag}
                      withRemoveButton
                      onRemove={() => handleRemoveTag(tag)}
                      size="md"
                    >
                      {tag}
                    </Pill>
                  ))}
                  <TextInput
                    value={newTag}
                    onChange={(e) => setNewTag(e.currentTarget.value)}
                    onKeyDown={(e) => e.key === "Enter" && handleAddTag()}
                    placeholder="+ 标签"
                    size="xs"
                    w={100}
                  />
                </Group>
              </Box>

              <Box>
                <Group gap="0.5rem" mb="0.4rem" align="center">
                  <Text
                    size="xs"
                    tt="uppercase"
                    c="var(--color-label-quaternary)"
                    fw={600}
                    style={{ letterSpacing: "0.06em", fontSize: "0.7rem" }}
                  >
                    关系
                  </Text>
                  <Anchor
                    size="xs"
                    component="button"
                    onClick={() => setShowRelModal(true)}
                  >
                    + 添加
                  </Anchor>
                </Group>
                {entry.relations.length === 0 ? (
                  <Text size="sm" c="var(--color-label-quaternary)" fs="italic">
                    暂无关系
                  </Text>
                ) : (
                  <Stack gap={4}>
                    {entry.relations.map((rel) => (
                      <RelationRow
                        key={rel.id}
                        rel={rel}
                        onRemove={() => handleRemoveRelation(rel.id)}
                      />
                    ))}
                  </Stack>
                )}
              </Box>
            </Box>
          </Box>
        </ScrollArea>

        {wiki && (
          <ScrollArea
            h="100%"
            w={180}
            style={{
              flexShrink: 0,
              borderLeft: "1px solid var(--color-separator)",
            }}
          >
            <Box px="0.75rem" py="1.25rem">
              {mode === "preview" && (
                <Outline content={wiki} containerRef={contentRef} />
              )}
            </Box>
          </ScrollArea>
        )}
      </Flex>

      {/* Status bar — `mt={4}` is the "small gap" between content and statusbar */}
      <Box
        mt={4}
        px="1.25rem"
        py="0.35rem"
        style={{
          flexShrink: 0,
          borderTop: "1px solid var(--color-separator)",
          background: "var(--color-bg-elevated)",
        }}
      >
        <Group gap="lg" wrap="nowrap">
          <Text size="xs" c="var(--color-label-tertiary)">
            {mode === "edit"
              ? `行 ${currentLine}/${stats.lines}`
              : `${stats.lines} 行`}
          </Text>
          <Text size="xs" c="var(--color-label-tertiary)">
            {stats.chars} 字
          </Text>
          <Text size="xs" c="var(--color-label-tertiary)">
            {stats.headings} 个标题
          </Text>
          <Box style={{ flex: 1 }} />
          <Text size="xs" c="var(--color-label-quaternary)">
            {mode === "edit" ? "编辑中" : "预览"}
          </Text>
        </Group>
      </Box>

      <AddRelationModal
        opened={showRelModal}
        currentId={entry.id}
        entries={entries}
        onClose={() => setShowRelModal(false)}
        onAdded={() => {
          setShowRelModal(false);
          onUpdated();
        }}
      />
    </Flex>
  );
}

function RelationRow({ rel, onRemove }: { rel: RelationEntry; onRemove: () => void }) {
  const arrow = rel.direction === "to" ? "→" : "←";
  return (
    <Group gap="0.4rem" py="0.15rem" align="center">
      <Text size="sm" c="var(--color-label-quaternary)">
        {arrow}
      </Text>
      <Text size="sm">{rel.target_name}</Text>
      <Text size="xs" c="var(--color-label-quaternary)">
        ({rel.relation_type})
      </Text>
      <ActionIcon variant="subtle" color="gray" size="xs" onClick={onRemove}>
        ✕
      </ActionIcon>
    </Group>
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
  const refresh = () => {
    refreshRef.current += 1;
    setRefreshKey(refreshRef.current);
  };
  const [refreshKey, setRefreshKey] = useState(0);

  useBackendEvent(BackendEvent.ENTRIES_CHANGED, refresh);

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
    // `flex: 1 + minHeight: 0` instead of `h="100%"`. The parent Main
    // (SpaceShell's <main>) is a flex column, so flex:1 gives this
    // row a DEFINITE column-axis height — without it, h="100%" sees
    // an "indefinite flex stretch height" in its parent and falls
    // back to auto, which makes the row take max(left, right) =
    // content height = thousands of pixels, overflowing everything.
    <Flex style={{ flex: 1, minHeight: 0, overflow: "hidden" }}>
      {/* Left Panel */}
      <Flex
        direction="column"
        w={260}
        style={{
          flexShrink: 0,
          minHeight: 0,
          borderRight: "1px solid var(--color-separator)",
        }}
      >
        <Group justify="space-between" px="0.75rem" pt="0.75rem">
          <Title order={2} fz="0.9rem" fw={700}>
            Wiki
          </Title>
          <Anchor size="sm" component="button" onClick={() => setShowAddModal(true)}>
            + 添加
          </Anchor>
        </Group>

        {allTags.length > 0 && (
          <Group gap={4} px="0.75rem" pt="0.5rem" wrap="wrap">
            {allTags.map((tag) => (
              <Pill
                key={tag}
                size="sm"
                onClick={() =>
                  setSelectedTag(selectedTag === tag ? undefined : tag)
                }
                styles={{
                  root: {
                    cursor: "pointer",
                    background:
                      selectedTag === tag
                        ? "var(--color-accent-soft)"
                        : "var(--color-bg-control)",
                    color:
                      selectedTag === tag
                        ? "var(--color-accent)"
                        : "var(--color-label-secondary)",
                  },
                }}
              >
                {tag}
              </Pill>
            ))}
          </Group>
        )}

        <Box px="0.75rem" py="0.5rem">
          <TextInput
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.currentTarget.value)}
            placeholder="搜索..."
            size="xs"
          />
        </Box>

        <ScrollArea style={{ flex: 1 }}>
          <Stack gap={2} px="0.25rem">
            {entries.map((entry) => (
              <UnstyledButton
                key={entry.id}
                onClick={() => setSelectedId(entry.id)}
                px="0.75rem"
                py="0.5rem"
                style={{
                  background:
                    selectedId === entry.id
                      ? "var(--color-bg-elevated)"
                      : "transparent",
                }}
              >
                <Text fz="0.82rem" fw={500}>
                  {entry.name}
                </Text>
                {entry.tags.length > 0 && (
                  <Group gap={4} mt={2} wrap="wrap">
                    {entry.tags.map((t) => (
                      <Badge
                        key={t}
                        size="xs"
                        variant="light"
                        color="gray"
                      >
                        {t}
                      </Badge>
                    ))}
                  </Group>
                )}
                <Text fz="0.65rem" c="var(--color-label-quaternary)" mt={2}>
                  {entry.updated_at.slice(0, 16)}
                </Text>
              </UnstyledButton>
            ))}
            {entries.length === 0 && (
              <Text ta="center" size="sm" c="var(--color-label-quaternary)" py="2rem">
                {searchQuery || selectedTag ? "未找到匹配条目" : "暂无条目"}
              </Text>
            )}
          </Stack>
        </ScrollArea>
      </Flex>

      {/* Right Panel: detail.
          `display: flex, flexDirection: column, minHeight: 0` gives
          EntryDetailView a definite height via flex layout instead
          of the indefinite-height `align-items: stretch` that
          prevents nested ScrollAreas from knowing their viewport. */}
      <Box
        style={{
          flex: 1,
          overflow: "hidden",
          display: "flex",
          flexDirection: "column",
          minWidth: 0,
          minHeight: 0,
        }}
      >
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
          <Flex align="center" justify="center" style={{ flex: 1 }}>
            <Text size="sm" c="var(--color-label-quaternary)">
              选择或创建一个条目
            </Text>
          </Flex>
        )}
      </Box>

      <AddEntryModal
        opened={showAddModal}
        onClose={() => setShowAddModal(false)}
        onCreated={handleCreated}
      />
    </Flex>
  );
}
