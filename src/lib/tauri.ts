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
  tools: { alass: string; ffmpeg: string; player: string };
  download: { max_concurrent: number; enable_dht: boolean; persist_session: boolean };
  search: { rate_limit_secs: number };
  subtitle: { default_lang: string };
  opensubtitles: { api_key: string };
  tmdb: { api_key: string };
  library: { root_dir: string };
  music: { enabled: boolean; mode: "sequential" | "random"; playlist: MusicTrack[] };
  cache: { max_entries: number };
}

// ── Library types ─────────────────────────────────────────────────
export interface PersonSummary { id: number; name: string; primary_role: string; nationality: string | null; film_count: number; }
export interface PersonFilmEntry { film_id: number; title: string; year: number | null; role: string; poster_cache_path: string | null; }
export interface PersonRelation { target_id: number; target_name: string; direction: string; relation_type: string; }
export interface PersonDetail {
  id: number; tmdb_id: number | null; name: string; primary_role: string;
  born_date: string | null; nationality: string | null; biography: string | null;
  wiki_content: string; films: PersonFilmEntry[]; relations: PersonRelation[];
}

export interface FilmSummary { id: number; title: string; year: number | null; tmdb_rating: number | null; poster_cache_path: string | null; }
export interface FilmPersonEntry { person_id: number; name: string; role: string; }
export interface ReviewEntry { id: number; is_personal: boolean; author: string | null; content: string; rating: number | null; created_at: string; }
export interface GenreSummary { id: number; name: string; film_count: number; child_count: number; }
export interface FilmDetail {
  id: number; tmdb_id: number | null; title: string; original_title: string | null;
  year: number | null; overview: string | null; tmdb_rating: number | null;
  poster_cache_path: string | null; wiki_content: string;
  people: FilmPersonEntry[]; genres: GenreSummary[]; reviews: ReviewEntry[];
}

export interface GenreDetail {
  id: number; name: string; description: string | null; parent_id: number | null;
  period: string | null; wiki_content: string;
  children: GenreSummary[]; people: PersonSummary[]; films: FilmSummary[];
}

export interface GenreTreeNode { id: number; name: string; period: string | null; film_count: number; children: GenreTreeNode[]; }

export interface TmdbPersonInput { tmdb_id: number | null; name: string; role: string; primary_role: string; }
export interface TmdbMovieInput {
  tmdb_id: number; title: string; original_title: string | null;
  year: number | null; overview: string | null; tmdb_rating: number | null;
  people: TmdbPersonInput[];
}

export interface GraphNode { id: string; label: string; node_type: string; role: string | null; weight: number; }
export interface GraphLink { source: string; target: string; role: string; }
export interface GraphData { nodes: GraphNode[]; links: GraphLink[]; }

export interface LibraryItemSummary {
  id: number;
  film_id: number | null;
  file_path: string;
  file_size: number | null;
  duration_secs: number | null;
  video_codec: string | null;
  audio_codec: string | null;
  resolution: string | null;
  added_at: string;
  film_title: string | null;
  film_year: number | null;
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
  total_films: number;
  films_with_files: number;
  total_file_size: number;
  unlinked_files: number;
  by_decade: StatEntry[];
  by_genre: StatEntry[];
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

export interface FilmListEntry {
  id: number;
  title: string;
  original_title: string | null;
  year: number | null;
  tmdb_rating: number | null;
  poster_cache_path: string | null;
  has_file: number;
}

export interface FilmFilterResult {
  films: FilmListEntry[];
  total: number;
  page: number;
  page_size: number;
}

export interface FilmFilterParams {
  query?: string;
  genreId?: number;
  yearFrom?: number;
  yearTo?: number;
  minRating?: number;
  hasFile?: boolean;
  sortBy?: string;
  sortDesc?: boolean;
  page?: number;
  pageSize?: number;
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
};

export const library = {
  listPeople: () => invoke<PersonSummary[]>("list_people"),
  getPerson: (id: number) => invoke<PersonDetail>("get_person", { id }),
  createPerson: (name: string, primaryRole: string, tmdbId?: number, bornDate?: string, nationality?: string) =>
    invoke<number>("create_person", { name, primaryRole, tmdbId, bornDate, nationality }),
  updatePersonWiki: (id: number, content: string) => invoke<void>("update_person_wiki", { id, content }),
  deletePerson: (id: number) => invoke<void>("delete_person", { id }),
  addPersonRelation: (fromId: number, toId: number, relationType: string) =>
    invoke<void>("add_person_relation", { fromId, toId, relationType }),
  removePersonRelation: (fromId: number, toId: number, relationType: string) =>
    invoke<void>("remove_person_relation", { fromId, toId, relationType }),

  listFilms: () => invoke<FilmSummary[]>("list_films"),
  getFilm: (id: number) => invoke<FilmDetail>("get_film", { id }),
  addFilmFromTmdb: (tmdbMovie: TmdbMovieInput) => invoke<number>("add_film_from_tmdb", { tmdbMovie }),
  updateFilmWiki: (id: number, content: string) => invoke<void>("update_film_wiki", { id, content }),
  deleteFilm: (id: number) => invoke<void>("delete_film", { id }),

  listGenresTree: () => invoke<GenreTreeNode[]>("list_genres_tree"),
  getGenre: (id: number) => invoke<GenreDetail>("get_genre", { id }),
  createGenre: (name: string, parentId?: number, description?: string, period?: string) =>
    invoke<number>("create_genre", { name, parentId, description, period }),
  updateGenreWiki: (id: number, content: string) => invoke<void>("update_genre_wiki", { id, content }),
  deleteGenre: (id: number) => invoke<void>("delete_genre", { id }),
  linkFilmGenre: (filmId: number, genreId: number) => invoke<void>("link_film_genre", { filmId, genreId }),
  unlinkFilmGenre: (filmId: number, genreId: number) => invoke<void>("unlink_film_genre", { filmId, genreId }),
  linkPersonGenre: (personId: number, genreId: number) => invoke<void>("link_person_genre", { personId, genreId }),
  unlinkPersonGenre: (personId: number, genreId: number) => invoke<void>("unlink_person_genre", { personId, genreId }),

  addReview: (filmId: number, isPersonal: boolean, author: string | null, content: string, rating: number | null) =>
    invoke<number>("add_review", { filmId, isPersonal, author, content, rating }),
  updateReview: (id: number, content: string, rating: number | null) =>
    invoke<void>("update_review", { id, content, rating }),
  deleteReview: (id: number) => invoke<void>("delete_review", { id }),

  getGraphData: () => invoke<GraphData>("get_graph_data"),

  // Library items
  listLibraryItems: () =>
    invoke<LibraryItemSummary[]>("list_library_items"),
  getLibraryItem: (id: number) =>
    invoke<LibraryItemDetail>("get_library_item", { id }),
  addLibraryItem: (filePath: string, filmId?: number) =>
    invoke<number>("add_library_item", { filePath, filmId }),
  linkItemToFilm: (itemId: number, filmId: number) =>
    invoke<void>("link_item_to_film", { itemId, filmId }),
  unlinkItemFromFilm: (itemId: number) =>
    invoke<void>("unlink_item_from_film", { itemId }),
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
  listFilmsFiltered: (params: FilmFilterParams) =>
    invoke<FilmFilterResult>("list_films_filtered", params as Record<string, unknown>),

  // Library index
  listIndexEntries: () =>
    invoke<IndexEntry[]>("list_index_entries"),
  listIndexByDirector: () =>
    invoke<Record<string, IndexEntry[]>>("list_index_by_director"),
  searchIndex: (query?: string, yearFrom?: number, yearTo?: number, genre?: string) =>
    invoke<IndexEntry[]>("search_index", { query, yearFrom, yearTo, genre }),
  rebuildIndex: () =>
    invoke<void>("rebuild_index"),
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
  redownload: (id: number) =>
    invoke<number>("redownload", { id }),
};

export const yts = {
  search: (query: string, year?: number, tmdbId?: number) =>
    invoke<MovieResult[]>("search_yify_cmd", { query, year, tmdbId }),
};

export const tracker = {
  update: (source?: string) =>
    invoke<void>("update_trackers", { source }),
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
  openInPlayer: (filePath: string) =>
    invoke<void>("cmd_open_player", { filePath }),
};
