"""Time from a real hotkey press to the pill showing the recording state.

Run with the app running and its hotkey set to PageUp:
    python scripts/press_latency_check.py [presses]
Each round: wait 7s (so "hide pill when idle" has hidden it), press PageUp,
poll the pill's pixels until the accent-coloured bars appear, hold ~1s, release.
Prints per-press latency; exit 1 if any press took > 300 ms or never showed.
"""
import ctypes, sys, time
from ctypes import wintypes
from PIL import ImageGrab

u32 = ctypes.windll.user32
u32.FindWindowW.restype = wintypes.HWND
ctypes.windll.shcore.SetProcessDpiAwareness(2)
VK_PRIOR, KEYUP, EXT = 0x21, 0x2, 0x1
N = int(sys.argv[1]) if len(sys.argv) > 1 else 5
IDLE = float(sys.argv[2]) if len(sys.argv) > 2 else 7.0

def key(up):
    u32.keybd_event(VK_PRIOR, 0x49, EXT | (KEYUP if up else 0), 0)

def pill_rect():
    h = u32.FindWindowW(None, "Silent Voice Overlay")
    r = wintypes.RECT()
    u32.GetWindowRect(h, ctypes.byref(r))
    return h, (r.left, r.top, r.right, r.bottom)

def orange_visible(rect):
    # The idle pill is grey; recording bars use the accent colour, whatever
    # theme is picked. Any strongly saturated pixel = recording is on screen.
    img = ImageGrab.grab(bbox=rect, all_screens=True).convert("RGB")
    return any(max(p) - min(p) > 90 and max(p) > 120 for p in img.get_flattened_data())

bad = 0
for i in range(N):
    time.sleep(IDLE)
    h, rect = pill_rect()
    t0 = time.perf_counter()
    key(False)
    seen = None
    while time.perf_counter() - t0 < 2.0:
        if u32.IsWindowVisible(h) and orange_visible(rect):
            seen = (time.perf_counter() - t0) * 1000
            break
        time.sleep(0.005)
    time.sleep(max(0, 1.0 - (time.perf_counter() - t0)))
    key(True)
    print(f"press {i+1}: " + (f"recording visible after {seen:.0f} ms" if seen else "NEVER showed within 2 s"))
    if seen is None or seen > 300:
        bad += 1
sys.exit(1 if bad else 0)
