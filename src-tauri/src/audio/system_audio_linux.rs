// Linux Others capture: the PulseAudio monitor of the default output, which
// PipeWire (pipewire-pulse) serves too. libpulse-simple is loaded at runtime
// so a system without it still starts the app (docs/adr/0001).

use super::system_audio::CaptureError;
use libloading::Library;
use std::any::Any;
use std::ffi::{c_char, c_int, c_void, CStr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

const RATE: u32 = 16_000;
const PA_SAMPLE_FLOAT32LE: c_int = 5;
const PA_STREAM_RECORD: c_int = 2;
/// 100 ms per read, the same tick live.rs works in.
const CHUNK: usize = RATE as usize / 10;

#[repr(C)]
struct SampleSpec {
    format: c_int,
    rate: u32,
    channels: u8,
}

#[repr(C)]
struct BufferAttr {
    maxlength: u32,
    tlength: u32,
    prebuf: u32,
    minreq: u32,
    fragsize: u32,
}

type NewFn = unsafe extern "C" fn(
    *const c_char,
    *const c_char,
    c_int,
    *const c_char,
    *const c_char,
    *const SampleSpec,
    *const c_void,
    *const BufferAttr,
    *mut c_int,
) -> *mut c_void;
type ReadFn = unsafe extern "C" fn(*mut c_void, *mut c_void, usize, *mut c_int) -> c_int;
type FreeFn = unsafe extern "C" fn(*mut c_void);
type StrErrorFn = unsafe extern "C" fn(c_int) -> *const c_char;

struct Pulse {
    new: NewFn,
    read: ReadFn,
    free: FreeFn,
    strerror: StrErrorFn,
    _lib: Library,
}

fn pulse() -> Result<&'static Pulse, CaptureError> {
    static P: OnceLock<Option<Pulse>> = OnceLock::new();
    P.get_or_init(|| unsafe {
        let lib = Library::new("libpulse-simple.so.0").ok()?;
        Some(Pulse {
            new: *lib.get::<NewFn>(b"pa_simple_new\0").ok()?,
            read: *lib.get::<ReadFn>(b"pa_simple_read\0").ok()?,
            free: *lib.get::<FreeFn>(b"pa_simple_free\0").ok()?,
            // libpulse-simple's own dependency; dlsym finds it through the handle.
            strerror: *lib.get::<StrErrorFn>(b"pa_strerror\0").ok()?,
            _lib: lib,
        })
    })
    .as_ref()
    .ok_or_else(|| CaptureError::Unsupported("needs PulseAudio or PipeWire (libpulse-simple not found)".into()))
}

struct Conn(*mut c_void);
// pa_simple is a blocking handle used by one thread at a time.
unsafe impl Send for Conn {}

fn connect(p: &Pulse) -> Result<Conn, CaptureError> {
    let spec = SampleSpec { format: PA_SAMPLE_FLOAT32LE, rate: RATE, channels: 1 };
    // Ask for small fragments: the default can hold seconds of audio back.
    let attr = BufferAttr { maxlength: u32::MAX, tlength: u32::MAX, prebuf: u32::MAX, minreq: u32::MAX, fragsize: (CHUNK * 4) as u32 };
    let mut err: c_int = 0;
    let s = unsafe {
        (p.new)(
            std::ptr::null(),
            c"Silent Voice".as_ptr(),
            PA_STREAM_RECORD,
            c"@DEFAULT_MONITOR@".as_ptr(),
            c"Meeting (Others)".as_ptr(),
            &spec,
            std::ptr::null(),
            &attr,
            &mut err,
        )
    };
    if s.is_null() {
        let why = unsafe { CStr::from_ptr((p.strerror)(err)) }.to_string_lossy().into_owned();
        // PA_ERR_CONNECTIONREFUSED (6): no sound server running.
        return Err(if err == 6 {
            CaptureError::Unsupported("needs PulseAudio or PipeWire (no sound server running)".into())
        } else {
            CaptureError::Failed(format!("Could not record the computer's sound: {why}"))
        });
    }
    Ok(Conn(s))
}

pub fn support() -> Result<(), CaptureError> {
    let p = pulse()?;
    let c = connect(p)?;
    unsafe { (p.free)(c.0) };
    Ok(())
}

/// Tells the reader thread to stop when dropped. Not joined: a read can block
/// while the server has nothing to give, and Stop must never hang on it; the
/// thread frees the connection after its current read (≤100 ms of audio lost).
struct Running(Arc<AtomicBool>);

impl Drop for Running {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

pub fn open(raw: Arc<Mutex<Vec<f32>>>, failed: Arc<Mutex<Option<String>>>) -> Result<(Box<dyn Any>, u32), CaptureError> {
    let p = pulse()?;
    let conn = connect(p)?;
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    std::thread::spawn(move || {
        let conn = conn;
        let mut buf = vec![0f32; CHUNK];
        while !stop2.load(Ordering::Relaxed) {
            let mut err: c_int = 0;
            if unsafe { (p.read)(conn.0, buf.as_mut_ptr().cast(), CHUNK * 4, &mut err) } < 0 {
                let why = unsafe { CStr::from_ptr((p.strerror)(err)) }.to_string_lossy().into_owned();
                if let Ok(mut slot) = failed.lock() {
                    slot.get_or_insert_with(|| format!("Computer sound stopped: {why}"));
                }
                break;
            }
            if let Ok(mut r) = raw.lock() {
                r.extend_from_slice(&buf);
            }
        }
        unsafe { (p.free)(conn.0) };
    });
    Ok((Box::new(Running(stop)), RATE))
}
