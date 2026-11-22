import { invoke } from "@tauri-apps/api/core";

export type CompressionLevel = "store" | "fast" | "balanced" | "maximum";

export interface CompressOptions {
  level: CompressionLevel;
  respectGitignore: boolean;
  skipCommonJunk: boolean;
  includeHidden: boolean;
  password: string | null;
}

export interface ExtractOptions {
  password: string | null;
  intoNewFolder: boolean;
  overwrite: boolean;
}

export type Phase = "scanning" | "compressing" | "extracting" | "finalizing";

export interface Progress {
  jobId: string;
  phase: Phase;
  processedBytes: number;
  totalBytes: number;
  processedFiles: number;
  totalFiles: number;
  current: string;
}

export interface Skipped {
  path: string;
  reason: string;
}

export interface CompressSummary {
  output: string;
  files: number;
  folders: number;
  originalBytes: number;
  archiveBytes: number;
  encrypted: boolean;
  skippedCount: number;
  skipped: Skipped[];
}

export interface ExtractSummary {
  outputDir: string;
  files: number;
  folders: number;
  bytes: number;
  skippedCount: number;
  skipped: Skipped[];
}

export interface EntryPreview {
  name: string;
  size: number;
  isDir: boolean;
}

export interface ArchiveInfo {
  path: string;
  entries: number;
  files: number;
  folders: number;
  totalBytes: number;
  archiveBytes: number;
  encrypted: boolean;
  unsafeEntries: number;
  symlinks: number;
  singleRoot: string | null;
  topLevel: string[];
  preview: EntryPreview[];
  warnings: string[];
}

export interface PathInfo {
  path: string;
  name: string;
  isDir: boolean;
  size: number | null;
  exists: boolean;
}

export interface CmdError {
  code: string;
  message: string;
}

export const PROGRESS_EVENT = "zipnest://progress";
export const OPEN_FILES_EVENT = "zipnest://open-files";

export const api = {
  compress: (jobId: string, inputs: string[], output: string, options: CompressOptions) =>
    invoke<CompressSummary>("compress", { jobId, inputs, output, options }),
  extract: (jobId: string, archive: string, destination: string, options: ExtractOptions) =>
    invoke<ExtractSummary>("extract", { jobId, archive, destination, options }),
  inspect: (archive: string) => invoke<ArchiveInfo>("inspect", { archive }),
  cancel: (jobId: string) => invoke<boolean>("cancel", { jobId }),
  describePaths: (paths: string[]) => invoke<PathInfo[]>("describe_paths", { paths }),
  defaultExcludes: () => invoke<string[]>("default_excludes"),
  takePendingOpen: () => invoke<string[]>("take_pending_open"),
};

export function toCmdError(e: unknown): CmdError {
  if (e && typeof e === "object" && "message" in e) {
    const o = e as { code?: unknown; message?: unknown };
    return { code: String(o.code ?? "unknown"), message: String(o.message) };
  }
  return { code: "unknown", message: String(e) };
}

export function newJobId(): string {
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
}
