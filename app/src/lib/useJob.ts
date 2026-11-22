import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, newJobId, PROGRESS_EVENT, toCmdError, type CmdError, type Progress } from "./api";

export type JobStatus = "idle" | "running" | "done" | "error" | "cancelled";

export interface JobState<T> {
  status: JobStatus;
  id: string | null;
  progress: Progress | null;
  result: T | null;
  error: CmdError | null;
  startedAt: number;
  finishedAt: number;
}

const idle = <T,>(): JobState<T> => ({
  status: "idle",
  id: null,
  progress: null,
  result: null,
  error: null,
  startedAt: 0,
  finishedAt: 0,
});

/** Runs one long operation at a time and tracks its live progress. */
export function useJob<T>() {
  const [job, setJob] = useState<JobState<T>>(idle);
  const currentId = useRef<string | null>(null);

  useEffect(() => {
    const unlisten = listen<Progress>(PROGRESS_EVENT, (e) => {
      if (e.payload.jobId !== currentId.current) return;
      setJob((j) => (j.status === "running" ? { ...j, progress: e.payload } : j));
    });
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  const run = useCallback(async (fn: (jobId: string) => Promise<T>) => {
    const id = newJobId();
    currentId.current = id;
    setJob({ ...idle<T>(), status: "running", id, startedAt: Date.now() });
    try {
      const result = await fn(id);
      setJob((j) => ({ ...j, status: "done", result, finishedAt: Date.now() }));
    } catch (e) {
      const err = toCmdError(e);
      setJob((j) => ({
        ...j,
        status: err.code === "cancelled" ? "cancelled" : "error",
        error: err,
        finishedAt: Date.now(),
      }));
    }
  }, []);

  const cancel = useCallback(() => {
    if (currentId.current) void api.cancel(currentId.current);
  }, []);

  const reset = useCallback(() => {
    currentId.current = null;
    setJob(idle<T>());
  }, []);

  return { job, run, cancel, reset };
}
