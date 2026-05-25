# Files 右键菜单一比一复刻说明

对照：`../Files`（WinUI 3）。CyberFiles 使用 **Rust + GPUI + gpui-component** 复刻同一产品行为，**不是**复刻资源管理器（Explorer）的 Win32 `TrackPopupMenu` 体验。

## Files 的真实行为

| 维度 | Explorer | Files | CyberFiles 目标 |
|------|----------|-------|-----------------|
| UI 容器 | 系统 HMENU 弹出 | WinUI `CommandBarFlyout`（应用内） | gpui-component `ContextMenu` / `PopupMenu` |
| 自有命令 | 无 | 打开、新标签、固定、标签、压缩等 | `commands` + 本地化条目 |
| Shell 扩展 | 全量原生 | `IContextMenu::QueryContextMenu` **解析为条目**后并入 Flyout | `platform-windows::query_shell_context_menu_items` → 并入 GPUI 菜单 |
| 默认右键 | 原生菜单 | **应用内 Flyout** | **应用内菜单** |
| Shift+扩展 | 系统行为 | `CMF_EXTENDEDVERBS` + 设置「更多」子菜单 | 后续对齐（设置 + Shift） |

参考源码：

- `Files.App/Views/Layouts/BaseLayoutPage.cs` — `ItemContextMenuFlyout` / `BaseContextMenuFlyout`
- `Files.App/Data/Factories/ContentPageContextFlyoutFactory.cs` — 先 Files 命令，再 Shell
- `Files.App/Utils/Shell/ContextMenu.cs` — `QueryContextMenu` + `EnumMenuItems`（**不** `TrackPopupMenu` 作为主 UI）

## 明确不做为默认路径

- ~~右键直接 `SHCreateDefaultContextMenu` + `TrackPopupMenu`~~（仅保留为可选调试/「完整系统菜单」入口，非主路径）
- ~~仅 GPUI 简化菜单、无 Shell 合并~~（M5 中间态，已废弃为主路径）

## CyberFiles 分阶段实现

### 阶段 A（已完成）

1. 文件列表右键 → **GPUI `ContextMenu`**（`ContextMenuExt`）
2. 菜单结构对齐 Files **首轮**：打开、重命名、复制、剪切、粘贴、删除、属性
3. **禁止**在 UI 线程同步调用 `query_shell_context_menu_items`；通过 `cx.background_spawn` 预取后并入菜单
4. 回收站虚拟页不合并 Shell（同 Files）
5. 已移除默认 `TrackPopupMenu` 路径（`ShellContextMenu` 动作）

### 阶段 B（已完成）

- **空白区域** `BaseContextMenuFlyout`：粘贴、布局/排序子菜单、新建、刷新
- **项菜单（Files 布局）**：顶栏图标快捷操作（剪切/复制/粘贴/重命名/删除/属性）→ 打开/新标签/新窗格 → 复制路径/用所选建文件夹/固定 → 终端 → **显示更多选项**（全部 Shell 扩展，含嵌套子菜单）
- **Shift** → `CMF_EXTENDEDVERBS`
- **设置** → Shell 扩展仅出现在「显示更多选项」
- **Shell 行图标**：`PopupMenuItem::icon_png`（CyberFiles fork `popup_menu`，彩色 PNG）
- **选中预取**：左键改选时后台 `query_shell_context_menu_items`，减轻首次右键「正在加载」
- **Shell 缓存热更新**：菜单打开期间预取完成会重建 `PopupMenu`（`shell_menu_revision`）
- **Open with 子菜单**：缓存命中或冷启动 `query_shell_context_menu_items` 提取 `openas`；底部「选择其他应用…」
- **Send to 子菜单**：同上提取 `sendto` /「发送到」；无子项时占位
- **压缩到 ZIP**：右键 / `CompressItems`（Windows `Compress-Archive`）

### 阶段 C（进行中）

- **设置 → 右键菜单**：开关内置项（压缩、发送到、固定、终端、文件标签、快捷方式）+ Shell 子菜单模式（`settings.json` / `ContextMenuItemPrefs`）
- **ShelfPane**：状态栏上方暂存条（复制/剪切计数、首项预览、粘贴、清空）
- 待做：与 `commands` 注册表统一生成、分享项、毛玻璃样式、`IContextMenu2`

## 当前与 `../Files` 的差距

下面这一组差距是 2026-05-26 对照 `../Files/src/Files.App/Data/Factories/ContentPageContextFlyoutFactory.cs` 后确认的，属于“行为还没一比一”的剩余项。

### 已对齐到位

- 右键点到**已选中的项**时，保持当前多选，不退化成单选
- `Send to` 已支持多选
- `Create shortcut` 已支持多选
- `Open in terminal` 已收紧为“仅在当前选择全部为文件夹时显示”
- 单选限定项已基本收紧到 `Open with`、`Open in new tab/window/pane`、`Open file location`

### 仍未对齐

1. **主动作分组结构还不够像 Files**
   `Files` 的 item 菜单会稳定包含 `Rename`、`Delete`、`Properties`、`Share` 等主动作，并按固定分组排布。CyberFiles 目前更偏向“自定义项 + Shell 扩展”的结构，主动作层次还不够稳定。

2. **背景菜单和项菜单还没作为一整套统一复刻**
   `Files` 会严格区分：
   - 空白区域右键：当前目录菜单
   - 选中项右键：选中项菜单
   - 是否有选择、是否全为文件夹，会共同决定菜单项显隐
   CyberFiles 的 item menu 已部分收敛，但 background menu 还没一起完全对齐。

3. **压缩/解压规则不完整**
   `Files` 会同时区分：
   - 可压缩选择：显示 `Compress`
   - 可解压选择：显示 `Extract`
   CyberFiles 当前主要只有压缩入口，`Extract` 及其类型判断还没完整复刻。

4. **`Open in terminal` 体验只对齐了一半**
   动作层现在已经支持多目录，但还没把“空白区域右键当前目录”和“item 右键多个已选中文件夹”统一成 `Files` 的完整终端行为模型。

5. **`Create shortcut` 规则还不是完整同构**
   现在只补到了多选和 `.lnk` 基础过滤；`Files` 还会结合页面类型、回收站等条件控制显示。CyberFiles 目前仍是常见场景对齐，不是完整规则复制。

6. **`Send to` 的组织方式还不是 Files 那套**
   `Files` 使用 `SendTo` / `SendToOverflow` 占位和延迟填充。CyberFiles 现在是直接提取 Shell submenu，结果能用，但结构还不是一比一。

7. **Shell 扩展的 overflow / loading 组织还没完全像 Files**
   CyberFiles 已支持内联或“显示更多选项”，也有缓存与热更新，但 `Files` 对 overflow、占位、异步填充顺序的组织更固定，当前还没完全复刻。

## 后续对齐顺序

1. 先统一 background menu 和 item menu 的结构与显示条件
2. 再补齐 `Compress / Extract / Share / Rename / Delete / Properties` 这一组主动作排布
3. 最后收紧 `Send to`、Shell overflow、终端行为这些“能用但还不完全像 Files”的部分

## 代码落点

| 层 | 路径 |
|----|------|
| 文档 | 本文档、`files-parity-roadmap.md`、`files-rust-port-plan.md` |
| Shell COM | `crates/platform-windows/src/context_menu.rs` |
| UI | `crates/ui/src/file_browser/context_menu.rs`；`crates/ui/src/popup_menu/`（fork）；`file_browser.rs` 预取 Shell |
| 命令 | `crates/commands` — 文件操作；Shell 项 invoke 用带索引的本地 action |

## 验收

- 右键出现 **CyberFiles 主题** 菜单，而非 Explorer 灰框系统菜单
- 菜单含 Files 核心项；本地路径下可见部分 Shell 扩展（7-Zip、发送到等）
- 与 `../Files` 侧栏/布局行为一致优先于与 Explorer 一致
