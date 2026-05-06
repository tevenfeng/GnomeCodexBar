# GnomeCodexBar Shell Extension 修复计划

## 问题 1：弹出窗口失去焦点不自动消隐

### 现象

点击顶栏控件弹出详情窗口后，失去焦点或点击其他程序、顶栏、Dock 栏等，弹出窗口不会自动关闭。只能再次点击顶栏按钮才能关闭。

### 根因

`extension.js` 中使用 `Main.layoutManager.addChrome()` 创建的 `St.BoxLayout` 作为弹出窗口，这是一个裸控件，没有任何焦点/点击外部关闭的机制。当前只有顶栏按钮的 `button-press-event` 事件切换显隐，没有监听外部点击。

### 修复方案

在 `enable()` 中注册 `global.stage` 的 `button-press-event` 全局监听：

1. 当 popup 可见时，获取点击事件坐标
2. 判断坐标是否在 popup 矩形区域外
3. 若在 popup 外部，则隐藏 popup
4. 若在 popup 内部或顶栏按钮上，让正常事件流处理

```js
// 伪代码
this._stageHandler = global.stage.connect('button-press-event', (stage, event) => {
    if (!this._popup.visible) return Clutter.EVENT_PROPAGATE;
    const [x, y] = event.get_coords();
    const [px, py] = this._popup.get_transformed_position();
    const [pw, ph] = this._popup.get_transformed_size();
    if (x < px || x > px + pw || y < py || y > py + ph) {
        this._popup.hide();
        return Clutter.EVENT_STOP;
    }
    return Clutter.EVENT_PROPAGATE;
});
```

`disable()` 中需断开连接：

```js
if (this._stageHandler) {
    global.stage.disconnect(this._stageHandler);
    this._stageHandler = null;
}
```

### 改动文件

| 文件 | 改动内容 |
|------|----------|
| `extension.js` | `enable()` 添加 stage 全局点击监听；`disable()` 断开监听 |

---

## 问题 2：StepFun 进度条 100% 但显示不满（仅约 2/3）

### 现象

StepFun 剩余量为 100%，但进度条绿色部分只占灰底轨道的约 2/3 长度，未填满。

### 根因

进度条填充部分（fill）使用了 `St.BoxLayout` 构建：

```js
// extension.js L128
tr.add_child(new St.BoxLayout({ style: `width:${f}px;`, style_class: `codex-bar-bar-fill ${cl}` }));
```

**`St.BoxLayout` 的布局算法不会严格尊重子元素的 CSS `width`**。`St.BoxLayout` 使用 `Clutter.BoxLayout` 分配策略，CSS `width` 仅设置 preferred width，实际分配宽度受父容器的 padding、spacing 及布局约束影响，可能被压缩。

相比之下，**`St.Widget` 使用 `Clutter.FixedLayout`**，子元素直接按照 CSS 指定的尺寸渲染，不受 BoxLayout 分配算法干扰。

### 修复方案

将 fill 的 `St.BoxLayout` 替换为 `St.Widget`，确保 CSS `width` 直接生效，不被 BoxLayout 布局算法压缩。同时在 CSS 中显式添加 `padding: 0; margin: 0;` 消除可能的间距。

**JS 改动：**

```js
// 修改前
tr.add_child(new St.BoxLayout({ style: `width:${f}px;`, style_class: `codex-bar-bar-fill ${cl}` }));

// 修改后
tr.add_child(new St.Widget({ style: `width:${f}px; height:6px;`, style_class: `codex-bar-bar-fill ${cl}` }));
```

> 注意：`St.Widget` 不会自动继承 CSS class 中的 `height: 6px`，需在 inline style 中显式指定。

**CSS 改动：**

```css
/* 修改前 */
.codex-bar-bar-bg {
    height: 6px;
    ...
}

.codex-bar-bar-fill {
    height: 6px;
    ...
}

/* 修改后 */
.codex-bar-bar-bg {
    height: 6px;
    padding: 0;
    ...
}

.codex-bar-bar-fill {
    height: 6px;
    padding: 0;
    margin: 0;
    ...
}
```

### 改动文件

| 文件 | 改动内容 |
|------|----------|
| `extension.js` | fill 构造从 `St.BoxLayout` 改为 `St.Widget`，inline style 补 `height:6px` |
| `popupMenu.js` | 同上：fill 构造从 `St.BoxLayout` 改为 `St.Widget`，inline style 补 `height:6px` |
| `stylesheet.css` | `.codex-bar-bar-bg` 添加 `padding: 0;`；`.codex-bar-bar-fill` 添加 `padding: 0; margin: 0;` |

---

## 改动总览

| 文件 | 问题 1 | 问题 2 |
|------|--------|--------|
| `extension.js` | ✅ 添加 stage click-outside 关闭逻辑 | ✅ fill 改用 `St.Widget` |
| `popupMenu.js` | — | ✅ fill 改用 `St.Widget` |
| `stylesheet.css` | — | ✅ track/fill 补 `padding: 0; margin: 0;` |

两个问题均只涉及 UI 层（事件逻辑 + 控件类型 + CSS），不涉及数据层或 Rust 后端改动。
