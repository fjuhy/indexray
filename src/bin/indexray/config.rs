use std::{collections::BTreeMap, path::PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub export_folder: Option<PathBuf>,
    #[serde(default)]
    pub target_dir: Option<PathBuf>,
    #[serde(default)]
    pub root_aliases: BTreeMap<PathBuf, PathBuf>,
    #[serde(default)]
    pub autostart_user_disabled: bool,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default = "default_lang")]
    pub lang: String,
}

fn default_hotkey() -> String {
    "Alt+Backquote".to_string()
}

fn default_lang() -> String {
    "auto".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            export_folder: None,
            target_dir: None,
            root_aliases: BTreeMap::new(),
            autostart_user_disabled: false,
            hotkey: default_hotkey(),
            lang: default_lang(),
        }
    }
}

pub(crate) use crate::profile::index_dir;

pub fn config_path() -> Result<PathBuf> {
    Ok(index_dir()?.with_file_name("config.json"))
}

pub fn load() -> AppConfig {
    let Ok(path) = config_path() else {
        return AppConfig::default();
    };
    let Ok(bytes) = std::fs::read(&path) else {
        return AppConfig::default();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

pub fn save(cfg: &AppConfig) -> Result<()> {
    let path = config_path()?;
    let parent = path.parent().context("설정 경로 오류")?;
    std::fs::create_dir_all(parent)?;
    let bytes = serde_json::to_vec_pretty(cfg)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    std::io::Write::write_all(&mut tmp, &bytes)?;
    tmp.persist(&path).context("설정 파일 저장 실패")?;
    Ok(())
}

/// Deterministic mutex/window key scoped by index directory so isolated tests
/// (`INDEXRAY_INDEX_DIR`) never collide with an installed user instance.
pub fn instance_scope_id() -> String {
    let raw = index_dir()
        .map(|p| p.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let digest = Sha256::digest(raw.as_bytes());
    format!("{:x}", digest)[..16].to_string()
}

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const RUN_VALUE: &str = "IndexRay";

pub fn is_autostart_enabled() -> bool {
    if crate::profile::index_override().is_some() {
        return false;
    }
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_SZ, RegGetValueW};
    use windows::core::w;

    let mut size: u32 = 0;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            w!("IndexRay"),
            RRF_RT_REG_SZ,
            None,
            None,
            Some(&mut size),
        )
    };
    status == ERROR_SUCCESS && size > 2
}

pub fn set_autostart_enabled(enable: bool) -> Result<()> {
    if crate::profile::index_override().is_some() {
        return Ok(());
    }
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        HKEY_CURRENT_USER, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW,
        RegOpenKeyExW, RegSetValueExW, REG_OPTION_NON_VOLATILE,
    };
    use windows::core::PCWSTR;

    let subkey: Vec<u16> = RUN_KEY.encode_utf16().chain(std::iter::once(0)).collect();
    let valname: Vec<u16> = RUN_VALUE.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        if enable {
            let exe = std::env::current_exe().context("실행 파일 경로 확인 실패")?;
            let quoted = format!("\"{}\"", exe.display());
            let wide: Vec<u16> = quoted.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes: &[u8] =
                std::slice::from_raw_parts(wide.as_ptr() as *const u8, wide.len() * 2);

            let mut hkey = Default::default();
            let err = RegCreateKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                Some(0),
                None,
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                None,
                &mut hkey,
                None,
            );
            anyhow::ensure!(err == ERROR_SUCCESS, "자동 실행 레지스트리 열기 실패: {:?}", err);
            let set_err = RegSetValueExW(
                hkey,
                PCWSTR(valname.as_ptr()),
                Some(0),
                REG_SZ,
                Some(bytes),
            );
            let _ = RegCloseKey(hkey);
            anyhow::ensure!(set_err == ERROR_SUCCESS, "자동 실행 등록 실패: {:?}", set_err);
        } else {
            let mut hkey = Default::default();
            let err = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                PCWSTR(subkey.as_ptr()),
                Some(0),
                KEY_SET_VALUE,
                &mut hkey,
            );
            if err == ERROR_SUCCESS {
                let _ = RegDeleteValueW(hkey, PCWSTR(valname.as_ptr()));
                let _ = RegCloseKey(hkey);
            }
        }
    }
    Ok(())
}

pub fn ensure_default_autostart(cfg: &AppConfig) {
    if crate::profile::index_override().is_some() {
        return;
    }
    if !cfg.autostart_user_disabled
        && !is_autostart_enabled()
        && let Err(err) = set_autostart_enabled(true)
    {
        crate::logging::log_line("autostart", &format!("자동 실행 기본 설정 실패: {err:#}"));
    }
}

/// Parse Tauri-compatible hotkey strings (e.g., "Alt+Backquote", "Ctrl+Space")
/// into Win32 `(fsModifiers, vk)`. Falls back to `Alt+Backquote` (`MOD_ALT`, `0xC0`).
pub fn parse_win32_hotkey(spec: &str) -> (u32, u32) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
    };

    let mut mods = MOD_NOREPEAT.0;
    let mut vk: Option<u32> = None;

    for raw in spec.split('+').map(str::trim).filter(|s| !s.is_empty()) {
        let lower = raw.to_ascii_lowercase();
        match lower.as_str() {
            "alt" | "option" => mods |= MOD_ALT.0,
            "ctrl" | "control" | "cmdorctrl" | "commandorcontrol" => mods |= MOD_CONTROL.0,
            "shift" => mods |= MOD_SHIFT.0,
            "super" | "meta" | "win" | "command" => mods |= MOD_WIN.0,
            "backquote" | "`" | "tilde" => vk = Some(0xC0), // VK_OEM_3
            "space" => vk = Some(0x20),
            "enter" | "return" => vk = Some(0x0D),
            "tab" => vk = Some(0x09),
            "slash" | "/" => vk = Some(0xBF),
            "backslash" | "\\" => vk = Some(0xDC),
            "comma" | "," => vk = Some(0xBC),
            "period" | "." => vk = Some(0xBE),
            "semicolon" | ";" => vk = Some(0xBA),
            "quote" | "'" => vk = Some(0xDE),
            "minus" | "-" => vk = Some(0xBD),
            "equal" | "=" => vk = Some(0xBB),
            s if s.starts_with("key") && s.len() == 4 => {
                let ch = s.as_bytes()[3].to_ascii_uppercase();
                if ch.is_ascii_uppercase() {
                    vk = Some(ch as u32);
                }
            }
            s if s.starts_with("digit") && s.len() == 6 => {
                let ch = s.as_bytes()[5];
                if ch.is_ascii_digit() {
                    vk = Some(ch as u32);
                }
            }
            s if s.len() == 1 => {
                let ch = s.as_bytes()[0].to_ascii_uppercase();
                if ch.is_ascii_alphanumeric() {
                    vk = Some(ch as u32);
                }
            }
            s if s.starts_with('f') && s.len() <= 3 => {
                if let Ok(n) = s[1..].parse::<u32>()
                    && (1..=12).contains(&n)
                {
                    vk = Some(0x70 + n - 1);
                }
            }
            _ => {}
        }
    }

    if mods == MOD_NOREPEAT.0 {
        mods |= MOD_ALT.0;
    }
    (mods, vk.unwrap_or(0xC0))
}

pub fn format_hotkey_label(spec: &str) -> String {
    let mut out = Vec::new();
    for part in spec.split('+').map(str::trim).filter(|s| !s.is_empty()) {
        let mapped = match part.to_ascii_lowercase().as_str() {
            "backquote" => "`".to_string(),
            "ctrl" | "control" => "Ctrl".to_string(),
            "alt" => "Alt".to_string(),
            "shift" => "Shift".to_string(),
            "super" | "meta" | "win" => "Win".to_string(),
            "space" => "Space".to_string(),
            s if s.starts_with("key") && s.len() == 4 => s[3..].to_ascii_uppercase(),
            s if s.starts_with("digit") && s.len() == 6 => s[5..].to_string(),
            _ => part.to_string(),
        };
        out.push(mapped);
    }
    if out.is_empty() {
        "Alt+`".to_string()
    } else {
        out.join("+")
    }
}
