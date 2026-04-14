import { useState, useEffect, useCallback, useRef } from "react";
import {
  ActionIcon,
  Box,
  Button,
  Checkbox,
  ColorInput,
  Flex,
  Group,
  Image,
  Menu,
  NumberInput,
  Paper,
  ScrollArea,
  Slider,
  Stack,
  Text,
  TextInput,
  Title,
  UnstyledButton,
} from "@mantine/core";
import { library, media, config, player } from "../lib/tauri";
import type {
  IndexEntry,
  SubtitleOverlayConfig,
  SubtitleDisplayConfig,
} from "../lib/tauri";
import { convertFileSrc } from "@tauri-apps/api/core";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

const VIDEO_EXTS = ["mp4", "mkv", "avi", "mov", "ts", "flv", "wmv", "webm", "m4v"];
const SUB_EXTS = ["srt", "ass", "sub", "idx"];
const CREDIT_ORDER = ["导演", "主演", "编剧", "摄影", "配乐", "剪辑", "制片"];
const getExt = (f: string) => f.split(".").pop()?.toLowerCase() ?? "";

interface SubConfig {
  enabled: boolean;
  y_position: number;
  color: string;
  font_size: number;
}

const DEFAULT_SUB_COLORS = ["#FFFFFF", "#FFFF00", "#00FF00", "#00FFFF"];

export default function Library() {
  const [directorMap, setDirectorMap] = useState<Record<string, IndexEntry[]>>({});
  const [selectedDirector, setSelectedDirector] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<IndexEntry | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<IndexEntry[] | null>(null);
  const [enriching, setEnriching] = useState(false);
  const [subConfigs, setSubConfigs] = useState<Map<string, SubConfig>>(new Map());
  const [contextMenu, setContextMenu] = useState<{
    x: number;
    y: number;
    entry: IndexEntry;
  } | null>(null);
  const rootDir = useRef("");

  const refresh = useCallback(async () => {
    const map = await library.listIndexByDirector();
    setDirectorMap(map);
  }, []);

  useEffect(() => {
    library.listIndexByDirector().then(setDirectorMap);
    config.get().then((c) => {
      rootDir.current = c.library.root_dir;
    });
  }, []);

  useBackendEvent(BackendEvent.LIBRARY_CHANGED, refresh);

  const selectEntry = useCallback((entry: IndexEntry) => {
    setSelectedEntry(entry);
    const saved = entry.subtitle_configs ?? {};
    const restored = new Map<string, SubConfig>();
    for (const [name, cfg] of Object.entries(saved)) {
      restored.set(name, { enabled: false, ...cfg });
    }
    setSubConfigs(restored);
    if (!entry.poster_url) {
      setEnriching(true);
      library
        .enrichIndexEntry(entry.tmdb_id)
        .then((enriched) => {
          setSelectedEntry(enriched);
        })
        .catch(() => {})
        .finally(() => setEnriching(false));
    }
  }, []);

  const doSearch = async () => {
    if (!searchQuery.trim()) {
      setSearchResults(null);
      return;
    }
    const results = await library.searchIndex(searchQuery.trim());
    setSearchResults(results);
  };

  const handlePlay = async (entry: IndexEntry, file: string) => {
    const root = rootDir.current;
    const fullPath = `${root}/${entry.path}/${file}`;
    await media.openInPlayer(fullPath);
    const enabledSubs = [...subConfigs.entries()].filter(([, c]) => c.enabled);
    if (enabledSubs.length > 0) {
      const configs: SubtitleOverlayConfig[] = enabledSubs.map(([name, c]) => ({
        path: `${root}/${entry.path}/${name}`,
        y_position: c.y_position,
        color: c.color,
        font_size: c.font_size,
      }));
      setTimeout(() => {
        player.loadOverlaySubs(configs).catch(console.error);
      }, 500);
    }
  };

  const handleDeleteResource = async (entry: IndexEntry, file: string) => {
    if (!confirm(`确定要删除文件 "${file}" 吗？此操作不可撤销。`)) return;
    try {
      const fullPath = `${rootDir.current}/${entry.path}/${file}`;
      await library.deleteLibraryResource(fullPath);
      await library.refreshIndexEntry(entry.tmdb_id);
      const map = await library.listIndexByDirector();
      setDirectorMap(map);
      for (const entries of Object.values(map)) {
        const updated = entries.find((e) => e.tmdb_id === entry.tmdb_id);
        if (updated) {
          setSelectedEntry(updated);
          return;
        }
      }
      setSelectedEntry(null);
    } catch (e) {
      alert(`删除失败: ${e}`);
    }
  };

  const handleDeleteFilm = async (entry: IndexEntry) => {
    setContextMenu(null);
    if (!confirm(`确定要删除电影 "${entry.title}" 及其所有文件吗？此操作不可撤销。`)) return;
    try {
      await library.deleteFilmDirectory(entry.tmdb_id);
      await refresh();
      if (selectedEntry?.tmdb_id === entry.tmdb_id) {
        setSelectedEntry(null);
      }
    } catch (e) {
      alert(`删除失败: ${e}`);
    }
  };

  const handleRefreshDetail = async () => {
    if (!selectedEntry) return;
    setEnriching(true);
    library
      .enrichIndexEntry(selectedEntry.tmdb_id, true)
      .then((enriched) => setSelectedEntry(enriched))
      .catch((e) => alert(`刷新失败: ${e}`))
      .finally(() => setEnriching(false));
  };

  const handleRebuild = async () => {
    await library.rebuildIndex();
    await refresh();
  };

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

  const directors = Object.keys(directorMap).sort();
  const videoFiles =
    selectedEntry?.files.filter((f) => VIDEO_EXTS.includes(getExt(f))) ?? [];
  const subtitleFiles =
    selectedEntry?.files.filter((f) => SUB_EXTS.includes(getExt(f))) ?? [];

  const credits = selectedEntry?.credits ?? {};

  const onEntryContextMenu = (e: React.MouseEvent, entry: IndexEntry) => {
    e.preventDefault();
    setContextMenu({ x: e.clientX, y: e.clientY, entry });
  };

  return (
    <Flex h="100%" style={{ overflow: "hidden" }}>
      {/* Left: director list + search */}
      <Flex
        direction="column"
        w={240}
        style={{
          flexShrink: 0,
          borderRight: "1px solid var(--color-separator)",
          overflow: "hidden",
        }}
      >
        <Box px="1rem" pt="1.4rem">
          <Title order={1} mb="0.8rem" fz="1.3rem" fw={700} style={{ letterSpacing: "-0.035em" }}>
            影片
          </Title>
          <TextInput
            leftSection={<span>⌕</span>}
            placeholder="搜索标题或导演…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.nativeEvent.isComposing) doSearch();
            }}
            mb="0.5rem"
            size="xs"
          />
          <Group gap="0.3rem" mb="0.6rem">
            <Button variant="default" size="compact-xs" onClick={handleRebuild}>
              重建索引
            </Button>
            {searchResults && (
              <Button
                variant="default"
                size="compact-xs"
                onClick={() => {
                  setSearchResults(null);
                  setSearchQuery("");
                }}
              >
                清除搜索
              </Button>
            )}
          </Group>
        </Box>

        <Box style={{ height: 1, background: "var(--color-separator)", margin: "0 1rem" }} />

        <ScrollArea style={{ flex: 1, userSelect: "none", WebkitUserSelect: "none" }}>
          <Box py="0.5rem">
            {searchResults
              ? searchResults.map((e) => (
                  <UnstyledButton
                    key={e.tmdb_id}
                    onClick={() => selectEntry(e)}
                    onContextMenu={(ev) => onEntryContextMenu(ev, e)}
                    px="1rem"
                    py="0.5rem"
                    w="100%"
                    style={{
                      background:
                        selectedEntry?.tmdb_id === e.tmdb_id
                          ? "var(--color-bg-elevated)"
                          : "transparent",
                    }}
                  >
                    <Text fz="0.82rem" fw={500}>
                      {e.title}
                    </Text>
                    <Text fz="0.7rem" c="var(--color-label-tertiary)">
                      {e.director_display}
                      {e.year ? ` · ${e.year}` : ""}
                    </Text>
                  </UnstyledButton>
                ))
              : directors.map((dir) => (
                  <Box key={dir}>
                    <UnstyledButton
                      onClick={() => {
                        setSelectedDirector(selectedDirector === dir ? null : dir);
                        setSelectedEntry(null);
                        setSubConfigs(new Map());
                      }}
                      w="100%"
                      px="1rem"
                      py="0.45rem"
                      style={{
                        background:
                          selectedDirector === dir ? "var(--color-bg-elevated)" : "transparent",
                      }}
                    >
                      <Group justify="space-between">
                        <Text fz="0.82rem" fw={600}>
                          {dir}
                        </Text>
                        <Text fz="0.7rem" c="var(--color-label-quaternary)">
                          {directorMap[dir].length}
                        </Text>
                      </Group>
                    </UnstyledButton>
                    {selectedDirector === dir &&
                      directorMap[dir].map((e) => (
                        <UnstyledButton
                          key={e.tmdb_id}
                          onClick={() => selectEntry(e)}
                          onContextMenu={(ev) => onEntryContextMenu(ev, e)}
                          w="100%"
                          py="0.35rem"
                          pl="1.8rem"
                          pr="1rem"
                          style={{
                            background:
                              selectedEntry?.tmdb_id === e.tmdb_id
                                ? "var(--color-hover)"
                                : "transparent",
                          }}
                        >
                          <Text fz="0.78rem" component="span">
                            {e.title}
                          </Text>
                          <Text
                            fz="0.68rem"
                            c="var(--color-label-quaternary)"
                            component="span"
                            ml="0.4rem"
                          >
                            {e.year ?? ""}
                          </Text>
                        </UnstyledButton>
                      ))}
                  </Box>
                ))}
            {!searchResults && directors.length === 0 && (
              <Text px="1rem" size="sm" c="var(--color-label-tertiary)">
                暂无影片。通过搜索页下载电影后会自动添加到此处。
              </Text>
            )}
          </Box>
        </ScrollArea>
      </Flex>

      {/* Right: detail panel */}
      <ScrollArea style={{ flex: 1 }}>
        <Box px="1.75rem" py="1.4rem">
          {selectedEntry ? (
            <Stack gap="md">
              <Group justify="flex-end">
                <ActionIcon
                  variant="default"
                  disabled={enriching}
                  loading={enriching}
                  onClick={handleRefreshDetail}
                >
                  ↻
                </ActionIcon>
              </Group>

              <Group gap="1.2rem" align="flex-start" wrap="nowrap">
                {selectedEntry.poster_url ? (
                  <Image
                    src={
                      selectedEntry.poster_url.startsWith("http")
                        ? selectedEntry.poster_url
                        : convertFileSrc(selectedEntry.poster_url)
                    }
                    alt=""
                    w={140}
                    radius="md"
                    fit="cover"
                    bg="var(--color-bg-secondary)"
                    style={{ flexShrink: 0 }}
                  />
                ) : (
                  <Box
                    w={140}
                    h={200}
                    bg="var(--color-bg-secondary)"
                    style={{
                      borderRadius: 8,
                      flexShrink: 0,
                      display: "flex",
                      alignItems: "center",
                      justifyContent: "center",
                    }}
                  >
                    <Text size="xs" c="var(--color-label-quaternary)">
                      {enriching ? "加载中…" : "无海报"}
                    </Text>
                  </Box>
                )}

                <Box style={{ flex: 1, minWidth: 0 }}>
                  <Title order={2} fz="1.2rem" fw={700} mb={4} style={{ letterSpacing: "-0.02em" }}>
                    {selectedEntry.title}
                  </Title>
                  {selectedEntry.original_title &&
                    selectedEntry.original_title !== selectedEntry.title && (
                      <Text fz="0.78rem" c="var(--color-label-tertiary)" mb="0.3rem">
                        {selectedEntry.original_title}
                      </Text>
                    )}
                  <Text fz="0.82rem" c="var(--color-label-secondary)" mb="0.6rem">
                    {selectedEntry.year}
                    {selectedEntry.year && selectedEntry.rating != null && " · "}
                    {selectedEntry.rating != null && (
                      <Text component="span">★ {selectedEntry.rating.toFixed(1)}</Text>
                    )}
                  </Text>

                  {Object.keys(credits).length > 0
                    ? CREDIT_ORDER.filter((role) => credits[role]?.length).map((role) => (
                        <Text key={role} fz="0.82rem" mb="0.3rem">
                          <Text component="span" c="var(--color-label-tertiary)">
                            {role}:{" "}
                          </Text>
                          {credits[role].join(", ")}
                        </Text>
                      ))
                    : selectedEntry.director_display && (
                        <Text fz="0.82rem" mb="0.3rem">
                          <Text component="span" c="var(--color-label-tertiary)">
                            导演:{" "}
                          </Text>
                          {selectedEntry.director_display}
                        </Text>
                      )}

                  {selectedEntry.genres.length > 0 && (
                    <Text fz="0.78rem" c="var(--color-label-tertiary)" mt="0.4rem">
                      {selectedEntry.genres.join(" / ")}
                    </Text>
                  )}
                </Box>
              </Group>

              {/* Video files */}
              <Box>
                <SectionHeader>视频文件</SectionHeader>
                {videoFiles.length === 0 ? (
                  <Text size="sm" c="var(--color-label-tertiary)">
                    无视频文件
                  </Text>
                ) : (
                  <Stack gap="0.3rem">
                    {videoFiles.map((file) => (
                      <Paper
                        key={file}
                        withBorder
                        px="0.75rem"
                        py="0.5rem"
                        bg="var(--color-bg-secondary)"
                        style={{ borderColor: "var(--color-separator)" }}
                      >
                        <Group justify="space-between" wrap="nowrap">
                          <Text fz="0.82rem" truncate style={{ flex: 1 }}>
                            {file}
                          </Text>
                          <Group gap="0.3rem" wrap="nowrap">
                            <Button
                              size="compact-xs"
                              onClick={() => handlePlay(selectedEntry, file)}
                            >
                              ▶ 播放
                            </Button>
                            <ActionIcon
                              variant="subtle"
                              color="danger"
                              onClick={() => handleDeleteResource(selectedEntry, file)}
                            >
                              ✕
                            </ActionIcon>
                          </Group>
                        </Group>
                      </Paper>
                    ))}
                  </Stack>
                )}
              </Box>

              {/* Subtitle library */}
              <Box>
                <SectionHeader>字幕文件</SectionHeader>
                {subtitleFiles.length === 0 ? (
                  <Text size="sm" c="var(--color-label-tertiary)">
                    目录中未发现字幕文件
                  </Text>
                ) : (
                  <Stack gap="0.4rem">
                    {subtitleFiles.map((file) => {
                      const cfg = subConfigs.get(file);
                      const enabled = cfg?.enabled ?? false;
                      return (
                        <Paper
                          key={file}
                          withBorder
                          px="0.75rem"
                          py="0.5rem"
                          bg="var(--color-bg-secondary)"
                          style={{
                            borderColor: enabled
                              ? "var(--color-accent)"
                              : "var(--color-separator)",
                          }}
                        >
                          <Stack gap="0.4rem">
                            <Checkbox
                              checked={enabled}
                              onChange={() => toggleSub(file)}
                              label={
                                <Text fz="0.82rem" truncate>
                                  {file}
                                </Text>
                              }
                              styles={{ label: { flex: 1, minWidth: 0, overflow: "hidden" } }}
                            />
                            {enabled && cfg && (
                              <Group gap="0.6rem" pl="1.5rem" wrap="nowrap">
                                <Group gap="0.25rem" wrap="nowrap">
                                  <Text fz="0.72rem" c="var(--color-label-tertiary)">
                                    颜色
                                  </Text>
                                  <ColorInput
                                    size="xs"
                                    value={cfg.color}
                                    onChange={(c) => updateSubConfig(file, { color: c })}
                                    w={120}
                                    withEyeDropper={false}
                                    format="hex"
                                  />
                                </Group>
                                <Group gap="0.25rem" wrap="nowrap">
                                  <Text fz="0.72rem" c="var(--color-label-tertiary)">
                                    字号
                                  </Text>
                                  <NumberInput
                                    size="xs"
                                    w={70}
                                    min={16}
                                    max={80}
                                    step={2}
                                    value={cfg.font_size}
                                    onChange={(v) =>
                                      updateSubConfig(file, {
                                        font_size: typeof v === "number" ? v : 48,
                                      })
                                    }
                                    hideControls
                                  />
                                </Group>
                                <Group gap="0.25rem" wrap="nowrap" style={{ flex: 1 }}>
                                  <Text fz="0.72rem" c="var(--color-label-tertiary)">
                                    位置
                                  </Text>
                                  <Slider
                                    style={{ flex: 1 }}
                                    min={0}
                                    max={100}
                                    value={Math.round(cfg.y_position * 100)}
                                    onChange={(v) =>
                                      updateSubConfig(file, { y_position: v / 100 })
                                    }
                                    label={(v) => `${v}%`}
                                  />
                                </Group>
                              </Group>
                            )}
                          </Stack>
                        </Paper>
                      );
                    })}
                    {[...subConfigs.values()].some((c) => c.enabled) && (
                      <Text fz="0.72rem" c="var(--color-label-tertiary)">
                        已选 {[...subConfigs.values()].filter((c) => c.enabled).length} 条字幕，播放视频时将自动叠加显示
                      </Text>
                    )}
                  </Stack>
                )}
              </Box>

              <Text fz="0.7rem" c="var(--color-label-quaternary)">
                路径: {selectedEntry.path}
                <br />
                添加时间: {new Date(selectedEntry.added_at).toLocaleString("zh-CN")}
              </Text>
            </Stack>
          ) : (
            <Flex
              align="center"
              justify="center"
              h="100%"
              style={{ minHeight: "60vh" }}
            >
              <Text size="sm" c="var(--color-label-tertiary)">
                {directors.length > 0 ? "选择一部电影查看详情" : ""}
              </Text>
            </Flex>
          )}
        </Box>
      </ScrollArea>

      {/* Context menu */}
      {contextMenu && (
        <Menu
          opened
          onClose={() => setContextMenu(null)}
          position="bottom-start"
          withinPortal
          shadow="md"
        >
          <Menu.Target>
            <Box
              style={{
                position: "fixed",
                left: contextMenu.x,
                top: contextMenu.y,
                width: 1,
                height: 1,
              }}
            />
          </Menu.Target>
          <Menu.Dropdown>
            <Menu.Item color="danger" onClick={() => handleDeleteFilm(contextMenu.entry)}>
              删除电影
            </Menu.Item>
          </Menu.Dropdown>
        </Menu>
      )}
    </Flex>
  );
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <Text
      size="xs"
      tt="uppercase"
      c="var(--color-label-quaternary)"
      mb="0.5rem"
      style={{ letterSpacing: "0.08em", fontSize: "0.7rem" }}
    >
      {children}
    </Text>
  );
}
