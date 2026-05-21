# 左侧边栏（Sidebar）vs Files — 差距分析

对照 `Files.App.ViewModels.UserControls.SidebarViewModel` + `NavigationControl`。  
CyberFiles 实现：[`crates/ui/src/sidebar/`](../crates/ui/src/sidebar/)（`cache.rs` + `data.rs` + `view.rs`）。

路线图索引：[`files-parity-roadmap.md`](files-parity-roadmap.md) §A 侧栏一行。

---

## Files 侧栏结构（Section 顺序）

| 顺序 | Section | 内容 |
|------|---------|------|
| 1 | **Home** | 虚拟「首页」 |
| 2 | **Pinned** | Shell Quick Access + `pintohome` 同步 |
| 3 | **Library** | Windows 库 |
| 4 | **Drives** | 逻辑盘（空驱隐藏、Eject） |
| 5 | **CloudDrives** | 云盘挂载 |
| 6 | **Network** | 网络计算机 / 共享 |
| 7 | **WSL** | WSL 发行版根 |
| 8 | **FileTag** | 标签 → **过滤** 当前列表 |
| 底栏 | **Settings** | 独立项（左下） |

---

## CyberFiles 现状（2026-05）

```
Header（应用名）
├─ Main        → Home、回收站
├─ Pinned      → Shell QA + settings.json（去重）
├─ Library / Drives / Cloud / Network / WSL / File tags（可设置隐藏，空则省略）
└─ Footer（左下）→ Settings（独立一行）+ 折叠钮（compact/minimal）
```

**近期已做：** 异步侧栏缓存、拖放到目录项、盘符仅根目录高亮、布局不溢出、`FilePathDrag`（gpui-component）。

---

## 能力对照表

| 能力 | Files | CyberFiles | 状态 |
|------|--------|------------|------|
| Section 顺序与开关 | 8 区 + 设置 | 同序 + 设置页开关 | ✅ |
| 可拖拽调宽度 | `h_resizable` | `flex_none` + `overflow_hidden` | ✅ |
| Settings 左下独立 | Footer 项 | Footer 仅设置（非 Main 组） | ✅ |
| 当前选中高亮 | 路径 / 虚拟目标 | `navigation_matches`（盘符不级联） | ✅ |
| 紧凑 / 展开 / 最小 | `SidebarDisplayMode` | expanded / compact / minimal | ✅ |
| 空 Section 隐藏 | 是 | 是 | ✅ |
| 中键新标签 | 是 | `on_middle_click` | ✅ |
| 拖放到侧栏项 | 是 | `on_file_drop`（Ctrl=复制） | ✅ |
| 侧栏列表缓存 | 异步刷新 | 后台 `build_sidebar_sections` | ✅ |
| 右键：打开 / 新标签 | 是 | 是 | ✅ |
| 右键：Pin / Unpin / 上下移 | 是（含对话框） | 仅 settings pinned 上下移 | 🟡 |
| 右键：属性 | Shell | `open_item_properties` | 🟡 |
| Home 页内容 | Home 小组件 | `NavigationTarget::Home` 占位 | 🟡 |
| Pinned ↔ Explorer | `pintohome` 双向 | 只读 QA + 本地 pinned | 🟡 |
| Library | `LibraryManager` | Known Folder 枚举 | 🟡 |
| Drives | 图标 + Eject + 空驱策略 | 枚举 + 属性；无 Eject | 🟡 |
| Cloud | `CloudDrivesManager` | 用户目录名探测 | 🟡 |
| Network | `INetworkService` 枚举 | Network Known Folder | 🟡 |
| WSL | `WSLDistroManager` | UNC 根目录 | 🟡 |
| File tags | **点击过滤列表** | 导航到路径占位 | ⬜ |
| Shell 位图图标 | 异步 Shell 图标 | Lucide 分类图标 | 🟡 |
| 与 Explorer 固定同步 | `UpdateItemsWithExplorerAsync` | 无 | ⬜ |
| 驱动器弹出 (Eject) | 有 | 无 | ⬜ |
| Shell 扩展右键 verbs | 完整 Flyout | 固定几项 | ⬜ |
| 重排对话框 | `ReorderSidebarItemsDialog` | 无 | ⬜ |
| 拖停悬停自动展开 | 有 | 复用面包屑 1.3s 预览 | 🟡 |

**图例：** ✅ 可用且对齐 · 🟡 有骨架/部分行为 · ⬜ 未做

---

## 还差多少（粗估）

| 档位 | 说明 | 约占比 |
|------|------|--------|
| **结构 + 日常导航** | 分区、高亮、折叠、设置底栏、拖放、中键、缓存 | **~75%** |
| **Windows 深度集成** | Shell 图标、Explorer 同步、Eject、网络枚举、云盘 API | **~25%** 未做 |
| **Files 独有产品** | 标签过滤、重排对话框、完整 Shell 右键 | **~15%** 未做 |

整体：**侧栏「能用的 Files 式导航」已基本具备**；与 Files **像素级 / Shell 级** 对齐还差 mainly **标签过滤、Explorer 同步、真实图标、Eject、网络/云盘完善、重排 UI**。

---

## 建议下一批（按性价比）

1. **File tags 点击 → 过滤当前文件列表**（Files 语义差异最大的一项）
2. **驱动器 Eject + 空可移动盘隐藏**
3. **Shell 图标异步加载**（侧栏 + 驱动器）
4. **Explorer `pintohome` 只读/写入同步**
5. **Pinned 重排对话框**（替代仅上下移）

---

## 依赖说明

拖放 / 整行点击依赖本地 [`../gpui-component`](../gpui-component) 的 `sidebar/menu.rs`（`FilePathDrag`、`on_file_drop`）。部署前需保证该 fork 已提交或与 CyberFiles 同机 path 一致。
