use crate::{io_err, push_skipped, CancelToken, Error, Phase, Progress, Reporter, Result, Skipped};
use ignore::overrides::OverrideBuilder;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;
use zip::{AesMode, CompressionMethod, ZipWriter};

/// Folders/files skipped when `skip_common_junk` is on. Matched at any depth.
pub const DEFAULT_EXCLUDES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".venv",
    ".next",
    ".nuxt",
    ".turbo",
    ".parcel-cache",
    ".gradle",
    ".DS_Store",
    "Thumbs.db",
    "desktop.ini",
];

/// File types that are already compressed; deflating them again wastes time.
const PRECOMPRESSED: &[&str] = &[
    "zip", "gz", "tgz", "bz2", "xz", "zst", "7z", "rar", "jar", "war", "apk", "aab", "ipa", "jpg",
    "jpeg", "png", "gif", "webp", "avif", "heic", "mp3", "aac", "ogg", "opus", "flac", "m4a",
    "mp4", "m4v", "mov", "mkv", "webm", "avi", "woff", "woff2", "pdf", "docx", "xlsx", "pptx",
    "odt", "dmg", "iso",
];

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CompressionLevel {
    /// No compression, just packaging (fastest).
    Store,
    Fast,
    #[default]
    Balanced,
    Maximum,
}

impl CompressionLevel {
    fn deflate_level(self) -> Option<i64> {
        match self {
            CompressionLevel::Store => None,
            CompressionLevel::Fast => Some(1),
            CompressionLevel::Balanced => Some(6),
            CompressionLevel::Maximum => Some(9),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CompressOptions {
    pub level: CompressionLevel,
    /// Honour `.gitignore`, `.ignore` and `.git/info/exclude` files.
    pub respect_gitignore: bool,
    /// Skip [`DEFAULT_EXCLUDES`].
    pub skip_common_junk: bool,
    /// Include dot-files such as `.env` or `.eslintrc`.
    pub include_hidden: bool,
    /// Encrypt every file with AES-256 using this password.
    pub password: Option<String>,
}

impl Default for CompressOptions {
    fn default() -> Self {
        Self {
            level: CompressionLevel::Balanced,
            respect_gitignore: true,
            skip_common_junk: true,
            include_hidden: true,
            password: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompressSummary {
    pub output: PathBuf,
    pub files: u64,
    pub folders: u64,
    pub original_bytes: u64,
    pub archive_bytes: u64,
    pub encrypted: bool,
    pub skipped_count: u64,
    pub skipped: Vec<Skipped>,
}

enum Item {
    Dir {
        name: String,
    },
    File {
        path: PathBuf,
        name: String,
        size: u64,
        mode: u32,
        modified: Option<std::time::SystemTime>,
    },
}

/// Compress `inputs` (files and/or folders) into the zip file at `output`.
///
/// Each folder is stored under its own name (`project/src/main.rs`), each file at
/// the archive root. If `output` exists it is replaced only after the new archive
/// has been written successfully.
pub fn compress(
    inputs: &[PathBuf],
    output: &Path,
    opts: &CompressOptions,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(&Progress),
) -> Result<CompressSummary> {
    if inputs.is_empty() {
        return Err(Error::NothingToCompress);
    }
    if let Some(pw) = &opts.password {
        if pw.is_empty() {
            return Err(Error::Invalid("Password cannot be empty".into()));
        }
    }

    let out_dir = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let out_name = output
        .file_name()
        .ok_or_else(|| Error::Invalid("Output path must include a file name".into()))?
        .to_string_lossy()
        .to_string();
    let out_dir = fs::canonicalize(out_dir).map_err(io_err(out_dir))?;
    let output = out_dir.join(&out_name);
    let temp = out_dir.join(format!(".{out_name}.zipnest-partial"));

    let mut rep = Reporter::new(on_progress, Phase::Scanning);
    rep.emit(true);

    let mut skipped = Vec::new();
    let mut skipped_count = 0u64;

    // ---------- Pass 1: scan ----------
    let mut items: Vec<Item> = Vec::new();
    let mut used_roots: HashSet<String> = HashSet::new();

    for input in inputs {
        cancel.check()?;
        let meta = fs::symlink_metadata(input).map_err(|_| Error::NotFound(input.clone()))?;
        let canon = fs::canonicalize(input).map_err(io_err(input))?;
        let base_name = canon
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "archive".to_string());
        let root = dedupe_root(&mut used_roots, &base_name);

        if meta.file_type().is_symlink() {
            push_skipped(
                &mut skipped,
                &mut skipped_count,
                input.display().to_string(),
                "Symbolic link (not followed)",
            );
            continue;
        }

        if meta.is_file() {
            if canon == output || canon == temp {
                continue;
            }
            rep.p.total_files += 1;
            rep.p.total_bytes += meta.len();
            items.push(file_item(&canon, root, &meta));
            continue;
        }

        // Directory
        items.push(Item::Dir {
            name: format!("{root}/"),
        });
        let mut wb = WalkBuilder::new(&canon);
        wb.hidden(!opts.include_hidden)
            .git_ignore(opts.respect_gitignore)
            .git_exclude(opts.respect_gitignore)
            .ignore(opts.respect_gitignore)
            .git_global(false)
            .parents(false)
            .require_git(false)
            .follow_links(false)
            .sort_by_file_name(|a, b| a.cmp(b));
        if opts.skip_common_junk {
            let mut ob = OverrideBuilder::new(&canon);
            for pat in DEFAULT_EXCLUDES {
                ob.add(&format!("!{pat}"))
                    .map_err(|e| Error::Invalid(e.to_string()))?;
            }
            wb.overrides(ob.build().map_err(|e| Error::Invalid(e.to_string()))?);
        }

        for entry in wb.build() {
            cancel.check()?;
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    push_skipped(
                        &mut skipped,
                        &mut skipped_count,
                        "(unreadable)",
                        e.to_string(),
                    );
                    continue;
                }
            };
            if entry.depth() == 0 {
                continue;
            }
            let path = entry.path();
            if path == output || path == temp {
                continue;
            }
            let rel = match path.strip_prefix(&canon) {
                Ok(r) => r,
                Err(_) => continue,
            };
            let name = format!("{root}/{}", to_zip_name(rel));
            let ft = match entry.file_type() {
                Some(ft) => ft,
                None => continue,
            };
            if ft.is_symlink() {
                push_skipped(
                    &mut skipped,
                    &mut skipped_count,
                    name,
                    "Symbolic link (not followed)",
                );
            } else if ft.is_dir() {
                items.push(Item::Dir {
                    name: format!("{name}/"),
                });
            } else if ft.is_file() {
                match entry.metadata() {
                    Ok(m) => {
                        rep.p.total_files += 1;
                        rep.p.total_bytes += m.len();
                        rep.p.current = name.clone();
                        items.push(file_item(path, name, &m));
                        rep.emit(false);
                    }
                    Err(e) => push_skipped(&mut skipped, &mut skipped_count, name, e.to_string()),
                }
            } else {
                push_skipped(
                    &mut skipped,
                    &mut skipped_count,
                    name,
                    "Special file (device, socket or pipe)",
                );
            }
        }
    }

    if rep.p.total_files == 0 && !items.iter().any(|i| matches!(i, Item::Dir { .. })) {
        return Err(Error::NothingToCompress);
    }

    // ---------- Pass 2: write ----------
    rep.phase(Phase::Compressing);
    let result = write_archive(
        &items,
        &temp,
        opts,
        cancel,
        &mut rep,
        &mut skipped,
        &mut skipped_count,
    );
    let (files, folders) = match result {
        Ok(v) => v,
        Err(e) => {
            let _ = fs::remove_file(&temp);
            return Err(e);
        }
    };

    rep.phase(Phase::Finalizing);
    if let Err(e) = fs::rename(&temp, &output) {
        let _ = fs::remove_file(&temp);
        return Err(Error::Io {
            path: output,
            source: e,
        });
    }
    let archive_bytes = fs::metadata(&output).map(|m| m.len()).unwrap_or(0);

    Ok(CompressSummary {
        output,
        files,
        folders,
        original_bytes: rep.p.total_bytes,
        archive_bytes,
        encrypted: opts.password.is_some(),
        skipped_count,
        skipped,
    })
}

fn write_archive(
    items: &[Item],
    temp: &Path,
    opts: &CompressOptions,
    cancel: &CancelToken,
    rep: &mut Reporter<'_>,
    skipped: &mut Vec<Skipped>,
    skipped_count: &mut u64,
) -> Result<(u64, u64)> {
    let file = File::create(temp).map_err(io_err(temp))?;
    let mut zip = ZipWriter::new(BufWriter::with_capacity(1 << 20, file));
    let offset = crate::local_offset();
    let mut buf = vec![0u8; 256 * 1024];
    let (mut files, mut folders) = (0u64, 0u64);

    for item in items {
        cancel.check()?;
        match item {
            Item::Dir { name } => {
                let mut o = SimpleFileOptions::default();
                #[cfg(unix)]
                {
                    o = o.unix_permissions(0o755);
                }
                zip.add_directory(name.as_str(), o)?;
                folders += 1;
            }
            Item::File {
                path,
                name,
                size,
                mode,
                modified,
            } => {
                rep.p.current = name.clone();
                rep.emit(false);

                // Open before starting the entry so an unreadable file is just skipped.
                let mut src = match File::open(path) {
                    Ok(f) => f,
                    Err(e) => {
                        rep.p.processed_bytes += size;
                        push_skipped(skipped, skipped_count, name.clone(), e.to_string());
                        continue;
                    }
                };

                let stored = opts.level == CompressionLevel::Store || is_precompressed(path);
                let mut o = SimpleFileOptions::default()
                    .compression_method(if stored {
                        CompressionMethod::Stored
                    } else {
                        CompressionMethod::Deflated
                    })
                    .compression_level(if stored {
                        None
                    } else {
                        opts.level.deflate_level()
                    })
                    .large_file(*size >= 0xFFFF_0000);
                if let Some(dt) = modified.and_then(|m| to_zip_time(m, offset)) {
                    o = o.last_modified_time(dt);
                }
                #[cfg(unix)]
                {
                    o = o.unix_permissions(mode & 0o777);
                }
                #[cfg(not(unix))]
                let _ = mode;
                if let Some(pw) = &opts.password {
                    o = o.with_aes_encryption(AesMode::Aes256, pw.as_str());
                }

                zip.start_file(name.as_str(), o)?;
                loop {
                    cancel.check()?;
                    let n = src.read(&mut buf).map_err(io_err(path))?;
                    if n == 0 {
                        break;
                    }
                    zip.write_all(&buf[..n]).map_err(io_err(temp))?;
                    rep.p.processed_bytes += n as u64;
                    rep.emit(false);
                }
                files += 1;
                rep.p.processed_files += 1;
            }
        }
    }

    let mut inner = zip.finish()?;
    inner.flush().map_err(io_err(temp))?;
    let file = inner.into_inner().map_err(|e| Error::Io {
        path: temp.to_path_buf(),
        source: e.into_error(),
    })?;
    file.sync_all().map_err(io_err(temp))?;
    rep.emit(true);
    Ok((files, folders))
}

fn file_item(path: &Path, name: String, meta: &fs::Metadata) -> Item {
    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::PermissionsExt;
        meta.permissions().mode()
    };
    #[cfg(not(unix))]
    let mode = 0o644;
    Item::File {
        path: path.to_path_buf(),
        name,
        size: meta.len(),
        mode,
        modified: meta.modified().ok(),
    }
}

fn dedupe_root(used: &mut HashSet<String>, base: &str) -> String {
    if used.insert(base.to_string()) {
        return base.to_string();
    }
    for n in 2.. {
        let candidate = format!("{base} ({n})");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
}

/// Convert a relative OS path into a zip entry name using `/` separators.
fn to_zip_name(rel: &Path) -> String {
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn is_precompressed(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| PRECOMPRESSED.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn to_zip_time(t: std::time::SystemTime, offset: time::UtcOffset) -> Option<zip::DateTime> {
    let odt = time::OffsetDateTime::from(t).to_offset(offset);
    let pdt = time::PrimitiveDateTime::new(odt.date(), odt.time());
    zip::DateTime::try_from(pdt).ok()
}
