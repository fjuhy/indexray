"""Exercise the Windows tray application with a disposable profile and real filesystem events."""
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

release = Path(__file__).resolve().parents[1] / "release/IndexRay-portable-x64"
with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    source = root / "원본"
    output = root / "Markdown"
    source.mkdir()
    output.mkdir()
    env = dict(os.environ, INDEXRAY_INDEX_DIR=str(root / "index"))
    original = source / "주간회의.md"
    original.write_text("예산 협의", encoding="utf8")
    secret = source / "secret"
    secret.mkdir()
    forbidden = secret / "forbidden.txt"
    forbidden.write_text("privatecanary", encoding="utf8")
    sibling = source / "secret-suffix"
    sibling.mkdir()
    (sibling / "allowed.txt").write_text("allowed sibling", encoding="utf8")
    (root / "config.json").write_text(json.dumps({
        "export_folder":str(output),
        "autostart_user_disabled":True, "lang":"ko"
    }), encoding="utf8")
    started = time.monotonic()
    gui = subprocess.Popen([release / "IndexRay.exe"], env=env)
    def desktop_window_ready():
        user32 = ctypes.WinDLL("user32", use_last_error=True)
        find_window = user32.FindWindowW
        find_window.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR]
        find_window.restype = wintypes.HWND
        window = find_window(None, "IndexRay")
        if not window:
            return False
        pid = wintypes.DWORD()
        user32.GetWindowThreadProcessId(window, ctypes.byref(pid))
        if pid.value != gui.pid or not user32.IsWindowVisible(window):
            return False
        style = user32.GetWindowLongW(window, -16)
        extended = user32.GetWindowLongW(window, -20)
        caption_and_buttons = 0x00C00000 | 0x00080000 | 0x00020000 | 0x00010000
        assert style & caption_and_buttons == caption_and_buttons
        assert not extended & 0x80  # WS_EX_TOOLWINDOW hides the window from the taskbar.
        return window

    def wait_until(predicate):
        deadline = time.monotonic() + 50
        while time.monotonic() < deadline:
            if gui.poll() is not None:
                raise RuntimeError(f"GUI exited: {gui.returncode}")
            if predicate():
                return
            time.sleep(0.3)
        log = root / "logs" / "indexray.log"
        details = log.read_bytes()[-4000:].decode("utf8", errors="replace") if log.exists() else ""
        raise TimeoutError(f"GUI Markdown sync\n{details}")
    try:
        wait_until(desktop_window_ready)
        print(f"Visible window after {time.monotonic() - started:.1f}s", flush=True)
        another = root / "another"
        another.mkdir()
        (another / "second.txt").write_text("second root", encoding="utf8")
        subprocess.run(["powershell.exe", "-NoProfile", "-File", str(Path(__file__).with_name("pick_folder.ps1")),
                        "-AppProcessId", str(gui.pid), "-Folder", str(source), "-SecondFolder", str(another),
                        "-Exclude", str(secret), "-IndexMetadata", str(root / "index-meta.json")],
                       check=True, timeout=160)
        wait_until(lambda: len(list(output.glob("*.md"))) == 4)
        assert not list(output.glob("forbidden*.md"))
        result = subprocess.run([release / "indexray-cli.exe", "search", "privatecanary"],
                                env=env, capture_output=True, check=True)
        assert b"forbidden.txt" not in result.stdout
        exported = next(output.glob("주간회의*.md"))
        assert "예산 협의" in exported.read_text("utf8")
        (secret / "new-forbidden.txt").write_text("privatecanary", encoding="utf8")
        original.write_text("예산안 최종 승인",encoding="utf8")
        wait_until(lambda: "최종 승인" in exported.read_text("utf8"))
        result = subprocess.run([release / "indexray-cli.exe", "search", "privatecanary"],
                                env=env, capture_output=True, check=True)
        assert b"forbidden.txt" not in result.stdout
        original.unlink()
        wait_until(lambda: not exported.exists())

        gui.terminate()
        gui.wait(timeout=10)
        gui = subprocess.Popen([release / "IndexRay.exe"], env=env)
        user32 = ctypes.WinDLL("user32", use_last_error=True)
        user32.GetDlgItem.argtypes = [wintypes.HWND, ctypes.c_int]
        user32.GetDlgItem.restype = wintypes.HWND
        user32.FindWindowExW.argtypes = [wintypes.HWND, wintypes.HWND, wintypes.LPCWSTR, wintypes.LPCWSTR]
        user32.FindWindowExW.restype = wintypes.HWND
        user32.SendMessageW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
        user32.SendMessageW.restype = wintypes.LPARAM

        def registered_roots_restored():
            if not (window := desktop_window_ready()):
                return False
            panel = user32.FindWindowExW(window, None, "IndexRaySettingsPanel", None)
            if not panel:
                user32.SendMessageW(user32.GetDlgItem(window, 1005), 0x00F5, 0, 0)
                return False
            return user32.SendMessageW(user32.GetDlgItem(panel, 2001), 0x018B, 0, 0) == 2

        wait_until(registered_roots_restored)
    finally:
        if gui.poll() is None:
            subprocess.run(["taskkill", "/PID", str(gui.pid), "/T", "/F"], capture_output=True, check=True)
        gui.wait(timeout=10)
        time.sleep(2)
    print("PASS: native batch setup, first-scan exclusion, Markdown sync, and restart persistence")
