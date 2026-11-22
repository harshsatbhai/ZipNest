import { useCallback, useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { api, OPEN_FILES_EVENT, type PathInfo } from "./lib/api";
import { detectPlatform } from "./lib/format";
import { CompressView } from "./views/CompressView";
import { ExtractView } from "./views/ExtractView";
import { IconCompress, IconExtract, IconWifiOff, Logo } from "./components/Icons";

type View = "compress" | "extract";
const platform = detectPlatform();
const isZip = (p: string) => /\.zip$/i.test(p);

export default function App() {
  const [view, setView] = useState<View>("compress");
  const [items, setItems] = useState<PathInfo[]>([]);
  const [archivePath, setArchivePath] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [version, setVersion] = useState("");
  const viewRef = useRef(view);
  viewRef.current = view;

  const addPaths = useCallback(async (paths: string[]) => {
    const infos = (await api.describePaths(paths)).filter((i) => i.exists);
    setItems((prev) => {
      const seen = new Set(prev.map((p) => p.path));
      return [...prev, ...infos.filter((i) => !seen.has(i.path))];
    });
  }, []);

  const openArchive = useCallback((p: string) => {
    setView("extract");
    setArchivePath(p);
  }, []);

  // Drag & drop from Finder / Explorer / file manager.
  useEffect(() => {
    const un = getCurrentWebview().onDragDropEvent((e) => {
      const t = e.payload.type;
      if (t === "enter" || t === "over") setDragOver(true);
      else if (t === "leave") setDragOver(false);
      else if (t === "drop") {
        setDragOver(false);
        const paths = e.payload.paths;
        if (!paths.length) return;
        if (viewRef.current === "extract") {
          const zip = paths.find(isZip);
          if (zip) setArchivePath(zip);
        } else {
          void addPaths(paths);
        }
      }
    });
    return () => {
      un.then((f) => f());
    };
  }, [addPaths]);

  // "Open with ZipNest" / double-clicked .zip files.
  useEffect(() => {
    const drain = async () => {
      const files = await api.takePendingOpen();
      const zip = files.find(isZip);
      if (zip) openArchive(zip);
      else if (files.length) {
        setView("compress");
        await addPaths(files);
      }
    };
    void drain();
    const un = listen(OPEN_FILES_EVENT, () => void drain());
    getVersion().then(setVersion).catch(() => {});
    return () => {
      un.then((f) => f());
    };
  }, [addPaths, openArchive]);

  // Keyboard shortcuts: Cmd/Ctrl+1 and Cmd/Ctrl+2 switch views.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      if (e.key === "1") setView("compress");
      if (e.key === "2") setView("extract");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className={`app platform-${platform}`}>
      <aside className="sidebar">
        <div className="drag-strip" data-tauri-drag-region />
        <div className="brand" data-tauri-drag-region>
          <Logo size={30} />
          <span>ZipNest</span>
        </div>
        <nav>
          <button className={view === "compress" ? "active" : ""} onClick={() => setView("compress")}>
            <IconCompress /> Compress
            {items.length > 0 && <span className="count">{items.length}</span>}
          </button>
          <button className={view === "extract" ? "active" : ""} onClick={() => setView("extract")}>
            <IconExtract /> Extract
          </button>
        </nav>
        <div className="sidebar-foot">
          <div className="offline-pill">
            <IconWifiOff size={14} />
            <div>
              <strong>100% offline</strong>
              <span>Your files never leave this computer</span>
            </div>
          </div>
          {version && <div className="version">Version {version}</div>}
        </div>
      </aside>
      <main className="content">
        <div className="drag-strip" data-tauri-drag-region />
        <div className="content-inner">
          {/* Both views stay mounted so switching tabs never loses work or a running job. */}
          <div hidden={view !== "compress"} className="view">
            <CompressView
              items={items}
              setItems={setItems}
              addPaths={addPaths}
              dragOver={dragOver && view === "compress"}
              platform={platform}
            />
          </div>
          <div hidden={view !== "extract"} className="view">
            <ExtractView
              archivePath={archivePath}
              setArchivePath={setArchivePath}
              dragOver={dragOver && view === "extract"}
              platform={platform}
            />
          </div>
        </div>
      </main>
    </div>
  );
}
