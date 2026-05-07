# Agents

本文件为 AI 编码助手提供项目上下文，帮助理解项目架构、约定和注意事项。

## 项目概述

GnomeCodexBar 是一个 **Rust CLI 后端 + GNOME Shell 扩展前端** 的双进程架构项目，用于在 GNOME 顶栏实时监控 DeepSeek 和 StepFun 的编程套餐用量。

- **后端（Rust CLI）**：定时拉取 API 数据，写入 `status.json`
- **前端（GNOME Shell Extension）**：通过 `Gio.FileMonitor` 监听 `status.json` 变化，渲染顶栏控件和弹出详情窗口

两者通过文件系统解耦，无直接通信。

## 架构与数据流

```
Rust CLI daemon
  ├── DeepSeek: GET /user/balance (API Key 认证)
  └── StepFun:  3-step login → QueryStepPlanRateLimit + GetStepPlanStatus (Cookie 认证)
        │
        ▼
  status.json  (~/.local/share/gnome-codex-bar/status.json)
        │
        ▼  (Gio.FileMonitor)
  GNOME Shell Extension
  ├── 顶栏按钮 (panelButton.js / extension.js)
  └── 弹出详情窗口 (extension.js / popupMenu.js)
```

## 关键文件

### Rust 后端 (`cli/`)

| 文件 | 职责 |
|------|------|
| `src/main.rs` | CLI 入口，clap 子命令定义 |
| `src/daemon.rs` | 守护进程循环 + `fetch_all()` 并行拉取所有 Provider |
| `src/autostart.rs` | 开机自启动管理（Linux systemd / macOS launchd） |
| `src/config.rs` | TOML 配置加载，`~/.config/codex-bar/config.toml` |
| `src/output.rs` | 写入 `status.json` + `selected_provider.json` |
| `src/providers/mod.rs` | `Provider` trait + `ProviderStatus` / `StatusSnapshot` 类型定义 |
| `src/providers/deepseek.rs` | DeepSeek Provider：API Key 认证 + 余额查询 |
| `src/providers/stepfun.rs` | StepFun Provider：3-step 登录 + 用量查询 + 套餐查询 |

### GNOME Shell 扩展 (`gnome-shell-extension/`)

| 文件 | 职责 |
|------|------|
| `extension.js` | 主扩展类：顶栏按钮 + popup 构建 + 详情渲染（当前活跃版本） |
| `popupMenu.js` | 基于 `PopupMenu.PopupMenu` 的备选实现（未启用） |
| `panelButton.js` | 基于 `PanelMenu.Button` 的备选顶栏按钮（未启用） |
| `statusReader.js` | 读取 `status.json` + `selected_provider.json` + 文件监听 |
| `stylesheet.css` | 所有样式定义 |

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

前端根据 `remaining_percent` 决定进度条颜色：
- ≥50%：绿色（`high`）
- 20-50%：黄色（`medium`）
- <20%：红色（`low`）

## 文件路径

| 路径 | 用途 |
|------|------|
| `~/.config/codex-bar/config.toml` | CLI 配置 |
| `~/.local/share/gnome-codex-bar/status.json` | 用量数据（CLI 写 → 扩展读） |
| `~/.local/share/gnome-codex-bar/selected_provider.json` | 当前选中的 Provider |
| `~/.local/share/gnome-shell/extensions/codex-bar@gnome/` | 扩展安装目录 |
| `~/.local/share/glib-2.0/schemas/` | GSettings schema |
| `~/.config/systemd/user/codex-bar-cli.service` | systemd user service（Linux 自启动） |
| `~/Library/LaunchAgents/com.codexbar.cli.plist` | launchd plist（macOS 自启动） |

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
- 日志输出到 `~/.local/share/gnome-codex-bar/logs/daemon.log` 和 `daemon.err`

## 单元测试

### 测试架构

CLI 后端使用 Rust 内置 `#[cfg(test)]` 模块进行单元测试，`cargo-llvm-cov` 生成覆盖率报告。

测试代码集中在 `cli/tests/unit/` 目录下，通过 `#[path]` 属性从源文件引用：

```
cli/
├── src/                            # 源代码
│   ├── config.rs                   # #[cfg(test)] #[path = "../tests/unit/config.rs"] mod tests;
│   ├── output.rs                   # #[cfg(test)] #[path = "../tests/unit/output.rs"] mod tests;
│   └── providers/
│       ├── mod.rs                  # #[cfg(test)] #[path = "../../tests/unit/providers_mod.rs"] mod tests;
│       ├── deepseek.rs             # #[cfg(test)] #[path = "../../tests/unit/providers_deepseek.rs"] mod tests;
│       └── stepfun.rs              # #[cfg(test)] #[path = "../../tests/unit/providers_stepfun.rs"] mod tests;
└── tests/
    └── unit/                       # 所有测试文件集中存放（与 src/ 同级）
        ├── config.rs               # config.rs 的测试
        ├── output.rs               # output.rs 的测试
        ├── providers_mod.rs        # providers/mod.rs 的测试
        ├── providers_deepseek.rs   # providers/deepseek.rs 的测试
        ├── providers_stepfun.rs    # providers/stepfun.rs 的测试
        └── autostart.rs            # autostart.rs 的测试
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
| `config.rs` | 6 | Config 默认值、TOML 序列化/反序列化、ProviderConfig 映射、skip_serializing_if |
| `output.rs` | 4 | StatusSnapshot/ProviderStatus JSON 序列化、selected_provider 读写逻辑 |
| `providers/mod.rs` | 6 | ProviderConfig/ProviderStatus/StatusSnapshot 序列化 + error 字段处理 |
| `providers/deepseek.rs` | 6 | Balance API 响应反序列化、余额计算逻辑、Provider id/name |
| `providers/stepfun.rs` | 25 | FlexibleNumber/Timestamp/IntOrString 反序列化、parse_timestamp、build_status、extract_set_cookie、各响应类型反序列化 |
| `autostart.rs` | 8 | 平台检测、systemd/launchd 文件路径、service/plist 内容生成、网络依赖、绝对路径 |

**共 55 个单元测试，覆盖所有纯逻辑函数。** 网络依赖的 `Provider::fetch()` 暂未覆盖（需 HTTP mock）。

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
