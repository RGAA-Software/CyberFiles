# Files 一比一复刻路线图

对照项目：`../Files`（WinUI 3）。本文档按 **Files 真实结构** 追踪 CyberFiles 的实现状态，作为长期复刻清单。

**复刻原则：** Rust + GPUI + gpui-component 一比一复刻 **Files**（非 Explorer）。右键菜单等行为见 [`files-context-menu-parity.md`](files-context-menu-parity.md)。

架构参考（Files `MainPage`）：

```text
MainWindow
└─ MainPage
   ├─ TabBar                    ← 应用级多标签
   ├─ NavigationToolbar         ← 后退/前进/上级/刷新/地址栏
   ├─ Sidebar                   ← Home / 固定 / 库 / 驱动器 / 网络 / WSL / 标签
   └─ RootGrid
      ├─ Toolbar（布局内）
      ├─ Shell 内容（ShellPanesPage → ModernShellPage → 布局页）
      ├─ InfoPane（可选）
      ├─ StatusBar
      └─ ShelfPane（可选）
```

CyberFiles 目标 workspace（与 `files-rust-port-plan.md` 一致，按边界逐步加 crate）：

| Crate | 职责 |
|-------|------|
| `core` | 配置、会话、常量 |
| `fs` | 与 UI 无关的文件模型与操作 |
| `commands` | CommandManager、热键、启用状态 |
| `ui` | GPUI MainPage、Shell、布局、设置 |
| `platform-windows` | Shell/COM、图标提示、回收站路径、剪贴板 CF_HDROP |
| `previews` / `search` / `tags` / `archive` / `git` | 按功能启用 |

---

## 状态图例

| 标记 | 含义 |
|------|------|
| ✅ | 已有可用实现 |
| 🟡 | 部分实现 / 占位 |
| ⬜ | 未开始 |

---

## A. 应用外壳（MainPage）

| Files 能力 | CyberFiles | 状态 |
|------------|------------|------|
| 应用级 TabBar | `main_page` | 🟡 |
| 新建/关闭/切换标签 | Tab +/- | 🟡 |
| 标签标题随路径更新 | `tab_title` 读当前目录 | 🟡 |
| 会话恢复 / 最近关闭标签 | — | ⬜ |
| NavigationToolbar（窗口级） | `nav_toolbar` | 🟡 |
| 地址栏 / Omnibar | 面包屑 + 编辑 + 建议 + 命令模式 + Ctrl+L | 🟡 |
| 侧栏折叠 | `h_resizable` | ✅ |
| Sidebar 分区结构 | Home/Pinned/Drives/Network/Settings | 🟡 |
| 固定文件夹（Pinned） | 侧栏 + Home + Pin + `settings.json` | ✅ |
| 内容区 Shell（单栏） | `PaneShell` | 🟡 |
| 双栏 ShellPanesPage | `ShellPanes` 可切换双栏 | ✅ |
| InfoPane 右/底 | Details + 文本/图片 Preview（仅右侧） | 🟡 |
| ShelfPane | 占位 | 🟡 |
| StatusBar | `status_bar` | 🟡 |
| 设置全页 | `settings_view` | ✅ |

---

## B. 导航与 Shell 内容

| Files 能力 | CyberFiles | 状态 |
|------------|------------|------|
| Home 虚拟路径 | `NavigationTarget::Home` | 🟡 |
| 路径导航 | `FileBrowser` | ✅ |
| 每标签独立历史 | 每标签 `FileBrowser` | ✅ |
| 后退/前进/上级/刷新 | 工具栏 + 快捷键 | ✅ |
| 列视图 | `ViewMode::Columns` + Ctrl+3 | 🟡 |
| 详情列表 | 默认 `ViewMode::Details` | ✅ |
| 网格布局 | `ViewMode::Grid` + Ctrl+2 | ✅ |
| 分栏布局 | `ShellPanes` 双栏 | ✅ |
| 搜索当前文件夹 | 导航栏过滤框 + Ctrl+F 聚焦 | 🟡 |
| 目录变更自动刷新 | `notify` 防抖监视 | 🟡 |
| 排序/分组 UI | 排序菜单 | 🟡 |
| 多选 Ctrl/Shift | ✅ | ✅ |
| 键盘快捷键 | `commands` | ✅ |

---

## C. Home 页小部件

| Widget | 状态 |
|--------|------|
| Quick Access | 固定文件夹列表（可点击导航） | 🟡 |
| Drives | 🟡 |
| Network locations | ⬜ |
| File tags | ⬜ |
| Recent files | Windows Recent `.lnk` 列表 | 🟡 |

---

## D. 文件操作

| 能力 | 状态 |
|------|------|
| 新建文件夹 | ✅ |
| 新建文件 | ✅ |
| 重命名 | ✅ |
| 系统默认打开 | ✅ |
| 复制路径 | ✅ |
| 复制/移动/粘贴 | Ctrl+C/X/V 应用内剪贴板 | ✅ |
| 粘贴（资源管理器） | CF_HDROP 读取 | 🟡 |
| 回收站删除 | Delete → `trash`（Windows） | ✅ |
| 永久删除 | Shift+Delete | ✅ |
| 拖拽 | GPUI 拖放移动/复制（Ctrl=复制） | 🟡 |
| 冲突/进度中心 | ⬜ |

---

## E. 文件模型扩展（ListedItem 对标）

| 类型 | 状态 |
|------|------|
| 本地文件/文件夹/符号链接 | ✅ |
| 快捷方式元数据 | ⬜ |
| 驱动器根 | 🟡 |
| 回收站虚拟浏览 | `read_recycle_bin` + `BrowseLocation::RecycleBin` | 🟡 |
| 库（Libraries） | ⬜ |
| 压缩包内浏览 | ⬜ |
| FTP | ⬜ |
| Git 状态列 | ⬜ |
| 云同步状态 | ⬜ |
| 文件标签 | ⬜ |

---

## F. Windows 平台（`platform-windows`）

| 能力 | 状态 |
|------|------|
| Shell 图标分类提示 | `icon_hint_for_path` | 🟡 |
| 快速枚举 | — | ⬜ |
| 右键菜单（Files 式 Flyout） | GPUI `ContextMenu` + 后台 `query_shell_context_menu_items` 合并 | 🟡 |
| 回收站虚拟浏览 | Shell `EnumObjects` 枚举已删除项 | 🟡 |
| 系统剪贴板文件列表 | `read_clipboard_file_paths` | 🟡 |
| 属性对话框 | `open_item_properties` | 🟡 |
| 快捷方式解析 | ⬜ |
| WSL 路径 | ⬜ |
| 云提供商占位 | ⬜ |

---

## G. 命令与设置

| 能力 | 状态 |
|------|------|
| CommandManager 代码生成 | 手写 `commands` | 🟡 |
| 可定制工具栏/热键 | — | ⬜ |
| 命令面板 | — | ⬜ |
| 完整设置页（Folders/Actions/Tags/Dev） | 仅 General | 🟡 |
| 固定文件夹持久化 | `pinned_folders` in `settings.json` | ✅ |
| 视图/排序/隐藏文件持久化 | `file_view_mode` 等 | 🟡 |

---

## H. 应用生命周期

| 能力 | 状态 |
|------|------|
| 正常启动 + 窗口尺寸持久化 | 退出时保存尺寸 🟡 |
| 单实例 | ⬜ |
| 命令行打开路径 | ⬜ |
| 协议/文件激活 | ⬜ |
| 托盘后台 | ⬜ |

---

## 推荐实施顺序（路径 B）

1. ~~MainPage 骨架~~ ✅
2. ~~Home 小部件（基础）~~ 🟡
3. ~~M3 收尾~~ ✅（新建文件、CF_HDROP 粘贴、Pinned 持久化）
4. ~~M4（首轮）~~ 🟡（notify、文件夹过滤、网格视图）
5. ~~`platform-windows`（首轮）~~ 🟡（图标提示、属性、回收站路径、剪贴板）
6. ~~双栏 + InfoPane 预览~~ ✅
7. ~~列视图 / 应用内右键 / 拖放 / Ctrl+F / 视图偏好持久化~~ 🟡（首轮）
8. ~~回收站虚拟列表~~ 🟡（首轮）
9. ~~Files 式右键菜单（首轮）~~ 🟡：GPUI 菜单 + 后台 Shell 合并；空白区菜单、Shift 扩展待做
10. **进行中**：Omnibar 增强（面包屑/建议/命令模式已首轮）、操作进度/冲突、窗口 resize 时保存
11. **下一步**：OS 级拖放、回收站还原、M6 高级功能。

每步保持 `cargo check` 通过；功能项在本文档更新 ✅/🟡/⬜。
