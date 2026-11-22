use std::fs;
use std::io::Write;
use std::path::Path;
use zipnest_core::*;

fn noop() -> impl FnMut(&Progress) {
    |_p: &Progress| {}
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn make_project(root: &Path) -> std::path::PathBuf {
    let p = root.join("my-app");
    write(&p.join("src/main.rs"), "fn main() { println!(\"hi\"); }\n");
    write(&p.join("src/lib/util.rs"), &"pub fn x() {}\n".repeat(500));
    write(&p.join("README.md"), "# My app\n");
    write(&p.join(".env.example"), "KEY=value\n");
    write(&p.join(".gitignore"), "build/\n*.log\n");
    write(
        &p.join("build/output.bin"),
        "should be ignored by gitignore",
    );
    write(&p.join("debug.log"), "ignored log");
    write(&p.join("node_modules/pkg/index.js"), "junk");
    write(&p.join(".git/HEAD"), "ref: refs/heads/main");
    fs::create_dir_all(p.join("empty-dir")).unwrap();
    p
}

#[test]
fn roundtrip_codebase_with_filters() {
    let tmp = tempfile::tempdir().unwrap();
    let project = make_project(tmp.path());
    let out = tmp.path().join("my-app.zip");

    let mut events = 0;
    let mut cb = |_p: &Progress| events += 1;
    let s = compress(
        std::slice::from_ref(&project),
        &out,
        &CompressOptions::default(),
        &CancelToken::new(),
        &mut cb,
    )
    .unwrap();
    assert!(events > 0);
    assert_eq!(
        s.files, 5,
        "main.rs, util.rs, README, .env.example, .gitignore"
    );
    assert!(out.exists());
    assert!(!tmp.path().join(".my-app.zip.zipnest-partial").exists());

    let info = inspect(&out).unwrap();
    assert_eq!(info.single_root.as_deref(), Some("my-app"));
    assert!(!info.encrypted);
    let names: Vec<_> = info.preview.iter().map(|e| e.name.clone()).collect();
    assert!(names.contains(&"my-app/empty-dir".to_string()));
    assert!(!names.iter().any(|n| n.contains("node_modules")
        || n.contains(".git/")
        || n.contains("build")
        || n.ends_with(".log")));

    let dest = tmp.path().join("out");
    let r = extract(
        &out,
        &dest,
        &ExtractOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    assert_eq!(
        r.output_dir,
        fs::canonicalize(dest.join("my-app")).unwrap(),
        "no double nesting"
    );
    assert_eq!(
        fs::read_to_string(dest.join("my-app/src/lib/util.rs")).unwrap(),
        "pub fn x() {}\n".repeat(500)
    );
    assert!(dest.join("my-app/empty-dir").is_dir());
    assert!(dest.join("my-app/.env.example").exists());

    // Second extraction never overwrites: goes to "my-app (2)".
    let r2 = extract(
        &out,
        &dest,
        &ExtractOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    assert!(r2.output_dir.ends_with("my-app (2)"));
}

#[test]
fn filters_can_be_disabled() {
    let tmp = tempfile::tempdir().unwrap();
    let project = make_project(tmp.path());
    let out = tmp.path().join("all.zip");
    let opts = CompressOptions {
        respect_gitignore: false,
        skip_common_junk: false,
        include_hidden: true,
        level: CompressionLevel::Store,
        password: None,
    };
    let s = compress(&[project], &out, &opts, &CancelToken::new(), &mut noop()).unwrap();
    assert_eq!(s.files, 9);
}

#[test]
fn multiple_inputs_and_duplicate_names() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join("a/notes.txt"), "one");
    write(&tmp.path().join("b/notes.txt"), "two");
    let out = tmp.path().join("multi.zip");
    compress(
        &[
            tmp.path().join("a/notes.txt"),
            tmp.path().join("b/notes.txt"),
        ],
        &out,
        &CompressOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    let info = inspect(&out).unwrap();
    assert_eq!(
        info.top_level,
        vec!["notes.txt".to_string(), "notes.txt (2)".to_string()]
    );
    assert!(info.single_root.is_none());
    let r = extract(
        &out,
        &tmp.path().join("x"),
        &ExtractOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    assert!(r.output_dir.ends_with("multi"));
    assert_eq!(
        fs::read_to_string(r.output_dir.join("notes.txt (2)")).unwrap(),
        "two"
    );
}

#[test]
fn output_inside_input_is_not_included() {
    let tmp = tempfile::tempdir().unwrap();
    let project = make_project(tmp.path());
    let out = project.join("self.zip");
    compress(
        std::slice::from_ref(&project),
        &out,
        &CompressOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    // Zip again: the previous self.zip is now an input file, but the output itself must not be.
    compress(
        std::slice::from_ref(&project),
        &out,
        &CompressOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    let info = inspect(&out).unwrap();
    assert!(!info.preview.iter().any(|e| e.name.ends_with("self.zip")));
}

#[test]
fn password_protection() {
    let tmp = tempfile::tempdir().unwrap();
    let project = make_project(tmp.path());
    let out = tmp.path().join("secret.zip");
    let opts = CompressOptions {
        password: Some("correct horse".into()),
        ..Default::default()
    };
    let s = compress(&[project], &out, &opts, &CancelToken::new(), &mut noop()).unwrap();
    assert!(s.encrypted);
    assert!(inspect(&out).unwrap().encrypted);

    let dest = tmp.path().join("d");
    let e = extract(
        &out,
        &dest,
        &ExtractOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap_err();
    assert!(matches!(e, Error::PasswordRequired), "{e:?}");

    let bad = ExtractOptions {
        password: Some("wrong".into()),
        ..Default::default()
    };
    let e = extract(&out, &dest, &bad, &CancelToken::new(), &mut noop()).unwrap_err();
    assert!(matches!(e, Error::WrongPassword), "{e:?}");
    assert!(
        !dest.join("my-app").exists(),
        "failed extraction is cleaned up"
    );

    let good = ExtractOptions {
        password: Some("correct horse".into()),
        ..Default::default()
    };
    let r = extract(&out, &dest, &good, &CancelToken::new(), &mut noop()).unwrap();
    assert_eq!(
        fs::read_to_string(r.output_dir.join("README.md")).unwrap(),
        "# My app\n"
    );
}

#[test]
fn blocks_zip_slip_and_symlinks() {
    let tmp = tempfile::tempdir().unwrap();
    let evil = tmp.path().join("evil.zip");
    {
        let f = fs::File::create(&evil).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("../../escaped.txt", o).unwrap();
        z.write_all(b"pwned").unwrap();
        z.start_file("/abs.txt", o).unwrap();
        z.write_all(b"pwned").unwrap();
        z.add_symlink("link", "/etc/passwd", o).unwrap();
        z.start_file("ok.txt", o).unwrap();
        z.write_all(b"fine").unwrap();
        z.finish().unwrap();
    }
    let info = inspect(&evil).unwrap();
    assert_eq!(info.symlinks, 1);
    assert!(!info.warnings.is_empty());

    let dest = tmp.path().join("safe");
    let r = extract(
        &evil,
        &dest,
        &ExtractOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    assert!(!tmp.path().join("escaped.txt").exists());
    assert!(!dest.join("escaped.txt").exists());
    assert!(!r.output_dir.join("link").exists());
    assert_eq!(
        fs::read_to_string(r.output_dir.join("ok.txt")).unwrap(),
        "fine"
    );
    assert!(r.skipped_count >= 2);
}

#[test]
fn cancel_cleans_up() {
    let tmp = tempfile::tempdir().unwrap();
    let project = make_project(tmp.path());
    let out = tmp.path().join("c.zip");
    let token = CancelToken::new();
    token.cancel();
    let e = compress(
        std::slice::from_ref(&project),
        &out,
        &CompressOptions::default(),
        &token,
        &mut noop(),
    )
    .unwrap_err();
    assert!(matches!(e, Error::Cancelled));
    assert!(!out.exists());
    assert!(!tmp.path().join(".c.zip.zipnest-partial").exists());

    compress(
        &[project],
        &out,
        &CompressOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    let dest = tmp.path().join("cx");
    let e = extract(&out, &dest, &ExtractOptions::default(), &token, &mut noop()).unwrap_err();
    assert!(matches!(e, Error::Cancelled));
    assert!(!dest.join("my-app").exists());
}

#[test]
fn rejects_non_zip() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("fake.zip");
    fs::write(&p, "definitely not a zip").unwrap();
    assert!(matches!(inspect(&p).unwrap_err(), Error::Invalid(_)));
    assert!(matches!(
        inspect(&tmp.path().join("missing.zip")).unwrap_err(),
        Error::NotFound(_)
    ));
}

#[cfg(unix)]
#[test]
fn preserves_executable_bit() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("tool/run.sh");
    write(&script, "#!/bin/sh\necho hi\n");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    let out = tmp.path().join("tool.zip");
    compress(
        &[tmp.path().join("tool")],
        &out,
        &CompressOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    let r = extract(
        &out,
        &tmp.path().join("o"),
        &ExtractOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    let mode = fs::metadata(r.output_dir.join("run.sh"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o755);
}

#[test]
fn extracts_zstd_archives() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("zstd.zip");
    {
        let f = fs::File::create(&p).unwrap();
        let mut z = zip::ZipWriter::new(f);
        let o = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Zstd);
        z.start_file("data/hello.txt", o).unwrap();
        z.write_all("hello zstd ".repeat(1000).as_bytes()).unwrap();
        z.finish().unwrap();
    }
    let r = extract(
        &p,
        &tmp.path().join("o"),
        &ExtractOptions::default(),
        &CancelToken::new(),
        &mut noop(),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(r.output_dir.join("hello.txt")).unwrap(),
        "hello zstd ".repeat(1000)
    );
}
