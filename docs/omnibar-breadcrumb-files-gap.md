# Omnibar 面包屑 vs Files — 功能差距

对照 `Files.App.Controls.BreadcrumbBar` + `NavigationToolbar.xaml.cs` + `NavigationToolbarViewModel.SetPathBoxDropDownFlyoutAsync`。  
CyberFiles 实现：`PathBreadcrumbBar` / `OmnibarBreadcrumbHost`（[`crates/ui/src/omnibar/`](crates/ui/src/omnibar/)）。

回归清单：[`omnibar-breadcrumb-bugs.md`](omnibar-breadcrumb-bugs.md)。

---

## 已对齐（✅）

| Files 行为 | CyberFiles |
|------------|------------|
| 路径模式失焦显示面包屑，聚焦显示路径框 | ✅ `omnibar_editing` |
| 独立根节点（Home 图标）+ 路径段 | ✅ `show_root` + `path_breadcrumbs` |
| 段 = 文字按钮 + 右侧 `›` 下拉，无段间 `›` | ✅ |
| 最后一段无 chevron、点文字不导航 | ✅ |
| 前缀过长 → `…` 隐藏左侧段，保留尾部 | ✅ `breadcrumb_visible_layout_for_width` |
| 点段 / `…` 项导航 | ✅ |
| 中键非最后段 → 新标签 | ✅ `open_path_in_new_tab` |
| 根 `›`：快速访问 + 驱动器分组 | ✅ pinned + `list_drives`（打开时刷新） |
| 段 `›`：子文件夹列表 | ✅ `read_dir` |
| 下拉排除当前工作目录 | ✅ `exclude_path` |
| 尊重「显示隐藏文件」（基础） | ✅ `show_hidden_items` |
| 拖放到目录段 | ✅ `on_drop` |
| 拖到非最后段悬停预览进目录 | ✅ `on_drag_move` + 350ms debounce（`BREADCRUMB_DRAG_HOVER_OPEN_MS`） |
| 失焦回 Path 模式 | ✅ |
| 避免嵌套 `MainPage::update` 崩溃 | ✅ `location_changed` defer |

---

## 待补齐（按优先级）

### P1 — 体验/与 Files 明显不一致

| # | Files | 现状 | 建议实现 |
|---|--------|------|----------|
| 1 | 拖放悬停 **1300ms** 后才进入该目录（`HoverToOpenTimespan`），且**最后一段不触发** | **350ms** debounce（可调常量）；最后段仍无 hover | 若需完全对齐 Files，将 `BREADCRUMB_DRAG_HOVER_OPEN_MS` 改为 `1300` |
| 2 | 无法枚举子目录时显示 **「访问被拒绝」** 占位项 | 空列表或「没有子文件夹」 | `breadcrumb_dropdown_entries` 区分 `read_dir` 失败 vs 空 |
| 3 | 隐藏项在下拉中 **半透明**；另含 **系统/点文件** 设置 | 仅过滤 hidden，无 dim、无 system/dot | 对齐 `DirectoryReadOptions` / Win32 `FindFirstFile` 逻辑 |
| 4 | 根快速访问来自 **Shell Quick Access**（非仅 pinned） | 仅 `settings.json` pinned | 平台层 `list_quick_access()`，根菜单合并 |
| 5 | 下拉 **打开时异步填充**、关闭清空（省内存） | 打开时同步 `read_dir`；菜单每次重建 | 打开回调里 spawn 填菜单；关时清空（若 API 支持） |

### P2 — 视觉/布局/键盘

| # | Files | 现状 | 建议 |
|---|--------|------|------|
| 6 | `BreadcrumbBarLayout` **逐段实测**子项宽度折叠 | host **canvas 实测**可用宽度 + 段标签字符估算 | 逐段 DOM/文本度量（GPUI 无现成 API） |
| 7 | Chevron **展开旋转 90°**（`ChevronNormalOn/Off`） | ✅ `Popover` + `BreadcrumbChevronTrigger` | — |
| 8 | 下拉项 **Shell 缩略图** 异步替换占位图标 | 菜单项左侧 **类型图标**（`icon_hint`，与文件列表同类） | 非 Win32 `SHGetFileInfo` 位图；`img(路径)` 对文件夹无效已移除 |
| 9 | `Tab` 失焦 Omnibar 后 **焦点到面包屑** | 无 | Path 模式下 Tab → `track_focus` 到 breadcrumb |
| 10 | 根/段 **圆角一体块**（根左侧大圆角） | 简单 `rounded` | 微调样式对齐 XAML |

### P3 — 进阶/非本地路径

| # | Files | 现状 |
|---|--------|------|
| 11 | 非文件系统路径（FTP 等）`StorageFolder` 回退枚举 | 仅本地 `read_dir` |
| 12 | 面包屑段 **拖放** 与列表拖放统一 deferral/锁 | 基础 drop，无 `_lockFlag` |
| 13 | 无障碍：Landmark、AutomationName | 未专门做 |
| 14 | 网络/特殊目标（Settings/Recycle）在 Files 中部分禁用拖放 | 伪路径段行为需单独测 |

---

## 建议实施顺序

1. **P1-1** 拖放悬停 1300ms debounce（改动小、体感最接近 Files）  
2. **P1-2** 访问拒绝 vs 空目录文案  
3. **P1-3** 隐藏/系统/点文件与 dim 透明度  
4. **P1-4** Shell Quick Access（依赖 `platform` / `fs`）  
5. **P2-6/7** 实测宽度 + chevron 动画  
6. 其余按产品优先级排期  

---

## Omnibar 整体（非仅面包屑）

Files 还有 **Command Palette / Search** 等 Omnibar 模式；CyberFiles 已有 Command 模式雏形，**Search 模式**仍缺。见 [`files-parity-roadmap.md`](files-parity-roadmap.md) 地址栏一行。
