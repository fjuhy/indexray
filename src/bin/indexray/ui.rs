use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::ops::Range;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;

use anyhow::{Context, Result};
use windows::Win32::Foundation::{
    COLORREF, ERROR_ALREADY_EXISTS, FILETIME, GetLastError, HANDLE, HGLOBAL, HWND, LPARAM,
    LRESULT, POINT, RECT, WPARAM,
};
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DWMWA_WINDOW_CORNER_PREFERENCE,
    DwmSetWindowAttribute,
};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateFontW, CreateSolidBrush, DEFAULT_CHARSET,
    DeleteObject, FW_BOLD, FW_NORMAL, FillRect, HBRUSH, HDC, HFONT, OUT_DEFAULT_PRECIS, RDW_ERASE,
    RDW_FRAME, RDW_INVALIDATE, RDW_UPDATENOW, RedrawWindow, SetBkColor, SetTextColor,
    UpdateWindow,
};
use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_NORMAL, FileTimeToLocalFileTime};
use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree,
};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
use windows::Win32::System::Ole::CF_UNICODETEXT;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::Accessibility::{CAccPropServices, IAccPropServices, PROPID_ACC_NAME};
use windows::Win32::UI::Controls::RichEdit::{
    CFE_BOLD, CFE_EFFECTS, CFM_BACKCOLOR, CFM_BOLD, CFM_COLOR, CHARFORMAT2W, CHARRANGE,
    EM_EXSETSEL, EM_HIDESELECTION, EM_SETBKGNDCOLOR, EM_SETCHARFORMAT, MSFTEDIT_CLASS, SCF_ALL,
    SCF_SELECTION,
};
use windows::Win32::UI::Controls::{
    EM_SETCUEBANNER, EM_SETMARGINS, EM_SETSEL, ICC_BAR_CLASSES, ICC_LISTVIEW_CLASSES,
    ICC_PROGRESS_CLASS, ICC_STANDARD_CLASSES, INITCOMMONCONTROLSEX, InitCommonControlsEx, LIST_VIEW_ITEM_STATE_FLAGS,
    LVCF_FMT, LVCF_SUBITEM, LVCF_TEXT, LVCF_WIDTH, LVCFMT_LEFT, LVCOLUMNW, LVIF_IMAGE, LVIF_STATE,
    LVIF_TEXT, LVIS_FOCUSED, LVIS_SELECTED, LVITEMW, LVM_ENSUREVISIBLE, LVM_GETNEXTITEM,
    LVM_INSERTCOLUMNW, LVM_SETCOLUMNWIDTH, LVM_SETEXTENDEDLISTVIEWSTYLE, LVM_SETIMAGELIST,
    LVM_SETITEMCOUNT, LVM_SETITEMSTATE, LVN_GETDISPINFOW, LVN_ITEMCHANGED, LVNI_SELECTED,
    LVS_EX_DOUBLEBUFFER, LVS_EX_FULLROWSELECT, LVS_OWNERDATA, LVS_REPORT, LVS_SHAREIMAGELISTS,
    LVS_SHOWSELALWAYS, LVS_SINGLESEL, LVSICF_NOSCROLL, LVSIL_SMALL, NM_DBLCLK, NM_RCLICK,
    NM_RETURN, NMHDR, NMLISTVIEW, NMLVDISPINFOW, SetWindowTheme, WC_LISTVIEWW,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetKeyState, HOT_KEY_MODIFIERS, RegisterHotKey, ReleaseCapture, SetCapture,
    SetFocus, UnregisterHotKey, VK_BACK, VK_CONTROL, VK_DOWN, VK_ESCAPE, VK_F, VK_F1, VK_K, VK_L,
    VK_NEXT, VK_OEM_COMMA, VK_PRIOR, VK_Q, VK_RETURN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::Shell::{
    FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog, IFileOpenDialog,
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    SHFILEINFOW, SHGFI_SMALLICON, SHGFI_SYSICONINDEX, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW,
    SIGDN_FILESYSPATH, Shell_NotifyIconW, ShellExecuteW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ASFW_ANY, AllowSetForegroundWindow, AppendMenuW, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL,
    CB_SETCURSEL, CBN_SELCHANGE, CBS_DROPDOWNLIST, CREATESTRUCTW, CreatePopupMenu,
    CB_GETDROPPEDSTATE, CBN_EDITCHANGE, CBS_DROPDOWN, IsChild,
    CreateWindowExW, DefWindowProcW, DestroyMenu, DestroyWindow, DispatchMessageW, EC_LEFTMARGIN,
    EC_RIGHTMARGIN, EN_CHANGE, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY,
    FindWindowW, GWLP_USERDATA, GetClientRect, GetCursorPos, GetMessageW, GetSystemMetrics,
    GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, HICON, HMENU, ICON_BIG, ICON_SMALL,
    IDC_ARROW, IDC_SIZEWE, IDI_APPLICATION, IsIconic, IsWindowVisible, LoadCursorW, LoadIconW,
    MB_ICONINFORMATION, MB_OK, MessageBoxW,
    MF_GRAYED, MF_SEPARATOR, MF_STRING, MINMAXINFO, MSG, OBJID_CLIENT, PostMessageW,
    PostQuitMessage, RegisterClassExW, RegisterWindowMessageW, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE,
    SW_RESTORE, SW_SHOW, SW_SHOWNORMAL, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetCursor,
    SetForegroundWindow, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, TranslateMessage, WINDOW_EX_STYLE,
    WINDOW_STYLE, WM_APP, WM_CLOSE, WM_COMMAND, WM_CONTEXTMENU, WM_CREATE, WM_CTLCOLOREDIT,
    WM_CTLCOLORSTATIC, WM_DESTROY, WM_DPICHANGED, WM_ERASEBKGND, WM_GETMINMAXINFO, WM_HOTKEY,
    WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_NOTIFY, WM_SETCURSOR,
    WM_SETFONT, WM_SETICON, WM_SETREDRAW, WM_SIZE, WM_SYSKEYDOWN, WNDCLASSEXW, WS_CHILD,
    WS_CLIPCHILDREN, WS_EX_CLIENTEDGE, WS_OVERLAPPEDWINDOW, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};
use windows::core::{PCWSTR, PWSTR, w};

use crate::config;
use crate::engine::{EngineHandle, HitRow, PreviewData, StateSnapshot, UiEvent, WM_APP_EVENT};
use crate::logging::log_line;
use crate::settings_dlg::{
    self, WM_APP_CLOSE_SETTINGS, WM_APP_RELOAD_HOTKEY, WM_APP_STATUS_NOTE,
};

const WM_TRAYICON: u32 = WM_APP + 10;
const WM_APP_SHOW: u32 = WM_APP + 11;
const WM_APP_POST_SHOW: u32 = WM_APP + 12;
const WM_APP_SEARCH_FILTER_CHANGED: u32 = WM_APP + 14;
const WM_APP_SHELL_ICONS: u32 = WM_APP + 13;
const HOTKEY_ID: i32 = 1;
const TRAY_UID: u32 = 1;
const SEARCH_LIMIT: usize = 100;

const ID_SEARCH_EDIT: usize = 1001;
const ID_FILTER_COMBO: usize = 1002;
const ID_SORT_COMBO: usize = 1003;
const ID_QUICK_ADD_BTN: usize = 1004;
const ID_SETTINGS_BTN: usize = 1005;
const ID_RESULTS_LIST: usize = 1006;
const ID_PREVIEW_META: usize = 1007;
const ID_OPEN_FILE_BTN: usize = 1008;
const ID_OPEN_DIR_BTN: usize = 1009;
const ID_COPY_PATH_BTN: usize = 1010;
const ID_PREVIEW_EDIT: usize = 1011;
const ID_NAME_EDIT: usize = 1013;
const ID_BODY_EDIT: usize = 1014;
const ID_EXT_COMBO: usize = 1015;

const SEARCH_FIELDS: [(usize, &str, PCWSTR); 3] = [
    (ID_NAME_EDIT, "파일명", w!("예: 회의록")),
    (ID_BODY_EDIT, "본문", w!("예: 최종 승인")),
    (ID_EXT_COMBO, "확장자", w!("")),
];
const EXT_CHOICES: &[PCWSTR] = &[
    w!("전체"), w!("pdf"), w!("docx"), w!("xlsx"), w!("pptx"), w!("txt"),
    w!("md"), w!("html"), w!("msg"), w!("eml"), w!("pst"), w!("ost"),
];
const ID_STATUS_BAR: usize = 1012;

const CMD_CTX_OPEN: usize = 3001;
const CMD_CTX_REVEAL: usize = 3002;
const CMD_CTX_COPY_PATH: usize = 3003;
const CMD_CTX_COPY_NAME: usize = 3004;

const CMD_TRAY_SHOW: usize = 3101;
const CMD_TRAY_ADD_FOLDER: usize = 3102;
const CMD_TRAY_SETTINGS: usize = 3103;
const CMD_TRAY_QUIT: usize = 3104;

const FILTER_VALUES: [&str; 5] = ["all", "doc", "mail", "code", "image"];
const FILTER_LABELS: [&str; 5] = ["전체 형식", "문서", "메일(PST·MSG)", "코드·설정", "이미지(OCR)"];

const SORT_VALUES: [&str; 4] = ["relevance", "mtime_desc", "mtime_asc", "size_desc"];
const SORT_LABELS: [&str; 4] = ["관련도순", "최신 수정순", "오래된순", "파일 크기순"];

const SEARCH_HELP: &str = "\
검색창 아래에서 파일명, 본문, 확장자를 각각 지정할 수 있습니다.\r\n\
여러 칸을 채우면 모든 조건에 맞는 파일만 표시합니다.\r\n\
확장자는 목록에서 고르거나 pdf,docx처럼 쉼표로 나눠 입력하세요.\r\n\r\n\
검색 범위\r\n\
  name:회의록  파일명에서만 검색\r\n\
  body:\"최종 승인\"  본문에서만 구문 검색\r\n\
  ext:pdf,docx  PDF 또는 DOCX만 표시\r\n\
  path:프로젝트  경로에서 검색\r\n\
  title:보고서  ·  author:홍길동  문서 속성에서 검색\r\n\
  mtime:>=2026-01  ·  size:>1mb  수정일과 크기로 제한\r\n\r\n\
조건 조합\r\n\
  공백과 AND는 모든 조건을, OR는 하나 이상의 조건을 요구합니다.\r\n\
  NOT 또는 -로 제외합니다. 우선순위는 NOT → AND → OR입니다.\r\n\
  예: ext:pdf,docx AND (name:회의록 OR body:\"최종 승인\") -name:초안\r\n\r\n\
관련도순\r\n\
  검색어의 희소성, 등장 횟수, 필드 길이를 반영한 BM25 점수로 정렬합니다.\r\n\
  일반 검색은 입력 중인 접두어도 찾고, 일반 토큰 검색까지 일치하면 가점을 줍니다.";

const BG_COLOR: COLORREF = rgb(244, 246, 249);
const PANEL_COLOR: COLORREF = rgb(255, 255, 255);
const PREVIEW_BG_COLOR: COLORREF = rgb(250, 251, 253);

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

fn scale(px: i32, dpi: u32) -> i32 {
    (px * dpi as i32 + 48) / 96
}

pub fn wide_null(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

pub fn create_ui_font(px_at_96: i32, bold: bool, dpi: u32) -> HFONT {
    let height = -scale(px_at_96, dpi);
    let weight = if bold {
        FW_BOLD.0 as i32
    } else {
        FW_NORMAL.0 as i32
    };
    unsafe {
        CreateFontW(
            height,
            0,
            0,
            0,
            weight,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            0,
            w!("Malgun Gothic"),
        )
    }
}

thread_local! {
    static ACC_SERVICES: Option<IAccPropServices> = unsafe {
        CoCreateInstance(&CAccPropServices, None, CLSCTX_INPROC_SERVER).ok()
    };
}

pub fn set_accessible_name(hwnd: HWND, name: &str) {
    if hwnd.0.is_null() {
        return;
    }
    ACC_SERVICES.with(|opt| {
        if let Some(acc) = opt {
            let wname = wide_null(name);
            unsafe {
                let _ = acc.SetHwndPropStr(
                    hwnd,
                    OBJID_CLIENT.0 as u32,
                    0,
                    PROPID_ACC_NAME,
                    PCWSTR(wname.as_ptr()),
                );
            }
        }
    });
}

pub fn pick_folder_dialog(owner: HWND, title: &str) -> Option<PathBuf> {
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER).ok()?;
        let opts = dialog.GetOptions().ok()?;
        dialog
            .SetOptions(opts | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)
            .ok()?;
        let wtitle = wide_null(title);
        let _ = dialog.SetTitle(PCWSTR(wtitle.as_ptr()));
        let owner_opt = if owner.0.is_null() { None } else { Some(owner) };
        if dialog.Show(owner_opt).is_err() {
            return None;
        }
        let item = dialog.GetResult().ok()?;
        let raw_pwstr = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
        let path_str = pwstr_to_string(raw_pwstr);
        CoTaskMemFree(Some(raw_pwstr.0 as *const core::ffi::c_void));
        if path_str.is_empty() {
            None
        } else {
            Some(PathBuf::from(path_str))
        }
    }
}

unsafe fn pwstr_to_string(ptr: PWSTR) -> String {
    if ptr.0.is_null() {
        return String::new();
    }
    let mut len = 0usize;
    unsafe {
        while *ptr.0.add(len) != 0 {
            len += 1;
        }
        let slice = std::slice::from_raw_parts(ptr.0, len);
        String::from_utf16_lossy(slice)
    }
}

#[derive(Default)]
struct HitsState {
    rows: Vec<HitRow>,
    snippets_inline: Vec<String>,
    total: usize,
    elapsed_ms: f64,
}

/// Re-entrancy-safe window state: scalar/handle fields use `Cell<T>`, heap collections
/// use narrow `RefCell<T>` never held across synchronous Win32 `SendMessageW`/dialog calls.
struct AppView {
    hwnd: Cell<HWND>,
    engine: RefCell<Option<EngineHandle>>,
    last_snapshot: RefCell<Option<StateSnapshot>>,
    settings_panel: Cell<HWND>,
    in_settings: Cell<bool>,
    hotkey_registered: Cell<bool>,
    hotkey_label: RefCell<String>,
    tray_added: Cell<bool>,
    app_icon: HICON,
    taskbar_created_msg: u32,

    dpi: Cell<u32>,
    font_ui: Cell<HFONT>,
    font_ui_bold: Cell<HFONT>,
    font_search: Cell<HFONT>,
    font_preview: Cell<HFONT>,
    bg_brush: HBRUSH,
    panel_brush: HBRUSH,
    preview_bg_brush: HBRUSH,
    richedit_available: Cell<bool>,

    search_edit: Cell<HWND>,
    field_labels: [Cell<HWND>; 3],
    field_inputs: [Cell<HWND>; 3],
    filter_combo: Cell<HWND>,
    sort_combo: Cell<HWND>,
    quick_add_btn: Cell<HWND>,
    settings_btn: Cell<HWND>,
    results_list: Cell<HWND>,
    preview_meta_lbl: Cell<HWND>,
    open_file_btn: Cell<HWND>,
    open_dir_btn: Cell<HWND>,
    copy_path_btn: Cell<HWND>,
    preview_edit: Cell<HWND>,
    status_lbl: Cell<HWND>,

    split_ratio: Cell<f32>,
    dragging_splitter: Cell<bool>,
    splitter_rect: Cell<RECT>,
    suppress_lv_select: Cell<bool>,

    search_token: Cell<u64>,
    preview_token: Cell<u64>,
    current_query: RefCell<String>,
    hits: RefCell<HitsState>,
    selected_idx: Cell<Option<usize>>,
    icon_cache: RefCell<HashMap<String, i32>>,
}

impl AppView {
    fn apply_hotkey(&self, spec: &str) {
        let hwnd = self.hwnd.get();
        unsafe {
            if self.hotkey_registered.get() {
                let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
                self.hotkey_registered.set(false);
            }
            let (mods, vk) = config::parse_win32_hotkey(spec);
            let label = config::format_hotkey_label(spec);
            *self.hotkey_label.borrow_mut() = label.clone();
            match RegisterHotKey(Some(hwnd), HOTKEY_ID, HOT_KEY_MODIFIERS(mods), vk) {
                Ok(()) => {
                    self.hotkey_registered.set(true);
                }
                Err(err) => {
                    log_line("hotkey", &format!("전역 단축키 등록 실패 ({spec}): {err}"));
                }
            }
            self.update_tray_tip(&format!("IndexRay — {label} 열기"));
        }
    }

    unsafe fn add_tray_icon(&self) {
        let tip = format!("IndexRay — {} 열기", self.hotkey_label.borrow());
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd.get(),
            uID: TRAY_UID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: self.app_icon,
            ..Default::default()
        };
        write_fixed_wide(&mut nid.szTip, &tip);
        let added = unsafe { Shell_NotifyIconW(NIM_ADD, &nid).as_bool() };
        self.tray_added.set(added);
    }

    unsafe fn update_tray_tip(&self, tip: &str) {
        if !self.tray_added.get() {
            return;
        }
        let mut nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd.get(),
            uID: TRAY_UID,
            uFlags: NIF_TIP,
            ..Default::default()
        };
        write_fixed_wide(&mut nid.szTip, tip);
        unsafe {
            let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
        }
    }

    unsafe fn remove_tray_icon(&self) {
        if !self.tray_added.get() {
            return;
        }
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd.get(),
            uID: TRAY_UID,
            ..Default::default()
        };
        unsafe {
            let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
        }
        self.tray_added.set(false);
    }

    unsafe fn create_fonts(&self) {
        let dpi = self.dpi.get();
        self.font_ui.set(create_ui_font(13, false, dpi));
        self.font_ui_bold.set(create_ui_font(13, true, dpi));
        self.font_search.set(create_ui_font(16, false, dpi));
        self.font_preview.set(create_ui_font(14, false, dpi));
    }

    unsafe fn destroy_fonts(&self) {
        unsafe {
            for f in [
                self.font_ui.get(),
                self.font_ui_bold.get(),
                self.font_search.get(),
                self.font_preview.get(),
            ] {
                if !f.0.is_null() {
                    let _ = DeleteObject(f.into());
                }
            }
        }
    }

    unsafe fn apply_fonts_to_controls(&self) {
        unsafe {
            let set = |h: HWND, f: HFONT| {
                if !h.0.is_null() {
                    SendMessageW(h, WM_SETFONT, Some(WPARAM(f.0 as usize)), Some(LPARAM(1)));
                }
            };
            set(self.search_edit.get(), self.font_search.get());
            for control in self.field_labels.iter().chain(&self.field_inputs) {
                set(control.get(), self.font_ui.get());
            }
            set(self.filter_combo.get(), self.font_ui.get());
            set(self.sort_combo.get(), self.font_ui.get());
            set(self.quick_add_btn.get(), self.font_ui_bold.get());
            set(self.settings_btn.get(), self.font_ui.get());
            set(self.results_list.get(), self.font_ui.get());
            set(self.preview_meta_lbl.get(), self.font_ui_bold.get());
            set(self.open_file_btn.get(), self.font_ui.get());
            set(self.open_dir_btn.get(), self.font_ui.get());
            set(self.copy_path_btn.get(), self.font_ui.get());
            set(self.preview_edit.get(), self.font_preview.get());
            set(self.status_lbl.get(), self.font_ui.get());

            let margin = scale(8, self.dpi.get()) as usize;
            SendMessageW(
                self.search_edit.get(),
                EM_SETMARGINS,
                Some(WPARAM((EC_LEFTMARGIN | EC_RIGHTMARGIN) as usize)),
                Some(LPARAM((margin | (margin << 16)) as isize)),
            );
        }
    }

    unsafe fn init_controls(&self) {
        let p = self.hwnd.get();
        // SS_NOPREFIX (0x80) | SS_CENTERIMAGE (0x200) | SS_ENDELLIPSIS (0x4000)
        let lbl_style = WS_CHILD | WS_VISIBLE | WINDOW_STYLE(0x0080 | 0x0200 | 0x4000);
        unsafe {
            self.create_fonts();

            let corner_pref: DWM_WINDOW_CORNER_PREFERENCE = DWMWCP_ROUND;
            let _ = DwmSetWindowAttribute(
                p,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &corner_pref as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
            );

            let search_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                0,
                0,
                100,
                32,
                Some(p),
                Some(HMENU(ID_SEARCH_EDIT as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.search_edit.set(search_edit);
            let cue = w!("전체 검색  ·  F1 검색 도움말");
            SendMessageW(
                search_edit,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(cue.as_ptr() as isize)),
            );

            for (i, &(id, label, cue)) in SEARCH_FIELDS.iter().enumerate() {
                let label = wide_null(label);
                self.field_labels[i].set(CreateWindowExW(
                    WINDOW_EX_STYLE(0), w!("STATIC"), PCWSTR(label.as_ptr()), lbl_style,
                    0, 0, 0, 0, Some(p), None, None, None,
                ).unwrap_or_default());
                let combo = id == ID_EXT_COMBO;
                let (class, edge, style) = if combo {
                    (w!("COMBOBOX"), WINDOW_EX_STYLE(0), CBS_DROPDOWN as u32 | WS_VSCROLL.0)
                } else {
                    (w!("EDIT"), WS_EX_CLIENTEDGE, ES_AUTOHSCROLL as u32)
                };
                let input = CreateWindowExW(
                    edge, class, w!(""), WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(style),
                    0, 0, 100, 28, Some(p), Some(HMENU(id as *mut core::ffi::c_void)), None, None,
                ).unwrap_or_default();
                self.field_inputs[i].set(input);
                if combo {
                    for choice in EXT_CHOICES {
                        SendMessageW(input, CB_ADDSTRING, None, Some(LPARAM(choice.as_ptr() as isize)));
                    }
                    SendMessageW(input, CB_SETCURSEL, Some(WPARAM(0)), None);
                } else {
                    SendMessageW(input, EM_SETCUEBANNER, None, Some(LPARAM(cue.as_ptr() as isize)));
                }
            }

            let filter_combo = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("COMBOBOX"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                0,
                0,
                100,
                240,
                Some(p),
                Some(HMENU(ID_FILTER_COMBO as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.filter_combo.set(filter_combo);
            for label in FILTER_LABELS {
                let wl = wide_null(label);
                SendMessageW(
                    filter_combo,
                    CB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(wl.as_ptr() as isize)),
                );
            }
            SendMessageW(filter_combo, CB_SETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));

            let sort_combo = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("COMBOBOX"),
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(CBS_DROPDOWNLIST as u32),
                0,
                0,
                100,
                240,
                Some(p),
                Some(HMENU(ID_SORT_COMBO as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.sort_combo.set(sort_combo);
            for label in SORT_LABELS {
                let wl = wide_null(label);
                SendMessageW(
                    sort_combo,
                    CB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(wl.as_ptr() as isize)),
                );
            }
            SendMessageW(sort_combo, CB_SETCURSEL, Some(WPARAM(0)), Some(LPARAM(0)));

            let quick_add_btn = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("+ 폴더 추가"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                100,
                32,
                Some(p),
                Some(HMENU(ID_QUICK_ADD_BTN as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.quick_add_btn.set(quick_add_btn);

            let settings_btn = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("색인 폴더 및 설정"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                130,
                32,
                Some(p),
                Some(HMENU(ID_SETTINGS_BTN as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.settings_btn.set(settings_btn);

            let lv_style = WS_CHILD
                | WS_VISIBLE
                | WS_TABSTOP
                | WINDOW_STYLE(
                    LVS_REPORT
                        | LVS_SINGLESEL
                        | LVS_SHOWSELALWAYS
                        | LVS_OWNERDATA
                        | LVS_SHAREIMAGELISTS,
                );
            let results_list = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                WC_LISTVIEWW,
                w!(""),
                lv_style,
                0,
                0,
                300,
                300,
                Some(p),
                Some(HMENU(ID_RESULTS_LIST as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.results_list.set(results_list);
            let ex_lv = (LVS_EX_FULLROWSELECT | LVS_EX_DOUBLEBUFFER) as usize;
            SendMessageW(
                results_list,
                LVM_SETEXTENDEDLISTVIEWSTYLE,
                Some(WPARAM(ex_lv)),
                Some(LPARAM(ex_lv as isize)),
            );
            let _ = SetWindowTheme(results_list, w!("Explorer"), None);

            let dpi = self.dpi.get();
            for (idx, (title, width)) in [
                ("파일명", 220),
                ("일치 내용", 240),
                ("폴더 경로", 200),
            ]
            .into_iter()
            .enumerate()
            {
                let mut wtitle = wide_null(title);
                let col = LVCOLUMNW {
                    mask: LVCF_FMT | LVCF_WIDTH | LVCF_TEXT | LVCF_SUBITEM,
                    fmt: LVCFMT_LEFT,
                    cx: scale(width, dpi),
                    pszText: PWSTR(wtitle.as_mut_ptr()),
                    iSubItem: idx as i32,
                    ..Default::default()
                };
                SendMessageW(
                    results_list,
                    LVM_INSERTCOLUMNW,
                    Some(WPARAM(idx)),
                    Some(LPARAM(&col as *const _ as isize)),
                );
            }

            let preview_meta_lbl = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("선택된 문서 없음"),
                lbl_style,
                0,
                0,
                200,
                24,
                Some(p),
                Some(HMENU(ID_PREVIEW_META as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.preview_meta_lbl.set(preview_meta_lbl);

            let open_file_btn = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("열기"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                64,
                26,
                Some(p),
                Some(HMENU(ID_OPEN_FILE_BTN as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.open_file_btn.set(open_file_btn);

            let open_dir_btn = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("폴더"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                64,
                26,
                Some(p),
                Some(HMENU(ID_OPEN_DIR_BTN as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.open_dir_btn.set(open_dir_btn);

            let copy_path_btn = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                w!("경로 복사"),
                WS_CHILD | WS_VISIBLE | WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                0,
                0,
                84,
                26,
                Some(p),
                Some(HMENU(ID_COPY_PATH_BTN as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.copy_path_btn.set(copy_path_btn);

            let status_lbl = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                w!("색인 상태를 확인하는 중…"),
                lbl_style,
                0,
                0,
                400,
                24,
                Some(p),
                Some(HMENU(ID_STATUS_BAR as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.status_lbl.set(status_lbl);

            for ctrl in [
                self.field_inputs[2].get(),
                filter_combo,
                sort_combo,
                quick_add_btn,
                settings_btn,
                open_file_btn,
                open_dir_btn,
                copy_path_btn,
            ] {
                let _ = SetWindowTheme(ctrl, w!("Explorer"), None);
            }

            self.apply_fonts_to_controls();
            self.update_action_buttons();
            self.layout();
        }
    }

    /// Runs immediately after first paint + search_edit focus so heavy OS DLLs
    /// (`Msftedit.dll`, `shell32` icon cache, `oleacc` MSAA) never delay window appearance.
    unsafe fn finish_post_show_init(&self) {
        let p = self.hwnd.get();
        *self.engine.borrow_mut() = Some(EngineHandle::start(p));

        let hwnd_raw = p.0 as isize;
        let _ = std::thread::Builder::new()
            .name("indexray-icons".into())
            .spawn(move || unsafe {
                let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
                let mut sfi = SHFILEINFOW::default();
                let sys_himl = SHGetFileInfoW(
                    w!("file.txt"),
                    FILE_ATTRIBUTE_NORMAL,
                    Some(&mut sfi),
                    std::mem::size_of::<SHFILEINFOW>() as u32,
                    SHGFI_SYSICONINDEX | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES,
                );
                if sys_himl != 0 {
                    let _ = PostMessageW(
                        Some(HWND(hwnd_raw as *mut core::ffi::c_void)),
                        WM_APP_SHELL_ICONS,
                        WPARAM(sys_himl),
                        LPARAM(0),
                    );
                }
            });

        unsafe {
            let rich_ok = LoadLibraryW(w!("Msftedit.dll")).is_ok();
            self.richedit_available.set(rich_ok);
            let preview_cls = if rich_ok { MSFTEDIT_CLASS } else { w!("EDIT") };
            let vis_flag = if self.in_settings.get() {
                WINDOW_STYLE(0)
            } else {
                WS_VISIBLE
            };
            let preview_edit = CreateWindowExW(
                WS_EX_CLIENTEDGE,
                preview_cls,
                w!(""),
                WS_CHILD
                    | vis_flag
                    | WS_VSCROLL
                    | WS_TABSTOP
                    | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_READONLY) as u32),
                0,
                0,
                300,
                300,
                Some(p),
                Some(HMENU(ID_PREVIEW_EDIT as *mut core::ffi::c_void)),
                None,
                None,
            )
            .unwrap_or_default();
            self.preview_edit.set(preview_edit);
            SendMessageW(
                preview_edit,
                WM_SETFONT,
                Some(WPARAM(self.font_preview.get().0 as usize)),
                Some(LPARAM(1)),
            );
            if rich_ok {
                SendMessageW(
                    preview_edit,
                    EM_SETBKGNDCOLOR,
                    Some(WPARAM(0)),
                    Some(LPARAM(PREVIEW_BG_COLOR.0 as isize)),
                );
                SendMessageW(preview_edit, EM_HIDESELECTION, Some(WPARAM(0)), Some(LPARAM(0)));
            }
            self.layout();
            if self.selected_idx.get().is_none() {
                self.render_empty_preview();
            }

            self.add_tray_icon();

            for (input, (_, name, _)) in self.field_inputs.iter().zip(SEARCH_FIELDS) {
                set_accessible_name(input.get(), name);
            }
            for (ctrl, name) in [
                (self.search_edit.get(), "문서 검색"),
                (self.filter_combo.get(), "유형 필터"),
                (self.sort_combo.get(), "정렬 기준"),
                (self.quick_add_btn.get(), "빠른 폴더 추가"),
                (self.settings_btn.get(), "색인 폴더 및 설정"),
                (self.results_list.get(), "검색 결과"),
                (self.preview_meta_lbl.get(), "선택 문서 정보"),
                (self.open_file_btn.get(), "원본 열기"),
                (self.open_dir_btn.get(), "파일 탐색기에서 보기"),
                (self.copy_path_btn.get(), "전체 경로 복사"),
                (preview_edit, "본문 미리보기"),
                (self.status_lbl.get(), "상태 표시줄"),
            ] {
                set_accessible_name(ctrl, name);
            }
        }
    }

    fn ensure_settings_panel(&self) -> HWND {
        let cur = self.settings_panel.get();
        if !cur.0.is_null() {
            return cur;
        }
        let Some(admin_tx) = self.engine.borrow().as_ref().map(EngineHandle::admin_sender) else {
            return HWND(null_mut());
        };
        let panel = settings_dlg::create_panel(self.hwnd.get(), admin_tx, self.dpi.get());
        self.settings_panel.set(panel);
        unsafe { self.layout() };
        if let Some(snap) = self.last_snapshot.borrow().clone() {
            settings_dlg::populate_panel(panel, &snap);
        }
        panel
    }

    fn set_settings_visible(&self, show: bool) {
        if show == self.in_settings.get() {
            return;
        }
        let panel = if show {
            let p = self.ensure_settings_panel();
            if p.0.is_null() {
                return;
            }
            p
        } else {
            self.settings_panel.get()
        };

        self.in_settings.set(show);
        let search_cmd = if show { SW_HIDE } else { SW_SHOW };
        let panel_cmd = if show { SW_SHOW } else { SW_HIDE };

        unsafe {
            for control in self.field_labels.iter().chain(&self.field_inputs) {
                let _ = ShowWindow(control.get(), search_cmd);
            }
            for ctrl in [
                self.search_edit.get(),
                self.filter_combo.get(),
                self.sort_combo.get(),
                self.quick_add_btn.get(),
                self.settings_btn.get(),
                self.results_list.get(),
                self.preview_meta_lbl.get(),
                self.open_file_btn.get(),
                self.open_dir_btn.get(),
                self.copy_path_btn.get(),
                self.preview_edit.get(),
            ] {
                if !ctrl.0.is_null() {
                    let _ = ShowWindow(ctrl, search_cmd);
                }
            }
            if !panel.0.is_null() {
                let _ = ShowWindow(panel, panel_cmd);
            }
            self.layout();
            let _ = RedrawWindow(
                Some(self.hwnd.get()),
                None,
                None,
                RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW,
            );
            if show {
                settings_dlg::focus_default(panel);
            } else {
                let _ = SetFocus(Some(self.search_edit.get()));
            }
        }
    }

    unsafe fn layout(&self) {
        let hwnd = self.hwnd.get();
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(hwnd, &mut rc);
        }
        let w = rc.right - rc.left;
        let h = rc.bottom - rc.top;
        if w <= 40 || h <= 40 {
            return;
        }
        let d = self.dpi.get();
        let pad = scale(12, d);
        let gap = scale(8, d);
        let top_h = scale(34, d);
        let status_h = scale(24, d);
        let status_y = (h - pad - status_h).max(scale(100, d));
        let total_body_w = (w - pad * 2).max(scale(200, d));

        let mv = |ctrl: HWND, x: i32, y: i32, cw: i32, ch: i32| unsafe {
            if !ctrl.0.is_null() {
                let _ = SetWindowPos(
                    ctrl,
                    None,
                    x,
                    y,
                    cw.max(4),
                    ch.max(4),
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        };

        let panel = self.settings_panel.get();
        if !panel.0.is_null() {
            mv(panel, 0, 0, w, (status_y - scale(4, d)).max(scale(80, d)));
        }

        let btn_settings_w = scale(138, d);
        let btn_add_w = scale(98, d);
        let combo_w = scale(118, d);

        let right_controls_w = combo_w * 2 + btn_add_w + btn_settings_w + gap * 4;
        let search_w = (w - pad * 2 - right_controls_w).max(scale(160, d));

        let mut x = pad;
        let top_y = pad;
        mv(self.search_edit.get(), x, top_y, search_w, top_h);
        x += search_w + gap;
        mv(
            self.filter_combo.get(),
            x,
            top_y + scale(2, d),
            combo_w,
            scale(260, d),
        );
        x += combo_w + gap;
        mv(
            self.sort_combo.get(),
            x,
            top_y + scale(2, d),
            combo_w,
            scale(260, d),
        );
        x += combo_w + gap;
        mv(self.quick_add_btn.get(), x, top_y, btn_add_w, top_h);
        x += btn_add_w + gap;
        mv(self.settings_btn.get(), x, top_y, btn_settings_w, top_h);

        let fields_y = top_y + top_h + gap;
        let field_h = scale(28, d);
        let label_w = scale(48, d);
        let ext_w = scale(175, d);
        let text_w = (total_body_w - ext_w - gap * 2) / 2;
        let mut x = pad;
        for (i, width) in [text_w, text_w, ext_w].into_iter().enumerate() {
            mv(self.field_labels[i].get(), x, fields_y, label_w, field_h);
            let h = if i == 2 { scale(260, d) } else { field_h };
            mv(self.field_inputs[i].get(), x + label_w, fields_y, width - label_w, h);
            x += width + gap;
        }
        let body_top = fields_y + field_h + gap;
        let body_h = (status_y - body_top - gap).max(scale(60, d));
        let split_w = scale(6, d);

        let left_w = (((total_body_w - split_w) as f32) * self.split_ratio.get().clamp(0.28, 0.72))
            .round() as i32;
        let right_w = (total_body_w - split_w - left_w).max(scale(140, d));

        let left_x = pad;
        let split_x = left_x + left_w;
        let right_x = split_x + split_w;

        self.splitter_rect.set(RECT {
            left: split_x,
            top: body_top,
            right: right_x,
            bottom: body_top + body_h,
        });

        let lv = self.results_list.get();
        mv(lv, left_x, body_top, left_w, body_h);

        let usable_lv = (left_w - scale(22, d)).max(scale(180, d));
        let col0 = (usable_lv * 34 / 100).max(scale(120, d));
        let col1 = (usable_lv * 38 / 100).max(scale(120, d));
        let col2 = (usable_lv - col0 - col1).max(scale(90, d));
        unsafe {
            SendMessageW(lv, LVM_SETCOLUMNWIDTH, Some(WPARAM(0)), Some(LPARAM(col0 as isize)));
            SendMessageW(lv, LVM_SETCOLUMNWIDTH, Some(WPARAM(1)), Some(LPARAM(col1 as isize)));
            SendMessageW(lv, LVM_SETCOLUMNWIDTH, Some(WPARAM(2)), Some(LPARAM(col2 as isize)));
        }

        let meta_bar_h = scale(28, d);
        let act_open_w = scale(58, d);
        let act_dir_w = scale(58, d);
        let act_copy_w = scale(78, d);
        let acts_total = act_open_w + act_dir_w + act_copy_w + scale(4, d) * 2;
        let meta_lbl_w = (right_w - acts_total - gap).max(scale(60, d));

        mv(
            self.preview_meta_lbl.get(),
            right_x,
            body_top + scale(2, d),
            meta_lbl_w,
            meta_bar_h - scale(2, d),
        );
        let mut ax = right_x + right_w - acts_total;
        mv(self.open_file_btn.get(), ax, body_top, act_open_w, meta_bar_h);
        ax += act_open_w + scale(4, d);
        mv(self.open_dir_btn.get(), ax, body_top, act_dir_w, meta_bar_h);
        ax += act_dir_w + scale(4, d);
        mv(self.copy_path_btn.get(), ax, body_top, act_copy_w, meta_bar_h);

        let preview_top = body_top + meta_bar_h + scale(6, d);
        let preview_h = (body_h - meta_bar_h - scale(6, d)).max(scale(40, d));
        mv(self.preview_edit.get(), right_x, preview_top, right_w, preview_h);

        mv(self.status_lbl.get(), pad, status_y, total_body_w, status_h);
    }

    fn current_filter(&self) -> &'static str {
        let idx = unsafe {
            SendMessageW(
                self.filter_combo.get(),
                CB_GETCURSEL,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
            .0
        };
        FILTER_VALUES
            .get(idx.max(0) as usize)
            .copied()
            .unwrap_or("all")
    }

    fn current_sort(&self) -> &'static str {
        let idx = unsafe {
            SendMessageW(
                self.sort_combo.get(),
                CB_GETCURSEL,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
            .0
        };
        SORT_VALUES
            .get(idx.max(0) as usize)
            .copied()
            .unwrap_or("relevance")
    }

    fn trigger_search(&self) {
        let mut query = get_window_string(self.search_edit.get());
        let [name, body, ext] = self.field_inputs.each_ref().map(|input| get_window_string(input.get()));
        let ext = if ext.trim() == "전체" { "" } else { ext.trim() };
        let fields = [("name", name.trim()), ("body", body.trim()), ("ext", ext)];
        if fields.iter().any(|(_, value)| !value.is_empty()) {
            if query.trim().is_empty() {
                query.clear();
            } else {
                query.insert(0, '(');
                query.push(')');
            }
            for (field, value) in fields.into_iter().filter(|(_, value)| !value.is_empty()) {
                if !query.is_empty() {
                    query.push(' ');
                }
                query.push_str(field);
                query.push_str(":\"");
                for ch in value.chars() {
                    if matches!(ch, '\\' | '"') {
                        query.push('\\');
                    }
                    query.push(ch);
                }
                query.push('"');
            }
        }
        *self.current_query.borrow_mut() = query.clone();
        let token = self.search_token.get() + 1;
        self.search_token.set(token);

        if query.trim().is_empty() {
            self.apply_search_results(token, Vec::new(), 0, 0.0);
            return;
        }
        let filter = self.current_filter().to_string();
        let sort = self.current_sort().to_string();
        if let Some(eng) = self.engine.borrow().as_ref() {
            eng.request_search(token, query, filter, sort, SEARCH_LIMIT);
        }
    }

    fn apply_search_results(
        &self,
        token: u64,
        rows: Vec<HitRow>,
        total: usize,
        elapsed_ms: f64,
    ) {
        if token != self.search_token.get() {
            return;
        }
        let count = rows.len();
        {
            let mut hs = self.hits.borrow_mut();
            hs.snippets_inline = rows.iter().map(|h| collapse_single_line(&h.snippet)).collect();
            hs.rows = rows;
            hs.total = total;
            hs.elapsed_ms = elapsed_ms;
        }

        let lv = self.results_list.get();
        unsafe {
            SendMessageW(
                lv,
                LVM_SETITEMCOUNT,
                Some(WPARAM(count)),
                Some(LPARAM(LVSICF_NOSCROLL as isize)),
            );
            let _ = RedrawWindow(Some(lv), None, None, RDW_INVALIDATE | RDW_ERASE);
        }

        if count == 0 {
            self.selected_idx.set(None);
            self.update_action_buttons();
            self.render_empty_preview();
        } else {
            self.select_list_row(0);
        }
        self.refresh_idle_or_search_status();
    }

    fn select_list_row(&self, index: usize) {
        let count = self.hits.borrow().rows.len();
        if index >= count {
            return;
        }
        let lv = self.results_list.get();
        let sel_mask = LIST_VIEW_ITEM_STATE_FLAGS(LVIS_SELECTED.0 | LVIS_FOCUSED.0);
        self.suppress_lv_select.set(true);
        unsafe {
            if let Some(prev) = self.selected_idx.get()
                && prev != index
            {
                let clear_item = LVITEMW {
                    stateMask: sel_mask,
                    state: LIST_VIEW_ITEM_STATE_FLAGS(0),
                    ..Default::default()
                };
                SendMessageW(
                    lv,
                    LVM_SETITEMSTATE,
                    Some(WPARAM(prev)),
                    Some(LPARAM(&clear_item as *const _ as isize)),
                );
            }
            let sel_item = LVITEMW {
                stateMask: sel_mask,
                state: sel_mask,
                ..Default::default()
            };
            SendMessageW(
                lv,
                LVM_SETITEMSTATE,
                Some(WPARAM(index)),
                Some(LPARAM(&sel_item as *const _ as isize)),
            );
            SendMessageW(lv, LVM_ENSUREVISIBLE, Some(WPARAM(index)), Some(LPARAM(0)));
        }
        self.suppress_lv_select.set(false);
        self.on_selection_changed(Some(index));
    }

    fn move_selection(&self, delta: isize) {
        let count = self.hits.borrow().rows.len();
        if count == 0 {
            return;
        }
        let cur = self.selected_idx.get().unwrap_or(0) as isize;
        let max_idx = (count as isize) - 1;
        let next = (cur + delta).clamp(0, max_idx) as usize;
        self.select_list_row(next);
    }

    fn on_selection_changed(&self, next_idx: Option<usize>) {
        self.selected_idx.set(next_idx);
        self.update_action_buttons();

        let Some(idx) = next_idx else {
            self.render_empty_preview();
            return;
        };
        let Some(hit) = self.hits.borrow().rows.get(idx).cloned() else {
            self.render_empty_preview();
            return;
        };

        let mut meta_parts = vec![hit.file_name.clone()];
        if let Some(title) = &hit.title
            && !title.is_empty()
            && !hit.file_name.starts_with(title)
        {
            meta_parts.push(title.clone());
        }
        if let Some(author) = &hit.author
            && !author.is_empty()
        {
            meta_parts.push(format!("작성자: {author}"));
        }
        if !hit.parent_dir.is_empty() {
            meta_parts.push(hit.parent_dir.clone());
        }
        set_window_string(self.preview_meta_lbl.get(), &meta_parts.join("  ·  "));

        self.render_highlighted_preview(&hit.snippet, &hit.highlighted);

        let token = self.preview_token.get() + 1;
        self.preview_token.set(token);
        let query = self.current_query.borrow().clone();
        if let Some(eng) = self.engine.borrow().as_ref() {
            eng.request_preview(token, hit.raw_path, query);
        }
    }

    fn apply_preview_data(&self, token: u64, data: Option<PreviewData>) {
        if token != self.preview_token.get() {
            return;
        }
        let Some(data) = data else {
            return;
        };
        let Some(idx) = self.selected_idx.get() else {
            return;
        };
        let Some(hit) = self.hits.borrow().rows.get(idx).cloned() else {
            return;
        };
        if hit.raw_path != data.raw_path {
            return;
        }

        let mut meta_parts = vec![hit.file_name.clone(), format_bytes(data.size)];
        if let Some(ts) = format_mtime(data.mtime) {
            meta_parts.push(ts);
        }
        if let Some(title) = &hit.title
            && !title.is_empty()
            && !hit.file_name.starts_with(title)
        {
            meta_parts.push(title.clone());
        }
        if let Some(author) = &hit.author
            && !author.is_empty()
        {
            meta_parts.push(format!("작성자 {author}"));
        }
        if !hit.parent_dir.is_empty() {
            meta_parts.push(hit.parent_dir);
        }
        set_window_string(self.preview_meta_lbl.get(), &meta_parts.join("  ·  "));
        self.render_highlighted_preview(&data.snippet, &data.highlighted);
    }

    fn render_empty_preview(&self) {
        set_window_string(self.preview_meta_lbl.get(), "선택된 문서 없음");
        let query_empty = self.current_query.borrow().trim().is_empty();
        let hk = self.hotkey_label.borrow().clone();
        let snap_opt = self.last_snapshot.borrow().clone();

        let guide = if let Some(snap) = snap_opt {
            if !snap.has_index {
                "아직 색인된 문서 폴더가 없습니다.\r\n\r\n\
                 1. 상단의 [+ 폴더 추가] 버튼으로 색인할 폴더를 고르세요.\r\n\
                 2. 설정에서 제외 폴더와 확장자를 확인한 뒤 [규칙 적용 및 새 폴더 색인 시작]을 누르세요.\r\n\
                 3. 지원 형식: PDF, DOCX, XLSX, PPTX, MD, TXT 및 코드·설정 파일, 이미지 OCR"
                    .to_string()
            } else if query_empty {
                let mut msg = format!(
                    "총 {}개 문서가 즉시 검색 준비 상태입니다 ({}개 폴더 등록됨).\r\n\r\n\
                     검색 문법 안내:\r\n\
                     • 파일명과 본문을 따로 입력하고 확장자를 고르세요.\r\n\
                     • 여러 칸을 채우면 모든 조건에 맞는 파일을 찾습니다.\r\n\
                     • F1을 누르면 고급 검색과 관련도순 기준을 확인할 수 있습니다.\r\n\r\n\
                     단축키:\r\n\
                     • {hk} : 어디서나 창 열기/숨기기\r\n\
                     • ↑ / ↓ : 검색창 포커스를 유지한 채 결과 이동\r\n\
                     • Enter : 원본 파일 열기  ·  Ctrl+Enter : 파일 탐색기에서 위치 열기\r\n\
                     • Ctrl+Shift+C : 전체 경로 복사  ·  Ctrl+, : 설정 열기  ·  Ctrl+Q : 완전 종료",
                    format_num(snap.num_docs),
                    snap.roots.len(),
                );
                if !snap.ko_reindex_roots.is_empty() {
                    msg.push_str(
                        "\r\n\r\n[안내] 한국어 형태소 사전(ko-dic) 구성 변경이 감지되었습니다. \
                         [색인 폴더 및 설정]에서 폴더를 다시 색인하면 한국어 검색 정확도가 향상됩니다.",
                    );
                }
                msg
            } else {
                "일치하는 문서가 없습니다. 검색어나 유형 필터를 변경해 보세요.".to_string()
            }
        } else {
            "색인 상태를 불러오는 중입니다…".to_string()
        };
        self.render_highlighted_preview(&guide, &[]);
    }

    fn render_highlighted_preview(&self, text: &str, ranges: &[Range<usize>]) {
        let pe = self.preview_edit.get();
        if pe.0.is_null() {
            return;
        }
        let (normalized_text, utf16_ranges) = build_richedit_text_and_ranges(text, ranges);
        unsafe {
            // WM_SETREDRAW(TRUE) can add WS_VISIBLE even to a hidden control.
            let visible = IsWindowVisible(pe).as_bool();
            if visible {
                SendMessageW(pe, WM_SETREDRAW, Some(WPARAM(0)), Some(LPARAM(0)));
            }
            set_window_string(pe, &normalized_text);

            if self.richedit_available.get() {
                let mut base_fmt = CHARFORMAT2W::default();
                base_fmt.Base.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
                base_fmt.Base.dwMask = CFM_COLOR | CFM_BACKCOLOR | CFM_BOLD;
                base_fmt.Base.dwEffects = CFE_EFFECTS(0);
                base_fmt.Base.crTextColor = rgb(30, 41, 59);
                base_fmt.crBackColor = PREVIEW_BG_COLOR;
                SendMessageW(
                    pe,
                    EM_SETCHARFORMAT,
                    Some(WPARAM(SCF_ALL as usize)),
                    Some(LPARAM(&base_fmt as *const _ as isize)),
                );

                if !utf16_ranges.is_empty() {
                    let mut hi_fmt = CHARFORMAT2W::default();
                    hi_fmt.Base.cbSize = std::mem::size_of::<CHARFORMAT2W>() as u32;
                    hi_fmt.Base.dwMask = CFM_COLOR | CFM_BACKCOLOR | CFM_BOLD;
                    hi_fmt.Base.dwEffects = CFE_BOLD;
                    hi_fmt.Base.crTextColor = rgb(15, 23, 42);
                    hi_fmt.crBackColor = rgb(254, 240, 138);

                    for (cp_min, cp_max) in utf16_ranges {
                        let cr = CHARRANGE {
                            cpMin: cp_min,
                            cpMax: cp_max,
                        };
                        SendMessageW(
                            pe,
                            EM_EXSETSEL,
                            Some(WPARAM(0)),
                            Some(LPARAM(&cr as *const _ as isize)),
                        );
                        SendMessageW(
                            pe,
                            EM_SETCHARFORMAT,
                            Some(WPARAM(SCF_SELECTION as usize)),
                            Some(LPARAM(&hi_fmt as *const _ as isize)),
                        );
                    }
                }

                let top = CHARRANGE { cpMin: 0, cpMax: 0 };
                SendMessageW(
                    pe,
                    EM_EXSETSEL,
                    Some(WPARAM(0)),
                    Some(LPARAM(&top as *const _ as isize)),
                );
            }

            if visible {
                SendMessageW(pe, WM_SETREDRAW, Some(WPARAM(1)), Some(LPARAM(0)));
                let _ = RedrawWindow(
                    Some(pe),
                    None,
                    None,
                    RDW_INVALIDATE | RDW_ERASE | RDW_FRAME | RDW_UPDATENOW,
                );
            }
        }
    }

    fn update_action_buttons(&self) {
        let has_sel = self.selected_idx.get().is_some();
        unsafe {
            let _ = EnableWindow(self.open_file_btn.get(), has_sel);
            let _ = EnableWindow(self.open_dir_btn.get(), has_sel);
            let _ = EnableWindow(self.copy_path_btn.get(), has_sel);
        }
    }

    fn refresh_idle_or_search_status(&self) {
        let query_empty = self.current_query.borrow().trim().is_empty();
        if query_empty {
            let snap_opt = self.last_snapshot.borrow().clone();
            if let Some(snap) = snap_opt {
                if !snap.has_index {
                    set_window_string(
                        self.status_lbl.get(),
                        "등록된 색인 폴더가 없습니다 · [+ 폴더 추가]를 눌러 시작하세요",
                    );
                } else {
                    let hk = self.hotkey_label.borrow().clone();
                    let mut parts = vec![format!(
                        "문서 {}개 준비됨 ({}개 폴더)",
                        format_num(snap.num_docs),
                        snap.roots.len()
                    )];
                    if !snap.export_status.is_empty() {
                        parts.push(format!("Markdown: {}", snap.export_status));
                    }
                    parts.push(format!("F1 검색 도움말 · {hk} 호출 · Ctrl+, 설정"));
                    set_window_string(self.status_lbl.get(), &parts.join("  ·  "));
                }
            }
        } else {
            let (shown, total, ms) = {
                let hs = self.hits.borrow();
                (hs.rows.len(), hs.total, hs.elapsed_ms)
            };
            let line = if total > shown {
                format!(
                    "상위 {shown}개 표시 / 전체 {}개 ({ms:.1}ms)  ·  Enter 열기 · Ctrl+Enter 폴더 · Esc 초기화",
                    format_num(total as u64),
                )
            } else {
                format!(
                    "검색 결과 {}개 ({ms:.1}ms)  ·  Enter 열기 · Ctrl+Enter 폴더 · Esc 초기화",
                    format_num(total as u64),
                )
            };
            set_window_string(self.status_lbl.get(), &line);
        }
    }

    fn icon_index_for_ext(&self, ext: &str) -> i32 {
        if let Some(&idx) = self.icon_cache.borrow().get(ext) {
            return idx;
        }
        let fake = if ext.is_empty() {
            "file".to_string()
        } else {
            format!("file.{ext}")
        };
        let wfake = wide_null(&fake);
        let mut sfi = SHFILEINFOW::default();
        let ok = unsafe {
            SHGetFileInfoW(
                PCWSTR(wfake.as_ptr()),
                FILE_ATTRIBUTE_NORMAL,
                Some(&mut sfi),
                std::mem::size_of::<SHFILEINFOW>() as u32,
                SHGFI_SYSICONINDEX | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES,
            )
        };
        let idx = if ok != 0 { sfi.iIcon } else { 0 };
        self.icon_cache.borrow_mut().insert(ext.to_string(), idx);
        idx
    }

    fn selected_hit_cloned(&self) -> Option<HitRow> {
        let idx = self.selected_idx.get()?;
        self.hits.borrow().rows.get(idx).cloned()
    }

    fn open_selected_file(&self) {
        let Some(hit) = self.selected_hit_cloned() else {
            return;
        };
        let target_raw = hit
            .raw_path
            .split_once("::")
            .map_or(hit.raw_path.as_str(), |(base, _)| base);
        let wpath = wide_null(target_raw);
        unsafe {
            let res = ShellExecuteW(
                Some(self.hwnd.get()),
                w!("open"),
                PCWSTR(wpath.as_ptr()),
                None,
                None,
                SW_SHOWNORMAL,
            );
            if (res.0 as isize) <= 32 {
                let wdisp = wide_null(&hit.display_path);
                let _ = ShellExecuteW(
                    Some(self.hwnd.get()),
                    w!("open"),
                    PCWSTR(wdisp.as_ptr()),
                    None,
                    None,
                    SW_SHOWNORMAL,
                );
            }
        }
    }

    fn reveal_selected_in_explorer(&self) {
        let Some(hit) = self.selected_hit_cloned() else {
            return;
        };
        let target = if Path::new(&hit.display_path).exists() {
            &hit.display_path
        } else {
            &hit.raw_path
        };
        let arg = format!("/select,\"{target}\"");
        let _ = std::process::Command::new("explorer.exe").raw_arg(arg).spawn();
    }

    fn copy_selected_path(&self, name_only: bool) {
        let Some(hit) = self.selected_hit_cloned() else {
            return;
        };
        let val = if name_only {
            &hit.file_name
        } else {
            &hit.display_path
        };
        if copy_to_clipboard(self.hwnd.get(), val) {
            set_window_string(
                self.status_lbl.get(),
                &format!("클립보드에 복사됨: {val}"),
            );
        }
    }

    fn quick_add_folder(&self) {
        self.set_settings_visible(true);
        let panel = self.settings_panel.get();
        if !panel.0.is_null()
            && let Some(folder) = pick_folder_dialog(self.hwnd.get(), "색인에 추가할 문서 폴더 선택")
        {
            settings_dlg::stage_folder(panel, &folder);
        }
    }

    unsafe fn show_and_focus(&self) {
        let hwnd = self.hwnd.get();
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            } else {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
            let _ = SetForegroundWindow(hwnd);
            if self.in_settings.get() {
                settings_dlg::focus_default(self.settings_panel.get());
            } else {
                let se = self.search_edit.get();
                let _ = SetFocus(Some(se));
                SendMessageW(se, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
            }
        }
    }

    fn drain_engine_events(&self) {
        loop {
            let next_ev = self.engine.borrow().as_ref().and_then(EngineHandle::try_recv);
            let Some(ev) = next_ev else {
                break;
            };
            match ev {
                UiEvent::StateReady(snap) => {
                    let first_load = self.last_snapshot.borrow().is_none();
                    let hk = snap.config.hotkey.clone();
                    *self.last_snapshot.borrow_mut() = Some(snap.clone());
                    if first_load {
                        self.apply_hotkey(&hk);
                    }
                    let panel = self.settings_panel.get();
                    if !panel.0.is_null() {
                        settings_dlg::populate_panel(panel, &snap);
                    }
                    if self.current_query.borrow().trim().is_empty() {
                        self.render_empty_preview();
                        self.refresh_idle_or_search_status();
                    } else {
                        self.trigger_search();
                    }
                }
                UiEvent::SearchDone {
                    token,
                    hits,
                    total,
                    elapsed_ms,
                } => {
                    self.apply_search_results(token, hits, total, elapsed_ms);
                }
                UiEvent::SearchError { token, message } => {
                    if token == self.search_token.get() {
                        self.apply_search_results(token, Vec::new(), 0, 0.0);
                        set_window_string(self.status_lbl.get(), &message);
                    }
                }
                UiEvent::PreviewDone { token, data } => {
                    self.apply_preview_data(token, data);
                }
                UiEvent::StatusLine(line) => {
                    set_window_string(self.status_lbl.get(), &line);
                }
                UiEvent::AdminDone { result, summary } => {
                    let panel = self.settings_panel.get();
                    if !panel.0.is_null() {
                        settings_dlg::finish_panel_operation(panel, result.is_ok());
                    }
                    match result {
                        Ok(snap) => {
                            *self.last_snapshot.borrow_mut() = Some(snap.clone());
                            if !panel.0.is_null() {
                                settings_dlg::populate_panel(panel, &snap);
                            }
                            if !summary.is_empty() {
                                set_window_string(self.status_lbl.get(), &summary);
                            } else {
                                self.refresh_idle_or_search_status();
                            }
                            if self.current_query.borrow().trim().is_empty() {
                                self.render_empty_preview();
                            } else {
                                self.trigger_search();
                            }
                        }
                        Err(err_msg) => {
                            set_window_string(self.status_lbl.get(), &format!("오류: {err_msg}"));
                        }
                    }
                }
            }
        }
    }

    unsafe fn show_results_context_menu(&self, screen_pt: POINT) {
        let has_sel = self.selected_idx.get().is_some();
        unsafe {
            let Ok(menu) = CreatePopupMenu() else {
                return;
            };
            let state_flag = if has_sel {
                MF_STRING
            } else {
                MF_STRING | MF_GRAYED
            };
            let _ = AppendMenuW(menu, state_flag, CMD_CTX_OPEN, w!("원본 열기\tEnter"));
            let _ = AppendMenuW(
                menu,
                state_flag,
                CMD_CTX_REVEAL,
                w!("파일 탐색기에서 보기\tCtrl+Enter"),
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                menu,
                state_flag,
                CMD_CTX_COPY_PATH,
                w!("전체 경로 복사\tCtrl+Shift+C"),
            );
            let _ = AppendMenuW(menu, state_flag, CMD_CTX_COPY_NAME, w!("파일 이름 복사"));

            let cmd = TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD,
                screen_pt.x,
                screen_pt.y,
                None,
                self.hwnd.get(),
                None,
            );
            let _ = DestroyMenu(menu);

            match cmd.0 as usize {
                CMD_CTX_OPEN => self.open_selected_file(),
                CMD_CTX_REVEAL => self.reveal_selected_in_explorer(),
                CMD_CTX_COPY_PATH => self.copy_selected_path(false),
                CMD_CTX_COPY_NAME => self.copy_selected_path(true),
                _ => {}
            }
        }
    }

    unsafe fn show_tray_menu(&self) {
        unsafe {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let Ok(menu) = CreatePopupMenu() else {
                return;
            };
            let open_label = wide_null(&format!("IndexRay 열기 ({})", self.hotkey_label.borrow()));
            let _ = AppendMenuW(menu, MF_STRING, CMD_TRAY_SHOW, PCWSTR(open_label.as_ptr()));
            let _ = AppendMenuW(menu, MF_STRING, CMD_TRAY_ADD_FOLDER, w!("색인 폴더 추가…"));
            let _ = AppendMenuW(menu, MF_STRING, CMD_TRAY_SETTINGS, w!("색인 폴더 및 설정…"));
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(menu, MF_STRING, CMD_TRAY_QUIT, w!("종료"));

            let hwnd = self.hwnd.get();
            let _ = SetForegroundWindow(hwnd);
            let cmd = TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD,
                pt.x,
                pt.y,
                None,
                hwnd,
                None,
            );
            let _ = DestroyMenu(menu);

            match cmd.0 as usize {
                CMD_TRAY_SHOW => self.show_and_focus(),
                CMD_TRAY_ADD_FOLDER => {
                    self.show_and_focus();
                    self.quick_add_folder();
                }
                CMD_TRAY_SETTINGS => {
                    self.show_and_focus();
                    self.set_settings_visible(true);
                }
                CMD_TRAY_QUIT => {
                    let _ = DestroyWindow(hwnd);
                }
                _ => {}
            }
        }
    }
}

pub fn run() -> Result<()> {
    let scope = config::instance_scope_id();
    let class_str = format!("IndexRayMain_{scope}");
    let class_wide = wide_null(&class_str);
    let mutex_str = format!("Local\\IndexRay.Instance.{scope}");
    let mutex_wide = wide_null(&mutex_str);

    unsafe {
        let _mutex_handle: HANDLE = CreateMutexW(None, true, PCWSTR(mutex_wide.as_ptr()))
            .context("인스턴스 뮤텍스 생성 실패")?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            if let Ok(existing) = FindWindowW(PCWSTR(class_wide.as_ptr()), None)
                && !existing.0.is_null()
            {
                let _ = AllowSetForegroundWindow(ASFW_ANY);
                let _ = PostMessageW(Some(existing), WM_APP_SHOW, WPARAM(0), LPARAM(0));
            }
            return Ok(());
        }

        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let icc = INITCOMMONCONTROLSEX {
            dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_LISTVIEW_CLASSES | ICC_BAR_CLASSES | ICC_PROGRESS_CLASS | ICC_STANDARD_CLASSES,
        };
        let _ = InitCommonControlsEx(&icc);

        let hinst = GetModuleHandleW(None).context("모듈 핸들 확인 실패")?;
        let app_icon = LoadIconW(Some(hinst.into()), PCWSTR(std::ptr::without_provenance(1)))
            .or_else(|_| LoadIconW(None, IDI_APPLICATION))
            .unwrap_or_default();

        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(main_wndproc),
            hInstance: hinst.into(),
            hIcon: app_icon,
            hIconSm: app_icon,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: PCWSTR(class_wide.as_ptr()),
            ..Default::default()
        };
        let atom = RegisterClassExW(&wc);
        anyhow::ensure!(atom != 0, "창 클래스 등록 실패");

        let screen_w = GetSystemMetrics(SM_CXSCREEN);
        let screen_h = GetSystemMetrics(SM_CYSCREEN);
        let init_w = 1060.min((screen_w - 60).max(720));
        let init_h = 720.min((screen_h - 60).max(560));
        let init_x = ((screen_w - init_w) / 2).max(20);
        let init_y = ((screen_h - init_h) / 2).max(20);

        let taskbar_created_msg = RegisterWindowMessageW(w!("TaskbarCreated"));

        let shared = Box::new(AppView {
            hwnd: Cell::new(HWND(null_mut())),
            engine: RefCell::new(None),
            last_snapshot: RefCell::new(None),
            settings_panel: Cell::new(HWND(null_mut())),
            in_settings: Cell::new(false),
            hotkey_registered: Cell::new(false),
            hotkey_label: RefCell::new("Alt+`".to_string()),
            tray_added: Cell::new(false),
            app_icon,
            taskbar_created_msg,
            dpi: Cell::new(96),
            font_ui: Cell::new(HFONT(null_mut())),
            font_ui_bold: Cell::new(HFONT(null_mut())),
            font_search: Cell::new(HFONT(null_mut())),
            font_preview: Cell::new(HFONT(null_mut())),
            bg_brush: CreateSolidBrush(BG_COLOR),
            panel_brush: CreateSolidBrush(PANEL_COLOR),
            preview_bg_brush: CreateSolidBrush(PREVIEW_BG_COLOR),
            richedit_available: Cell::new(false),
            search_edit: Cell::new(HWND(null_mut())),
            field_labels: std::array::from_fn(|_| Cell::new(HWND(null_mut()))),
            field_inputs: std::array::from_fn(|_| Cell::new(HWND(null_mut()))),
            filter_combo: Cell::new(HWND(null_mut())),
            sort_combo: Cell::new(HWND(null_mut())),
            quick_add_btn: Cell::new(HWND(null_mut())),
            settings_btn: Cell::new(HWND(null_mut())),
            results_list: Cell::new(HWND(null_mut())),
            preview_meta_lbl: Cell::new(HWND(null_mut())),
            open_file_btn: Cell::new(HWND(null_mut())),
            open_dir_btn: Cell::new(HWND(null_mut())),
            copy_path_btn: Cell::new(HWND(null_mut())),
            preview_edit: Cell::new(HWND(null_mut())),
            status_lbl: Cell::new(HWND(null_mut())),
            split_ratio: Cell::new(0.52),
            dragging_splitter: Cell::new(false),
            splitter_rect: Cell::new(RECT::default()),
            suppress_lv_select: Cell::new(false),
            search_token: Cell::new(0),
            preview_token: Cell::new(0),
            current_query: RefCell::new(String::new()),
            hits: RefCell::new(HitsState::default()),
            selected_idx: Cell::new(None),
            icon_cache: RefCell::new(HashMap::with_capacity(64)),
        });

        let raw_shared = Box::into_raw(shared) as *const AppView;
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_wide.as_ptr()),
            w!("IndexRay"),
            WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
            init_x,
            init_y,
            init_w,
            init_h,
            None,
            None,
            Some(hinst.into()),
            Some(raw_shared as *const core::ffi::c_void),
        )
        .context("메인 창 생성 실패")?;

        SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_BIG as usize)),
            Some(LPARAM(app_icon.0 as isize)),
        );
        SendMessageW(
            hwnd,
            WM_SETICON,
            Some(WPARAM(ICON_SMALL as usize)),
            Some(LPARAM(app_icon.0 as isize)),
        );

        // Paint window and focus search input BEFORE loading optional DLLs / worker threads.
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
        let _ = SetFocus(Some((*raw_shared).search_edit.get()));
        let _ = PostMessageW(Some(hwnd), WM_APP_POST_SHOW, WPARAM(0), LPARAM(0));

        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret.0 <= 0 {
                break;
            }
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AppView;
            if !ptr.is_null() && handle_accelerators(&*ptr, &msg) {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

fn key_down(vk: i32) -> bool {
    unsafe { (GetKeyState(vk) as u16 & 0x8000) != 0 }
}

unsafe fn delete_prev_word(edit: HWND) {
    let text = get_window_string(edit);
    if text.is_empty() {
        return;
    }
    let mut sel_start: u32 = 0;
    let mut sel_end: u32 = 0;
    unsafe {
        SendMessageW(
            edit,
            windows::Win32::UI::Controls::EM_GETSEL,
            Some(WPARAM(&mut sel_start as *mut u32 as usize)),
            Some(LPARAM(&mut sel_end as *mut u32 as isize)),
        );
    }
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let end = (sel_end as usize).min(utf16.len());
    let start = (sel_start as usize).min(end);
    if start != end {
        let mut next = Vec::with_capacity(utf16.len());
        next.extend_from_slice(&utf16[..start]);
        next.extend_from_slice(&utf16[end..]);
        next.push(0);
        unsafe {
            let _ = SetWindowTextW(edit, PCWSTR(next.as_ptr()));
            SendMessageW(edit, EM_SETSEL, Some(WPARAM(start)), Some(LPARAM(start as isize)));
        }
        return;
    }
    if start == 0 {
        return;
    }
    let mut cut = start;
    while cut > 0 && char::from_u32(utf16[cut - 1] as u32).is_some_and(char::is_whitespace) {
        cut -= 1;
    }
    while cut > 0 && !char::from_u32(utf16[cut - 1] as u32).is_some_and(char::is_whitespace) {
        cut -= 1;
    }
    let mut next = Vec::with_capacity(utf16.len());
    next.extend_from_slice(&utf16[..cut]);
    next.extend_from_slice(&utf16[start..]);
    next.push(0);
    unsafe {
        let _ = SetWindowTextW(edit, PCWSTR(next.as_ptr()));
        SendMessageW(edit, EM_SETSEL, Some(WPARAM(cut)), Some(LPARAM(cut as isize)));
    }
}

unsafe fn handle_accelerators(app: &AppView, msg: &MSG) -> bool {
    if msg.message != WM_KEYDOWN && msg.message != WM_SYSKEYDOWN {
        return false;
    }

    let vk = msg.wParam.0 as u16;
    // Never interfere with active IME composition (Korean Hangul assembly).
    if vk == 0xE5 {
        return false;
    }

    let ctrl = key_down(VK_CONTROL.0 as i32);
    let shift = key_down(VK_SHIFT.0 as i32);

    if ctrl && !shift && vk == VK_Q.0 {
        unsafe {
            let _ = DestroyWindow(app.hwnd.get());
        }
        return true;
    }

    if ctrl && !shift && vk == VK_OEM_COMMA.0 {
        app.set_settings_visible(!app.in_settings.get());
        return true;
    }

    if app.in_settings.get() {
        if vk == VK_ESCAPE.0 {
            app.set_settings_visible(false);
            return true;
        }
        if settings_dlg::handle_panel_keydown(app.settings_panel.get(), msg.hwnd, vk) {
            return true;
        }
        return false;
    }

    if vk == VK_F1.0 {
        let help = wide_null(SEARCH_HELP);
        unsafe {
            MessageBoxW(
                Some(app.hwnd.get()), PCWSTR(help.as_ptr()), w!("검색 도움말"),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        return true;
    }

    let se = app.search_edit.get();
    let [name, body, ext] = app.field_inputs.each_ref().map(Cell::get);
    let in_ext = msg.hwnd == ext || unsafe { IsChild(ext, msg.hwnd).as_bool() };
    let in_search_text = [se, name, body].contains(&msg.hwnd);
    if in_ext && unsafe { SendMessageW(ext, CB_GETDROPPEDSTATE, None, None).0 != 0 } {
        return false;
    }
    if !ctrl && vk == VK_TAB.0 {
        let fields = [se, name, body, ext];
        let current = if in_ext { ext } else { msg.hwnd };
        if let Some(i) = fields.iter().position(|&hwnd| hwnd == current) {
            let next = fields[(i + if shift { 3 } else { 1 }) % fields.len()];
            unsafe { let _ = SetFocus(Some(next)); }
            return true;
        }
    }

    if ctrl && !shift && (vk == VK_K.0 || vk == VK_F.0 || vk == VK_L.0) {
        unsafe {
            let _ = SetFocus(Some(se));
            SendMessageW(se, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
        }
        return true;
    }

    if ctrl && !shift && vk == VK_BACK.0 && in_search_text {
        unsafe { delete_prev_word(msg.hwnd) };
        return true;
    }

    if ctrl && shift && vk == b'C' as u16 && msg.hwnd != app.preview_edit.get() {
        app.copy_selected_path(false);
        return true;
    }

    if vk == VK_ESCAPE.0 {
        if !app.current_query.borrow().trim().is_empty() {
            for input in [se, name, body] {
                set_window_string(input, "");
            }
            unsafe { SendMessageW(ext, CB_SETCURSEL, Some(WPARAM(0)), None); }
            app.trigger_search();
            unsafe {
                let _ = SetFocus(Some(se));
            }
        } else if app.tray_added.get() {
            unsafe {
                let _ = ShowWindow(app.hwnd.get(), SW_HIDE);
            }
        }
        return true;
    }

    if vk == VK_RETURN.0 && (in_search_text || in_ext || msg.hwnd == app.results_list.get()) {
        if ctrl {
            app.reveal_selected_in_explorer();
        } else {
            app.open_selected_file();
        }
        return true;
    }

    if in_search_text && !ctrl {
        match vk {
            v if v == VK_DOWN.0 => {
                app.move_selection(1);
                return true;
            }
            v if v == VK_UP.0 => {
                app.move_selection(-1);
                return true;
            }
            v if v == VK_NEXT.0 => {
                app.move_selection(10);
                return true;
            }
            v if v == VK_PRIOR.0 => {
                app.move_selection(-10);
                return true;
            }
            _ => {}
        }
    }

    false
}

unsafe extern "system" fn main_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let ptr = cs.lpCreateParams as *const AppView;
        unsafe {
            (*ptr).hwnd.set(hwnd);
            let d = GetDpiForWindow(hwnd);
            (*ptr).dpi.set(if d == 0 { 96 } else { d });
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
        }
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }

    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const AppView };
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let app = unsafe { &*ptr };

    if app.taskbar_created_msg != 0 && msg == app.taskbar_created_msg {
        unsafe { app.add_tray_icon() };
        return LRESULT(0);
    }

    match msg {
        WM_CREATE => {
            unsafe { app.init_controls() };
            LRESULT(0)
        }
        WM_APP_SEARCH_FILTER_CHANGED => {
            app.trigger_search();
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            let mut rc = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rc);
                FillRect(hdc, &rc, app.bg_brush);
            }
            LRESULT(1)
        }
        WM_SIZE => {
            unsafe { app.layout() };
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let mmi = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            let d = app.dpi.get();
            mmi.ptMinTrackSize.x = scale(720, d);
            mmi.ptMinTrackSize.y = scale(560, d);
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let new_dpi = (wparam.0 & 0xFFFF) as u32;
            let resolved_dpi = if new_dpi == 0 { 96 } else { new_dpi };
            app.dpi.set(resolved_dpi);
            unsafe {
                app.destroy_fonts();
                app.create_fonts();
                app.apply_fonts_to_controls();
                let panel = app.settings_panel.get();
                if !panel.0.is_null() {
                    settings_dlg::update_dpi(panel, resolved_dpi);
                }
                let suggested = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                app.layout();
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let hit_hwnd = HWND(wparam.0 as *mut core::ffi::c_void);
            if hit_hwnd == hwnd && !app.in_settings.get() {
                let mut pt = POINT::default();
                unsafe {
                    let _ = GetCursorPos(&mut pt);
                    let _ = windows::Win32::Graphics::Gdi::ScreenToClient(hwnd, &mut pt);
                    if pt_in_rect(&app.splitter_rect.get(), pt)
                        && let Ok(cur) = LoadCursorW(None, IDC_SIZEWE)
                    {
                        SetCursor(Some(cur));
                        return LRESULT(1);
                    }
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_LBUTTONDOWN => {
            if !app.in_settings.get() {
                let pt = POINT {
                    x: (lparam.0 & 0xFFFF) as i16 as i32,
                    y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                };
                if pt_in_rect(&app.splitter_rect.get(), pt) {
                    app.dragging_splitter.set(true);
                    unsafe {
                        SetCapture(hwnd);
                    }
                    return LRESULT(0);
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_MOUSEMOVE => {
            if app.dragging_splitter.get() {
                let x = (lparam.0 & 0xFFFF) as i16 as i32;
                let mut rc = RECT::default();
                unsafe {
                    let _ = GetClientRect(hwnd, &mut rc);
                }
                let pad = scale(12, app.dpi.get());
                let total_w = (rc.right - rc.left - pad * 2).max(200) as f32;
                let rel = ((x - pad) as f32 / total_w).clamp(0.28, 0.72);
                if (rel - app.split_ratio.get()).abs() > 0.003 {
                    app.split_ratio.set(rel);
                    unsafe {
                        app.layout();
                        let _ = RedrawWindow(
                            Some(hwnd),
                            None,
                            None,
                            RDW_INVALIDATE | RDW_ERASE | RDW_UPDATENOW,
                        );
                    }
                }
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_LBUTTONUP => {
            if app.dragging_splitter.get() {
                app.dragging_splitter.set(false);
                unsafe {
                    let _ = ReleaseCapture();
                }
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            let child = HWND(lparam.0 as *mut core::ffi::c_void);
            unsafe {
                let pe = app.preview_edit.get();
                if !pe.0.is_null() && child == pe && !app.richedit_available.get() {
                    SetBkColor(hdc, PREVIEW_BG_COLOR);
                    SetTextColor(hdc, rgb(30, 41, 59));
                    return LRESULT(app.preview_bg_brush.0 as isize);
                }
                SetBkColor(hdc, BG_COLOR);
                SetTextColor(hdc, rgb(30, 41, 59));
            }
            LRESULT(app.bg_brush.0 as isize)
        }
        WM_CTLCOLOREDIT => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            unsafe {
                SetBkColor(hdc, PANEL_COLOR);
                SetTextColor(hdc, rgb(15, 23, 42));
            }
            LRESULT(app.panel_brush.0 as isize)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            let code = ((wparam.0 >> 16) & 0xFFFF) as u32;
            match id {
                ID_SEARCH_EDIT | ID_NAME_EDIT | ID_BODY_EDIT if code == EN_CHANGE => {
                    app.trigger_search();
                }
                ID_EXT_COMBO if code == CBN_EDITCHANGE => app.trigger_search(),
                ID_EXT_COMBO if code == CBN_SELCHANGE => {
                    // The edit text is updated after CBN_SELCHANGE returns.
                    unsafe { let _ = PostMessageW(Some(hwnd), WM_APP_SEARCH_FILTER_CHANGED, WPARAM(0), LPARAM(0)); }
                }
                ID_FILTER_COMBO | ID_SORT_COMBO if code == CBN_SELCHANGE => {
                    app.trigger_search();
                }
                ID_QUICK_ADD_BTN if code == 0 => {
                    app.quick_add_folder();
                }
                ID_SETTINGS_BTN if code == 0 => {
                    app.set_settings_visible(true);
                }
                ID_OPEN_FILE_BTN if code == 0 => {
                    app.open_selected_file();
                }
                ID_OPEN_DIR_BTN if code == 0 => {
                    app.reveal_selected_in_explorer();
                }
                ID_COPY_PATH_BTN if code == 0 => {
                    app.copy_selected_path(false);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_NOTIFY => {
            let hdr = unsafe { &*(lparam.0 as *const NMHDR) };
            if hdr.idFrom == ID_RESULTS_LIST {
                match hdr.code {
                    LVN_GETDISPINFOW => {
                        let disp = unsafe { &mut *(lparam.0 as *mut NMLVDISPINFOW) };
                        let idx = disp.item.iItem as usize;
                        let mut ext_for_icon: Option<String> = None;
                        {
                            let hs = app.hits.borrow();
                            if let Some(hit) = hs.rows.get(idx) {
                                if (disp.item.mask & LVIF_TEXT) == LVIF_TEXT
                                    && !disp.item.pszText.0.is_null()
                                    && disp.item.cchTextMax > 0
                                {
                                    let src: &str = match disp.item.iSubItem {
                                        0 => &hit.file_name,
                                        1 => hs
                                            .snippets_inline
                                            .get(idx)
                                            .map(String::as_str)
                                            .unwrap_or(""),
                                        2 => &hit.parent_dir,
                                        _ => "",
                                    };
                                    unsafe {
                                        copy_utf16_truncated(
                                            src,
                                            disp.item.pszText.0,
                                            disp.item.cchTextMax as usize,
                                        );
                                    }
                                }
                                if disp.item.iSubItem == 0
                                    && (disp.item.mask & LVIF_IMAGE) == LVIF_IMAGE
                                {
                                    ext_for_icon = Some(hit.ext.clone());
                                }
                            }
                        }
                        if let Some(ext) = ext_for_icon {
                            disp.item.iImage = app.icon_index_for_ext(&ext);
                        }
                        return LRESULT(0);
                    }
                    LVN_ITEMCHANGED => {
                        if !app.suppress_lv_select.get() {
                            let nmlv = unsafe { &*(lparam.0 as *const NMLISTVIEW) };
                            if nmlv.iItem >= 0 && (nmlv.uChanged & LVIF_STATE) == LVIF_STATE {
                                let was_sel = (nmlv.uOldState & LVIS_SELECTED.0) != 0;
                                let now_sel = (nmlv.uNewState & LVIS_SELECTED.0) != 0;
                                if !was_sel && now_sel {
                                    let target = nmlv.iItem as usize;
                                    if app.selected_idx.get() != Some(target) {
                                        app.on_selection_changed(Some(target));
                                    }
                                } else if was_sel && !now_sel {
                                    let next = unsafe {
                                        SendMessageW(
                                            app.results_list.get(),
                                            LVM_GETNEXTITEM,
                                            Some(WPARAM(usize::MAX)),
                                            Some(LPARAM(LVNI_SELECTED as isize)),
                                        )
                                        .0
                                    };
                                    if next < 0 {
                                        app.on_selection_changed(None);
                                    }
                                }
                            }
                        }
                        return LRESULT(0);
                    }
                    NM_DBLCLK | NM_RETURN => {
                        app.open_selected_file();
                        return LRESULT(0);
                    }
                    NM_RCLICK => {
                        let mut pt = POINT::default();
                        unsafe {
                            let _ = GetCursorPos(&mut pt);
                            app.show_results_context_menu(pt);
                        }
                        return LRESULT(0);
                    }
                    _ => {}
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_CONTEXTMENU => {
            let target_hwnd = HWND(wparam.0 as *mut core::ffi::c_void);
            if target_hwnd == app.results_list.get() {
                let mut pt = POINT {
                    x: (lparam.0 & 0xFFFF) as i16 as i32,
                    y: ((lparam.0 >> 16) & 0xFFFF) as i16 as i32,
                };
                unsafe {
                    if pt.x == -1 && pt.y == -1 {
                        let _ = GetCursorPos(&mut pt);
                    }
                    app.show_results_context_menu(pt);
                }
                return LRESULT(0);
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_APP_EVENT => {
            app.drain_engine_events();
            LRESULT(0)
        }
        WM_APP_SHOW => {
            unsafe { app.show_and_focus() };
            LRESULT(0)
        }
        WM_APP_POST_SHOW => {
            unsafe { app.finish_post_show_init() };
            LRESULT(0)
        }
        WM_APP_SHELL_ICONS => {
            let sys_himl = wparam.0 as isize;
            if sys_himl != 0 {
                let lv = app.results_list.get();
                unsafe {
                    SendMessageW(
                        lv,
                        LVM_SETIMAGELIST,
                        Some(WPARAM(LVSIL_SMALL as usize)),
                        Some(LPARAM(sys_himl)),
                    );
                    let _ = RedrawWindow(Some(lv), None, None, RDW_INVALIDATE);
                }
            }
            LRESULT(0)
        }
        WM_APP_CLOSE_SETTINGS => {
            app.set_settings_visible(false);
            LRESULT(0)
        }
        WM_APP_RELOAD_HOTKEY => {
            let cfg = config::load();
            app.apply_hotkey(&cfg.hotkey);
            if let Some(snap) = app.last_snapshot.borrow_mut().as_mut() {
                snap.config = cfg;
                snap.autostart_enabled = config::is_autostart_enabled();
            }
            LRESULT(0)
        }
        WM_APP_STATUS_NOTE => {
            if let Some(note) = settings_dlg::take_status_note(app.settings_panel.get()) {
                set_window_string(app.status_lbl.get(), &note);
            }
            LRESULT(0)
        }
        WM_HOTKEY => {
            if wparam.0 as i32 == HOTKEY_ID {
                unsafe {
                    if IsWindowVisible(hwnd).as_bool() && !IsIconic(hwnd).as_bool() {
                        if app.tray_added.get() {
                            let _ = ShowWindow(hwnd, SW_HIDE);
                        } else {
                            app.show_and_focus();
                        }
                    } else {
                        app.show_and_focus();
                    }
                }
            }
            LRESULT(0)
        }
        WM_TRAYICON => {
            let ev = (lparam.0 & 0xFFFF) as u32;
            match ev {
                windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONUP
                | windows::Win32::UI::WindowsAndMessaging::WM_LBUTTONDBLCLK => unsafe {
                    app.show_and_focus();
                },
                windows::Win32::UI::WindowsAndMessaging::WM_RBUTTONUP | WM_CONTEXTMENU => unsafe {
                    app.show_tray_menu();
                },
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            if app.tray_added.get() {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
            } else {
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                if app.hotkey_registered.get() {
                    let _ = UnregisterHotKey(Some(hwnd), HOTKEY_ID);
                    app.hotkey_registered.set(false);
                }
                app.remove_tray_icon();
                let mut eng_opt = app.engine.borrow_mut().take();
                if let Some(mut eng) = eng_opt.take() {
                    eng.shutdown();
                }
                app.destroy_fonts();
                let _ = DeleteObject(app.bg_brush.into());
                let _ = DeleteObject(app.panel_brush.into());
                let _ = DeleteObject(app.preview_bg_brush.into());
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(ptr as *mut AppView));
                PostQuitMessage(0);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn pt_in_rect(rc: &RECT, pt: POINT) -> bool {
    pt.x >= rc.left && pt.x < rc.right && pt.y >= rc.top && pt.y < rc.bottom
}

fn write_fixed_wide(dst: &mut [u16], text: &str) {
    dst.fill(0);
    if dst.is_empty() {
        return;
    }
    let max = dst.len() - 1;
    let mut i = 0usize;
    for ch in text.chars() {
        let len = ch.len_utf16();
        if i + len > max {
            break;
        }
        let mut tmp = [0u16; 2];
        ch.encode_utf16(&mut tmp);
        dst[i..i + len].copy_from_slice(&tmp[..len]);
        i += len;
    }
}

unsafe fn copy_utf16_truncated(src: &str, dst: *mut u16, max_units: usize) {
    if max_units == 0 {
        return;
    }
    let limit = max_units - 1;
    let mut i = 0usize;
    for ch in src.chars() {
        let len = ch.len_utf16();
        if i + len > limit {
            break;
        }
        let mut tmp = [0u16; 2];
        ch.encode_utf16(&mut tmp);
        unsafe {
            *dst.add(i) = tmp[0];
            if len == 2 {
                *dst.add(i + 1) = tmp[1];
            }
        }
        i += len;
    }
    unsafe {
        *dst.add(i) = 0;
    }
}

fn get_window_string(hwnd: HWND) -> String {
    if hwnd.0.is_null() {
        return String::new();
    }
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; (len as usize) + 1];
        let copied = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..copied as usize])
    }
}

fn set_window_string(hwnd: HWND, text: &str) {
    if hwnd.0.is_null() {
        return;
    }
    let w = wide_null(text);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(w.as_ptr()));
    }
}

fn copy_to_clipboard(owner: HWND, text: &str) -> bool {
    let wide = wide_null(text);
    let byte_len = wide.len() * 2;
    unsafe {
        if OpenClipboard(Some(owner)).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let Ok(hmem): windows::core::Result<HGLOBAL> = GlobalAlloc(GMEM_MOVEABLE, byte_len) else {
            let _ = CloseClipboard();
            return false;
        };
        let ptr = GlobalLock(hmem) as *mut u16;
        if ptr.is_null() {
            let _ = CloseClipboard();
            return false;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
        let _ = GlobalUnlock(hmem);
        let ok = SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hmem.0))).is_ok();
        let _ = CloseClipboard();
        ok
    }
}

fn collapse_single_line(s: &str) -> String {
    let mut out = String::with_capacity(s.len().min(200));
    let mut prev_ws = false;
    for ch in s.chars() {
        if ch.is_whitespace() {
            if !prev_ws && !out.is_empty() {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(ch);
            prev_ws = false;
            if out.len() >= 220 {
                break;
            }
        }
    }
    out
}

/// Normalizes line breaks to `\r\n` for Win32/RichEdit display and translates
/// UTF-8 byte highlight ranges into RichEdit character positions (`CHARRANGE`),
/// where each `\r\n` paragraph break counts as 1 RichEdit character (`\r`).
fn build_richedit_text_and_ranges(
    text: &str,
    ranges: &[Range<usize>],
) -> (String, Vec<(i32, i32)>) {
    let mut out = String::with_capacity(text.len() + 32);
    let mut cp_ranges = Vec::with_capacity(ranges.len());
    let mut cursor = 0usize;
    let mut cp: i32 = 0;

    let append_chunk = |chunk: &str, out: &mut String, cp: &mut i32| {
        let mut chars = chunk.chars().peekable();
        while let Some(ch) = chars.next() {
            if ch == '\r' {
                if chars.peek() == Some(&'\n') {
                    let _ = chars.next();
                }
                out.push_str("\r\n");
                *cp += 1;
            } else if ch == '\n' {
                out.push_str("\r\n");
                *cp += 1;
            } else {
                out.push(ch);
                *cp += ch.len_utf16() as i32;
            }
        }
    };

    for r in ranges {
        if r.start >= r.end || r.end > text.len() || r.start < cursor {
            continue;
        }
        if !text.is_char_boundary(r.start) || !text.is_char_boundary(r.end) {
            continue;
        }
        if r.start > cursor {
            append_chunk(&text[cursor..r.start], &mut out, &mut cp);
        }
        let start_cp = cp;
        append_chunk(&text[r.start..r.end], &mut out, &mut cp);
        if cp > start_cp {
            cp_ranges.push((start_cp, cp));
        }
        cursor = r.end;
    }

    if cursor < text.len() {
        append_chunk(&text[cursor..], &mut out, &mut cp);
    }

    (out, cp_ranges)
}

fn format_num(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i != 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    if (bytes as f64) >= MB {
        format!("{:.1} MB", (bytes as f64) / MB)
    } else if (bytes as f64) >= KB {
        format!("{:.1} KB", (bytes as f64) / KB)
    } else {
        format!("{bytes} B")
    }
}

fn format_mtime(ms: i64) -> Option<String> {
    const UNIX_EPOCH_FT_TICKS: u64 = 116_444_736_000_000_000;
    if ms <= 0 {
        return None;
    }
    let utc_ticks = (ms as u64)
        .saturating_mul(10_000)
        .saturating_add(UNIX_EPOCH_FT_TICKS);
    let utc_ft = FILETIME {
        dwLowDateTime: utc_ticks as u32,
        dwHighDateTime: (utc_ticks >> 32) as u32,
    };
    let mut local_ft = FILETIME::default();
    unsafe { FileTimeToLocalFileTime(&utc_ft, &mut local_ft).ok()? };
    let local_ticks = ((local_ft.dwHighDateTime as u64) << 32) | (local_ft.dwLowDateTime as u64);
    let local_secs = local_ticks.checked_sub(UNIX_EPOCH_FT_TICKS)? / 10_000_000;
    let days = (local_secs / 86_400) as i64;
    let rem = local_secs % 86_400;
    let hour = rem / 3600;
    let min = (rem % 3600) / 60;
    let (year, month, day) = civil_from_days(days);
    Some(format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}"))
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
