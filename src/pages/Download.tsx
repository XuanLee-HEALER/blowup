import { useState, useEffect, useCallback, useRef } from "react";
import {
  Badge,
  Box,
  Button,
  Checkbox,
  Group,
  Modal,
  Progress,
  ScrollArea,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { download } from "../lib/tauri";
import type { DownloadRecord, TorrentFileInfo } from "../lib/tauri";
import { formatSize } from "../lib/format";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

// ── Helpers ──────────────────────────────────────────────────────

function statusLabel(s: string) {
  switch (s) {
    case "downloading":
      return "下载中";
    case "paused":
      return "已暂停";
    case "completed":
      return "已完成";
    case "failed":
      return "失败";
    default:
      return "等待中";
  }
}

function statusBadgeStyle(s: string): { bg: string; fg: string } {
  switch (s) {
    case "downloading":
      return { bg: "var(--color-accent-soft)", fg: "var(--color-accent)" };
    case "paused":
      return { bg: "var(--color-warning-soft)", fg: "var(--color-warning)" };
    case "completed":
      return { bg: "var(--color-success-soft)", fg: "var(--color-success)" };
    case "failed":
      return { bg: "var(--color-danger-soft)", fg: "var(--color-danger)" };
    default:
      return { bg: "var(--color-bg-control)", fg: "var(--color-label-secondary)" };
  }
}

// ── Download Row ─────────────────────────────────────────────────

function DownloadRow({
  record,
  prevBytes,
  isLast,
  onPause,
  onResume,
  onDelete,
  onRedownload,
}: {
  record: DownloadRecord;
  prevBytes: number;
  isLast: boolean;
  onPause: () => void;
  onResume: () => void;
  onDelete: () => void;
  onRedownload: () => void;
}) {
  const isActive = record.status === "downloading";
  const progress =
    record.total_bytes > 0
      ? Math.min(100, (record.progress_bytes / record.total_bytes) * 100)
      : 0;

  // Speed calc: bytes diff over 2s event interval
  const speed =
    isActive && prevBytes > 0
      ? Math.max(0, record.progress_bytes - prevBytes) / 2
      : 0;

  return (
    <Box
      py="0.75rem"
      style={{ borderBottom: isLast ? undefined : "1px solid var(--color-separator)" }}
    >
      <Group justify="space-between" align="center" mb="0.3rem" wrap="nowrap">
        <Box style={{ flex: 1, minWidth: 0 }}>
          <Text fz="0.85rem" fw={500} truncate>
            {record.title}
          </Text>
          <Group gap="0.4rem" mt={2} wrap="nowrap">
            {record.director && (
              <Text fz="0.7rem" c="var(--color-label-tertiary)">
                {record.director}
              </Text>
            )}
            {record.quality && (
              <Text fz="0.7rem" c="var(--color-label-tertiary)">
                · {record.quality}
              </Text>
            )}
            <Badge
              size="xs"
              variant="light"
              styles={{
                root: {
                  backgroundColor: statusBadgeStyle(record.status).bg,
                  color: statusBadgeStyle(record.status).fg,
                },
              }}
            >
              {statusLabel(record.status)}
            </Badge>
          </Group>
        </Box>
        <Group gap="0.4rem" wrap="nowrap" style={{ flexShrink: 0 }}>
          {isActive && (
            <Button variant="default" size="compact-xs" onClick={onPause}>
              暂停
            </Button>
          )}
          {record.status === "paused" && (
            <Button variant="light" size="compact-xs" onClick={onResume}>
              继续
            </Button>
          )}
          {(record.status === "completed" || record.status === "failed") && (
            <Button variant="default" size="compact-xs" onClick={onRedownload}>
              重新下载
            </Button>
          )}
          <Button variant="subtle" color="danger" size="compact-xs" onClick={onDelete}>
            删除
          </Button>
        </Group>
      </Group>

      {(isActive || record.status === "paused") && record.total_bytes > 0 && (
        <Stack gap={2}>
          <Progress
            value={progress}
            size="xs"
            color={isActive ? "accent" : "gray"}
            transitionDuration={500}
          />
          <Group justify="space-between">
            <Text fz="0.65rem" c="var(--color-label-quaternary)">
              {formatSize(record.progress_bytes)} / {formatSize(record.total_bytes)}
            </Text>
            <Text fz="0.65rem" c="var(--color-label-quaternary)">
              {progress.toFixed(1)}%{speed > 0 ? ` · ${formatSize(speed)}/s` : ""}
            </Text>
          </Group>
        </Stack>
      )}

      {record.error_message && (
        <Text fz="0.7rem" c="var(--color-danger)" mt="0.25rem">
          {record.error_message}
        </Text>
      )}
    </Box>
  );
}

// ── Main Page ────────────────────────────────────────────────────

export default function Download() {
  const [downloads, setDownloads] = useState<DownloadRecord[]>([]);
  const prevBytesRef = useRef<Map<number, number>>(new Map());
  const [deleteTarget, setDeleteTarget] = useState<DownloadRecord | null>(null);
  // Redownload file-pick modal state
  const [redownloadTarget, setRedownloadTarget] = useState<DownloadRecord | null>(null);
  const [redownloadFiles, setRedownloadFiles] = useState<TorrentFileInfo[]>([]);
  const [redownloadExisting, setRedownloadExisting] = useState<Set<string>>(new Set());
  const [redownloadSelected, setRedownloadSelected] = useState<Set<number>>(new Set());
  const [redownloadFetching, setRedownloadFetching] = useState(false);
  const [redownloadSubmitting, setRedownloadSubmitting] = useState(false);
  const [overwriteConfirm, setOverwriteConfirm] = useState(false);

  const refresh = useCallback(async () => {
    const list = await download.listDownloads();
    setDownloads((prev) => {
      const map = new Map<number, number>();
      for (const d of prev) {
        map.set(d.id, d.progress_bytes);
      }
      prevBytesRef.current = map;
      return list;
    });
  }, []);

  useEffect(() => {
    download.listDownloads().then(setDownloads);
  }, []);

  useBackendEvent(BackendEvent.DOWNLOADS_CHANGED, refresh);

  const handlePause = async (id: number) => {
    await download.pauseDownload(id);
    refresh();
  };

  const handleResume = async (id: number) => {
    await download.resumeDownload(id);
    refresh();
  };

  const handleDelete = async (record: DownloadRecord) => {
    const isActive =
      record.status === "downloading" ||
      record.status === "paused" ||
      record.status === "pending";
    if (isActive) {
      setDeleteTarget(record);
    } else {
      await download.deleteDownload(record.id);
      refresh();
    }
  };

  const handleDeleteConfirm = async () => {
    if (!deleteTarget) return;
    await download.deleteDownload(deleteTarget.id);
    setDeleteTarget(null);
    refresh();
  };

  const handleRedownload = async (record: DownloadRecord) => {
    setRedownloadFetching(true);
    setRedownloadTarget(record);
    try {
      const [files, existing] = await Promise.all([
        download.getTorrentFiles(record.target),
        download.listExistingFiles(record.id),
      ]);
      setRedownloadFiles(files);
      setRedownloadExisting(new Set(existing));
      setRedownloadSelected(new Set(files.map((f) => f.index)));
    } catch (e) {
      console.error("fetch torrent files failed:", e);
      setRedownloadTarget(null);
    } finally {
      setRedownloadFetching(false);
    }
  };

  const handleRedownloadConfirm = async () => {
    if (!redownloadTarget) return;

    const selectedNames = redownloadFiles
      .filter((f) => redownloadSelected.has(f.index))
      .map((f) => f.name);
    const conflicts = selectedNames.filter((n) => redownloadExisting.has(n));

    if (conflicts.length > 0 && !overwriteConfirm) {
      setOverwriteConfirm(true);
      return;
    }

    setRedownloadSubmitting(true);
    try {
      await download.redownload(redownloadTarget.id, [...redownloadSelected]);
      closeRedownloadModal();
      refresh();
    } catch (e) {
      console.error("redownload failed:", e);
      closeRedownloadModal();
    }
  };

  const closeRedownloadModal = () => {
    setRedownloadTarget(null);
    setRedownloadFiles([]);
    setRedownloadExisting(new Set());
    setRedownloadSelected(new Set());
    setRedownloadSubmitting(false);
    setOverwriteConfirm(false);
    setRedownloadFetching(false);
  };

  const active = downloads.filter(
    (d) => d.status === "downloading" || d.status === "paused" || d.status === "pending"
  );
  const history = downloads.filter((d) => d.status === "completed" || d.status === "failed");

  return (
    <ScrollArea style={{ flex: 1 }}>
      <Box px="1.75rem" pt="1.4rem" pb="3rem">
        <Title order={1} mb="1.5rem" fz="1.6rem" fw={700} style={{ letterSpacing: "-0.035em" }}>
          下载
        </Title>

        {active.length > 0 && (
          <Box mb="2rem">
            <Text
              size="xs"
              tt="uppercase"
              c="var(--color-label-quaternary)"
              mb="0.5rem"
              style={{ letterSpacing: "0.08em", fontSize: "0.7rem" }}
            >
              进行中
            </Text>
            {active.map((d, i) => (
              <DownloadRow
                key={d.id}
                record={d}
                prevBytes={prevBytesRef.current.get(d.id) ?? 0}
                isLast={i === active.length - 1}
                onPause={() => handlePause(d.id)}
                onResume={() => handleResume(d.id)}
                onDelete={() => handleDelete(d)}
                onRedownload={() => handleRedownload(d)}
              />
            ))}
          </Box>
        )}

        <Box>
          <Text
            size="xs"
            tt="uppercase"
            c="var(--color-label-quaternary)"
            mb="0.5rem"
            style={{ letterSpacing: "0.08em", fontSize: "0.7rem" }}
          >
            历史记录
          </Text>
          {history.length === 0 && (
            <Text size="sm" c="var(--color-label-tertiary)">
              暂无下载记录
            </Text>
          )}
          {history.map((d, i) => (
            <DownloadRow
              key={d.id}
              record={d}
              prevBytes={0}
              isLast={i === history.length - 1}
              onPause={() => {}}
              onResume={() => {}}
              onDelete={() => handleDelete(d)}
              onRedownload={() => handleRedownload(d)}
            />
          ))}
        </Box>
      </Box>

      {/* Delete confirm */}
      <Modal
        opened={!!deleteTarget}
        onClose={() => setDeleteTarget(null)}
        title="确认删除"
        centered
        size="sm"
      >
        <Stack gap="md">
          <Text size="sm" c="var(--color-label-secondary)">
            将永久删除「{deleteTarget?.title}」的下载记录及所有已下载文件（包括种子文件），此操作不可恢复。
          </Text>
          <Group justify="flex-end" gap="0.5rem">
            <Button variant="subtle" onClick={() => setDeleteTarget(null)}>
              取消
            </Button>
            <Button color="danger" onClick={handleDeleteConfirm}>
              删除
            </Button>
          </Group>
        </Stack>
      </Modal>

      {/* Redownload file picker */}
      <Modal
        opened={!!redownloadTarget}
        onClose={() => {
          if (!redownloadSubmitting && !redownloadFetching) closeRedownloadModal();
        }}
        title={overwriteConfirm ? "文件已存在" : "选择下载文件"}
        centered
        size="lg"
      >
        {redownloadFetching ? (
          <Text ta="center" py="2rem" c="var(--color-label-secondary)" size="sm">
            正在获取种子文件列表...
          </Text>
        ) : overwriteConfirm ? (
          <Stack gap="md">
            <Text size="sm" c="var(--color-label-secondary)">
              以下文件已存在于库目录中，继续下载将覆盖现有文件：
            </Text>
            <ScrollArea.Autosize mah={300}>
              <Stack gap={4}>
                {redownloadFiles
                  .filter(
                    (f) => redownloadSelected.has(f.index) && redownloadExisting.has(f.name)
                  )
                  .map((f) => (
                    <Text key={f.index} size="sm" c="var(--color-danger)">
                      {f.name}
                    </Text>
                  ))}
              </Stack>
            </ScrollArea.Autosize>
            <Group justify="flex-end" gap="0.5rem">
              <Button variant="default" onClick={closeRedownloadModal}>
                取消
              </Button>
              <Button color="danger" loading={redownloadSubmitting} onClick={handleRedownloadConfirm}>
                覆盖下载
              </Button>
            </Group>
          </Stack>
        ) : (
          <Stack gap="md">
            <Text size="xs" c="var(--color-label-secondary)">
              重新下载: {redownloadTarget?.title} · 共 {redownloadFiles.length} 个文件
            </Text>
            <ScrollArea.Autosize mah={400}>
              <Stack gap={0}>
                {redownloadFiles.map((f) => {
                  const exists = redownloadExisting.has(f.name);
                  return (
                    <Group
                      key={f.index}
                      gap="xs"
                      py="6px"
                      wrap="nowrap"
                      style={{ borderBottom: "1px solid var(--color-separator)" }}
                    >
                      <Checkbox
                        checked={redownloadSelected.has(f.index)}
                        onChange={() => {
                          setRedownloadSelected((prev) => {
                            const next = new Set(prev);
                            if (next.has(f.index)) next.delete(f.index);
                            else next.add(f.index);
                            return next;
                          });
                        }}
                      />
                      <Text
                        size="sm"
                        truncate
                        style={{ flex: 1 }}
                        c={exists ? "var(--color-danger)" : undefined}
                      >
                        {f.name}
                        {exists ? " (已存在)" : ""}
                      </Text>
                      <Text size="sm" c="var(--color-label-tertiary)" style={{ flexShrink: 0 }}>
                        {formatSize(f.size)}
                      </Text>
                    </Group>
                  );
                })}
              </Stack>
            </ScrollArea.Autosize>
            <Group justify="space-between">
              <Button
                variant="default"
                size="xs"
                onClick={() => {
                  if (redownloadSelected.size === redownloadFiles.length)
                    setRedownloadSelected(new Set());
                  else setRedownloadSelected(new Set(redownloadFiles.map((f) => f.index)));
                }}
              >
                {redownloadSelected.size === redownloadFiles.length ? "取消全选" : "全选"}
              </Button>
              <Group gap="0.5rem">
                <Button variant="default" onClick={closeRedownloadModal}>
                  取消
                </Button>
                <Button
                  disabled={redownloadSelected.size === 0}
                  loading={redownloadSubmitting}
                  onClick={handleRedownloadConfirm}
                >
                  确认下载 ({redownloadSelected.size})
                </Button>
              </Group>
            </Group>
          </Stack>
        )}
      </Modal>
    </ScrollArea>
  );
}
