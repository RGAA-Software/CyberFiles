# Omnibar 面包屑 vs Files — 功能差距

对照 `Files.App.Controls.BreadcrumbBar` + `NavigationToolbar.xaml.cs`。  
CyberFiles：`PathBreadcrumbBar` / `OmnibarBreadcrumbHost` / `BreadcrumbFlyout`（[`crates/ui/src/omnibar/`](../crates/ui/src/omnibar/)）。

回归清单：[`omnibar-breadcrumb-bugs.md`](omnibar-breadcrumb-bugs.md)。

---

## 已对齐（✅）

| Files 行为 | CyberFiles |
|------------|------------|
| 路径模式：面包屑 ↔ 可编辑路径框（点击空白进编辑） | ✅ `omnibar_show_full_path` + `Input` |
| 文件夹内搜索在地址栏右侧独立框 | ✅ `search_input`；`Ctrl+L` 聚焦搜索 |
| 独立根节点 + 路径段 + 段 `›` 下拉 | ✅ |
| 最后一段无 chevron、点文字不导航 | ✅ |
| 前缀 `…`、CJK 宽度估算、菜单像素截断 | ✅ |
| 中键新标签、拖放、悬停进目录 | ✅ |
| 悬停 **1300ms**（`HoverToOpenTimespan`） | ✅ `BREADCRUMB_DRAG_HOVER_OPEN_MS` |
| 无法枚举子目录 → **访问被拒绝** | ✅ `BreadcrumbDropdownResult::AccessDenied` |
| 空子文件夹占位 | ✅ `BreadcrumbDropdownResult::Empty` |
| 根快速访问（Shell Frequent）+ 驱动器 | ✅ Windows `list_shell_quick_access_folders` |
| 下拉隐藏项半透明（显示隐藏时） | ✅ `OmnibarPathSuggestion.dimmed` |
| 下拉排除当前目录、隐藏项过滤 | ✅ |
| `BreadcrumbFlyout` 窗口坐标 + chevron 旋转 | ✅ |
| 点击外部（含标题栏）/ Esc 回面包屑 | ✅ `AppShell` + `omnibar-host` stop_propagation |
| 段下拉异步 `read_dir`、关闭释放菜单 | ✅ `BreadcrumbFlyout::new_async` |
| 窗口 resize 防抖写入 `settings.json` | ✅ `AppShell` 400ms debounce |

**已移除（相对旧文档）**：Path/Commands 切换、Omnibar 内路径自动完成、命令面板。

---

## 待补齐

### P1

| # | Files | 现状 |
|---|--------|------|
| 1 | system 项单独开关时的下拉规则 | 与列表一致过滤；无单独 dim |
| 2 | （已完成）Shell Quick Access | — |
| 3 | （已完成）下拉异步填充、关闭清空 | — |

### P2

| # | Files | 现状 |
|---|--------|------|
| 4 | 逐段 **实测**宽度折叠 | unicode-width 估算 |
| 5 | Tab 失焦后焦点回面包屑 | 无 |
| 6 | 根/段一体圆角块 | 简单 `rounded` |
| 7 | 下拉 Shell 缩略图 | 类型图标 |

### P3

非本地路径、拖放锁、无障碍等 — 见 [`files-parity-roadmap.md`](files-parity-roadmap.md)。

---

## 文件列表（本轮）

| Files | CyberFiles |
|-------|------------|
| 默认不选中第一项 | ✅ |
| 点空白取消选中 | ✅ |
| 空白区右键：粘贴/新建/刷新 | ✅ `build_background_context_menu` |

---

## 建议下一步

1. 操作进度/冲突对话框  
2. Tab 失焦后焦点回面包屑（P2）  
