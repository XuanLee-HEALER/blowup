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
  tools: { aria2c: string; alass: string; ffmpeg: string };
  search: { rate_limit_secs: number };
  subtitle: { default_lang: string };
  opensubtitles: { api_key: string };
  tmdb: { api_key: string };
  library: { root_dir: string };
  music: { enabled: boolean; mode: string; playlist: MusicTrack[] };
}

// ── Library types ─────────────────────────────────────────────────
export interface PersonSummary { id: number; name: string; primary_role: string; film_count: number; }
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

// ── Invoke wrappers ───────────────────────────────────────────────
export const tmdb = {
  searchMovies: (apiKey: string, query: string, filters: SearchFilters) =>
    invoke<MovieListItem[]>("search_movies", { apiKey, query, filters }),
  discoverMovies: (apiKey: string, filters: SearchFilters) =>
    invoke<MovieListItem[]>("discover_movies", { apiKey, filters }),
  listGenres: (apiKey: string) => invoke<TmdbGenre[]>("list_genres", { apiKey }),
  getMovieCredits: (apiKey: string, tmdbId: number) =>
    invoke<TmdbMovieCredits>("get_tmdb_movie_credits", { apiKey, tmdbId }),
};

export const config = {
  get: () => invoke<AppConfig>("get_config"),
  set: (key: string, value: string) => invoke<void>("set_config_key", { key, value }),
  setMusicPlaylist: (tracks: MusicTrack[]) => invoke<void>("set_music_playlist", { tracks }),
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
};
