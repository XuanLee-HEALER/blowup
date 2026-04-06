# blowup v2 · M1 设计规格

**日期**: 2026-04-06  
**状态**: 已批准  
**里程碑**: M1 — 骨架（Foundation）  
**分支**: v2

---

## 1. 背景与目标

blowup v1 是一个 Rust CLI 工具，服务于中文观影流水线。v2 是一次完整的重写，目标是将其改造为**以个人品味为核心的桌面电影管理套件**。

v2 按四个里程碑交付：

| 里程碑 | 内容 |
|--------|------|
| **M1（本文）** | Tauri v2 骨架、React Shell、CLI 逻辑迁移为 Tauri commands、TMDB 高级搜索、Settings |
| M2 | 知识库（影人/流派/关系图/Wiki/影评） |
| M3 | 资源管理 + 下载（本地库、YIFY、种子软件集成） |
| M4 | 媒体工具 + 字幕管理 |

M1 的目标：建立可运行的应用骨架，所有后续功能都在此基础上叠加，不需要再改项目结构。

---

## 2. 技术栈

| 层 | 技术 |
|----|------|
| 桌面框架 | Tauri v2 |
| 前端 | React 19 + TypeScript |
| 构建工具 | Vite |
| 样式 | Tailwind CSS v4 |
| Rust 后端 | Tauri commands（现有模块迁入） |
| 数据库 | SQLite via `sqlx`（`sqlx` migrations） |
| HTTP 客户端 | `reqwest`（现有，保留） |
| 前端路由 | React Router v7（单页，hash 模式） |

---

## 3. 项目结构

```
blowup/
├── src/                          # React + TypeScript 前端
│   ├── components/
│   │   └── ui/                   # 设计系统基础组件
│   ├── pages/
│   │   ├── Search.tsx            # TMDB 搜索（M1 唯一完整功能页）
│   │   ├── Settings.tsx          # 配置管理（M1 实现）
│   │   ├── Library.tsx           # 占位（M2 知识库入口）
│   │   ├── Download.tsx          # 占位（M3）
│   │   ├── Subtitle.tsx          # 占位（M4）
│   │   └── Tools.tsx             # 占位（M4）
│   ├── App.tsx                   # 固定侧边栏 Shell + 路由
│   └── main.tsx
├── src-tauri/
│   ├── src/
│   │   ├── main.rs               # Tauri 入口
│   │   ├── lib.rs                # 注册所有 commands
│   │   ├── commands/
│   │   │   ├── search.rs         # ← 现有 search.rs
│   │   │   ├── tmdb.rs           # ← 现有 tmdb.rs
│   │   │   ├── download.rs       # ← 现有 download.rs
│   │   │   ├── subtitle.rs       # ← 现有 sub/
│   │   │   ├── tracker.rs        # ← 现有 tracker.rs
│   │   │   ├── media.rs          # ← 现有 ffmpeg.rs
│   │   │   └── config.rs         # ← 现有 config.rs
│   │   └── db/
│   │       ├── mod.rs            # SQLite 连接池初始化
│   │       └── migrations/       # .sql 迁移文件
│   └── Cargo.toml
├── package.json
├── tauri.conf.json
└── vite.config.ts
```

**CLI 完全替代**：原 `main.rs` CLI 入口删除，所有模块核心函数保留并包装为 `#[tauri::command]`。不保留 CLI binary。

---

## 4. 设计系统

### 4.1 设计原则

遵循 **Apple Human Interface Guidelines**：
- **内容优先**：UI 为内容服务，不喧宾夺主
- **克制**：无装饰性元素，每个视觉元素都有功能目的
- **层次分明**：用透明度层级区分信息重要性，而非颜色

### 4.2 颜色系统（青金石主题）

```css
/* Backgrounds — 青金石深蓝系 */
--bg-primary:      #0B1628;   /* 主底色（极暗青金石） */
--bg-secondary:    #122040;   /* 侧边栏、卡片 */
--bg-elevated:     #1A2E56;   /* 选中、悬停 */
--bg-control:      rgba(255,255,255,0.06);

/* Separators */
--separator:       rgba(100,130,200,0.12);

/* Labels — Apple HIG Dark Mode 透明度层级 */
--label-primary:   #FFFFFF;
--label-secondary: rgba(255,255,255,0.60);
--label-tertiary:  rgba(255,255,255,0.30);
--label-quaternary:rgba(255,255,255,0.16);

/* Accent — 黄铁矿金（克制使用） */
--accent:          #C5A050;
--accent-soft:     rgba(197,160,80,0.15);
```

### 4.3 字体

系统字体栈：`-apple-system, BlinkMacSystemFont, "SF Pro Text", "Helvetica Neue", sans-serif`

### 4.4 交互规范

- Hover：`bg-elevated`（无动效，即时）
- 无高饱和强调色用于导航激活态，仅用透明度加深
- `accent` 金色仅用于 active filter、少数 CTA 按钮

---

## 5. 应用 Shell

### 5.1 侧边栏

固定宽度 `188px`，始终可见，不可折叠。

```
侧边栏结构：
  [搜索]                    ← M1 可用
  ─── 知识库 ───
  [影人]                    ← M2，置灰
  [流派]                    ← M2，置灰
  [关系图]                  ← M2，置灰
  ─── 资源 ───
  [我的库]                  ← M3，置灰
  [下载]                    ← M3，置灰
  ─── 工具 ───
  [字幕]                    ← M4，置灰
  [媒体]                    ← M4，置灰
  ────────────
  [设置]                    ← M1 可用
```

置灰项：`opacity: 0.25`，`pointer-events: none`，无 tooltip。

---

## 6. TMDB 搜索页（Search.tsx）

### 6.1 搜索逻辑

```
用户输入
├── 有文本 → 并发：
│           ① GET /3/search/movie?query={text}&...filters
│           ② GET /3/search/person?query={text}
│                └─→ GET /3/discover/movie?with_people={id}&...filters
│           合并去重（tmdb_id），标题匹配结果优先
│
└── 无文本 → GET /3/discover/movie?...filters（纯发现模式）
```

### 6.2 过滤器

| UI 控件 | TMDB API 参数 | 备注 |
|---------|--------------|------|
| 年代区间 | `primary_release_date.gte` / `.lte` | 格式 `YYYY-01-01` |
| 类型多选 | `with_genres` | 启动时缓存 `/3/genre/movie/list` |
| 最低评分 | `vote_average.gte` | 步进 0.5 |
| 排序 | `sort_by` | `popularity.desc` / `vote_average.desc` / `release_date.desc` |

### 6.3 结果列表

行式布局：`海报缩略图(34×48) + 标题 + 元信息（年份·类型·导演）+ 评分`

分页：每页 20 条，滚动到底部加载下一页（`page` 参数）。

### 6.4 详情面板

点击任意行，右侧滑出详情面板（不离开页面）：

```
[海报]  标题 (年份)
        ★ 评分 / 10
        导演：XXX
        主演：A, B, C
        类型：Drama · Thriller
        ──────────────────
        [简介文本]
        ──────────────────
        [加入知识库]  ← M2 解锁，M1 置灰
        [搜索资源]    ← M3 解锁，M1 置灰
```

---

## 7. Settings 页（Settings.tsx）

读写 `~/.config/blowup/config.toml`，通过 Tauri command 操作。

```
TMDB
  API Key          [••••••••••••••••]  [显示/隐藏]

字幕
  默认语言          [中文(zh) ▾]

工具路径
  aria2c           [aria2c          ]
  alass            [alass           ]
  ffmpeg           [ffmpeg          ]

库目录（新增）
  本地库根目录      [~/Movies/blowup ]  [选择…]
```

新增 `library.root_dir` 配置项到 `config.toml`。

---

## 8. SQLite Schema（M1 建库，全为空表）

迁移文件：`src-tauri/src/db/migrations/001_initial.sql`

```sql
-- 影人
CREATE TABLE people (
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

-- 流派 / 运动（支持父子层级）
CREATE TABLE genres (
  id          INTEGER PRIMARY KEY,
  name        TEXT NOT NULL,
  description TEXT,
  parent_id   INTEGER REFERENCES genres(id),
  period      TEXT
);

-- 电影
CREATE TABLE films (
  id               INTEGER PRIMARY KEY,
  tmdb_id          INTEGER UNIQUE,
  title            TEXT NOT NULL,
  original_title   TEXT,
  year             INTEGER,
  overview         TEXT,
  tmdb_rating      REAL,
  poster_cache_path TEXT,
  created_at       TEXT DEFAULT (datetime('now')),
  updated_at       TEXT DEFAULT (datetime('now'))
);

-- 连接表
CREATE TABLE person_films (
  person_id INTEGER NOT NULL REFERENCES people(id),
  film_id   INTEGER NOT NULL REFERENCES films(id),
  role      TEXT NOT NULL,
  PRIMARY KEY (person_id, film_id, role)
);

CREATE TABLE film_genres (
  film_id  INTEGER NOT NULL REFERENCES films(id),
  genre_id INTEGER NOT NULL REFERENCES genres(id),
  PRIMARY KEY (film_id, genre_id)
);

CREATE TABLE person_genres (
  person_id INTEGER NOT NULL REFERENCES people(id),
  genre_id  INTEGER NOT NULL REFERENCES genres(id),
  PRIMARY KEY (person_id, genre_id)
);

-- 影人影响关系
CREATE TABLE person_relations (
  from_id       INTEGER NOT NULL REFERENCES people(id),
  to_id         INTEGER NOT NULL REFERENCES people(id),
  relation_type TEXT NOT NULL CHECK(relation_type IN (
                  'influenced','contemporary','collaborated')),
  PRIMARY KEY (from_id, to_id, relation_type)
);

-- Wiki 正文（导演 / 作品 / 流派，Markdown）
CREATE TABLE wiki_entries (
  id          INTEGER PRIMARY KEY,
  entity_type TEXT NOT NULL CHECK(entity_type IN ('person','film','genre')),
  entity_id   INTEGER NOT NULL,
  content     TEXT NOT NULL DEFAULT '',
  updated_at  TEXT DEFAULT (datetime('now')),
  UNIQUE (entity_type, entity_id)
);

-- 影评
CREATE TABLE reviews (
  id          INTEGER PRIMARY KEY,
  film_id     INTEGER NOT NULL REFERENCES films(id),
  is_personal INTEGER NOT NULL DEFAULT 0,
  author      TEXT,
  content     TEXT NOT NULL,
  rating      REAL,
  created_at  TEXT DEFAULT (datetime('now'))
);

-- 资源库
CREATE TABLE library_items (
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

CREATE TABLE library_assets (
  id         INTEGER PRIMARY KEY,
  item_id    INTEGER NOT NULL REFERENCES library_items(id),
  asset_type TEXT NOT NULL CHECK(asset_type IN ('subtitle','edited','poster')),
  file_path  TEXT NOT NULL,
  lang       TEXT,
  created_at TEXT DEFAULT (datetime('now'))
);
```

---

## 9. CLI 模块迁移对照

| 现有文件 | 迁移目标 | 处理方式 |
|----------|----------|----------|
| `src/search.rs` | `commands/search.rs` | 函数不变，加 `#[tauri::command]` 包装 |
| `src/tmdb.rs` | `commands/tmdb.rs` | 同上 |
| `src/download.rs` | `commands/download.rs` | 同上 |
| `src/sub/` | `commands/subtitle.rs` | 合并为单文件，函数不变 |
| `src/tracker.rs` | `commands/tracker.rs` | 同上 |
| `src/ffmpeg.rs` | `commands/media.rs` | 同上 |
| `src/config.rs` | `commands/config.rs` + `config.rs` | Config 结构体保留，加 read/write commands |
| `src/common.rs` | `common.rs`（内部工具） | 不暴露为 command |
| `src/main.rs` | 删除 | 替换为 Tauri `main.rs` |
| `src/error.rs` | `error.rs` | 保留，各 domain error 不变 |
| `src/ai.rs` | 删除 | v3 roadmap，M1 不需要 |

---

## 10. M1 不包含的内容

以下功能在 M1 中置灰/存根，不实现：

- 知识库（影人、流派、Wiki、关系图、影评）→ M2
- 资源库管理、下载功能 → M3
- 字幕管理、媒体工具、ffmpeg 工作流 → M4
- AI 功能 → v3

---

## 11. 关系图查询（参考，供 M2 使用）

```sql
-- 导演 + 所有作品
SELECT f.* FROM films f
JOIN person_films pf ON f.id = pf.film_id
WHERE pf.person_id = :person_id AND pf.role = 'director';

-- 作品 + 全部关联职能角色
SELECT p.*, pf.role FROM people p
JOIN person_films pf ON p.id = pf.person_id
WHERE pf.film_id = :film_id;
```
