# blowup Skill Bridge — 设计文档

**日期**: 2026-04-14
**状态**: Draft
**作者**: lixuan + Claude (Opus 4.6)

## 背景与动机

blowup 的知识库(SQLite 中的 `entries`/`entry_tags`/`relations` 表)目前只能通过 desktop UI 编辑。日常使用中,大量条目的初稿其实可以由 LLM 助手代写 —— 它能搜网络、组织内容、按用户给的写作框架成文,效率远超手写。

直接的方案是让 skill(Claude Code 中的可复用指令包)通过 HTTP API 写 SQLite,但 token 管理是个矛盾点:写到环境变量长期暴露,不写又没法用。我们希望:

- **MCP server 是动态的**: 默认 desktop app 没有任何额外端口/服务/凭证,只在用户主动开启"skill bridge"开关时才存在
- **零持久凭证**: 不依赖长 token,关闭即失效
- **跨 agent 复用**: 不只是 Claude Code,任何支持 MCP 协议的客户端都能用
- **一次写对**: skill 的 tool schema 和指令足够清晰,Claude 不靠试错就能完成完整工作流(查重 → 研究 → 写库 → 加标签 → 加关系)

## 总体架构

```
┌──────────────────┐  stdio (MCP)   ┌──────────────┐  HTTP-over-Unix-socket   ┌──────────────────┐
│  Claude Code     │ ─────────────► │ blowup-mcp   │ ─────────────────────►   │ blowup-tauri     │
│  (or other       │                │ (rmcp-based  │                          │ (axum 同时绑     │
│   MCP client)    │ ◄───────────── │  stdio       │ ◄─────────────────────   │  TCP 17690 +     │
│                  │                │  bridge)     │                          │  Unix socket,    │
│                  │                │              │                          │  socket 由开关   │
│                  │                │              │                          │  控制启停)       │
└──────────────────┘                └──────────────┘                          └──────────────────┘
                                       常驻 stdio                                desktop app 进程
                                       零端口/零状态                              内部
```

**核心原理**:

1. **`blowup-mcp` (新 crate)** 是个 stdio 进程,常驻 Claude Code 的 MCP 配置中。它用 [`rmcp`](https://github.com/modelcontextprotocol/rust-sdk) 注册 9 个 tool,每次 tool 被调用就用 `hyper` + `hyperlocal` 通过 Unix domain socket 发 HTTP 请求到 desktop app,把响应转回 MCP 结果。bridge 进程本身**没有任何端口、状态或服务** —— 只是 stdio ↔ socket 的转发器。
2. **`blowup-tauri`** 在启动时**不**绑 socket。Settings 页面有一个"Skill Bridge"区域,开关打开时:
   - 创建 socket 父目录(`0700`)
   - `axum::serve(unix_listener, build_router(state))` —— **复用现有的 entries router**,不重写任何路由
   - chmod 0600 socket 文件
   - listener 跑在独立 tokio task,handle 保存在 `AppContext::skill_bridge` 中
3. **开关关闭** / desktop 退出: 优雅 shutdown axum task,unlink socket 文件
4. **不走 bearer token middleware** —— Unix socket 的访问控制由文件权限(`0600`)保证,token 反而是冗余

### 接口扩展规则

默认复用 `crates/server/src/routes/entries.rs`。如果开发/测试阶段发现 skill 工作流需要现有 entries API 不提供的能力(例如 wiki 全文搜索、按 tag 组合查询等),在新文件 `crates/server/src/routes/skill.rs` 中新增专用接口,挂在 `/api/v1/skill/*` 前缀下。这些接口对 TCP 17690 也开放(带 bearer token),不做物理隔离,只在 README 中注明语义为 "skill 工作流专用"。

### 跨平台 socket 路径

| OS | 路径 | 备注 |
|----|------|------|
| macOS | `~/Library/Application Support/blowup/skill.sock` | bundle id 的 data dir |
| Linux | `$XDG_RUNTIME_DIR/blowup/skill.sock`,fallback `~/.local/share/blowup/skill.sock` | runtime dir 优先(自动清理) |
| Windows | `\\.\pipe\blowup-skill` | named pipe |

bridge 二进制和 desktop app **必须使用同一个解析函数**(在 `blowup-core` 或 `blowup-server` 中导出),保证一致性。解析函数支持环境变量 `BLOWUP_MCP_SOCKET_OVERRIDE` 强制覆盖,用于测试和调试。

## 组件清单

### 新增

- **`crates/mcp/`** — 新 workspace crate `blowup-mcp`
  - `Cargo.toml` deps: `rmcp`, `hyper`, `hyperlocal` (Linux/macOS), `tokio`, `serde_json`, `anyhow`, `tracing`, `tracing-subscriber`. Windows 用 `tokio::net::windows::named_pipe`
  - `src/main.rs` — 入口: 初始化 tracing,启动 rmcp stdio server
  - `src/tools/` — 9 个 tool 实现,每个 ~20 行: rmcp tool handler → hyper request → 转换响应
  - `src/socket.rs` — 跨平台 socket 路径解析 + 客户端连接
  - `src/error.rs` — `McpError` struct 及 4 层错误映射(见"错误处理")
  - `tests/smoke.rs` — 跨进程 smoke 测试(见"测试策略")

- **`crates/server/src/routes/skill.rs`** — 空 router 占位,为后续 skill-only 接口预留

- **`crates/tauri/src/commands/skill.rs`** — 5 个新 Tauri 命令:
  - `skill_bridge_status() -> SkillBridgeStatus { running: bool, socket_path: Option<String> }`
  - `skill_bridge_start() -> Result<()>` — bind socket,spawn axum task
  - `skill_bridge_stop() -> Result<()>` — shutdown task,unlink socket
  - `skill_bridge_install_to_claude_code() -> Result<InstallReport>`
  - `skill_bridge_get_install_snippets() -> InstallSnippets` — 返回 Claude Desktop / Cursor / Cline / Zed 的 JSON 片段

- **`crates/tauri/resources/skills/blowup-wiki-writer/SKILL.md`** — 预置 skill 文件(见"Skill 内容")

- **`crates/tauri/resources/blowup-mcp`** — bridge 二进制的拷贝目标(build 时 cargo build 后拷入)

### 修改

- **根 `Cargo.toml`** — `workspace.members` 加入 `"crates/mcp"`

- **`crates/server/src/lib.rs`** — 新增 `pub async fn serve_unix(socket_path: &Path, state: AppState, shutdown: oneshot::Receiver<()>) -> std::io::Result<()>`,内部用 `tokio::net::UnixListener::bind` + `axum::serve_with_graceful_shutdown`。**不修改** `build_router`(同一 router 既给 TCP 也给 socket)。

- **`crates/server/src/lib.rs`** 的 `build_router` — 在 routes 列表中加入 `routes::skill::router()`(目前为空)

- **`crates/tauri/src/lib.rs`** 的 `AppContext` — 加 `pub skill_bridge: Arc<Mutex<Option<SkillBridgeHandle>>>` 字段。`SkillBridgeHandle { task: JoinHandle<()>, shutdown_tx: oneshot::Sender<()>, socket_path: PathBuf }`。Drop guard 在 desktop 退出时自动清理。

- **`crates/tauri/tauri.conf.json`** — `bundle.resources` 加 `["resources/skills/**/*", "resources/blowup-mcp"]`

- **`src/pages/Settings.tsx`** — 新增 "Skill Bridge" section: 开关、状态行、socket 路径只读显示、"安装到 Claude Code" 按钮、"其他客户端配置"折叠面板(4 个 JSON snippet + 复制按钮)

- **`src/lib/tauri.ts`** — 新增 `skillBridge` namespace,封装 5 个 invoke

- **`justfile`** — 新增 `build-mcp` recipe,`build` 在打包前先 `build-mcp`(确保 resources 中有最新二进制)

## 关键数据流

### A. 打开开关

1. 用户在 Settings 点 "Skill Bridge" 开关
2. 前端 `invoke("skill_bridge_start")`
3. tauri command:
   - 解析 socket_path(跨平台)
   - 检查父目录,不存在则 `mkdir 0700`
   - **stale socket 处理**: 如果 socket 文件已存在,先 `connect()` 探测;连不上(孤儿文件)→ unlink 后再 bind;连得上 → 返回错误"socket 已被另一进程占用"
   - `UnixListener::bind(socket_path)`
   - `chmod 0600`
   - `tokio::spawn` 一个 task 跑 `serve_unix(listener, ctx.clone(), shutdown_rx)`
   - 把 `JoinHandle` + `shutdown_tx` + `socket_path` 存到 `ctx.skill_bridge`
4. 前端收到 ok → 重新调 `skill_bridge_status()` 刷新 UI

### B. Skill 调一个 tool

```
1. 用户                       → 在 Claude Code 输入 /wiki 情书 1995 按导演的方式写
2. Claude                    → 解析 → 调 list_entries(query="情书")
3. Claude Code MCP runtime   → stdio JSON-RPC 发给 blowup-mcp 进程
4. blowup-mcp tool handler   → connect(socket_path)
                                ↓ 失败
                                返回 MCP error "[FATAL] blowup app 未启用 skill bridge..."
                                ↓ 成功
                              → hyper Request GET /api/v1/entries?query=情书
                              → 收到 Response,反序列化 JSON
                              → 转成 rmcp ToolResult
5. Claude Code MCP runtime   ← stdio 收到结果
6. Claude                    → 看到结果,继续工作流
```

每次 tool 调用都重新 `connect()`,没有连接池 —— Unix socket 的 connect 成本几乎为零,而且这样能让"开关关闭"立即生效。

### C. 关闭开关 / 关闭 desktop

1. 前端开关关闭 / window close / app exit
2. tauri command `skill_bridge_stop` 或 `AppContext` 的 Drop guard:
   - `ctx.skill_bridge.lock().take()`
   - `shutdown_tx.send(())`
   - `task.await`(超时 2s,超时则 abort)
   - `std::fs::remove_file(socket_path)`(忽略 NotFound 错误)
3. 此后 bridge 任何 tool 调用 → `connect()` 失败 → 返回 BRIDGE_OFFLINE

### D. "安装到 Claude Code" 按钮

1. 前端按钮 → `invoke("skill_bridge_install_to_claude_code")`
2. tauri command:
   - 通过 `app.path().resource_dir()` 拿到打包的 `blowup-mcp` 二进制路径
   - 目标稳定路径:
     - macOS/Linux: `$HOME/.local/share/blowup/blowup-mcp`
     - Windows: `%LOCALAPPDATA%\blowup\blowup-mcp.exe`
   - 复制二进制: 先比较源和目标的 SHA256,相同则跳过;不同则覆盖
   - 复制 `resources/skills/blowup-wiki-writer/SKILL.md` 到 `$HOME/.claude/skills/blowup-wiki-writer/SKILL.md`(始终覆盖,因为 skill 文件是预置 spec 的一部分)
   - 调 `Command::new("claude").args(["mcp", "add", "blowup-skill", &binary_path]).status()`
   - 返回 `InstallReport { binary_path, skill_path, claude_added: bool, manual_command: Option<String> }`
3. 前端显示成功 toast;若 `claude_added: false`,显示 `manual_command` 让用户手动跑

### E. Stale socket(desktop crash 后)

不预防(没法预防进程崩溃),只在 `skill_bridge_start` 中**修复**: bind 前先 connect 探测孤儿文件并清理。见流程 A。

## 错误处理(一次写对的核心)

错误分四层,每层有明确的恢复策略:

| 层级 | 错误来源 | bridge 转换 | Claude 应该怎么办 |
|------|---------|------------|------------------|
| **L1 / 连接** | socket 不存在、connect 失败、permission denied | MCP error: `"[FATAL] blowup app 未启用 skill bridge,请在 desktop 设置中打开 'Skill Bridge' 开关后重试"` + `code: BRIDGE_OFFLINE`,`retryable: false` | **不重试**,停止并把错误**原样**告诉用户 |
| **L2 / 协议** | 传输 OK 但 HTTP 5xx、body 解析失败 | MCP error: `"[FATAL] blowup app 内部错误: <详情>"` + `code: INTERNAL`,`retryable: false` | **不重试**,停止报告,**不要捏造内容** |
| **L3 / 业务** | 4xx,例如 entry not found / relation type 不合法 / tag 已存在。core 已用 `status::not_found(...)` / `status::bad_request(...)` 前缀标记 | MCP error: `"<原始中文消息>"` + `code: BAD_REQUEST` 或 `NOT_FOUND` + `hint`(由 bridge hard-code,如 "提示:请先用 list_relation_types 查看可用的关系类型"),`retryable: true` | **重试一次**,根据 hint 调整参数;仍失败则停止 |
| **L4 / Schema** | rmcp 在 tool 调用前就拒绝(参数缺失/类型错) | rmcp 自动返回 invalid_params | Claude 直接根据 schema 重新构造 |

### Schema 是第一道防线

每个 tool 的 args struct 用 `#[derive(JsonSchema)]` + `#[schemars(description = "...")]` 把所有约束写到字段上。例:

```rust
/// 创建一个新的知识库条目并返回其 ID。
/// 调用前必须先用 `list_entries(query=name)` 查重 ——
/// 同名条目存在时应改为 `update_wiki`,而不是新建。
#[derive(Deserialize, JsonSchema)]
struct CreateEntryArgs {
    /// 条目名称,中文,不含书名号 / 引号 / 年份后缀。
    /// 例:`情书` (✓), `《情书》` (✗), `情书 (1995)` (✗)
    #[schemars(min_length = 1, max_length = 200)]
    name: String,
}
```

skill 文件中的指令是这些约束的**复述**,作为冗余保险。

### 统一错误结构

```rust
pub struct McpError {
    pub code: ErrorCode,        // BRIDGE_OFFLINE / INTERNAL / BAD_REQUEST / NOT_FOUND
    pub message: String,        // 中文,直接展示给 Claude
    pub hint: Option<String>,   // 修复建议,L3 才有
    pub retryable: bool,        // L3=true, 其他=false
}
```

`retryable: false` 的错误消息一律加前缀 `"[FATAL] "`,Claude 看到就知道不该重试。

**核心信条**: 任何错误都不该让 Claude 困惑要不要重试 —— 二选一,要么 "调一次就停",要么 "调一次就改"。

## MCP Tools (9 个)

读类:

1. `list_entries(query?, tag?) -> Vec<EntrySummary>` — GET /api/v1/entries
2. `get_entry(id) -> EntryDetail` — GET /api/v1/entries/{id}
3. `list_all_tags() -> Vec<String>` — GET /api/v1/entries/tags
4. `list_relation_types() -> Vec<String>` — GET /api/v1/entries/relation-types

写类:

5. `create_entry(name) -> i64` — POST /api/v1/entries
6. `update_wiki(id, wiki) -> ()` — PUT /api/v1/entries/{id}/wiki
7. `update_name(id, name) -> ()` — PUT /api/v1/entries/{id}/name
8. `add_tag(entry_id, tag) -> ()` — POST /api/v1/entries/{id}/tags
9. `add_relation(from_id, to_id, relation_type) -> i64` — POST /api/v1/entries/relations

每个 tool 的描述、参数 doc comment、修复 hint 在实现阶段细化。原则:中文、明确、举例。

## Skill 内容(`SKILL.md` 大纲)

```markdown
---
name: blowup-wiki-writer
description: 给 blowup 知识库写 wiki 条目。用户会指定条目名称和(可选的)写作角度,
             skill 自动完成研究、生成内容、写库、加标签、加关系。需要 desktop app
             启用 skill bridge 开关。
---

# Blowup Wiki Writer

## 前置条件
调用任何 tool 之前,确认 desktop app 的 skill bridge 开关已打开。
若 tool 报错以 "[FATAL]" 开头,直接停止并把错误原样告诉用户,不重试。

## 工作流
1. **解析用户意图** — 提取条目名称 + 可选的写作框架
2. **查重** — list_entries(query=名称)
   - 已存在 → 问用户"已有同名条目 #N,要更新还是新建变体?"
   - 不存在 → 继续
3. **了解上下文** — 并行调 list_all_tags() + list_relation_types();
   根据写作框架决定要查找哪些相关条目,用 list_entries(query=...) 找到现有相关条目的 id
4. **网络搜索** — 用 WebSearch / WebFetch 收集事实
5. **生成 wiki markdown** — 严格按用户给的写作框架,中文,不主观吹捧,注明信息来源
6. **写库**:
   a. create_entry(name) → 拿到 id
   b. update_wiki(id, markdown)
   c. 复用 list_all_tags 的结果,挑出最匹配的现有标签 add_tag,
      只在确实需要时新建标签
   d. add_relation 把相关条目串起来
7. **报告完成** — 一句话告诉用户"条目 #N 已写入,desktop app 应该已自动刷新"

## 写作硬约束
- 全部中文
- 不主观评价(如"经典"、"伟大",除非引述他人观点并注明来源)
- 涉及具体年份、人名、获奖记录等事实必须有来源
- 信息冲突时,标记不确定性而非选一个写

## 失败处理
- tool 返回业务错误(中文,不带 [FATAL]):按 hint 调整参数后**重试一次**,仍失败则停止
- tool 返回 [FATAL] 错误:停止并报告,不重试
- 不要为了"完成"而捏造内容
```

## 测试策略

| 层 | 范围 | 实现 |
|----|------|------|
| **U1 / blowup-mcp 单元** | tool 实现里 hyper request 构造和响应解析的纯逻辑 | `#[tokio::test]`,`tower::ServiceExt::oneshot` 喂 mock router |
| **U2 / blowup-server `serve_unix`** | UnixListener bind / shutdown / cleanup | `#[tokio::test]`: tempdir 中创建 socket,启动 serve_unix,hyperlocal 客户端发 GET /api/v1/health,验证响应,发 shutdown,验证 task 退出和 socket 文件被删 |
| **U3 / 跨进程 smoke** | 真实 stdio + 真实 unix socket | `crates/mcp/tests/smoke.rs`: `TestHarness` 在 tempdir 启 socket server(挂最小 router),`Command::new(env!("CARGO_BIN_EXE_blowup-mcp"))` spawn bridge,通过 stdin 写 MCP `initialize` + `tools/list` + `tools/call`,从 stdout 读响应,断言。socket 路径用 `BLOWUP_MCP_SOCKET_OVERRIDE` 环境变量注入 |
| **U4 / desktop ↔ bridge 手动** | 真实流程的最后一公里 | `just dev` 起 desktop → 开开关 → `claude mcp add` → Claude Code 中 `/wiki ...`。手动 checklist,跑完后记到本 spec 底部"验证记录"section |

**关键约束**: socket 路径解析函数**必须**先看 `BLOWUP_MCP_SOCKET_OVERRIDE` 环境变量,fallback 到默认路径。否则 U3 跨进程测试没法跑。

**不测**:

- rmcp 的 stdio 解析(假设 rmcp 自己的测试是对的)
- axum entries 路由(已被 `crates/server/tests/smoke.rs` 覆盖)
- Settings UI 的渲染(手动 visual check)

CI 集成: U1 + U2 + U3 进 `just check` 跑的 `cargo test`。

## YAGNI(本期不做)

- 多 desktop 实例并存 — 单用户单机不会发生
- 加密 socket 通信 — 文件权限 0600 已足够
- Token 认证 — 同上
- 自动检测 desktop crash 并恢复 — 用户重开开关即可
- 配置开关持久化 — 会话级是有意的安全选择
- bridge 自动升级 — 重装 desktop 即可
- 多 skill 变体(电影 / 影人 / 流派各自独立) — 一个通用 skill 配运行时风格指令足够
- iOS 客户端集成 skill — 它有自己的工作流,本期不考虑

## 验证记录

**Date:** 2026-04-14
**Platform:** macOS 25.4.0 (Darwin)
**Implementation:** 37 commits on `main`, T1 → T20 complete

### 自动验证(已完成)

| Step | Result | Notes |
|------|--------|-------|
| `cargo test --workspace` | ✅ 148 tests pass | 121 core + 11 server smoke + 10 mcp lib + 3 socket + 1 server unix + 1 mcp smoke + 1 server smoke breakdown |
| `cargo build -p blowup-mcp` (release) | ✅ Clean | 4.8 MB binary at `crates/tauri/resources/blowup-mcp` |
| `cargo build -p blowup-tauri` | ✅ Clean | Only pre-existing `ar -D` build script warnings unrelated to this work |
| `bunx tsc --noEmit` | ✅ Clean | All TS bindings + Settings.tsx type-check |
| `crates/mcp/tests/smoke.rs` | ✅ Pass | Full JSON-RPC handshake (initialize → tools/list → tools/call list_entries → tools/call create_entry) against a tempdir Unix socket router; verifies all 9 tools appear in tools/list and the rmcp 1.4 wiring is correct |
| `crates/server/tests/serve_unix.rs` | ✅ Pass | Real HTTP-over-Unix-socket request through the bridged blowup-server router with bearer token |

### 待手动验证(由用户在桌面环境中跑完)

| Step | Notes |
|------|-------|
| 1. `just build-mcp && just build` produces an installable Tauri bundle | Run on the developer machine; the bundler step takes several minutes |
| 2. `just dev` opens the desktop app, Settings → "Skill Bridge" section renders | Verify the section appears at the bottom of Settings, with a Switch + Install button + collapsible snippets panel |
| 3. Click "安装到 Claude Code" | Verify `~/Library/Application Support/blowup/blowup-mcp` exists and is executable; `~/.claude/skills/blowup-wiki-writer/SKILL.md` exists; install message shows success or the manual `claude mcp add` fallback |
| 4. Toggle the Switch ON | Verify status flips to `运行中`; verify socket file exists at `~/Library/Application Support/blowup/skill.sock` with `srw-------` (0600) perms |
| 5. Direct JSON-RPC test from terminal | `echo '{"jsonrpc":"2.0","id":1,"method":"initialize",...}' \| timeout 3 ~/Library/Application\ Support/blowup/blowup-mcp` should produce a JSON-RPC response on stdout |
| 6. Real Claude Code test | In a fresh `claude` session: "列出 blowup 知识库现有的所有标签" — should call `mcp__blowup-skill__list_all_tags` and return the existing tags |
| 7. End-to-end wiki write test | "帮我写一个 blowup wiki 条目: 岩井俊二, 按导演的角度写" — should walk through the SKILL.md workflow (查重 → 上下文 → web search → write → tags → relations → done). Switch to the desktop's 知识库 tab and verify the new entry appears with correct content (event bus auto-refresh) |
| 8. Toggle the Switch OFF | Verify socket file gone; retry the JSON-RPC echo from step 5 → expect `[FATAL] blowup app 未启用 skill bridge` error; retry the Claude prompt → expect Claude to surface the FATAL message and stop |
| 9. Stale socket recovery | Toggle ON → force-kill desktop with `pkill -9 blowup-tauri` → confirm orphan socket file remains → restart `just dev` → toggle ON → verify success (the `handle_stale_socket` connect-probe should detect and unlink the orphan) |
| 10. Window-close cleanup | Toggle ON → close the main window via the red traffic-light button → verify socket file is gone (the `on_window_event` hook gated on `window.label() == "main"` should run `shutdown_blocking`) |
| 11. Multi-window non-interference | Toggle ON → open the player popout window → close the player → verify the bridge is STILL running (the main-window guard from T14 review fix should prevent the player close from triggering shutdown) |

When all manual steps pass, mark each row ✅ and commit.
