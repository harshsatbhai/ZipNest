//! ZipNest desktop shell: thin Tauri commands around `zipnest-core`.
//!
//! The app never opens a network connection. All work happens locally on a
//! background thread; progress is streamed to the UI through events.

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use zipnest_core::{
    ArchiveInfo, CancelToken, CompressOptions, CompressSummary, ExtractOptions, ExtractSummary,
    Progress,
};

const PROGRESS_EVENT: &str = "zipnest://progress";
const OPEN_FILES_EVENT: &str = "zipnest://open-files";

#[derive(Default)]
struct Jobs(Mutex<HashMap<String, CancelToken>>);

/// Files the OS asked us to open (double-click on a .zip, "Open with ZipNest").
#[derive(Default)]
struct PendingOpen(Mutex<Vec<String>>);

#[derive(Debug, Serialize)]
struct CmdError {
    code: String,
    message: String,
}

impl From<zipnest_core::Error> for CmdError {
    fn from(e: zipnest_core::Error) -> Self {
        CmdError {
            code: e.code().to_string(),
            message: e.to_string(),
        }
    }
}

impl CmdError {
    fn internal(msg: impl ToString) -> Self {
        CmdError {
            code: "internal".into(),
            message: msg.to_string(),
        }
    }
}

type CmdResult<T> = Result<T, CmdError>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProgressEvent {
    job_id: String,
    #[serde(flatten)]
    progress: Progress,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PathInfo {
    path: String,
    name: String,
    is_dir: bool,
    size: Option<u64>,
    exists: bool,
}

fn register_job(jobs: &State<'_, Jobs>, id: &str) -> CmdResult<CancelToken> {
    let token = CancelToken::new();
    let mut map = jobs.0.lock().map_err(CmdError::internal)?;
    if map.contains_key(id) {
        return Err(CmdError::internal("A job with this id is already running"));
    }
    map.insert(id.to_string(), token.clone());
    Ok(token)
}

fn finish_job(jobs: &State<'_, Jobs>, id: &str) {
    if let Ok(mut map) = jobs.0.lock() {
        map.remove(id);
    }
}

fn progress_emitter(app: AppHandle, job_id: String) -> impl FnMut(&Progress) {
    move |p: &Progress| {
        let _ = app.emit(
            PROGRESS_EVENT,
            ProgressEvent {
                job_id: job_id.clone(),
                progress: p.clone(),
            },
        );
    }
}

#[tauri::command]
async fn compress(
    app: AppHandle,
    jobs: State<'_, Jobs>,
    job_id: String,
    inputs: Vec<String>,
    output: String,
    options: CompressOptions,
) -> CmdResult<CompressSummary> {
    let token = register_job(&jobs, &job_id)?;
    let mut emit = progress_emitter(app, job_id.clone());
    let inputs: Vec<PathBuf> = inputs.into_iter().map(PathBuf::from).collect();
    let result = tauri::async_runtime::spawn_blocking(move || {
        zipnest_core::compress(&inputs, &PathBuf::from(output), &options, &token, &mut emit)
    })
    .await;
    finish_job(&jobs, &job_id);
    result.map_err(CmdError::internal)?.map_err(Into::into)
}

#[tauri::command]
async fn extract(
    app: AppHandle,
    jobs: State<'_, Jobs>,
    job_id: String,
    archive: String,
    destination: String,
    options: ExtractOptions,
) -> CmdResult<ExtractSummary> {
    let token = register_job(&jobs, &job_id)?;
    let mut emit = progress_emitter(app, job_id.clone());
    let result = tauri::async_runtime::spawn_blocking(move || {
        zipnest_core::extract(
            &PathBuf::from(archive),
            &PathBuf::from(destination),
            &options,
            &token,
            &mut emit,
        )
    })
    .await;
    finish_job(&jobs, &job_id);
    result.map_err(CmdError::internal)?.map_err(Into::into)
}

#[tauri::command]
async fn inspect(archive: String) -> CmdResult<ArchiveInfo> {
    tauri::async_runtime::spawn_blocking(move || zipnest_core::inspect(&PathBuf::from(archive)))
        .await
        .map_err(CmdError::internal)?
        .map_err(Into::into)
}

#[tauri::command]
fn cancel(jobs: State<'_, Jobs>, job_id: String) -> bool {
    match jobs.0.lock() {
        Ok(map) => map.get(&job_id).map(|t| t.cancel()).is_some(),
        Err(_) => false,
    }
}

#[tauri::command]
fn describe_paths(paths: Vec<String>) -> Vec<PathInfo> {
    paths
        .into_iter()
        .map(|p| {
            let pb = PathBuf::from(&p);
            let meta = std::fs::metadata(&pb).ok();
            PathInfo {
                name: pb
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.clone()),
                is_dir: meta.as_ref().map(|m| m.is_dir()).unwrap_or(false),
                size: meta.as_ref().filter(|m| m.is_file()).map(|m| m.len()),
                exists: meta.is_some(),
                path: p,
            }
        })
        .collect()
}

#[tauri::command]
fn default_excludes() -> Vec<&'static str> {
    zipnest_core::DEFAULT_EXCLUDES.to_vec()
}

/// Returns (and clears) files the OS asked us to open before the UI was ready.
#[tauri::command]
fn take_pending_open(pending: State<'_, PendingOpen>) -> Vec<String> {
    pending
        .0
        .lock()
        .map(|mut v| std::mem::take(&mut *v))
        .unwrap_or_default()
}

fn queue_open(app: &AppHandle, files: Vec<String>) {
    if files.is_empty() {
        return;
    }
    if let Some(state) = app.try_state::<PendingOpen>() {
        if let Ok(mut v) = state.0.lock() {
            v.extend(files.iter().cloned());
        }
    }
    let _ = app.emit(OPEN_FILES_EVENT, files);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Jobs::default())
        .manage(PendingOpen::default())
        .setup(|app| {
            // Windows & Linux pass "Open with" files as command-line arguments.
            let files: Vec<String> = std::env::args()
                .skip(1)
                .filter(|a| !a.starts_with('-') && std::path::Path::new(a).is_file())
                .collect();
            queue_open(app.handle(), files);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            compress,
            extract,
            inspect,
            cancel,
            describe_paths,
            default_excludes,
            take_pending_open
        ])
        .build(tauri::generate_context!())
        .expect("error while building ZipNest");

    app.run(|_handle, _event| {
        // macOS delivers "Open with" files through an event instead of argv.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = _event {
            let files = urls
                .into_iter()
                .filter_map(|u| u.to_file_path().ok())
                .map(|p| p.to_string_lossy().to_string())
                .collect();
            queue_open(_handle, files);
        }
    });
}
