-- Refactor downloads table for librqbit integration
-- Replaces old aria2c-based schema with torrent session fields
DROP TABLE IF EXISTS downloads;

CREATE TABLE downloads (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    tmdb_id       INTEGER,
    title         TEXT NOT NULL,
    director      TEXT,
    quality       TEXT,
    target        TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK(status IN ('pending','downloading','paused','completed','failed')),
    torrent_id    INTEGER,
    progress_bytes INTEGER DEFAULT 0,
    total_bytes   INTEGER DEFAULT 0,
    error_message TEXT,
    started_at    TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at  TEXT
);
