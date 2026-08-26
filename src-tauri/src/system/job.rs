use std::process::Child;

#[cfg(windows)]
use std::sync::OnceLock;

#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

#[cfg(windows)]
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, SetInformationJobObject,
    JobObjectExtendedLimitInformation, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

#[cfg(windows)]
use windows::Win32::Foundation::{CloseHandle, HANDLE};

#[cfg(windows)]
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE,
};

#[cfg(windows)]
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

#[cfg(windows)]
static JOB_OBJECT: OnceLock<isize> = OnceLock::new();

#[cfg(windows)]
fn get_job_object() -> Result<HANDLE, String> {
    let handle_isize = JOB_OBJECT.get_or_init(|| unsafe {
        let job = CreateJobObjectW(None, None).unwrap_or_default();
        if job.is_invalid() {
            return job.0 as isize;
        }

        let mut limit_info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limit_info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

        let res = SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &limit_info as *const _ as *const std::ffi::c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );

        if res.is_err() {
            let _ = CloseHandle(job);
            return HANDLE::default().0 as isize;
        }

        job.0 as isize
    });

    let handle = HANDLE(*handle_isize as *mut std::ffi::c_void);
    if handle.is_invalid() {
        return Err("Failed to create or configure Job Object".into());
    }
    Ok(handle)
}

/// Assigns the given child process to a process-wide Job Object configured with
/// JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.
///
/// When the job handle closes — which the OS does automatically when our process dies
/// for any reason at all, including abort, panic, Task Manager, or power loss —
/// Windows terminates every process assigned to that job. No cleanup code path can be
/// missed because there is no cleanup code path.
#[cfg(windows)]
pub fn adopt(child: &Child) -> Result<(), String> {
    let job = get_job_object()?;
    let process_handle = HANDLE(child.as_raw_handle() as *mut _);

    unsafe {
        AssignProcessToJobObject(job, process_handle)
            .map_err(|e| format!("AssignProcessToJobObject failed: {e}"))?;
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn adopt(_child: &Child) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
pub fn reap_orphans() {
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0).unwrap_or_default();
        if snapshot.is_invalid() {
            return;
        }

        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };

        if Process32FirstW(snapshot, &mut entry).is_err() {
            let _ = CloseHandle(snapshot);
            return;
        }

        let exe = std::env::current_exe().unwrap_or_default();
        let app_dir = exe.parent().unwrap_or_else(|| std::path::Path::new(""));

        let whisper_path = app_dir.join("whisper-server.exe");
        let llama_path = app_dir.join("llama").join("llama-server.exe");

        let mut reaped_count = 0;

        loop {
            let exe_name = String::from_utf16_lossy(&entry.szExeFile);
            let exe_name = exe_name.trim_end_matches('\0');

            if exe_name == "whisper-server.exe" || exe_name == "llama-server.exe" {
                if let Ok(process) = OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_TERMINATE,
                    false,
                    entry.th32ProcessID,
                ) {
                        let mut path_buf = [0u16; 32768];
                        let mut size = path_buf.len() as u32;

                        if QueryFullProcessImageNameW(
                            process,
                            PROCESS_NAME_WIN32,
                            windows::core::PWSTR(path_buf.as_mut_ptr()),
                            &mut size,
                        )
                        .is_ok()
                        {
                            let path_str = String::from_utf16_lossy(&path_buf[..size as usize]);
                            let process_path = std::path::PathBuf::from(&path_str);

                            let mut is_match = false;

                            if process_path.exists() {
                                let c_process = std::fs::canonicalize(&process_path)
                                    .unwrap_or_else(|_| process_path.clone());
                                if exe_name == "whisper-server.exe" {
                                    let c_expected = std::fs::canonicalize(&whisper_path)
                                        .unwrap_or_else(|_| whisper_path.clone());
                                    if c_process == c_expected {
                                        is_match = true;
                                    }
                                } else if exe_name == "llama-server.exe" {
                                    let c_expected = std::fs::canonicalize(&llama_path)
                                        .unwrap_or_else(|_| llama_path.clone());
                                    if c_process == c_expected {
                                        is_match = true;
                                    }
                                }
                            }

                            if is_match {
                                if TerminateProcess(process, 1).is_ok() {
                                    reaped_count += 1;
                                }
                            }
                        }
                    let _ = CloseHandle(process);
                }
            }

            if Process32NextW(snapshot, &mut entry).is_err() {
                break;
            }
        }

        let _ = CloseHandle(snapshot);

        if reaped_count > 0 {
            crate::logging::log_info(
                "job",
                &format!("Reaped {} orphaned sidecar processes", reaped_count),
            );
        }
    }
}

#[cfg(unix)]
pub fn reap_orphans() {
    let exe = std::env::current_exe().unwrap_or_default();
    let app_dir = exe.parent().unwrap_or_else(|| std::path::Path::new(""));

    let whisper_path = app_dir.join("whisper-server");
    let llama_path = app_dir.join("llama").join("llama-server");

    let mut reaped_count = 0;

    // pkill -f matches against the full command line. By passing the absolute path,
    // we only kill processes we actually launched from our own install directory -
    // never a whisper-server the user is running from somewhere else.
    for path in [whisper_path, llama_path] {
        if let Some(path_str) = path.to_str() {
            if let Ok(status) = std::process::Command::new("pkill")
                .arg("-f")
                .arg(path_str)
                .status()
            {
                if status.success() {
                    reaped_count += 1;
                }
            }
        }
    }

    // pkill reports only whether it matched, not how many, so this counts
    // sidecars reaped rather than processes.
    if reaped_count > 0 {
        crate::logging::log_info(
            "job",
            &format!("Reaped {} orphaned sidecar(s)", reaped_count),
        );
    }
}

#[cfg(not(any(windows, unix)))]
pub fn reap_orphans() {}
