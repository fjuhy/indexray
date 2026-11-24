use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

fn log_dir() -> Option<PathBuf> {
    crate::profile::index_dir().ok()?.parent().map(|p| p.join("logs"))
}

fn rotate_if_needed(dir: &Path) {
    let current = dir.join("indexray.log");
    let Ok(meta) = std::fs::metadata(&current) else {
        return;
    };
    if meta.len() < MAX_LOG_BYTES {
        return;
    }
    let rotated = dir.join("indexray.log.1");
    let _ = std::fs::remove_file(&rotated);
    let _ = std::fs::rename(&current, &rotated);
}

pub fn init() {
    let Some(dir) = log_dir() else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    rotate_if_needed(&dir);

    let path = dir.join("indexray.log");
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };

    redirect_std_handles(&file);
    std::mem::forget(file);

    log_line("startup", "IndexRay native 시작");
    install_panic_hook();
}

fn redirect_std_handles(file: &std::fs::File) {
    use std::os::windows::io::AsRawHandle;
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{STD_ERROR_HANDLE, STD_OUTPUT_HANDLE, SetStdHandle};

    let handle = HANDLE(file.as_raw_handle());
    unsafe {
        let _ = SetStdHandle(STD_OUTPUT_HANDLE, handle);
        let _ = SetStdHandle(STD_ERROR_HANDLE, handle);
    }
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());
        log_line(
            "panic",
            &format!("스레드 [{thread_name}] 패닉 ({location}): {payload}"),
        );
        default_hook(info);
    }));
}

pub fn log_line(component: &str, msg: &str) {
    eprintln!("[{}] [{component}] {msg}", format_now());
}

fn format_now() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (hour, rem) = (rem / 3600, rem % 3600);
    let (min, sec) = (rem / 60, rem % 60);
    let (year, month, day) = civil_from_days(days as i64);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{min:02}:{sec:02} UTC")
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
