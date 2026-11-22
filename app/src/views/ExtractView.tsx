import { useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, toCmdError, type ArchiveInfo, type ExtractSummary } from "../lib/api";
import { useJob } from "../lib/useJob";
import { basename, dirname, formatBytes, plural, revealLabel, type Platform } from "../lib/format";
import { DropZone } from "../components/DropZone";
import { PasswordInput, Toggle } from "../components/Controls";
import { ErrorPanel, ProgressPanel, ResultPanel, elapsedText } from "../components/JobPanels";
import { IconAlert, IconExtract, IconFile, IconFolder, IconLock, IconShield, IconZip } from "../components/Icons";

export function ExtractView({
  archivePath,
  setArchivePath,
  dragOver,
  platform,
}: {
  archivePath: string | null;
  setArchivePath: (p: string | null) => void;
  dragOver: boolean;
  platform: Platform;
}) {
  const [info, setInfo] = useState<ArchiveInfo | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [destination, setDestination] = useState("");
  const [intoNewFolder, setIntoNewFolder] = useState(true);
  const [overwrite, setOverwrite] = useState(false);
  const [password, setPassword] = useState("");
  const { job, run, cancel, reset } = useJob<ExtractSummary>();

  useEffect(() => {
    setInfo(null);
    setLoadError(null);
    setPassword("");
    reset();
    if (!archivePath) return;
    setDestination(dirname(archivePath));
    setLoading(true);
    api
      .inspect(archivePath)
      .then(setInfo)
      .catch((e) => setLoadError(toCmdError(e).message))
      .finally(() => setLoading(false));
  }, [archivePath, reset]);

  const pickArchive = async () => {
    const r = await open({
      multiple: false,
      directory: false,
      title: "Choose a ZIP archive",
      filters: [{ name: "ZIP archive", extensions: ["zip"] }],
    });
    if (typeof r === "string") setArchivePath(r);
  };

  const pickDestination = async () => {
    const r = await open({ multiple: false, directory: true, title: "Extract to…", defaultPath: destination });
    if (typeof r === "string") setDestination(r);
  };

  const start = () =>
    archivePath &&
    run((id) =>
      api.extract(id, archivePath, destination, {
        password: info?.encrypted ? password : null,
        intoNewFolder,
        overwrite: !intoNewFolder && overwrite,
      }),
    );

  // ----- Job states -----
  if (job.status === "running") {
    return <ProgressPanel title="Extracting" progress={job.progress} startedAt={job.startedAt} onCancel={cancel} />;
  }
  if (job.status === "done" && job.result) {
    const r = job.result;
    return (
      <ResultPanel
        title="Extraction complete"
        subtitle={r.outputDir}
        stats={[
          { label: "Files", value: r.files.toLocaleString() },
          { label: "Folders", value: r.folders.toLocaleString() },
          { label: "Size", value: formatBytes(r.bytes) },
          { label: "Time", value: elapsedText(job.startedAt, job.finishedAt) },
        ]}
        skipped={r.skipped}
        skippedCount={r.skippedCount}
        actions={
          <>
            <button
              className="btn"
              onClick={() => {
                reset();
                setArchivePath(null);
              }}
            >
              Extract another
            </button>
            <button className="btn primary" onClick={() => revealItemInDir(r.outputDir)}>
              {revealLabel(platform)}
            </button>
          </>
        }
      />
    );
  }
  if (job.status === "error" || job.status === "cancelled") {
    const wrongPw = job.error?.code === "wrong_password" || job.error?.code === "password_required";
    return (
      <ErrorPanel
        title={job.status === "cancelled" ? "Cancelled" : wrongPw ? "Wrong password" : "Couldn't extract"}
        message={
          job.status === "cancelled"
            ? "Partially extracted files were removed."
            : wrongPw
              ? "That password didn't unlock the archive. Check it and try again."
              : (job.error?.message ?? "Unknown error")
        }
        cancelled={job.status === "cancelled"}
        actions={
          <button className="btn primary" onClick={reset}>
            {wrongPw ? "Try again" : "Back"}
          </button>
        }
      />
    );
  }

  // ----- Empty / loading / error -----
  if (!archivePath) {
    return (
      <DropZone
        active={dragOver}
        icon={<IconExtract size={40} />}
        title="Drop a ZIP file to extract"
        subtitle="ZipNest checks every archive for unsafe paths and zip bombs before it touches your disk."
      >
        <button className="btn primary" onClick={pickArchive}>
          <IconZip size={16} /> Choose ZIP file
        </button>
      </DropZone>
    );
  }
  if (loading) {
    return (
      <div className="panel center-panel">
        <div className="spinner" />
        <p className="muted">Reading {basename(archivePath)}…</p>
      </div>
    );
  }
  if (loadError || !info) {
    return (
      <ErrorPanel
        title="Can't open this file"
        message={loadError ?? "Unknown error"}
        actions={
          <button className="btn primary" onClick={() => setArchivePath(null)}>
            Choose another file
          </button>
        }
      />
    );
  }

  const ratio = info.archiveBytes > 0 ? info.totalBytes / info.archiveBytes : 1;
  const canStart = !!destination && (!info.encrypted || password.length > 0);

  return (
    <div className={`compose ${dragOver ? "drag-hint" : ""}`}>
      <section className="panel">
        <div className="archive-head">
          <span className="item-icon big">
            <IconZip size={26} />
          </span>
          <div className="item-text">
            <div className="archive-name">{basename(info.path)}</div>
            <div className="muted small">
              {formatBytes(info.archiveBytes)} · {plural(info.files, "file")}, {plural(info.folders, "folder")} ·
              expands to {formatBytes(info.totalBytes)}
              {ratio > 1.05 && ` (${ratio.toFixed(1)}×)`}
            </div>
          </div>
          <button className="btn small" onClick={() => setArchivePath(null)}>
            Change
          </button>
        </div>

        <div className="badges">
          {info.encrypted && (
            <span className="badge warn">
              <IconLock size={13} /> Password protected
            </span>
          )}
          {info.warnings.length === 0 ? (
            <span className="badge ok">
              <IconShield size={13} /> Safety check passed
            </span>
          ) : (
            <span className="badge danger">
              <IconAlert size={13} /> {plural(info.warnings.length, "warning")}
            </span>
          )}
        </div>

        {info.warnings.length > 0 && (
          <ul className="warnings">
            {info.warnings.map((w) => (
              <li key={w}>
                <IconAlert size={14} /> {w}
              </li>
            ))}
          </ul>
        )}

        <h3>Contents</h3>
        <ul className="tree">
          {info.preview.map((e) => {
            const depth = e.name.split("/").length - 1;
            return (
              <li key={e.name} style={{ paddingLeft: 10 + Math.min(depth, 8) * 16 }}>
                <span className={`item-icon tiny ${e.isDir ? "dir" : ""}`}>
                  {e.isDir ? <IconFolder size={14} /> : <IconFile size={14} />}
                </span>
                <span className="tree-name">{basename(e.name)}</span>
                {!e.isDir && <span className="tree-size">{formatBytes(e.size)}</span>}
              </li>
            );
          })}
          {info.entries > info.preview.length && (
            <li className="muted more">…and {(info.entries - info.preview.length).toLocaleString()} more</li>
          )}
        </ul>
      </section>

      <section className="panel options">
        <h3>Destination</h3>
        <button className="path-picker" onClick={pickDestination} title={destination}>
          <IconFolder size={16} />
          <span className="path-text">
            <bdi>{destination}</bdi>
          </span>
          <span className="link">Change…</span>
        </button>
        <Toggle
          label="Extract into a new folder"
          hint={
            intoNewFolder
              ? `Creates “${info.singleRoot ?? basename(info.path).replace(/\.zip$/i, "")}”, never overwrites anything`
              : "Files are placed directly in the destination"
          }
          checked={intoNewFolder}
          onChange={setIntoNewFolder}
        />
        {!intoNewFolder && (
          <Toggle
            label="Replace existing files"
            hint="Off: files that already exist are kept and skipped"
            checked={overwrite}
            onChange={setOverwrite}
          />
        )}

        {info.encrypted && (
          <>
            <h3>Password</h3>
            <PasswordInput value={password} onChange={setPassword} placeholder="Archive password" autoFocus />
          </>
        )}

        <div className="options-footer">
          <span />
          <button className="btn primary large" disabled={!canStart} onClick={start}>
            <IconExtract size={17} /> Extract
          </button>
        </div>
      </section>
    </div>
  );
}
