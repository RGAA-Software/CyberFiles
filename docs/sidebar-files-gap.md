# 左侧边栏（Sidebar）vs Files — 差距分析

对照 `Files.App.ViewModels.UserControls.SidebarViewModel` + `NavigationControl`。  
CyberFiles 实现：[`crates/ui/src/main_page/mod.rs`](../crates/ui/src/main_page/mod.rs) 中 `render_sidebar`（`gpui_component::sidebar`）。

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

## CyberFiles 现状

```
Sidebar Header（应用名 + workspace 文案）
├─ Main        → Home（LayoutDashboard 图标）
├─ Pinned      → settings.json pinned_folders（Star）
├─ Places      → 仅回收站
├─ Drives      → list_drives()
├─ Network     → 占位 disabled
└─ Footer      → Settings
```

| 能力 | Files | CyberFiles | 状态 |
|------|--------|------------|------|
| 可拖拽调宽度 | `h_resizable` / 侧栏模式 | `h_resizable` | ✅ |
| Home 入口 | SectionType.Home | `NavigationTarget::Home` | 🟡 有入口，Home 页 widget 不全 |
| Pinned 列表 | Shell 同步 + 排序 + 重排对话框 | `pinned_folders` + 工具栏 Pin | 🟡 未与 Shell 固定同步 |
| 库（Library） | `LibraryManager` | — | ⬜ |
| 驱动器 | `DrivesViewModel`、图标、弹出、属性 | `list_drives` 平铺列表 | 🟡 |
| 云驱动器 | `CloudDrivesManager` | — | ⬜ |
| 网络 | `INetworkService` 枚举计算机 | 占位文案 | ⬜ |
| WSL | `WSLDistroManager` | — | ⬜ |
| 文件标签 | `FileTagsManager` + 侧栏过滤 | — | ⬜ |
| 回收站 | 常在 Pinned/Places 或独立 | Places 一节一项 | 🟡 |
| Settings 底栏 | `SettingsSidebarItem` | Footer 一项 | ✅ |
| **当前选中高亮** | `SidebarSelectedItem` 随路径更新 | 无选中态 | ⬜ |
| **紧凑/展开/最小** | `SidebarDisplayMode` | 仅宽度折叠 | ⬜ |
| Section 标题/折叠 | `LocationItem` 可展开分组 | `SidebarGroup` 静态标题 | 🟡 |
| 右键菜单 | 固定/取消固定、重排、打开新标签、Shell  verbs、弹出 | 无 | ⬜ |
| 拖放文件到侧栏项 | 复制/移动到该路径 | 无 | ⬜ |
| 驱动器弹出（Eject） | `EjectDeviceCommand` | 无 | ⬜ |
| 与 Explorer 固定同步 | `UpdateItemsWithExplorerAsync` | 无 | ⬜ |
| Shell 图标/缩略图 | 异步替换占位 | 固定 `IconName` | ⬜ |
| 空 Section 隐藏 | `AreSectionsHidden` | 仍显示 Network 占位 | 🟡 |
| 中键新标签打开 | 支持 | 无 | ⬜ |
| 侧栏项重排 | `ReorderSidebarItemsDialog` | 无（顺序=配置顺序） | ⬜ |

---

## 已对齐（✅ / 部分 🟡）

- 左侧可收起、与内容区 `h_resizable` 分割。
- **Home / Pinned / Drives / Settings** 骨架与 Files 概念对应。
- Pinned 持久化（`settings.json`）、工具栏 ⭐ Pin/Unpin。
- 驱动器枚举与点击导航。
- 回收站虚拟路径可进（`NavigationTarget::RecycleBin`）。

---

## 一比一复刻建议分期

### P0 — 结构与导航体感（优先）

1. **选中态**：当前路径 / Home / Settings / Recycle 对应项高亮（`SidebarMenuItem::selected` 或等价）。
2. **Section 对齐 Files 命名与顺序**：Home → Pinned →（Library）→ Drives → …；回收站位置与 Files 一致（通常不进 Drives）。
3. **Pinned 与 Shell 同步（Windows）**：复用 `list_shell_quick_access_folders` 或 `pintohome` 服务，与 `pinned_folders` 合并去重（面包屑已用 Frequent）。
4. **空 Section 不显示**：Network 无项时隐藏整组，而非 disabled 占位。

### P1 — 交互

5. 侧栏项 **右键菜单**：打开、新标签、固定/取消固定、从侧栏取消固定、属性（平台层已有 `open_item_properties` 可接）。
6. **中键** → `open_path_in_new_tab`。
7. Pinned **拖拽排序** 或「重排侧栏」对话框（可先只做配置顺序持久化）。
8. 驱动器项 **Shell 图标**（`icon_hint_for_path` / 已有 platform icons）。

### P2 — 平台扩展

9. **Library**（`FOLDERID_*` 库 Known Folders）。
10. **Network**（枚举 `Network`/`NetHood`）。
11. **CloudDrives** / **WSL** / **FileTag**（依赖后续 crate：tags、WSL 路径）。

### P3 — 显示模式

12. `SidebarDisplayMode`：Compact（仅图标）/ Expanded / Minimal，与设置联动。
13. 拖放文件到侧栏文件夹项（与 Files `HandleItemDragOverAsync` 一致）。

---

## 实现落点（CyberFiles）

| 改动 | 建议位置 |
|------|----------|
| 侧栏数据模型 | 新 `crates/ui/src/sidebar/` 或 `main_page/sidebar.rs`：`SidebarSection`、`SidebarEntry` |
| 选中态 | `MainPage` 根据 `active_pane` 的 `NavigationTarget` 计算 `selected_id` |
| Shell Pinned 同步 | `platform-windows` + `core::pinned_folders` 合并 |
| Library / Network | `platform-windows` Known Folder / `IShellFolder`（参考 `quick_access.rs`） |
| 右键 | `PopupMenu` + `build_sidebar_context_menu` |
| 设置开关 Section | `settings_view` + `AppConfig` 字段（对齐 Files `Show*Section`） |

---

## 与 Home 页关系

Files：**侧栏 Home** 进入 `Home` 聚合页（Quick access、Drives、Recent、Tags widgets）。  
CyberFiles：侧栏 Home ✅，[`home/page.rs`](../crates/ui/src/home/page.rs) 仅有 Pinned/Drives/Recent 部分 widget。侧栏复刻应与 Home widget 数据源统一（避免 Pinned 三处不一致：侧栏 / Home / 面包屑根菜单）。

---

## 建议你先拍板的范围

复刻侧栏时建议先选一档：

- **A. 最小可用**：选中态 + Section 顺序 + Pinned Shell 同步 + 空组隐藏（约 1–2 天）。
- **B. 标准 Files**：A + 右键 + 中键新标签 + 驱动器图标 + Pinned 重排（约 3–5 天）。
- **C. 完整**：B + Library + Network + 显示模式 + 拖放（按平台逐项，数周）。

确认档位后按 P0→P1 开 issue/分支实现即可。
