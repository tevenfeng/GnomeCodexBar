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

## 构建与开发

```bash
# 编译 CLI
cd cli && cargo build --release

# 安装（编译 + 部署扩展 + schema）
./install.sh

# 单次拉取测试
codex-bar-cli fetch

# 调试日志
RUST_LOG=debug codex-bar-cli fetch
```

Rust 工具链路径（如通过 rustup 安装但 cargo 不在 PATH）：

```
~/.rustup/toolchains/stable-aarch64-apple-darwin/bin/
```

## 已知问题

详见 [FIX_PLAN.md](./FIX_PLAN.md)：

1. **弹出窗口不自动消隐**：popup 使用 `addChrome` 裸控件，缺少 click-outside 关闭逻辑
2. **StepFun 进度条不满**：fill 使用 `St.BoxLayout` 导致 CSS width 被布局压缩，需改用 `St.Widget`

## 注意事项

- **不要修改 status.json 的结构**：它是后端和前端之间的契约，任何字段变更需同时更新 Rust 和 JS 两端
- **GNOME Shell Extension 使用 GJS 运行时**：不是 Node.js，不支持 npm 包，使用 ES module import（`import X from 'gi://X'`）
- **StepFun API 可能随时变更**：保持灵活类型解析，解析失败时优雅降级（参考 `query_plan_status()` 的 `Option<String>` 返回策略）
- **扩展有双套实现**：`extension.js` 是当前活跃版本（直写 popup），`popupMenu.js` / `panelButton.js` 是备选方案（基于 GNOME 内置 PopupMenu/PanelMenu），改动时注意区分
- **配置文件中 password 是明文存储**：当前未加密，不要将 config.toml 提交到版本控制
