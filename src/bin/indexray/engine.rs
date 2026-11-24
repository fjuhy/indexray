use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use dowse::{
    IndexRules, IndexUpdater, OcrPipeline, PreviewHit, SearchHit, Searcher, SortMode,
    WatchProgress, display_path, ext_group_by_name, index_root_incremental_with_progress,
    korean_reindex_roots, load_rules, rebuild_index_with_progress, rebuild_root_with_progress,
    registered_roots, remove_root, save_rules, watch_roots_auto,
};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::config::{self, AppConfig};
use crate::logging::log_line;

pub const WM_APP_EVENT: u32 = 0x8000 + 42;

#[derive(Clone, Debug)]
pub struct HitRow {
    pub raw_path: String,
    pub display_path: String,
    pub file_name: String,
    pub parent_dir: String,
    pub ext: String,
    pub snippet: String,
    pub highlighted: Vec<std::ops::Range<usize>>,
    pub author: Option<String>,
    pub title: Option<String>,
}

#[derive(Clone, Debug)]
pub struct PreviewData {
    pub raw_path: String,
    pub snippet: String,
    pub highlighted: Vec<std::ops::Range<usize>>,
    pub size: u64,
    pub mtime: i64,
}

#[derive(Clone, Debug)]
pub struct StateSnapshot {
    pub has_index: bool,
    pub num_docs: u64,
    pub roots: Vec<String>,
    pub rules: IndexRules,
    pub config: AppConfig,
    pub autostart_enabled: bool,
    pub ko_reindex_roots: Vec<String>,
    pub export_status: String,
}

pub enum UiEvent {
    StateReady(StateSnapshot),
    SearchDone {
        token: u64,
        hits: Vec<HitRow>,
        total: usize,
        elapsed_ms: f64,
    },
    SearchError {
        token: u64,
        message: String,
    },
    PreviewDone {
        token: u64,
        data: Option<PreviewData>,
    },
    StatusLine(String),
    AdminDone {
        result: Result<StateSnapshot, String>,
        summary: String,
    },
}

enum SearchCmd {
    Search {
        token: u64,
        query: String,
        ext_group: String,
        sort: String,
        limit: usize,
    },
    Preview {
        token: u64,
        raw_path: String,
        query: String,
    },
    RefreshReader,
    Shutdown,
}

pub enum AdminCmd {
    Startup,
    RemoveRoot {
        folder: String,
    },
    RebuildRoot {
        folder: String,
    },
    SaveRulesAndRebuild {
        folders: Vec<PathBuf>,
        extensions: Vec<String>,
        exclude_dirs: Vec<String>,
        max_file_mb: u64,
    },
    SetExportFolder {
        folder: Option<PathBuf>,
    },
    Shutdown,
}

pub struct EngineHandle {
    search_tx: Sender<SearchCmd>,
    admin_tx: Sender<AdminCmd>,
    events_rx: Receiver<UiEvent>,
    search_thread: Option<JoinHandle<()>>,
    admin_thread: Option<JoinHandle<()>>,
}

#[derive(Clone, Copy)]
struct WindowNotify(isize);

impl WindowNotify {
    fn post(self) {
        unsafe {
            let _ = PostMessageW(
                Some(HWND(self.0 as *mut core::ffi::c_void)),
                WM_APP_EVENT,
                WPARAM(0),
                LPARAM(0),
            );
        }
    }
}

impl EngineHandle {
    pub fn start(hwnd: HWND) -> Self {
        let notify = WindowNotify(hwnd.0 as isize);
        let (ui_tx, ui_rx) = mpsc::channel::<UiEvent>();
        let (search_tx, search_rx) = mpsc::channel::<SearchCmd>();
        let (admin_tx, admin_rx) = mpsc::channel::<AdminCmd>();

        let search_ui_tx = ui_tx.clone();
        let search_thread = thread::Builder::new()
            .name("indexray-search".into())
            .spawn(move || search_worker(search_rx, search_ui_tx, notify))
            .expect("spawn search worker");

        let search_tx_for_admin = search_tx.clone();
        let admin_thread = thread::Builder::new()
            .name("indexray-admin".into())
            .spawn(move || admin_worker(admin_rx, ui_tx, search_tx_for_admin, notify))
            .expect("spawn admin worker");

        let _ = admin_tx.send(AdminCmd::Startup);

        Self {
            search_tx,
            admin_tx,
            events_rx: ui_rx,
            search_thread: Some(search_thread),
            admin_thread: Some(admin_thread),
        }
    }

    pub fn admin_sender(&self) -> Sender<AdminCmd> {
        self.admin_tx.clone()
    }

    pub fn try_recv(&self) -> Option<UiEvent> {
        self.events_rx.try_recv().ok()
    }

    pub fn request_search(
        &self,
        token: u64,
        query: String,
        ext_group: String,
        sort: String,
        limit: usize,
    ) {
        let _ = self.search_tx.send(SearchCmd::Search {
            token,
            query,
            ext_group,
            sort,
            limit,
        });
    }

    pub fn request_preview(&self, token: u64, raw_path: String, query: String) {
        let _ = self.search_tx.send(SearchCmd::Preview {
            token,
            raw_path,
            query,
        });
    }

    pub fn shutdown(&mut self) {
        let _ = self.search_tx.send(SearchCmd::Shutdown);
        let _ = self.admin_tx.send(AdminCmd::Shutdown);
        if let Some(h) = self.search_thread.take() {
            let _ = h.join();
        }
        if let Some(h) = self.admin_thread.take() {
            let _ = h.join();
        }
    }
}

impl Drop for EngineHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn emit(tx: &Sender<UiEvent>, notify: WindowNotify, ev: UiEvent) {
    if tx.send(ev).is_ok() {
        notify.post();
    }
}

fn search_worker(rx: Receiver<SearchCmd>, ui_tx: Sender<UiEvent>, notify: WindowNotify) {
    let Ok(index_dir) = config::index_dir() else {
        return;
    };
    let mut searcher: Option<Searcher> = Searcher::open(&index_dir).ok();
    let mut last_meta_sig = meta_signature(&index_dir);
    let mut last_ext_probe = Instant::now();

    while let Ok(first) = rx.recv() {
        // Coalesce bursty search/preview commands so fast typing only runs the latest query.
        let mut latest_search: Option<(u64, String, String, String, usize)> = None;
        let mut latest_preview: Option<(u64, String, String)> = None;
        let mut force_refresh = false;
        let mut quit = false;

        let mut apply = |cmd: SearchCmd| match cmd {
            SearchCmd::Search {
                token,
                query,
                ext_group,
                sort,
                limit,
            } => {
                latest_search = Some((token, query, ext_group, sort, limit));
            }
            SearchCmd::Preview {
                token,
                raw_path,
                query,
            } => {
                latest_preview = Some((token, raw_path, query));
            }
            SearchCmd::RefreshReader => {
                force_refresh = true;
            }
            SearchCmd::Shutdown => {
                quit = true;
            }
        };

        apply(first);
        while let Ok(next) = rx.try_recv() {
            apply(next);
        }
        if quit {
            break;
        }

        // Zero filesystem syscalls on rapid keystrokes; check external CLI commits at most every 400ms.
        let should_probe =
            force_refresh || searcher.is_none() || last_ext_probe.elapsed() >= Duration::from_millis(400);
        if should_probe {
            last_ext_probe = Instant::now();
            let current_sig = meta_signature(&index_dir);
            if force_refresh || current_sig != last_meta_sig || searcher.is_none() {
            if current_sig.is_some() {
                if let Some(s) = searcher.as_ref() {
                    if s.reload().is_err() {
                        searcher = Searcher::open(&index_dir).ok();
                    }
                } else {
                    searcher = Searcher::open(&index_dir).ok();
                }
                last_meta_sig = current_sig;
            } else {
                    searcher = None;
                    last_meta_sig = None;
                }
            }
        }

        if let Some((token, query, ext_group, sort, limit)) = latest_search {
            let trimmed = query.trim();
            if trimmed.is_empty() {
                emit(
                    &ui_tx,
                    notify,
                    UiEvent::SearchDone {
                        token,
                        hits: Vec::new(),
                        total: 0,
                        elapsed_ms: 0.0,
                    },
                );
            } else if let Some(s) = searcher.as_ref() {
                let t0 = Instant::now();
                let group = ext_group_by_name(Some(ext_group.as_str()));
                let sort_mode = SortMode::parse(Some(sort.as_str()));
                match s.search_paged(trimmed, limit, 0, group, sort_mode) {
                    Ok(page) => {
                        let elapsed_ms = t0.elapsed().as_secs_f64() * 1000.0;
                        let hits = page.hits.into_iter().map(hit_to_row).collect();
                        emit(
                            &ui_tx,
                            notify,
                            UiEvent::SearchDone {
                                token,
                                hits,
                                total: page.total,
                                elapsed_ms,
                            },
                        );
                    }
                    Err(err) => {
                        emit(
                            &ui_tx,
                            notify,
                            UiEvent::SearchError { token, message: format!("검색 조건을 확인하세요: {err}") },
                        );
                    }
                }
            } else {
                emit(
                    &ui_tx,
                    notify,
                    UiEvent::SearchDone {
                        token,
                        hits: Vec::new(),
                        total: 0,
                        elapsed_ms: 0.0,
                    },
                );
            }
        }

        if let Some((token, raw_path, query)) = latest_preview {
            if let Some(s) = searcher.as_ref() {
                let data = s
                    .preview(&raw_path, query.trim())
                    .ok()
                    .flatten()
                    .map(|p: PreviewHit| PreviewData {
                        raw_path,
                        snippet: p.snippet,
                        highlighted: p.highlighted,
                        size: p.size,
                        mtime: p.mtime,
                    });
                emit(&ui_tx, notify, UiEvent::PreviewDone { token, data });
            } else {
                emit(&ui_tx, notify, UiEvent::PreviewDone { token, data: None });
            }
        }
    }
}

fn hit_to_row(h: SearchHit) -> HitRow {
    if let Some((container_raw, _nid)) = h.path.split_once("::") {
        let container_disp = display_path(container_raw);
        let pst_name = Path::new(&container_disp)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| container_disp.clone());
        let file_name = match h.title.as_deref() {
            Some(t) if !t.is_empty() => format!("[메일] {t}"),
            _ => format!("[메일] {pst_name}"),
        };
        return HitRow {
            raw_path: h.path,
            display_path: container_disp.clone(),
            file_name,
            parent_dir: container_disp,
            ext: h.ext,
            snippet: h.snippet,
            highlighted: h.highlighted,
            author: h.author,
            title: h.title,
        };
    }

    let disp = display_path(&h.path);
    let path_obj = Path::new(&disp);
    let file_name = path_obj
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| disp.clone());
    let parent_dir = path_obj
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default();
    HitRow {
        raw_path: h.path,
        display_path: disp,
        file_name,
        parent_dir,
        ext: h.ext,
        snippet: h.snippet,
        highlighted: h.highlighted,
        author: h.author,
        title: h.title,
    }
}

fn meta_signature(index_dir: &Path) -> Option<Vec<u8>> {
    std::fs::read(index_dir.join("meta.json")).ok()
}

struct RunningWatch {
    stop: Arc<AtomicBool>,
    watch_thread: JoinHandle<()>,
    export_stop: Arc<AtomicBool>,
    export_wake: Sender<()>,
    export_thread: JoinHandle<()>,
}

impl RunningWatch {
    fn start(
        index_dir: &Path,
        export_status: Arc<Mutex<String>>,
        search_tx: Sender<SearchCmd>,
        ui_tx: Sender<UiEvent>,
        notify: WindowNotify,
    ) -> Option<Self> {
        let roots = safe_roots(index_dir);
        if roots.is_empty() {
            return None;
        }
        let updater = match IndexUpdater::open(index_dir) {
            Ok(u) => Arc::new(Mutex::new(u)),
            Err(err) => {
                log_line("watch", &format!("IndexUpdater 열기 실패: {err:#}"));
                return None;
            }
        };

        let stop = Arc::new(AtomicBool::new(false));
        let stop_signal = stop.clone();
        let watch_index = index_dir.to_path_buf();
        let search_tx_watch = search_tx.clone();
        let ui_tx_watch = ui_tx.clone();

        let (export_wake, export_wake_rx) = mpsc::channel::<()>();
        let export_wake_for_watch = export_wake.clone();
        let export_wake_for_ocr = export_wake.clone();
        let search_tx_ocr = search_tx.clone();

        let watch_thread = thread::Builder::new()
            .name("indexray-watcher".into())
            .spawn(move || {
                // Defer heavy OCR worker pool briefly so startup search/reconcile has zero CPU/lock contention.
                let stop_for_ocr = stop_signal.clone();
                let updater_for_ocr = updater.clone();
                let index_for_ocr = watch_index.clone();
                let ocr_starter = thread::Builder::new()
                    .name("indexray-ocr-init".into())
                    .spawn(move || {
                        for _ in 0..25 {
                            if stop_for_ocr.load(Ordering::Relaxed) {
                                return None;
                            }
                            thread::sleep(Duration::from_millis(100));
                        }
                        OcrPipeline::start(
                            updater_for_ocr,
                            index_for_ocr,
                            2,
                            move |_| {
                                let _ = search_tx_ocr.send(SearchCmd::RefreshReader);
                                let _ = export_wake_for_ocr.send(());
                            },
                        )
                    })
                    .ok();

                let res = watch_roots_auto(&watch_index, &roots, updater, stop_signal, move |ev| {
                    match ev {
                        WatchProgress::Committed { .. } => {
                            let _ = search_tx_watch.send(SearchCmd::RefreshReader);
                            let _ = export_wake_for_watch.send(());
                        }
                        WatchProgress::CommitFailed(err) => {
                            log_line("watch", &format!("색인 반영 오류: {err}"));
                            emit(
                                &ui_tx_watch,
                                notify,
                                UiEvent::StatusLine(format!("색인 반영 오류: {err}")),
                            );
                        }
                        _ => {}
                    }
                });
                if let Some(handle) = ocr_starter
                    && let Ok(Some(p)) = handle.join()
                {
                    p.stop();
                }
                if let Err(err) = res {
                    log_line("watch", &format!("파일 감시 종료: {err:#}"));
                }
            })
            .expect("spawn watcher");

        let export_stop = Arc::new(AtomicBool::new(false));
        let export_stop_flag = export_stop.clone();
        let export_index = index_dir.to_path_buf();
        let export_thread = thread::Builder::new()
            .name("indexray-exporter".into())
            .spawn(move || {
                let mut previous_key: Option<(PathBuf, Option<Vec<u8>>)> = None;
                loop {
                    if export_stop_flag.load(Ordering::Relaxed) {
                        break;
                    }
                    let cfg = config::load();
                    if let Some(folder) = cfg.export_folder {
                        let rev = meta_signature(&export_index);
                        let key = (folder.clone(), rev);
                        if previous_key.as_ref() != Some(&key) {
                            match dowse::export::sync(&export_index, &folder) {
                                Ok(msg) => {
                                    *export_status.lock().unwrap() = msg;
                                    previous_key = Some(key);
                                }
                                Err(err) => {
                                    let err_msg = format!("내보내기 오류: {err:#}");
                                    log_line("export", &err_msg);
                                    *export_status.lock().unwrap() = err_msg;
                                }
                            }
                        }
                    } else {
                        previous_key = None;
                        export_status.lock().unwrap().clear();
                    }

                    let _ = export_wake_rx.recv_timeout(Duration::from_millis(1200));
                    while export_wake_rx.try_recv().is_ok() {}
                }
            })
            .expect("spawn exporter");

        Some(Self {
            stop,
            watch_thread,
            export_stop,
            export_wake,
            export_thread,
        })
    }

    fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        self.export_stop.store(true, Ordering::Relaxed);
        let _ = self.export_wake.send(());
        let _ = self.watch_thread.join();
        let _ = self.export_thread.join();
    }
}

fn safe_roots(index_dir: &Path) -> Vec<PathBuf> {
    if !index_dir.exists() {
        return Vec::new();
    }
    registered_roots(index_dir).unwrap_or_default()
}

fn registered_root_labels(roots: &[PathBuf], cfg: &AppConfig) -> Vec<String> {
    use std::path::{Component, Prefix};
    use windows::Win32::Storage::FileSystem::{GetDriveTypeW, GetLogicalDrives};
    use windows::core::PCWSTR;

    // Old profiles stored only the resolved UNC path. Recover labels from connected drives.
    let mut drives = Vec::new();
    if roots.iter().any(|root| {
        matches!(root.components().next(), Some(Component::Prefix(p))
            if matches!(p.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..)))
            && !cfg.root_aliases.contains_key(root)
    }) {
        let mask = unsafe { GetLogicalDrives() };
        for i in 0..26 {
            if mask & (1 << i) == 0 {
                continue;
            }
            let drive = [u16::from(b'A') + i, u16::from(b':'), u16::from(b'\\'), 0];
            const DRIVE_REMOTE: u32 = 4;
            if unsafe { GetDriveTypeW(PCWSTR(drive.as_ptr())) } == DRIVE_REMOTE {
                let alias = PathBuf::from(String::from_utf16_lossy(&drive[..3]));
                if let Ok(resolved) = alias.canonicalize() {
                    drives.push((alias, resolved));
                }
            }
        }
    }
    roots.iter().map(|root| {
        if let Some(alias) = cfg.root_aliases.get(root) {
            return display_path(&alias.to_string_lossy());
        }
        if let Some((alias, suffix)) = drives.iter()
            .filter_map(|(alias, resolved)| root.strip_prefix(resolved).ok().map(|tail| (alias, tail)))
            .min_by_key(|(_, suffix)| suffix.as_os_str().len())
        {
            return alias.join(suffix).to_string_lossy().into_owned();
        }
        display_path(&root.to_string_lossy())
    }).collect()
}

fn index_progress_path(path: &Path, root: &Path, label: &str) -> String {
    match path.strip_prefix(root) {
        Ok(relative) => Path::new(label).join(relative).to_string_lossy().into_owned(),
        Err(_) => display_path(&path.to_string_lossy()),
    }
}

fn collect_snapshot_inner(
    index_dir: &Path,
    export_status: &Arc<Mutex<String>>,
    check_ko_dic: bool,
) -> StateSnapshot {
    let roots_raw = safe_roots(index_dir);
    let cfg = config::load();
    let roots = registered_root_labels(&roots_raw, &cfg);
    let (has_index, num_docs) = if !roots.is_empty() && index_dir.join("meta.json").is_file() {
        match Searcher::open(index_dir) {
            Ok(s) => (true, s.num_docs()),
            Err(_) => (false, 0),
        }
    } else {
        (false, 0)
    };
    let mut rules = load_rules(index_dir);
    rules.enabled_extensions = Some(rules.effective_extensions());
    let autostart_enabled = config::is_autostart_enabled();
    let ko_reindex_roots = if has_index && check_ko_dic {
        let roots = korean_reindex_roots(index_dir).unwrap_or_default();
        registered_root_labels(&roots, &cfg)
    } else {
        Vec::new()
    };
    let exp_status = export_status.lock().unwrap().clone();

    StateSnapshot {
        has_index,
        num_docs,
        roots,
        rules,
        config: cfg,
        autostart_enabled,
        ko_reindex_roots,
        export_status: exp_status,
    }
}

fn collect_snapshot(index_dir: &Path, export_status: &Arc<Mutex<String>>) -> StateSnapshot {
    collect_snapshot_inner(index_dir, export_status, true)
}

fn admin_worker(
    rx: Receiver<AdminCmd>,
    ui_tx: Sender<UiEvent>,
    search_tx: Sender<SearchCmd>,
    notify: WindowNotify,
) {
    let Ok(index_dir) = config::index_dir() else {
        return;
    };
    let export_status = Arc::new(Mutex::new(String::new()));
    let mut watch: Option<RunningWatch> = None;

    while let Ok(cmd) = rx.recv() {
        match cmd {
            AdminCmd::Startup => {
                // Emit initial state FIRST so the UI shows ready doc counts in <200ms!
                let fast_snap = collect_snapshot_inner(&index_dir, &export_status, false);
                let _ = search_tx.send(SearchCmd::RefreshReader);
                emit(&ui_tx, notify, UiEvent::StateReady(fast_snap));

                let cfg = config::load();
                config::ensure_default_autostart(&cfg);
                if watch.is_none() {
                    watch = RunningWatch::start(
                        &index_dir,
                        export_status.clone(),
                        search_tx.clone(),
                        ui_tx.clone(),
                        notify,
                    );
                }
            }
            AdminCmd::RemoveRoot { folder } => {
                if let Some(w) = watch.take() {
                    w.stop();
                }
                let res = remove_root_impl(&index_dir, &folder, &export_status);
                watch = RunningWatch::start(
                    &index_dir,
                    export_status.clone(),
                    search_tx.clone(),
                    ui_tx.clone(),
                    notify,
                );
                let _ = search_tx.send(SearchCmd::RefreshReader);
                finish_admin_op(res, &index_dir, &export_status, &ui_tx, notify);
            }
            AdminCmd::RebuildRoot { folder } => {
                if let Some(w) = watch.take() {
                    w.stop();
                }
                let res = rebuild_root_impl(&index_dir, &folder, &ui_tx, notify, &export_status);
                watch = RunningWatch::start(
                    &index_dir,
                    export_status.clone(),
                    search_tx.clone(),
                    ui_tx.clone(),
                    notify,
                );
                let _ = search_tx.send(SearchCmd::RefreshReader);
                finish_admin_op(res, &index_dir, &export_status, &ui_tx, notify);
            }
            AdminCmd::SaveRulesAndRebuild {
                folders,
                extensions,
                exclude_dirs,
                max_file_mb,
            } => {
                if let Some(w) = watch.take() {
                    w.stop();
                }
                let res = save_rules_and_rebuild_impl(
                    &index_dir,
                    folders,
                    extensions,
                    exclude_dirs,
                    max_file_mb,
                    &ui_tx,
                    notify,
                    &export_status,
                );
                watch = RunningWatch::start(
                    &index_dir,
                    export_status.clone(),
                    search_tx.clone(),
                    ui_tx.clone(),
                    notify,
                );
                let _ = search_tx.send(SearchCmd::RefreshReader);
                finish_admin_op(res, &index_dir, &export_status, &ui_tx, notify);
            }
            AdminCmd::SetExportFolder { folder } => {
                let res = set_export_folder_impl(&index_dir, folder, &export_status);
                finish_admin_op(res, &index_dir, &export_status, &ui_tx, notify);
            }
            AdminCmd::Shutdown => {
                if let Some(w) = watch.take() {
                    w.stop();
                }
                break;
            }
        }
    }
}

fn finish_admin_op(
    res: Result<String>,
    index_dir: &Path,
    export_status: &Arc<Mutex<String>>,
    ui_tx: &Sender<UiEvent>,
    notify: WindowNotify,
) {
    match res {
        Ok(summary) => {
            let snap = collect_snapshot(index_dir, export_status);
            emit(
                ui_tx,
                notify,
                UiEvent::AdminDone {
                    result: Ok(snap),
                    summary,
                },
            );
        }
        Err(err) => {
            emit(
                ui_tx,
                notify,
                UiEvent::AdminDone {
                    result: Err(format!("{err:#}")),
                    summary: String::new(),
                },
            );
        }
    }
}

fn resolve_registered_root(index_dir: &Path, display_or_raw: &str) -> Result<PathBuf> {
    let trimmed = display_or_raw.trim();
    let roots = registered_roots(index_dir)?;
    let labels = registered_root_labels(&roots, &config::load());
    roots.into_iter().zip(labels)
        .find_map(|(root, label)| {
            (label.eq_ignore_ascii_case(trimmed)
                || display_path(&root.to_string_lossy()).eq_ignore_ascii_case(trimmed))
                .then_some(root)
        })
        .context("등록된 색인 폴더를 찾을 수 없습니다")
}

fn sync_configured_export(index_dir: &Path, export_status: &Arc<Mutex<String>>) -> String {
    let Some(out) = config::load().export_folder else {
        return String::new();
    };
    match dowse::export::sync(index_dir, &out) {
        Ok(msg) => {
            *export_status.lock().unwrap() = msg;
            String::new()
        }
        Err(err) => {
            let msg = format!("Markdown 동기화 실패: {err:#}");
            log_line("export", &msg);
            *export_status.lock().unwrap() = msg.clone();
            format!(" · {msg}")
        }
    }
}

fn remove_root_impl(
    index_dir: &Path,
    folder: &str,
    export_status: &Arc<Mutex<String>>,
) -> Result<String> {
    let target = resolve_registered_root(index_dir, folder)?;
    let mut updater = IndexUpdater::open(index_dir)?;
    let stats = remove_root(index_dir, &target, &mut updater)?;
    drop(updater);
    let export_note = sync_configured_export(index_dir, export_status);
    Ok(format!("폴더 해제 완료: {}개 문서 제거됨{export_note}", stats.removed))
}

fn rebuild_root_impl(
    index_dir: &Path,
    folder: &str,
    ui_tx: &Sender<UiEvent>,
    notify: WindowNotify,
    export_status: &Arc<Mutex<String>>,
) -> Result<String> {
    let target = resolve_registered_root(index_dir, folder)?;
    let t0 = Instant::now();
    let mut updater = IndexUpdater::open(index_dir)?;
    let stats = rebuild_root_with_progress(index_dir, &target, &mut updater, |p| {
        emit(
            ui_tx,
            notify,
            UiEvent::StatusLine(format!(
                "다시 색인 중… {}개 처리됨 ({})",
                p.processed,
                index_progress_path(&p.path, &target, folder)
            )),
        );
    })?;
    drop(updater);
    let export_note = sync_configured_export(index_dir, export_status);
    Ok(format!(
        "다시 색인 완료: {}개 문서 반영 ({:.1}초){export_note}",
        stats.indexed,
        t0.elapsed().as_secs_f64()
    ))
}

fn save_rules_and_rebuild_impl(
    index_dir: &Path,
    folders: Vec<PathBuf>,
    extensions: Vec<String>,
    exclude_dirs: Vec<String>,
    max_file_mb: u64,
    ui_tx: &Sender<UiEvent>,
    notify: WindowNotify,
    export_status: &Arc<Mutex<String>>,
) -> Result<String> {
    let mut cfg = config::load();
    let has_index = index_dir.join("meta.json").try_exists()?;
    let roots = if has_index {
        registered_roots(index_dir)?
    } else {
        Vec::new()
    };
    let path_key = |path: &Path| {
        PathBuf::from(display_path(&path.to_string_lossy()).to_lowercase())
    };
    let mut keys: Vec<_> = roots
        .iter()
        .map(|root| path_key(&root.canonicalize().unwrap_or_else(|_| root.clone())))
        .collect();
    let mut aliases_changed = false;
    let out_key = cfg.export_folder.as_deref().map(|out| {
        path_key(&out.canonicalize().unwrap_or_else(|_| out.to_owned()))
    });
    let mut additions = Vec::with_capacity(folders.len());
    for folder in folders {
        let canon = folder.canonicalize()
            .with_context(|| format!("폴더를 열 수 없습니다: {}", folder.display()))?;
        ensure!(canon.is_dir(), "폴더가 아닙니다: {}", folder.display());
        let key = path_key(&canon);
        let existing = keys.iter().position(|k| k == &key);
        let actual = existing.map_or(&canon, |i| {
            if i < roots.len() { &roots[i] } else { &additions[i - roots.len()] }
        });
        if matches!(folder.components().next(), Some(std::path::Component::Prefix(p))
            if matches!(p.kind(), std::path::Prefix::Disk(_) | std::path::Prefix::VerbatimDisk(_)))
            && folder.has_root()
            && path_key(&folder) != key
            && cfg.root_aliases.get(actual) != Some(&folder)
        {
            cfg.root_aliases.retain(|_, alias| !alias.as_os_str().eq_ignore_ascii_case(folder.as_os_str()));
            cfg.root_aliases.insert(actual.clone(), folder.clone());
            aliases_changed = true;
        }
        if existing.is_some() {
            continue;
        }
        ensure!(
            !keys.iter().any(|root| root.starts_with(&key) || key.starts_with(root)),
            "등록한 폴더나 추가할 폴더와 상하위 경로가 겹칩니다: {}", folder.display()
        );
        ensure!(
            !out_key.as_ref().is_some_and(|out| out.starts_with(&key)),
            "Markdown 내보내기 폴더가 포함되어 있습니다: {}", folder.display()
        );
        keys.push(key);
        additions.push(canon);
    }
    let previous = load_rules(index_dir);
    let mut rules = previous.clone();
    rules.enabled_extensions = Some(extensions);
    rules.extra_text_exts.clear();
    rules.exclude_dirs = exclude_dirs;
    rules.max_file_mb = max_file_mb.max(1);
    rules.normalize();
    let rules_changed = previous.effective_extensions() != rules.effective_extensions()
        || previous.exclude_dirs != rules.exclude_dirs
        || previous.max_file_mb != rules.max_file_mb;
    if rules_changed {
        save_rules(index_dir, &rules)?;
    }
    if aliases_changed {
        config::save(&cfg)?;
    }
    let progress = |root: &Path, label: &str, p: dowse::IndexProgress| {
        emit(ui_tx, notify, UiEvent::StatusLine(format!(
            "색인 중… {}개 처리됨 ({})", p.processed, index_progress_path(&p.path, root, label)
        )));
    };
    let rebuild = rules_changed && !roots.is_empty();
    if !rebuild && additions.is_empty() {
        return Ok(if rules_changed || aliases_changed { "설정을 저장했습니다." } else { "변경 사항이 없습니다." }.into());
    }
    let mut total_indexed = 0;
    let mut pending = additions.iter();
    // A live reader may still hold the empty index after its last root was removed.
    if !has_index && let Some(first) = pending.next() {
        let label = &registered_root_labels(std::slice::from_ref(first), &cfg)[0];
        total_indexed += rebuild_index_with_progress(index_dir, first, |p| progress(first, label, p))?.indexed;
    }
    let remaining = pending.as_slice();
    if rebuild || !remaining.is_empty() {
        let mut updater = IndexUpdater::open(index_dir)?;
        if rebuild {
            let labels = registered_root_labels(&roots, &cfg);
            for (root, label) in roots.iter().zip(labels) {
                total_indexed += rebuild_root_with_progress(index_dir, root, &mut updater, |p| progress(root, &label, p))?.indexed;
            }
        }
        let labels = registered_root_labels(remaining, &cfg);
        for (root, label) in remaining.iter().zip(labels) {
            total_indexed += index_root_incremental_with_progress(index_dir, root, &mut updater, |p| progress(root, &label, p))?.indexed;
        }
    }
    if let Some(last) = additions.last() {
        cfg.target_dir = Some(last.clone());
        config::save(&cfg)?;
    }
    let export_note = sync_configured_export(index_dir, export_status);
    Ok(format!("색인 완료: {total_indexed}개 문서를 반영했습니다.{export_note}"))
}

fn set_export_folder_impl(
    index_dir: &Path,
    folder: Option<PathBuf>,
    export_status: &Arc<Mutex<String>>,
) -> Result<String> {
    if let Some(dir) = &folder {
        if !dir.exists() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("출력 폴더를 만들 수 없습니다: {}", dir.display()))?;
        }
        let canon = dir
            .canonicalize()
            .with_context(|| format!("출력 폴더를 열 수 없습니다: {}", dir.display()))?;
        for r in safe_roots(index_dir) {
            if let Ok(r_canon) = r.canonicalize() {
                ensure!(
                    !canon.starts_with(&r_canon),
                    "출력 폴더는 검색 폴더 밖에 지정하세요"
                );
            }
        }
        let summary = if index_dir.join("meta.json").is_file() {
            let msg = dowse::export::sync(index_dir, &canon)?;
            *export_status.lock().unwrap() = msg.clone();
            msg
        } else {
            let msg = "내보내기 폴더가 저장되었습니다 (색인 폴더 등록 후 동기화됨).".to_string();
            *export_status.lock().unwrap() = msg.clone();
            msg
        };
        let mut cfg = config::load();
        cfg.export_folder = Some(canon);
        config::save(&cfg)?;
        Ok(format!("Markdown 동기화 완료: {summary}"))
    } else {
        let mut cfg = config::load();
        cfg.export_folder = None;
        config::save(&cfg)?;
        export_status.lock().unwrap().clear();
        Ok("Markdown 내보내기를 중지했습니다 (기존 생성물은 유지됨).".into())
    }
}
