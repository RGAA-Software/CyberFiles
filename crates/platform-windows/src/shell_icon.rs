//! Windows Shell icons (colorful folder / drive / file icons like Files.app).

use std::collections::HashMap;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC,
    SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::UI::Shell::{
    IShellItem, IShellItemImageFactory, SHCreateItemFromParsingName, SIIGBF_ICONONLY,
    SIIGBF_SCALEUP,
};

use crate::com::ensure_com_apartment;
use crate::paths::{is_recycle_bin_path, SHELL_RECYCLE_BIN_PATH};

/// Maximum Shell icon dimension we request (matches Files `ShellIconSizes.Jumbo`).
const MAX_ICON_SIZE: u32 = 256;

static ICON_CACHE: Mutex<Option<HashMap<(PathBuf, u32), Vec<u8>>>> = Mutex::new(None);

/// Physical pixel size for a logical UI size at the given display scale (Files: `size * DPI`).
pub fn shell_icon_pixel_size(logical_px: f32, scale_factor: f32) -> u32 {
    let scaled = (logical_px * scale_factor).ceil();
    (scaled as u32).clamp(16, MAX_ICON_SIZE)
}

fn path_to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// PNG bytes for the Shell icon of `path` (same source as Files `FileThumbnailHelper`).
pub fn shell_icon_png(path: &Path, size: u32) -> anyhow::Result<Vec<u8>> {
    let size = size.max(16);
    let key = (path.to_path_buf(), size);
    let mut guard = ICON_CACHE.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
    let cache = guard.get_or_insert_with(HashMap::new);
    if let Some(bytes) = cache.get(&key) {
        return Ok(bytes.clone());
    }
    let png = shell_icon_png_uncached(path, size)?;
    cache.insert(key, png.clone());
    Ok(png)
}

/// PNG bytes sized for how the icon is drawn in the UI (logical px × scale factor).
pub fn shell_icon_png_scaled(
    path: &Path,
    logical_px: f32,
    scale_factor: f32,
) -> anyhow::Result<Vec<u8>> {
    shell_icon_png(path, shell_icon_pixel_size(logical_px, scale_factor))
}

fn shell_icon_png_uncached(path: &Path, size: u32) -> anyhow::Result<Vec<u8>> {
    ensure_com_apartment()?;
    unsafe { shell_icon_png_inner(path, size) }
}

/// Path string passed to `SHCreateItemFromParsingName` (Files uses `Shell:RecycleBinFolder` for the bin icon).
fn shell_icon_parsing_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.eq_ignore_ascii_case(SHELL_RECYCLE_BIN_PATH)
        || s.eq_ignore_ascii_case("recycle")
        || is_recycle_bin_path(path)
    {
        PathBuf::from(SHELL_RECYCLE_BIN_PATH)
    } else {
        path.to_path_buf()
    }
}

unsafe fn shell_icon_png_inner(path: &Path, size: u32) -> anyhow::Result<Vec<u8>> {
    let parsing = shell_icon_parsing_path(path);
    let wide = path_to_wide(&parsing);
    let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)?;
    let factory: IShellItemImageFactory = item.cast()?;
    let hbitmap = factory.GetImage(
        SIZE {
            cx: size as i32,
            cy: size as i32,
        },
        SIIGBF_ICONONLY | SIIGBF_SCALEUP,
    )?;
    hbitmap_to_png(hbitmap)
}

unsafe fn hbitmap_to_png(hbitmap: HBITMAP) -> anyhow::Result<Vec<u8>> {
    let mut bm = BITMAP::default();
    if GetObjectW(
        hbitmap,
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut _),
    ) == 0
    {
        anyhow::bail!("GetObjectW failed for shell icon bitmap");
    }

    let width = bm.bmWidth.unsigned_abs() as u32;
    let height = bm.bmHeight.unsigned_abs() as u32;
    if width == 0 || height == 0 {
        anyhow::bail!("shell icon bitmap has zero size");
    }

    let hdc_screen = GetDC(None);
    let hdc_mem = CreateCompatibleDC(hdc_screen);
    let _selected = SelectObject(hdc_mem, hbitmap);

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let stride = (width * 4) as usize;
    let mut pixels = vec![0u8; stride * height as usize];
    let lines = GetDIBits(
        hdc_mem,
        hbitmap,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );
    SelectObject(hdc_mem, _selected);
    let _ = DeleteDC(hdc_mem);
    let _ = ReleaseDC(None, hdc_screen);
    let _ = DeleteObject(hbitmap);

    if lines == 0 {
        anyhow::bail!("GetDIBits failed for shell icon");
    }

    let mut img = image::RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let i = (y as usize * stride) + (x as usize * 4);
            let b = pixels[i];
            let g = pixels[i + 1];
            let r = pixels[i + 2];
            let a = pixels[i + 3];
            img.put_pixel(x, y, image::Rgba([r, g, b, a]));
        }
    }

    let mut png = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png);
    img.write_to(&mut cursor, image::ImageFormat::Png)?;
    Ok(png)
}
