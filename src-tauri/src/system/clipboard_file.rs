#[cfg(windows)]
pub fn copy_audio_file(path: &std::path::Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    // GlobalFree lives in Foundation, not System::Memory, in windows 0.58.
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HWND};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW,
        SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::UI::Shell::{DROPFILES, FD_FILESIZE, FILEDESCRIPTORW, FILEGROUPDESCRIPTORW};
    use windows::core::w;

    let wide_path: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .chain(std::iter::once(0))
        .collect();

    let dropfiles = DROPFILES {
        pFiles: std::mem::size_of::<DROPFILES>() as u32,
        pt: windows::Win32::Foundation::POINT { x: 0, y: 0 },
        fNC: windows::Win32::Foundation::BOOL(0),
        fWide: windows::Win32::Foundation::BOOL(1),
    };

    let total_size = std::mem::size_of::<DROPFILES>() + (wide_path.len() * 2);

    unsafe {
        // --- Build CF_HDROP HGLOBAL ---
        let h_drop = GlobalAlloc(GMEM_MOVEABLE, total_size).map_err(|e| e.to_string())?;
        let ptr = GlobalLock(h_drop);
        if ptr.is_null() {
            let _ = GlobalFree(h_drop);
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
        let _ = GlobalUnlock(h_drop);

        // --- Build CFSTR_FILEDESCRIPTORW HGLOBAL ---
        let file_name = path
            .file_name()
            .unwrap_or(path.as_os_str())
            .encode_wide()
            .collect::<Vec<u16>>();
        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
        let file_len = meta.len();

        let mut fd = FILEDESCRIPTORW::default();
        fd.dwFlags = FD_FILESIZE.0 as u32;
        fd.nFileSizeLow = file_len as u32;
        fd.nFileSizeHigh = (file_len >> 32) as u32;
        let name_copy_len = file_name.len().min(259);
        for i in 0..name_copy_len {
            fd.cFileName[i] = file_name[i];
        }
        // cFileName is already zero-init from Default, so null terminator is in place.

        let fgd_size = std::mem::size_of::<FILEGROUPDESCRIPTORW>();
        let h_fgd = GlobalAlloc(GMEM_MOVEABLE, fgd_size).map_err(|e| e.to_string())?;
        let fgd_ptr = GlobalLock(h_fgd);
        if fgd_ptr.is_null() {
            let _ = GlobalFree(h_fgd);
            let _ = GlobalFree(h_drop);
            return Err("Failed to lock global memory".into());
        }
        let fgd = &mut *(fgd_ptr as *mut FILEGROUPDESCRIPTORW);
        fgd.cItems = 1;
        fgd.fgd[0] = fd;
        let _ = GlobalUnlock(h_fgd);

        // --- Build CFSTR_FILECONTENTS HGLOBAL (raw bytes) ---
        // Electron/Chromium apps (e.g. WhatsApp Desktop) ignore CF_HDROP and only
        // read the FileGroupDescriptorW + FileContents virtual-file pair.
        let file_bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let h_contents =
            GlobalAlloc(GMEM_MOVEABLE, file_bytes.len()).map_err(|e| e.to_string())?;
        let contents_ptr = GlobalLock(h_contents);
        if contents_ptr.is_null() {
            let _ = GlobalFree(h_contents);
            let _ = GlobalFree(h_fgd);
            let _ = GlobalFree(h_drop);
            return Err("Failed to lock global memory".into());
        }
        std::ptr::copy_nonoverlapping(file_bytes.as_ptr(), contents_ptr as *mut u8, file_bytes.len());
        let _ = GlobalUnlock(h_contents);

        // --- Build CF_UNICODETEXT HGLOBAL (file path as fallback) ---
        let path_wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let text_size = path_wide.len() * 2;
        let h_text = GlobalAlloc(GMEM_MOVEABLE, text_size).map_err(|e| e.to_string())?;
        let text_ptr = GlobalLock(h_text);
        if text_ptr.is_null() {
            let _ = GlobalFree(h_text);
            let _ = GlobalFree(h_contents);
            let _ = GlobalFree(h_fgd);
            let _ = GlobalFree(h_drop);
            return Err("Failed to lock global memory".into());
        }
        std::ptr::copy_nonoverlapping(path_wide.as_ptr() as *const u8, text_ptr as *mut u8, text_size);
        let _ = GlobalUnlock(h_text);

        // --- Register custom clipboard format IDs ---
        let cf_descriptor = RegisterClipboardFormatW(w!("FileGroupDescriptorW"));
        let cf_contents = RegisterClipboardFormatW(w!("FileContents"));

        // --- Single clipboard session: set all formats ---
        // A handle SetClipboardData accepted is owned by the clipboard and must
        // never be freed here; only unaccepted ones are ours to release.
        let mut owned = vec![h_drop, h_fgd, h_contents, h_text];
        let mut hdrop_ok = false;

        if OpenClipboard(HWND::default()).is_ok() {
            if EmptyClipboard().is_ok() {
                for (format, handle) in [
                    (15u32, h_drop),
                    (cf_descriptor, h_fgd),
                    (cf_contents, h_contents),
                    (13u32, h_text),
                ] {
                    if SetClipboardData(format, HANDLE(handle.0)).is_ok() {
                        owned.retain(|h| h.0 != handle.0);
                        if format == 15 {
                            hdrop_ok = true;
                        }
                    }
                }
            }
            let _ = CloseClipboard();
        }

        for h in owned {
            let _ = GlobalFree(h);
        }

        if hdrop_ok {
            Ok(())
        } else {
            Err("Failed to set clipboard data".into())
        }
    }
}

#[cfg(not(windows))]
pub fn copy_audio_file(_path: &std::path::Path) -> Result<(), String> {
    Err("Audio file copying is only supported on Windows".into())
}
