# Home 页 vs Files — 拆分与复刻进度

对照 `Files.App/Views/HomePage.xaml` + `HomeViewModel` + `UserControls/Widgets/*`。  
CyberFiles 实现：`crates/ui/src/home/`（`page.rs`、`widgets.rs`、`widget_shell.rs`）。

路线图索引：[`files-parity-roadmap.md`](files-parity-roadmap.md) §C。

---

## 实施拆分（PR / 阶段）

| 阶段 | 内容 | 状态 |
|------|------|------|
| **H0 子系统** | Widget 注册、`HomeSnapshot` 异步加载、`HomeWidgetPrefs` 持久化 | ✅ |
| **H1 布局** | 可折叠区块头、卡片网格（QA 小卡 / 驱动器大卡）、`Progress` 容量条 | ✅ |
| **H2 数据** | 卷标 + 容量（`platform-windows::volume`）、空可移动盘隐藏 | ✅ |
| **H3 交互** | 左键打开、Ctrl+新标签、右键 Pin/属性、标签预览容器 | ✅ |
| **H4 可配置** | 设置页开关 widget、页面右键切换显示、展开状态写入 `settings.json` | ✅ |
| **H5 深度** | Shell 缩略图、QA FileSystemWatcher、`pintohome` 同步、Eject、Storage Sense | ✅ |
| **H6 打磨** | Recent InfoBar、Storage Sense、Shell 子菜单设置、widget 顺序（标题右键上移/下移） | ✅ |

---

## 能力对照

| 能力 | Files | CyberFiles | 状态 |
|------|--------|------------|------|
| Widget 列表 | 5 个可开关 Expander | 5 个可折叠区块 + 设置/右键开关 | ✅ |
| Quick Access | 卡片 + 缩略图 + Pin 角标 | 小卡片 + Shell 图标 + 星标 | 🟡 |
| Drives | 容量条 + 卷标 + Storage Sense | 容量条 + 卷标 | 🟡 |
| Network | 驱动器式卡片 | 同 Drives 卡片（无容量） | 🟡 |
| File tags | 每标签容器 + 文件网格预览 | 容器 + 最多 8 项列表 | 🟡 |
| Recent | 图标 + 名 + 路径 | 图标 + 名 + 路径列 | 🟡 |
| 异步刷新 | `ReloadWidgetsCommand` | `HomeSnapshot` 后台加载 | ✅ |
| 页面右键 Widgets 菜单 | 有 | 有（✓ 标记） | ✅ |

**图例：** ✅ 对齐 · 🟡 部分 · ⬜ 未做

---

## 配置项（`settings.json`）

- `show_home_quick_access` / `show_home_drives` / `show_home_network` / `show_home_file_tags` / `show_home_recent`
- `home_*_expanded` — 各区块折叠状态
- `home_widget_order` — 区块显示顺序（`quick_access`、`drives`、`network`、`file_tags`、`recent`）

设置页：**Settings → Home**。

---

## 建议下一批（H5）

1. 驱动器/文件夹 **Shell 缩略图**（异步 `SIIGBF_THUMBNAILONLY`，无缩略图时回退图标）✅
2. Quick Access **AutomaticDestinations** 监听刷新 ✅
3. **Eject** / 断开网络盘（驱动器/网络卡片右键）✅
4. Explorer **pintohome** 固定/取消固定同步（Pin 时调用 Shell `pintohome`）✅

## H6（已完成）

1. Recent 隐私关闭时显示 **InfoBar** ✅
2. 驱动器右键 **Storage Sense**（打开系统设置）✅
3. 设置「Shell 扩展子菜单」开关接入右键菜单 ✅
4. widget 重排：`home_widget_order` + 区块标题右键 **上移 / 下移** ✅

### H6 明确不做（产品决策）

与 Files 对照后**刻意不做**，避免重复排期：

| Files 能力 | CyberFiles 替代 | 原因 |
|------------|-----------------|------|
| Home widget **拖放重排** | 标题右键上移/下移（持久化 `home_widget_order`） | GPUI 拖放成本高；菜单重排已满足排序需求 |
| Expander **折叠/展开动画** | 即时显示/隐藏内容 | 优先级低；GPUI 动画投入产出比不足 |

---

## 建议下一批（Home 打磨之外）

Home H0–H6 主线已收口；后续 Home 仅做体验抛光（缩略图质量、卡片密度等），见 [`files-parity-roadmap.md`](files-parity-roadmap.md) 第二梯队。
