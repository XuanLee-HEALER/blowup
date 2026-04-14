import { useEffect, useMemo, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  ActionIcon,
  Box,
  Group,
  Image,
  Menu,
  ScrollArea,
  SegmentedControl,
  Select,
  SimpleGrid,
  Stack,
  Tabs,
  Text,
  TextInput,
  UnstyledButton,
} from "@mantine/core";
import {
  IconDots,
  IconLayoutGrid,
  IconList,
  IconSearch,
  IconTrash,
} from "@tabler/icons-react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { SpaceShell } from "../layout/SpaceShell";
import { LibraryDetailTab, type SubConfig } from "../components/contextpanel/LibraryDetailTab";
import { LibraryDarkroomTab } from "../components/contextpanel/LibraryDarkroomTab";
import { config, library, media, player } from "../lib/tauri";
import type {
  IndexEntry,
  SubtitleDisplayConfig,
  SubtitleOverlayConfig,
} from "../lib/tauri";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

type ViewMode = "list" | "grid";
type SortMode = "title" | "year" | "added";

const DEFAULT_SUB_COLORS = ["#FFFFFF", "#FFFF00", "#00FF00", "#00FFFF"];

function sortEntries(entries: IndexEntry[], mode: SortMode): IndexEntry[] {
  const sorted = [...entries];
  switch (mode) {
    case "title":
      sorted.sort((a, b) => a.title.localeCompare(b.title, "zh"));
      break;
    case "year":
      sorted.sort((a, b) => (b.year ?? 0) - (a.year ?? 0));
      break;
    case "added":
      sorted.sort((a, b) => b.added_at.localeCompare(a.added_at));
      break;
  }
  return sorted;
}

export function LibrarySpace() {
  const navigate = useNavigate();
  const { movieId } = useParams<{ movieId?: string }>();

  const [directorMap, setDirectorMap] = useState<Record<string, IndexEntry[]>>({});
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<IndexEntry[] | null>(null);
  const [viewMode, setViewMode] = useState<ViewMode>("list");
  const [sortMode, setSortMode] = useState<SortMode>("title");
  const [enriching, setEnriching] = useState(false);
  const [subConfigs, setSubConfigs] = useState<Map<string, SubConfig>>(new Map());
  const [rootDir, setRootDir] = useState("");

  const refresh = async () => {
    const map = await library.listIndexByDirector();
    setDirectorMap(map);
  };

  useEffect(() => {
    library.listIndexByDirector().then(setDirectorMap);
    config.get().then((c) => {
      setRootDir(c.library.root_dir);
    });
  }, []);

  useBackendEvent(BackendEvent.LIBRARY_CHANGED, refresh);

  // Flat tmdb_id → entry index, rebuilt only when directorMap changes.
  // Lets selectedEntry resolution be a single Map.get instead of an
  // O(n) nested .find scan on every render.
  const entriesById = useMemo(() => {
    const map = new Map<number, IndexEntry>();
    for (const entries of Object.values(directorMap)) {
      for (const entry of entries) map.set(entry.tmdb_id, entry);
    }
    return map;
  }, [directorMap]);

  const selectedEntry: IndexEntry | null = useMemo(() => {
    if (!movieId) return null;
    const id = parseInt(movieId, 10);
    return isNaN(id) ? null : entriesById.get(id) ?? null;
  }, [movieId, entriesById]);

  // Restore the saved overlay configs for whichever entry just became
  // selected. Auto-enrich missing posters; the resulting LIBRARY_CHANGED
  // event re-pulls directorMap which feeds back into selectedEntry, so
  // no local override state is needed.
  useEffect(() => {
    if (!selectedEntry) {
      setSubConfigs((prev) => (prev.size === 0 ? prev : new Map()));
      return;
    }
    const saved = selectedEntry.subtitle_configs ?? {};
    const restored = new Map<string, SubConfig>();
    for (const [name, cfg] of Object.entries(saved)) {
      restored.set(name, { enabled: false, ...cfg });
    }
    setSubConfigs(restored);

    if (!selectedEntry.poster_url && !enriching) {
      setEnriching(true);
      library
        .enrichIndexEntry(selectedEntry.tmdb_id)
        .catch(() => {})
        .finally(() => setEnriching(false));
    }
    // Only depend on tmdb_id so this fires once per selection, not on
    // every directorMap refresh.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selectedEntry?.tmdb_id]);

  const persistSubConfigs = (configs: Map<string, SubConfig>) => {
    if (!selectedEntry) return;
    const toSave: Record<string, SubtitleDisplayConfig> = {};
    for (const [name, cfg] of configs) {
      toSave[name] = {
        y_position: cfg.y_position,
        color: cfg.color,
        font_size: cfg.font_size,
      };
    }
    library.saveSubtitleConfigs(selectedEntry.tmdb_id, toSave).catch(console.error);
  };

  const toggleSub = (file: string) => {
    setSubConfigs((prev) => {
      const next = new Map(prev);
      const existing = next.get(file);
      if (existing) {
        next.set(file, { ...existing, enabled: !existing.enabled });
      } else {
        const idx = [...prev.values()].filter((c) => c.enabled).length;
        next.set(file, {
          enabled: true,
          y_position: idx === 0 ? 0.05 : 0.95,
          color: DEFAULT_SUB_COLORS[idx % DEFAULT_SUB_COLORS.length],
          font_size: idx === 0 ? 48 : 36,
        });
      }
      persistSubConfigs(next);
      return next;
    });
  };

  const updateSubConfig = (file: string, patch: Partial<SubConfig>) => {
    setSubConfigs((prev) => {
      const next = new Map(prev);
      const existing = next.get(file);
      if (existing) next.set(file, { ...existing, ...patch });
      persistSubConfigs(next);
      return next;
    });
  };

  const handlePlay = async (file: string) => {
    if (!selectedEntry) return;
    const fullPath = `${rootDir}/${selectedEntry.path}/${file}`;
    await media.openInPlayer(fullPath);
    const enabledSubs = [...subConfigs.entries()].filter(([, c]) => c.enabled);
    if (enabledSubs.length > 0) {
      const configs: SubtitleOverlayConfig[] = enabledSubs.map(([name, c]) => ({
        path: `${rootDir}/${selectedEntry.path}/${name}`,
        y_position: c.y_position,
        color: c.color,
        font_size: c.font_size,
      }));
      setTimeout(() => {
        player.loadOverlaySubs(configs).catch(console.error);
      }, 500);
    }
  };

  const handleDeleteResource = async (file: string) => {
    if (!selectedEntry) return;
    if (!confirm(`确定要删除文件 "${file}" 吗？此操作不可撤销。`)) return;
    try {
      const fullPath = `${rootDir}/${selectedEntry.path}/${file}`;
      await library.deleteLibraryResource(fullPath);
      await library.refreshIndexEntry(selectedEntry.tmdb_id);
      await refresh();
    } catch (e) {
      alert(`删除失败: ${e}`);
    }
  };

  const handleRefreshDetail = async () => {
    if (!selectedEntry) return;
    setEnriching(true);
    library
      .enrichIndexEntry(selectedEntry.tmdb_id, true)
      .catch((e) => alert(`刷新失败: ${e}`))
      .finally(() => setEnriching(false));
  };

  const handleDeleteFilm = async (entry: IndexEntry) => {
    if (!confirm(`确定要删除电影 "${entry.title}" 及其所有文件吗？此操作不可撤销。`)) return;
    try {
      await library.deleteFilmDirectory(entry.tmdb_id);
      if (selectedEntry?.tmdb_id === entry.tmdb_id) {
        navigate("/library");
      }
      await refresh();
    } catch (e) {
      alert(`删除失败: ${e}`);
    }
  };

  const handleRebuild = async () => {
    await library.rebuildIndex();
    await refresh();
  };

  const doSearch = async () => {
    if (!searchQuery.trim()) {
      setSearchResults(null);
      return;
    }
    const results = await library.searchIndex(searchQuery.trim());
    setSearchResults(results);
  };

  const directors = useMemo(() => Object.keys(directorMap).sort(), [directorMap]);

  const onSelectEntry = (entry: IndexEntry) => navigate(`/library/${entry.tmdb_id}`);
  const onCloseDetail = () => navigate("/library");

  // ── Toolbar slots ──
  const toolbarLeft = (
    <>
      <TextInput
        size="xs"
        leftSection={<IconSearch size={14} />}
        placeholder="搜索标题或导演…"
        value={searchQuery}
        onChange={(e) => setSearchQuery(e.currentTarget.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && !e.nativeEvent.isComposing) doSearch();
        }}
        style={{ flex: 1, maxWidth: 280 }}
      />
      {searchResults && (
        <UnstyledButton
          onClick={() => {
            setSearchResults(null);
            setSearchQuery("");
          }}
          style={{ fontSize: "0.7rem", color: "var(--color-label-tertiary)" }}
        >
          清除
        </UnstyledButton>
      )}
    </>
  );

  const toolbarRight = (
    <>
      <SegmentedControl
        size="xs"
        value={viewMode}
        onChange={(v) => setViewMode(v as ViewMode)}
        data={[
          { label: <IconList size={14} />, value: "list" },
          { label: <IconLayoutGrid size={14} />, value: "grid" },
        ]}
      />
      <Select
        size="xs"
        w={110}
        value={sortMode}
        onChange={(v) => v && setSortMode(v as SortMode)}
        data={[
          { value: "title", label: "按标题" },
          { value: "year", label: "按年份" },
          { value: "added", label: "按添加" },
        ]}
      />
      <Menu position="bottom-end" shadow="md" withinPortal>
        <Menu.Target>
          <ActionIcon variant="default" size="sm">
            <IconDots size={14} />
          </ActionIcon>
        </Menu.Target>
        <Menu.Dropdown>
          <Menu.Item onClick={handleRebuild}>重建索引</Menu.Item>
        </Menu.Dropdown>
      </Menu>
    </>
  );

  // ── Main area ──
  const renderRow = (entry: IndexEntry) => {
    const isSelected = selectedEntry?.tmdb_id === entry.tmdb_id;
    return (
      <UnstyledButton
        key={entry.tmdb_id}
        onClick={() => onSelectEntry(entry)}
        onContextMenu={(e) => {
          e.preventDefault();
          handleDeleteFilm(entry);
        }}
        w="100%"
        px="1rem"
        py="0.5rem"
        style={{
          background: isSelected ? "var(--color-bg-elevated)" : "transparent",
          borderBottom: "0.5px solid var(--color-separator)",
        }}
      >
        <Group gap="md" wrap="nowrap">
          <Text fz="0.85rem" fw={500} style={{ flex: 1 }} truncate>
            {entry.title}
          </Text>
          <Text fz="0.72rem" c="var(--color-label-tertiary)" w={50} ta="right">
            {entry.year ?? ""}
          </Text>
          <Text fz="0.68rem" c="var(--color-label-quaternary)" w={80} ta="right">
            {entry.files.length} 文件
          </Text>
        </Group>
      </UnstyledButton>
    );
  };

  // Memoized to avoid re-sorting the entire library on every keystroke /
  // hover / unrelated state change. groupedSorted holds the per-director
  // sorted slices for the list view; flatList is the same data flattened
  // for the grid view and the search-results path.
  const groupedSorted = useMemo(
    () => Object.fromEntries(directors.map((dir) => [dir, sortEntries(directorMap[dir], sortMode)])),
    [directors, directorMap, sortMode]
  );
  const flatList = useMemo(
    () =>
      searchResults
        ? sortEntries(searchResults, sortMode)
        : directors.flatMap((dir) => groupedSorted[dir]),
    [searchResults, sortMode, directors, groupedSorted]
  );

  const main = (
    <ScrollArea style={{ flex: 1 }}>
      {viewMode === "list" ? (
        searchResults ? (
          <Stack gap={0}>{flatList.map(renderRow)}</Stack>
        ) : (
          directors.map((dir) => (
            <Box key={dir}>
              <Box
                px="1rem"
                py="0.4rem"
                style={{
                  background: "var(--color-bg-secondary)",
                  borderBottom: "0.5px solid var(--color-separator)",
                }}
              >
                <Group justify="space-between">
                  <Text fz="0.78rem" fw={600} c="var(--color-label-secondary)">
                    {dir}
                  </Text>
                  <Text fz="0.68rem" c="var(--color-label-quaternary)">
                    {directorMap[dir].length}
                  </Text>
                </Group>
              </Box>
              {groupedSorted[dir].map(renderRow)}
            </Box>
          ))
        )
      ) : (
        <Box p="1rem">
          <SimpleGrid cols={{ base: 2, sm: 3, md: 4, lg: 5 }} spacing="md">
            {flatList.map((entry) => {
              const isSelected = selectedEntry?.tmdb_id === entry.tmdb_id;
              return (
                <UnstyledButton
                  key={entry.tmdb_id}
                  onClick={() => onSelectEntry(entry)}
                  onContextMenu={(e) => {
                    e.preventDefault();
                    handleDeleteFilm(entry);
                  }}
                  style={{
                    border: isSelected
                      ? "2px solid var(--color-accent)"
                      : "1px solid var(--color-separator)",
                    overflow: "hidden",
                    background: "var(--color-bg-secondary)",
                  }}
                >
                  {entry.poster_url ? (
                    <Image
                      src={
                        entry.poster_url.startsWith("http")
                          ? entry.poster_url
                          : convertFileSrc(entry.poster_url)
                      }
                      alt={entry.title}
                      w="100%"
                      h="auto"
                      fit="contain"
                    />
                  ) : (
                    <Box
                      style={{
                        aspectRatio: "2 / 3",
                        display: "flex",
                        alignItems: "center",
                        justifyContent: "center",
                      }}
                    >
                      <IconTrash
                        size={20}
                        style={{ opacity: 0.2, color: "var(--color-label-quaternary)" }}
                      />
                    </Box>
                  )}
                  <Box p="0.5rem">
                    <Text fz="0.75rem" fw={500} truncate>
                      {entry.title}
                    </Text>
                    <Text fz="0.65rem" c="var(--color-label-tertiary)">
                      {entry.year ?? "—"}
                    </Text>
                  </Box>
                </UnstyledButton>
              );
            })}
          </SimpleGrid>
        </Box>
      )}
      {!searchResults && directors.length === 0 && (
        <Text px="1rem" py="2rem" size="sm" c="var(--color-label-tertiary)" ta="center">
          暂无影片。通过发现页搜索并下载电影后会自动添加到此处。
        </Text>
      )}
    </ScrollArea>
  );

  // ── Context panel ──
  const context = selectedEntry ? (
    <Tabs defaultValue="detail" style={{ display: "flex", flexDirection: "column", flex: 1 }}>
      <Tabs.List>
        <Tabs.Tab value="detail">详情</Tabs.Tab>
        <Tabs.Tab value="darkroom">暗房</Tabs.Tab>
      </Tabs.List>
      <Tabs.Panel value="detail" style={{ flex: 1, display: "flex", flexDirection: "column" }}>
        <LibraryDetailTab
          entry={selectedEntry}
          enriching={enriching}
          subConfigs={subConfigs}
          onPlay={handlePlay}
          onDeleteResource={handleDeleteResource}
          onRefresh={handleRefreshDetail}
          onToggleSub={toggleSub}
          onUpdateSubConfig={updateSubConfig}
        />
      </Tabs.Panel>
      <Tabs.Panel value="darkroom" style={{ flex: 1, display: "flex", flexDirection: "column" }}>
        <LibraryDarkroomTab entry={selectedEntry} rootDir={rootDir} />
      </Tabs.Panel>
    </Tabs>
  ) : null;

  return (
    <SpaceShell
      toolbarLeft={toolbarLeft}
      toolbarRight={toolbarRight}
      main={main}
      context={context}
      contextOpened={!!selectedEntry}
      onContextClose={onCloseDetail}
    />
  );
}
