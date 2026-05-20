use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

/// Ensures COM is initialized on the current thread (STA).
///
/// Does **not** call `CoUninitialize` — the UI thread keeps COM alive for the process lifetime.
pub fn ensure_com_apartment() -> anyhow::Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    }
    Ok(())
}
