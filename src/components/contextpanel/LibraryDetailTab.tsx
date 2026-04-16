import {
  ActionIcon,
  Box,
  Checkbox,
  ColorInput,
  Group,
  Image,
  NumberInput,
  Paper,
  ScrollArea,
  Slider,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { convertFileSrc } from "@tauri-apps/api/core";
import type { IndexEntry } from "../../lib/tauri";

import { isSubtitleFile, isVideoFile } from "../../lib/mediaExts";

const CREDIT_ORDER = ["导演", "主演", "编剧", "摄影", "配乐", "剪辑", "制片"];

export interface SubConfig {
  enabled: boolean;
  y_position: number;
  color: string;
  font_size: number;
}

interface LibraryDetailTabProps {
  entry: IndexEntry;
  enriching: boolean;
  subConfigs: Map<string, SubConfig>;
  onPlay: (file: string) => void;
  onDeleteResource: (file: string) => void;
  onRefresh: () => void;
  onSyncToWiki: () => void;
  onToggleSub: (file: string) => void;
  onUpdateSubConfig: (file: string, patch: Partial<SubConfig>) => void;
}

/** Library context panel — "details" tab. Fully controlled by the
 *  parent space; no internal state. */
export function LibraryDetailTab({
  entry,
  enriching,
  subConfigs,
  onPlay,
  onDeleteResource,
  onRefresh,
  onSyncToWiki,
  onToggleSub,
  onUpdateSubConfig,
}: LibraryDetailTabProps) {
  const videoFiles = entry.files.filter(isVideoFile);
  const subtitleFiles = entry.files.filter(isSubtitleFile);
  const credits = entry.credits ?? {};

  return (
    <ScrollArea style={{ flex: 1 }}>
      <Box p="1rem">
        <Stack gap="md">
          <Group justify="flex-end" gap="xs">
            <ActionIcon
              variant="default"
              size="sm"
              onClick={onSyncToWiki}
              title="同步到知识库"
            >
              📖
            </ActionIcon>
            <ActionIcon
              variant="default"
              size="sm"
              disabled={enriching}
              loading={enriching}
              onClick={onRefresh}
              title="刷新元数据"
            >
              ↻
            </ActionIcon>
          </Group>

          <Group gap="0.75rem" align="flex-start" wrap="nowrap">
            {entry.poster_url ? (
              <Image
                src={
                  entry.poster_url.startsWith("http")
                    ? entry.poster_url
                    : convertFileSrc(entry.poster_url)
                }
                alt=""
                w={120}
                fit="cover"
                bg="var(--color-bg-secondary)"
                style={{ flexShrink: 0 }}
              />
            ) : (
              <Box
                w={120}
                h={170}
                bg="var(--color-bg-secondary)"
                style={{
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
              <Title order={2} fz="1.05rem" fw={700} mb={4} style={{ letterSpacing: "-0.02em" }}>
                {entry.title}
              </Title>
              {entry.original_title && entry.original_title !== entry.title && (
                <Text fz="0.72rem" c="var(--color-label-tertiary)" mb="0.3rem" truncate>
                  {entry.original_title}
                </Text>
              )}
              <Text fz="0.78rem" c="var(--color-label-secondary)" mb="0.5rem">
                {entry.year}
                {entry.year && entry.rating != null && " · "}
                {entry.rating != null && (
                  <Text component="span">★ {entry.rating.toFixed(1)}</Text>
                )}
              </Text>
            </Box>
          </Group>

          {/* Credits */}
          {Object.keys(credits).length > 0 ? (
            <Stack gap={2}>
              {CREDIT_ORDER.filter((role) => credits[role]?.length).map((role) => (
                <Text key={role} fz="0.78rem">
                  <Text component="span" c="var(--color-label-tertiary)">
                    {role}:{" "}
                  </Text>
                  {credits[role].join(", ")}
                </Text>
              ))}
            </Stack>
          ) : (
            entry.director_display && (
              <Text fz="0.78rem">
                <Text component="span" c="var(--color-label-tertiary)">
                  导演:{" "}
                </Text>
                {entry.director_display}
              </Text>
            )
          )}

          {entry.genres.length > 0 && (
            <Text fz="0.72rem" c="var(--color-label-tertiary)">
              {entry.genres.join(" / ")}
            </Text>
          )}

          {/* Video files */}
          <Box>
            <SectionHeader>视频文件</SectionHeader>
            {videoFiles.length === 0 ? (
              <Text size="xs" c="var(--color-label-tertiary)">
                无视频文件
              </Text>
            ) : (
              <Stack gap="0.3rem">
                {videoFiles.map((file) => (
                  <Paper
                    key={file}
                    withBorder
                    px="0.6rem"
                    py="0.4rem"
                    bg="var(--color-bg-secondary)"
                    style={{ borderColor: "var(--color-separator)" }}
                  >
                    <Group justify="space-between" wrap="nowrap" gap="0.3rem">
                      <Text fz="0.75rem" truncate style={{ flex: 1 }} title={file}>
                        {file}
                      </Text>
                      <Group gap={2} wrap="nowrap">
                        <ActionIcon
                          size="sm"
                          variant="filled"
                          color="accent"
                          onClick={() => onPlay(file)}
                          title="播放"
                        >
                          ▶
                        </ActionIcon>
                        <ActionIcon
                          size="sm"
                          variant="subtle"
                          color="danger"
                          onClick={() => onDeleteResource(file)}
                          title="删除"
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

          {/* Subtitle files + per-file overlay config */}
          <Box>
            <SectionHeader>字幕文件</SectionHeader>
            {subtitleFiles.length === 0 ? (
              <Text size="xs" c="var(--color-label-tertiary)">
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
                      px="0.6rem"
                      py="0.4rem"
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
                          onChange={() => onToggleSub(file)}
                          label={file}
                          title={file}
                          styles={{
                            root: { overflow: "hidden" },
                            body: { overflow: "hidden" },
                            labelWrapper: { overflow: "hidden", minWidth: 0 },
                            label: {
                              overflow: "hidden",
                              textOverflow: "ellipsis",
                              whiteSpace: "nowrap",
                              fontSize: "0.75rem",
                            },
                          }}
                        />
                        {enabled && cfg && (
                          <Stack gap="0.3rem" pl="1.5rem">
                            <Group gap="0.3rem" wrap="nowrap">
                              <Text fz="0.68rem" c="var(--color-label-tertiary)" w={36}>
                                颜色
                              </Text>
                              <ColorInput
                                size="xs"
                                value={cfg.color}
                                onChange={(c) => onUpdateSubConfig(file, { color: c })}
                                style={{ flex: 1 }}
                                withEyeDropper={false}
                                format="hex"
                              />
                            </Group>
                            <Group gap="0.3rem" wrap="nowrap">
                              <Text fz="0.68rem" c="var(--color-label-tertiary)" w={36}>
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
                                  onUpdateSubConfig(file, {
                                    font_size: typeof v === "number" ? v : 48,
                                  })
                                }
                                hideControls
                              />
                            </Group>
                            <Group gap="0.3rem" wrap="nowrap">
                              <Text fz="0.68rem" c="var(--color-label-tertiary)" w={36}>
                                位置
                              </Text>
                              <Slider
                                style={{ flex: 1 }}
                                min={0}
                                max={100}
                                value={Math.round(cfg.y_position * 100)}
                                onChange={(v) =>
                                  onUpdateSubConfig(file, { y_position: v / 100 })
                                }
                                label={(v) => `${v}%`}
                              />
                            </Group>
                          </Stack>
                        )}
                      </Stack>
                    </Paper>
                  );
                })}
                {[...subConfigs.values()].some((c) => c.enabled) && (
                  <Text fz="0.65rem" c="var(--color-label-tertiary)">
                    已选 {[...subConfigs.values()].filter((c) => c.enabled).length} 条字幕，播放视频时将自动叠加显示
                  </Text>
                )}
              </Stack>
            )}
          </Box>

          <Text fz="0.65rem" c="var(--color-label-quaternary)">
            路径: {entry.path}
            <br />
            添加时间: {new Date(entry.added_at).toLocaleString("zh-CN")}
          </Text>
        </Stack>
      </Box>
    </ScrollArea>
  );
}

function SectionHeader({ children }: { children: React.ReactNode }) {
  return (
    <Text
      size="xs"
      tt="uppercase"
      c="var(--color-label-quaternary)"
      mb="0.4rem"
      fw={600}
      style={{ letterSpacing: "0.08em", fontSize: "0.65rem" }}
    >
      {children}
    </Text>
  );
}

