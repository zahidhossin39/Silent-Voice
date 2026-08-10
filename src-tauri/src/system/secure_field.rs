// Best-effort detection of whether the currently focused control is a
// password / secure text field, so dictation is never auto-typed into one.
// Reuses the same UI Automation `IsPassword` property the inline-proofread
// watcher relies on (covers native Win32 edits AND web/Electron inputs).
//
// Runs on a throwaway thread so COM init/teardown never touches the paste
// thread. Fails OPEN: any COM/UIA failure returns false, so a normal paste is
// never blocked just because detection couldn't run.

#[cfg(windows)]
pub fn focused_is_password() -> bool {
    std::thread::spawn(|| unsafe {
        use windows::Win32::System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
        };
        use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation};

        if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
            return false;
        }
        let automation: IUIAutomation =
            match CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                Ok(a) => a,
                Err(_) => return false,
            };
        match automation.GetFocusedElement() {
            Ok(el) => el.CurrentIsPassword().map(|b| b.as_bool()).unwrap_or(false),
            Err(_) => false,
        }
    })
    .join()
    .unwrap_or(false)
}

#[cfg(not(windows))]
pub fn focused_is_password() -> bool {
    false
}
