# GNOME Shell Extension 设置页同步 `config.toml` 修复计划

## 背景

当前项目的配置与运行数据已经统一放在平台数据目录下：

- Linux: `~/.local/share/gnome-codex-bar/`
- macOS: `~/Library/Application Support/gnome-codex-bar/`

CLI daemon 读取的主配置文件为：

```text
~/.local/share/gnome-codex-bar/config.toml
```

其中与本问题相关的字段为：

```toml
[general]
refresh_interval_secs = 300

[providers.deepseek]
enabled = true

[providers.stepfun]
enabled = true
```

macOS 菜单栏应用的设置页已经改为读写同一个 `config.toml`。GNOME Shell Extension 目前仍使用 GSettings / dconf 存储设置，因此 Linux 侧设置页与 CLI daemon 配置不同步。

---

## 当前问题结论

### 相关文件

- `gnome-shell-extension/prefs.js`
- `gnome-shell-extension/schemas/org.gnome.shell.extensions.codex-bar.gschema.xml`
- `gnome-shell-extension/extension.js`
- `gnome-shell-extension/statusReader.js`
- `cli/src/config.rs`
- `cli/src/daemon.rs`

### 当前行为

1. GNOME 设置页 `prefs.js` 只读写 GSettings。

   当前 schema 中只有两个 key：

   ```xml
   <key name="refresh-interval" type="i">
   <key name="cli-path" type="s">
   ```

2. `extension.js` 读取：

   ```js
   this.getSettings().get_int('refresh-interval')
   ```

   该值只控制 GNOME 扩展自身的 fallback poll timer。

3. CLI daemon 读取的是 `config.toml`：

   ```toml
   [general]
   refresh_interval_secs = 300
   ```

4. 因此，在 GNOME 设置页修改刷新间隔，并不会改变 CLI daemon 的实际 API 拉取间隔。

5. GNOME 设置页目前没有 Provider enable/disable 开关。

6. `statusReader.js` 只处理：

   - `status.json`
   - `selected_provider.json`

   不读取也不写入 `config.toml`。

---

## 修复目标

让 Linux GNOME Shell Extension 和 macOS 菜单栏应用保持一致：

1. `config.toml` 成为刷新间隔和 Provider 开关的唯一真源。
2. GNOME 设置页修改刷新间隔时，写入：

   ```toml
   [general]
   refresh_interval_secs = <seconds>
   ```

3. GNOME 设置页支持 Provider enable/disable 开关，写入：

   ```toml
   [providers.deepseek]
   enabled = true/false

   [providers.stepfun]
   enabled = true/false
   ```

4. GNOME extension 运行时从 `config.toml` 读取配置。
5. 当 `config.toml` 变化时，extension 能更新自身 UI / fallback poll timer。
6. CLI daemon 已支持每轮重新加载 `config.toml`，因此设置页写入后无需重启 daemon。

---

## 推荐方案

### 方案概述

新增一个 GNOME 侧配置管理模块：

```text
gnome-shell-extension/configManager.js
```

该模块负责：

- 定位 `config.toml`
- 读取刷新间隔
- 写入刷新间隔
- 读取 Provider enabled 状态
- 写入 Provider enabled 状态
- 监听 `config.toml` 变化

`prefs.js`、`extension.js`、`statusReader.js` 根据职责调用该模块。

---

## 详细修复计划

### 1. 新增 `configManager.js`

新增文件：

```text
gnome-shell-extension/configManager.js
```

#### 1.1 配置路径

使用 GJS/GLib 获取 Linux 用户数据目录：

```js
GLib.build_filenamev([
    GLib.get_user_data_dir(),
    'gnome-codex-bar',
    'config.toml',
])
```

Linux 下应解析为：

```text
~/.local/share/gnome-codex-bar/config.toml
```

#### 1.2 导出 API

建议导出：

```js
export class ConfigManager {
    constructor();

    getRefreshInterval();
    setRefreshInterval(seconds);

    isProviderEnabled(providerId);
    setProviderEnabled(providerId, enabled);

    getProviderEnabledMap();

    monitorConfig(onChanged);
    destroy();
}
```

#### 1.3 默认配置模板

如果 `config.toml` 不存在，则创建最小默认配置：

```toml
[providers.deepseek]
enabled = true

[providers.stepfun]
enabled = true

[general]
refresh_interval_secs = 300
selected_provider = "deepseek"
```

注意：如果文件已存在，绝不能重建整个文件，避免丢失：

- `api_key`
- `username`
- `password`
- `cached_token`
- `cached_ingress_cookie`
- `budget_monthly`
- 其他未来新增字段

#### 1.4 TOML 读写策略

GJS 不是 Node.js，不能使用 npm TOML 包。采用轻量行级读写策略：

- 只识别简单 section：

  ```toml
  [general]
  [providers.deepseek]
  [providers.stepfun]
  ```

- 只修改目标 key 行：

  ```toml
  refresh_interval_secs = 60
  enabled = false
  ```

- 如果 section 存在但 key 不存在，则插入 key。
- 如果 section 不存在，则追加 section 和 key。
- 写入时保留其他行，避免破坏密钥字段。

这与 macOS 当前 `StatusReader.swift` 的实现思路一致。

#### 1.5 原子写入

写入时建议：

1. 确保父目录存在。
2. 写入临时文件。
3. rename 到 `config.toml`。

这样可以降低写入中断导致配置损坏的风险。

---

### 2. 修改 `prefs.js`

当前设置页只处理 GSettings。计划改为：

#### 2.1 Refresh Interval 从 `config.toml` 读取

设置页打开时：

```js
const interval = configManager.getRefreshInterval();
```

显示到 spinner / dropdown。

#### 2.2 修改 Refresh Interval 时写入 `config.toml`

用户修改后调用：

```js
configManager.setRefreshInterval(value);
```

写入：

```toml
[general]
refresh_interval_secs = <value>
```

#### 2.3 新增 Provider 开关

设置页新增两个 switch：

- DeepSeek
- StepFun

初始化：

```js
deepseekSwitch.active = configManager.isProviderEnabled('deepseek');
stepfunSwitch.active = configManager.isProviderEnabled('stepfun');
```

修改时：

```js
configManager.setProviderEnabled('deepseek', deepseekSwitch.active);
configManager.setProviderEnabled('stepfun', stepfunSwitch.active);
```

#### 2.4 CLI Path 继续保留 GSettings

`cli-path` 更像扩展自身配置，不属于 CLI daemon 主配置。

因此：

- `cli-path` 继续读写 GSettings。
- `refresh-interval` GSettings key 暂时保留，但不再作为主数据源。

---

### 3. 修改 `extension.js`

#### 3.1 fallback poll timer 改读 `config.toml`

当前逻辑：

```js
this._settings.get_int('refresh-interval')
```

计划改为：

```js
this._configManager.getRefreshInterval()
```

#### 3.2 监听 `config.toml` 变化

使用 `Gio.FileMonitor` 监听：

```text
~/.local/share/gnome-codex-bar/config.toml
```

当文件变化时：

1. 重新读取刷新间隔。
2. 如果 interval 变化：
   - remove 旧 `GLib.timeout_add_seconds`
   - 创建新 timer
3. 重新读取 Provider enabled 状态。
4. 重绘顶部 summary 和 popup。

#### 3.3 Provider 显示过滤

popup 中当前会显示 `status.json` 返回的所有 Provider。

计划改为：

```js
const providers = snapshot.providers.filter(provider =>
    this._configManager.isProviderEnabled(provider.provider_id)
);
```

行为建议：

- disabled Provider 不在 popup 中显示。
- 如果当前 selected provider 被禁用：
  - 顶栏显示第一个 enabled provider。
  - 如果所有 Provider 都禁用，显示 `--`。

这与 macOS 当前行为保持一致。

---

### 4. 修改 `statusReader.js`（可选）

有两种实现方式：

#### 方案 A：`extension.js` 直接使用 `ConfigManager`

优点：

- 改动集中在 `extension.js` 和 `prefs.js`
- `statusReader.js` 继续只负责 status / selected provider

缺点：

- extension 内部需要同时持有 statusReader 和 configManager

#### 方案 B：把 config 能力合并进 `statusReader.js`

优点：

- 类似 macOS `StatusReader.swift`
- status / selected provider / config 都由一个 reader 管理

缺点：

- `statusReader.js` 职责变重

推荐：**方案 A**。新增独立 `configManager.js`，避免让 `statusReader.js` 过度膨胀。

---

### 5. GSettings schema 策略

当前 schema 中有：

```xml
<key name="refresh-interval" type="i">
<key name="cli-path" type="s">
```

建议：

1. 暂时保留 `refresh-interval`。
2. 不再把它作为主数据源。
3. 可以作为 fallback：
   - 如果 `config.toml` 不存在且创建失败，则使用 GSettings 的 `refresh-interval`。
4. 继续保留 `cli-path`。

这样可以减少 schema 迁移风险，不影响已有用户 dconf 数据。

---

### 6. 文档更新

需要更新：

- `README.md`
- `AGENTS.md`

说明：

1. Linux GNOME 设置页现在写入：

   ```text
   ~/.local/share/gnome-codex-bar/config.toml
   ```

2. macOS 设置页写入：

   ```text
   ~/Library/Application Support/gnome-codex-bar/config.toml
   ```

3. 两个平台的设置页都控制 CLI daemon 的实际刷新间隔：

   ```toml
   [general]
   refresh_interval_secs = 300
   ```

4. Provider 开关写入：

   ```toml
   [providers.<id>]
   enabled = true/false
   ```

---

## 验证计划

### 1. Schema 验证

```bash
glib-compile-schemas gnome-shell-extension/schemas
```

预期：无错误。

### 2. 安装验证

```bash
./install-linux.sh
```

预期：扩展安装成功，schema 编译成功。

### 3. 设置页验证

打开 GNOME 设置页：

```bash
gnome-extensions prefs codex-bar@gnome
```

修改刷新间隔为 60s。

检查：

```bash
cat ~/.local/share/gnome-codex-bar/config.toml
```

应看到：

```toml
[general]
refresh_interval_secs = 60
```

关闭 StepFun。

应看到：

```toml
[providers.stepfun]
enabled = false
```

### 4. daemon 生效验证

启动 daemon：

```bash
RUST_LOG=info codex-bar-cli daemon
```

修改设置页 interval。

预期：无需重启 daemon，后续日志出现新的 interval。

### 5. popup 显示验证

1. 两个 Provider 都 enabled：popup 显示 DeepSeek + StepFun。
2. 禁用 StepFun：popup 只显示 DeepSeek。
3. 禁用 DeepSeek：popup 只显示 StepFun。
4. 两个都禁用：顶栏显示 `--` 或空状态，popup 显示无 enabled provider 的提示。

### 6. 配置保护验证

准备带密钥的配置：

```toml
[providers.deepseek]
enabled = true
api_key = "sk-xxx"

[providers.stepfun]
enabled = true
username = "user@example.com"
password = "secret"
cached_token = "token"

[general]
refresh_interval_secs = 300
selected_provider = "deepseek"
```

通过 GNOME 设置页修改 interval / provider enabled 后，确认以下字段仍保留：

- `api_key`
- `username`
- `password`
- `cached_token`
- `selected_provider`

---

## 风险与规避

### 风险 1：轻量 TOML 写入逻辑破坏配置

规避：

- 只做 section/key 级替换。
- 不重新序列化整个 TOML。
- 增加手动验证密钥字段不丢失。

### 风险 2：GSettings 与 `config.toml` 双源冲突

规避：

- `config.toml` 作为唯一真源。
- `refresh-interval` GSettings 仅保留兼容，不再主动写入或读取为主值。

### 风险 3：`config.toml` 不存在

规避：

- `ConfigManager` 创建默认最小配置。
- 确保目录存在后再写入。

### 风险 4：设置页与 daemon 同时写配置

规避：

- daemon 通常只读配置。
- StepFun token cache 可能由 CLI 写入配置时，需要保证扩展写入逻辑基于最新文件内容做局部修改。
- 每次写入前重新读取当前文件，不使用长期缓存覆盖。

---

## 建议实施顺序

1. 新增 `gnome-shell-extension/configManager.js`。
2. 修改 `prefs.js`：刷新间隔读写 `config.toml`，新增 Provider toggles。
3. 修改 `extension.js`：fallback poll timer 和 popup provider 过滤读取 `ConfigManager`。
4. 增加 config 文件监听，配置变化时刷新 UI / timer。
5. 更新 `README.md` 和 `AGENTS.md`。
6. 执行 schema / install / 手动验证。

---

## 预期结果

修复后：

- Linux GNOME 设置页与 macOS 设置页行为一致。
- 修改刷新间隔会同步到 CLI daemon 实际 API 拉取间隔。
- Provider enable/disable 状态由 `config.toml` 统一管理。
- `status.json` 仍作为后端到前端的数据契约，不需要变更结构。
- 已有敏感字段不会因设置页修改而丢失。
