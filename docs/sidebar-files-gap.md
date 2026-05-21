# 左侧边栏（Sidebar）vs Files — 差距分析

对照 `Files.App.ViewModels.UserControls.SidebarViewModel` + `NavigationControl`。  
CyberFiles 实现：[`crates/ui/src/sidebar/`](../crates/ui/src/sidebar/)（`render_sidebar` + `data.rs` 分区加载）。

路线图索引：[`files-parity-roadmap.md`](files-parity-roadmap.md) §A 侧栏一行。

---

## Files 侧栏结构（Section 顺序）

固定顺序（`SectionOrder`）：

| 顺序 | Section | 内容 |
|------|---------|------|
| 1 | **Home** | 虚拟「首页」入口（非路径） |
| 2 | **Pinned** | 固定到侧栏的文件夹（与 Shell Quick Access / `pintohome` 同步） |
| 3 | **Library** | Windows 库（文档、图片、音乐等 Known Folder） |
| 4 | **Drives** | 逻辑盘（含可移动、空驱隐藏策略） |
| 5 | **CloudDrives** | OneDrive 等云盘挂载 |
| 6 | **Network** | 网络计算机 / 共享 |
| 7 | **WSL** | 已安装 WSL 发行版根目录 |
| 8 | **FileTag** | 用户定义文件标签（点击过滤列表） |
| 底栏 | **Settings** | 设置页（独立项，非 Section 子项） |

各 Section 可在设置中单独显示/隐藏（`ShowPinnedSection`、`ShowLibrarySection` 等）。

---

## CyberFiles 现状（C 档骨架，2026-05）

```
Sidebar Header（应用名 + workspace）
├─ Main        → Home + 回收站
├─ Pinned      → Shell Quick Access（Windows）+ settings.json pinned_folders
├─ Library     → FOLDERID_LIBRARIES（Windows）
├─ Drives      → list_drives()
├─ Cloud       → OneDrive / Google Drive / Dropbox 用户目录（Windows）
├─ Network     → FOLDERID_NETWORK（Windows）
├─ WSL         → \\wsl.localhost\ / \\wsl$\ 发行版根（Windows）
├─ File tags   → settings.json file_tags（首路径导航，非过滤）
└─ Footer      → Settings + 折叠按钮（compact/minimal 模式）
```

| 能力 | Files | CyberFiles | 状态 |
|------|--------|------------|------|
| 可拖拽调宽度 | `h_resizable` | `h_resizable` | ✅ |
| Home 入口 | SectionType.Home | `NavigationTarget::Home` | 🟡 Home 页 widget 仍不全 |
| Pinned | Shell 同步 + 重排对话框 | QA + `pinned_folders`；右键上移/下移 | 🟡 未 `pintohome` 双向同步 |
| 库（Library） | `LibraryManager` | `list_known_folder_folders` | 🟡 |
| 驱动器 | 图标、弹出、属性 | `list_drives` + 属性菜单 | 🟡 无 Eject |
| 云驱动器 | `CloudDrivesManager` | 用户目录探测 | 🟡 |
| 网络 | `INetworkService` | Known Folder Network | 🟡 |
| WSL | `WSLDistroManager` | UNC 根枚举 | 🟡 |
| 文件标签 | 侧栏过滤列表 | 配置项导航到路径 | ⬜ 过滤未做 |
| 回收站 | 多处入口 | Main 区一项 | ✅ |
| Settings 底栏 | 独立项 | Footer + `render` | ✅ |
| **当前选中高亮** | 随路径更新 | `navigation_matches` + canonicalize | ✅ |
| **紧凑/展开/最小** | `SidebarDisplayMode` | `expanded` / `compact` / `minimal` + 设置页 | ✅ |
| Section 显示开关 | 各 Section 布尔 | 设置 → Sidebar 页 | ✅ |
| 空 Section 隐藏 | `AreSectionsHidden` | `build_sidebar_sections` 跳过空列表 | ✅ |
| 右键菜单 | 固定/重排/新标签/属性 | 打开、新标签、Pin/Unpin、上下移、属性 | 🟡 无 Shell verbs / Eject |
| 拖放文件到侧栏项 | 复制/移动到该路径 | — | ⬜ |
| 与 Explorer 固定同步 | `UpdateItemsWithExplorerAsync` | 只读 Shell QA 列表 | ⬜ |
| Shell 位图图标 | 异步替换 | `icon_hint_for_path` → Lucide | 🟡 |
| 中键新标签 | 支持 | `on_middle_click` | ✅ |
| 侧栏项重排对话框 | `ReorderSidebarItemsDialog` | 右键上移/下移（仅 settings pinned） | 🟡 |

---

## 仍待你验收 / 后续打磨

- 拖放到侧栏路径、驱动器弹出、Explorer `pintohome` 同步、真实 Shell 图标。
- `file_tags` 点击应过滤当前列表（Files `FileTagsManager`），现为导航占位。
- 云盘 / 网络 / WSL 在慢机或权限不足时的错误与缓存策略。
- Home Section 与 Files 虚拟 Home 的 widget 对齐。

---

## 一比一复刻建议分期（历史）

- **A**：Home / Pinned / Drives / Settings + 高亮 + 折叠  
- **B**：Library / Network / 空 Section 隐藏  
- **C**：Cloud / WSL / FileTags 骨架 + 右键 + 中键 + 设置开关 + display mode（**当前批次**）
