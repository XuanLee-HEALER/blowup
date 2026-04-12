CREATE TABLE IF NOT EXISTS downloads (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    film_id       INTEGER REFERENCES films(id),
    title         TEXT NOT NULL,
    quality       TEXT,
    target        TEXT NOT NULL,
    output_dir    TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK(status IN ('pending','downloading','completed','failed','cancelled')),
    pid           INTEGER,
    file_path     TEXT,
    error_message TEXT,
    started_at    TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at  TEXT
);
