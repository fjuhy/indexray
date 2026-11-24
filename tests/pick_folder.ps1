param([int]$AppProcessId, [string]$Folder, [string]$SecondFolder, [string]$Exclude, [string]$IndexMetadata)
$ErrorActionPreference = 'Stop'

python -c @"
import ctypes, sys, time
from ctypes import wintypes
from pathlib import Path

pid = int(sys.argv[1])
folder = sys.argv[2]
second, exclude, metadata = sys.argv[3:]
user32 = ctypes.WinDLL('user32', use_last_error=True)

EnumWindowsProc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)

def find_main_hwnd():
    found = []
    @EnumWindowsProc
    def cb(hwnd, _):
        p = wintypes.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(p))
        if p.value == pid and user32.IsWindowVisible(hwnd):
            buf = ctypes.create_unicode_buffer(64)
            user32.GetWindowTextW(hwnd, buf, 64)
            if buf.value == 'IndexRay':
                found.append(hwnd)
                return False
        return True
    user32.EnumWindows(cb, 0)
    return found[0] if found else None

def wait_for(fn, desc, timeout=30.0):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        val = fn()
        if val:
            return val
        time.sleep(0.05)
    raise RuntimeError(f'Timeout waiting for {desc}')

main_hwnd = wait_for(find_main_hwnd, 'IndexRay main window')

# 1. Click '색인 폴더 및 설정' button (ID 1005)
user32.GetDlgItem.argtypes = [wintypes.HWND, ctypes.c_int]
user32.GetDlgItem.restype = wintypes.HWND
settings_btn = wait_for(lambda: user32.GetDlgItem(main_hwnd, 1005), 'Settings button')
user32.SendMessageW(settings_btn, 0x00F5, 0, 0)  # BM_CLICK

# 2. Locate in-window SettingsPanel, set path (ID 2002), click Add Folder (ID 2004)
user32.FindWindowExW.argtypes = [wintypes.HWND, wintypes.HWND, wintypes.LPCWSTR, wintypes.LPCWSTR]
user32.FindWindowExW.restype = wintypes.HWND
panel = wait_for(
    lambda: (h := user32.FindWindowExW(main_hwnd, None, 'IndexRaySettingsPanel', None)) and user32.IsWindowVisible(h) and h,
    'SettingsPanel'
)
path_edit = wait_for(lambda: user32.GetDlgItem(panel, 2002), 'RootPathEdit')
send_text = user32.SendMessageW
send_text.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
send_text.restype = wintypes.LPARAM
def set_text(control, value):
    send_text.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPCWSTR]
    send_text(control, 0x000C, 0, value)  # WM_SETTEXT
    send_text.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]

add_btn = wait_for(
    lambda: (b := user32.GetDlgItem(panel, 2004)) and user32.IsWindowEnabled(b) and b,
    'AddRootButton'
)
ext_edit = user32.GetDlgItem(panel, 2010)
wait_for(lambda: user32.SendMessageW(ext_edit, 0x000E, 0, 0), 'loaded indexing rules')
roots = user32.GetDlgItem(panel, 2001)
rebuild = user32.GetDlgItem(panel, 2005)
apply = user32.GetDlgItem(panel, 2014)
preview = user32.GetDlgItem(main_hwnd, 1011)
progress = user32.GetDlgItem(panel, 2015)
assert progress and not user32.IsWindowVisible(progress)
assert preview and not user32.IsWindowVisible(preview)
for count, path in enumerate((folder, second), 1):
    set_text(path_edit, path)
    user32.SendMessageW(add_btn, 0x00F5, 0, 0)  # BM_CLICK
    assert user32.SendMessageW(roots, 0x018B, 0, 0) == count
    assert not user32.IsWindowEnabled(rebuild), 'pending roots should use the batch action'
    assert not Path(metadata).exists(), 'staging started indexing before exclusions were set'

set_text(user32.GetDlgItem(panel, 2011), exclude + '\r\n')
user32.SendMessageW(apply, 0x00F5, 0, 0)
if not user32.IsWindowEnabled(apply):
    assert user32.IsWindowVisible(progress)
    assert not user32.IsWindowEnabled(ext_edit)
wait_for(lambda: user32.IsWindowEnabled(rebuild), 'registered roots after batch indexing')
assert user32.SendMessageW(roots, 0x018B, 0, 0) == 2
assert not user32.IsWindowVisible(progress)
assert not user32.IsWindowVisible(preview), 'search preview overlapped settings after batch indexing'

user32.SendMessageW(rebuild, 0x00F5, 0, 0)
wait_for(lambda: user32.IsWindowEnabled(rebuild), 'selected-root rebuild')
assert user32.SendMessageW(roots, 0x018B, 0, 0) == 2
assert not user32.IsWindowVisible(preview), 'search preview overlapped settings after rebuild'

remove = user32.GetDlgItem(panel, 2006)
def root_count():
    return user32.SendMessageW(roots, 0x018B, 0, 0)
output = Path(metadata).parent / 'Markdown'
personal = output / 'personal.md'
personal.write_text('my own file', encoding='utf8')
for remaining in (1, 0):
    user32.SendMessageW(remove, 0x00F5, 0, 0)
    wait_for(lambda: user32.IsWindowEnabled(remove) and root_count() == remaining,
             'folder removal')
assert set(output.glob('*.md')) == {personal}, 'last root removal left managed exports behind'
assert personal.read_text(encoding='utf8') == 'my own file'
assert not user32.IsWindowEnabled(rebuild)
for path in (folder, second):
    set_text(path_edit, path)
    user32.SendMessageW(add_btn, 0x00F5, 0, 0)
user32.SendMessageW(apply, 0x00F5, 0, 0)
wait_for(lambda: user32.IsWindowEnabled(rebuild) and root_count() == 2,
         'folder registration after removing the last root')
assert not user32.IsWindowVisible(preview), 'search preview overlapped settings after re-add'
user32.SendMessageW(user32.GetDlgItem(panel, 2000), 0x00F5, 0, 0)
wait_for(lambda: not user32.IsWindowVisible(panel) and user32.IsWindowVisible(preview),
         'search preview restored when leaving settings')
print('PASS: pending roots, reindex, remove and re-add, and preview visibility')
"@ $AppProcessId $Folder $SecondFolder $Exclude $IndexMetadata
if ($LASTEXITCODE -ne 0) {
    throw "Failed to add folder via native UI (exit $LASTEXITCODE)"
}
