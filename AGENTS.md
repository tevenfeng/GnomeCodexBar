# Agents

本文件为 AI 编码助手提供项目上下文，帮助理解项目架构、约定和注意事项。

## 项目概述

GnomeCodexBar 是一个 **Rust CLI 后端 + 平台原生前端** 的双进程架构项目，用于在顶栏实时监控 DeepSeek、StepFun 和 OpenCode Go 的编程套餐用量。

- **后端（Rust CLI）**：定时拉取 API 数据，写入 `status.json`
- **Linux 前端（GNOME Shell Extension）**：通过 `Gio.FileMonitor` 监听 `status.json` 变化，渲染顶栏控件和弹出详情窗口
- **macOS 前端（SwiftUI Menu Bar App）**：通过 `DispatchSource` 监听 `status.json` 变化，渲染菜单栏指示器和弹出详情窗口

两者通过文件系统解耦，无直接通信。

## 架构与数据流

```
Rust CLI daemon                                       GNOME Shell Extension
  ─────────────                                          ────────────────────
  Config::load()──┬                                      ConfigManager ──┐
                  │                                        │              │
                  ▼                                        ▼              ▼
              config.toml  ◄──── 两边读写同一个文件 ────  config.toml     
                  │                                                       │
                  ▼                                                       ▼
  DeepSeek: GET /user/balance                         extension.js
  StepFun:  3-step login → QueryStepPlanRateLimit      ├── 顶栏按钮
  OpenCode: Console Cookie → /console/api/go/status    ├── 弹出窗口
                  │                                    ├── Provider 过滤
                  ▼                                    ├── 轮询定时器
              status.json  ──── Gio.FileMonitor ────→  └── 详情渲染
```

- **配置源**：`config.toml` 是刷新间隔和 Provider 开关的**唯一真源**
- **CLI daemon**：每轮循环重新加载 `config.toml`，修改后无需重启
- **Extension**：通过 `ConfigManager` 读写 `config.toml`，`Gio.FileMonitor` 监听变化
- **GNOME 设置页**：修改刷新间隔和 Provider 开关直接写入 `config.toml`
- **macOS 设置页**：同样读写同一个 `config.toml`（平台对应路径）

## 关键文件

### Rust 后端 (`cli/`)

| 文件 | 职责 |
|------|------|
| `src/main.rs` | CLI 入口，clap 子命令定义 |
| `src/daemon.rs` | 守护进程循环 + `fetch_all()` 并行拉取所有 Provider |
| `src/autostart.rs` | 开机自启动管理（Linux systemd / macOS launchd） |
| `src/config.rs` | TOML 配置加载（与 status.json 同目录，自动从旧路径迁移） |
| `src/output.rs` | 写入 `status.json` + `selected_provider.json` |
| `src/browser_cookies.rs` | 从本机浏览器读取会话 Cookie（Chromium 解密 / Firefox 明文） |
| `src/providers/mod.rs` | `Provider` trait + `ProviderStatus` / `StatusSnapshot` 类型定义 |
| `src/providers/deepseek.rs` | DeepSeek Provider：API Key 认证 + 余额查询 |
| `src/providers/stepfun.rs` | StepFun Provider：3-step 登录 + 用量查询 + 套餐查询 |
| `src/providers/opencodego.rs` | OpenCode Go Provider：Console 会话 Cookie + `/console/api` 用量查询 |

### GNOME Shell 扩展 (`gnome-shell-extension/`)

| 文件 | 职责 |
|------|------|
| `extension.js` | 主扩展类：顶栏按钮 + popup 构建 + 详情渲染（当前活跃版本） |
| `configManager.js` | 轻量 TOML 读写（`config.toml`），管理刷新间隔和 Provider 开关 |
| `statusReader.js` | 读取 `status.json` + `selected_provider.json` + 文件监听 |
| `prefs.js` | GNOME 设置页：刷新间隔 + Provider enable/disable 开关 |
| `stylesheet.css` | 所有样式定义 |

### macOS 菜单栏应用 (`macos-bar/`)

| 文件 | 职责 |
|------|------|
| `Sources/CodexBarApp.swift` | @main 入口，MenuBarExtra(.window) 菜单栏指示器 |
| `Sources/StatusReader.swift` | 读取 `status.json` + `selected_provider.json` + 文件监听 (DispatchSource) |
| `Sources/Models.swift` | StatusSnapshot / ProviderStatus / JSONValue 数据模型 |
| `Sources/PopoverContent.swift` | 弹出窗口内容：标题行 + Provider 卡片列表 + 刷新按钮 |
| `Sources/ProviderCardView.swift` | 单个 Provider 卡片：头部 + 进度条 + 详情 |
| `Sources/BarSection.swift` | 进度条区域：标题 + 条 + 信息行 |

## 核心约定

### status.json 数据结构

```json
{
  "updated_at": "2026-05-06T12:00:00+00:00",
  "providers": [
    {
      "provider_id": "deepseek",
      "provider_name": "DeepSeek",
      "available": true,
      "remaining_percent": 100.0,
      "details": {
        "currency": "CNY",
        "total_balance": 10.5,
        "granted_balance": 5.0,
        "topped_up_balance": 5.5
      },
      "error": null
    },
    {
      "provider_id": "stepfun",
      "provider_name": "StepFun",
      "available": true,
      "remaining_percent": 85.0,
      "details": {
        "plan_name": "Plus",
        "five_hour_usage_left_rate": 0.85,
        "weekly_usage_left_rate": 0.92,
        "five_hour_usage_reset_time": "2026-05-06T17:00:00+00:00",
        "weekly_usage_reset_time": "2026-05-12T00:00:00+00:00"
      },
      "error": null
    }
  ]
}
```

### Provider trait

新增 Provider 需实现 `Provider` trait（`cli/src/providers/mod.rs`）：

```rust
#[async_trait::async_trait]
pub trait Provider: Send + Sync {
    fn id(&self) -> &'static str;        // 唯一标识，如 "stepfun"
    fn name(&self) -> &'static str;      // 显示名称，如 "StepFun"
    async fn fetch(&self, config: &ProviderConfig) -> Result<ProviderStatus, anyhow::Error>;
}
```

然后在 `daemon.rs` 的 `fetch_all()` 中注册并并行拉取。

### StepFun 认证流程

StepFun 不支持 API Key，需 3-step Cookie 登录：

1. `GET https://platform.stepfun.com` → 提取 `INGRESSCOOKIE`
2. `POST RegisterDevice` (空 body + INGRESSCOOKIE) → 获取匿名 `accessToken` + `refreshToken`，拼接为 `Oasis-Token`
3. `POST SignInByPassword` (username/password + Oasis-Token + INGRESSCOOKIE) → 获取认证 `Oasis-Token`

之后用 `Oasis-Token` Cookie 调用数据 API。

### StepFun API 灵活类型

StepFun API 返回的 JSON 字段类型不稳定（有时 int 有时 float/string），因此有自定义反序列化器：

- `FlexibleNumber`：兼容 int/float/string → `f64`
- `FlexibleTimestamp`：兼容 int/string → `i64`（秒级时间戳）
- `FlexibleIntOrString`：兼容 int/string → `i64`

新增 StepFun API 解析时务必使用这些类型。

### remaining_percent 语义

- **DeepSeek**：二值逻辑（余额 > 0 → 100%，否则 0%）
- **StepFun**：`five_hour_usage_left_rate * 100`（0-100 范围，表示 5h 窗口剩余比例）
- **OpenCode Go**：`(limitMicroCents - usedMicroCents) / limitMicroCents * 100`，
  优先取 `fiveHour` 窗口，缺失时依次回退到 `week`、`month`

前端根据 `remaining_percent` 决定进度条颜色：
- ≥50%：绿色（`high`）
- 20-50%：黄色（`medium`）
- <20%：红色（`low`）

### OpenCode Go 数据来源

OpenCode 网页端自 2026 年起改为客户端渲染的 Console SPA（`/console/`），HTML 中不再包含用量数据，
因此必须走 JSON 接口（`cli/src/providers/opencodego.rs`）：

1. `GET https://opencode.ai/console/api/orgs` → `[{"id":"wrk_…","name":"Default"}]`（orgId 即原 workspace id）
2. `GET https://opencode.ai/console/api/go/status`（**必须带 `x-org-id` 请求头**）→ 订阅与 `access.meters`

鉴权用 Console 的会话 Cookie `__Host-console_session`（HTTPS 下带 `__Host-` 前缀）。Cookie 获取优先级：

`config.toml` 的 `cookie_header` → 环境变量 → 本地浏览器自动读取（401 时会回退到浏览器）

响应字段的坑：`limitMicroCents` / `usedMicroCents` 是**字符串**（BigInt），`access` 实际是**对象**
（官方 schema 写的是数组），未启用的 5h 窗口 `resetsAt` 为 `null`。解析时务必用 `FlexibleAmount`
并同时兼容对象/数组两种 `access`（`AccessField`）。

### 浏览器 Cookie 读取（`cli/src/browser_cookies.rs`）

Chromium 系浏览器的 Cookie 值加密存储：

| 平台 | 密钥来源 | PBKDF2 迭代 |
|------|---------|------------|
| Linux | Secret Service 的 `chrome_libsecret_os_crypt_password_v2` 项 | 1 |
| macOS | 钥匙串 `<浏览器> Safe Storage` | 1003 |

统一算法：`AES-128-CBC`，IV = 16 个空格，密钥 = `PBKDF2-HMAC-SHA1(密码, "saltysalt", 迭代数, 16)`；
新版 Chromium 会在明文前加 **32 字节随机前缀**，解密后需剥掉。Firefox 系为明文，直接读 `cookies.sqlite`。

> 注意：Edge 的 Linux 构建会把密钥存在 **`application=chromium`** 的 "Chromium Safe Storage" 项下，
> 因此实现是「列出所有 Safe Storage 密钥逐个尝试解密」，而不是按浏览器名精确查找。
> 读取数据库前先复制到临时目录（含 `-wal`/`-shm`），避免浏览器占用锁。

## 文件路径

### Linux (ZorinOS)

| 路径 | 用途 |
|------|------|
| `~/.local/share/gnome-codex-bar/config.toml` | CLI 配置 |
| `~/.local/share/gnome-codex-bar/status.json` | 用量数据（CLI 写 → 扩展读） |
| `~/.local/share/gnome-codex-bar/selected_provider.json` | 当前选中的 Provider |
| `~/.local/share/gnome-shell/extensions/codex-bar@gnome/` | 扩展安装目录 |
| `~/.local/share/glib-2.0/schemas/` | GSettings schema |
| `~/.config/systemd/user/codex-bar-cli.service` | systemd user service（自启动） |

### macOS

| 路径 | 用途 |
|------|------|
| `~/Library/Application Support/gnome-codex-bar/config.toml` | CLI 配置 |
| `~/Library/Application Support/gnome-codex-bar/status.json` | 用量数据（CLI 写 → Swift App 读） |
| `~/Library/Application Support/gnome-codex-bar/selected_provider.json` | 当前选中的 Provider |
| `~/Library/LaunchAgents/com.codexbar.cli.plist` | launchd plist（自启动） |

> 注：配置和数据统一使用 `dirs::data_local_dir()` 下的 `gnome-codex-bar/` 目录。首次加载时自动从旧路径 `dirs::config_dir()/codex-bar/` 迁移 config.toml。

## 构建与开发

```bash
# 编译 CLI
cd cli && cargo build --release

# 安装（编译 + 部署扩展 + schema）
# Linux:
./install-linux.sh
# macOS:
# ./install-macos.sh

# 单次拉取测试
codex-bar-cli fetch

# 调试日志
RUST_LOG=debug codex-bar-cli fetch
```

Rust 工具链路径（如通过 rustup 安装但 cargo 不在 PATH）：

```
~/.rustup/toolchains/stable-aarch64-apple-darwin/bin/
```

## 开机自启动

CLI 支持 `autostart` 子命令管理守护进程的开机自启动：

```bash
# 启用开机自启动
codex-bar-cli autostart enable

# 禁用开机自启动
codex-bar-cli autostart disable

# 查看自启动状态
codex-bar-cli autostart status
```

运行时自动检测平台，选择对应机制：

| 平台 | 机制 | 服务文件 |
|------|------|---------|
| Linux (ZorinOS 等) | systemd user service | `~/.config/systemd/user/codex-bar-cli.service` |
| macOS | launchd plist | `~/Library/LaunchAgents/com.codexbar.cli.plist` |

### systemd (Linux)

- `enable`：写入 service 文件 → `daemon-reload` → `enable` → `start`
- `disable`：`stop` → `disable` → 删除 service 文件 → `daemon-reload`
- `Restart=on-failure` + `RestartSec=10`：崩溃后自动重启
- `After=network-online.target`：等待网络就绪

### launchd (macOS)

- `enable`：写入 plist → `launchctl load`
- `disable`：`launchctl unload` → 删除 plist
- `KeepAlive = true`：进程退出后自动重启
- 日志输出到 `~/Library/Application Support/gnome-codex-bar/logs/daemon.log` 和 `daemon.err`

## 单元测试

### 测试架构

CLI 后端使用 Rust 内置 `#[cfg(test)]` 模块进行单元测试，`cargo-llvm-cov` 生成覆盖率报告。

测试代码集中在 `cli/tests/unit/` 目录下，通过 `#[path]` 属性从源文件引用：

```
cli/
├── src/                            # 源代码
│   ├── atomic_write.rs             # #[cfg(test)] #[path = "../tests/unit/atomic_write.rs"] mod tests;
│   ├── browser_cookies.rs          # #[cfg(test)] #[path = "../tests/unit/browser_cookies.rs"] mod tests;
│   ├── config.rs                   # #[cfg(test)] #[path = "../tests/unit/config.rs"] mod tests;
│   ├── daemon.rs                   # #[cfg(test)] #[path = "../tests/unit/daemon.rs"] mod tests;
│   ├── output.rs                   # #[cfg(test)] #[path = "../tests/unit/output.rs"] mod tests;
│   └── providers/
│       ├── mod.rs                  # #[cfg(test)] #[path = "../../tests/unit/providers_mod.rs"] mod tests;
│       ├── deepseek.rs             # #[cfg(test)] #[path = "../../tests/unit/providers_deepseek.rs"] mod tests;
│       ├── opencodego.rs           # #[cfg(test)] #[path = "../../tests/unit/providers_opencodego.rs"] mod tests;
│       └── stepfun.rs              # #[cfg(test)] #[path = "../../tests/unit/providers_stepfun.rs"] mod tests;
└── tests/
    └── unit/                       # 所有测试文件集中存放（与 src/ 同级）
        ├── atomic_write.rs         # atomic_write.rs 的测试
        ├── autostart.rs            # autostart.rs 的测试
        ├── browser_cookies.rs      # browser_cookies.rs 的测试
        ├── config.rs               # config.rs 的测试
        ├── daemon.rs               # daemon.rs 的测试
        ├── output.rs               # output.rs 的测试
        ├── providers_mod.rs        # providers/mod.rs 的测试
        ├── providers_deepseek.rs   # providers/deepseek.rs 的测试
        ├── providers_opencodego.rs # providers/opencodego.rs 的测试
        └── providers_stepfun.rs    # providers/stepfun.rs 的测试
```

这种方式的优点：
- 测试代码集中在 `cli/tests/unit/` 目录（与 `src/` 同级），源文件保持简洁
- 通过 `#[path]` 引用，测试仍可访问私有类型（`FlexibleNumber` 等）
- 无需将私有类型改为 `pub`，不暴露内部 API
- 使用 `unit/` 子目录避免 Cargo 将测试文件当作集成测试编译

### 一键测试

```bash
# 运行测试 + 覆盖率报告
./test.sh

# 仅运行测试（跳过覆盖率）
./test.sh --no-cov

# 手动运行
cd cli && cargo test

# HTML 覆盖率报告
cd cli && cargo llvm-cov --html --open
```

### 测试覆盖范围

| 模块 | 测试数 | 覆盖内容 |
|------|--------|---------|
| `config.rs` | 12 | Config 默认值、TOML 序列化/反序列化、ProviderConfig 映射、skip_serializing_if |
| `output.rs` | 5 | StatusSnapshot/ProviderStatus JSON 序列化、selected_provider 读写逻辑 |
| `providers/mod.rs` | 9 | ProviderConfig/ProviderStatus/StatusSnapshot 序列化 + error 字段处理 + 敏感信息脱敏 |
| `providers/deepseek.rs` | 6 | Balance API 响应反序列化、余额计算逻辑、Provider id/name |
| `providers/stepfun.rs` | 38 | FlexibleNumber/Timestamp/IntOrString 反序列化、parse_timestamp、build_status、extract_set_cookie、token 缓存 |
| `providers/opencodego.rs` | 12 | Console API 响应反序列化（对象/数组两种 access、字符串/数字金额）、remaining_rate、workspace/cookie 规范化、错误映射 |
| `browser_cookies.rs` | 10 | PBKDF2 已知向量、v10/v11 AES-CBC 加解密往返、32 字节随机前缀、浏览器清单与选择 |
| `autostart.rs` | 12 | 平台检测、systemd/launchd 文件路径、service/plist 内容生成、网络依赖、绝对路径 |
| `atomic_write.rs` / `daemon.rs` | 3 | 原子写入、Provider 失败降级 |

**共 110 个单元测试，覆盖所有纯逻辑函数。** 网络依赖的 `Provider::fetch()` 暂未覆盖（需 HTTP mock）。

### 测试约定

- **纯逻辑优先**：优先测试数据转换、序列化、解析等不依赖网络的函数
- **集中测试目录**：测试代码集中在 `cli/tests/unit/` 目录（与 `src/` 同级），源文件通过 `#[path]` 属性引用，测试仍可访问私有类型
- **StepFun 灵活类型**：每次新增 StepFun API 解析逻辑，务必为对应的灵活类型反序列化器添加测试
- **status.json 契约**：ProviderStatus 和 StatusSnapshot 的序列化测试确保后端-前端数据格式一致

## 已知问题

暂无

## 注意事项

- **不要修改 status.json 的结构**：它是后端和前端之间的契约，任何字段变更需同时更新 Rust 和 JS 两端
- **GNOME Shell Extension 使用 GJS 运行时**：不是 Node.js，不支持 npm 包，使用 ES module import（`import X from 'gi://X'`）
- **StepFun API 可能随时变更**：保持灵活类型解析，解析失败时优雅降级（参考 `query_plan_status()` 的 `Option<String>` 返回策略）
- **扩展有双套实现**：`extension.js` 是当前活跃版本（直写 popup），`popupMenu.js` / `panelButton.js` 是备选方案（基于 GNOME 内置 PopupMenu/PanelMenu），改动时注意区分
- **配置文件中 password 是明文存储**：当前未加密，不要将 config.toml 提交到版本控制
- **OpenCode 接口可能再次变更**：网页端已从 HTML 抓取改为 Console JSON API，解析失败时优先怀疑接口/字段变化，
  用 `RUST_LOG=debug codex-bar-cli fetch` 观察请求与解析细节
- **浏览器 Cookie 读取会新增依赖**：`rusqlite`（bundled，内置编译 SQLite）、`aes`/`cbc`/`pbkdf2`/`sha1`，
  Linux 额外有 `secret-service`（zbus）。首次编译需要 C 编译器（cc/gcc）
- **不要打印 Cookie 内容**：调试时只记录来源（浏览器 + Profile），错误信息统一走 `sanitize_error_message`
