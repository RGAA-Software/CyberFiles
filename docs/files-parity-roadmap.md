# Files 一比一复刻路线图

对照项目：**[`../Files`](../Files)**（WinUI 3 社区版，C# / MVVM）。本文档按 **Files 真实结构** 追踪 CyberFiles 的实现状态，作为长期复刻清单。

**复刻原则：** Rust + GPUI + gpui-component 一比一复刻 **Files**（非 Explorer）。右键菜单等行为见 [`files-context-menu-parity.md`](files-context-menu-parity.md)。

**明确不复刻（产品决策，非 backlog）：** 见下文 [不复刻清单](#不复刻清单)。

**差距专题：**

| 区域 | 文档 |
|------|------|
| Omnibar / 面包屑 | [`omnibar-breadcrumb-files-gap.md`](omnibar-breadcrumb-files-gap.md) |
| 侧栏 | [`sidebar-files-gap.md`](sidebar-files-gap.md) |
| Home 小部件 | [`home-files-parity.md`](home-files-parity.md) |
| 右键菜单 | [`files-context-menu-parity.md`](files-context-menu-parity.md) |
| 总计划 / crate 边界 | [`files-rust-port-plan.md`](files-rust-port-plan.md) |

---

## Files 侧「真源」文件（查行为时优先打开）

| 能力 | `../Files` 路径 |
|------|-----------------|
| 主布局 | `src/Files.App/Views/MainPage.xaml` |
| 地址栏 | `UserControls/NavigationToolbar.xaml(.cs)` |
| 列表 / 右键 | `Views/Layouts/BaseLayoutPage.cs` |
| 菜单工厂 | `Data/Factories/ContentPageContextFlyoutFactory.cs` |
| Shell 解析 | `Utils/Shell/ContextMenu.cs` |
| 常量（行高、拖悬 1300ms 等） | `Constants.cs` |
| 设置模型 | `Data/Contracts/I*SettingsService.cs` |

CyberFiles 无根目录 README；产品与 parity 说明以 `docs/` 为准。

---

## 架构对照

```text
Files (WinUI)                          CyberFiles (GPUI)
─────────────────────────────────────────────────────────────
MainWindow                             AppShell + open_main_window
└─ MainPage                            └─ MainPage (main_page/mod.rs)
   ├─ TabBar                               ├─ TabBar（标题栏内嵌）           🟡
   ├─ NavigationToolbar (48px)             ├─ navigation-toolbar            🟡
   │     Omnibar: Path / Search / Command   │     面包屑 + 路径编辑 + 右侧过滤搜索 ✅（不复刻 Search/Command 模式切换）
   ├─ SidebarView                          ├─ sidebar/                      🟡
   └─ RootGrid                             └─ content + InfoPane + status
      ├─ Inner Toolbar                     ├─ file_browser content-toolbar  🟡
      ├─ ShellPanesPage                     ├─ ShellPanes（双栏）            ✅
      ├─ InfoPane                           ├─ InfoPane（右栏）              🟡
      ├─ StatusBar + StatusCenter           ├─ status_bar + 通知（传输进度）  🟡
      └─ ShelfPane                          └─ （未实现）                    ⬜
```

**Workspace crates（当前 7 个）：** `app`、`core`、`fs`、`commands`、`ui`、`platform-windows`、`assets`。  
**规划未启用：** `previews`、`search`、`tags`、`archive`、`git`（见 `files-rust-port-plan.md`）。

---

## 粗算完成度（2026-05，相对 Files）

| 模块 | 约 % | 说明 |
|------|------|------|
| MainPage 骨架 / 双栏 / 基础导航 | 75–85% | 缺 Shelf、会话恢复；Omnibar 模式切换已明确不复刻 |
| FileBrowser 三视图 + 快捷键 | 65–75% | 缺分组、悬停选中、完整 StatusCenter |
| Omnibar 面包屑 | 80–85% | P1 基本完成，见 gap 文档 P2 |
| 侧栏结构 | ~75% | Shell/Explorer 深度集成弱 |
| 右键菜单 | 55–65% | Flyout + Shell 合并；Open with 子菜单已首轮 |
| Home | ~70% | H0–H6 ✅（拖放 widget 重排、Expander 动画明确不做） |
| 设置 | 25–30% | 4 页 vs Files 8+ |
| 标签 / Git / 压缩 / FTP | &lt;15% | 大多 ⬜ |

---

## 状态图例

| 标记 | 含义 |
|------|------|
| ✅ | 已有可用实现 |
| 🟡 | 部分实现 / 占位 |
| ⬜ | 未开始 |
| ⛔ | 明确不复刻（见下表） |

---

## 不复刻清单

与 Files 行为对照后**刻意不做**的能力；文档保留说明以免后续重复排期。

| Files 能力 | Files 参考 | CyberFiles 替代 | 原因 |
|------------|------------|-----------------|------|
| **Omnibar `OmnibarMode`：Search / Command 模式** | `NavigationToolbar`、`OmnibarMode` 枚举；地址栏内切换路径 / 当前夹搜索 / 命令面板 | 导航栏 **Path 面包屑** + 右侧独立 **`search_input`**（`Ctrl+F` / `Ctrl+L`）过滤当前列表 | 已有独立搜索框，无需再复刻 Files 三态 Omnibar；命令面板另项（⬜）不在 Omnibar 内做 |

详见 [`omnibar-breadcrumb-files-gap.md`](omnibar-breadcrumb-files-gap.md)「不复刻」一节。

---

## A. 应用外壳（MainPage）

| Files 能力 | CyberFiles | 状态 |
|------------|------------|------|
| 应用级 TabBar | `main_page` | 🟡 |
| 新建/关闭/切换标签 | Tab +/- + tooltip i18n | 🟡 |
| 标签标题随路径更新 | `tab_title` | 🟡 |
| 会话恢复 / 最近关闭标签 | `session_tabs` + `session_closed_tabs`（Ctrl+Shift+T、View 菜单列表） | 🟡 |
| NavigationToolbar | `navigation-toolbar` | 🟡 |
| InnerNavigationToolbar | `file_browser` `content-toolbar` | 🟡 |
| 地址栏 / Omnibar | 面包屑 + 路径编辑 + 右侧 `search_input` 过滤 | ✅ |
| Omnibar Search/Command 模式切换 | — | ⛔ 不复刻 |
| Home widget 拖放重排 | 标题右键上移/下移 + `home_widget_order` | ⛔ 不做 |
| Home Expander 折叠动画 | 即时展开/收起 | ⛔ 不做 |
| 侧栏折叠 | `h_resizable` | ✅ |
| Sidebar 分区 | 8 区 + 设置页开关 | 🟡 |
| 固定文件夹 | 侧栏 + Home + Pin + `settings.json` | ✅ |
| 单栏 Shell | `PaneShell` | 🟡 |
| 双栏 ShellPanesPage | `ShellPanes` | ✅ |
| InfoPane | Details + 文本/图片预览 | 🟡 |
| ShelfPane | 暂存条：计数、首项预览、粘贴、清空 | 🟡 |
| StatusBar | `status_bar` | 🟡 |
| 设置全页 | General / Sidebar / Home / About | 🟡 |

---

## B. 导航与 Shell 内容

| Files 能力 | CyberFiles | 状态 |
|------------|------------|------|
| Home 虚拟路径 | `NavigationTarget::Home` | 🟡 |
| 路径导航 | `FileBrowser` | ✅ |
| 每标签独立历史 | 每标签 `FileBrowser` | ✅ |
| 后退/前进/上级/刷新 | 工具栏 + 快捷键 + tooltip | ✅ |
| 列 / 详情 / 网格 | `ViewMode` + Ctrl+1/2/3 | 🟡 |
| 分栏布局 | `ShellPanes` | ✅ |
| 搜索当前文件夹 | 导航栏过滤 + Ctrl+F | 🟡 |
| 目录变更自动刷新 | `notify` 防抖 | 🟡 |
| 排序/分组 | 排序菜单；**无分组** | 🟡 |
| 多选 / 空白取消选中 | ✅ | ✅ |
| 列表空白区右键 | `build_background_context_menu` | 🟡 |
| 键盘快捷键 | `commands` | ✅ |

---

## C. Home 页

详见 [`home-files-parity.md`](home-files-parity.md)。

| Widget | 状态 |
|--------|------|
| H0–H4 子系统 / 卡片 / 设置开关 | ✅ |
| Quick Access / Drives / Network / Tags / Recent | 🟡 |
| H5 缩略图 / QA 监听 / Eject / pintohome / Storage Sense | ✅ |
| H6 InfoBar / Shell 菜单设置 / widget 顺序（右键上移下移） | ✅ |
| H6 拖放 widget 重排、Expander 动画 | ⛔ 不做（见 [`home-files-parity.md`](home-files-parity.md)） |

---

## D. 文件操作

| 能力 | 状态 |
|------|------|
| 新建文件夹 / 文件 | ✅ |
| 重命名 | ✅ |
| 系统默认打开 | ✅ |
| 复制路径 | ✅ |
| 复制/移动/粘贴 | 应用内剪贴板 | ✅ |
| 粘贴（资源管理器 CF_HDROP） | 读取 + 后台粘贴 | 🟡 |
| 回收站删除 / 永久删除 | ✅ |
| 拖拽 | GPUI 拖放 + 后台传输通知 | 🟡 |
| 冲突对话框 / StatusCenter | 五键冲突框 + 状态栏进度条/取消；传输可逐项取消 🟡 |

实现：`crates/ui/src/file_ops.rs`（后台 `copy_items` / `move_items` + 进行中/完成通知）。

---

## E. 文件模型扩展

| 类型 | 状态 |
|------|------|
| 本地文件/文件夹/符号链接 | ✅ |
| 回收站虚拟浏览 | 🟡 |
| 驱动器根 | 🟡 |
| 库 / 压缩包 / FTP / Git 列 / 云 / 完整标签系统 | ⬜ |

---

## F. Windows（`platform-windows`）

| 能力 | 状态 |
|------|------|
| Shell 图标 PNG | 🟡 |
| Files 式 Flyout + `IContextMenu` 解析 | 🟡 |
| 回收站枚举 / 剪贴板 / 属性 / OpenAs 对话框 | 🟡 |
| Open with 子菜单（`openas` Shell 子树） | 🟡（依赖 Shell 预取缓存） |
| WSL / 云 API / 快捷方式解析 | ⬜ |

---

## G. 命令与设置

| 能力 | 状态 |
|------|------|
| CommandManager | 手写 `commands` | 🟡 |
| 可定制热键 / 命令面板 | ⬜ |
| 完整设置页 | 仅 4 页 | 🟡 |
| 固定文件夹 / 视图偏好持久化 | ✅ / 🟡 |

---

## H. 应用生命周期

| 能力 | 状态 |
|------|------|
| 启动 + 窗口尺寸持久化 | 🟡 |
| 单实例 / CLI / 协议 / 托盘 | ⬜ |

---

## 推荐实施顺序（下一轮）

按 **体验差距 × 文档成熟度** 排序；每步更新本文档 ✅/🟡/⬜。

### 第一梯队（进行中 / 当前迭代）

1. **文件传输反馈** — `file_ops.rs` 后台传输 + 冲突五键对话框 + 状态栏提示 🟡。
2. **右键菜单** — 阶段 B ✅；阶段 C：内置项可配置 🟡、分享项 ⬜。
3. **子菜单行高** — PopupMenu Submenu 与 Item 统一 32px hover ✅。
4. **工具栏 tooltip i18n** — 全 icon button ✅。

### 第二梯队

5. **ShelfPane** 暂存条（粘贴 + 预览）🟡；拖放进 Shelf ⬜。
6. 设置扩展：Folders、Tags、Actions、Context menu 开关。🟡。
7. 会话恢复增强：双栏状态 + 最近关闭标签（View 菜单列表）。🟡。
8. StatusCenter：进度条、逐项/大文件取消。🟡。

### 第三梯队

9. Home H5–H6 ✅（拖放重排、Expander 动画见 [`home-files-parity.md`](home-files-parity.md) 明确不做）。
10. 侧栏 Shell 位图深度、库/云分区 polish。
11. 新 crate：`tags`、`archive`、`git`。

### 已完成里程碑（归档）

- MainPage 骨架、Home H0–H6、双栏 + InfoPane、列/网格/详情、回收站列表、Files 式右键首轮、Omnibar 面包屑 P1、Pinned 持久化、窗口 bounds 防抖保存、设置标签 Enter 提交、Home widget 顺序。

每步保持 `cargo check` / `cargo run -p cyberfiles` 通过。
