#!/usr/bin/env node
/**
 * Import cinema_wiki.json into blowup's SQLite database.
 * Fetches Wikipedia content for each director and film.
 *
 * Usage: node scripts/import-cinema-wiki.mjs
 */

import { readFileSync } from "fs";
import { join } from "path";
import Database from "better-sqlite3";

// ── Config ───────────────────────────────────────────────────────

const DB_PATH = join(
  process.env.APPDATA || "",
  "io.github.xuanlee-healer.blowup",
  "blowup.db"
);
const JSON_PATH = join(import.meta.dirname, "..", "cinema_wiki.json");
const RATE_LIMIT_MS = 200; // polite crawling

// ── Wikipedia fetcher ────────────────────────────────────────────

async function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

/**
 * Fetch Wikipedia extract for a given title.
 * Tries Chinese Wikipedia first, falls back to English.
 * Returns markdown-formatted content.
 */
async function fetchWikiContent(titleZh, titleEn, entityType) {
  // Try Chinese Wikipedia first
  for (const [lang, title] of [["zh", titleZh], ["en", titleEn]]) {
    if (!title) continue;
    try {
      const content = await fetchWikiExtract(lang, title);
      if (content && content.length > 100) {
        return content;
      }
    } catch {
      // try next
    }
    await sleep(RATE_LIMIT_MS);
  }

  // Fallback: try English with " (film)" suffix for films
  if (entityType === "film" && titleEn) {
    try {
      const content = await fetchWikiExtract("en", titleEn + " (film)");
      if (content && content.length > 100) return content;
    } catch { /* */ }
    await sleep(RATE_LIMIT_MS);
  }

  // Fallback: try English with " (director)" suffix for people
  if (entityType === "person" && titleEn) {
    try {
      const content = await fetchWikiExtract("en", titleEn + " (filmmaker)");
      if (content && content.length > 100) return content;
    } catch { /* */ }
  }

  return "";
}

async function fetchWikiExtract(lang, title) {
  const url = new URL(`https://${lang}.wikipedia.org/w/api.php`);
  url.searchParams.set("action", "query");
  url.searchParams.set("prop", "extracts");
  url.searchParams.set("exintro", "1");
  url.searchParams.set("explaintext", "1");
  url.searchParams.set("redirects", "1");
  url.searchParams.set("titles", title);
  url.searchParams.set("format", "json");

  const resp = await fetch(url.toString(), {
    headers: { "User-Agent": "blowup-cinema-wiki-importer/1.0 (personal project)" },
  });

  if (!resp.ok) return "";

  const data = await resp.json();
  const pages = data.query?.pages;
  if (!pages) return "";

  for (const page of Object.values(pages)) {
    if (page.missing !== undefined) continue;
    const extract = page.extract?.trim();
    if (extract) return extract;
  }
  return "";
}

// ── Main ─────────────────────────────────────────────────────────

async function main() {
  console.log("Reading cinema_wiki.json...");
  const wiki = JSON.parse(readFileSync(JSON_PATH, "utf-8"));

  console.log(`Opening database at ${DB_PATH}`);
  const db = new Database(DB_PATH);
  db.pragma("journal_mode = WAL");
  db.pragma("foreign_keys = ON");

  // Run migrations if tables don't exist
  const tableCheck = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name='genres'").get();
  if (!tableCheck) {
    console.log("Running migrations...");
    const migrationsDir = join(import.meta.dirname, "..", "src-tauri", "migrations");
    const migration1 = readFileSync(join(migrationsDir, "001_initial.sql"), "utf-8");
    const migration2 = readFileSync(join(migrationsDir, "002_downloads.sql"), "utf-8");
    db.exec(migration1);
    db.exec(migration2);
    console.log("Migrations applied.");
  }

  // Prepared statements
  const insertGenre = db.prepare(
    "INSERT INTO genres (name, description, period) VALUES (?, ?, ?)"
  );
  const insertPerson = db.prepare(
    "INSERT INTO people (name, nationality, primary_role) VALUES (?, ?, 'director')"
  );
  const insertFilm = db.prepare(
    "INSERT INTO films (title, original_title, year) VALUES (?, ?, ?)"
  );
  const insertPersonFilm = db.prepare(
    "INSERT OR IGNORE INTO person_films (person_id, film_id, role) VALUES (?, ?, 'director')"
  );
  const insertFilmGenre = db.prepare(
    "INSERT OR IGNORE INTO film_genres (film_id, genre_id) VALUES (?, ?)"
  );
  const insertPersonGenre = db.prepare(
    "INSERT OR IGNORE INTO person_genres (person_id, genre_id) VALUES (?, ?)"
  );
  const insertRelation = db.prepare(
    "INSERT OR IGNORE INTO person_relations (from_id, to_id, relation_type) VALUES (?, ?, 'influenced')"
  );
  const upsertWiki = db.prepare(
    `INSERT INTO wiki_entries (entity_type, entity_id, content, updated_at)
     VALUES (?, ?, ?, datetime('now'))
     ON CONFLICT(entity_type, entity_id)
     DO UPDATE SET content = excluded.content, updated_at = excluded.updated_at`
  );

  // ── Step 1: Import genres (movements) ──────────────────────────

  console.log("\n--- Importing genres ---");
  const genreIdMap = {}; // movement.id -> db id

  for (const m of wiki.movements) {
    const result = insertGenre.run(m.name, m.description, m.period || null);
    genreIdMap[m.id] = result.lastInsertRowid;
    console.log(`  Genre: ${m.name} (id=${result.lastInsertRowid})`);
  }

  // Write genre wiki (use description + name_en as wiki content)
  for (const m of wiki.movements) {
    const dbId = genreIdMap[m.id];
    const wikiContent = [
      `# ${m.name}`,
      `**${m.name_en}**`,
      "",
      `**时期**: ${m.period || "N/A"}`,
      `**地区**: ${m.region || "N/A"}`,
      "",
      m.description,
    ].join("\n");
    upsertWiki.run("genre", dbId, wikiContent);
  }

  // ── Step 2: Import directors ───────────────────────────────────

  console.log("\n--- Importing directors ---");
  const personIdMap = {}; // director.id -> db id

  for (const d of wiki.directors) {
    const result = insertPerson.run(d.name, d.nationality);
    personIdMap[d.id] = result.lastInsertRowid;
    console.log(`  Person: ${d.name} (id=${result.lastInsertRowid})`);

    // Link person to genres
    for (const mId of d.movements || []) {
      if (genreIdMap[mId]) {
        insertPersonGenre.run(result.lastInsertRowid, genreIdMap[mId]);
      }
    }
  }

  // ── Step 3: Import films ───────────────────────────────────────

  console.log("\n--- Importing films ---");
  const filmIdMap = {}; // "director_id:title_en" -> db id

  for (const d of wiki.directors) {
    const personId = personIdMap[d.id];
    for (const work of d.key_works || []) {
      const filmKey = `${d.id}:${work.title_en}`;
      const result = insertFilm.run(work.title, work.title_en, work.year);
      const filmId = result.lastInsertRowid;
      filmIdMap[filmKey] = filmId;

      // Link film to director
      insertPersonFilm.run(personId, filmId);

      // Link film to director's genres
      for (const mId of d.movements || []) {
        if (genreIdMap[mId]) {
          insertFilmGenre.run(filmId, genreIdMap[mId]);
        }
      }

      console.log(`  Film: ${work.title} / ${work.title_en} (${work.year})`);
    }
  }

  // ── Step 4: Import relations (influences) ──────────────────────

  console.log("\n--- Importing relations ---");
  for (const d of wiki.directors) {
    const fromId = personIdMap[d.id];
    for (const targetId of d.influences?.influenced || []) {
      const toId = personIdMap[targetId];
      if (fromId && toId) {
        insertRelation.run(fromId, toId);
        console.log(`  ${d.name} -> influenced -> ${targetId}`);
      }
    }
  }

  // ── Step 5: Fetch Wikipedia content ────────────────────────────

  console.log("\n--- Fetching Wikipedia content for directors ---");
  let fetched = 0;
  const totalDirectors = wiki.directors.length;

  for (const d of wiki.directors) {
    fetched++;
    process.stdout.write(`  [${fetched}/${totalDirectors}] ${d.name}... `);

    const content = await fetchWikiContent(d.name, d.name_en, "person");
    if (content) {
      // Build rich wiki with style tags
      const styleTags = (d.style_tags || []).join("、");
      const wikiText = [
        `# ${d.name}`,
        `**${d.name_en}**`,
        "",
        `**国籍**: ${d.nationality}`,
        `**活跃时期**: ${d.active_period || "N/A"}`,
        styleTags ? `**风格标签**: ${styleTags}` : "",
        "",
        content,
      ]
        .filter(Boolean)
        .join("\n");

      upsertWiki.run("person", personIdMap[d.id], wikiText);
      console.log(`OK (${content.length} chars)`);
    } else {
      // Fallback: use style tags only
      const styleTags = (d.style_tags || []).join("、");
      const fallback = [
        `# ${d.name}`,
        `**${d.name_en}**`,
        "",
        `**国籍**: ${d.nationality}`,
        `**活跃时期**: ${d.active_period || "N/A"}`,
        styleTags ? `**风格标签**: ${styleTags}` : "",
      ]
        .filter(Boolean)
        .join("\n");
      upsertWiki.run("person", personIdMap[d.id], fallback);
      console.log("FALLBACK (no wiki found)");
    }

    await sleep(RATE_LIMIT_MS);
  }

  console.log("\n--- Fetching Wikipedia content for films ---");
  let filmFetched = 0;
  const allFilms = wiki.directors.flatMap((d) =>
    (d.key_works || []).map((w) => ({ ...w, directorId: d.id, directorName: d.name }))
  );
  const totalFilms = allFilms.length;

  for (const film of allFilms) {
    filmFetched++;
    const filmKey = `${film.directorId}:${film.title_en}`;
    const filmId = filmIdMap[filmKey];
    if (!filmId) continue;

    process.stdout.write(`  [${filmFetched}/${totalFilms}] ${film.title}... `);

    const content = await fetchWikiContent(film.title, film.title_en, "film");
    if (content) {
      const wikiText = [
        `# ${film.title}`,
        `**${film.title_en}** (${film.year})`,
        "",
        `**导演**: ${film.directorName}`,
        "",
        content,
      ].join("\n");

      upsertWiki.run("film", filmId, wikiText);
      console.log(`OK (${content.length} chars)`);
    } else {
      const fallback = [
        `# ${film.title}`,
        `**${film.title_en}** (${film.year})`,
        "",
        `**导演**: ${film.directorName}`,
      ].join("\n");
      upsertWiki.run("film", filmId, fallback);
      console.log("FALLBACK");
    }

    await sleep(RATE_LIMIT_MS);
  }

  // ── Done ───────────────────────────────────────────────────────

  const genreCount = wiki.movements.length;
  const personCount = wiki.directors.length;
  const filmCount = allFilms.length;

  console.log(`\n=== Import complete ===`);
  console.log(`  Genres:    ${genreCount}`);
  console.log(`  Directors: ${personCount}`);
  console.log(`  Films:     ${filmCount}`);
  console.log(`  DB:        ${DB_PATH}`);

  db.close();
}

main().catch((err) => {
  console.error("Fatal error:", err);
  process.exit(1);
});
