import { useEffect, useState, type ReactNode } from "react";
import type { Progress, Skipped } from "../lib/api";
import { formatBytes, formatDuration } from "../lib/format";
import { IconAlert, IconCheck } from "./Icons";

const phaseLabel: Record<string, string> = {
  scanning: "Scanning files…",
  compressing: "Compressing…",
  extracting: "Extracting…",
  finalizing: "Finishing up…",
};

export function ProgressPanel({
  title,
  progress,
  startedAt,
  onCancel,
}: {
  title: string;
  progress: Progress | null;
  startedAt: number;
  onCancel: () => void;
}) {
  const [now, setNow] = useState(Date.now());
  const [cancelling, setCancelling] = useState(false);
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 500);
    return () => clearInterval(t);
  }, []);

  const total = progress?.totalBytes ?? 0;
  const done = progress?.processedBytes ?? 0;
  const scanning = !progress || progress.phase === "scanning";
  const pct = total > 0 ? Math.min(100, (done / total) * 100) : scanning ? 0 : 100;
  const elapsed = now - startedAt;
  const rate = elapsed > 800 ? done / (elapsed / 1000) : 0;
  const eta = rate > 0 && total > done ? ((total - done) / rate) * 1000 : 0;

  return (
    <div className="panel progress-panel" aria-live="polite">
      <div className="progress-head">
        <div>
          <h2>{title}</h2>
          <p className="muted">{progress ? phaseLabel[progress.phase] : "Starting…"}</p>
        </div>
        <div className="pct">{scanning ? "" : `${Math.floor(pct)}%`}</div>
      </div>

      <div className={`bar ${scanning ? "indeterminate" : ""}`}>
        <div className="fill" style={{ width: scanning ? undefined : `${pct}%` }} />
      </div>

      <div className="progress-stats">
        <span>
          {progress && progress.totalFiles > 0
            ? `${progress.processedFiles.toLocaleString()} / ${progress.totalFiles.toLocaleString()} files`
            : progress
              ? `${progress.totalFiles.toLocaleString()} files found`
              : ""}
        </span>
        <span>{total > 0 ? `${formatBytes(done)} of ${formatBytes(total)}` : ""}</span>
        <span>{rate > 0 ? `${formatBytes(rate)}/s` : ""}</span>
        <span>{eta > 1500 ? `about ${formatDuration(eta)} left` : ""}</span>
      </div>
      <div className="current" title={progress?.current}>
        <bdi>{progress?.current || "\u00a0"}</bdi>
      </div>

      <div className="actions">
        <button
          className="btn"
          disabled={cancelling}
          onClick={() => {
            setCancelling(true);
            onCancel();
          }}
        >
          {cancelling ? "Cancelling…" : "Cancel"}
        </button>
      </div>
    </div>
  );
}

export function ResultPanel({
  title,
  subtitle,
  stats,
  skipped,
  skippedCount,
  actions,
}: {
  title: string;
  subtitle: ReactNode;
  stats: { label: string; value: string }[];
  skipped: Skipped[];
  skippedCount: number;
  actions: ReactNode;
}) {
  return (
    <div className="panel result-panel">
      <div className="result-icon success">
        <IconCheck size={28} />
      </div>
      <h2>{title}</h2>
      <p className="muted path-line">{subtitle}</p>
      <div className="stats">
        {stats.map((s) => (
          <div key={s.label} className="stat">
            <div className="stat-value">{s.value}</div>
            <div className="stat-label">{s.label}</div>
          </div>
        ))}
      </div>
      {skippedCount > 0 && (
        <details className="skipped">
          <summary>
            <IconAlert size={15} /> {skippedCount.toLocaleString()} item{skippedCount === 1 ? "" : "s"} skipped
          </summary>
          <ul>
            {skipped.map((s, i) => (
              <li key={i}>
                <code>{s.path}</code> <span className="muted">{s.reason}</span>
              </li>
            ))}
            {skippedCount > skipped.length && <li className="muted">…and {skippedCount - skipped.length} more</li>}
          </ul>
        </details>
      )}
      <div className="actions center">{actions}</div>
    </div>
  );
}

export function ErrorPanel({
  title,
  message,
  cancelled,
  actions,
}: {
  title: string;
  message: string;
  cancelled?: boolean;
  actions: ReactNode;
}) {
  return (
    <div className="panel result-panel">
      <div className={`result-icon ${cancelled ? "neutral" : "error"}`}>
        <IconAlert size={28} />
      </div>
      <h2>{title}</h2>
      <p className="muted">{message}</p>
      <div className="actions center">{actions}</div>
    </div>
  );
}

export function elapsedText(start: number, end: number) {
  return formatDuration(end - start);
}
