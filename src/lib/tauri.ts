// src/lib/tauri.ts
import { invoke } from "@tauri-apps/api/core";

// ── TMDB (M1) ─────────────────────────────────────────────────────
export interface SearchFilters {
  year_from?: number;
  year_to?: number;
  genre_ids: number[];
  min_rating?: number;
  sort_by?: string;
  page?: number;
}

export interface MovieListItem {
  id: number;
  title: string;
  original_title: string;
  year: string;
  overview: string;
  vote_average: number;
  poster_path: string | null;
  genre_ids: number[];
  director?: string;
  cast?: string[];
}

export interface MovieCreditsEnriched {
  id: number;
  director: string | null;
  cast: string[];
}

export interface TmdbGenre { id: number; name: string; }

export interface TmdbCrewMember { id: number; name: string; job: string; department: string; }
export interface TmdbCastMember { id: number; name: string; character: string; }
export interface TmdbMovieCredits {
  tmdb_id: number; title: string; original_title: string | null;
  year: number | null; overview: string | null; vote_average: number | null;
  poster_path: string | null; crew: TmdbCrewMember[]; cast: TmdbCastMember[];
}

// ── Config ────────────────────────────────────────────────────────
export interface MusicTrack { src: string; name: string; }

export interface AppConfig {
  tools: { alass: string; ffmpeg: string };
  download: { max_concurrent: number; enable_dht: boolean; persist_session: boolean };
  search: { rate_limit_secs: number };
  subtitle: { default_lang: string };
  opensubtitles: { api_key: string; username: string; password: string };
  tmdb: { api_key: string };
  library: { root_dir: string };
  music: { enabled: boolean; mode: "sequential" | "random"; playlist: MusicTrack[] };
  cache: { max_entries: number };
  sync: { endpoint: string; bucket: string; access_key: string; secret_key: string };
}

// ── Knowledge Base types ─────────────────────────────────────────
export interface EntrySummary {
  id: number;
  name: string;
  tags: string[];
  updated_at: string;
}

export interface EntryDetail {
  id: number;
  name: string;
  wiki: string;
  tags: string[];
  relations: RelationEntry[];
  created_at: string;
  updated_at: string;
}

export interface RelationEntry {
  id: number;
  target_id: number;
  target_name: string;
  direction: string;
  relation_type: string;
}

export interface GraphNode { id: string; label: string; weight: number; }
export interface GraphLink { source: string; target: string; relation_type: string; }
export interface GraphData { nodes: GraphNode[]; links: GraphLink[]; }

// ── Library Items ───────────────────────────────────────────────
export interface LibraryItemSummary {
  id: number;
  file_path: string;
  file_size: number | null;
  duration_secs: number | null;
  video_codec: string | null;
  audio_codec: string | null;
  resolution: string | null;
  added_at: string;
}

export interface LibraryItemDetail extends LibraryItemSummary {
  assets: LibraryAssetEntry[];
}

export interface LibraryAssetEntry {
  id: number;
  asset_type: string;
  file_path: string;
  lang: string | null;
  created_at: string;
}

export interface LibraryStats {
  total_items: number;
  total_file_size: number;
  by_resolution: StatEntry[];
}

export interface StatEntry {
  label: string;
  count: number;
}

export interface ScanResult {
  added: number;
  skipped: number;
  errors: string[];
}

// ── Download ─────────────────────────────────────────────────────

export interface DownloadRecord {
  id: number;
  tmdb_id: number | null;
  title: string;
  director: string | null;
  quality: string | null;
  target: string;
  status: "pending" | "downloading" | "paused" | "completed" | "failed";
  torrent_id: number | null;
  progress_bytes: number;
  total_bytes: number;
  error_message: string | null;
  started_at: string;
  completed_at: string | null;
}

export interface TorrentFileInfo {
  index: number;
  name: string;
  size: number;
}

export interface StartDownloadRequest {
  title: string;
  target: string;
  director: string;
  tmdbId: number;
  year?: number;
  genres?: string[];
  quality?: string;
  onlyFiles?: number[];
}

export interface FileMediaInfo {
  file_size: number | null;
  duration_secs: number | null;
  format_name: string | null;
  bit_rate: number | null;
  streams: FileStreamInfo[];
}

export interface FileStreamInfo {
  index: number;
  codec_type: string;
  codec_name: string;
  width: number | null;
  height: number | null;
  frame_rate: string | null;
  bit_rate: number | null;
  channels: number | null;
  sample_rate: string | null;
  language: string | null;
  title: string | null;
}

export interface IndexEntry {
  tmdb_id: number;
  title: string;
  director: string;
  director_display: string;
  year: number | null;
  genres: string[];
  path: string;
  files: string[];
  added_at: string;
  // TMDB enriched data (lazy-loaded, cached in index)
  poster_url?: string | null;
  overview?: string | null;
  rating?: number | null;
  credits?: Record<string, string[]>;
  original_title?: string | null;
  media_info?: Record<string, FileMediaInfo>;
}

export interface MovieResult {
  title: string;
  year: number;
  quality: string;
  magnet: string | null;
  torrent_url: string | null;
  seeds: number;
}

export interface SubtitleStreamInfo {
  index: number;
  codec_name: string;
  duration: number;
  language: string | null;
  title: string | null;
}

export interface MediaInfo {
  file_path: string;
  file_size: number | null;
  duration_secs: number | null;
  format_name: string | null;
  bit_rate: number | null;
  streams: StreamInfo[];
}

export interface StreamInfo {
  index: number;
  codec_type: string;
  codec_name: string;
  width: number | null;
  height: number | null;
  frame_rate: string | null;
  bit_rate: number | null;
  channels: number | null;
  sample_rate: string | null;
  language: string | null;
  title: string | null;
}

// ── Invoke wrappers ───────────────────────────────────────────────
export const tmdb = {
  searchMovies: (apiKey: string, query: string, filters: SearchFilters) =>
    invoke<MovieListItem[]>("search_movies", { apiKey, query, filters }),
  discoverMovies: (apiKey: string, filters: SearchFilters) =>
    invoke<MovieListItem[]>("discover_movies", { apiKey, filters }),
  listGenres: (apiKey: string) => invoke<TmdbGenre[]>("list_genres", { apiKey }),
  getMovieCredits: (apiKey: string, tmdbId: number) =>
    invoke<TmdbMovieCredits>("get_tmdb_movie_credits", { apiKey, tmdbId }),
  enrichCredits: (apiKey: string, ids: number[]) =>
    invoke<MovieCreditsEnriched[]>("enrich_movie_credits", { apiKey, ids }),
};

export const config = {
  get: () => invoke<AppConfig>("get_config"),
  save: (newConfig: AppConfig) => invoke<void>("save_config_cmd", { newConfig }),
  exportConfig: (path: string) => invoke<void>("export_config", { path }),
  importConfig: (path: string) => invoke<void>("import_config", { path }),
  getCachePath: () => invoke<string>("get_cache_path"),
};

export const dataIO = {
  exportKnowledgeBase: (path: string) => invoke<void>("export_knowledge_base", { path }),
  importKnowledgeBase: (path: string) => invoke<string>("import_knowledge_base", { path }),
  exportKnowledgeBaseS3: () => invoke<void>("export_knowledge_base_s3"),
  importKnowledgeBaseS3: () => invoke<string>("import_knowledge_base_s3"),
  exportConfigS3: () => invoke<void>("export_config_s3"),
  importConfigS3: () => invoke<void>("import_config_s3"),
  testS3Connection: () => invoke<string>("test_s3_connection"),
};

// ── Knowledge Base ──────────────────────────────────────────────
export const kb = {
  listEntries: (query?: string, tag?: string) =>
    invoke<EntrySummary[]>("list_entries", { query, tag }),
  getEntry: (id: number) =>
    invoke<EntryDetail>("get_entry", { id }),
  createEntry: (name: string) =>
    invoke<number>("create_entry", { name }),
  updateEntryName: (id: number, name: string) =>
    invoke<void>("update_entry_name", { id, name }),
  updateEntryWiki: (id: number, wiki: string) =>
    invoke<void>("update_entry_wiki", { id, wiki }),
  deleteEntry: (id: number) =>
    invoke<void>("delete_entry", { id }),
  addTag: (entryId: number, tag: string) =>
    invoke<void>("add_entry_tag", { entryId, tag }),
  removeTag: (entryId: number, tag: string) =>
    invoke<void>("remove_entry_tag", { entryId, tag }),
  listAllTags: () =>
    invoke<string[]>("list_all_tags"),
  addRelation: (fromId: number, toId: number, relationType: string) =>
    invoke<number>("add_relation", { fromId, toId, relationType }),
  removeRelation: (id: number) =>
    invoke<void>("remove_relation", { id }),
  listRelationTypes: () =>
    invoke<string[]>("list_relation_types"),
  getGraphData: (centerId?: number) =>
    invoke<GraphData>("get_graph_data", { centerId }),
};

// ── Library (film library — file index system) ──────────────────
export const library = {
  // Library items (DB-backed)
  listLibraryItems: () =>
    invoke<LibraryItemSummary[]>("list_library_items"),
  getLibraryItem: (id: number) =>
    invoke<LibraryItemDetail>("get_library_item", { id }),
  addLibraryItem: (filePath: string) =>
    invoke<number>("add_library_item", { filePath }),
  removeLibraryItem: (id: number) =>
    invoke<void>("remove_library_item", { id }),
  scanLibraryDirectory: (dirPath: string) =>
    invoke<ScanResult>("scan_library_directory", { dirPath }),
  addLibraryAsset: (itemId: number, assetType: string, filePath: string, lang?: string) =>
    invoke<number>("add_library_asset", { itemId, assetType, filePath, lang }),
  removeLibraryAsset: (id: number) =>
    invoke<void>("remove_library_asset", { id }),
  getLibraryStats: () =>
    invoke<LibraryStats>("get_library_stats"),

  // Library index (file-based)
  listIndexEntries: () =>
    invoke<IndexEntry[]>("list_index_entries"),
  listIndexByDirector: () =>
    invoke<Record<string, IndexEntry[]>>("list_index_by_director"),
  searchIndex: (query?: string, yearFrom?: number, yearTo?: number, genre?: string) =>
    invoke<IndexEntry[]>("search_index", { query, yearFrom, yearTo, genre }),
  rebuildIndex: () =>
    invoke<void>("rebuild_index"),
  deleteLibraryResource: (filePath: string) =>
    invoke<void>("delete_library_resource", { filePath }),
  refreshIndexEntry: (tmdbId: number) =>
    invoke<void>("refresh_index_entry", { tmdbId }),
  deleteFilmDirectory: (tmdbId: number) =>
    invoke<void>("delete_film_directory", { tmdbId }),
  enrichIndexEntry: (tmdbId: number, force?: boolean) =>
    invoke<IndexEntry>("enrich_index_entry", { tmdbId, force }),
};

export const download = {
  startDownload: (req: StartDownloadRequest) =>
    invoke<number>("start_download", { req }),
  getTorrentFiles: (target: string) =>
    invoke<TorrentFileInfo[]>("get_torrent_files", { target }),
  listDownloads: () =>
    invoke<DownloadRecord[]>("list_downloads"),
  pauseDownload: (id: number) =>
    invoke<void>("pause_download", { id }),
  resumeDownload: (id: number) =>
    invoke<void>("resume_download", { id }),
  deleteDownload: (id: number) =>
    invoke<void>("delete_download", { id }),
  redownload: (id: number, onlyFiles?: number[]) =>
    invoke<void>("redownload", { id, onlyFiles }),
  listExistingFiles: (id: number) =>
    invoke<string[]>("list_download_existing_files", { id }),
};

export const yts = {
  search: (query: string, year?: number, tmdbId?: number) =>
    invoke<MovieResult[]>("search_yify_cmd", { query, year, tmdbId }),
};

export interface TrackerStatus {
  auto_count: number;
  user_count: number;
  total_count: number;
  last_updated: string | null;
}

export const tracker = {
  getStatus: () => invoke<TrackerStatus>("get_tracker_status"),
  refresh: () => invoke<TrackerStatus>("refresh_trackers"),
  addUserTrackers: (raw: string) => invoke<TrackerStatus>("add_user_trackers", { raw }),
};

export const subtitle = {
  fetch: (video: string, lang: string) =>
    invoke<void>("fetch_subtitle_cmd", { video, lang, apiKey: "" }),
  align: (video: string, srt: string) =>
    invoke<void>("align_subtitle_cmd", { video, srt }),
  extract: (video: string, stream?: number) =>
    invoke<void>("extract_subtitle_cmd", { video, stream }),
  listStreams: (video: string) =>
    invoke<SubtitleStreamInfo[]>("list_subtitle_streams_cmd", { video }),
  shift: (srt: string, offsetMs: number) =>
    invoke<void>("shift_subtitle_cmd", { srt, offsetMs }),
};

export const media = {
  probeDetail: (filePath: string) =>
    invoke<MediaInfo>("probe_media_detail", { filePath }),
  probeAndCache: (tmdbId: number, filename: string) =>
    invoke<FileMediaInfo>("probe_and_cache", { tmdbId, filename }),
  openInPlayer: (filePath: string) =>
    invoke<void>("cmd_open_player", { filePath }),
};

export const player = {
  getCurrentFile: () =>
    invoke<string | null>("cmd_player_get_current_file"),
  subAdd: (path: string) =>
    invoke<void>("cmd_player_sub_add", { path }),
};
