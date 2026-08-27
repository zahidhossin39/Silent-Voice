// macOS Accessibility reader — the counterpart to the UI Automation code in
// inline_check.rs. Reads the focused editable field's text and the on-screen
// rectangle of any character range, so the same proofreading pipeline can drive
// underlines on this platform.
//
// Three things differ from the Windows side and cause real bugs if forgotten:
//
// 1. AX ranges are UTF-16 code units, not chars. Harper spans are char indices.
//    They coincide for ASCII and diverge the moment an emoji or accented
//    character appears earlier in the field, which silently shifts every rect
//    after it. Convert at the boundary; never assume.
// 2. AX reports points, not pixels. Tauri positions windows in logical units
//    too, so the renderer stays in points and never touches the scale factor.
// 3. AX screen coordinates are top-left origin (unlike most of AppKit), which
//    happens to match what SquiggleInfo already expects.

#![allow(non_upper_case_globals)]

use core_foundation::base::{CFRelease, CFRetain, CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CFRange {
    location: isize,
    length: isize,
}

type AXUIElementRef = *const std::ffi::c_void;
type AXValueRef = *const std::ffi::c_void;
type AXError = i32;

const kAXErrorSuccess: AXError = 0;
const kAXValueCGPointType: u32 = 1;
const kAXValueCGSizeType: u32 = 2;
const kAXValueCGRectType: u32 = 3;
const kAXValueCFRangeType: u32 = 4;

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXUIElementCreateSystemWide() -> AXUIElementRef;
    fn AXUIElementCopyAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementSetAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        value: CFTypeRef,
    ) -> AXError;
    fn AXUIElementCopyParameterizedAttributeValue(
        element: AXUIElementRef,
        attribute: CFStringRef,
        parameter: CFTypeRef,
        result: *mut CFTypeRef,
    ) -> AXError;
    fn AXUIElementIsAttributeSettable(
        element: AXUIElementRef,
        attribute: CFStringRef,
        settable: *mut u8,
    ) -> AXError;
    fn AXUIElementGetPid(element: AXUIElementRef, pid: *mut i32) -> AXError;
    fn AXValueCreate(the_type: u32, value_ptr: *const std::ffi::c_void) -> AXValueRef;
    fn AXValueGetValue(value: AXValueRef, the_type: u32, value_ptr: *mut std::ffi::c_void) -> u8;
}

/// Copy an attribute. The returned CFTypeRef is owned by the caller.
unsafe fn copy_attr(el: AXUIElementRef, name: &str) -> Option<CFTypeRef> {
    let key = CFString::new(name);
    let mut out: CFTypeRef = std::ptr::null();
    if AXUIElementCopyAttributeValue(el, key.as_concrete_TypeRef(), &mut out) != kAXErrorSuccess
        || out.is_null()
    {
        return None;
    }
    Some(out)
}

unsafe fn copy_string_attr(el: AXUIElementRef, name: &str) -> Option<String> {
    let raw = copy_attr(el, name)?;
    Some(CFString::wrap_under_create_rule(raw as CFStringRef).to_string())
}

unsafe fn element_rect(el: AXUIElementRef) -> Option<(f64, f64, f64, f64)> {
    let pos_raw = copy_attr(el, "AXPosition")?;
    let mut p = CGPoint::default();
    let got_pos =
        AXValueGetValue(pos_raw, kAXValueCGPointType, &mut p as *mut _ as *mut std::ffi::c_void)
            != 0;
    CFRelease(pos_raw);

    let size_raw = copy_attr(el, "AXSize")?;
    let mut s = CGSize::default();
    let got_size =
        AXValueGetValue(size_raw, kAXValueCGSizeType, &mut s as *mut _ as *mut std::ffi::c_void)
            != 0;
    CFRelease(size_raw);

    if got_pos && got_size {
        Some((p.x, p.y, s.width, s.height))
    } else {
        None
    }
}

/// Roles that are unambiguously editable text. Anything else has to prove it.
fn role_is_editable(role: &str) -> bool {
    matches!(role, "AXTextField" | "AXTextArea" | "AXComboBox")
}

/// The focused editable field, borrowed for the duration of one poll.
pub struct Field {
    el: AXUIElementRef,
    pub pid: i32,
    pub text: String,
    /// Screen box of the field itself, for clipping rects of scrolled-away text.
    pub rect: (f64, f64, f64, f64),
    chars: Vec<char>,
}

impl Drop for Field {
    fn drop(&mut self) {
        unsafe {
            if !self.el.is_null() {
                CFRelease(self.el);
            }
        }
    }
}

pub fn focused_field() -> Result<Field, &'static str> {
    unsafe {
        let sys = AXUIElementCreateSystemWide();
        if sys.is_null() {
            return Err("no system-wide element");
        }
        let focused = copy_attr(sys, "AXFocusedUIElement");
        CFRelease(sys);
        let el = focused.ok_or("no focused element")? as AXUIElementRef;

        // Every exit from here on owns `el`, so failures release it rather than
        // returning early with `?`.
        macro_rules! fail {
            ($reason:expr) => {{
                CFRelease(el);
                return Err($reason);
            }};
        }

        let mut pid: i32 = 0;
        if AXUIElementGetPid(el, &mut pid) != kAXErrorSuccess {
            fail!("no pid");
        }
        if pid == std::process::id() as i32 {
            fail!("own element focused");
        }

        let role = copy_string_attr(el, "AXRole").unwrap_or_default();
        if role == "AXSecureTextField" {
            fail!("password field");
        }

        // Editability gate, same intent as the Windows ValuePattern IsReadOnly
        // check: if the value cannot be written back, a fix could never apply,
        // so underlining it would only ever frustrate.
        let value_key = CFString::new("AXValue");
        let mut settable: u8 = 0;
        if AXUIElementIsAttributeSettable(el, value_key.as_concrete_TypeRef(), &mut settable)
            != kAXErrorSuccess
            || settable == 0
        {
            fail!("read-only");
        }

        // A settable AXValue is not proof on its own — some custom controls
        // expose one. Anything outside the known text roles has to also carry a
        // selection range, which static labels do not.
        if !role_is_editable(&role) {
            match copy_attr(el, "AXSelectedTextRange") {
                Some(r) => CFRelease(r),
                None => fail!("not a text field"),
            }
        }

        let text = match copy_string_attr(el, "AXValue") {
            Some(t) => t,
            None => fail!("no value"),
        };
        if text.trim().is_empty() {
            fail!("empty text");
        }

        let rect = element_rect(el).unwrap_or((0.0, 0.0, 0.0, 0.0));
        let chars = text.chars().collect();
        Ok(Field { el, pid, text, rect, chars })
    }
}

impl Field {
    pub fn chars(&self) -> &[char] {
        &self.chars
    }

    /// Char range → the UTF-16 offsets AX actually indexes by.
    fn utf16_range(&self, start: usize, end: usize) -> Option<CFRange> {
        if start > end || end > self.chars.len() {
            return None;
        }
        let location: usize = self.chars[..start].iter().map(|c| c.len_utf16()).sum();
        let length: usize = self.chars[start..end].iter().map(|c| c.len_utf16()).sum();
        Some(CFRange { location: location as isize, length: length as isize })
    }

    /// Screen rect of a char range, or None when it is not currently visible.
    /// AX reports a degenerate rect for scrolled-away text, which gives the
    /// same free clipping the Windows side gets from UIA.
    pub fn bounds_for_range(&self, start: usize, end: usize) -> Option<(f64, f64, f64, f64)> {
        unsafe {
            let range = self.utf16_range(start, end)?;
            let param =
                AXValueCreate(kAXValueCFRangeType, &range as *const _ as *const std::ffi::c_void);
            if param.is_null() {
                return None;
            }
            let key = CFString::new("AXBoundsForRange");
            let mut out: CFTypeRef = std::ptr::null();
            let err = AXUIElementCopyParameterizedAttributeValue(
                self.el,
                key.as_concrete_TypeRef(),
                param as CFTypeRef,
                &mut out,
            );
            CFRelease(param as CFTypeRef);
            if err != kAXErrorSuccess || out.is_null() {
                return None;
            }
            let mut r = CGRect::default();
            let got =
                AXValueGetValue(out, kAXValueCGRectType, &mut r as *mut _ as *mut std::ffi::c_void)
                    != 0;
            CFRelease(out);
            if !got || r.size.width < 1.0 || r.size.height < 1.0 {
                return None;
            }
            Some((r.origin.x, r.origin.y, r.size.width, r.size.height))
        }
    }

    /// Replace a char range by selecting it and writing the selection, rather
    /// than rewriting the whole value — that keeps the host app's undo stack
    /// intact and leaves the caret where the user expects it.
    pub fn replace_range(&self, start: usize, end: usize, replacement: &str) -> bool {
        unsafe {
            let Some(range) = self.utf16_range(start, end) else {
                return false;
            };
            let val =
                AXValueCreate(kAXValueCFRangeType, &range as *const _ as *const std::ffi::c_void);
            if val.is_null() {
                return false;
            }
            let sel_key = CFString::new("AXSelectedTextRange");
            let err =
                AXUIElementSetAttributeValue(self.el, sel_key.as_concrete_TypeRef(), val as CFTypeRef);
            CFRelease(val as CFTypeRef);
            if err != kAXErrorSuccess {
                return false;
            }
            let text_key = CFString::new("AXSelectedText");
            let new_text = CFString::new(replacement);
            AXUIElementSetAttributeValue(
                self.el,
                text_key.as_concrete_TypeRef(),
                new_text.as_concrete_TypeRef() as CFTypeRef,
            ) == kAXErrorSuccess
        }
    }
}

/// A retained reference to a field, kept alive past the poll that found it.
///
/// The suggestion popup is a real window, so clicking it moves focus away and
/// `focused_field()` would then describe the popup rather than the text the
/// user is fixing. Holding the element instead means the fix lands where it was
/// aimed, no matter what has focus by the time the click arrives.
pub struct ElementRef(AXUIElementRef);

// The element is only ever touched from the watcher thread; this exists so the
// handle can be parked in shared state between the hover and the click.
unsafe impl Send for ElementRef {}

impl Drop for ElementRef {
    fn drop(&mut self) {
        unsafe {
            if !self.0.is_null() {
                CFRelease(self.0);
            }
        }
    }
}

impl Field {
    /// Retain this field's element so it outlives the current poll.
    pub fn retain(&self) -> ElementRef {
        unsafe {
            let _ = CFRetain(self.el);
            ElementRef(self.el)
        }
    }
}

impl ElementRef {
    fn text(&self) -> Option<String> {
        unsafe { copy_string_attr(self.0, "AXValue") }
    }

    /// Replace a char range, but only if it still holds exactly `expected`.
    ///
    /// Between the popup opening and the click landing the user may have typed,
    /// scrolled, or undone something. Replacing blind would corrupt whatever
    /// moved into those offsets, so a mismatch refuses the fix instead.
    pub fn replace_if_matches(
        &self,
        start: usize,
        end: usize,
        expected: &str,
        replacement: &str,
    ) -> bool {
        let text = match self.text() {
            Some(t) => t,
            None => return false,
        };
        let chars: Vec<char> = text.chars().collect();
        if start > end || end > chars.len() {
            return false;
        }
        let current: String = chars[start..end].iter().collect();
        if current != expected {
            return false;
        }
        let location: usize = chars[..start].iter().map(|c| c.len_utf16()).sum();
        let length: usize = chars[start..end].iter().map(|c| c.len_utf16()).sum();
        let range = CFRange { location: location as isize, length: length as isize };

        unsafe {
            let val =
                AXValueCreate(kAXValueCFRangeType, &range as *const _ as *const std::ffi::c_void);
            if val.is_null() {
                return false;
            }
            let sel_key = CFString::new("AXSelectedTextRange");
            let err =
                AXUIElementSetAttributeValue(self.0, sel_key.as_concrete_TypeRef(), val as CFTypeRef);
            CFRelease(val as CFTypeRef);
            if err != kAXErrorSuccess {
                return false;
            }
            let text_key = CFString::new("AXSelectedText");
            let new_text = CFString::new(replacement);
            AXUIElementSetAttributeValue(
                self.0,
                text_key.as_concrete_TypeRef(),
                new_text.as_concrete_TypeRef() as CFTypeRef,
            ) == kAXErrorSuccess
        }
    }
}

/// Current mouse position in the same top-left-origin screen points AX reports
/// rects in, so a hover test is a plain rectangle check.
pub fn cursor_position() -> Option<(f64, f64)> {
    unsafe {
        let event = CGEventCreate(std::ptr::null());
        if event.is_null() {
            return None;
        }
        let p = CGEventGetLocation(event);
        CFRelease(event as CFTypeRef);
        Some((p.x, p.y))
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: *const std::ffi::c_void) -> *const std::ffi::c_void;
    fn CGEventGetLocation(event: *const std::ffi::c_void) -> CGPoint;
}
