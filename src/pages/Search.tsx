import { useState, useEffect, useCallback } from "react";
import {
  Box,
  Button,
  Checkbox,
  Divider,
  Flex,
  Group,
  Image,
  NumberInput,
  Pill,
  Popover,
  Radio,
  ScrollArea,
  Select,
  Stack,
  Text,
  TextInput,
  Title,
  UnstyledButton,
} from "@mantine/core";
import {
  tmdb,
  config,
  type MovieListItem,
  type TmdbGenre,
  type SearchFilters,
} from "../lib/tauri";
import { FilmDetailPanel } from "../components/FilmDetailPanel";
import { useBackendEvent, BackendEvent } from "../lib/useBackendEvent";

const SORT_OPTIONS = [
  { value: "vote_average.desc", label: "按评分排序" },
  { value: "popularity.desc", label: "按热度排序" },
  { value: "release_date.desc", label: "按年份排序" },
];

const YEAR_MIN = 1920;
const YEAR_MAX = new Date().getFullYear();

// ── Filter triggers ──────────────────────────────────────────────

function YearFilterChip({
  yearFrom,
  yearTo,
  onChange,
}: {
  yearFrom?: number;
  yearTo?: number;
  onChange: (from?: number, to?: number) => void;
}) {
  const [opened, setOpened] = useState(false);
  const active = !!(yearFrom || yearTo);
  const label = active ? `${yearFrom ?? "?"} – ${yearTo ?? "?"}` : "年代";

  return (
    <Popover opened={opened} onChange={setOpened} position="bottom-start" shadow="md" withinPortal>
      <Popover.Target>
        <Pill
          size="md"
          withRemoveButton={active}
          onRemove={() => {
            onChange(undefined, undefined);
          }}
          onClick={() => setOpened((o) => !o)}
          styles={{
            root: {
              cursor: "pointer",
              background: active ? "var(--color-accent-soft)" : "var(--color-bg-control)",
              color: active ? "var(--color-accent)" : "var(--color-label-secondary)",
            },
          }}
        >
          {label}
        </Pill>
      </Popover.Target>
      <Popover.Dropdown>
        <Stack gap="xs">
          <Text size="xs" tt="uppercase" c="var(--color-label-quaternary)">
            年代范围
          </Text>
          <Group gap="xs" wrap="nowrap">
            <NumberInput
              size="xs"
              w={90}
              placeholder="起始"
              min={YEAR_MIN}
              max={yearTo ?? YEAR_MAX}
              value={yearFrom ?? ""}
              onChange={(v) => onChange(typeof v === "number" ? v : undefined, yearTo)}
              hideControls
            />
            <Text c="var(--color-label-quaternary)">—</Text>
            <NumberInput
              size="xs"
              w={90}
              placeholder="结束"
              min={yearFrom ?? YEAR_MIN}
              max={YEAR_MAX}
              value={yearTo ?? ""}
              onChange={(v) => onChange(yearFrom, typeof v === "number" ? v : undefined)}
              hideControls
            />
          </Group>
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
}

function GenreFilterChip({
  genres,
  selected,
  onChange,
}: {
  genres: TmdbGenre[];
  selected: number[];
  onChange: (ids: number[]) => void;
}) {
  const [opened, setOpened] = useState(false);
  const [draft, setDraft] = useState<string[]>(selected.map(String));

  useEffect(() => {
    setDraft(selected.map(String));
  }, [selected]);

  const selectedNames = genres.filter((g) => selected.includes(g.id)).map((g) => g.name);

  return (
    <Popover opened={opened} onChange={setOpened} position="bottom-start" shadow="md" withinPortal>
      <Popover.Target>
        <Group gap="0.3rem" wrap="wrap">
          {selectedNames.length === 0 ? (
            <Pill
              size="md"
              onClick={() => setOpened((o) => !o)}
              styles={{
                root: {
                  cursor: "pointer",
                  background: "var(--color-bg-control)",
                  color: "var(--color-label-secondary)",
                },
              }}
            >
              类型
            </Pill>
          ) : (
            selectedNames.map((name, i) => (
              <Pill
                key={selected[i]}
                size="md"
                withRemoveButton
                onRemove={() =>
                  onChange(selected.filter((id) => id !== selected[i]))
                }
                onClick={() => setOpened((o) => !o)}
                styles={{
                  root: {
                    cursor: "pointer",
                    background: "var(--color-accent-soft)",
                    color: "var(--color-accent)",
                  },
                }}
              >
                {name}
              </Pill>
            ))
          )}
        </Group>
      </Popover.Target>
      <Popover.Dropdown>
        <Stack gap="xs" w={200}>
          <Text size="xs" tt="uppercase" c="var(--color-label-quaternary)">
            选择类型
          </Text>
          <ScrollArea.Autosize mah={200}>
            <Checkbox.Group value={draft} onChange={setDraft}>
              <Stack gap={4}>
                {genres.map((g) => (
                  <Checkbox key={g.id} value={String(g.id)} label={g.name} size="xs" />
                ))}
              </Stack>
            </Checkbox.Group>
          </ScrollArea.Autosize>
          <Button
            size="xs"
            onClick={() => {
              onChange(draft.map(Number));
              setOpened(false);
            }}
          >
            确定
          </Button>
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
}

function RatingFilterChip({
  current,
  onChange,
}: {
  current?: number;
  onChange: (v?: number) => void;
}) {
  const [opened, setOpened] = useState(false);
  const ratings = Array.from({ length: 20 }, (_, i) => (i + 1) * 0.5);

  return (
    <Popover opened={opened} onChange={setOpened} position="bottom-start" shadow="md" withinPortal>
      <Popover.Target>
        <Pill
          size="md"
          withRemoveButton={!!current}
          onRemove={() => onChange(undefined)}
          onClick={() => setOpened((o) => !o)}
          styles={{
            root: {
              cursor: "pointer",
              background: current ? "var(--color-accent-soft)" : "var(--color-bg-control)",
              color: current ? "var(--color-accent)" : "var(--color-label-secondary)",
            },
          }}
        >
          {current ? `≥ ${current}` : "评分"}
        </Pill>
      </Popover.Target>
      <Popover.Dropdown>
        <Stack gap="xs">
          <Text size="xs" tt="uppercase" c="var(--color-label-quaternary)">
            最低评分
          </Text>
          <Select
            size="xs"
            placeholder="不限"
            data={ratings.map((r) => ({ value: String(r), label: r.toFixed(1) }))}
            value={current ? String(current) : null}
            onChange={(v) => {
              onChange(v ? Number(v) : undefined);
              setOpened(false);
            }}
            clearable
          />
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
}

function SortFilterChip({
  current,
  onChange,
}: {
  current: string;
  onChange: (v: string) => void;
}) {
  const [opened, setOpened] = useState(false);
  const label = SORT_OPTIONS.find((o) => o.value === current)?.label ?? "排序";

  return (
    <Popover opened={opened} onChange={setOpened} position="bottom-start" shadow="md" withinPortal>
      <Popover.Target>
        <Pill
          size="md"
          onClick={() => setOpened((o) => !o)}
          styles={{
            root: {
              cursor: "pointer",
              background: "var(--color-accent-soft)",
              color: "var(--color-accent)",
            },
          }}
        >
          {label}
        </Pill>
      </Popover.Target>
      <Popover.Dropdown>
        <Stack gap="xs">
          <Text size="xs" tt="uppercase" c="var(--color-label-quaternary)">
            排序方式
          </Text>
          <Radio.Group value={current} onChange={(v) => { onChange(v); setOpened(false); }}>
            <Stack gap={4}>
              {SORT_OPTIONS.map((o) => (
                <Radio key={o.value} value={o.value} label={o.label} size="xs" />
              ))}
            </Stack>
          </Radio.Group>
        </Stack>
      </Popover.Dropdown>
    </Popover>
  );
}

// ── Main page ────────────────────────────────────────────────────

export default function Search() {
  const [apiKey, setApiKey] = useState("");
  const [query, setQuery] = useState("");
  const [genres, setGenres] = useState<TmdbGenre[]>([]);
  const [results, setResults] = useState<MovieListItem[]>([]);
  const [selected, setSelected] = useState<MovieListItem | null>(null);
  const [loading, setLoading] = useState(false);
  const [page, setPage] = useState(1);
  const [hasMore, setHasMore] = useState(false);

  const [yearFrom, setYearFrom] = useState<number | undefined>();
  const [yearTo, setYearTo] = useState<number | undefined>();
  const [genreIds, setGenreIds] = useState<number[]>([]);
  const [minRating, setMinRating] = useState<number | undefined>();
  const [sortBy, setSortBy] = useState("vote_average.desc");

  const loadApiConfig = useCallback(() => {
    config.get().then((cfg) => {
      setApiKey(cfg.tmdb.api_key);
      if (cfg.tmdb.api_key) {
        tmdb.listGenres(cfg.tmdb.api_key).then(setGenres).catch(() => {});
      }
    });
  }, []);

  useEffect(loadApiConfig, [loadApiConfig]);
  useBackendEvent(BackendEvent.CONFIG_CHANGED, loadApiConfig);

  const buildFilters = useCallback(
    (p = 1): SearchFilters => ({
      year_from: yearFrom,
      year_to: yearTo,
      genre_ids: genreIds,
      min_rating: minRating,
      sort_by: sortBy,
      page: p,
    }),
    [yearFrom, yearTo, genreIds, minRating, sortBy]
  );

  const runSearch = useCallback(
    async (q: string, p: number, append: boolean) => {
      if (!apiKey) return;
      setLoading(true);
      try {
        const filters = buildFilters(p);
        const items = q.trim()
          ? await tmdb.searchMovies(apiKey, q.trim(), filters)
          : await tmdb.discoverMovies(apiKey, filters);
        setResults(append ? (prev) => [...prev, ...items] : items);
        setPage(p);
        setHasMore(items.length >= 20);

        const top3Ids = items.slice(0, 3).map((m) => m.id);
        if (top3Ids.length > 0) {
          tmdb
            .enrichCredits(apiKey, top3Ids)
            .then((credits) => {
              const map = new Map(credits.map((c) => [c.id, c]));
              setResults((prev) =>
                prev.map((m) => {
                  const c = map.get(m.id);
                  return c ? { ...m, director: c.director ?? undefined, cast: c.cast } : m;
                })
              );
            })
            .catch(() => {});
        }
      } catch {
        /* */
      } finally {
        setLoading(false);
      }
    },
    [apiKey, buildFilters]
  );

  const doSearch = useCallback(() => runSearch(query, 1, false), [query, runSearch]);
  const loadMore = () => runSearch(query, page + 1, true);

  return (
    <Flex h="100%" style={{ overflow: "hidden" }}>
      {/* Left: search + list */}
      <Flex direction="column" style={{ flex: 1, overflow: "hidden" }}>
        <Box px="1.5rem" pt="1.4rem">
          <Title order={1} mb="1.1rem" fz="1.6rem" fw={700} style={{ letterSpacing: "-0.035em" }}>
            搜索
          </Title>

          <TextInput
            leftSection={<span>⌕</span>}
            placeholder="电影名称、导演… (回车搜索)"
            value={query}
            onChange={(e) => setQuery(e.currentTarget.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && !e.nativeEvent.isComposing) doSearch();
            }}
            mb="0.7rem"
          />

          <Group gap="0.4rem" mb="1rem" wrap="wrap">
            <YearFilterChip
              yearFrom={yearFrom}
              yearTo={yearTo}
              onChange={(f, t) => {
                setYearFrom(f);
                setYearTo(t);
              }}
            />
            <GenreFilterChip
              genres={genres}
              selected={genreIds}
              onChange={setGenreIds}
            />
            <RatingFilterChip current={minRating} onChange={setMinRating} />
            <SortFilterChip current={sortBy} onChange={setSortBy} />
          </Group>
        </Box>

        <Divider mx="1.5rem" />

        <ScrollArea style={{ flex: 1 }}>
          <Box px="1.5rem" py="0.9rem">
            {!apiKey && (
              <Text size="sm" c="var(--color-label-tertiary)">
                请先在设置中配置 TMDB API Key。
              </Text>
            )}

            <Stack gap={4}>
              {results.map((m) => (
                <UnstyledButton
                  key={m.id}
                  onClick={() => {
                    setSelected(m);
                    if (!m.director && apiKey) {
                      tmdb
                        .enrichCredits(apiKey, [m.id])
                        .then(([credits]) => {
                          if (!credits) return;
                          const enriched = {
                            ...m,
                            director: credits.director ?? undefined,
                            cast: credits.cast,
                          };
                          setResults((prev) =>
                            prev.map((x) => (x.id === m.id ? enriched : x))
                          );
                          setSelected((current) =>
                            current && current.id === m.id ? enriched : current
                          );
                        })
                        .catch(() => {});
                    }
                  }}
                  p="0.65rem 0.5rem"
                  style={{
                    borderRadius: 6,
                    background:
                      selected?.id === m.id ? "var(--color-bg-elevated)" : "transparent",
                  }}
                >
                  <Group gap="0.75rem" wrap="nowrap" align="flex-start">
                    {m.poster_path && (
                      <Image
                        src={`https://image.tmdb.org/t/p/w92${m.poster_path}`}
                        alt=""
                        w={46}
                        h={69}
                        radius={4}
                        fit="cover"
                      />
                    )}
                    <Box style={{ flex: 1, minWidth: 0 }}>
                      <Text fz="0.85rem" fw={500} truncate>
                        {m.title}
                      </Text>
                      <Text fz="0.72rem" c="var(--color-label-tertiary)" mt={2}>
                        {m.year ?? "—"}
                        {m.vote_average > 0 && (
                          <Text component="span" ml="0.5rem" c="var(--color-accent)">
                            ★ {m.vote_average.toFixed(1)}
                          </Text>
                        )}
                      </Text>
                      {m.director && (
                        <Text fz="0.68rem" c="var(--color-label-quaternary)" mt={2} truncate>
                          导演: {m.director}
                        </Text>
                      )}
                      {m.cast && m.cast.length > 0 && (
                        <Text fz="0.68rem" c="var(--color-label-quaternary)" mt={1} truncate>
                          主演: {m.cast.join(", ")}
                        </Text>
                      )}
                    </Box>
                  </Group>
                </UnstyledButton>
              ))}
            </Stack>

            {hasMore && !loading && (
              <Box ta="center" my="0.75rem">
                <Button variant="default" size="xs" onClick={loadMore}>
                  加载更多
                </Button>
              </Box>
            )}

            {loading && (
              <Text ta="center" c="var(--color-label-tertiary)" size="sm">
                搜索中…
              </Text>
            )}
          </Box>
        </ScrollArea>
      </Flex>

      {/* Right panel */}
      {selected && (
        <ScrollArea
          w={380}
          style={{
            flexShrink: 0,
            borderLeft: "1px solid var(--color-separator)",
          }}
        >
          <FilmDetailPanel film={selected} onClose={() => setSelected(null)} />
        </ScrollArea>
      )}
    </Flex>
  );
}
