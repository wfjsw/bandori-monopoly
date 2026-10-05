#!/usr/bin/env python3
"""Drive the original Unity build to capture reference screenshots of its UI.

Capture uses PrintWindow, so it works while the window is covered and never
touches the cursor or focus. Note: Unity reads raw input, so the *posted* clicks
below do not reach its UI -- drive screens with `-sceneTour` instead.

  python drive.py launch                 # start the game windowed at 1600x900
  python drive.py shot NAME              # save docs/ui-ref/NAME.png (window client area)
  python drive.py click X Y              # click at client-area coordinates (1600x900 space)
  python drive.py type TEXT              # type Unicode text
  python drive.py key NAME               # press enter | esc | tab | backspace
  python drive.py quit                   # close the game

Windows only (user32/gdi32 via ctypes, PIL).
"""

import ctypes
import ctypes.wintypes as wt
import subprocess
import sys
import time
from pathlib import Path


GAME = Path(r"D:/BanG Dream 大富翁/BandoriMonopoly.exe")
OUT = Path(__file__).resolve().parents[2] / "docs" / "ui-ref"
TITLE = "BanG Dream 大富翁"
W, H = 1600, 900

user32 = ctypes.windll.user32
try:
    ctypes.windll.shcore.SetProcessDpiAwareness(2)  # real pixel coordinates
except Exception:  # noqa: BLE001
    user32.SetProcessDPIAware()


def window():
    hwnd = user32.FindWindowW(None, TITLE)
    if not hwnd:
        sys.exit(f"window {TITLE!r} not found")
    return hwnd


def client_origin(hwnd):
    pt = wt.POINT(0, 0)
    user32.ClientToScreen(hwnd, ctypes.byref(pt))
    rc = wt.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(rc))
    return pt.x, pt.y, rc.right, rc.bottom


# Input is *posted* to the game window: the real cursor, keyboard focus and the
# foreground window are never touched, so the user can keep working meanwhile.
WM_MOUSEMOVE, WM_LBUTTONDOWN, WM_LBUTTONUP = 0x0200, 0x0201, 0x0202
WM_KEYDOWN, WM_KEYUP, WM_CHAR = 0x0100, 0x0101, 0x0102
MK_LBUTTON = 0x0001


def _lparam(hwnd, x, y):
    rc = wt.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(rc))
    cx, cy = round(x * rc.right / W), round(y * rc.bottom / H)
    return (cy << 16) | (cx & 0xFFFF)


def click(x, y):
    hwnd = window()
    lp = _lparam(hwnd, x, y)
    user32.PostMessageW(hwnd, WM_MOUSEMOVE, 0, lp)
    time.sleep(0.08)
    user32.PostMessageW(hwnd, WM_LBUTTONDOWN, MK_LBUTTON, lp)
    time.sleep(0.08)
    user32.PostMessageW(hwnd, WM_LBUTTONUP, 0, lp)


def type_text(text):
    hwnd = window()
    for ch in text:
        user32.PostMessageW(hwnd, WM_CHAR, ord(ch), 1)
        time.sleep(0.03)


KEYS = {"enter": 0x0D, "esc": 0x1B, "tab": 0x09, "backspace": 0x08}


def key(name):
    hwnd = window()
    vk = KEYS[name]
    user32.PostMessageW(hwnd, WM_KEYDOWN, vk, 1)
    user32.PostMessageW(hwnd, WM_KEYUP, vk, 0xC0000001)


gdi32 = ctypes.windll.gdi32


def capture(hwnd):
    """The window's own client-area pixels via PrintWindow (works when covered)."""
    rc = wt.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(rc))
    w, h = rc.right, rc.bottom
    hdc = user32.GetDC(hwnd)
    mdc = gdi32.CreateCompatibleDC(hdc)
    bmp = gdi32.CreateCompatibleBitmap(hdc, w, h)
    gdi32.SelectObject(mdc, bmp)
    # PW_CLIENTONLY | PW_RENDERFULLCONTENT (needed for DirectX swap chains)
    ok = user32.PrintWindow(hwnd, mdc, 0x1 | 0x2)
    buf = ctypes.create_string_buffer(w * h * 4)

    class BITMAPINFOHEADER(ctypes.Structure):
        _fields_ = [("biSize", wt.DWORD), ("biWidth", wt.LONG), ("biHeight", wt.LONG), ("biPlanes", wt.WORD), ("biBitCount", wt.WORD),
                    ("biCompression", wt.DWORD), ("biSizeImage", wt.DWORD), ("biXPelsPerMeter", wt.LONG), ("biYPelsPerMeter", wt.LONG),
                    ("biClrUsed", wt.DWORD), ("biClrImportant", wt.DWORD)]

    bi = BITMAPINFOHEADER(ctypes.sizeof(BITMAPINFOHEADER), w, -h, 1, 32, 0, 0, 0, 0, 0, 0)
    gdi32.GetDIBits(mdc, bmp, 0, h, buf, ctypes.byref(bi), 0)
    gdi32.DeleteObject(bmp)
    gdi32.DeleteDC(mdc)
    user32.ReleaseDC(hwnd, hdc)
    from PIL import Image
    return ok, Image.frombuffer("RGBA", (w, h), buf, "raw", "BGRA", 0, 1).convert("RGB")


def shot(name):
    ok, img = capture(window())
    OUT.mkdir(parents=True, exist_ok=True)
    path = OUT / f"{name}.png"
    img.save(path)
    print(path, img.size, "PrintWindow ok" if ok else "PrintWindow FAILED")


def main():
    cmd, *args = sys.argv[1:] or ["help"]
    if cmd == "launch":
        subprocess.Popen([str(GAME), "-screen-fullscreen", "0", "-screen-width", str(W), "-screen-height", str(H)], cwd=GAME.parent)
        for _ in range(60):
            time.sleep(1)
            if user32.FindWindowW(None, TITLE):
                print("window up")
                return
        sys.exit("game window did not appear")
    elif cmd == "shot":
        shot(args[0])
    elif cmd == "click":
        click(float(args[0]), float(args[1]))
    elif cmd == "type":
        type_text(" ".join(args))
    elif cmd == "key":
        key(args[0])
    elif cmd == "quit":
        user32.PostMessageW(window(), 0x0010, 0, 0)  # WM_CLOSE
    else:
        print(__doc__)


if __name__ == "__main__":
    main()
