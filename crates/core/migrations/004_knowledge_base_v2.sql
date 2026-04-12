-- Knowledge Base v2: unified entry model (entries + tags + typed relations)
-- Replaces old people/films/genres model. No data migration — clean break.

-- Recreate library_items without the FK to films
CREATE TABLE library_items_new (
    id            INTEGER PRIMARY KEY,
    file_path     TEXT NOT NULL UNIQUE,
    file_size     INTEGER,
    duration_secs INTEGER,
    video_codec   TEXT,
    audio_codec   TEXT,
    resolution    TEXT,
    added_at      TEXT DEFAULT (datetime('now'))
);
INSERT INTO library_items_new (id, file_path, file_size, duration_secs, video_codec, audio_codec, resolution, added_at)
    SELECT id, file_path, file_size, duration_secs, video_codec, audio_codec, resolution, added_at FROM library_items;

-- Recreate library_assets pointing to new items table
CREATE TABLE library_assets_new (
    id         INTEGER PRIMARY KEY,
    item_id    INTEGER NOT NULL REFERENCES library_items_new(id),
    asset_type TEXT NOT NULL CHECK(asset_type IN ('subtitle','edited','poster')),
    file_path  TEXT NOT NULL,
    lang       TEXT,
    created_at TEXT DEFAULT (datetime('now'))
);
INSERT INTO library_assets_new (id, item_id, asset_type, file_path, lang, created_at)
    SELECT id, item_id, asset_type, file_path, lang, created_at FROM library_assets;

DROP TABLE library_assets;
DROP TABLE library_items;
ALTER TABLE library_items_new RENAME TO library_items;
ALTER TABLE library_assets_new RENAME TO library_assets;

-- Drop old knowledge base tables (dependency order)
DROP TABLE IF EXISTS reviews;
DROP TABLE IF EXISTS wiki_entries;
DROP TABLE IF EXISTS person_relations;
DROP TABLE IF EXISTS person_genres;
DROP TABLE IF EXISTS film_genres;
DROP TABLE IF EXISTS person_films;
DROP TABLE IF EXISTS films;
DROP TABLE IF EXISTS genres;
DROP TABLE IF EXISTS people;

-- New knowledge base tables
CREATE TABLE entries (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    wiki       TEXT NOT NULL DEFAULT '',
    created_at TEXT DEFAULT (datetime('now')),
    updated_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE entry_tags (
    entry_id INTEGER NOT NULL REFERENCES entries(id),
    tag      TEXT NOT NULL,
    PRIMARY KEY (entry_id, tag)
);

CREATE TABLE relations (
    id            INTEGER PRIMARY KEY,
    from_id       INTEGER NOT NULL REFERENCES entries(id),
    to_id         INTEGER NOT NULL REFERENCES entries(id),
    relation_type TEXT NOT NULL
);
