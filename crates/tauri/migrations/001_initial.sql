-- crates/tauri/migrations/001_initial.sql

CREATE TABLE IF NOT EXISTS people (
  id            INTEGER PRIMARY KEY,
  tmdb_id       INTEGER UNIQUE,
  name          TEXT NOT NULL,
  born_date     TEXT,
  biography     TEXT,
  nationality   TEXT,
  primary_role  TEXT NOT NULL CHECK(primary_role IN (
                  'director','cinematographer','composer',
                  'editor','screenwriter','producer','actor')),
  created_at    TEXT DEFAULT (datetime('now')),
  updated_at    TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS genres (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  description TEXT,
  parent_id   INTEGER REFERENCES genres(id),
  period      TEXT
);

CREATE TABLE IF NOT EXISTS films (
  id                INTEGER PRIMARY KEY,
  tmdb_id           INTEGER UNIQUE,
  title             TEXT NOT NULL,
  original_title    TEXT,
  year              INTEGER,
  overview          TEXT,
  tmdb_rating       REAL,
  poster_cache_path TEXT,
  created_at        TEXT DEFAULT (datetime('now')),
  updated_at        TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS person_films (
  person_id INTEGER NOT NULL REFERENCES people(id),
  film_id   INTEGER NOT NULL REFERENCES films(id),
  role      TEXT NOT NULL,
  PRIMARY KEY (person_id, film_id, role)
);

CREATE TABLE IF NOT EXISTS film_genres (
  film_id  INTEGER NOT NULL REFERENCES films(id),
  genre_id INTEGER NOT NULL REFERENCES genres(id),
  PRIMARY KEY (film_id, genre_id)
);

CREATE TABLE IF NOT EXISTS person_genres (
  person_id INTEGER NOT NULL REFERENCES people(id),
  genre_id  INTEGER NOT NULL REFERENCES genres(id),
  PRIMARY KEY (person_id, genre_id)
);

CREATE TABLE IF NOT EXISTS person_relations (
  from_id       INTEGER NOT NULL REFERENCES people(id),
  to_id         INTEGER NOT NULL REFERENCES people(id),
  relation_type TEXT NOT NULL CHECK(relation_type IN (
                  'influenced','contemporary','collaborated')),
  PRIMARY KEY (from_id, to_id, relation_type)
);

CREATE TABLE IF NOT EXISTS wiki_entries (
  id          INTEGER PRIMARY KEY,
  entity_type TEXT NOT NULL CHECK(entity_type IN ('person','film','genre')),
  entity_id   INTEGER NOT NULL,
  content     TEXT NOT NULL DEFAULT '',
  updated_at  TEXT DEFAULT (datetime('now')),
  UNIQUE (entity_type, entity_id)
);

CREATE TABLE IF NOT EXISTS reviews (
  id          INTEGER PRIMARY KEY,
  film_id     INTEGER NOT NULL REFERENCES films(id),
  is_personal INTEGER NOT NULL DEFAULT 0,
  author      TEXT,
  content     TEXT NOT NULL,
  rating      REAL,
  created_at  TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS library_items (
  id            INTEGER PRIMARY KEY,
  film_id       INTEGER REFERENCES films(id),
  file_path     TEXT NOT NULL UNIQUE,
  file_size     INTEGER,
  duration_secs INTEGER,
  video_codec   TEXT,
  audio_codec   TEXT,
  resolution    TEXT,
  added_at      TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS library_assets (
  id         INTEGER PRIMARY KEY,
  item_id    INTEGER NOT NULL REFERENCES library_items(id),
  asset_type TEXT NOT NULL CHECK(asset_type IN ('subtitle','edited','poster')),
  file_path  TEXT NOT NULL,
  lang       TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);
