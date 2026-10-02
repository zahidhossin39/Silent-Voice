"""Red/green check: does the Silent Voice pill stay above other always-on-top windows?

Run with the app running (npm run tauri:dev, or installed):
    python scripts/pill_zorder_check.py
Opens a small always-on-top window (like Task Manager or a video pop-out),
then presses the hotkey (PageUp) the way a user would, and checks the pill
shows up ABOVE it. Then waits for the keep-alive with the pill visible.
Exit 0 = pill on top both times, 1 = buried.
"""
import ctypes, sys, time, tkinter as tk
from ctypes import wintypes

u32 = ctypes.windll.user32
u32.FindWindowW.restype = wintypes.HWND
u32.GetWindow.restype = wintypes.HWND
u32.GetParent.restype = wintypes.HWND
GW_HWNDPREV = 3
WAIT = float(sys.argv[1]) if len(sys.argv) > 1 else 3.0

pill = u32.FindWindowW(None, "Silent Voice Overlay")
if not pill:
    sys.exit("pill window not found - is the app running with the pill visible?")

root = tk.Tk()
root.title("zorder-intruder")
root.geometry("200x80+50+50")
root.attributes("-topmost", True)
root.update()
intruder = u32.GetParent(root.winfo_id()) or root.winfo_id()

def above(a, b):
    """True if window a is above window b in z-order."""
    h = b
    while h:
        h = u32.GetWindow(h, GW_HWNDPREV)
        if h == a:
            return True
    return False

def pump(secs):
    end = time.time() + secs
    while time.time() < end:
        root.update()
        time.sleep(0.02)

VK_PRIOR = 0x21
pump(0.5)
u32.keybd_event(VK_PRIOR, 0x49, 1, 0)          # press hotkey
pump(0.4)
on_press = u32.IsWindowVisible(pill) and not above(intruder, pill)
print("on hotkey press, pill visible and on top:", bool(on_press))
pump(0.6)
u32.keybd_event(VK_PRIOR, 0x49, 1 | 2, 0)      # release
# Raise the intruder again while the pill is still visible, then let the
# 2 s keep-alive run.
root.attributes("-topmost", False); root.attributes("-topmost", True)
pump(WAIT)
kept = u32.IsWindowVisible(pill) and not above(intruder, pill)
print(f"intruder re-raised, after {WAIT}s keep-alive pill on top:", bool(kept))
root.destroy()
sys.exit(0 if on_press and kept else 1)
