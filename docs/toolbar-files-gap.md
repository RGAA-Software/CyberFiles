# Toolbar / InnerNavigationToolbar 差距文档

对照：`../Files/src/Files.App/UserControls/Toolbar.xaml`

## 当前状态

CyberFiles `file_browser` 有两处工具栏：
- `render_content_toolbar`（内容区上方）
- Omnibar 下方导航工具栏（`show_toolbar` 分支）

两者功能高度重复，且都远未对齐 Files。

---

## 差距清单

### 1. Context Commands（左侧动态命令栏）⬜

Files 的 `ContextCommandBar` 根据选中项动态显示快捷操作按钮。

| 按钮 | 图标需求 | 状态 |
|------|----------|------|
| Cut（剪切） | `content_cut` | ❌ 无按钮 |
| Copy（复制） | `content_copy` | ❌ 无按钮 |
| Paste（粘贴） | `content_paste` | ❌ 无按钮 |
| Rename（重命名） | `edit` / `drive_file_rename_outline` | ❌ 无按钮 |
| Delete（删除） | `delete` | ✅ 已有 |
| Properties（属性） | `info` | ❌ 无按钮 |
| Share（共享） | `share` | ❌ 无按钮 |
| Open with（打开方式） | `widgets` | ❌ 无按钮 |

**实现要点：**
- 左侧动态区域，有选中项时显示；无选中项时隐藏或置灰
- 图标+文字标签（或仅图标）
- 与右键菜单对应命令复用同一 action

### 2. Filter Toggle ⬜

Files 有独立的 Filter header toggle 按钮。

### 3. Selection Options（选择操作）⬜

| 菜单项 | 状态 |
|--------|------|
| Select All（全选） | ❌ |
| Invert Selection（反选） | ❌ |
| Clear Selection（清除选择） | ❌ |

### 4. Sort / Arrangement（排序与排列）🟡

**当前已有：** Name / Modified / Created / Size（4种）

**Files 有但 CyberFiles 缺：**

| SortBy 项 | 状态 |
|-----------|------|
| Type（类型） | ❌ |
| Sync Status（同步状态） | ❌ |
| Tag（标签） | ❌ |
| Path（路径） | ❌ |
| Original Folder（原始文件夹） | ❌ |
| Date Deleted（删除日期） | ❌ |

**GroupBy（分组）—— 完全未实现 ⬜：**

| GroupBy 项 | 状态 |
|------------|------|
| None | ❌ |
| Name | ❌ |
| Date Modified（Year/Month/Day） | ❌ |
| Date Created（Year/Month/Day） | ❌ |
| Size | ❌ |
| Type | ❌ |
| Sync Status | ❌ |
| Tag | ❌ |
| Original Folder | ❌ |
| Date Deleted（Year/Month/Day） | ❌ |
| Folder Path | ❌ |

**排序优先级：**
| 选项 | 状态 |
|------|------|
| Sort Folders First | ❌ |
| Sort Files First | ❌ |
| Sort Files and Folders Together | ❌ |

### 5. Layout Options（布局选项）🟡

**当前已有：** Details / Grid / Columns（3种）

**Files 有但 CyberFiles 缺：**

| 布局 | 状态 |
|------|------|
| List（列表） | ❌ |
| Cards（卡片/平铺） | ❌ |

**视图大小调节（Slider）—— 完全未实现 ⬜：**

Files 每种布局都有独立的大小 Slider（Compact/Small/Medium/Large/ExtraLarge）。

### 6. Toggle Options（开关选项）🟡

| 开关 | 状态 |
|------|------|
| Show Hidden Files（显示隐藏文件） | ✅ 已有（排序菜单中） |
| Show File Extensions（显示扩展名） | ❌ |
| Adaptive Layout（自适应布局） | ❌ |

### 7. Preview Pane Toggle（预览面板开关）✅

已有，在 Omnibar 右侧工具栏。

---

## 推进优先级

按「体验差距 × 实现成本」排序：

### P0 - 当前迭代（工具栏补齐）
1. **Context Commands 按钮栏** — 剪切/复制/粘贴/重命名/属性 ✅
2. **List 布局** — 增加列表视图（比 Details 更紧凑）✅
3. **Cards 布局** — 卡片/平铺视图 ✅
4. **显示扩展名开关** — 设置项 + 工具栏开关 ✅

### ⛔ 明确不复刻
- **视图大小 Slider 调节** — gpui-component 无 Slider 组件；改用固定尺寸设计 ✅

### P1 - 下一轮
5. **GroupBy 分组** — 文件分组显示（按名称首字母、日期等）
6. **Selection Options** — 全选/反选/清除选择

### P2 - 远期
7. **更多 SortBy 选项** — Type / Tag / Path 等
8. **更多 GroupBy 选项** — Sync Status / Original Folder 等
9. **Adaptive Layout** — 自适应布局
10. **Filter Toggle** — 过滤头

---

## 相关代码落点

| 层 | 路径 |
|----|------|
| 工具栏渲染 | `crates/ui/src/file_browser.rs` — `render_content_toolbar` / `show_toolbar` 分支 |
| 视图模式 | `crates/ui/src/file_browser.rs` — `ViewMode` 枚举 + `set_view_mode` |
| 排序 | `crates/ui/src/file_browser.rs` — `sort_label` / `on_sort_by_*` |
| 右键菜单 | `crates/ui/src/file_browser/context_menu.rs` |
| 配置持久化 | `crates/core/src/config.rs` — `AppConfig` |
