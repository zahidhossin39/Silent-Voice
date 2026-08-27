// Linux Accessibility (AT-SPI2) reader — the counterpart to ax.rs on macOS and
// the UI Automation code on Windows. Reads the focused editable field's text
// and the on-screen rectangle of a character range, so the same proofreading
// pipeline can drive underlines here.
//
// WHY dlopen AND NOT LINK: at-spi2 is not guaranteed present, and the
// accessibility bus it needs is not guaranteed running. Linking would make the
// whole app refuse to start on a machine without it. Loaded at runtime, its
// absence costs exactly one feature and is written to the log. Same reasoning
// as sherpa.rs.
//
// Four things differ from the other two platforms:
//
// 1. AT-SPI text offsets are CHARACTER offsets, so Harper's char spans map
//    straight across — no UTF-16 conversion, unlike macOS.
// 2. Every call is a D-Bus round trip. Reading rects for many words is
//    genuinely slow, which is why the caller caps how many it asks for.
// 3. AT-SPI is not thread safe and its event callbacks arrive on whichever
//    thread pumps the GLib main context. Everything here therefore runs on ONE
//    thread — the watcher's — which pumps the context itself rather than
//    handing control to atspi_event_main().
// 4. There is no "get focused element" call. Focus is tracked by subscribing to
//    state-changed:focused and remembering the last object that gained it.

#![allow(non_upper_case_globals)]

use libloading::{Library, Symbol};
use std::cell::RefCell;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, OnceLock};

type Accessible = *mut c_void;
type GError = *mut c_void;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct AtspiRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

/// Only the fields BEFORE AtspiEvent's embedded GValue are modelled. Reading a
/// prefix of a C struct is well defined; modelling the GValue would mean
/// guessing at a layout that differs between glib versions, and getting it
/// wrong would corrupt memory rather than fail visibly.
#[repr(C)]
struct AtspiEventPrefix {
    type_: *mut c_char,
    source: Accessible,
    detail1: c_int,
    detail2: c_int,
}

// AtspiStateType, from atspi-constants.h. Order is API, not incidental.
const ATSPI_STATE_EDITABLE: u32 = 7;
const ATSPI_STATE_FOCUSED: u32 = 12;
// AtspiCoordType::ATSPI_COORD_TYPE_SCREEN
const ATSPI_COORD_TYPE_SCREEN: u32 = 0;

// glib and gobject are already in the process — Tauri links GTK on Linux — so
// these resolve without adding a runtime requirement of their own.
#[link(name = "gobject-2.0")]
extern "C" {
    fn g_object_ref(obj: *mut c_void) -> *mut c_void;
    fn g_object_unref(obj: *mut c_void);
}

#[link(name = "glib-2.0")]
extern "C" {
    fn g_free(mem: *mut c_void);
    fn g_error_free(err: *mut c_void);
    fn g_main_context_iteration(context: *mut c_void, may_block: c_int) -> c_int;
}

type EventCb = unsafe extern "C" fn(*mut AtspiEventPrefix, *mut c_void);

/// Everything we call in libatspi, resolved once.
struct Api {
    _lib: Library,
    init: unsafe extern "C" fn() -> c_int,
    listener_new: unsafe extern "C" fn(EventCb, *mut c_void, *mut c_void) -> *mut c_void,
    listener_register: unsafe extern "C" fn(*mut c_void, *const c_char, *mut GError) -> c_int,
    get_state_set: unsafe extern "C" fn(Accessible) -> *mut c_void,
    state_contains: unsafe extern "C" fn(*mut c_void, u32) -> c_int,
    get_text_iface: unsafe extern "C" fn(Accessible) -> *mut c_void,
    get_editable_iface: unsafe extern "C" fn(Accessible) -> *mut c_void,
    get_component_iface: unsafe extern "C" fn(Accessible) -> *mut c_void,
    text_char_count: unsafe extern "C" fn(*mut c_void, *mut GError) -> c_int,
    text_get_text: unsafe extern "C" fn(*mut c_void, c_int, c_int, *mut GError) -> *mut c_char,
    text_range_extents:
        unsafe extern "C" fn(*mut c_void, c_int, c_int, u32, *mut GError) -> *mut AtspiRect,
    component_extents: unsafe extern "C" fn(*mut c_void, u32, *mut GError) -> *mut AtspiRect,
    role_name: unsafe extern "C" fn(Accessible, *mut GError) -> *mut c_char,
    get_application: unsafe extern "C" fn(Accessible, *mut GError) -> Accessible,
    get_name: unsafe extern "C" fn(Accessible, *mut GError) -> *mut c_char,
    delete_text: unsafe extern "C" fn(*mut c_void, c_int, c_int, *mut GError) -> c_int,
    insert_text: unsafe extern "C" fn(*mut c_void, c_int, *const c_char, c_int, *mut GError) -> c_int,
}

static API: OnceLock<Option<Api>> = OnceLock::new();

macro_rules! sym {
    ($lib:expr, $name:literal) => {{
        let s: Symbol<_> = match unsafe { $lib.get($name) } {
            Ok(s) => s,
            Err(_) => {
                crate::logging::log_error(
                    "atspi",
                    concat!("libatspi is missing ", stringify!($name)),
                );
                return None;
            }
        };
        unsafe { *s }
    }};
}

fn load() -> Option<Api> {
    // Distros ship the versioned soname; the unversioned one only exists with
    // the -dev package installed.
    let lib = ["libatspi.so.0", "libatspi.so"]
        .iter()
        .find_map(|n| unsafe { Library::new(*n) }.ok())?;

    let api = Api {
        init: sym!(lib, b"atspi_init\0"),
        listener_new: sym!(lib, b"atspi_event_listener_new\0"),
        listener_register: sym!(lib, b"atspi_event_listener_register\0"),
        get_state_set: sym!(lib, b"atspi_accessible_get_state_set\0"),
        state_contains: sym!(lib, b"atspi_state_set_contains\0"),
        get_text_iface: sym!(lib, b"atspi_accessible_get_text_iface\0"),
        get_editable_iface: sym!(lib, b"atspi_accessible_get_editable_text_iface\0"),
        get_component_iface: sym!(lib, b"atspi_accessible_get_component_iface\0"),
        text_char_count: sym!(lib, b"atspi_text_get_character_count\0"),
        text_get_text: sym!(lib, b"atspi_text_get_text\0"),
        text_range_extents: sym!(lib, b"atspi_text_get_range_extents\0"),
        component_extents: sym!(lib, b"atspi_component_get_extents\0"),
        role_name: sym!(lib, b"atspi_accessible_get_role_name\0"),
        get_application: sym!(lib, b"atspi_accessible_get_application\0"),
        get_name: sym!(lib, b"atspi_accessible_get_name\0"),
        delete_text: sym!(lib, b"atspi_editable_text_delete_text\0"),
        insert_text: sym!(lib, b"atspi_editable_text_insert_text\0"),
        _lib: lib,
    };
    Some(api)
}

fn api() -> Option<&'static Api> {
    API.get_or_init(load).as_ref()
}

// The focused object, owned by the thread that pumps the main context. A
// thread_local rather than a static because AT-SPI objects must not be touched
// from another thread, and this makes that structural instead of a convention.
thread_local! {
    static FOCUSED: RefCell<Accessible> = const { RefCell::new(std::ptr::null_mut()) };
}

unsafe extern "C" fn on_focus(event: *mut AtspiEventPrefix, _user: *mut c_void) {
    if event.is_null() {
        return;
    }
    let ev = &*event;
    // state-changed:focused fires for both gain and loss; detail1 says which.
    if ev.detail1 != 1 || ev.source.is_null() {
        return;
    }
    let src = g_object_ref(ev.source);
    FOCUSED.with(|f| {
        let mut slot = f.borrow_mut();
        if !slot.is_null() {
            g_object_unref(*slot);
        }
        *slot = src;
    });
}

static AVAILABLE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether AT-SPI came up. Unlike macOS this cannot change at runtime — the bus
/// is either reachable when we start or it is not — so the result is cached.
pub fn available() -> bool {
    AVAILABLE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Owning application name, already resolved while reading the field.
pub fn app_name(field: &Field) -> String {
    field.app.clone()
}

/// Queue a fix from the UI thread. Named to match the macOS backend; the work
/// itself happens on the watcher thread via drain_fixes.
pub fn apply_fix(start: usize, end: usize, expected: &str, replacement: &str) -> bool {
    request_fix(start, end, expected, replacement)
}

// ---- pointer position (X11) ----
//
// The overlay is click-through here too, so hover is detected by polling the
// pointer rather than by receiving events. Wayland deliberately offers no
// equivalent, which is one more reason inline proofreading is X11-only.

struct X11 {
    _lib: Library,
    display: *mut c_void,
    root: u64,
    query_pointer: unsafe extern "C" fn(
        *mut c_void,
        u64,
        *mut u64,
        *mut u64,
        *mut c_int,
        *mut c_int,
        *mut c_int,
        *mut c_int,
        *mut u32,
    ) -> c_int,
}

// Only ever used from the watcher thread; this exists so it can live in a
// OnceLock alongside everything else.
unsafe impl Send for X11 {}
unsafe impl Sync for X11 {}

static X11_API: OnceLock<Option<X11>> = OnceLock::new();

fn x11() -> Option<&'static X11> {
    X11_API
        .get_or_init(|| {
            let lib = ["libX11.so.6", "libX11.so"]
                .iter()
                .find_map(|n| unsafe { Library::new(*n) }.ok())?;
            unsafe {
                let open: Symbol<unsafe extern "C" fn(*const c_char) -> *mut c_void> =
                    lib.get(b"XOpenDisplay\0").ok()?;
                let root_of: Symbol<unsafe extern "C" fn(*mut c_void) -> u64> =
                    lib.get(b"XDefaultRootWindow\0").ok()?;
                let query: Symbol<
                    unsafe extern "C" fn(
                        *mut c_void,
                        u64,
                        *mut u64,
                        *mut u64,
                        *mut c_int,
                        *mut c_int,
                        *mut c_int,
                        *mut c_int,
                        *mut u32,
                    ) -> c_int,
                > = lib.get(b"XQueryPointer\0").ok()?;
                let display = open(std::ptr::null());
                if display.is_null() {
                    return None;
                }
                let root = root_of(display);
                let query_pointer = *query;
                Some(X11 { _lib: lib, display, root, query_pointer })
            }
        })
        .as_ref()
}

/// Pointer position in screen coordinates — the same space AT-SPI reports rects
/// in, so a hover test is a plain rectangle check.
pub fn cursor_position() -> Option<(f64, f64)> {
    let x = x11()?;
    unsafe {
        let (mut root_ret, mut child_ret) = (0u64, 0u64);
        let (mut rx, mut ry, mut wx, mut wy) = (0, 0, 0, 0);
        let mut mask = 0u32;
        let ok = (x.query_pointer)(
            x.display,
            x.root,
            &mut root_ret,
            &mut child_ret,
            &mut rx,
            &mut ry,
            &mut wx,
            &mut wy,
            &mut mask,
        );
        if ok == 0 {
            return None;
        }
        Some((rx as f64, ry as f64))
    }
}

/// Start AT-SPI and subscribe to focus changes. Must be called on the thread
/// that will later poll, and returns false if accessibility is unavailable —
/// which is normal on a machine where the a11y bus was never started.
pub fn init() -> bool {
    let Some(api) = api() else {
        crate::logging::log_info(
            "atspi",
            "libatspi not found — inline proofreading is unavailable on this system",
        );
        return false;
    };
    unsafe {
        // Non-zero means the accessibility bus could not be reached.
        if (api.init)() != 0 {
            crate::logging::log_info(
                "atspi",
                "the accessibility bus is not running — inline proofreading is unavailable. \
                 On GTK desktops: gsettings set org.gnome.desktop.interface toolkit-accessibility true",
            );
            return false;
        }
        let listener = (api.listener_new)(on_focus, std::ptr::null_mut(), std::ptr::null_mut());
        if listener.is_null() {
            crate::logging::log_error("atspi", "could not create the focus listener");
            return false;
        }
        // Toolkits disagree on which of these they emit, so take both.
        for ev in [c_str("object:state-changed:focused"), c_str("focus:")] {
            let mut err: GError = std::ptr::null_mut();
            (api.listener_register)(listener, ev.as_ptr(), &mut err);
            clear_err(err);
        }
    }
    AVAILABLE.store(true, std::sync::atomic::Ordering::Relaxed);
    true
}

/// Process any accessibility events that have arrived, without blocking. This
/// is what keeps the focused-object cache current.
pub fn pump() {
    unsafe {
        // Bounded so a storm of events cannot stall the poll loop.
        for _ in 0..64 {
            if g_main_context_iteration(std::ptr::null_mut(), 0) == 0 {
                break;
            }
        }
    }
}

fn c_str(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

unsafe fn clear_err(err: GError) {
    if !err.is_null() {
        g_error_free(err);
    }
}

/// Take ownership of a glib-allocated string.
unsafe fn take_gstring(p: *mut c_char) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let s = CStr::from_ptr(p).to_string_lossy().into_owned();
    g_free(p as *mut c_void);
    Some(s)
}

unsafe fn take_rect(p: *mut AtspiRect) -> Option<(f64, f64, f64, f64)> {
    if p.is_null() {
        return None;
    }
    let r = *p;
    g_free(p as *mut c_void);
    if r.width < 1 || r.height < 1 {
        return None;
    }
    Some((r.x as f64, r.y as f64, r.width as f64, r.height as f64))
}

/// The focused editable field, valid for the duration of one poll.
pub struct Field {
    accessible: Accessible,
    text_iface: *mut c_void,
    pub text: String,
    /// Screen box of the field, for clipping rects of scrolled-away text.
    pub rect: (f64, f64, f64, f64),
    /// Name of the owning application, for the user's ignore list.
    pub app: String,
    chars: Vec<char>,
}

impl Drop for Field {
    fn drop(&mut self) {
        unsafe {
            if !self.text_iface.is_null() {
                g_object_unref(self.text_iface);
            }
        }
    }
}

/// Longest field we will read. Every extra character is D-Bus traffic, and the
/// Windows watcher caps at the same number for the same reason.
const MAX_TEXT: c_int = 6000;

pub fn focused_field() -> Result<Field, &'static str> {
    let Some(api) = api() else {
        return Err("no atspi");
    };
    let accessible = FOCUSED.with(|f| *f.borrow());
    if accessible.is_null() {
        return Err("nothing focused yet");
    }

    unsafe {
        let role = {
            let mut err: GError = std::ptr::null_mut();
            let r = take_gstring((api.role_name)(accessible, &mut err)).unwrap_or_default();
            clear_err(err);
            r
        };
        // Role names are the stable, version-independent identifier here;
        // the numeric enum has grown over time.
        if role == "password text" {
            return Err("password field");
        }
        if role == "terminal" {
            return Err("terminal");
        }

        // Editability gate: without it we would underline read-only labels the
        // user has no way to correct.
        let states = (api.get_state_set)(accessible);
        if states.is_null() {
            return Err("no state set");
        }
        let editable = (api.state_contains)(states, ATSPI_STATE_EDITABLE) != 0;
        let focused = (api.state_contains)(states, ATSPI_STATE_FOCUSED) != 0;
        g_object_unref(states);
        if !editable {
            return Err("read-only");
        }
        // The cached object may have lost focus without us seeing the event.
        if !focused {
            return Err("stale focus");
        }

        let text_iface = (api.get_text_iface)(accessible);
        if text_iface.is_null() {
            return Err("no text interface");
        }

        let mut err: GError = std::ptr::null_mut();
        let count = (api.text_char_count)(text_iface, &mut err);
        clear_err(err);
        if count <= 0 {
            g_object_unref(text_iface);
            return Err("empty text");
        }

        let mut err: GError = std::ptr::null_mut();
        let text = take_gstring((api.text_get_text)(
            text_iface,
            0,
            count.min(MAX_TEXT),
            &mut err,
        ));
        clear_err(err);
        let Some(text) = text else {
            g_object_unref(text_iface);
            return Err("could not read text");
        };
        if text.trim().is_empty() {
            g_object_unref(text_iface);
            return Err("empty text");
        }

        let rect = {
            let comp = (api.get_component_iface)(accessible);
            if comp.is_null() {
                (0.0, 0.0, 0.0, 0.0)
            } else {
                let mut err: GError = std::ptr::null_mut();
                let r = take_rect((api.component_extents)(comp, ATSPI_COORD_TYPE_SCREEN, &mut err))
                    .unwrap_or((0.0, 0.0, 0.0, 0.0));
                clear_err(err);
                g_object_unref(comp);
                r
            }
        };

        let app = {
            let mut err: GError = std::ptr::null_mut();
            let a = (api.get_application)(accessible, &mut err);
            clear_err(err);
            if a.is_null() {
                String::new()
            } else {
                let mut err: GError = std::ptr::null_mut();
                let n = take_gstring((api.get_name)(a, &mut err)).unwrap_or_default();
                clear_err(err);
                g_object_unref(a);
                n.to_lowercase()
            }
        };

        let chars = text.chars().collect();
        Ok(Field { accessible, text_iface, text, rect, app, chars })
    }
}

impl Field {
    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    /// Screen rect of a char range. AT-SPI offsets are character offsets, so
    /// Harper's spans pass straight through with no conversion.
    pub fn bounds_for_range(&self, start: usize, end: usize) -> Option<(f64, f64, f64, f64)> {
        let api = api()?;
        if start > end || end > self.chars.len() {
            return None;
        }
        unsafe {
            let mut err: GError = std::ptr::null_mut();
            let r = take_rect((api.text_range_extents)(
                self.text_iface,
                start as c_int,
                end as c_int,
                ATSPI_COORD_TYPE_SCREEN,
                &mut err,
            ));
            clear_err(err);
            r
        }
    }

    /// Retain the underlying accessible so it survives past this poll — the
    /// popup click moves focus, and the fix has to land in the original field.
    pub fn retain(&self) -> ElementRef {
        unsafe { ElementRef(g_object_ref(self.accessible)) }
    }
}

/// A retained accessible, owned by the AT-SPI thread.
pub struct ElementRef(Accessible);

impl Drop for ElementRef {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                g_object_unref(self.0);
            }
        }
    }
}

impl ElementRef {
    /// Replace a char range, but only if it still holds exactly `expected`.
    /// A mismatch means the user typed or undid something while the popup was
    /// open, so the fix is refused rather than corrupting whatever moved in.
    fn replace_if_matches(
        &self,
        start: usize,
        end: usize,
        expected: &str,
        replacement: &str,
    ) -> bool {
        let Some(api) = api() else { return false };
        unsafe {
            let text_iface = (api.get_text_iface)(self.0);
            if text_iface.is_null() {
                return false;
            }
            let mut err: GError = std::ptr::null_mut();
            let count = (api.text_char_count)(text_iface, &mut err);
            clear_err(err);
            let mut err: GError = std::ptr::null_mut();
            let current = take_gstring((api.text_get_text)(
                text_iface,
                start as c_int,
                end.min(count.max(0) as usize) as c_int,
                &mut err,
            ));
            clear_err(err);
            g_object_unref(text_iface);
            if current.as_deref() != Some(expected) {
                return false;
            }

            let editable = (api.get_editable_iface)(self.0);
            if editable.is_null() {
                return false;
            }
            // Delete then insert: AT-SPI has no atomic replace, and doing it in
            // this order keeps the insert offset equal to the original start.
            let mut err: GError = std::ptr::null_mut();
            let deleted =
                (api.delete_text)(editable, start as c_int, end as c_int, &mut err) != 0;
            clear_err(err);
            if !deleted {
                g_object_unref(editable);
                return false;
            }
            let c = c_str(replacement);
            let mut err: GError = std::ptr::null_mut();
            let inserted = (api.insert_text)(
                editable,
                start as c_int,
                c.as_ptr(),
                replacement.chars().count() as c_int,
                &mut err,
            ) != 0;
            clear_err(err);
            g_object_unref(editable);
            inserted
        }
    }
}

// ---- fixes, marshalled onto the AT-SPI thread ----
//
// The Tauri command that applies a suggestion runs on a different thread, and
// AT-SPI objects must not be touched from one. The request is queued instead
// and applied by the watcher on its next pass, which replies with the outcome.

struct FixRequest {
    start: usize,
    end: usize,
    expected: String,
    replacement: String,
    reply: Sender<bool>,
}

static FIX_TX: OnceLock<Mutex<Sender<FixRequest>>> = OnceLock::new();
thread_local! {
    static FIX_RX: RefCell<Option<Receiver<FixRequest>>> = const { RefCell::new(None) };
    static PENDING: RefCell<Option<ElementRef>> = const { RefCell::new(None) };
}

/// Called once from the watcher thread, which then owns the receiving end.
pub fn install_fix_channel() {
    let (tx, rx) = channel::<FixRequest>();
    let _ = FIX_TX.set(Mutex::new(tx));
    FIX_RX.with(|r| *r.borrow_mut() = Some(rx));
}

/// Remember the field the popup is offering to fix.
pub fn remember(field: &Field) {
    PENDING.with(|p| *p.borrow_mut() = Some(field.retain()));
}

pub fn forget() {
    PENDING.with(|p| *p.borrow_mut() = None);
}

/// Queue a fix from any thread and wait briefly for the watcher to apply it.
pub fn request_fix(start: usize, end: usize, expected: &str, replacement: &str) -> bool {
    let Some(tx) = FIX_TX.get() else { return false };
    let (reply_tx, reply_rx) = channel::<bool>();
    let req = FixRequest {
        start,
        end,
        expected: expected.to_string(),
        replacement: replacement.to_string(),
        reply: reply_tx,
    };
    {
        let Ok(guard) = tx.lock() else { return false };
        if guard.send(req).is_err() {
            return false;
        }
    }
    // One poll interval plus slack. Waiting forever would hang the UI thread if
    // the watcher ever stopped.
    reply_rx
        .recv_timeout(std::time::Duration::from_millis(1500))
        .unwrap_or(false)
}

/// Apply any queued fixes. Watcher thread only.
pub fn drain_fixes() {
    FIX_RX.with(|r| {
        let borrowed = r.borrow();
        let Some(rx) = borrowed.as_ref() else { return };
        while let Ok(req) = rx.try_recv() {
            let applied = PENDING.with(|p| {
                p.borrow()
                    .as_ref()
                    .map(|el| {
                        el.replace_if_matches(req.start, req.end, &req.expected, &req.replacement)
                    })
                    .unwrap_or(false)
            });
            let _ = req.reply.send(applied);
        }
    });
}
