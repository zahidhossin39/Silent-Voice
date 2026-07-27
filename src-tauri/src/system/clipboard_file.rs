#[cfg(windows)]
pub fn copy_audio_file(path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    // GlobalFree lives in Foundation, not System::Memory, in windows 0.58.
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HWND};
    use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::UI::Shell::DROPFILES;

    let wide_path: Vec<u16> = path.as_os_str().encode_wide().chain(std::iter::once(0)).chain(std::iter::once(0)).collect();

    let dropfiles = DROPFILES {
        pFiles: std::mem::size_of::<DROPFILES>() as u32,
        pt: windows::Win32::Foundation::POINT { x: 0, y: 0 },
        fNC: windows::Win32::Foundation::BOOL(0),
        fWide: windows::Win32::Foundation::BOOL(1),
    };

    let total_size = std::mem::size_of::<DROPFILES>() + (wide_path.len() * 2);

    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, total_size).map_err(|e| e.to_string())?;
        
        let ptr = GlobalLock(handle);
        if ptr.is_null() {
            let _ = GlobalFree(handle);
            return Err("Failed to lock global memory".into());
        }

        std::ptr::copy_nonoverlapping(
            &dropfiles as *const DROPFILES as *const u8,
            ptr as *mut u8,
            std::mem::size_of::<DROPFILES>(),
        );

        std::ptr::copy_nonoverlapping(
            wide_path.as_ptr() as *const u8,
            (ptr as *mut u8).add(std::mem::size_of::<DROPFILES>()),
            wide_path.len() * 2,
        );

        let _ = GlobalUnlock(handle);

        if OpenClipboard(HWND::default()).is_ok() {
            if EmptyClipboard().is_ok() {
                if SetClipboardData(15, HANDLE(handle.0)).is_ok() {
                    let _ = CloseClipboard();
                    return Ok(()); // Success - do not free memory
                }
            }
            let _ = CloseClipboard();
        }
        
        let _ = GlobalFree(handle);
        Err("Failed to set clipboard data".into())
    }
}

#[cfg(not(windows))]
pub fn copy_audio_file(_path: &std::path::Path) -> Result<(), String> {
    Err("Audio file copying is only supported on Windows".into())
}
