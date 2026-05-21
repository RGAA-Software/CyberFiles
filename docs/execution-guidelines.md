# CyberFiles 执行规范

本文档为开发/ Agent 执行时的硬性约定。与 [`files-rust-port-plan.md`](files-rust-port-plan.md) 中的技术栈一致。

---

## UI 组件原则（必须遵守）

**所有的组件都应该使用 gpui-component 的组件；如果 gpui-component 没有对应能力，则提示我（仓库维护者），不要擅自用裸 GPUI 拼一套等价控件而不说明。**

### 适用范围

| 类别 | 要求 |
|------|------|
| 交互控件 | 按钮、输入框、开关、下拉、对话框、菜单、标签页、侧边栏、表格/列表、面包屑等 → **必须** `gpui_component` |
| 布局 | 优先 `gpui_component::{v_flex, h_flex, resizable, …}`；纯容器可用 GPUI `div` + theme 样式 |
| 主题/尺寸 | 使用 `ActiveTheme`、`Sizable`、`.small()` 等组件库约定，勿自建一套样式 |
| 平台/Shell | `platform-windows`、COM、拖拽数据等 **不** 算 UI 组件，可保持独立 |
| **文件列表（硬性）** | **永远**使用 `gpui_component::v_virtual_list` + 现有行/列/网格模板；**禁止**改为 `Table`/`DataTable` 或其它非虚拟化列表，以保证海量目录性能 |

### 无对应组件时的流程

1. 查阅 [`gpui-component-analysis.md`](gpui-component-analysis.md) 与 `../gpui-component/crates/story` 画廊。
2. 确认库中确实无合适模块后，在 PR/任务说明中 **明确列出** 缺失的组件名与用途，并 @ 维护者。
3. 临时方案仅允许：GPUI 原语（`div`、`img`）、或与官方示例一致的 `VirtualList` 行模板组合；须在排查表中登记为「已报备」。

### 参考

- 组件清单：[`gpui-component-analysis.md` §4](gpui-component-analysis.md#4-组件全览按分类)
- 初始化：`gpui_component::init(cx)`，窗口根节点 `Root::new`

---

## gpui-component 使用排查（2026-05-21）

排查范围：`crates/ui`（项目唯一 UI crate）。非 UI crate（`app`、`core`、`fs`、`commands`、`platform-windows`）无界面代码。

### 汇总

| 状态 | 说明 |
|------|------|
| ✅ 已符合 | 壳层、设置、Omnibar、Home、InfoPane、状态栏、通知、文件栏工具栏/右键菜单、**虚拟列表文件列表** |
| 🔒 不可变 | `file_browser` 内 `v_virtual_list` 及行/列/网格/详细信息行模板（性能约束，见上文） |
| ⚪ 已报备（库无专用组件） | 见下表；维持现状 |

---

### 按文件

#### `crates/ui/src/main_page/mod.rs`

| 区域 | 当前 | 建议 | 状态 |
|------|------|------|------|
| 导航工具栏 | `Button`、`Input`、`Breadcrumb` | — | ✅ |
| 侧栏 | `Sidebar` / `SidebarMenu` / `SidebarMenuItem` | — | ✅ |
| 标签栏 | `TabBar` / `Tab` | — | ✅ |
| 分栏 | `h_resizable` / `resizable_panel` | — | ✅ |
| 状态栏 | `Label` | ✅ |
| 主内容区容器 | `div` 布局 | 容器可保留 | ✅ |

#### `crates/ui/src/file_browser.rs`

| 区域 | 当前 | 状态 |
|------|------|------|
| 工具栏 / 重命名 / 删除对话框 / 右键菜单 | gpui-component | ✅ |
| **文件列表** | `v_virtual_list` + 行/列/网格/详细信息模板 | 🔒 **禁止改动列表实现** |
| 通知 | `Notification::success` / `::error` | ✅ |
| 拖拽预览 | 自定义 `DragPathPreview` | ⚪ 库无 DragPreview |

#### `crates/ui/src/home/page.rs`

| 区域 | 当前 | 状态 |
|------|------|------|
| 各 widget 区块 | `GroupBox` + `Label` 标题 + `Alert` 空状态 / `Button` 列表 | ✅ |

#### `crates/ui/src/info_pane.rs`

| 区域 | 当前 | 状态 |
|------|------|------|
| 标签 | `TabBar` | ✅ |
| 详情字段 | `DescriptionList` + `Label` | ✅ |
| 空/提示 | `Alert` | ✅ |
| 图片预览 | `gpui::img` | ⚪ 库无 `Image` 模块 |
| 文本预览 | `Label` | ✅ |

#### `crates/ui/src/settings_view.rs`

| 区域 | 当前 | 建议 | 状态 |
|------|------|------|------|
| 设置页 | `Settings` / `SettingItem` / `GroupBoxVariant` | — | ✅ |

#### `crates/ui/src/shell/*`

| 文件 | 当前 | 建议 | 状态 |
|------|------|------|------|
| `window.rs` | `Root`、`TitleBar` | — | ✅ |
| `app_shell.rs` | `Root`、`v_flex` | — | ✅ |
| `title_bar.rs` | `TitleBar`、`Button`、`Badge` | — | ✅ |
| `shell_panes.rs` | `h_resizable` | — | ✅ |
| `app_menus.rs` | `AppMenuBar` + **`gpui::Menu` / `MenuItem`** | 原生菜单栏与 GPUI 平台 API 绑定；`AppMenuBar` 为库组件，条目结构来自 GPUI | ⚪ **需你确认** |
| `pane_shell.rs` | `div` 容器 | 容器可保留 | ✅ |

#### 其他

| 文件 | 说明 | 状态 |
|------|------|------|
| `omnibar/mod.rs` | 仅逻辑，UI 在 `main_page` | ✅ |
| `app_state.rs` | 无 UI | — |
| `i18n.rs` | `gpui_component::set_locale` | ✅ |

---

### gpui-component 暂无、需维护者确认的场景

以下情况实现时 **请提示我**，勿默认长期用裸 `div` 代替而不记录：

| # | 场景 | 当前做法 | 说明 |
|---|------|----------|------|
| 1 | **图片预览** | `gpui::img` | 分析文档：无 `pub mod image`，Gallery 亦用 GPUI `img` |
| 2 | **拖拽幽灵预览** | 自定义 `DragPathPreview` | 无 DragPreview / DragOverlay 组件 |
| 3 | **系统菜单栏条目** | `gpui::Menu` + `MenuItem::action` | 与 `cx.set_menus` / 平台菜单联动；内窗菜单用 `AppMenuBar` |
| 4 | **状态栏** | 自绘 `h_flex` | 无 StatusBar 模块；可用 `Label` 替代纯 `div` |
| 5 | **文件行模板** | `v_virtual_list` + 行内 `div` | **产品决定：永不改为 Table**；仅允许在虚拟列表框架内改样式/列内容 |

---

### 迁移记录

- **2026-05-21**：Home / InfoPane / 状态栏 / 通知已迁移；文件列表锁定为 `v_virtual_list`。

---

### 排查方法（供下次复查）

```bash
# UI 中仍直接依赖 gpui 的文件
rg "^use gpui::" crates/ui/src

# 未引用 gpui_component 的 UI 源文件（应仅有 app_state / navigation 等少量）
rg -L "gpui_component" crates/ui/src --glob "*.rs"

# 裸 div 文本（候选迁移 Label）
rg "div\(\)\s*\.(text|child)" crates/ui/src
```

---

*最后更新：2026-05-21。Omnibar 面包屑：`PathBreadcrumbBar`；回归 [`omnibar-breadcrumb-bugs.md`](omnibar-breadcrumb-bugs.md)；与 Files 差距 [`omnibar-breadcrumb-files-gap.md`](omnibar-breadcrumb-files-gap.md)。文件列表锁定 `v_virtual_list`。*
