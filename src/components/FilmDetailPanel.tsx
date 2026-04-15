// src/components/FilmDetailPanel.tsx
import { useState, useEffect } from "react";
import {
  Box,
  Button,
  Checkbox,
  CloseButton,
  Divider,
  Group,
  Image,
  Loader,
  Modal,
  ScrollArea,
  Stack,
  Text,
  Title,
} from "@mantine/core";
import { search, download } from "../lib/tauri";
import { formatSize } from "../lib/format";
import type {
  MovieListItem,
  ScoredTorrent,
  TorrentFileInfo,
} from "../lib/tauri";

const resolutionLabel = (r: ScoredTorrent["resolution"]): string =>
  ({
    p2160: "4K",
    p1080: "1080p",
    p720: "720p",
    p480: "480p",
    sd: "SD",
    unknown: "",
  }[r] ?? "");

const sourceLabel = (s: ScoredTorrent["source_kind"]): string =>
  ({
    remux: "Remux",
    bluray: "Bluray",
    webdl: "WEB-DL",
    webrip: "WEBRip",
    hdtv: "HDTV",
    ts: "TS",
    cam: "CAM",
    unknown: "",
  }[s] ?? "");

const codecLabel = (c: ScoredTorrent["codec"]): string =>
  ({ x265: "x265", x264: "x264", av1: "AV1", unknown: "" }[c] ?? "");

const qualityTags = (r: ScoredTorrent): string =>
  [
    resolutionLabel(r.resolution),
    sourceLabel(r.source_kind),
    codecLabel(r.codec),
    r.hdr ? "HDR" : "",
  ]
    .filter(Boolean)
    .join(" · ");

function renderBreakdownLine(label: string, value: number, note: string) {
  const sign = value >= 0 ? "+" : "";
  return (
    <div style={{ display: "flex", justifyContent: "space-between" }}>
      <span>{label}</span>
      <span>
        {sign}
        {value}
        {note && (
          <span style={{ marginLeft: 8, color: "var(--color-label-tertiary)" }}>
            ({note})
          </span>
        )}
      </span>
    </div>
  );
}

function TorrentSearchModal({
  film,
  opened,
  onClose,
}: {
  film: MovieListItem;
  opened: boolean;
  onClose: () => void;
}) {
  const [results, setResults] = useState<ScoredTorrent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [fetching, setFetching] = useState<Set<string>>(new Set());
  const [started, setStarted] = useState<Set<string>>(new Set());
  const [filePickResult, setFilePickResult] = useState<ScoredTorrent | null>(null);
  const [detailIndex, setDetailIndex] = useState<number | null>(null);
  const [fileList, setFileList] = useState<TorrentFileInfo[]>([]);
  const [selectedFiles, setSelectedFiles] = useState<Set<number>>(new Set());
  const [submitting, setSubmitting] = useState(false);

  const year = film.year ? parseInt(film.year) : undefined;

  useEffect(() => {
    if (!opened) return;
    setLoading(true);
    setError("");
    search
      .movie(film.title, year, film.id)
      .then(setResults)
      .catch((e) => setError(String(e)))
      .finally(() => setLoading(false));
  }, [opened, film.title, year, film.id]);

  const handleFetchFiles = async (r: ScoredTorrent) => {
    const target = r.magnet ?? r.torrent_url;
    if (!target) return;
    setFetching((prev) => new Set(prev).add(target));
    try {
      const files = await download.getTorrentFiles(target);
      setFileList(files);
      setSelectedFiles(new Set(files.map((f) => f.index)));
      setFilePickResult(r);
    } catch (e) {
      console.error("fetch torrent files failed:", e);
    } finally {
      setFetching((prev) => {
        const next = new Set(prev);
        next.delete(target);
        return next;
      });
    }
  };

  const handleConfirmDownload = async () => {
    if (!filePickResult) return;
    const target = filePickResult.magnet ?? filePickResult.torrent_url;
    if (!target) return;
    if (!film.director || !film.director.trim()) {
      setError("无法确定导演，请重试或稍后再试 (TMDB credits 未就绪)");
      return;
    }
    setSubmitting(true);
    try {
      await download.startDownload({
        title: film.title,
        target,
        director: film.director,
        tmdbId: film.id,
        year: year,
        genres: [],
        quality: resolutionLabel(filePickResult.resolution), // was filePickResult.quality
        onlyFiles: [...selectedFiles],
      });
      setStarted((prev) => new Set(prev).add(target));
      setFilePickResult(null);
    } catch (e) {
      console.error("download failed:", e);
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <>
      <Modal
        opened={opened}
        onClose={onClose}
        title={`搜索资源: ${film.title}`}
        size="md"
        centered
      >
        {loading && (
          <Group gap="xs">
            <Loader size="xs" />
            <Text size="sm" c="var(--color-label-secondary)">
              搜索中... (YTS · Nyaa · 1337x)
            </Text>
          </Group>
        )}

        {error && (
          <Text size="sm" c="var(--color-danger)">
            {error.includes("NoResults") ? "未找到资源" : `搜索失败: ${error}`}
          </Text>
        )}

        {!loading && !error && results.length === 0 && (
          <Text size="sm" c="var(--color-label-tertiary)">
            未找到资源
          </Text>
        )}

        <Stack gap={0}>
          {results.map((r, i) => {
            const target = r.magnet ?? r.torrent_url ?? "";
            const isStarted = started.has(target);
            const showDetail = detailIndex === i;
            return (
              <Box
                key={i}
                py="8px"
                style={{ borderBottom: "1px solid var(--color-separator)" }}
              >
                <Group justify="space-between" wrap="nowrap" gap="md">
                  <Group gap="sm" wrap="nowrap" style={{ flex: 1, minWidth: 0 }}>
                    <Text size="sm" fw={600} c="var(--color-accent)">
                      ⭐ {r.score}
                    </Text>
                    <Text size="sm" truncate>
                      {qualityTags(r)}
                    </Text>
                    {r.size_bytes != null && (
                      <Text size="xs" c="var(--color-label-secondary)">
                        {formatSize(r.size_bytes)}
                      </Text>
                    )}
                    <Text size="xs" c="var(--color-label-secondary)">
                      ▸ {r.seeders} seeds
                    </Text>
                  </Group>
                  <Group gap="xs" wrap="nowrap" style={{ flexShrink: 0 }}>
                    <Button
                      size="compact-xs"
                      variant="default"
                      onClick={() => setDetailIndex(showDetail ? null : i)}
                    >
                      {showDetail ? "收起" : "详情"}
                    </Button>
                    {isStarted ? (
                      <Text size="xs" c="var(--color-accent)">
                        下载中
                      </Text>
                    ) : fetching.has(target) ? (
                      <Loader size="xs" />
                    ) : (
                      <Button
                        size="compact-xs"
                        disabled={!target}
                        onClick={() => handleFetchFiles(r)}
                      >
                        下载
                      </Button>
                    )}
                  </Group>
                </Group>
                <Text size="xs" c="var(--color-label-tertiary)" truncate mt={4}>
                  [{r.source}] {r.raw_title}
                </Text>
                {showDetail && (
                  <Box
                    mt="xs"
                    p="xs"
                    style={{
                      background: "var(--color-surface-2)",
                      border: "1px solid var(--color-separator)",
                      fontFamily: "monospace",
                      fontSize: 11,
                    }}
                  >
                    {renderBreakdownLine("seeders", r.breakdown.seeders, `${r.seeders} peers`)}
                    {renderBreakdownLine(
                      "resolution",
                      r.breakdown.resolution,
                      resolutionLabel(r.resolution) || "unknown",
                    )}
                    {renderBreakdownLine(
                      "source",
                      r.breakdown.source,
                      sourceLabel(r.source_kind) || "unknown",
                    )}
                    {renderBreakdownLine(
                      "codec",
                      r.breakdown.codec,
                      codecLabel(r.codec) || "unknown",
                    )}
                    {renderBreakdownLine(
                      "size",
                      r.breakdown.size,
                      r.size_bytes != null ? formatSize(r.size_bytes) : "—",
                    )}
                    {renderBreakdownLine(
                      "group",
                      r.breakdown.group,
                      r.release_group ?? "—",
                    )}
                    {renderBreakdownLine("hdr", r.breakdown.hdr, r.hdr ? "yes" : "no")}
                    <div
                      style={{
                        marginTop: 4,
                        paddingTop: 4,
                        borderTop: "1px solid var(--color-separator)",
                      }}
                    >
                      {renderBreakdownLine("total", r.score, "")}
                    </div>
                  </Box>
                )}
              </Box>
            );
          })}
        </Stack>

        <Button variant="default" fullWidth mt="md" onClick={onClose}>
          关闭
        </Button>
      </Modal>

      {/* File selection modal */}
      <Modal
        opened={!!filePickResult}
        onClose={() => !submitting && setFilePickResult(null)}
        title="选择下载文件"
        size="lg"
        centered
      >
        <Text size="xs" c="var(--color-label-secondary)" mb="md">
          {filePickResult ? qualityTags(filePickResult) : ""} · 共 {fileList.length} 个文件
        </Text>
        <ScrollArea.Autosize mah={400} mb="md">
          <Stack gap={0}>
            {fileList.map((f) => (
              <Group
                key={f.index}
                gap="xs"
                py="6px"
                wrap="nowrap"
                style={{ borderBottom: "1px solid var(--color-separator)" }}
              >
                <Checkbox
                  checked={selectedFiles.has(f.index)}
                  onChange={() => {
                    setSelectedFiles((prev) => {
                      const next = new Set(prev);
                      if (next.has(f.index)) next.delete(f.index);
                      else next.add(f.index);
                      return next;
                    });
                  }}
                />
                <Text size="sm" truncate style={{ flex: 1 }}>
                  {f.name}
                </Text>
                <Text size="sm" c="var(--color-label-tertiary)" style={{ flexShrink: 0 }}>
                  {formatSize(f.size)}
                </Text>
              </Group>
            ))}
          </Stack>
        </ScrollArea.Autosize>

        <Group justify="space-between">
          <Button
            variant="default"
            size="xs"
            onClick={() => {
              if (selectedFiles.size === fileList.length) setSelectedFiles(new Set());
              else setSelectedFiles(new Set(fileList.map((f) => f.index)));
            }}
          >
            {selectedFiles.size === fileList.length ? "取消全选" : "全选"}
          </Button>
          <Group gap="xs">
            <Button variant="default" disabled={submitting} onClick={() => setFilePickResult(null)}>
              取消
            </Button>
            <Button
              disabled={selectedFiles.size === 0}
              loading={submitting}
              onClick={handleConfirmDownload}
            >
              确认下载 ({selectedFiles.size})
            </Button>
          </Group>
        </Group>
      </Modal>
    </>
  );
}

interface FilmDetailPanelProps {
  film: MovieListItem;
  onClose: () => void;
}

export function FilmDetailPanel({ film, onClose }: FilmDetailPanelProps) {
  const [showTorrentModal, setShowTorrentModal] = useState(false);

  return (
    <>
      <Box
        w="100%"
        bg="var(--color-bg-secondary)"
        px="1.25rem"
        pt="1.25rem"
        pb="2rem"
      >
        <Stack gap="0.75rem">
          <Group justify="flex-end">
            <CloseButton onClick={onClose} />
          </Group>

          {film.poster_path && (
            <Image
              src={`https://image.tmdb.org/t/p/w300${film.poster_path}`}
              alt={film.title}
              w="100%"
              fit="contain"
            />
          )}

          <Box>
            <Title order={2} fz="1rem" fw={700} style={{ letterSpacing: "-0.02em" }}>
              {film.title}
            </Title>
            {film.original_title !== film.title && (
              <Text size="xs" c="var(--color-label-tertiary)" mt={2}>
                {film.original_title}
              </Text>
            )}
          </Box>

          <Group gap="0.5rem">
            {film.year && (
              <Text size="xs" c="var(--color-label-secondary)">
                {film.year}
              </Text>
            )}
            <Text size="xs" c="var(--color-label-tertiary)">
              ·
            </Text>
            <Text size="xs" c="var(--color-accent)" fw={500}>
              ★ {film.vote_average.toFixed(1)}
            </Text>
          </Group>

          {(film.director || (film.cast && film.cast.length > 0)) && (
            <Box>
              {film.director && (
                <Text size="xs" c="var(--color-label-secondary)" lh={1.5}>
                  导演: {film.director}
                </Text>
              )}
              {film.cast && film.cast.length > 0 && (
                <Text size="xs" c="var(--color-label-secondary)" lh={1.5}>
                  主演: {film.cast.join(", ")}
                </Text>
              )}
            </Box>
          )}

          <Text size="sm" c="var(--color-label-secondary)" lh={1.6}>
            {film.overview || "暂无简介。"}
          </Text>

          <Divider />

          <Button variant="default" size="xs" w="fit-content" onClick={() => setShowTorrentModal(true)}>
            搜索资源
          </Button>
        </Stack>
      </Box>

      <TorrentSearchModal
        film={film}
        opened={showTorrentModal}
        onClose={() => setShowTorrentModal(false)}
      />
    </>
  );
}
