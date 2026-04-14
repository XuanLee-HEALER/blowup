import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ActionIcon,
  Badge,
  Box,
  Button,
  Group,
  Menu,
  Modal,
  NumberInput,
  Paper,
  Popover,
  ScrollArea,
  Select,
  Stack,
  Text,
  Tooltip,
} from "@mantine/core";
import {
  IconAdjustmentsHorizontal,
  IconDotsVertical,
  IconDownload,
  IconEye,
  IconInfoCircle,
  IconPlayerPlay,
  IconScissors,
  IconSearch,
  IconTrash,
  IconWaveSine,
} from "@tabler/icons-react";
import { library, subtitle, media, audio, tasks } from "../../lib/tauri";
import type {
  IndexEntry,
  FileMediaInfo,
  SubtitleSearchResult,
  TaskRecord,
} from "../../lib/tauri";
import {
  formatSize,
  formatDuration,
  formatBitrate,
  formatFrameRate,
} from "../../lib/format";
import { useBackendEvent, BackendEvent } from "../../lib/useBackendEvent";

const VIDEO_EXTS = ["mp4", "mkv", "avi", "mov", "ts", "flv", "wmv", "webm", "m4v"];
const SUB_EXTS = ["srt", "ass", "sub", "idx", "vtt"];
const AUDIO_EXTS = ["mp3", "aac", "flac", "opus", "m4a", "wav", "ogg", "ac3", "dts", "mka"];
const getExt = (f: string) => f.split(".").pop()?.toLowerCase() ?? "";
const getStem = (f: string) => f.replace(/\.[^.]+$/, "");

function openWaveformWindow(filePath: string) {
  invoke("open_waveform_window", { filePath }).catch(console.error);
}

interface StatusMsg {
  ok: boolean;
  msg: string;
}

const AUDIO_FORMATS = [
  { label: "MP3", value: "mp3" },
  { label: "AAC", value: "aac" },
  { label: "FLAC", value: "flac" },
  { label: "Opus", value: "opus" },
  { label: "原始", value: "copy" },
];

// ── Shared helpers ──────────────────────────────────────────────

function StatusBadge({ status }: { status: StatusMsg | null }) {
  if (!status) return null;
  return (
    <Text size="xs" c={status.ok ? "var(--color-success)" : "var(--color-danger)"}>
      {status.ok ? "✓ " : "✗ "}
      {status.msg}
    </Text>
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

// ── Video row ───────────────────────────────────────────────────

function VideoRow({
  file,
  rootPath,
  tmdbId,
  cachedInfo,
  onStatusChange,
  onRefresh,
}: {
  file: string;
  rootPath: string;
  tmdbId: number;
  cachedInfo?: FileMediaInfo;
  onStatusChange: (s: StatusMsg) => void;
  onRefresh: () => void;
}) {
  const fullPath = `${rootPath}/${file}`;
  const [probeInfo, setProbeInfo] = useState<FileMediaInfo | null>(cachedInfo ?? null);
  const [probing, setProbing] = useState(false);
  const [showExtractAudio, setShowExtractAudio] = useState(false);
  const [extracting, setExtracting] = useState(false);

  const handlePlay = async () => {
    try {
      await media.openInPlayer(fullPath);
    } catch (e) {
      onStatusChange({ ok: false, msg: `播放失败: ${e}` });
    }
  };

  const handleProbe = async () => {
    setProbing(true);
    try {
      const info = await media.probeAndCache(tmdbId, file);
      setProbeInfo(info);
    } catch (e) {
      onStatusChange({ ok: false, msg: `获取媒体信息失败: ${e}` });
    } finally {
      setProbing(false);
    }
  };

  const handleExtractSubtitle = async () => {
    try {
      const streams = await subtitle.listStreams(fullPath);
      if (streams.length === 0) {
        onStatusChange({ ok: false, msg: "未找到内嵌字幕轨" });
        return;
      }
      await subtitle.extract(fullPath, streams[0].index);
      onStatusChange({ ok: true, msg: "字幕提取成功" });
      onRefresh();
    } catch (e) {
      onStatusChange({ ok: false, msg: `提取失败: ${e}` });
    }
  };

  const handleExtractAudio = async (format: string) => {
    setExtracting(true);
    setShowExtractAudio(false);
    try {
      await audio.extract(fullPath, 0, format);
      onStatusChange({ ok: true, msg: `音轨提取成功 (${format})` });
      onRefresh();
    } catch (e) {
      onStatusChange({ ok: false, msg: `音轨提取失败: ${e}` });
    } finally {
      setExtracting(false);
    }
  };

  return (
    <Box mb="0.4rem">
      <Paper
        withBorder
        px="0.6rem"
        py="0.4rem"
        bg="var(--color-bg-secondary)"
        style={{ borderColor: "var(--color-separator)" }}
      >
        <Group gap={4} wrap="nowrap">
          <Text fz="0.75rem" truncate style={{ flex: 1 }} title={file}>
            {file}
          </Text>
          <Tooltip label="播放" withArrow openDelay={400}>
            <ActionIcon size="sm" variant="filled" color="accent" onClick={handlePlay}>
              <IconPlayerPlay size={14} />
            </ActionIcon>
          </Tooltip>
          <Tooltip label="媒体信息" withArrow openDelay={400}>
            <ActionIcon
              size="sm"
              variant="default"
              onClick={handleProbe}
              loading={probing}
            >
              <IconInfoCircle size={14} />
            </ActionIcon>
          </Tooltip>
          <Menu position="bottom-end" shadow="md" withinPortal>
            <Menu.Target>
              <ActionIcon size="sm" variant="default">
                <IconDotsVertical size={14} />
              </ActionIcon>
            </Menu.Target>
            <Menu.Dropdown>
              <Menu.Item leftSection={<IconScissors size={14} />} onClick={handleExtractSubtitle}>
                提取字幕轨
              </Menu.Item>
              <Menu.Item
                leftSection={<IconWaveSine size={14} />}
                onClick={() => setShowExtractAudio(true)}
                disabled={extracting}
              >
                {extracting ? "提取中…" : "提取音轨"}
              </Menu.Item>
            </Menu.Dropdown>
          </Menu>
        </Group>
      </Paper>

      <Modal
        opened={showExtractAudio}
        onClose={() => setShowExtractAudio(false)}
        title="选择输出格式"
        size="sm"
        centered
      >
        <Stack gap="0.4rem">
          {AUDIO_FORMATS.map((fmt) => (
            <Button
              key={fmt.value}
              variant="default"
              fullWidth
              justify="flex-start"
              onClick={() => handleExtractAudio(fmt.value)}
            >
              {fmt.label}
            </Button>
          ))}
        </Stack>
      </Modal>

      {probeInfo && <ProbeDetail info={probeInfo} />}
    </Box>
  );
}

function ProbeDetail({ info }: { info: FileMediaInfo }) {
  return (
    <Box pl="0.5rem" pt="0.4rem" pb="0.2rem">
      <Stack gap={2}>
        <Text size="xs" c="var(--color-label-secondary)">
          {info.format_name ?? "—"} · {formatSize(info.file_size)} ·{" "}
          {formatDuration(info.duration_secs)} · {formatBitrate(info.bit_rate)}
        </Text>
        {info.streams.map((s) => (
          <Text
            key={s.index}
            size="xs"
            c="var(--color-label-tertiary)"
            style={{ fontSize: "0.65rem" }}
          >
            #{s.index} {s.codec_type} — {s.codec_name}
            {s.codec_type === "video" && s.width && s.height && ` ${s.width}×${s.height}`}
            {s.codec_type === "video" && ` ${formatFrameRate(s.frame_rate)}`}
            {s.codec_type === "audio" && s.channels && ` ${s.channels}ch`}
            {s.codec_type === "audio" && s.sample_rate && ` ${s.sample_rate}Hz`}
            {s.language && ` (${s.language})`}
          </Text>
        ))}
      </Stack>
    </Box>
  );
}

// ── Subtitle row ────────────────────────────────────────────────

function SubtitleRow({
  file,
  rootPath,
  audioFiles,
  alignedFiles,
  task,
  onStatusChange,
  onRefresh,
}: {
  file: string;
  rootPath: string;
  audioFiles: string[];
  alignedFiles: string[];
  task: TaskRecord | undefined;
  onStatusChange: (s: StatusMsg) => void;
  onRefresh: () => void;
}) {
  const fullPath = `${rootPath}/${file}`;
  const [shifting, setShifting] = useState(false);
  const [offsetMs, setOffsetMs] = useState(0);
  const [showShift, setShowShift] = useState(false);
  const [showAlignModal, setShowAlignModal] = useState(false);
  const [alignState, setAlignState] = useState<"idle" | "loading" | "success" | "error">("idle");
  const [alignMsg, setAlignMsg] = useState("");
  const [showAlignBubble, setShowAlignBubble] = useState(false);
  const lastCompletedRef = useRef<string | null>(null);

  useEffect(() => {
    if (!task) return;
    if (task.status.state === "running") {
      setAlignState("loading");
      setAlignMsg("");
    } else if (task.status.state === "completed") {
      setAlignState("success");
      setAlignMsg(task.status.summary);
      if (lastCompletedRef.current !== task.updated_at) {
        lastCompletedRef.current = task.updated_at;
        onRefresh();
      }
    } else if (task.status.state === "failed") {
      setAlignState("error");
      setAlignMsg(task.status.error);
    }
  }, [task, onRefresh]);

  const handleAlignConfirm = (audioFile: string) => {
    setShowAlignModal(false);
    setAlignState("loading");
    setAlignMsg("");
    setShowAlignBubble(false);
    subtitle.alignToAudio(fullPath, `${rootPath}/${audioFile}`).catch((e) => {
      setAlignState("error");
      setAlignMsg(`${e}`);
    });
  };

  const handleShift = async () => {
    if (offsetMs === 0) return;
    setShifting(true);
    try {
      await subtitle.shift(fullPath, offsetMs);
      onStatusChange({
        ok: true,
        msg: `${file} 偏移 ${offsetMs > 0 ? "+" : ""}${offsetMs}ms 完成`,
      });
      setShowShift(false);
      setOffsetMs(0);
    } catch (e) {
      onStatusChange({ ok: false, msg: `偏移失败: ${e}` });
    } finally {
      setShifting(false);
    }
  };

  const handleDelete = async () => {
    try {
      await library.deleteLibraryResource(fullPath);
      onStatusChange({ ok: true, msg: `已删除 ${file}` });
      onRefresh();
    } catch (e) {
      onStatusChange({ ok: false, msg: `删除失败: ${e}` });
    }
  };

  const dismissBubble = () => {
    setShowAlignBubble(false);
    setAlignState("idle");
    tasks.dismiss(fullPath).catch(() => {});
  };

  return (
    <Box mb="0.3rem">
      <Paper
        withBorder
        px="0.6rem"
        py="0.4rem"
        bg="var(--color-bg-secondary)"
        style={{ borderColor: "var(--color-separator)" }}
      >
        <Group gap={4} wrap="nowrap">
          <Text fz="0.75rem" truncate style={{ flex: 1 }} title={file}>
            {file}
          </Text>

          <Popover
            opened={showAlignBubble && (alignState === "success" || alignState === "error")}
            onChange={(o) => {
              if (!o) dismissBubble();
            }}
            position="bottom-end"
            shadow="md"
            withinPortal
          >
            <Popover.Target>
              <Tooltip label="对齐字幕" withArrow openDelay={400}>
                <ActionIcon
                  size="sm"
                  variant="default"
                  loading={alignState === "loading"}
                  onClick={() => {
                    if (alignState === "success" || alignState === "error") {
                      setShowAlignBubble(!showAlignBubble);
                    } else {
                      setShowAlignModal(true);
                    }
                  }}
                  style={{ position: "relative" }}
                >
                  <IconAdjustmentsHorizontal size={14} />
                  {(alignState === "success" || alignState === "error") && (
                    <Box
                      style={{
                        position: "absolute",
                        top: -2,
                        right: -2,
                        width: 7,
                        height: 7,
                        borderRadius: "50%",
                        background:
                          alignState === "success"
                            ? "var(--color-success)"
                            : "var(--color-danger)",
                      }}
                    />
                  )}
                </ActionIcon>
              </Tooltip>
            </Popover.Target>
            <Popover.Dropdown maw={280}>
              <Text
                size="xs"
                c={alignState === "success" ? "var(--color-success)" : "var(--color-danger)"}
                style={{ whiteSpace: "pre-wrap", wordBreak: "break-word" }}
              >
                {alignMsg}
              </Text>
            </Popover.Dropdown>
          </Popover>

          <Menu position="bottom-end" shadow="md" withinPortal>
            <Menu.Target>
              <ActionIcon size="sm" variant="default">
                <IconDotsVertical size={14} />
              </ActionIcon>
            </Menu.Target>
            <Menu.Dropdown>
              <Menu.Item
                leftSection={<IconEye size={14} />}
                onClick={() => subtitle.openViewer(fullPath)}
              >
                查看
              </Menu.Item>
              <Menu.Item
                leftSection={<IconAdjustmentsHorizontal size={14} />}
                onClick={() => setShowShift(!showShift)}
              >
                时间偏移
              </Menu.Item>
              <Menu.Item
                color="danger"
                leftSection={<IconTrash size={14} />}
                onClick={handleDelete}
              >
                删除
              </Menu.Item>
            </Menu.Dropdown>
          </Menu>
        </Group>
      </Paper>

      {/* Align modal */}
      <Modal
        opened={showAlignModal}
        onClose={() => setShowAlignModal(false)}
        title="选择对齐目标音频"
        size="sm"
        centered
      >
        {audioFiles.length === 0 ? (
          <Text size="sm" c="var(--color-label-tertiary)">
            当前目录下没有音频文件。请先从视频中提取音轨。
          </Text>
        ) : (
          <Stack gap="0.35rem">
            {audioFiles.map((af) => (
              <Button
                key={af}
                variant="default"
                fullWidth
                justify="flex-start"
                onClick={() => handleAlignConfirm(af)}
              >
                {af}
              </Button>
            ))}
          </Stack>
        )}
      </Modal>

      {/* Shift panel */}
      {showShift && (
        <Group gap="0.3rem" px="0.6rem" py="0.35rem" wrap="wrap">
          <NumberInput
            size="xs"
            w={80}
            value={offsetMs}
            onChange={(v) => setOffsetMs(typeof v === "number" ? v : 0)}
            hideControls
          />
          <Text size="xs" c="var(--color-label-tertiary)">
            ms
          </Text>
          {[-1000, -500, 500, 1000].map((v) => (
            <Button
              key={v}
              variant="default"
              size="compact-xs"
              onClick={() => setOffsetMs((p) => p + v)}
            >
              {v > 0 ? `+${v}` : v}
            </Button>
          ))}
          <Button
            size="compact-xs"
            disabled={offsetMs === 0 || shifting}
            loading={shifting}
            onClick={handleShift}
          >
            应用
          </Button>
        </Group>
      )}

      {/* Aligned children */}
      {alignedFiles.map((af) => (
        <Paper
          key={af}
          withBorder
          mt={4}
          px="0.6rem"
          py="0.3rem"
          ml="1rem"
          bg="var(--color-bg-secondary)"
          style={{ borderColor: "var(--color-separator)", opacity: 0.85 }}
        >
          <Group gap={4} wrap="nowrap">
            <Text size="xs" c="var(--color-accent)">
              ↳
            </Text>
            <Text fz="0.7rem" c="var(--color-label-secondary)" truncate style={{ flex: 1 }}>
              {af}
            </Text>
            <ActionIcon
              size="sm"
              variant="default"
              onClick={() => subtitle.openViewer(`${rootPath}/${af}`)}
              title="查看"
            >
              <IconEye size={12} />
            </ActionIcon>
            <ActionIcon
              size="sm"
              variant="default"
              color="danger"
              onClick={async () => {
                try {
                  await library.deleteLibraryResource(`${rootPath}/${af}`);
                  onRefresh();
                } catch (e) {
                  onStatusChange({ ok: false, msg: `删除失败: ${e}` });
                }
              }}
              title="删除"
            >
              <IconTrash size={12} />
            </ActionIcon>
          </Group>
        </Paper>
      ))}
    </Box>
  );
}

// ── Audio row ──────────────────────────────────────────────────

function AudioRow({
  file,
  rootPath,
  onStatusChange,
  onRefresh,
}: {
  file: string;
  rootPath: string;
  onStatusChange: (s: StatusMsg) => void;
  onRefresh: () => void;
}) {
  const fullPath = `${rootPath}/${file}`;
  const handlePlay = () => openWaveformWindow(fullPath);
  const handleDelete = async () => {
    try {
      await library.deleteLibraryResource(fullPath);
      onStatusChange({ ok: true, msg: `已删除 ${file}` });
      onRefresh();
    } catch (e) {
      onStatusChange({ ok: false, msg: `删除失败: ${e}` });
    }
  };

  return (
    <Paper
      withBorder
      mb="0.3rem"
      px="0.6rem"
      py="0.4rem"
      bg="var(--color-bg-secondary)"
      style={{ borderColor: "var(--color-separator)" }}
    >
      <Group gap={4} wrap="nowrap">
        <Text fz="0.75rem" truncate style={{ flex: 1 }} title={file}>
          {file}
        </Text>
        <ActionIcon size="sm" variant="filled" color="accent" onClick={handlePlay} title="播放波形">
          <IconWaveSine size={14} />
        </ActionIcon>
        <ActionIcon
          size="sm"
          variant="default"
          color="danger"
          onClick={handleDelete}
          title="删除"
        >
          <IconTrash size={14} />
        </ActionIcon>
      </Group>
    </Paper>
  );
}

// ── Main panel ─────────────────────────────────────────────────

interface LibraryDarkroomTabProps {
  entry: IndexEntry;
  rootDir: string;
}

export function LibraryDarkroomTab({ entry, rootDir }: LibraryDarkroomTabProps) {
  const rootPath = `${rootDir}/${entry.path}`;
  const [status, setStatus] = useState<StatusMsg | null>(null);
  const [files, setFiles] = useState(entry.files);
  const [fetchingLang, setFetchingLang] = useState("zh");
  const [fetchingSub, setFetchingSub] = useState(false);
  const [subResults, setSubResults] = useState<SubtitleSearchResult[] | null>(null);
  const [downloadingSub, setDownloadingSub] = useState<string | null>(null);
  const [taskMap, setTaskMap] = useState<Map<string, TaskRecord>>(() => new Map());

  // Sync local files when the parent passes a new entry
  useEffect(() => {
    setFiles(entry.files);
  }, [entry]);

  const refreshFiles = useCallback(async () => {
    await library.refreshIndexEntry(entry.tmdb_id);
    const entries = await library.listIndexEntries();
    const updated = entries.find((e) => e.tmdb_id === entry.tmdb_id);
    if (updated) setFiles(updated.files);
  }, [entry.tmdb_id]);

  const refreshTasks = useCallback(async () => {
    try {
      const list = await tasks.list();
      setTaskMap(new Map(list.map((t) => [t.id, t])));
    } catch {
      /* */
    }
  }, []);
  useEffect(() => {
    refreshTasks();
  }, [refreshTasks]);
  useBackendEvent(BackendEvent.TASKS_CHANGED, refreshTasks);

  const videoFiles = files.filter((f) => VIDEO_EXTS.includes(getExt(f)));
  const allSubtitleFiles = files.filter((f) => SUB_EXTS.includes(getExt(f)));
  const isAligned = (f: string) => f.includes(".aligned.");
  const subtitleFiles = allSubtitleFiles.filter((f) => !isAligned(f));
  const getAlignedFiles = (parentFile: string) => {
    const stem = getStem(parentFile);
    return allSubtitleFiles.filter(
      (f) => isAligned(f) && f.startsWith(stem + ".aligned.")
    );
  };
  const audioFiles = files.filter((f) => AUDIO_EXTS.includes(getExt(f)));
  const primaryVideo = videoFiles[0] ?? null;

  const handleSearchSubtitle = async () => {
    if (!primaryVideo) {
      setStatus({ ok: false, msg: "无视频文件，无法搜索字幕" });
      return;
    }
    setFetchingSub(true);
    setStatus(null);
    setSubResults(null);
    try {
      const results = await subtitle.search(
        `${rootPath}/${primaryVideo}`,
        fetchingLang,
        entry.title,
        entry.year ?? undefined,
        entry.tmdb_id
      );
      if (results.length === 0) {
        setStatus({ ok: false, msg: "未找到字幕" });
      } else {
        setSubResults(results);
      }
    } catch (e) {
      setStatus({ ok: false, msg: `字幕搜索失败: ${e}` });
    } finally {
      setFetchingSub(false);
    }
  };

  const handleDownloadSubtitle = async (downloadId: string) => {
    if (!primaryVideo) return;
    setDownloadingSub(downloadId);
    try {
      await subtitle.download(`${rootPath}/${primaryVideo}`, fetchingLang, downloadId);
      setStatus({ ok: true, msg: "字幕下载成功" });
      setSubResults(null);
      await refreshFiles();
    } catch (e) {
      setStatus({ ok: false, msg: `字幕下载失败: ${e}` });
    } finally {
      setDownloadingSub(null);
    }
  };

  const handleExtractAllSubs = async () => {
    if (!primaryVideo) return;
    setStatus(null);
    try {
      const streams = await subtitle.listStreams(`${rootPath}/${primaryVideo}`);
      if (streams.length === 0) {
        setStatus({ ok: false, msg: "未找到内嵌字幕轨" });
        return;
      }
      for (const s of streams) {
        await subtitle.extract(`${rootPath}/${primaryVideo}`, s.index);
      }
      setStatus({ ok: true, msg: `提取了 ${streams.length} 条字幕轨` });
      await refreshFiles();
    } catch (e) {
      setStatus({ ok: false, msg: `提取失败: ${e}` });
    }
  };

  return (
    <ScrollArea style={{ flex: 1 }}>
      <Box p="0.75rem">
        {status && (
          <Box mb="0.5rem">
            <StatusBadge status={status} />
          </Box>
        )}

        {/* Video */}
        <SectionHeader>视频</SectionHeader>
        {videoFiles.length === 0 ? (
          <Text size="xs" c="var(--color-label-tertiary)" mb="0.75rem">
            无视频文件
          </Text>
        ) : (
          <Box mb="0.75rem">
            {videoFiles.map((f) => (
              <VideoRow
                key={f}
                file={f}
                rootPath={rootPath}
                tmdbId={entry.tmdb_id}
                cachedInfo={entry.media_info?.[f]}
                onStatusChange={setStatus}
                onRefresh={refreshFiles}
              />
            ))}
          </Box>
        )}

        {/* Subtitle */}
        <SectionHeader>字幕</SectionHeader>
        {subtitleFiles.length === 0 ? (
          <Text size="xs" c="var(--color-label-tertiary)" mb="0.4rem">
            无字幕文件
          </Text>
        ) : (
          <Box mb="0.4rem">
            {subtitleFiles.map((f) => (
              <SubtitleRow
                key={f}
                file={f}
                rootPath={rootPath}
                audioFiles={audioFiles}
                alignedFiles={getAlignedFiles(f)}
                task={taskMap.get(`subtitle-align-audio::${rootPath}/${f}`)}
                onStatusChange={setStatus}
                onRefresh={refreshFiles}
              />
            ))}
          </Box>
        )}

        <Group gap="0.3rem" mb={subResults ? "0.4rem" : "0.75rem"} wrap="wrap">
          <Button
            variant="default"
            size="compact-xs"
            leftSection={<IconSearch size={12} />}
            disabled={!primaryVideo || fetchingSub}
            loading={fetchingSub}
            onClick={handleSearchSubtitle}
          >
            搜索字幕
          </Button>
          <Select
            size="xs"
            w={90}
            value={fetchingLang}
            onChange={(v) => v && setFetchingLang(v)}
            data={[
              { value: "zh", label: "中文" },
              { value: "en", label: "English" },
              { value: "ja", label: "日本語" },
              { value: "ko", label: "한국어" },
              { value: "fr", label: "Français" },
            ]}
          />
          <Button
            variant="default"
            size="compact-xs"
            leftSection={<IconScissors size={12} />}
            disabled={!primaryVideo}
            onClick={handleExtractAllSubs}
          >
            从视频提取
          </Button>
        </Group>

        {subResults && (
          <Box mb="0.75rem">
            <Stack gap={4}>
              {subResults.map((r) => (
                <Paper
                  key={r.download_id}
                  withBorder
                  px="0.6rem"
                  py="0.4rem"
                  bg="var(--color-bg-secondary)"
                  style={{ borderColor: "var(--color-separator)" }}
                >
                  <Group gap="0.3rem" wrap="nowrap" align="center">
                    <Badge
                      size="xs"
                      variant="light"
                      color={r.source === "assrt" ? "warning" : "accent"}
                    >
                      {r.source === "assrt" ? "ASSRT" : "OS"}
                    </Badge>
                    <Text fz="0.7rem" truncate style={{ flex: 1 }} title={r.title}>
                      {r.title}
                    </Text>
                    {r.language && (
                      <Text fz="0.6rem" c="var(--color-label-quaternary)">
                        {r.language}
                      </Text>
                    )}
                    <ActionIcon
                      size="sm"
                      variant="filled"
                      color="accent"
                      loading={downloadingSub === r.download_id}
                      disabled={downloadingSub !== null}
                      onClick={() => handleDownloadSubtitle(r.download_id)}
                      title="下载"
                    >
                      <IconDownload size={12} />
                    </ActionIcon>
                  </Group>
                </Paper>
              ))}
            </Stack>
            <Button
              variant="subtle"
              size="compact-xs"
              color="gray"
              mt={4}
              onClick={() => setSubResults(null)}
            >
              关闭结果
            </Button>
          </Box>
        )}

        {/* Audio */}
        {audioFiles.length > 0 && (
          <>
            <SectionHeader>音频</SectionHeader>
            <Box mb="0.75rem">
              {audioFiles.map((f) => (
                <AudioRow
                  key={f}
                  file={f}
                  rootPath={rootPath}
                  onStatusChange={setStatus}
                  onRefresh={refreshFiles}
                />
              ))}
            </Box>
          </>
        )}
      </Box>
    </ScrollArea>
  );
}
