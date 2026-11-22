//! # zipnest-core
//!
//! The offline zip/unzip engine behind the ZipNest desktop app.
//!
//! * **Streaming** – files are never fully loaded into memory, so multi‑GB
//!   inputs are fine.
//! * **Codebase aware** – honours `.gitignore` and skips common junk folders
//!   (`node_modules`, `.git`, `__pycache__`, …).
//! * **Safe extraction** – blocks path traversal ("zip‑slip"), refuses to
//!   create symlinks, and detects zip bombs.
//! * **Optional AES‑256 encryption** when creating archives.
//! * **Atomic output** – archives are written to a temporary file and only
//!   renamed into place when complete; cancelled or failed extractions are
//!   cleaned up.
//!
//! The crate does no networking whatsoever.

mod compress;
mod extract;

use serde::Serialize;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

pub use compress::{
    compress, CompressOptions, CompressSummary, CompressionLevel, DEFAULT_EXCLUDES,
};
pub use extract::{extract, inspect, ArchiveInfo, EntryPreview, ExtractOptions, ExtractSummary};

/// Errors returned by ZipNest operations. `Display` output is written for end users.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Could not access \"{}\": {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("ZIP error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("\"{}\" does not exist", .0.display())]
    NotFound(PathBuf),
    #[error(
        "Nothing to compress: no files were selected, or every file was excluded by your filters"
    )]
    NothingToCompress,
    #[error("Operation cancelled")]
    Cancelled,
    #[error("This archive is password-protected. Enter the password and try again.")]
    PasswordRequired,
    #[error("Incorrect password")]
    WrongPassword,
    #[error("Blocked for your safety: {0}")]
    Unsafe(String),
    #[error("{0}")]
    Invalid(String),
}

impl Error {
    /// Short machine-readable code, handy for UIs.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Io { .. } => "io",
            Error::Zip(_) => "zip",
            Error::NotFound(_) => "not_found",
            Error::NothingToCompress => "nothing_to_compress",
            Error::Cancelled => "cancelled",
            Error::PasswordRequired => "password_required",
            Error::WrongPassword => "wrong_password",
            Error::Unsafe(_) => "unsafe",
            Error::Invalid(_) => "invalid",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn io_err(path: &Path) -> impl FnOnce(io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Cooperative cancellation flag shared between the UI and a running job.
#[derive(Clone, Default, Debug)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
    pub(crate) fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(Error::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Scanning,
    Compressing,
    Extracting,
    Finalizing,
}

/// A progress snapshot. Emitted at most every ~60 ms plus on phase changes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub phase: Phase,
    pub processed_bytes: u64,
    pub total_bytes: u64,
    pub processed_files: u64,
    pub total_files: u64,
    pub current: String,
}

/// An item that was skipped (and why), reported back to the user.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub path: String,
    pub reason: String,
}

pub(crate) const MAX_SKIPPED_REPORTED: usize = 500;

pub(crate) fn push_skipped(
    list: &mut Vec<Skipped>,
    count: &mut u64,
    path: impl Into<String>,
    reason: impl Into<String>,
) {
    *count += 1;
    if list.len() < MAX_SKIPPED_REPORTED {
        list.push(Skipped {
            path: path.into(),
            reason: reason.into(),
        });
    }
}

pub(crate) struct Reporter<'a> {
    cb: &'a mut dyn FnMut(&Progress),
    last: Option<Instant>,
    pub p: Progress,
}

impl<'a> Reporter<'a> {
    pub fn new(cb: &'a mut dyn FnMut(&Progress), phase: Phase) -> Self {
        Self {
            cb,
            last: None,
            p: Progress {
                phase,
                processed_bytes: 0,
                total_bytes: 0,
                processed_files: 0,
                total_files: 0,
                current: String::new(),
            },
        }
    }

    pub fn phase(&mut self, phase: Phase) {
        self.p.phase = phase;
        self.emit(true);
    }

    pub fn emit(&mut self, force: bool) {
        let due = self
            .last
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(60));
        if force || due {
            (self.cb)(&self.p);
            self.last = Some(Instant::now());
        }
    }
}

/// Return `dir/name`, or `dir/name (2)`, `dir/name (3)` … whichever does not exist yet.
pub(crate) fn unique_path(dir: &Path, name: &str) -> PathBuf {
    let first = dir.join(name);
    if !first.exists() {
        return first;
    }
    for n in 2..10_000 {
        let candidate = dir.join(format!("{name} ({n})"));
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{name} ({})", std::process::id()))
}

pub(crate) fn local_offset() -> time::UtcOffset {
    time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC)
}

/// Human friendly byte formatting (used in messages).
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
