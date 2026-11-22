import { useEffect, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, type CompressionLevel, type CompressSummary, type PathInfo } from "../lib/api";
import { useJob } from "../lib/useJob";
import { basename, dirname, formatBytes, joinPath, plural, revealLabel, type Platform } from "../lib/format";
import { DropZone } from "../components/DropZone";
import { PasswordInput, Segmented, Toggle } from "../components/Controls";
import { ErrorPanel, ProgressPanel, ResultPanel, elapsedText } from "../components/JobPanels";
import { IconCompress, IconFile, IconFolder, IconLock, IconPlus, IconX } from "../components/Icons";

const LEVELS: { value: CompressionLevel; label: string; hint: string }[] = [
  { value: "store", label: "None", hint: "Just package the files (fastest)" },
  { value: "fast", label: "Fast", hint: "Light compression, very quick" },
  { value: "balanced", label: "Balanced", hint: "Recommended for most files" },
  { value: "maximum", label: "Maximum", hint: "Smallest file, slower" },
];

export function CompressView({
  items,
  setItems,
  addPaths,
  dragOver,
  platform,
}: {
  items: PathInfo[];
  setItems: (items: PathInfo[]) => void;
  addPaths: (paths: string[]) => Promise<void>;
  dragOver: boolean;
  platform: Platform;
}) {
  const [level, setLevel] = useState<CompressionLevel>("balanced");
  const [respectGitignore, setRespectGitignore] = useState(true);
  const [skipJunk, setSkipJunk] = useState(true);
  const [includeHidden, setIncludeHidden] = useState(true);
  const [usePassword, setUsePassword] = useState(false);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [excludes, setExcludes] = useState<string[]>([]);
  const { job, run, cancel, reset } = useJob<CompressSummary>();

  useEffect(() => {
    api.defaultExcludes().then(setExcludes).catch(() => {});
  }, []);

  const pickFiles = async () => {
    const r = await open({ multiple: true, directory: false, title: "Choose files to compress" });
    if (r) await addPaths(Array.isArray(r) ? r : [r]);
  };
  const pickFolder = async () => {
    const r = await open({ multiple: true, directory: true, title: "Choose folders to compress" });
    if (r) await addPaths(Array.isArray(r) ? r : [r]);
  };

  const passwordError =
    usePassword && password.length > 0 && password.length < 8
      ? "Use at least 8 characters"
      : usePassword && confirm.length > 0 && confirm !== password
        ? "Passwords don't match"
        : null;
  const canStart = items.length > 0 && (!usePassword || (password.length >= 8 && password === confirm));

  const start = async () => {
    const first = items[0];
    const suggested = items.length === 1 ? `${first.name.replace(/\.zip$/i, "")}.zip` : "Archive.zip";
    const target = await save({
      title: "Save ZIP archive as",
      defaultPath: joinPath(dirname(first.path), suggested),
      filters: [{ name: "ZIP archive", extensions: ["zip"] }],
    });
    if (!target) return;
    const output = /\.zip$/i.test(target) ? target : `${target}.zip`;
    await run((id) =>
      api.compress(
        id,
        items.map((i) => i.path),
        output,
        {
          level,
          respectGitignore,
          skipCommonJunk: skipJunk,
          includeHidden,
          password: usePassword ? password : null,
        },
      ),
    );
  };

  const startOver = () => {
    reset();
    setItems([]);
    setPassword("");
    setConfirm("");
  };

  // ----- Job states -----
  if (job.status === "running") {
    return (
      <ProgressPanel title="Creating your ZIP" progress={job.progress} startedAt={job.startedAt} onCancel={cancel} />
    );
  }
  if (job.status === "done" && job.result) {
    const r = job.result;
    const saved = r.originalBytes > 0 ? Math.max(0, 100 - (r.archiveBytes / r.originalBytes) * 100) : 0;
    return (
      <ResultPanel
        title={`${basename(r.output)} is ready`}
        subtitle={r.output}
        stats={[
          { label: "Files", value: r.files.toLocaleString() },
          { label: "Original size", value: formatBytes(r.originalBytes) },
          { label: "ZIP size", value: formatBytes(r.archiveBytes) },
          { label: "Space saved", value: `${saved.toFixed(0)}%` },
          { label: "Time", value: elapsedText(job.startedAt, job.finishedAt) },
        ]}
        skipped={r.skipped}
        skippedCount={r.skippedCount}
        actions={
          <>
            <button className="btn" onClick={startOver}>
              Compress something else
            </button>
            <button className="btn primary" onClick={() => revealItemInDir(r.output)}>
              {revealLabel(platform)}
            </button>
          </>
        }
      />
    );
  }
  if (job.status === "error" || job.status === "cancelled") {
    return (
      <ErrorPanel
        title={job.status === "cancelled" ? "Cancelled" : "Couldn't create the ZIP"}
        message={job.status === "cancelled" ? "Nothing was saved." : (job.error?.message ?? "Unknown error")}
        cancelled={job.status === "cancelled"}
        actions={
          <button className="btn primary" onClick={reset}>
            Back
          </button>
        }
      />
    );
  }

  // ----- Empty state -----
  if (items.length === 0) {
    return (
      <DropZone
        active={dragOver}
        icon={<IconCompress size={40} />}
        title="Drop files or folders to compress"
        subtitle="Whole project folders welcome: .gitignore and node_modules are handled for you."
      >
        <button className="btn primary" onClick={pickFolder}>
          <IconFolder size={16} /> Choose folder
        </button>
        <button className="btn" onClick={pickFiles}>
          <IconFile size={16} /> Choose files
        </button>
      </DropZone>
    );
  }

  // ----- Ready -----
  const totalKnown = items.reduce((s, i) => s + (i.size ?? 0), 0);
  const folders = items.filter((i) => i.isDir).length;
  return (
    <div className={`compose ${dragOver ? "drag-hint" : ""}`}>
      <section className="panel">
        <div className="panel-head">
          <div>
            <h2>Selected items</h2>
            <p className="muted">
              {plural(items.length, "item")}
              {folders > 0 && ` · ${plural(folders, "folder")}`}
              {totalKnown > 0 && ` · ${formatBytes(totalKnown)} in files`}
            </p>
          </div>
          <div className="row gap-s">
            <button className="btn small" onClick={pickFolder}>
              <IconPlus size={14} /> Folder
            </button>
            <button className="btn small" onClick={pickFiles}>
              <IconPlus size={14} /> Files
            </button>
          </div>
        </div>
        <ul className="item-list">
          {items.map((it) => (
            <li key={it.path}>
              <span className={`item-icon ${it.isDir ? "dir" : ""}`}>
                {it.isDir ? <IconFolder size={18} /> : <IconFile size={18} />}
              </span>
              <div className="item-text">
                <div className="item-name">{it.name}</div>
                <div className="item-path" title={it.path}>
                  <bdi>{it.path}</bdi>
                </div>
              </div>
              <span className="item-size">{it.isDir ? "Folder" : formatBytes(it.size ?? 0)}</span>
              <button
                className="icon-btn"
                aria-label={`Remove ${it.name}`}
                onClick={() => setItems(items.filter((x) => x.path !== it.path))}
              >
                <IconX size={15} />
              </button>
            </li>
          ))}
        </ul>
      </section>

      <section className="panel options">
        <h3>Compression</h3>
        <Segmented ariaLabel="Compression level" value={level} options={LEVELS} onChange={setLevel} />
        <p className="hint">{LEVELS.find((l) => l.value === level)?.hint}</p>

        <h3>Project folders</h3>
        <Toggle
          label="Respect .gitignore"
          hint="Leave out build output and anything your repo ignores"
          checked={respectGitignore}
          onChange={setRespectGitignore}
        />
        <Toggle
          label="Skip junk folders"
          hint={
            <span title={excludes.join(", ")}>
              node_modules, .git, __pycache__, .venv, .DS_Store and more
            </span>
          }
          checked={skipJunk}
          onChange={setSkipJunk}
        />
        <Toggle
          label="Include hidden files"
          hint="Dotfiles such as .env.example or .eslintrc"
          checked={includeHidden}
          onChange={setIncludeHidden}
        />

        <h3>Security</h3>
        <Toggle
          label={
            <span className="row gap-xs">
              <IconLock size={14} /> Protect with password
            </span>
          }
          hint="AES-256 encryption"
          checked={usePassword}
          onChange={setUsePassword}
        />
        {usePassword && (
          <div className="pw-fields">
            <PasswordInput value={password} onChange={setPassword} placeholder="Password" showStrength autoFocus />
            <PasswordInput
              value={confirm}
              onChange={setConfirm}
              placeholder="Confirm password"
              invalid={!!passwordError && confirm.length > 0}
            />
            {passwordError && <p className="field-error">{passwordError}</p>}
            <p className="hint">There's no way to recover a forgotten password, so store it somewhere safe.</p>
          </div>
        )}

        <div className="options-footer">
          <button className="btn ghost" onClick={() => setItems([])}>
            Clear
          </button>
          <button className="btn primary large" disabled={!canStart} onClick={start}>
            <IconCompress size={17} /> Create ZIP…
          </button>
        </div>
      </section>
    </div>
  );
}
