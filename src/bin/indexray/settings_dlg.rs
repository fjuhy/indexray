use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::mpsc::Sender;

use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateSolidBrush, DeleteObject, FillRect, HBRUSH, HDC, HFONT, SetBkColor, SetTextColor,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    EM_SETCUEBANNER, PBM_SETMARQUEE, PBS_MARQUEE, PROGRESS_CLASSW, SetWindowTheme,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, SetFocus, VK_RETURN};
use windows::Win32::UI::WindowsAndMessaging::{
    BM_GETCHECK, BM_SETCHECK, BS_AUTOCHECKBOX, BS_PUSHBUTTON, CREATESTRUCTW, CreateWindowExW,
    DefWindowProcW, ES_AUTOHSCROLL, ES_AUTOVSCROLL, ES_MULTILINE, ES_NUMBER, GWLP_USERDATA,
    GetClientRect, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW, HMENU, IDC_ARROW,
    IsWindow, LB_ADDSTRING, LB_GETCURSEL, LB_RESETCONTENT, LB_SETCURSEL, LBS_NOTIFY, LoadCursorW,
    PostMessageW, RegisterClassExW, SWP_NOACTIVATE, SWP_NOZORDER, SendMessageW, SetWindowLongPtrW,
    SetWindowPos, SetWindowTextW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_COMMAND, WM_CTLCOLOREDIT,
    WM_CTLCOLORLISTBOX, WM_CTLCOLORSTATIC, WM_DESTROY, WM_ERASEBKGND, WM_NCCREATE, WM_SETFONT,
    WM_SIZE, WNDCLASSEXW, WS_CHILD, WS_CLIPCHILDREN, WS_EX_CLIENTEDGE, WS_TABSTOP, WS_VISIBLE,
    WS_VSCROLL,
    SW_HIDE, SW_SHOWNA, ShowWindow,
};
use windows::core::{PCWSTR, w};

use crate::config;
use crate::engine::{AdminCmd, StateSnapshot};
use crate::ui::{create_ui_font, pick_folder_dialog, set_accessible_name, wide_null};

pub const WM_APP_RELOAD_HOTKEY: u32 = 0x8000 + 44;
pub const WM_APP_CLOSE_SETTINGS: u32 = 0x8000 + 45;
pub const WM_APP_STATUS_NOTE: u32 = 0x8000 + 46;

const ID_BACK_BTN: usize = 2000;
const ID_ROOTS_LIST: usize = 2001;
const ID_ROOT_PATH_EDIT: usize = 2002;
const ID_ROOT_BROWSE_BTN: usize = 2003;
const ID_ROOT_ADD_BTN: usize = 2004;
const ID_ROOT_REBUILD_BTN: usize = 2005;
const ID_ROOT_REMOVE_BTN: usize = 2006;

const ID_EXT_EDIT: usize = 2010;
const ID_EXCLUDE_EDIT: usize = 2011;
const ID_EXCLUDE_BROWSE_BTN: usize = 2012;
const ID_MAX_MB_EDIT: usize = 2013;
const ID_SAVE_RULES_BTN: usize = 2014;
const ID_PROGRESS: usize = 2015;

const ID_EXPORT_EDIT: usize = 2020;
const ID_EXPORT_BROWSE_BTN: usize = 2021;
const ID_EXPORT_SAVE_BTN: usize = 2022;
const ID_EXPORT_STOP_BTN: usize = 2023;

const ID_HOTKEY_EDIT: usize = 2030;
const ID_AUTOSTART_CHK: usize = 2031;
const ID_SAVE_GENERAL_BTN: usize = 2032;

const BG_COLOR: COLORREF = COLORREF(0x00F9F6F4); // RGB(244, 246, 249)
const CARD_COLOR: COLORREF = COLORREF(0x00FFFFFF);
const TEXT_COLOR: COLORREF = COLORREF(0x003B291E); // RGB(30, 41, 59)
const INPUT_TEXT_COLOR: COLORREF = COLORREF(0x002A170F); // RGB(15, 23, 42)

struct SettingsPanel {
    hwnd: Cell<HWND>,
    owner: HWND,
    admin_tx: Sender<AdminCmd>,
    dpi: Cell<u32>,
    font_title: Cell<HFONT>,
    font_normal: Cell<HFONT>,
    font_bold: Cell<HFONT>,
    bg_brush: HBRUSH,
    card_brush: HBRUSH,

    lbl_header: Cell<HWND>,
    back_btn: Cell<HWND>,

    lbl_roots: Cell<HWND>,
    roots_list: Cell<HWND>,
    root_path_edit: Cell<HWND>,
    root_browse_btn: Cell<HWND>,
    root_add_btn: Cell<HWND>,
    root_rebuild_btn: Cell<HWND>,
    root_remove_btn: Cell<HWND>,

    lbl_ext: Cell<HWND>,
    ext_edit: Cell<HWND>,
    lbl_max_mb: Cell<HWND>,
    max_mb_edit: Cell<HWND>,
    lbl_exclude: Cell<HWND>,
    exclude_browse_btn: Cell<HWND>,
    exclude_edit: Cell<HWND>,
    save_rules_btn: Cell<HWND>,
    progress: Cell<HWND>,

    lbl_export: Cell<HWND>,
    export_edit: Cell<HWND>,
    export_browse_btn: Cell<HWND>,
    export_save_btn: Cell<HWND>,
    export_stop_btn: Cell<HWND>,

    lbl_hotkey: Cell<HWND>,
    hotkey_edit: Cell<HWND>,
    autostart_chk: Cell<HWND>,
    save_general_btn: Cell<HWND>,

    busy: Cell<bool>,
    current_roots: RefCell<Vec<String>>,
    pending_roots: RefCell<Vec<String>>,
    saved_rules: RefCell<Option<dowse::IndexRules>>,
    applying_index: Cell<bool>,
    note_buf: RefCell<String>,
}

fn scale(px: i32, dpi: u32) -> i32 {
    (px * dpi as i32 + 48) / 96
}

pub fn create_panel(owner: HWND, admin_tx: Sender<AdminCmd>, dpi: u32) -> HWND {
    unsafe {
        let hinst = GetModuleHandleW(None).unwrap_or_default();
        let class_name = w!("IndexRaySettingsPanel");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(panel_wndproc),
            hInstance: hinst.into(),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            lpszClassName: class_name,
            ..Default::default()
        };
        let _ = RegisterClassExW(&wc);

        let state = Box::new(SettingsPanel {
            hwnd: Cell::new(HWND(null_mut())),
            owner,
            admin_tx,
            dpi: Cell::new(dpi),
            font_title: Cell::new(create_ui_font(16, true, dpi)),
            font_normal: Cell::new(create_ui_font(13, false, dpi)),
            font_bold: Cell::new(create_ui_font(13, true, dpi)),
            bg_brush: CreateSolidBrush(BG_COLOR),
            card_brush: CreateSolidBrush(CARD_COLOR),
            lbl_header: Cell::new(HWND(null_mut())),
            back_btn: Cell::new(HWND(null_mut())),
            lbl_roots: Cell::new(HWND(null_mut())),
            roots_list: Cell::new(HWND(null_mut())),
            root_path_edit: Cell::new(HWND(null_mut())),
            root_browse_btn: Cell::new(HWND(null_mut())),
            root_add_btn: Cell::new(HWND(null_mut())),
            root_rebuild_btn: Cell::new(HWND(null_mut())),
            root_remove_btn: Cell::new(HWND(null_mut())),
            lbl_ext: Cell::new(HWND(null_mut())),
            ext_edit: Cell::new(HWND(null_mut())),
            lbl_max_mb: Cell::new(HWND(null_mut())),
            max_mb_edit: Cell::new(HWND(null_mut())),
            lbl_exclude: Cell::new(HWND(null_mut())),
            exclude_browse_btn: Cell::new(HWND(null_mut())),
            exclude_edit: Cell::new(HWND(null_mut())),
            save_rules_btn: Cell::new(HWND(null_mut())),
            progress: Cell::new(HWND(null_mut())),
            lbl_export: Cell::new(HWND(null_mut())),
            export_edit: Cell::new(HWND(null_mut())),
            export_browse_btn: Cell::new(HWND(null_mut())),
            export_save_btn: Cell::new(HWND(null_mut())),
            export_stop_btn: Cell::new(HWND(null_mut())),
            lbl_hotkey: Cell::new(HWND(null_mut())),
            hotkey_edit: Cell::new(HWND(null_mut())),
            autostart_chk: Cell::new(HWND(null_mut())),
            save_general_btn: Cell::new(HWND(null_mut())),
            busy: Cell::new(false),
            current_roots: RefCell::new(Vec::new()),
            pending_roots: RefCell::new(Vec::new()),
            saved_rules: RefCell::new(None),
            applying_index: Cell::new(false),
            note_buf: RefCell::new(String::new()),
        });

        let raw_state = Box::into_raw(state);
        match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            class_name,
            w!(""),
            WS_CHILD | WS_CLIPCHILDREN,
            0,
            0,
            400,
            400,
            Some(owner),
            None,
            Some(hinst.into()),
            Some(raw_state as *const core::ffi::c_void),
        ) {
            Ok(hwnd) => hwnd,
            Err(_) => {
                drop(Box::from_raw(raw_state));
                HWND(null_mut())
            }
        }
    }
}

fn with_panel<R>(panel: HWND, f: impl FnOnce(&SettingsPanel) -> R) -> Option<R> {
    if panel.0.is_null() || !unsafe { IsWindow(Some(panel)).as_bool() } {
        return None;
    }
    let ptr = unsafe { GetWindowLongPtrW(panel, GWLP_USERDATA) as *const SettingsPanel };
    if ptr.is_null() {
        return None;
    }
    Some(f(unsafe { &*ptr }))
}

pub fn populate_panel(panel: HWND, snap: &StateSnapshot) {
    let _ = with_panel(panel, |v| v.populate(snap));
}

pub fn stage_folder(panel: HWND, folder: &Path) {
    let _ = with_panel(panel, |v| v.stage_root(&folder.to_string_lossy()));
}

pub fn finish_panel_operation(panel: HWND, succeeded: bool) {
    let _ = with_panel(panel, |v| {
        if v.applying_index.replace(false) && succeeded {
            v.pending_roots.borrow_mut().clear();
        }
        v.set_busy(false);
    });
}

pub fn focus_default(panel: HWND) {
    let _ = with_panel(panel, |v| unsafe {
        let _ = SetFocus(Some(v.root_path_edit.get()));
    });
}

pub fn take_status_note(panel: HWND) -> Option<String> {
    with_panel(panel, |v| {
        let mut g = v.note_buf.borrow_mut();
        if g.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut *g))
        }
    })
    .flatten()
}

pub fn update_dpi(panel: HWND, new_dpi: u32) {
    let _ = with_panel(panel, |v| unsafe {
        v.dpi.set(new_dpi);
        for old in [v.font_title.get(), v.font_normal.get(), v.font_bold.get()] {
            if !old.0.is_null() {
                let _ = DeleteObject(old.into());
            }
        }
        v.font_title.set(create_ui_font(16, true, new_dpi));
        v.font_normal.set(create_ui_font(13, false, new_dpi));
        v.font_bold.set(create_ui_font(13, true, new_dpi));
        v.apply_fonts();
        v.layout();
    });
}

pub fn handle_panel_keydown(panel: HWND, msg_hwnd: HWND, vk: u16) -> bool {
    if vk != VK_RETURN.0 {
        return false;
    }
    with_panel(panel, |v| {
        if v.busy.get() {
            return false;
        }
        if msg_hwnd == v.root_path_edit.get() {
            v.on_command(ID_ROOT_ADD_BTN);
            true
        } else if msg_hwnd == v.export_edit.get() {
            v.on_command(ID_EXPORT_SAVE_BTN);
            true
        } else if msg_hwnd == v.hotkey_edit.get() {
            v.on_command(ID_SAVE_GENERAL_BTN);
            true
        } else {
            false
        }
    })
    .unwrap_or(false)
}

#[allow(clippy::too_many_arguments)]
unsafe fn mk_ctrl(
    parent: HWND,
    ex: WINDOW_EX_STYLE,
    cls: PCWSTR,
    text: &str,
    style: WINDOW_STYLE,
    id: usize,
    font: HFONT,
    acc_name: Option<&str>,
) -> HWND {
    let wtext = wide_null(text);
    let h = unsafe {
        CreateWindowExW(
            ex,
            cls,
            PCWSTR(wtext.as_ptr()),
            WS_CHILD | WS_VISIBLE | style,
            0,
            0,
            10,
            10,
            Some(parent),
            Some(HMENU(id as *mut core::ffi::c_void)),
            None,
            None,
        )
        .unwrap_or_default()
    };
    unsafe {
        SendMessageW(h, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)));
        let _ = SetWindowTheme(h, w!("Explorer"), None);
    }
    if let Some(name) = acc_name {
        set_accessible_name(h, name);
    }
    h
}

impl SettingsPanel {
    fn post_status(&self, msg: impl Into<String>) {
        *self.note_buf.borrow_mut() = msg.into();
        unsafe {
            let _ = PostMessageW(Some(self.owner), WM_APP_STATUS_NOTE, WPARAM(0), LPARAM(0));
        }
    }

    unsafe fn init_controls(&self) {
        let p = self.hwnd.get();
        let fn_title = self.font_title.get();
        let fn_norm = self.font_normal.get();
        let fn_bold = self.font_bold.get();
        // SS_NOPREFIX (0x80) | SS_CENTERIMAGE (0x200) | SS_ENDELLIPSIS (0x4000)
        let lbl_style = WINDOW_STYLE(0x0080 | 0x0200 | 0x4000);

        self.lbl_header.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                "색인 폴더 및 환경 설정",
                lbl_style,
                0,
                fn_title,
                None,
            )
        });
        self.back_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "← 검색 화면으로 (Esc)",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_BACK_BTN,
                fn_bold,
                Some("검색 화면으로 돌아가기"),
            )
        });

        self.lbl_roots.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                "색인 폴더",
                lbl_style,
                0,
                fn_bold,
                None,
            )
        });
        self.roots_list.set(unsafe {
            mk_ctrl(
                p,
                WS_EX_CLIENTEDGE,
                w!("LISTBOX"),
                "",
                WS_VSCROLL | WS_TABSTOP | WINDOW_STYLE(LBS_NOTIFY as u32),
                ID_ROOTS_LIST,
                fn_norm,
                Some("등록한 폴더 및 추가 예정 폴더"),
            )
        });
        let root_edit = unsafe {
            mk_ctrl(
                p,
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                "",
                WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                ID_ROOT_PATH_EDIT,
                fn_norm,
                Some("추가할 폴더 경로"),
            )
        };
        self.root_path_edit.set(root_edit);
        let cue1 = wide_null("폴더 경로를 입력하거나 찾아본 뒤 [목록에 추가]를 누르세요 (Enter).");
        unsafe {
            SendMessageW(
                root_edit,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(cue1.as_ptr() as isize)),
            );
        }
        self.root_browse_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "찾아보기…",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_ROOT_BROWSE_BTN,
                fn_norm,
                Some("찾아보기…"),
            )
        });
        self.root_add_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "목록에 추가",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_ROOT_ADD_BTN,
                fn_bold,
                Some("목록에 추가"),
            )
        });
        self.root_rebuild_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "선택 폴더만 다시 색인",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_ROOT_REBUILD_BTN,
                fn_norm,
                Some("선택 폴더만 다시 색인"),
            )
        });
        self.root_remove_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "선택 폴더 빼기",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_ROOT_REMOVE_BTN,
                fn_norm,
                Some("선택 폴더 빼기"),
            )
        });

        self.lbl_ext.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                "색인 확장자 (쉼표로 구분)",
                lbl_style,
                0,
                fn_bold,
                None,
            )
        });
        self.ext_edit.set(unsafe {
            mk_ctrl(
                p,
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                "",
                WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                ID_EXT_EDIT,
                fn_norm,
                Some("색인 확장자"),
            )
        });
        self.lbl_max_mb.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                "크기 상한(MB)",
                lbl_style,
                0,
                fn_bold,
                None,
            )
        });
        self.max_mb_edit.set(unsafe {
            mk_ctrl(
                p,
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                "20",
                WS_TABSTOP | WINDOW_STYLE((ES_AUTOHSCROLL | ES_NUMBER) as u32),
                ID_MAX_MB_EDIT,
                fn_norm,
                Some("단일 파일 크기 상한 MB"),
            )
        });

        self.lbl_exclude.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                "제외 폴더 이름 또는 전체 경로 (한 줄에 하나)",
                lbl_style,
                0,
                fn_bold,
                None,
            )
        });
        self.exclude_browse_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "제외 폴더 찾아보기…",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_EXCLUDE_BROWSE_BTN,
                fn_norm,
                Some("제외 폴더 찾아보기…"),
            )
        });
        self.exclude_edit.set(unsafe {
            mk_ctrl(
                p,
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                "",
                WS_VSCROLL
                    | WS_TABSTOP
                    | WINDOW_STYLE((ES_MULTILINE | ES_AUTOVSCROLL | ES_AUTOHSCROLL) as u32),
                ID_EXCLUDE_EDIT,
                fn_norm,
                Some("제외 폴더 목록"),
            )
        });
        self.save_rules_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "규칙 적용 및 색인 시작",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_SAVE_RULES_BTN,
                fn_norm,
                Some("규칙 적용 및 색인 시작"),
            )
        });
        self.progress.set(unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PROGRESS_CLASSW,
                w!(""),
                WS_CHILD | WINDOW_STYLE(PBS_MARQUEE),
                0, 0, 0, 0,
                Some(p),
                Some(HMENU(ID_PROGRESS as *mut core::ffi::c_void)),
                None,
                None,
            ).unwrap_or_default()
        });

        self.lbl_export.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                "Markdown 내보내기 (색인 폴더 밖 · 색인 변경 시 같은 위치에 자동 반영)",
                lbl_style,
                0,
                fn_bold,
                None,
            )
        });
        let export_edit = unsafe {
            mk_ctrl(
                p,
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                "",
                WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                ID_EXPORT_EDIT,
                fn_norm,
                Some("내보내기 경로"),
            )
        };
        self.export_edit.set(export_edit);
        let cue2 = wide_null("OneDrive 또는 로컬 전용 출력 폴더 경로");
        unsafe {
            SendMessageW(
                export_edit,
                EM_SETCUEBANNER,
                Some(WPARAM(1)),
                Some(LPARAM(cue2.as_ptr() as isize)),
            );
        }
        self.export_browse_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "찾아보기…",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_EXPORT_BROWSE_BTN,
                fn_norm,
                Some("내보내기 폴더 찾아보기…"),
            )
        });
        self.export_save_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "저장 및 동기화",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_EXPORT_SAVE_BTN,
                fn_norm,
                Some("저장 및 동기화"),
            )
        });
        self.export_stop_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "내보내기 중지",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_EXPORT_STOP_BTN,
                fn_norm,
                Some("내보내기 중지"),
            )
        });

        self.lbl_hotkey.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                "전역 단축키 (예: Alt+Backquote, Ctrl+Space)",
                lbl_style,
                0,
                fn_bold,
                None,
            )
        });
        self.hotkey_edit.set(unsafe {
            mk_ctrl(
                p,
                WS_EX_CLIENTEDGE,
                w!("EDIT"),
                "Alt+Backquote",
                WS_TABSTOP | WINDOW_STYLE(ES_AUTOHSCROLL as u32),
                ID_HOTKEY_EDIT,
                fn_norm,
                Some("전역 호출 단축키"),
            )
        });
        self.autostart_chk.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "Windows 시작 시 자동 실행",
                WS_TABSTOP | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
                ID_AUTOSTART_CHK,
                fn_norm,
                Some("Windows 시작 시 자동 실행"),
            )
        });
        self.save_general_btn.set(unsafe {
            mk_ctrl(
                p,
                WINDOW_EX_STYLE(0),
                w!("BUTTON"),
                "단축키·자동실행 저장",
                WS_TABSTOP | WINDOW_STYLE(BS_PUSHBUTTON as u32),
                ID_SAVE_GENERAL_BTN,
                fn_norm,
                Some("단축키·자동실행 저장"),
            )
        });

        unsafe { self.layout() };
    }

    unsafe fn apply_fonts(&self) {
        let set = |h: HWND, f: HFONT| unsafe {
            if !h.0.is_null() {
                SendMessageW(h, WM_SETFONT, Some(WPARAM(f.0 as usize)), Some(LPARAM(1)));
            }
        };
        let ft = self.font_title.get();
        let fn_n = self.font_normal.get();
        let fn_b = self.font_bold.get();

        set(self.lbl_header.get(), ft);
        set(self.back_btn.get(), fn_b);
        for b in [
            self.lbl_roots.get(),
            self.root_add_btn.get(),
            self.lbl_ext.get(),
            self.lbl_max_mb.get(),
            self.lbl_exclude.get(),
            self.lbl_export.get(),
            self.lbl_hotkey.get(),
        ] {
            set(b, fn_b);
        }
        for n in [
            self.roots_list.get(),
            self.root_path_edit.get(),
            self.root_browse_btn.get(),
            self.root_rebuild_btn.get(),
            self.root_remove_btn.get(),
            self.ext_edit.get(),
            self.max_mb_edit.get(),
            self.exclude_browse_btn.get(),
            self.exclude_edit.get(),
            self.save_rules_btn.get(),
            self.export_edit.get(),
            self.export_browse_btn.get(),
            self.export_save_btn.get(),
            self.export_stop_btn.get(),
            self.hotkey_edit.get(),
            self.autostart_chk.get(),
            self.save_general_btn.get(),
        ] {
            set(n, fn_n);
        }
    }

    unsafe fn layout(&self) {
        let hwnd = self.hwnd.get();
        if hwnd.0.is_null() {
            return;
        }
        let mut rc = RECT::default();
        unsafe {
            let _ = GetClientRect(hwnd, &mut rc);
        }
        let w = rc.right - rc.left;
        if w <= 40 {
            return;
        }
        let d = self.dpi.get();
        let pad_x = scale(14, d);
        let gap = scale(6, d);
        let row_h = scale(28, d);
        let lbl_h = scale(19, d);
        let btn_w_sm = scale(104, d);
        let btn_w_md = scale(152, d);
        let btn_w_lg = scale(176, d);
        let content_w = (w - pad_x * 2).max(scale(300, d));

        let mv = |ctrl: HWND, x: i32, y: i32, cw: i32, ch: i32| unsafe {
            if !ctrl.0.is_null() {
                let _ = SetWindowPos(
                    ctrl,
                    None,
                    x,
                    y,
                    cw.max(8),
                    ch.max(8),
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        };

        let mut y = scale(10, d);
        let back_w = scale(176, d);
        mv(
            self.lbl_header.get(),
            pad_x,
            y,
            content_w - back_w - gap,
            scale(30, d),
        );
        mv(
            self.back_btn.get(),
            pad_x + content_w - back_w,
            y,
            back_w,
            scale(30, d),
        );
        y += scale(38, d);

        // 1. Indexed folders
        mv(self.lbl_roots.get(), pad_x, y, content_w, lbl_h);
        y += lbl_h + scale(3, d);
        let list_h = scale(68, d);
        mv(self.roots_list.get(), pad_x, y, content_w, list_h);
        y += list_h + gap;

        mv(
            self.root_path_edit.get(),
            pad_x,
            y,
            content_w - btn_w_sm - gap,
            row_h,
        );
        mv(
            self.root_browse_btn.get(),
            pad_x + content_w - btn_w_sm,
            y,
            btn_w_sm,
            row_h,
        );
        y += row_h + gap;

        mv(self.root_add_btn.get(), pad_x, y, btn_w_sm, row_h);
        mv(
            self.root_rebuild_btn.get(),
            pad_x + btn_w_sm + gap,
            y,
            btn_w_md,
            row_h,
        );
        mv(
            self.root_remove_btn.get(),
            pad_x + btn_w_sm + gap + btn_w_md + gap,
            y,
            btn_w_md,
            row_h,
        );
        y += row_h + scale(12, d);

        // 2. Extensions & Max MB
        let mb_w = scale(110, d);
        let ext_w = content_w - mb_w - scale(12, d);
        mv(self.lbl_ext.get(), pad_x, y, ext_w, lbl_h);
        mv(
            self.lbl_max_mb.get(),
            pad_x + ext_w + scale(12, d),
            y,
            mb_w,
            lbl_h,
        );
        y += lbl_h + scale(3, d);
        mv(self.ext_edit.get(), pad_x, y, ext_w, row_h);
        mv(
            self.max_mb_edit.get(),
            pad_x + ext_w + scale(12, d),
            y,
            mb_w,
            row_h,
        );
        y += row_h + scale(10, d);

        // 3. Exclude dirs
        mv(
            self.lbl_exclude.get(),
            pad_x,
            y + scale(4, d),
            content_w - btn_w_md - gap,
            lbl_h,
        );
        mv(
            self.exclude_browse_btn.get(),
            pad_x + content_w - btn_w_md,
            y,
            btn_w_md,
            row_h,
        );
        y += row_h + scale(3, d);
        let excl_h = scale(58, d);
        mv(self.exclude_edit.get(), pad_x, y, content_w, excl_h);
        y += excl_h + gap;
        mv(self.save_rules_btn.get(), pad_x, y, scale(250, d), row_h);
        let progress_x = scale(250, d) + gap;
        mv(self.progress.get(), pad_x + progress_x, y + scale(6, d),
           content_w - progress_x, scale(16, d));
        y += row_h + scale(12, d);

        // 4. Markdown export
        mv(self.lbl_export.get(), pad_x, y, content_w, lbl_h);
        y += lbl_h + scale(3, d);
        mv(
            self.export_edit.get(),
            pad_x,
            y,
            content_w - btn_w_sm - gap,
            row_h,
        );
        mv(
            self.export_browse_btn.get(),
            pad_x + content_w - btn_w_sm,
            y,
            btn_w_sm,
            row_h,
        );
        y += row_h + gap;
        mv(self.export_save_btn.get(), pad_x, y, scale(136, d), row_h);
        mv(
            self.export_stop_btn.get(),
            pad_x + scale(136, d) + gap,
            y,
            scale(128, d),
            row_h,
        );
        y += row_h + scale(12, d);

        // 5. General: Hotkey + Autostart
        mv(self.lbl_hotkey.get(), pad_x, y, content_w, lbl_h);
        y += lbl_h + scale(3, d);
        let hk_w = scale(210, d);
        let chk_w = scale(215, d);
        mv(self.hotkey_edit.get(), pad_x, y, hk_w, row_h);
        mv(
            self.autostart_chk.get(),
            pad_x + hk_w + scale(12, d),
            y,
            chk_w,
            row_h,
        );
        mv(
            self.save_general_btn.get(),
            pad_x + hk_w + scale(12, d) + chk_w + gap,
            y,
            btn_w_lg,
            row_h,
        );
    }

    fn set_busy(&self, busy: bool) {
        self.busy.set(busy);
        let controls = [
            self.roots_list.get(),
            self.root_path_edit.get(),
            self.ext_edit.get(),
            self.max_mb_edit.get(),
            self.exclude_edit.get(),
            self.export_edit.get(),
            self.hotkey_edit.get(),
            self.autostart_chk.get(),
            self.root_browse_btn.get(),
            self.root_add_btn.get(),
            self.root_remove_btn.get(),
            self.exclude_browse_btn.get(),
            self.save_rules_btn.get(),
            self.export_browse_btn.get(),
            self.export_save_btn.get(),
            self.export_stop_btn.get(),
            self.save_general_btn.get(),
        ];
        for c in controls {
            unsafe {
                let _ = EnableWindow(c, !busy);
            }
        }
        let progress = self.progress.get();
        unsafe {
            SendMessageW(progress, PBM_SETMARQUEE, Some(WPARAM(usize::from(busy))), Some(LPARAM(0)));
            let _ = ShowWindow(progress, if busy { SW_SHOWNA } else { SW_HIDE });
        }
        self.refresh_index_actions();
    }

    fn refresh_index_actions(&self) {
        let pending = !self.pending_roots.borrow().is_empty();
        let label = if pending {
            "규칙 적용 및 새 폴더 색인 시작"
        } else {
            "규칙 적용 및 색인 시작"
        };
        set_text(self.save_rules_btn.get(), label);
        unsafe {
            let _ = EnableWindow(
                self.root_rebuild_btn.get(),
                !self.busy.get() && !pending && !self.current_roots.borrow().is_empty(),
            );
        }
    }

    fn render_roots(&self, selected: usize) {
        let roots_lb = self.roots_list.get();
        unsafe {
            SendMessageW(roots_lb, LB_RESETCONTENT, Some(WPARAM(0)), Some(LPARAM(0)));
        }
        let registered = self.current_roots.borrow();
        let pending = self.pending_roots.borrow();
        let rows = registered.iter().map(|r| wide_null(r))
            .chain(pending.iter().map(|r| wide_null(&format!("[추가 예정] {r}"))));
        for w in rows {
            unsafe {
                SendMessageW(
                    roots_lb,
                    LB_ADDSTRING,
                    Some(WPARAM(0)),
                    Some(LPARAM(w.as_ptr() as isize)),
                );
            }
        }
        let len = registered.len() + pending.len();
        drop(pending);
        drop(registered);
        if len != 0 {
            unsafe {
                SendMessageW(roots_lb, LB_SETCURSEL, Some(WPARAM(selected.min(len - 1))), Some(LPARAM(0)));
            }
        }
        self.refresh_index_actions();
    }

    fn populate(&self, snap: &StateSnapshot) {
        self.current_roots.replace(snap.roots.clone());
        self.render_roots(0);
        if self.saved_rules.borrow().as_ref() != Some(&snap.rules) {
            set_text(self.ext_edit.get(), &snap.rules.effective_extensions().join(", "));
            set_text(self.max_mb_edit.get(), &snap.rules.max_file_mb.to_string());
            set_text(self.exclude_edit.get(), &snap.rules.exclude_dirs.join("\r\n"));
            self.saved_rules.replace(Some(snap.rules.clone()));
        }

        let exp = snap
            .config
            .export_folder
            .as_ref()
            .map(|p| dowse::display_path(&p.to_string_lossy()))
            .unwrap_or_default();
        set_text(self.export_edit.get(), &exp);

        set_text(self.hotkey_edit.get(), &snap.config.hotkey);
        unsafe {
            SendMessageW(
                self.autostart_chk.get(),
                BM_SETCHECK,
                Some(WPARAM(usize::from(snap.autostart_enabled))),
                Some(LPARAM(0)),
            );
        }
    }

    fn stage_root(&self, folder: &str) {
        if self.busy.get() {
            return;
        }
        let roots = self.current_roots.borrow();
        let mut pending = self.pending_roots.borrow_mut();
        if roots.iter().chain(pending.iter()).any(|p| p.eq_ignore_ascii_case(folder)) {
            drop(pending);
            drop(roots);
            self.post_status("목록에 이미 있는 폴더입니다.");
            return;
        }
        pending.push(folder.to_owned());
        let selected = roots.len() + pending.len() - 1;
        drop(pending);
        drop(roots);
        self.render_roots(selected);
        self.post_status("제외 폴더를 확인한 뒤 [규칙 적용 및 새 폴더 색인 시작]을 누르세요.");
    }

    fn selected_root_index(&self) -> Option<usize> {
        let idx = unsafe {
            SendMessageW(
                self.roots_list.get(),
                LB_GETCURSEL,
                Some(WPARAM(0)),
                Some(LPARAM(0)),
            )
            .0
        };
        usize::try_from(idx).ok()
    }

    fn on_command(&self, id: usize) {
        if id == ID_BACK_BTN {
            unsafe {
                let _ = PostMessageW(Some(self.owner), WM_APP_CLOSE_SETTINGS, WPARAM(0), LPARAM(0));
            }
            return;
        }
        if self.busy.get() {
            return;
        }

        match id {
            ID_ROOT_BROWSE_BTN => {
                if let Some(folder) = pick_folder_dialog(self.owner, "색인할 폴더 선택") {
                    set_text(self.root_path_edit.get(), &folder.to_string_lossy());
                }
            }
            ID_ROOT_ADD_BTN => {
                let raw = get_text(self.root_path_edit.get());
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    self.post_status("추가할 폴더 경로를 입력하거나 찾아보기로 선택하세요.");
                    return;
                }
                self.stage_root(trimmed);
                set_text(self.root_path_edit.get(), "");
            }
            ID_ROOT_REBUILD_BTN => {
                if !self.pending_roots.borrow().is_empty() {
                    self.post_status("추가 예정 폴더는 [규칙 적용 및 새 폴더 색인 시작]을 눌러 색인하세요.");
                    return;
                }
                let root = self.selected_root_index()
                    .and_then(|idx| self.current_roots.borrow().get(idx).cloned());
                let Some(root) = root else {
                    self.post_status("목록에서 등록한 폴더를 선택하세요.");
                    return;
                };
                self.set_busy(true);
                self.post_status(format!("선택 폴더 다시 색인 중… ({root})"));
                let _ = self.admin_tx.send(AdminCmd::RebuildRoot { folder: root });
            }
            ID_ROOT_REMOVE_BTN => {
                let Some(idx) = self.selected_root_index() else {
                    self.post_status("목록에서 해제할 폴더를 선택하세요.");
                    return;
                };
                let registered = self.current_roots.borrow().len();
                if idx >= registered {
                    self.pending_roots.borrow_mut().remove(idx - registered);
                    self.render_roots(idx);
                    return;
                }
                let root = self.current_roots.borrow()[idx].clone();
                self.set_busy(true);
                self.post_status(format!("선택 폴더 해제 중… ({root})"));
                let _ = self.admin_tx.send(AdminCmd::RemoveRoot { folder: root });
            }
            ID_EXCLUDE_BROWSE_BTN => {
                if let Some(folder) = pick_folder_dialog(self.owner, "제외할 폴더 선택") {
                    let mut cur = get_text(self.exclude_edit.get());
                    if !cur.trim().is_empty() && !cur.ends_with('\n') {
                        cur.push_str("\r\n");
                    }
                    cur.push_str(&folder.to_string_lossy());
                    set_text(self.exclude_edit.get(), &cur);
                }
            }
            ID_SAVE_RULES_BTN => {
                let extensions: Vec<String> = get_text(self.ext_edit.get())
                    .split([',', '\r', '\n'])
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let exclude_dirs: Vec<String> = get_text(self.exclude_edit.get())
                    .lines()
                    .flat_map(|line| {
                        let t = line.trim();
                        if t.is_empty() {
                            Vec::new()
                        } else if t.contains(':') || t.starts_with(r"\\") {
                            vec![t.to_string()]
                        } else {
                            t.split(',')
                                .map(|p| p.trim().to_string())
                                .filter(|p| !p.is_empty())
                                .collect()
                        }
                    })
                    .collect();
                let max_file_mb = get_text(self.max_mb_edit.get())
                    .trim()
                    .parse::<u64>()
                    .unwrap_or(20)
                    .max(1);
                if self.saved_rules.borrow().is_none() {
                    self.post_status("기존 규칙을 불러온 뒤 색인을 시작하세요.");
                    return;
                }
                let folders = self.pending_roots.borrow().iter().map(PathBuf::from).collect();
                self.set_busy(true);
                self.applying_index.set(true);
                self.post_status("색인 규칙을 확인하는 중…");
                if self.admin_tx.send(AdminCmd::SaveRulesAndRebuild {
                    folders, extensions, exclude_dirs, max_file_mb,
                }).is_err() {
                    self.applying_index.set(false);
                    self.set_busy(false);
                    self.post_status("색인 작업을 시작하지 못했습니다.");
                }
            }
            ID_EXPORT_BROWSE_BTN => {
                if let Some(folder) = pick_folder_dialog(self.owner, "Markdown 내보내기 폴더 선택")
                {
                    set_text(self.export_edit.get(), &folder.to_string_lossy());
                }
            }
            ID_EXPORT_SAVE_BTN => {
                let raw = get_text(self.export_edit.get());
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    self.post_status("내보내기 폴더 경로를 입력하거나 찾아보기로 선택하세요.");
                    return;
                }
                self.set_busy(true);
                self.post_status("Markdown 동기화 중…");
                let _ = self.admin_tx.send(AdminCmd::SetExportFolder {
                    folder: Some(PathBuf::from(trimmed)),
                });
            }
            ID_EXPORT_STOP_BTN => {
                set_text(self.export_edit.get(), "");
                self.set_busy(true);
                self.post_status("내보내기 중지 처리 중…");
                let _ = self.admin_tx.send(AdminCmd::SetExportFolder { folder: None });
            }
            ID_SAVE_GENERAL_BTN => {
                let hk = get_text(self.hotkey_edit.get()).trim().to_string();
                let checked = unsafe {
                    SendMessageW(
                        self.autostart_chk.get(),
                        BM_GETCHECK,
                        Some(WPARAM(0)),
                        Some(LPARAM(0)),
                    )
                    .0 != 0
                };
                let mut cfg = config::load();
                if !hk.is_empty() {
                    cfg.hotkey = hk;
                }
                cfg.autostart_user_disabled = !checked;
                let mut msg = Vec::new();
                if let Err(err) = config::set_autostart_enabled(checked) {
                    msg.push(format!("자동 실행 변경 실패: {err}"));
                }
                if let Err(err) = config::save(&cfg) {
                    msg.push(format!("설정 저장 실패: {err}"));
                } else {
                    unsafe {
                        let _ = PostMessageW(
                            Some(self.owner),
                            WM_APP_RELOAD_HOTKEY,
                            WPARAM(0),
                            LPARAM(0),
                        );
                    }
                    msg.push(format!(
                        "저장됨 (단축키: {})",
                        config::format_hotkey_label(&cfg.hotkey)
                    ));
                }
                self.post_status(msg.join(" · "));
            }
            _ => {}
        }
    }
}

fn get_text(hwnd: HWND) -> String {
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

fn set_text(hwnd: HWND, text: &str) {
    if hwnd.0.is_null() {
        return;
    }
    let w = wide_null(text);
    unsafe {
        let _ = SetWindowTextW(hwnd, PCWSTR(w.as_ptr()));
    }
}

unsafe extern "system" fn panel_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = unsafe { &*(lparam.0 as *const CREATESTRUCTW) };
        let ptr = cs.lpCreateParams as *const SettingsPanel;
        unsafe {
            (*ptr).hwnd.set(hwnd);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
        }
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }

    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const SettingsPanel };
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let view = unsafe { &*ptr };

    match msg {
        windows::Win32::UI::WindowsAndMessaging::WM_CREATE => {
            unsafe { view.init_controls() };
            LRESULT(0)
        }
        WM_ERASEBKGND => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            let mut rc = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rc);
                FillRect(hdc, &rc, view.bg_brush);
            }
            LRESULT(1)
        }
        WM_SIZE => {
            unsafe { view.layout() };
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            unsafe {
                SetBkColor(hdc, BG_COLOR);
                SetTextColor(hdc, TEXT_COLOR);
            }
            LRESULT(view.bg_brush.0 as isize)
        }
        WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
            let hdc = HDC(wparam.0 as *mut core::ffi::c_void);
            unsafe {
                SetBkColor(hdc, CARD_COLOR);
                SetTextColor(hdc, INPUT_TEXT_COLOR);
            }
            LRESULT(view.card_brush.0 as isize)
        }
        WM_COMMAND => {
            let id = wparam.0 & 0xFFFF;
            let code = (wparam.0 >> 16) & 0xFFFF;
            if code == 0 {
                view.on_command(id);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                let boxed = Box::from_raw(ptr as *mut SettingsPanel);
                for f in [
                    boxed.font_title.get(),
                    boxed.font_normal.get(),
                    boxed.font_bold.get(),
                ] {
                    if !f.0.is_null() {
                        let _ = DeleteObject(f.into());
                    }
                }
                let _ = DeleteObject(boxed.bg_brush.into());
                let _ = DeleteObject(boxed.card_brush.into());
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
