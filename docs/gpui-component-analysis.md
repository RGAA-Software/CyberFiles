# GPUI Component 项目分析文档

> 分析来源：Git 依赖 [longbridge/gpui-component](https://github.com/longbridge/gpui-component)（版本以 `Cargo.lock` 为准，约 **0.5.x**）  
> 用途：在 CyberFiles 等项目中选用控件、查阅 API 与示例时的参考手册。  
> 路径约定：下文 `gpui-component/...` 指**单独 clone 的上游仓库**根目录，不在 CyberFiles 仓库内。  
> 官方文档：<https://longbridge.github.io/gpui-component/> | <https://docs.rs/gpui-component>

---

## 1. 项目概览

**GPUI Component** 是基于 [Zed GPUI](https://gpui.rs) 的跨平台桌面 UI 组件库，设计风格参考 shadcn/ui，控件形态参考 macOS / Windows 原生控件。

| 维度 | 说明 |
|------|------|
| 组件数量 | 60+ UI 组件（`lib.rs` 导出 **54** 个 `pub mod`） |
| 编程模型 | 多数为无状态 `RenderOnce`；部分复杂控件为 `Render` + `Entity<State>` |
| 主题 | JSON 驱动 `Theme` / `ThemeColor`，亮/暗模式 |
| 尺寸 | `xs` / `sm` / `md` / `lg`（`Sizable` trait） |
| 高性能 | `VirtualList`、`DataTable` 虚拟化 |
| 编辑器 | 代码编辑器 + Tree-sitter 高亮 + LSP（补全、诊断、悬停等） |
| 图表 | `plot` 底层 + `chart` 高层封装 |
| 布局 | `Dock`（IDE 式面板）、`Resizable`（可拖拽分栏） |

### 1.1 Workspace 结构

| 路径 | Crate | 作用 |
|------|-------|------|
| `gpui-component/crates/ui` | `gpui-component` | **主 UI 库** |
| `gpui-component/crates/macros` | `gpui-component-macros` | 过程宏：`icon_named!`、`IntoPlot` |
| `gpui-component/crates/assets` | `gpui-component-assets` | 内置 99 个 SVG 图标 + `AssetSource` |
| `gpui-component/crates/story` | `story` | 组件画廊（交互演示，等同官方 Gallery） |
| `gpui-component/crates/story-web` | — | Gallery 的 WASM 构建 |
| `gpui-component/crates/webview` | — | 实验性 `wry` WebView 嵌入 |
| `gpui-component/examples/*` | 11 个示例 | 单功能最小示例 |
| `gpui-component/docs/` | VitePress | 中英文站点与组件文档 |

---

## 2. 集成与初始化

### 2.1 依赖（Cargo.toml）

CyberFiles 通过 **Git** 使用 gpui-component（硬性要求见 [`dependency-policy.md`](dependency-policy.md)）：

```toml
gpui = { git = "https://github.com/zed-industries/zed" }
gpui_platform = { git = "https://github.com/zed-industries/zed", features = ["font-kit"] }
gpui-component = { git = "https://github.com/longbridge/gpui-component" }
gpui-component-assets = { git = "https://github.com/longbridge/gpui-component", package = "gpui-component-assets" }
anyhow = "1.0"
```

上游没有的能力在 **`crates/ui`** 实现（侧栏拖放、面包屑异步菜单等），见 [`dependency-policy.md`](dependency-policy.md)。

### 2.2 应用入口

```rust
fn main() {
  // 使用内置图标时：
  // gpui_platform::application().with_assets(gpui_component_assets::Assets).run(...)

  gpui_platform::application().run(move |cx| {
    gpui_component::init(cx); // 必须在 app.run 内最先调用

    let window_options = WindowOptions {
      window_bounds: Some(WindowBounds::centered(size(px(1280.), px(720.)), cx)),
      ..Default::default()
    };

    cx.spawn(async move |cx| {
      cx.open_window(window_options, |window, cx| {
        let view = cx.new(|_| MyView);
        cx.new(|cx| Root::new(view, window, cx)) // 窗口第一层必须是 Root
      }).expect("...");
    }).detach();
  });
}
```

**注意：** `WindowBounds::centered` / `Bounds::centered` 需要 `&App`，应在 `app.run` 同步闭包内构造 `WindowOptions`，再 `move` 进 `cx.spawn`。

### 2.3 `gpui_component::init` 注册的子系统

`theme`、`global_state`、`inspector`（debug）、`root`、`focus_trap`、`color_picker`、`date_picker`、`dock`、`sheet`、`combobox`、`select`、`input`、`list`、`dialog`、`popover`、`menu`、`table`、`text`、`tree`、`tooltip`。

### 2.4 国际化

- 文案：`locales/ui.yml`（`rust_i18n`）
- API：`gpui_component::locale()`、`gpui_component::set_locale("zh-CN")`

---

## 3. 核心概念

### 3.1 无状态 vs 有状态组件

| 类型 | 实现 | 典型控件 | 用法 |
|------|------|----------|------|
| 无状态 | `RenderOnce` / `IntoElement` | `Button`、`Tag`、`Alert` | 直接在 `render` 里 `.child(Button::new(...))` |
| 有状态 | `Render` + `Entity<State>` | `Input`、`Select`、`List`、`DataTable` | 在 View 结构体中持有 `Entity<InputState>` 等 |

有状态示例模式：

```rust
struct MyView {
  input: Entity<InputState>,
}

impl MyView {
  fn new(window: &Window, cx: &mut Context<Self>) -> Self {
    let input = cx.new(|cx| InputState::new(window, cx).placeholder("..."));
    Self { input }
  }
}

impl Render for MyView {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    self.input.clone() // Input 绑定到 Entity
  }
}
```

### 3.2 `Root`（窗口根视图）

- 每个窗口内容的最外层必须是 `Root::new(view, window, cx)`。
- 统一管理：**Sheet**、**Dialog**、**Notification**、**Tooltip** 叠加层与 Tab 焦点路由。
- 可自定义背景：`Root::new(...).bg(cx.theme().background)`。

### 3.3 `WindowExt`（窗口级 API）

对 `Window` 扩展，通过 `Root` 调度：

| 方法 | 说明 |
|------|------|
| `open_sheet` / `open_sheet_at` | 侧边抽屉（默认右侧） |
| `has_active_sheet` / `close_sheet` | Sheet 状态 |
| `open_dialog` | 自定义 Dialog |
| `open_alert_dialog` | 预设样式的警告框 |
| `has_active_dialog` / `close_dialog` / `close_all_dialogs` | Dialog 状态 |
| `push_notification` / `remove_notification` / `clear_notifications` | Toast 通知 |
| `focused_input` / `has_focused_input` | 当前聚焦的输入框 |

### 3.4 主题与样式工具

| API | 说明 |
|-----|------|
| `ActiveTheme` | `cx.theme()` 获取 `Theme` |
| `ThemeMode` | Light / Dark |
| `ThemeColor` | 语义色 token（primary、background、foreground…） |
| `ThemeRegistry` | 加载 JSON 主题 |
| `Colorize` | `mix_oklab` 等颜色混合 |
| `StyledExt` | 组件链式样式扩展 |
| `Sizable` | `.small()` / `.medium()` / `.large()` / `.xsmall()` |
| `Disableable` | `.disabled(true)` |
| `h_flex()` / `v_flex()` | 布局快捷构造 |

### 3.5 图标

- 内置 **99** 个 Lucide 风格 SVG（`gpui-component-assets`）。
- `Icon::new(IconName::Search)`、`.small()`、`.path("icons/foo.svg")`。
- 自定义：`icon_named!` 宏扫描目录生成枚举；或实现 `AssetSource`（见 `gpui-component/examples/app_assets`）。

### 3.6 其他基础设施

| 模块 | 说明 |
|------|------|
| `TitleBar` | 自定义标题栏 |
| `WindowBorder` | 窗口边框/阴影/内边距 |
| `VirtualList` | 虚拟列表，`v_virtual_list` / `h_virtual_list` |
| `FocusTrapElement` | 焦点陷阱（模态、Sheet 内 Tab 循环） |
| `Placement` / `Side` | 浮层定位（Popover、Tooltip、Sheet） |
| `measure` / `measure_if` | 性能测量（`GPUI_MEASUREMENTS=1`） |

---

## 4. 组件全览（按分类）

下列 **54** 个模块均从 `gpui_component` crate 根导出。文档站部分页面（如 `editor`、`data-table`）对应同一模块内的子能力。

### 4.1 基础组件（Basic）

| 模块 | 主要类型 | 功能简述 |
|------|----------|----------|
| `accordion` | `Accordion`, `AccordionItem` | 手风琴折叠面板 |
| `alert` | `Alert`, `AlertVariant` | 行内提示条（info/warning/error 等） |
| `avatar` | `Avatar`, `AvatarGroup` | 头像与头像组 |
| `badge` | `Badge` | 数字/状态角标 |
| `breadcrumb` | `Breadcrumb`, `BreadcrumbItem` | 面包屑导航 |
| `button` | `Button`, `ButtonGroup`, `ButtonIcon`, `DropdownButton`, `Toggle`, `ToggleGroup` | 按钮、图标按钮、下拉按钮、切换组 |
| `checkbox` | `Checkbox` | 复选框 |
| `collapsible` | `Collapsible` | 可展开/收起区域 |
| `kbd` | `Kbd` | 快捷键展示 |
| `label` | `Label`, `HighlightsMatch` | 文本标签（支持高亮匹配） |
| `link` | `Link` | 超链接样式 |
| `pagination` | `Pagination` | 分页 |
| `progress` | `Progress`, `ProgressCircle` | 线性与环形进度 |
| `radio` | `Radio`, `RadioGroup` | 单选与单选组 |
| `rating` | `Rating` | 星级评分 |
| `skeleton` | `Skeleton` | 加载占位骨架屏 |
| `slider` | `Slider`, `SliderState`, `SliderEvent` | 滑块（单值/区间） |
| `spinner` | `Spinner` | 加载旋转指示 |
| `stepper` | `Stepper`, `StepperItem` | 分步向导 |
| `switch` | `Switch` | 开关 |
| `tag` | `Tag`, `TagVariant` | 标签/芯片 |
| `tooltip` | `Tooltip`, `TooltipOverlay` | 悬停提示 |

**Button 常用链式 API：**

- 变体 trait `ButtonVariants`：`.primary()` `.danger()` `.warning()` `.success()` `.ghost()` `.outline()`
- `ButtonRounded`、`.label()`、`.on_click()`
- `Toggle` / `ToggleGroup`：`ToggleVariants`
- `DropdownButton`：带下拉菜单的按钮

**文档中的 Image：** 无独立 `pub mod image`；图片通过 GPUI `img` 或 `text::TextView` 内嵌，Gallery 有 `image_story`。

---

### 4.2 表单组件（Form）

| 模块 | 主要类型 | 功能简述 |
|------|----------|----------|
| `input` | `Input`, `InputState`, `InputEvent`, `NumberInput`, `OtpInput`, `MaskPattern` | 文本框、数字框、OTP、掩码、**代码编辑器** |
| `select` | `Select<D>`, `SelectState<D>`, `SelectDelegate` | 单选下拉（可搜索） |
| `combobox` | `Combobox<D>`, `ComboboxState<D>`, `ComboboxEvent` | 可搜索单选/多选下拉 |
| `color_picker` | `ColorPicker`, `ColorPickerState`, `ColorPickerEvent` | 颜色选择器 |
| `form` | `Form`, `Field`, `v_form()`, `h_form()`, `field()` | 表单布局与字段 |
| `time::calendar` | `Calendar`, `CalendarState`, `CalendarEvent` | 日历网格 |
| `time::date_picker` | `DatePicker`, `DatePickerState`, `DateRangePreset` | 日期/范围选择 |

**Input 能力明细：**

| 能力 | 说明 |
|------|------|
| `InputMode` | `PlainText`、`AutoGrow`、`CodeEditor`（行号、折叠、高亮、诊断） |
| `InputEvent` | `Change`、`PressEnter`、`Focus`、`Blur` |
| LSP | `CompletionProvider`、`HoverProvider` 等 |
| 显示 | placeholder、prefix/suffix、disabled、loading、masked、validation、cleanup |
| 子模块 | `display_map`（坐标映射）、`lsp`、`popovers`、`element` |

**searchable_list（内部）：** `Select` / `Combobox` 共用的搜索列表基础（`SearchableListDelegate`、`SearchableVec`）。

---

### 4.3 布局与浮层（Layout & Overlay）

| 模块 | 主要类型 | 功能简述 |
|------|----------|----------|
| `description_list` | `DescriptionList`, `DescriptionItem` | 键值对/术语表布局 |
| `group_box` | `GroupBox`, `GroupBoxVariant` | 带边框分组容器 |
| `dialog` | `Dialog`, `AlertDialog`, `DialogHeader`, `DialogFooter`, `DialogContent`… | 模态对话框与警告框 |
| `sheet` | `Sheet`, `SheetSettings` | 边缘滑入抽屉 |
| `popover` | `Popover`, `PopoverState` | 锚定浮层 |
| `hover_card` | `HoverCard`, `HoverCardState` | 悬停卡片（富内容） |
| `notification` | `Notification`, `NotificationType`, `NotificationList` | Toast 通知栈 |
| `resizable` | `ResizablePanelGroup`, `ResizablePanel`, `h_resizable`, `v_resizable` | 可拖拽分割面板 |
| `scroll` | `Scrollable`, `Scrollbar`, `ScrollableMask` | 滚动区域与自定义滚动条 |
| `sidebar` | `Sidebar<E>`, `SidebarMenu`, `SidebarGroup`, `SidebarCollapsible` | 应用侧栏导航 |
| `separator` | `Separator`, `SeparatorStyle` | 分隔线 |
| `tab` | `Tab`, `TabBar`, `TabVariant` | 标签页 |

**Dialog 组合件：** `DialogTitle`、`DialogDescription`、`DialogHeader`、`DialogFooter`、`DialogClose`、`DialogAction`；`AlertDialog` 支持 `.warning()` 等变体。

---

### 4.4 数据展示（Data Display）

| 模块 | 主要类型 | 功能简述 |
|------|----------|----------|
| `list` | `List<D>`, `ListState<D>`, `ListDelegate`, `ListItem` | 委托驱动的虚拟列表 |
| `table` | `Table`*, `DataTable<D>`, `TableDelegate`, `Column`, `ColumnSort` | 静态表格 + **高性能 DataTable** |
| `tree` | `Tree`, `TreeState`, `TreeItem`, `tree()` | 树形结构 |
| `text` | `TextView`, `TextViewState`, `markdown()`, `html()` | Markdown / HTML 富文本 |
| `virtual_list` | `VirtualList`, `VirtualListScrollHandle` | 底层虚拟列表 |

**Table 两套 API：**

1. **语义表格：** `Table`, `TableHeader`, `TableBody`, `TableRow`, `TableCell`…（类似 HTML table 组合）
2. **DataTable：** `TableDelegate` 提供数据；`Column` 支持排序 `ColumnSort`、固定列 `ColumnFixed`；`TableEvent`、`TableVisibleRange`

---

### 4.5 图表（Chart & Plot）

| 模块 | 主要类型 | 功能简述 |
|------|----------|----------|
| `plot` | `Plot` trait, `ScaleLinear`, `ScaleBand`, `Line`, `Bar`, `Area`, `Pie`… | 绘图原语（轴、网格、形状） |
| `chart` | `LineChart`, `BarChart`, `AreaChart`, `PieChart`, `CandlestickChart` | 开箱即用图表组件 |

宏：`#[derive(IntoPlot)]`（`gpui-component-macros`）用于数据序列转换。

---

### 4.6 导航与菜单（Navigation & Menu）

| 模块 | 主要类型 | 功能简述 |
|------|----------|----------|
| `menu` | `AppMenuBar`, `ContextMenu`, `PopupMenu`, `DropdownMenu` | 菜单栏、右键菜单、弹出菜单 |

`ContextMenuExt` 为元素绑定右键菜单；`DropdownMenu` / `DropdownMenuPopover` 用于下拉。

---

### 4.7 高级布局（Advanced Layout）

| 模块 | 主要类型 | 功能简述 |
|------|----------|----------|
| `dock` | `Dock`, `DockArea`, `Panel`, `TabPanel`, `StackPanel`, `Tiles`, `PanelRegistry` | IDE 式 Dock：标签页、堆叠、瓦片自由布局 |
| `setting` | `Settings`, `SettingPage`, `SettingGroup`, `SettingField<T>` | 设置页 UI（bool/string/number/dropdown 字段） |

**Dock 核心概念：**

- `Panel` trait：自定义面板内容
- `register_panel` / `PanelRegistry`：注册面板类型
- `DockState` / `PanelState`：持久化布局状态
- `DockPlacement`、`PanelControl`：停靠位置与控制按钮

---

### 4.8 编辑与高亮（Editor & Highlighting）

| 模块 | 主要类型 | 功能简述 |
|------|----------|----------|
| `highlighter` | `SyntaxHighlighter`, `LanguageRegistry`, `Diagnostic`, `HighlightTheme` | Tree-sitter 语法高亮与诊断 |
| `history` | `History<I>`, `HistoryItem` | 输入撤销/重做栈 |

`input` 的 `CodeEditor` 模式与 `highlighter` 配合；Cargo feature `tree-sitter-languages` 启用多语言（Rust、TS、Python、Go…共 30+）。

---

### 4.9 其他 UI 模块

| 模块 | 主要类型 | 功能简述 |
|------|----------|----------|
| `animation` | `Transition`, `Lerp`, easing 函数 | 动画与缓动 |
| `clipboard` | `Clipboard` | 复制到剪贴板 UI |
| `theme` | `Theme`, `ThemeRegistry`, `ThemeColor`, `ColorName` | 主题系统（见 §3.4） |

---

## 5. 未单独导出但重要的能力

| 名称 | 位置 | 说明 |
|------|------|------|
| Icon | `icon.rs` | `Icon`, `IconName`, `IconNamed` |
| Root | `root.rs` | 窗口根（§3.2） |
| Focus Trap | `focus_trap.rs` | `.focus_trap()` |
| Searchable List | `searchable_list/` | Select/Combobox 底层 |
| Inspector | `inspector`（feature） | 调试组件检查器 |
| WebView | `gpui-component/crates/webview` | 实验性网页嵌入（`wry`） |

---

## 6. Cargo Features（`gpui-component`）

| Feature | 说明 |
|---------|------|
| `decimal` | 数字输入十进制支持（`rust_decimal`） |
| `inspector` | 组件检查器 |
| `tree-sitter-languages` | 一次性启用全部语法高亮语言 |
| `tree-sitter-rust` 等 | 按语言单独启用 |

---

## 7. 示例项目（`gpui-component/examples/`）

每个示例聚焦**单一功能**；综合演示请运行 **`gpui-component/crates/story`**（Gallery）。

| 示例 | 演示内容 |
|------|----------|
| `hello_world` | 最小应用：`init`、`Root`、`Button` |
| `input` | `InputState`、`InputEvent::Change`、订阅 |
| `window_title` | 自定义 `TitleBar` |
| `dialog_overlay` | `open_dialog`、`open_sheet`、右键菜单 |
| `sidebar` | 侧栏折叠模式、菜单、分组 |
| `focus_trap` | `focus_trap()` Tab 循环 |
| `tooltip_top_edge` | 贴顶窗口时 Tooltip 翻转 |
| `system_monitor` | 综合：`DataTable`、`AreaChart`、`TabBar`、`Progress`、主题 |
| `app_assets` | 自定义 `AssetSource` 与图标 |
| `webview` | 嵌入 WebView + 地址栏 Input |
| `color_mix_oklab` | 主题色 OKLab 混合 |

**运行 Gallery：**

```bash
cd gpui-component
cargo run -p story
```

**运行单个示例：**

```bash
cd gpui-component
cargo run --example hello_world
# 或进入 gpui-component/examples/hello_world 目录 cargo run
```

---

## 8. Story 画廊模块（`gpui-component/crates/story`）

与文档组件一一对应的交互演示（节选）：

`AccordionStory`, `AlertStory`, `AlertDialogStory`, `AvatarStory`, `BadgeStory`, `BreadcrumbStory`, `ButtonStory`, `CalendarStory`, `ChartStory`, `CheckboxStory`, `ClipboardStory`, `CollapsibleStory`, `ColorPickerStory`, `ComboboxStory`, `DataTableStory`, `DatePickerStory`, `DescriptionListStory`, `DialogStory`, `DropdownButtonStory`, `EditorStory`, `FormStory`, `GroupBoxStory`, `HoverCardStory`, `IconStory`, `ImageStory`, `InputStory`, `KbdStory`, `LabelStory`, `ListStory`, `MenuStory`, `NotificationStory`, `NumberInputStory`, `OtpInputStory`, `PaginationStory`, `PopoverStory`, `ProgressStory`, `RadioStory`, `RatingStory`, `ResizableStory`, `ScrollbarStory`, `SelectStory`, `SeparatorStory`, `SettingsStory`, `SheetStory`, `SidebarStory`, `SkeletonStory`, `SliderStory`, `SpinnerStory`, `StepperStory`, `SwitchStory`, `TableStory`, `TabsStory`, `TagStory`, `TextareaStory`, `ThemeColorsStory`, `ToggleStory`, `TooltipStory`, `TreeStory`, `VirtualListStory`, `WelcomeStory`

**查用法时建议：** 先在本表定位 Story 名 → 在 `gpui-component/crates/story/src/stories/*_story.rs` 阅读源码。

---

## 9. 官方文档目录（`gpui-component/docs/docs/`）

### 9.1 非组件文档

| 文档 | 主题 |
|------|------|
| `getting-started.md` | 入门 |
| `installation.md` | 安装 |
| `root.md` | Root 视图 |
| `theme.md` | 主题 |
| `assets.md` | 图标与资源 |
| `context.md` | GPUI Context 模式 |
| `element_id.md` | 元素 ID |

### 9.2 组件文档索引（59 页）

与 §4 分类一致，在线入口：`gpui-component/docs/docs/components/index.md`。

**Basic：** Accordion, Alert, Avatar, Badge, Button, Checkbox, Collapsible, DropdownButton, Icon, Image, Kbd, Label, Pagination, Progress, Radio, Rating, Skeleton, Slider, Spinner, Stepper, Switch, Tag, Toggle, Tooltip  

**Form：** Input, Select, Combobox, NumberInput, DatePicker, OtpInput, ColorPicker, Editor, Form  

**Layout：** DescriptionList, GroupBox, Dialog, Notification, Popover, Resizable, Scrollable, Sheet, Sidebar  

**Advanced：** Calendar, Chart, List, Menu, Settings, DataTable, Tabs, Tree, VirtualList  

另含专题页：`alert-dialog`, `data-table`, `editor`, `focus-trap`, `number-input`, `otp-input`, `plot`, `scrollable`, `settings`, `table`, `title-bar` 等。

---

## 10. 内置图标列表（99 个）

路径：`gpui-component/crates/assets/assets/icons/*.svg`（Lucide 风格）。

| 图标名 | | 图标名 | | 图标名 |
|--------|--|--------|--|--------|
| alert-circle | alert-triangle | arrow-down | arrow-left |
| arrow-right | arrow-up | bell | bot |
| calendar | check | chevron-down | chevron-left |
| chevron-right | chevron-up | chevrons-up-down | circle-check |
| circle-user | circle-x | close | copy |
| cpu | dash | delete | ellipsis |
| ellipsis-vertical | external-link | eye | eye-off |
| file | folder | folder-closed | folder-open |
| frame | gallery-vertical-end | github | globe |
| hard-drive | heart | heart-off | inbox |
| info | inspector | layout-dashboard | loader |
| loader-circle | map | maximize | memory-stick |
| menu | minimize | minus | moon |
| network | palette | panel-bottom | panel-bottom-open |
| panel-left | panel-left-close | panel-left-open | panel-right |
| panel-right-close | panel-right-open | pause | play |
| plus | redo | redo-2 | replace |
| resize-corner | search | settings | settings-2 |
| sort-ascending | sort-descending | square-terminal | star |
| star-fill | star-off | sun | thumbs-down |
| thumbs-up | triangle-alert | undo | undo-2 |
| user | window-close | window-maximize | window-minimize |
| window-restore | | | |

使用：`Icon::new(IconName::Search)`（需 `gpui-component-assets` 或自定义 assets）。

---

## 11. CyberFiles 项目选用速查

本仓库通过 **Git** 依赖 gpui-component，默认窗口 1366×768。

| 需求 | 推荐控件 | 参考 |
|------|----------|------|
| 主布局 + 侧栏 | `sidebar` + `resizable` / `dock` | `gpui-component/examples/sidebar`, `SidebarStory` |
| 文件列表 | `list` 或 `virtual_list` | `ListStory`, `VirtualListStory` |
| 文件表格 | `DataTable` | `DataTableStory`, `system_monitor` |
| 目录树 | `tree` | `TreeStory` |
| 工具栏按钮 | `button`, `ButtonIcon`, `Toggle` | `ButtonStory` |
| 搜索框 | `Input` + `InputState` | `gpui-component/examples/input` |
| 设置页 | `setting` | `SettingsStory` |
| 关于/确认框 | `open_alert_dialog` | `AlertDialogStory` |
| 详情面板 | `open_sheet` | `SheetStory` |
| 状态提示 | `push_notification` | `NotificationStory` |
| 主题切换 | `Theme`, `ThemeMode` | `ThemeColorsStory` |
| 预览 Markdown | `TextView` + `markdown()` | `TextareaStory` |
| 代码预览 | `InputState` CodeEditor + `highlighter` | `EditorStory` |

---

## 12. 源码索引（快速定位）

| 目标 | 路径 |
|------|------|
| 所有组件模块声明 | `gpui-component/crates/ui/src/lib.rs` |
| 组件实现 | `gpui-component/crates/ui/src/<module>/` |
| 窗口扩展 API | `gpui-component/crates/ui/src/window_ext.rs` |
| Root 实现 | `gpui-component/crates/ui/src/root.rs` |
| 主题 JSON | `gpui-component/crates/ui/src/theme/default-theme.json` |
| 交互演示 | `gpui-component/crates/story/src/stories/` |
| 最小示例 | `gpui-component/examples/*/src/main.rs` |
| 中英文文档站 | `gpui-component/docs/docs/`, `gpui-component/docs/zh-CN/` |

---

## 13. 版本与维护说明

- 本文档基于 `gpui-component` 源码梳理，crate 版本 **0.5.2**。
- 本仓库的 **gpui-component** 为 Git 依赖；**GPUI** 从 Zed Git 拉取。改组件或查 API 时 clone 上游仓库并执行 `cargo run -p story`。
- 更完整的交互行为以 **Story 源码** 与 **官方站点** 为准。

---

*文档生成目的：供 CyberFiles 及后续桌面功能开发时查阅 gpui-component 全部控件与系统能力。*
