// src/lib/tauri.ts
import { invoke } from "@tauri-apps/api/core";

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

export interface TmdbGenre {
  id: number;
  name: string;
}

export interface AppConfig {
  tools: { aria2c: string; alass: string; ffmpeg: string };
  search: { rate_limit_secs: number };
  subtitle: { default_lang: string };
  opensubtitles: { api_key: string };
  tmdb: { api_key: string };
  library: { root_dir: string };
}

export const tmdb = {
  searchMovies: (apiKey: string, query: string, filters: SearchFilters) =>
    invoke<MovieListItem[]>("search_movies", { apiKey, query, filters }),

  discoverMovies: (apiKey: string, filters: SearchFilters) =>
    invoke<MovieListItem[]>("discover_movies", { apiKey, filters }),

  listGenres: (apiKey: string) =>
    invoke<TmdbGenre[]>("list_genres", { apiKey }),
};

export const config = {
  get: () => invoke<AppConfig>("get_config"),
  set: (key: string, value: string) =>
    invoke<void>("set_config_key", { key, value }),
};
