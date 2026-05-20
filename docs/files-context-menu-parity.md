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

### 阶段 A（当前）

1. 文件列表右键 → **GPUI `ContextMenu`**（`ContextMenuExt`）
2. 菜单结构对齐 Files **首轮**：打开、重命名、复制、剪切、粘贴、删除、属性
3. **禁止**在 UI 线程 / `browser.update` 内同步调用 `query_shell_context_menu_items`（会卡死；Files 用带消息泵的专用线程）
4. `platform-windows` 保留 `query_*` / `invoke_*` 供后台线程合并；菜单构建只用 `browser.read` + `item_context_menu`

### 阶段 B

- 空白区域 / 当前文件夹 `BaseContextMenuFlyout` 等价物
- Shift → `CMF_EXTENDEDVERBS`；设置项「将 Shell 扩展移入子菜单」
- 子菜单、图标、`IContextMenu2` 消息泵
- 回收站 / ZIP / FTP 页禁用 Shell 合并（同 Files）

### 阶段 C

- 与 `commands` 注册表统一生成；可配置显示项（对齐 Files 设置 → Context menu）

## 代码落点

| 层 | 路径 |
|----|------|
| 文档 | 本文档、`files-parity-roadmap.md`、`files-rust-port-plan.md` |
| Shell COM | `crates/platform-windows/src/context_menu.rs` |
| UI | `crates/ui/src/file_browser.rs` — `populate_item_context_menu` |
| 命令 | `crates/commands` — 文件操作；Shell 项 invoke 用带索引的本地 action |

## 验收

- 右键出现 **CyberFiles 主题** 菜单，而非 Explorer 灰框系统菜单
- 菜单含 Files 核心项；本地路径下可见部分 Shell 扩展（7-Zip、发送到等）
- 与 `../Files` 侧栏/布局行为一致优先于与 Explorer 一致
