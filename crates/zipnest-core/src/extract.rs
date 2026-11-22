use crate::{
    io_err, push_skipped, unique_path, CancelToken, Error, Phase, Progress, Reporter, Result,
    Skipped,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Component, Path, PathBuf};
use zip::ZipArchive;

/// Archives larger than this (uncompressed) are checked for a suspicious ratio.
const BOMB_CHECK_MIN_BYTES: u64 = 1 << 30; // 1 GiB

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ExtractOptions {
    pub password: Option<String>,
    /// Extract into a new folder (named after the archive, or its single top-level
    /// folder) instead of directly into the destination. Never overwrites anything.
    pub into_new_folder: bool,
    /// Only used when `into_new_folder` is false: replace existing files.
    pub overwrite: bool,
    /// Hard cap on total uncompressed size.
    pub max_total_bytes: u64,
    /// Max allowed uncompressed/compressed ratio for archives over 1 GiB.
    pub max_ratio: u64,
    /// Max number of entries.
    pub max_entries: u64,
}

impl Default for ExtractOptions {
    fn default() -> Self {
        Self {
            password: None,
            into_new_folder: true,
            overwrite: false,
            max_total_bytes: 512 * (1 << 30), // 512 GiB
            max_ratio: 1000,
            max_entries: 2_000_000,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPreview {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveInfo {
    pub path: PathBuf,
    pub entries: u64,
    pub files: u64,
    pub folders: u64,
    pub total_bytes: u64,
    pub archive_bytes: u64,
    pub encrypted: bool,
    pub unsafe_entries: u64,
    pub symlinks: u64,
    /// Present when everything lives inside one top-level folder.
    pub single_root: Option<String>,
    pub top_level: Vec<String>,
    /// First entries, for a quick look inside.
    pub preview: Vec<EntryPreview>,
    /// Warnings about suspicious content (zip-bomb heuristics etc).
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtractSummary {
    pub output_dir: PathBuf,
    pub files: u64,
    pub folders: u64,
    pub bytes: u64,
    pub skipped_count: u64,
    pub skipped: Vec<Skipped>,
}

fn open_archive(path: &Path) -> Result<ZipArchive<BufReader<File>>> {
    if !path.exists() {
        return Err(Error::NotFound(path.to_path_buf()));
    }
    let f = File::open(path).map_err(io_err(path))?;
    ZipArchive::new(BufReader::new(f)).map_err(|e| match e {
        zip::result::ZipError::InvalidArchive(_) => {
            Error::Invalid("This file is not a valid ZIP archive, or it is damaged".into())
        }
        other => Error::Zip(other),
    })
}

/// Look inside an archive without extracting it.
pub fn inspect(archive_path: &Path) -> Result<ArchiveInfo> {
    let mut archive = open_archive(archive_path)?;
    let archive_bytes = fs::metadata(archive_path).map(|m| m.len()).unwrap_or(0);
    let mut info = ArchiveInfo {
        path: archive_path.to_path_buf(),
        entries: archive.len() as u64,
        files: 0,
        folders: 0,
        total_bytes: 0,
        archive_bytes,
        encrypted: false,
        unsafe_entries: 0,
        symlinks: 0,
        single_root: None,
        top_level: Vec::new(),
        preview: Vec::new(),
        warnings: Vec::new(),
    };
    let mut tops: BTreeSet<String> = BTreeSet::new();
    let mut all_nested = true;

    for i in 0..archive.len() {
        let f = archive.by_index_raw(i)?;
        if f.encrypted() {
            info.encrypted = true;
        }
        if f.is_symlink() {
            info.symlinks += 1;
            continue;
        }
        let Some(name) = f.enclosed_name() else {
            info.unsafe_entries += 1;
            continue;
        };
        let comps: Vec<String> = name
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(s.to_string_lossy().to_string()),
                _ => None,
            })
            .collect();
        if comps.is_empty() {
            continue;
        }
        tops.insert(comps[0].clone());
        let is_dir = f.is_dir();
        if !is_dir && comps.len() == 1 {
            all_nested = false;
        }
        if is_dir {
            info.folders += 1;
        } else {
            info.files += 1;
            info.total_bytes = info.total_bytes.saturating_add(f.size());
        }
        if info.preview.len() < 300 {
            info.preview.push(EntryPreview {
                name: comps.join("/"),
                size: f.size(),
                is_dir,
            });
        }
    }

    if tops.len() == 1 && all_nested {
        info.single_root = tops.iter().next().cloned();
    }
    info.top_level = tops.into_iter().take(100).collect();
    if info.unsafe_entries > 0 {
        info.warnings.push(format!(
            "{} entr{} tr{} to write outside the destination folder and will be skipped",
            info.unsafe_entries,
            if info.unsafe_entries == 1 { "y" } else { "ies" },
            if info.unsafe_entries == 1 { "ies" } else { "y" }
        ));
    }
    if info.symlinks > 0 {
        info.warnings.push(format!(
            "{} symbolic link(s) will be skipped for safety",
            info.symlinks
        ));
    }
    if looks_like_bomb(
        info.total_bytes,
        archive_bytes,
        ExtractOptions::default().max_ratio,
    ) {
        info.warnings
            .push("Unusually high compression ratio: this may be a zip bomb".into());
    }
    Ok(info)
}

fn looks_like_bomb(total: u64, archive_bytes: u64, max_ratio: u64) -> bool {
    total > BOMB_CHECK_MIN_BYTES && total / archive_bytes.max(1) > max_ratio
}

/// Extract `archive_path` into `dest_dir`.
pub fn extract(
    archive_path: &Path,
    dest_dir: &Path,
    opts: &ExtractOptions,
    cancel: &CancelToken,
    on_progress: &mut dyn FnMut(&Progress),
) -> Result<ExtractSummary> {
    let info = inspect(archive_path)?;
    if info.entries > opts.max_entries {
        return Err(Error::Unsafe(format!(
            "the archive has {} entries (limit {})",
            info.entries, opts.max_entries
        )));
    }
    if info.total_bytes > opts.max_total_bytes {
        return Err(Error::Unsafe(format!(
            "it would expand to {} (limit {})",
            crate::format_bytes(info.total_bytes),
            crate::format_bytes(opts.max_total_bytes)
        )));
    }
    if looks_like_bomb(info.total_bytes, info.archive_bytes, opts.max_ratio) {
        return Err(Error::Unsafe(format!(
            "a {} file claims to expand to {}; this looks like a zip bomb",
            crate::format_bytes(info.archive_bytes),
            crate::format_bytes(info.total_bytes)
        )));
    }
    if info.encrypted && opts.password.as_deref().is_none_or(str::is_empty) {
        return Err(Error::PasswordRequired);
    }

    fs::create_dir_all(dest_dir).map_err(io_err(dest_dir))?;
    let dest_dir = fs::canonicalize(dest_dir).map_err(io_err(dest_dir))?;

    // Decide the target folder.
    let (target, strip_first) = if opts.into_new_folder {
        let stem = archive_path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Extracted".into());
        let name = info.single_root.clone().unwrap_or(stem);
        (
            unique_path(&dest_dir, &sanitize_component(&name)),
            info.single_root.is_some(),
        )
    } else {
        (dest_dir.clone(), false)
    };
    let created_root = !target.exists();
    fs::create_dir_all(&target).map_err(io_err(&target))?;
    let root = fs::canonicalize(&target).map_err(io_err(&target))?;

    let mut rep = Reporter::new(on_progress, Phase::Extracting);
    rep.p.total_bytes = info.total_bytes;
    rep.p.total_files = info.files;
    rep.emit(true);

    let result = extract_entries(archive_path, &root, strip_first, opts, cancel, &mut rep);
    match result {
        Ok((files, folders, skipped, skipped_count)) => {
            rep.phase(Phase::Finalizing);
            Ok(ExtractSummary {
                output_dir: root,
                files,
                folders,
                bytes: rep.p.processed_bytes,
                skipped_count,
                skipped,
            })
        }
        Err(e) => {
            if created_root {
                let _ = fs::remove_dir_all(&root);
            }
            Err(e)
        }
    }
}

type EntriesResult = (u64, u64, Vec<Skipped>, u64);

fn extract_entries(
    archive_path: &Path,
    root: &Path,
    strip_first: bool,
    opts: &ExtractOptions,
    cancel: &CancelToken,
    rep: &mut Reporter<'_>,
) -> Result<EntriesResult> {
    let mut archive = open_archive(archive_path)?;
    let mut skipped = Vec::new();
    let mut skipped_count = 0u64;
    let (mut files, mut folders) = (0u64, 0u64);
    let mut buf = vec![0u8; 256 * 1024];
    let offset = crate::local_offset();

    for i in 0..archive.len() {
        cancel.check()?;

        // Metadata pass (no decryption needed).
        let (encrypted, raw_name) = {
            let f = archive.by_index_raw(i)?;
            let raw_name = f
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|_| format!("entry #{i}"));
            if f.is_symlink() {
                push_skipped(
                    &mut skipped,
                    &mut skipped_count,
                    raw_name,
                    "Symbolic link (skipped for safety)",
                );
                continue;
            }
            (f.encrypted(), raw_name)
        };

        let mut entry = if encrypted {
            let pw = opts.password.as_deref().unwrap_or_default();
            archive
                .by_index_decrypt(i, pw.as_bytes())
                .map_err(map_pw_err)?
        } else {
            archive.by_index(i).map_err(map_pw_err)?
        };

        let Some(enclosed) = entry.enclosed_name() else {
            push_skipped(
                &mut skipped,
                &mut skipped_count,
                raw_name,
                "Path points outside the destination (blocked)",
            );
            continue;
        };
        let mut comps: Vec<String> = enclosed
            .components()
            .filter_map(|c| match c {
                Component::Normal(s) => Some(sanitize_component(&s.to_string_lossy())),
                _ => None,
            })
            .collect();
        if strip_first && !comps.is_empty() {
            comps.remove(0);
        }
        if comps.is_empty() {
            continue;
        }
        let rel: PathBuf = comps.iter().collect();
        let out = root.join(&rel);
        let display = comps.join("/");

        if entry.is_dir() {
            fs::create_dir_all(&out).map_err(io_err(&out))?;
            ensure_inside(root, &out)?;
            folders += 1;
            continue;
        }

        let parent = out.parent().unwrap_or(root);
        fs::create_dir_all(parent).map_err(io_err(parent))?;
        ensure_inside(root, parent)?;

        if let Ok(m) = fs::symlink_metadata(&out) {
            if m.file_type().is_symlink() {
                push_skipped(
                    &mut skipped,
                    &mut skipped_count,
                    display,
                    "A symbolic link already exists here",
                );
                continue;
            }
            if !opts.overwrite {
                rep.p.processed_bytes += entry.size();
                push_skipped(
                    &mut skipped,
                    &mut skipped_count,
                    display,
                    "File already exists",
                );
                continue;
            }
        }

        rep.p.current = display.clone();
        rep.emit(false);

        let declared = entry.size();
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&out)
            .map_err(io_err(&out))?;
        let mut w = BufWriter::with_capacity(1 << 20, file);
        let mut written = 0u64;
        loop {
            cancel.check()?;
            let n = entry.read(&mut buf).map_err(|e| {
                if e.to_string().to_lowercase().contains("password") {
                    Error::WrongPassword
                } else {
                    Error::Invalid(format!(
                        "Could not read \"{display}\" from the archive: {e}"
                    ))
                }
            })?;
            if n == 0 {
                break;
            }
            written += n as u64;
            if written > declared {
                return Err(Error::Unsafe(format!(
                    "\"{display}\" is larger than the archive says (possible zip bomb)"
                )));
            }
            w.write_all(&buf[..n]).map_err(io_err(&out))?;
            rep.p.processed_bytes += n as u64;
            rep.emit(false);
        }
        let file = w.into_inner().map_err(|e| Error::Io {
            path: out.clone(),
            source: e.into_error(),
        })?;

        if let Some(dt) = entry.last_modified() {
            if let Ok(pdt) = time::PrimitiveDateTime::try_from(dt) {
                let _ = file.set_modified(pdt.assume_offset(offset).into());
            }
        }
        drop(file);

        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            // Keep rwx bits but never setuid/setgid/sticky; always owner-readable/writable.
            let safe = (mode & 0o777) | 0o600;
            let _ = fs::set_permissions(&out, fs::Permissions::from_mode(safe));
        }

        files += 1;
        rep.p.processed_files += 1;
    }
    rep.emit(true);
    Ok((files, folders, skipped, skipped_count))
}

fn map_pw_err(e: zip::result::ZipError) -> Error {
    match e {
        zip::result::ZipError::InvalidPassword => Error::WrongPassword,
        zip::result::ZipError::UnsupportedArchive(msg)
            if msg == zip::result::ZipError::PASSWORD_REQUIRED =>
        {
            Error::PasswordRequired
        }
        other => Error::Zip(other),
    }
}

/// Make sure `path` (after resolving any symlinks) is still inside `root`.
fn ensure_inside(root: &Path, path: &Path) -> Result<()> {
    let canon = fs::canonicalize(path).map_err(io_err(path))?;
    if canon.starts_with(root) {
        Ok(())
    } else {
        Err(Error::Unsafe(format!(
            "\"{}\" resolves outside the destination folder",
            path.display()
        )))
    }
}

/// Replace characters that are invalid in file names on Windows, and reserved names.
fn sanitize_component(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' | '\\' => '_',
            c if (c as u32) < 32 => '_',
            c => c,
        })
        .collect();
    if cfg!(windows) {
        let trimmed = out.trim_end_matches([' ', '.']).to_string();
        out = if trimmed.is_empty() {
            "_".into()
        } else {
            trimmed
        };
        let stem = out.split('.').next().unwrap_or("").to_ascii_uppercase();
        const RESERVED: &[&str] = &[
            "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
            "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
        ];
        if RESERVED.contains(&stem.as_str()) {
            out = format!("_{out}");
        }
    }
    if out == "." || out == ".." || out.is_empty() {
        "_".into()
    } else {
        out
    }
}
