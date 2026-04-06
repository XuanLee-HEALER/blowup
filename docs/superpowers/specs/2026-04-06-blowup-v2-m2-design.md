# blowup v2 · M2 设计规格

**日期**: 2026-04-06  
**状态**: 已批准  
**里程碑**: M2 — 知识库（Knowledge Base）  
**分支**: v2

---

## 1. 背景与目标

M1 建立了 Tauri v2 桌面应用骨架、TMDB 搜索页和 Settings 页，侧边栏中"影人/流派/关系图"三项已置灰占位。M2 解锁这三个入口，实现以个人品味为核心的**电影知识库**：

- 以导演为中心的**影人档案**（传记 Wiki + 作品 + 影人关系）
- **电影流派**树（层级管理 + 流派 Wiki + 关联影人/作品）
- **关系图谱**（D3 力导向，所有影人 + 作品节点，轨道旋转特效）
- 每部作品的**特点介绍 Wiki** + **影评**（个人评分文字 + 收录影评人评论）
- **背景音乐播放器**（仅知识库模块生效，配置驱动）
- 搜索页"加入知识库"按钮解锁

所有内容为**手动策划**，用户自己决定收录哪些影人与作品。

---

## 2. 技术栈新增

| 新增 | 说明 |
|------|------|
| `d3` | 力导向图谱，SVG 渲染，`@types/d3` 配套 |
| 浏览器原生 `HTMLAudioElement` | 背景音乐，无需额外库 |
| Tauri `convertFileSrc()` | 将本地文件路径转为可访问的 asset URL |

Markdown 渲染使用轻量库 `marked`（纯 JS，无 deps）。

---

## 3. 数据库

M1 已创建所有必要表，M2 **不新增迁移文件**。用到的表：

| 表 | 用途 |
|----|------|
| `people` | 影人档案 |
| `films` | 电影档案 |
| `genres` | 流派（支持父子层级 via `parent_id`） |
| `person_films` | 影人-电影关联（含角色） |
| `film_genres` | 电影-流派关联 |
| `person_genres` | 影人-流派关联 |
| `person_relations` | 影人间关系（influenced/contemporary/collaborated） |
| `wiki_entries` | Markdown 正文（person/film/genre 三种 entity） |
| `reviews` | 影评（is_personal=1 本人，is_personal=0 收录） |

---

## 4. 项目结构变更

```
src-tauri/src/commands/
  library.rs          ← 新增，知识库所有 CRUD 命令

src/pages/
  People.tsx          ← 解锁（原 Placeholder）
  Genres.tsx          ← 解锁（原 Placeholder）
  Graph.tsx           ← 解锁（原 Placeholder）

src/components/
  WikiEditor.tsx      ← 新增，Write/Preview 标签切换
  ReviewSection.tsx   ← 新增，影评展示+编辑
  FilmDetailPanel.tsx ← 从 Search.tsx 提取为共用组件
  MusicPlayer.tsx     ← 新增，悬浮迷你播放条

src/lib/tauri.ts      ← 扩展：新增 library 命令的 invoke 类型
src/pages/Settings.tsx← 扩展：新增"背景音乐"配置区块
src/App.tsx           ← 扩展：注入 MusicPlayer，检测知识库路由
src-tauri/src/config.rs ← 扩展：新增 MusicConfig / MusicTrack
```

---

## 5. Rust 命令层（library.rs）

### 5.1 数据结构

```rust
#[derive(Serialize)]
pub struct PersonSummary {
    pub id: i64,
    pub name: String,
    pub primary_role: String,
    pub film_count: i64,
}

#[derive(Serialize)]
pub struct PersonDetail {
    pub id: i64,
    pub tmdb_id: Option<i64>,
    pub name: String,
    pub primary_role: String,
    pub born_date: Option<String>,
    pub nationality: Option<String>,
    pub biography: Option<String>,       // raw TMDB bio（只读参考）
    pub wiki_content: String,            // wiki_entries 内容，空串表示未写
    pub films: Vec<PersonFilmEntry>,
    pub relations: Vec<PersonRelation>,
}

#[derive(Serialize)]
pub struct PersonFilmEntry {
    pub film_id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub role: String,
    pub poster_cache_path: Option<String>,
}

#[derive(Serialize)]
pub struct PersonRelation {
    pub target_id: i64,
    pub target_name: String,
    pub direction: String,   // "from" | "to"
    pub relation_type: String,
}

#[derive(Serialize)]
pub struct FilmDetail {
    pub id: i64,
    pub tmdb_id: Option<i64>,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub tmdb_rating: Option<f64>,
    pub poster_cache_path: Option<String>,
    pub wiki_content: String,
    pub people: Vec<FilmPersonEntry>,
    pub genres: Vec<GenreSummary>,
    pub reviews: Vec<ReviewEntry>,
}

#[derive(Serialize)]
pub struct FilmPersonEntry {
    pub person_id: i64,
    pub name: String,
    pub role: String,
}

#[derive(Serialize)]
pub struct ReviewEntry {
    pub id: i64,
    pub is_personal: bool,
    pub author: Option<String>,
    pub content: String,
    pub rating: Option<f64>,   // 0.5 步进，最高 10.0
    pub created_at: String,
}

#[derive(Serialize)]
pub struct GenreSummary {
    pub id: i64,
    pub name: String,
    pub film_count: i64,
    pub child_count: i64,
}

#[derive(Serialize)]
pub struct GenreDetail {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub parent_id: Option<i64>,
    pub period: Option<String>,
    pub wiki_content: String,
    pub children: Vec<GenreSummary>,
    pub people: Vec<PersonSummary>,
    pub films: Vec<FilmSummary>,
}

#[derive(Serialize)]
pub struct FilmSummary {
    pub id: i64,
    pub title: String,
    pub year: Option<i64>,
    pub tmdb_rating: Option<f64>,
    pub poster_cache_path: Option<String>,
}

// 流派树（递归嵌套）
#[derive(Serialize)]
pub struct GenreTreeNode {
    pub id: i64,
    pub name: String,
    pub period: Option<String>,
    pub film_count: i64,
    pub children: Vec<GenreTreeNode>,
}

// 搜索页"加入知识库"输入
#[derive(Deserialize)]
pub struct TmdbMovieInput {
    pub tmdb_id: i64,
    pub title: String,
    pub original_title: Option<String>,
    pub year: Option<i64>,
    pub overview: Option<String>,
    pub tmdb_rating: Option<f64>,
    pub poster_path: Option<String>,
    pub people: Vec<TmdbPersonInput>,
}

#[derive(Deserialize)]
pub struct TmdbPersonInput {
    pub tmdb_id: Option<i64>,
    pub name: String,
    pub role: String,          // person_films.role（如 "director"）
    pub primary_role: String,  // people.primary_role
}

// 图谱数据
#[derive(Serialize)]
pub struct GraphData {
    pub nodes: Vec<GraphNode>,
    pub links: Vec<GraphLink>,
}

#[derive(Serialize)]
pub struct GraphNode {
    pub id: String,       // "p{id}" or "f{id}"
    pub label: String,
    pub node_type: String, // "person" | "film"
    pub role: Option<String>,
    pub weight: f64,      // 归一化影响力，用于节点大小
}

#[derive(Serialize)]
pub struct GraphLink {
    pub source: String,
    pub target: String,
    pub role: String,
}
```

### 5.2 Tauri 命令列表

**影人**
```rust
#[tauri::command] list_people() -> Vec<PersonSummary>
#[tauri::command] get_person(id: i64) -> PersonDetail
#[tauri::command] create_person(name, primary_role, tmdb_id?, born_date?, nationality?) -> i64
#[tauri::command] update_person_wiki(id: i64, content: String)
#[tauri::command] delete_person(id: i64)
#[tauri::command] add_person_relation(from_id, to_id, relation_type)
#[tauri::command] remove_person_relation(from_id, to_id, relation_type)
```

**电影**
```rust
#[tauri::command] list_films() -> Vec<FilmSummary>
#[tauri::command] get_film(id: i64) -> FilmDetail
#[tauri::command] add_film_from_tmdb(tmdb_movie: TmdbMovieInput) -> i64
    // TmdbMovieInput: { tmdb_id, title, original_title, year, overview, tmdb_rating, poster_path }
    // 同时接收 people: Vec<{tmdb_id?, name, role, primary_role}> 批量写入
#[tauri::command] update_film_wiki(id: i64, content: String)
#[tauri::command] delete_film(id: i64)
```

**流派**
```rust
#[tauri::command] list_genres_tree() -> Vec<GenreTreeNode>
    // 返回带 children 嵌套的完整树
#[tauri::command] get_genre(id: i64) -> GenreDetail
#[tauri::command] create_genre(name, parent_id?, description?, period?) -> i64
#[tauri::command] update_genre_wiki(id: i64, content: String)
#[tauri::command] delete_genre(id: i64)
#[tauri::command] link_film_genre(film_id: i64, genre_id: i64)
#[tauri::command] unlink_film_genre(film_id: i64, genre_id: i64)
#[tauri::command] link_person_genre(person_id: i64, genre_id: i64)
#[tauri::command] unlink_person_genre(person_id: i64, genre_id: i64)
```

**影评**
```rust
#[tauri::command] add_review(film_id, is_personal, author?, content, rating?) -> i64
#[tauri::command] update_review(id: i64, content: String, rating: Option<f64>)
#[tauri::command] delete_review(id: i64)
```

**图谱**
```rust
#[tauri::command] get_graph_data() -> GraphData
    // 查询所有 films + people + person_films
    // weight = person 的 film_count 归一化到 [0.5, 3.0]
```

---

## 6. 前端页面设计

### 6.1 影人页（People.tsx）

**布局**：左侧列表（260px）+ 右侧详情面板（flex-1）。

**左侧列表**
- 顶部"+ 添加影人"按钮（弹窗：姓名 + primary_role + 可选 TMDB ID）
- 每行：姓名、角色标签、作品数
- 支持按 primary_role 分组 / 字母排序切换

**右侧详情面板**
```
[姓名]  [角色标签]
出生：YYYY  国籍：XXX
──────────────────────
[传记 Wiki — WikiEditor]
──────────────────────
作品列表（含角色）
  [海报缩图] 标题 (年份) · 角色  →可点击打开电影详情
──────────────────────
影人关系
  → 影响了：[A] [B]
  ← 受影响于：[C]
  ↔ 同时期：[D]
  [+ 添加关系]
```

所有编辑失焦即保存，无独立编辑页面。

### 6.2 流派页（Genres.tsx）

**布局**：左侧可折叠树（260px）+ 右侧详情面板（flex-1）。

**左侧树**
- 缩进层级，每节点显示名称 + 电影数
- Hover 显示"添加子流派"
- 顶部"+ 添加流派"按钮（弹窗：名称 + 描述 + 父流派 + 年代区间）

**右侧详情面板**
```
[流派名]  [年代区间]
──────────────────────
[简介 Wiki — WikiEditor]
──────────────────────
关联影人（带角色标签）  [+ 关联影人]
──────────────────────
收录电影              [+ 从知识库选择]
──────────────────────
子流派列表
```

"从知识库选择"弹窗：搜索 + 勾选已收录电影。

### 6.3 关系图（Graph.tsx）

**全屏布局**：无固定侧边内容，工具栏浮动在右上角。

**D3 配置**
```
linkDistance:    120px（人→电影）
chargeStrength: -200
collideRadius:   按 weight 动态，最小 20px
alphaDecay:      0.02（慢收敛，保持动感）
```

**轨道旋转特效**
- 力模拟稳定后（alpha < 0.01）启动 `d3.timer`
- 每帧：对每个 film 节点，遍历其直连 person 节点，施加切向偏移
  - `angle += 0.002`（约 50 秒一圈）
  - `dx = -sin(angle) * orbitRadius * 0.3`
  - `dy =  cos(angle) * orbitRadius * 0.3`
- 用 `simulation.restart()` 叠加，不重置速度

**节点视觉**
- film：金色描边（`#C5A050`），深蓝填充（`#122040`），固定半径 18px
- person（director）：金色填充，半径 `8 + weight * 6`
- person（other）：白色低透明填充，半径 `6 + weight * 4`

**交互**
- 悬停：高亮直连子图，其余 opacity 0.15
- 点击：右侧迷你卡片（名称/年份/评分 + "查看详情"按钮）
- 工具栏：重置视角、暂停/恢复旋转、全屏

### 6.4 WikiEditor 组件

```
[Write] [Preview]    ← 标签切换
─────────────────────
Write 模式：<textarea> 全宽，monospace，行号可选
Preview 模式：marked() 渲染输出，基础 Markdown 样式
```

Props: `value: string, onChange: (v: string) => void, onSave: () => void`  
失焦自动调用 `onSave`。

### 6.5 ReviewSection 组件

**个人影评区块**
- 评分滑块/点击选择：0.5 步进，10 个圆点显示
- textarea：最多 500 字，字数倒计时显示
- 失焦保存（upsert：已有则 update，无则 insert）

**收录影评区块**
- 列表显示：作者名 + 可选评分 + 正文（折叠超过 3 行）
- "+" 按钮：弹窗输入作者 + 可选评分 + 正文

### 6.6 FilmDetailPanel 组件（从 Search.tsx 提取）

M2 将 Search.tsx 里的 `FilmDetailPanel` 提取为 `src/components/FilmDetailPanel.tsx`，解锁"加入知识库"按钮。

**"加入知识库"弹窗**
1. 展示 TMDB 数据（标题、年份、导演、主要演员）
2. 用户勾选哪些影人收录 + 指定每人的 primary_role
3. 确认 → 调用 `add_film_from_tmdb` 批量写入

---

## 7. 背景音乐播放器

### 7.1 Config 扩展

```toml
[music]
enabled = false
mode    = "sequential"   # "sequential" | "random"

[[music.playlist]]
src  = "/path/to/file.mp3"
name = "曲目名称"

[[music.playlist]]
src  = "https://example.com/ambient.mp3"
name = "在线曲目"
```

**Rust 结构体**（新增到 `config.rs`）
```rust
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct MusicConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_music_mode")]
    pub mode: String,           // "sequential" | "random"
    #[serde(default)]
    pub playlist: Vec<MusicTrack>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MusicTrack {
    pub src: String,
    pub name: String,
}
```

### 7.2 MusicPlayer 组件

**触发条件**：`useLocation().pathname` 以 `/people`、`/genres`、`/graph` 开头时激活；离开时调用 `audio.pause()`。

**实现**
- `useRef<HTMLAudioElement>` 管理 Audio 实例
- 本地路径用 `import { convertFileSrc } from "@tauri-apps/api/core"` 转换
- 在线 URL 直接使用
- 顺序/随机模式在 `onended` 事件中处理下一首逻辑

**UI**：固定在内容区右下角（`position: fixed, bottom: 1.25rem, right: 1.25rem`），`z-index: 50`。

```
♪  曲目名称          ▶ ⏭ 🔀
   ████░░░░  2:14 / 4:32
```

### 7.3 Settings 新增区块

```
背景音乐
  启用              [开关]
  播放模式          [顺序 / 随机]
  播放列表
    曲目名称  [路径/URL输入]  [选择文件]  [删除]
    ...
    [+ 添加曲目]
```

每条曲目修改即调用 `set_config_key` 写入（playlist 整体替换）。新增 `set_music_playlist` 命令接收完整 playlist 数组。

---

## 8. M2 不包含的内容

- 海报缓存下载（poster_cache_path 字段预留，M3 下载模块实现）
- 知识库导出/导入
- 多语言 Wiki
- TMDB 自动同步（手动录入即可）

---

## 9. 侧边栏变更

M2 解锁三个导航项，从 `disabled: true` 改为可用：
- 影人（`/people`）
- 流派（`/genres`）
- 关系图（`/graph`）

Placeholder 组件不再使用，三个页面替换为实际实现。
