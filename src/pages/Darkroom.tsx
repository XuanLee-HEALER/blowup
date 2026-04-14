import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  ActionIcon,
  Badge,
  Box,
  Button,
  Flex,
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
  TextInput,
  Title,
  UnstyledButton,
} from "@mantine/core";
import { library, subtitle, media, audio, config, tasks } from "../lib/tauri";
import type {
  IndexEntry,
  FileMediaInfo,
  SubtitleSearchResult,
  TaskRecord,
} from "../lib/tauri";
import {
  formatSize,
  formatDuration,
  formatBitrate,
  formatFrameRate,
} from "../lib/format";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

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

// ── Shared small components ──────────────────────────────────────

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
      mb="0.5rem"
      style={{ letterSpacing: "0.08em", fontSize: "0.7rem" }}
    >
      {children}
    </Text>
  );
}

// ── Video resource row ──────────────────────────────────────────

function VideoRow({
  file,
  rootPath,
  tmdbId,
  cachedInfo,
  highlighted,
  onHover,
  onStatusChange,
  onRefresh,
}: {
  file: string;
  rootPath: string;
  tmdbId: number;
  cachedInfo?: FileMediaInfo;
  highlighted: boolean;
  onHover: (file: string | null) => void;
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
    <Box mb="0.4rem" onMouseEnter={() => onHover(file)} onMouseLeave={() => onHover(null)}>
      <Paper
        withBorder
        px="0.75rem"
        py="0.5rem"
        bg={highlighted ? "var(--color-accent-soft)" : "var(--color-bg-secondary)"}
        style={{
          borderColor: "var(--color-separator)",
          boxShadow: highlighted ? "inset 3px 0 0 var(--color-accent)" : undefined,
          transition: "background 0.15s",
        }}
      >
        <Group gap="0.4rem" wrap="nowrap">
          <Text fz="0.82rem" truncate style={{ flex: 1 }}>
            {file}
          </Text>
          <Button size="compact-xs" onClick={handlePlay}>
            ▶ 播放
          </Button>
          <Button variant="default" size="compact-xs" onClick={handleProbe} loading={probing}>
            媒体信息
          </Button>
          <Menu position="bottom-end" shadow="md" withinPortal>
            <Menu.Target>
              <ActionIcon variant="default" size="sm">
                ⋮
              </ActionIcon>
            </Menu.Target>
            <Menu.Dropdown>
              <Menu.Item onClick={handleExtractSubtitle}>提取字幕轨</Menu.Item>
              <Menu.Item onClick={() => setShowExtractAudio(true)} disabled={extracting}>
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
    <Box pl="1.5rem" pr="0.75rem" pt="0.5rem" pb="0.25rem">
      <Group gap="1rem" mb="0.3rem" wrap="wrap">
        <Text size="xs" c="var(--color-label-secondary)">
          格式: {info.format_name ?? "—"}
        </Text>
        <Text size="xs" c="var(--color-label-secondary)">
          大小: {formatSize(info.file_size)}
        </Text>
        <Text size="xs" c="var(--color-label-secondary)">
          时长: {formatDuration(info.duration_secs)}
        </Text>
        <Text size="xs" c="var(--color-label-secondary)">
          码率: {formatBitrate(info.bit_rate)}
        </Text>
      </Group>
      {info.streams.map((s) => (
        <Text key={s.index} size="xs" c="var(--color-label-tertiary)">
          #{s.index} {s.codec_type} — {s.codec_name}
          {s.codec_type === "video" && s.width && s.height && ` ${s.width}x${s.height}`}
          {s.codec_type === "video" && ` ${formatFrameRate(s.frame_rate)}`}
          {s.codec_type === "audio" && s.channels && ` ${s.channels}ch`}
          {s.codec_type === "audio" && s.sample_rate && ` ${s.sample_rate}Hz`}
          {s.language && ` (${s.language})`}
          {s.title && ` "${s.title}"`}
        </Text>
      ))}
    </Box>
  );
}

// ── Subtitle resource row ───────────────────────────────────────

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
        px="0.75rem"
        py="0.4rem"
        bg="var(--color-bg-secondary)"
        style={{ borderColor: "var(--color-separator)" }}
      >
        <Group gap="0.4rem" wrap="nowrap">
          <Text fz="0.82rem" truncate style={{ flex: 1 }}>
            {file}
          </Text>

          {/* Align button + bubble */}
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
              <Button
                variant="default"
                size="compact-xs"
                disabled={alignState === "loading"}
                loading={alignState === "loading"}
                onClick={() => {
                  if (alignState === "success" || alignState === "error") {
                    setShowAlignBubble(!showAlignBubble);
                  } else {
                    setShowAlignModal(true);
                  }
                }}
                rightSection={
                  alignState === "success" || alignState === "error" ? (
                    <Box
                      w={7}
                      h={7}
                      style={{
                        borderRadius: "50%",
                        background:
                          alignState === "success"
                            ? "var(--color-success)"
                            : "var(--color-danger)",
                      }}
                    />
                  ) : null
                }
              >
                {alignState === "loading" ? "对齐中…" : "对齐"}
              </Button>
            </Popover.Target>
            <Popover.Dropdown maw={360}>
              <Text
                size="xs"
                c={
                  alignState === "success" ? "var(--color-success)" : "var(--color-danger)"
                }
                style={{ whiteSpace: "pre-wrap", wordBreak: "break-word" }}
              >
                {alignMsg}
              </Text>
            </Popover.Dropdown>
          </Popover>

          <Menu position="bottom-end" shadow="md" withinPortal>
            <Menu.Target>
              <ActionIcon variant="default" size="sm">
                ⋮
              </ActionIcon>
            </Menu.Target>
            <Menu.Dropdown>
              <Menu.Item onClick={() => subtitle.openViewer(fullPath)}>查看</Menu.Item>
              <Menu.Item onClick={() => setShowShift(!showShift)}>时间偏移</Menu.Item>
              <Menu.Item color="danger" onClick={handleDelete}>
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
        <Group gap="0.4rem" px="1.5rem" py="0.35rem" align="center">
          <NumberInput
            size="xs"
            w={100}
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

      {/* Aligned child files */}
      {alignedFiles.map((af) => (
        <Paper
          key={af}
          withBorder
          mt="0.15rem"
          px="0.75rem"
          py="0.3rem"
          ml="1.8rem"
          bg="var(--color-bg-secondary)"
          style={{ borderColor: "var(--color-separator)", opacity: 0.85 }}
        >
          <Group gap="0.4rem" wrap="nowrap">
            <Text size="xs" c="var(--color-accent)">
              ↳
            </Text>
            <Text fz="0.78rem" c="var(--color-label-secondary)" truncate style={{ flex: 1 }}>
              {af}
            </Text>
            <Button
              variant="default"
              size="compact-xs"
              onClick={() => subtitle.openViewer(`${rootPath}/${af}`)}
            >
              查看
            </Button>
            <Button
              variant="default"
              color="danger"
              size="compact-xs"
              onClick={async () => {
                try {
                  await library.deleteLibraryResource(`${rootPath}/${af}`);
                  onRefresh();
                } catch (e) {
                  onStatusChange({ ok: false, msg: `删除失败: ${e}` });
                }
              }}
            >
              删除
            </Button>
          </Group>
        </Paper>
      ))}
    </Box>
  );
}

// ── Audio resource row ─────────────────────────────────────────

function AudioRow({
  file,
  rootPath,
  highlighted,
  onHover,
  onStatusChange,
  onRefresh,
}: {
  file: string;
  rootPath: string;
  highlighted: boolean;
  onHover: (file: string | null) => void;
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
    <Box mb="0.3rem" onMouseEnter={() => onHover(file)} onMouseLeave={() => onHover(null)}>
      <Paper
        withBorder
        px="0.75rem"
        py="0.4rem"
        bg={highlighted ? "var(--color-accent-soft)" : "var(--color-bg-secondary)"}
        style={{
          borderColor: "var(--color-separator)",
          boxShadow: highlighted ? "inset 3px 0 0 var(--color-accent)" : undefined,
          transition: "background 0.15s",
        }}
      >
        <Group gap="0.4rem" wrap="nowrap">
          <Text fz="0.82rem" truncate style={{ flex: 1 }}>
            {file}
          </Text>
          <Button size="compact-xs" onClick={handlePlay}>
            ▶ 播放
          </Button>
          <Button variant="default" color="danger" size="compact-xs" onClick={handleDelete}>
            删除
          </Button>
        </Group>
      </Paper>
    </Box>
  );
}

// ── Workspace panel (right side) ────────────────────────────────

function WorkspacePanel({ entry, rootDir }: { entry: IndexEntry; rootDir: string }) {
  const rootPath = `${rootDir}/${entry.path}`;
  const [status, setStatus] = useState<StatusMsg | null>(null);
  const [files, setFiles] = useState(entry.files);
  const [fetchingLang, setFetchingLang] = useState("zh");
  const [fetchingSub, setFetchingSub] = useState(false);
  const [subResults, setSubResults] = useState<SubtitleSearchResult[] | null>(null);
  const [downloadingSub, setDownloadingSub] = useState<string | null>(null);
  const [hoveredVideo, setHoveredVideo] = useState<string | null>(null);
  const [hoveredAudio, setHoveredAudio] = useState<string | null>(null);
  const [taskMap, setTaskMap] = useState<Map<string, TaskRecord>>(() => new Map());

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
    return allSubtitleFiles.filter((f) => isAligned(f) && f.startsWith(stem + ".aligned."));
  };
  const audioFiles = files.filter((f) => AUDIO_EXTS.includes(getExt(f)));
  const otherFiles = files.filter(
    (f) =>
      !VIDEO_EXTS.includes(getExt(f)) &&
      !SUB_EXTS.includes(getExt(f)) &&
      !AUDIO_EXTS.includes(getExt(f))
  );
  const primaryVideo = videoFiles[0] ?? null;

  const isAudioLinkedToVideo = (audioFile: string, videoFile: string) =>
    audioFile.startsWith(`${getStem(videoFile)}_audio_`);

  const isVideoHighlighted = (videoFile: string) =>
    !!hoveredAudio && isAudioLinkedToVideo(hoveredAudio, videoFile);

  const isAudioHighlighted = (audioFile: string) =>
    !!hoveredVideo && isAudioLinkedToVideo(audioFile, hoveredVideo);

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
    <Box>
      {/* Header */}
      <Box mb="1.2rem">
        <Title order={2} fz="1.1rem" fw={700} mb={4} style={{ letterSpacing: "-0.02em" }}>
          {entry.title}
        </Title>
        <Text size="sm" c="var(--color-label-tertiary)">
          {entry.director_display} · {entry.year ?? "—"}
        </Text>
        <Text size="xs" c="var(--color-label-quaternary)" mt={2}>
          {rootPath}
        </Text>
      </Box>

      {status && (
        <Box mb="0.75rem">
          <StatusBadge status={status} />
        </Box>
      )}

      {/* Video section */}
      <SectionHeader>视频</SectionHeader>
      {videoFiles.length === 0 ? (
        <Text size="sm" c="var(--color-label-tertiary)" mb="1rem">
          无视频文件
        </Text>
      ) : (
        <Box mb="1rem">
          {videoFiles.map((f) => (
            <VideoRow
              key={f}
              file={f}
              rootPath={rootPath}
              tmdbId={entry.tmdb_id}
              cachedInfo={entry.media_info?.[f]}
              highlighted={isVideoHighlighted(f)}
              onHover={setHoveredVideo}
              onStatusChange={setStatus}
              onRefresh={refreshFiles}
            />
          ))}
        </Box>
      )}

      {/* Subtitle section */}
      <SectionHeader>字幕</SectionHeader>
      {subtitleFiles.length === 0 ? (
        <Text size="sm" c="var(--color-label-tertiary)" mb="0.5rem">
          无字幕文件
        </Text>
      ) : (
        <Box mb="0.5rem">
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

      {/* Subtitle actions */}
      <Group gap="0.4rem" mb={subResults ? "0.5rem" : "1.2rem"} align="center">
        <Button
          variant="default"
          size="compact-xs"
          disabled={!primaryVideo || fetchingSub}
          loading={fetchingSub}
          onClick={handleSearchSubtitle}
        >
          + 搜索字幕
        </Button>
        <Select
          size="xs"
          w={110}
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
          disabled={!primaryVideo}
          onClick={handleExtractAllSubs}
        >
          + 从视频提取
        </Button>
      </Group>

      {/* Subtitle search results */}
      {subResults && (
        <Box mb="1.2rem">
          <Stack gap={4}>
            {subResults.map((r) => (
              <Paper
                key={r.download_id}
                withBorder
                px="0.75rem"
                py="0.4rem"
                bg="var(--color-bg-secondary)"
                style={{ borderColor: "var(--color-separator)" }}
              >
                <Group gap="0.5rem" wrap="nowrap" align="center">
                  <Badge
                    size="xs"
                    variant="light"
                    color={r.source === "assrt" ? "warning" : "accent"}
                  >
                    {r.source === "assrt" ? "ASSRT" : "OS"}
                  </Badge>
                  <Text fz="0.78rem" truncate style={{ flex: 1 }}>
                    {r.title}
                  </Text>
                  {r.language && (
                    <Text fz="0.65rem" c="var(--color-label-quaternary)">
                      {r.language}
                    </Text>
                  )}
                  {r.download_count != null && (
                    <Text fz="0.68rem" c="var(--color-label-quaternary)">
                      {r.download_count} 次
                    </Text>
                  )}
                  <Button
                    size="compact-xs"
                    loading={downloadingSub === r.download_id}
                    disabled={downloadingSub !== null}
                    onClick={() => handleDownloadSubtitle(r.download_id)}
                  >
                    下载
                  </Button>
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

      {/* Audio section */}
      {audioFiles.length > 0 && (
        <>
          <SectionHeader>音频</SectionHeader>
          <Box mb="1rem">
            {audioFiles.map((f) => (
              <AudioRow
                key={f}
                file={f}
                rootPath={rootPath}
                highlighted={isAudioHighlighted(f)}
                onHover={setHoveredAudio}
                onStatusChange={setStatus}
                onRefresh={refreshFiles}
              />
            ))}
          </Box>
        </>
      )}

      {/* Other files */}
      {otherFiles.length > 0 && (
        <>
          <SectionHeader>其他文件</SectionHeader>
          <Box mb="1rem">
            {otherFiles.map((f) => (
              <Text key={f} size="sm" c="var(--color-label-tertiary)" px="0.75rem" py="0.3rem">
                {f}
              </Text>
            ))}
          </Box>
        </>
      )}
    </Box>
  );
}

// ── Main Darkroom page ──────────────────────────────────────────

export default function Darkroom() {
  const [directorMap, setDirectorMap] = useState<Record<string, IndexEntry[]>>({});
  const [selectedDirector, setSelectedDirector] = useState<string | null>(null);
  const [selectedEntry, setSelectedEntry] = useState<IndexEntry | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<IndexEntry[] | null>(null);
  const [rootDir, setRootDir] = useState("");

  const refreshDirectorMap = useCallback(() => {
    library.listIndexByDirector().then(setDirectorMap);
  }, []);

  useEffect(() => {
    refreshDirectorMap();
    config.get().then((c) => setRootDir(c.library.root_dir));
  }, [refreshDirectorMap]);

  useBackendEvent(BackendEvent.LIBRARY_CHANGED, refreshDirectorMap);

  const doSearch = async () => {
    if (!searchQuery.trim()) {
      setSearchResults(null);
      return;
    }
    const results = await library.searchIndex(searchQuery.trim());
    setSearchResults(results);
  };

  const directors = Object.keys(directorMap).sort();

  return (
    <Flex h="100%" style={{ overflow: "hidden" }}>
      {/* Left: film selector */}
      <Flex
        direction="column"
        w={240}
        style={{
          flexShrink: 0,
          borderRight: "1px solid var(--color-separator)",
          overflow: "hidden",
          userSelect: "none",
          WebkitUserSelect: "none",
        }}
      >
        <Box px="1rem" pt="1.4rem">
          <Title order={1} mb="0.8rem" fz="1.3rem" fw={700} style={{ letterSpacing: "-0.035em" }}>
            暗房
          </Title>
          <TextInput
            leftSection={<span>⌕</span>}
            placeholder="搜索影片…"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.nativeEvent.isComposing) doSearch();
            }}
            mb="0.5rem"
            size="xs"
          />
          {searchResults && (
            <Box mb="0.5rem">
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
            </Box>
          )}
        </Box>

        <Box style={{ height: 1, background: "var(--color-separator)", margin: "0 1rem" }} />

        <ScrollArea style={{ flex: 1 }}>
          <Box py="0.5rem">
            {searchResults
              ? searchResults.map((e) => (
                  <UnstyledButton
                    key={e.tmdb_id}
                    onClick={() => setSelectedEntry(e)}
                    w="100%"
                    px="1rem"
                    py="0.5rem"
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
                      {e.director_display} · {e.year ?? "—"}
                    </Text>
                  </UnstyledButton>
                ))
              : directors.map((dir) => (
                  <Box key={dir}>
                    <UnstyledButton
                      onClick={() =>
                        setSelectedDirector(selectedDirector === dir ? null : dir)
                      }
                      w="100%"
                      px="1rem"
                      py="0.45rem"
                      style={{
                        background:
                          selectedDirector === dir
                            ? "var(--color-bg-elevated)"
                            : "transparent",
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
                          onClick={() => setSelectedEntry(e)}
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
                暂无影片。通过搜索页下载电影后即可使用暗房。
              </Text>
            )}
          </Box>
        </ScrollArea>
      </Flex>

      {/* Right: workspace */}
      <ScrollArea style={{ flex: 1 }}>
        <Box px="1.75rem" py="1.4rem">
          {selectedEntry && rootDir ? (
            <WorkspacePanel
              key={selectedEntry.tmdb_id}
              entry={selectedEntry}
              rootDir={rootDir}
            />
          ) : (
            <Flex align="center" justify="center" h="100%" style={{ minHeight: "60vh" }}>
              <Text size="sm" c="var(--color-label-tertiary)">
                选择一部电影开始工作
              </Text>
            </Flex>
          )}
        </Box>
      </ScrollArea>
    </Flex>
  );
}
