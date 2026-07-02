use serde::Serialize;
use sysinfo::{Disks, System};

#[derive(Serialize, Clone, Debug)]
pub struct HardwareInfo {
    pub cpu_brand: String,
    pub physical_cores: usize,
    pub logical_cores: usize,
    pub total_ram_gb: f64,
    pub available_ram_gb: f64,
    pub has_avx2: bool,
    pub has_avx512: bool,
    pub gpu_vendor: Option<String>,
    pub gpu_name: Option<String>,
    pub gpu_vram_gb: Option<f64>,
    pub free_disk_gb: f64,
    pub os: String,
}

fn detect_avx2() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return std::arch::is_x86_feature_detected!("avx2");
    }
    #[allow(unreachable_code)]
    false
}

fn detect_avx512() -> bool {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        return std::arch::is_x86_feature_detected!("avx512f");
    }
    #[allow(unreachable_code)]
    false
}

const BYTES_PER_GB: f64 = 1024.0 * 1024.0 * 1024.0;

pub fn detect() -> HardwareInfo {
    let mut sys = System::new_all();
    sys.refresh_all();

    let cpu_brand = sys
        .cpus()
        .first()
        .map(|c| c.brand().trim().to_string())
        .unwrap_or_else(|| "Unknown CPU".to_string());

    let physical_cores = sys.physical_core_count().unwrap_or_else(|| sys.cpus().len());
    let logical_cores = sys.cpus().len();

    let total_ram_gb = sys.total_memory() as f64 / BYTES_PER_GB;
    let available_ram_gb = sys.available_memory() as f64 / BYTES_PER_GB;

    // Free space on the disk holding the executable (fallback: largest disk).
    let disks = Disks::new_with_refreshed_list();
    let free_disk_gb = disks
        .iter()
        .map(|d| d.available_space())
        .max()
        .unwrap_or(0) as f64
        / BYTES_PER_GB;

    let (gpu_vendor, gpu_name, gpu_vram_gb) = detect_gpu();

    HardwareInfo {
        cpu_brand,
        physical_cores,
        logical_cores,
        total_ram_gb,
        available_ram_gb,
        has_avx2: detect_avx2(),
        has_avx512: detect_avx512(),
        gpu_vendor,
        gpu_name,
        gpu_vram_gb,
        free_disk_gb,
        os: format!(
            "{} {}",
            System::name().unwrap_or_else(|| "OS".into()),
            System::os_version().unwrap_or_default()
        ),
    }
}

#[cfg(windows)]
fn detect_gpu() -> (Option<String>, Option<String>, Option<f64>) {
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIAdapter1, IDXGIFactory1,
    };

    unsafe {
        let factory: IDXGIFactory1 = match CreateDXGIFactory1() {
            Ok(f) => f,
            Err(_) => return (None, None, None),
        };

        let mut best: Option<(String, f64)> = None;
        let mut i = 0u32;
        while let Ok(adapter) = factory.EnumAdapters1(i) {
            let adapter: IDXGIAdapter1 = adapter;
            if let Ok(desc) = adapter.GetDesc1() {
                let name = String::from_utf16_lossy(&desc.Description)
                    .trim_end_matches('\u{0}')
                    .trim()
                    .to_string();
                let vram_gb = desc.DedicatedVideoMemory as f64 / BYTES_PER_GB;
                // Prefer the adapter with the most dedicated VRAM.
                if best.as_ref().map(|(_, v)| vram_gb > *v).unwrap_or(true) {
                    best = Some((name, vram_gb));
                }
            }
            i += 1;
        }

        match best {
            Some((name, vram)) => {
                let vendor = if name.to_lowercase().contains("nvidia") {
                    "NVIDIA"
                } else if name.to_lowercase().contains("amd")
                    || name.to_lowercase().contains("radeon")
                {
                    "AMD"
                } else if name.to_lowercase().contains("intel") {
                    "Intel"
                } else {
                    "Unknown"
                };
                (Some(vendor.to_string()), Some(name), Some(vram))
            }
            None => (None, None, None),
        }
    }
}

#[cfg(not(windows))]
fn detect_gpu() -> (Option<String>, Option<String>, Option<f64>) {
    (None, None, None)
}
