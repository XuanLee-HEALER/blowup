import { useState, useEffect, useRef } from "react";
import {
  ActionIcon,
  Anchor,
  Badge,
  Box,
  Button,
  Flex,
  Group,
  Modal,
  Pill,
  ScrollArea,
  Select,
  Stack,
  Text,
  TextInput,
  Title,
  UnstyledButton,
} from "@mantine/core";
import { kb } from "../lib/tauri";
import type { EntrySummary, EntryDetail, RelationEntry } from "../lib/tauri";
import { WikiDetailView } from "../components/WikiDetailView";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

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
    <Text component="span" onDoubleClick={() => setEditingName(true)} style={{ cursor: "pointer" }} title="双击编辑名称">
      {entry.name}
    </Text>
  );

  const footer = (
    <Stack gap="0.75rem">
      <Box>
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
            <Pill key={tag} withRemoveButton onRemove={() => handleRemoveTag(tag)} size="md">
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
          <Anchor size="xs" component="button" onClick={() => setShowRelModal(true)}>
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
              <RelationRow key={rel.id} rel={rel} onRemove={() => handleRemoveRelation(rel.id)} />
            ))}
          </Stack>
        )}
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
    </Stack>
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
    <Flex h="100%">
      {/* Left Panel */}
      <Flex
        direction="column"
        w={260}
        style={{
          flexShrink: 0,
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

      {/* Right Panel: detail */}
      <Box style={{ flex: 1, overflow: "hidden" }}>
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
          <Flex align="center" justify="center" h="100%">
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
