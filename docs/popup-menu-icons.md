# Popup Menu Icons

All popup menu / context menu icons use **Google Material Symbols Rounded** (24px, default weight) sourced from [Google Fonts](https://fonts.google.com/icons?icon.style=Rounded).

They are loaded via `Icon::new(IconName::X).path("icons/{filename}.svg")` so they render with the active theme foreground color (`currentColor`).

## Custom Material Icons (not in gpui-component Lucide set)

| Menu Item | Material Icon | SVG Filename | URL |
|-----------|--------------|--------------|-----|
| Cut | `content_cut` | `content_cut.svg` | `https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsrounded/content_cut/default/24px.svg` |
| Paste | `content_paste` | `content_paste.svg` | `https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsrounded/content_paste/default/24px.svg` |
| Open With | `widgets` | `widgets.svg` | `https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsrounded/widgets/default/24px.svg` |
| Compress | `folder_zip` | `folder_zip.svg` | `https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsrounded/folder_zip/default/24px.svg` |
| Add to Tag | `label` | `label.svg` | `https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsrounded/label/default/24px.svg` |
| Remove from Tag | `label_off` | `label_off.svg` | `https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsrounded/label_off/default/24px.svg` |
| Open in New Tab | `tab` | `tab.svg` | `https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsrounded/tab/default/24px.svg` |
| Open in New Window | `open_in_new` | `external-link.svg` | `https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsrounded/open_in_new/default/24px.svg` |
| Open in New Pane / Split Pane | `splitscreen` | `splitscreen.svg` | `https://fonts.gstatic.com/s/i/short-term/release/materialsymbolsrounded/splitscreen/default/24px.svg` |

## gpui-component Built-in Icons Used in Menus

| Menu Item | IconName |
|-----------|----------|
| Copy / Copy Path | `Copy` |
| Delete / Delete Permanent | `Delete` |
| Rename / Sort by Type / Compress fallback | `File` |
| New Folder / Create Folder from Selection | `Folder` |
| Open / Open File Location | `FolderOpen` |
| Create Shortcut / Send To | `ExternalLink` |
| Layout / Grid view | `LayoutDashboard` |
| Details view | `GalleryVerticalEnd` |
| Columns view | `PanelLeft` |
| Open in New Pane | `PanelLeftOpen` |
| Sort submenu | `ChevronsUpDown` |
| Sort by Name | `ALargeSmall` |
| Sort by Modified / Created | `Calendar` |
| Sort by Size | `HardDrive` |
| Toggle Direction | `ChevronsUpDown` |
| Show Hidden | `Eye` / `EyeOff` |
| New submenu | `Plus` |
| Open in New Tab | `File` → `tab.svg` |
| Open in New Window | `ExternalLink` → `external-link.svg` |
| Open in New Pane | `PanelLeftOpen` → `splitscreen.svg` |
| Properties | `Info` |
| Open With (fallback) | `Settings2` → `widgets.svg` |
| Open in Terminal | `SquareTerminal` |
| Pin | `Star` |
| Unpin | `StarOff` |
| Show More Options | `Ellipsis` |
| Refresh | `Replace` |
