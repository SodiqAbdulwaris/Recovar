import { useState, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface DriveInfo {
  path: string;
  label: string;
  size: number;
  filesystem: string;
  removable: boolean;
}

interface AdbDevice {
  serial: string;
  model: string;
  state: string;
}

interface RecoveredFile {
  name: string | null;
  original_path: string | null;
  size: number;
  file_type: string;
  disk_offset: number;
  recovery_method: string;
  confidence: number;
  index: number;
}

interface ScanProgress {
  bytes_scanned: number;
  bytes_total: number;
  files_found: number;
  phase: string;
  complete: boolean;
  warning: string | null;
}

type Target = "laptop" | "android";
type ScanMode = "quick" | "deep" | "both";
type ScanState = "idle" | "scanning" | "complete" | "error";

export default function App() {
  const [target, setTarget] = useState<Target>("laptop");
  const [scanMode, setScanMode] = useState<ScanMode>("both");
  const [drives, setDrives] = useState<DriveInfo[]>([]);
  const [devices, setDevices] = useState<AdbDevice[]>([]);
  const [selectedDrive, setSelectedDrive] = useState("");
  const [selectedDevice, setSelectedDevice] = useState("");
  const [outputDir, setOutputDir] = useState("./recovered");

  const [scanState, setScanState] = useState<ScanState>("idle");
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [results, setResults] = useState<RecoveredFile[]>([]);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [statusMsg, setStatusMsg] = useState("Ready");
  const [error, setError] = useState<string | null>(null);

  const unlistenRef = useRef<(() => void) | null>(null);

  const loadDrives = useCallback(async () => {
    try {
      const list = await invoke<DriveInfo[]>("list_drives");
      setDrives(list);
      if (list.length > 0 && !selectedDrive) {
        setSelectedDrive(list[0].path);
      }
    } catch (e) {
      setError(`Failed to list drives: ${e}`);
    }
  }, [selectedDrive]);

  const loadDevices = useCallback(async () => {
    try {
      const list = await invoke<AdbDevice[]>("list_android_devices");
      setDevices(list);
      if (list.length > 0 && !selectedDevice) {
        setSelectedDevice(list[0].serial);
      }
    } catch (e) {
      setError(`ADB error: ${e}`);
    }
  }, [selectedDevice]);

  const handleTargetChange = (t: Target) => {
    setTarget(t);
    setResults([]);
    setSelected(new Set());
    setProgress(null);
    setScanState("idle");
    if (t === "laptop") loadDrives();
    else loadDevices();
  };

  const startScan = async () => {
    if (scanState === "scanning") {
      await invoke("stop_scan").catch(() => {});
      setScanState("idle");
      return;
    }

    setResults([]);
    setSelected(new Set());
    setScanState("scanning");
    setError(null);
    setProgress({ bytes_scanned: 0, bytes_total: 1, files_found: 0, phase: "Initializing...", complete: false, warning: null });

    const unlisten = await listen<ScanProgress>("scan-progress", (event) => {
      setProgress(event.payload);
      if (event.payload.complete) {
        setScanState("complete");
        setStatusMsg(`Scan complete — ${event.payload.files_found} file(s) found`);
      }
    });

    const unlistenFile = await listen<RecoveredFile>("file-found", (event) => {
      setResults(prev => [...prev, event.payload]);
    });

    unlistenRef.current = () => { unlisten(); unlistenFile(); };

    try {
      const targetPayload = target === "laptop"
        ? { Laptop: { drive: selectedDrive || "C:\\" } }
        : { Android: { serial: selectedDevice || null, image_path: null } };

      await invoke("start_scan", {
        target: targetPayload,
        mode: scanMode,
        outputDir,
      });
    } catch (e) {
      setScanState("error");
      setError(`Scan failed: ${e}`);
      setStatusMsg("Scan error");
    } finally {
      unlistenRef.current?.();
    }
  };

  const recoverSelected = async () => {
    if (selected.size === 0) return;
    try {
      const indices = Array.from(selected);
      await invoke("recover_files", { indices, outputDir });
      setStatusMsg(`Saved ${selected.size} file(s) to ${outputDir}`);
    } catch (e) {
      setError(`Recovery failed: ${e}`);
    }
  };

  const toggleSelect = (idx: number) => {
    setSelected(prev => {
      const next = new Set(prev);
      if (next.has(idx)) next.delete(idx);
      else next.add(idx);
      return next;
    });
  };

  const selectAll = () => {
    if (selected.size === results.length) {
      setSelected(new Set());
    } else {
      setSelected(new Set(results.map(r => r.index)));
    }
  };

  const pct = progress ? Math.round((progress.bytes_scanned / Math.max(progress.bytes_total, 1)) * 100) : 0;

  const getTypeBadgeClass = (type: string) => {
    if (["jpeg","png","gif","bmp"].includes(type)) return "image";
    if (["mp4","mov","avi","mkv"].includes(type)) return "video";
    if (["pdf","docx","zip"].includes(type)) return "doc";
    if (["mp3"].includes(type)) return "audio";
    return "unknown";
  };

  const formatSize = (bytes: number) => {
    if (!bytes) return "—";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  };

  const getConfClass = (c: number) => c >= 0.85 ? "high" : c >= 0.65 ? "med" : "low";

  const getMethodLabel = (m: string) => {
    const map: Record<string, string> = {
      NtfsMft: "NTFS MFT",
      Fat32Directory: "FAT32 Dir",
      DiskCarving: "Carving",
      AdbPull: "ADB Pull",
    };
    return map[m] ?? m;
  };

  return (
    <div className="app">
      <div className="titlebar">
        <div className="titlebar-logo">
          <div className="titlebar-logo-icon">R</div>
          <span className="titlebar-title">Recovar</span>
          <span className="titlebar-version">v0.1.0</span>
        </div>
      </div>

      <div className="main-layout">
        <aside className="sidebar">
          <div>
            <div className="sidebar-section-label">Recovery Target</div>
            <div className="target-grid">
              <button
                className={`target-btn ${target === "laptop" ? "active" : ""}`}
                onClick={() => handleTargetChange("laptop")}
              >
                <span className="target-btn-icon">💻</span>
                <span className="target-btn-label">Laptop</span>
              </button>
              <button
                className={`target-btn ${target === "android" ? "active" : ""}`}
                onClick={() => handleTargetChange("android")}
              >
                <span className="target-btn-icon">📱</span>
                <span className="target-btn-label">Android</span>
              </button>
            </div>
          </div>

          <div>
            <div className="sidebar-section-label">
              {target === "laptop" ? "Drive" : "Device"}
            </div>
            {target === "laptop" ? (
              <div className="field">
                <label className="field-label">Select drive to scan</label>
                <select
                  className="select"
                  value={selectedDrive}
                  onChange={e => setSelectedDrive(e.target.value)}
                  onClick={loadDrives}
                >
                  {drives.length === 0 && (
                    <option value="">Click to load drives...</option>
                  )}
                  {drives.map(d => (
                    <option key={d.path} value={d.path}>
                      {d.label} [{d.filesystem}]
                    </option>
                  ))}
                </select>
                <label className="field-label" style={{marginTop: 8}}>Or type drive path</label>
                <input
                  className="input"
                  placeholder="e.g. D:\\"
                  value={selectedDrive}
                  onChange={e => setSelectedDrive(e.target.value)}
                />
              </div>
            ) : (
              <div className="field">
                <label className="field-label">Android device</label>
                <select
                  className="select"
                  value={selectedDevice}
                  onChange={e => setSelectedDevice(e.target.value)}
                  onClick={loadDevices}
                >
                  {devices.length === 0 && (
                    <option value="">Click to scan for devices...</option>
                  )}
                  {devices.map(d => (
                    <option key={d.serial} value={d.serial}>
                      {d.model} ({d.serial})
                    </option>
                  ))}
                </select>
              </div>
            )}
          </div>

          <div>
            <div className="sidebar-section-label">Scan Mode</div>
            <div className="mode-grid">
              {([
                { key: "quick", label: "Quick Scan", desc: "Parse filesystem metadata" },
                { key: "deep",  label: "Deep Scan",  desc: "Raw signature carving" },
                { key: "both",  label: "Both",       desc: "Thorough — recommended" },
              ] as const).map(m => (
                <button
                  key={m.key}
                  className={`mode-btn ${scanMode === m.key ? "active" : ""}`}
                  onClick={() => setScanMode(m.key)}
                >
                  <span className="mode-btn-dot" />
                  <span>
                    <div>{m.label}</div>
                    <div className="mode-desc">{m.desc}</div>
                  </span>
                </button>
              ))}
            </div>
          </div>

          <div>
            <div className="sidebar-section-label">Output Directory</div>
            <div className="field">
              <input
                className="input"
                placeholder="./recovered"
                value={outputDir}
                onChange={e => setOutputDir(e.target.value)}
              />
            </div>
          </div>

          <div style={{ flex: 1 }} />

          <button
            className={`scan-btn ${scanState === "scanning" ? "scanning" : ""}`}
            onClick={startScan}
          >
            {scanState === "scanning" ? (
              <><span>⏹</span> Stop Scan</>
            ) : (
              <><span>▶</span> Start Scan</>
            )}
          </button>
        </aside>

        <div className="content">
          {progress && (
            <div className="progress-bar-container fade-in">
              <div className="progress-header">
                <span className="progress-phase">{progress.phase}</span>
                <div className="progress-stats">
                  <span className="progress-stat">
                    Found: <span className="progress-stat-value">{progress.files_found}</span>
                  </span>
                  <span className="progress-stat">
                    Progress: <span className="progress-stat-value">{pct}%</span>
                  </span>
                </div>
              </div>
              <div className="progress-track">
                <div
                  className={`progress-fill ${scanState === "scanning" ? "active" : ""}`}
                  style={{ width: `${pct}%` }}
                />
              </div>
              {progress.warning && (
                <div style={{ marginTop: 6, fontSize: 11, color: "var(--color-warning)" }}>
                  ⚠ {progress.warning}
                </div>
              )}
            </div>
          )}

          {results.length > 0 && (
            <div className="toolbar fade-in">
              <button className="toolbar-btn" onClick={selectAll}>
                {selected.size === results.length ? "☐ Deselect All" : "☑ Select All"}
              </button>
              <button
                className="toolbar-btn primary"
                disabled={selected.size === 0}
                onClick={recoverSelected}
              >
                ↓ Recover Selected ({selected.size})
              </button>
              <span className="toolbar-count">
                {results.length} file{results.length !== 1 ? "s" : ""} found
              </span>
            </div>
          )}

          <div className="results-area">
            {results.length === 0 ? (
              <div className="empty-state">
                <div className="empty-state-icon">
                  {scanState === "scanning" ? "🔍" : "📂"}
                </div>
                <div className="empty-state-title">
                  {scanState === "scanning" ? "Scanning..." :
                   scanState === "error" ? "Scan Failed" :
                   "No results yet"}
                </div>
                <div className="empty-state-desc">
                  {scanState === "idle" ? (
                    "Configure your target and scan mode on the left, then click Start Scan."
                  ) : scanState === "scanning" ? (
                    "Searching for recoverable files. This may take a while for deep scans."
                  ) : error ? (
                    error
                  ) : (
                    "No recoverable files were found on the selected target."
                  )}
                </div>
              </div>
            ) : (
              <table className="results-table">
                <thead>
                  <tr>
                    <th style={{width: 32}}></th>
                    <th>#</th>
                    <th>Filename</th>
                    <th>Size</th>
                    <th>Type</th>
                    <th>Method</th>
                    <th>Confidence</th>
                  </tr>
                </thead>
                <tbody>
                  {results.map(file => {
                    const isSelected = selected.has(file.index);
                    const conf = getConfClass(file.confidence);
                    const typeClass = getTypeBadgeClass(file.file_type);
                    const displayName = file.name ?? `recovered_${String(file.index).padStart(4, "0")}.${file.file_type}`;

                    return (
                      <tr
                        key={file.index}
                        className={`result-row ${isSelected ? "selected" : ""}`}
                        onClick={() => toggleSelect(file.index)}
                      >
                        <td>
                          <div className={`checkbox ${isSelected ? "checked" : ""}`}>
                            {isSelected && <span className="checkbox-check">✓</span>}
                          </div>
                        </td>
                        <td style={{ color: "var(--color-text-muted)", fontFamily: "var(--font-mono)", fontSize: 11 }}>
                          {file.index}
                        </td>
                        <td style={{ color: "var(--color-text-primary)", maxWidth: 240, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                          {displayName}
                        </td>
                        <td style={{ color: "var(--color-text-secondary)", fontFamily: "var(--font-mono)", fontSize: 11 }}>
                          {formatSize(file.size)}
                        </td>
                        <td>
                          <span className={`type-badge ${typeClass}`}>
                            {file.file_type.toUpperCase()}
                          </span>
                        </td>
                        <td style={{ color: "var(--color-text-muted)", fontSize: 11 }}>
                          {getMethodLabel(file.recovery_method)}
                        </td>
                        <td>
                          <div className="confidence">
                            <div className="confidence-bar">
                              <div
                                className={`confidence-fill ${conf}`}
                                style={{ width: `${Math.round(file.confidence * 100)}%` }}
                              />
                            </div>
                            <span className={`confidence-text ${conf}`}>
                              {Math.round(file.confidence * 100)}%
                            </span>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>

          <div className="statusbar">
            <div className={`statusbar-dot ${
              scanState === "scanning" ? "scanning" :
              scanState === "error" ? "error" :
              "ready"
            }`} />
            <span>{statusMsg}</span>
            {error && <span style={{ color: "var(--color-danger)" }}>⚠ {error}</span>}
            <span className="statusbar-spacer" />
            <span>Recovar v0.1.0</span>
          </div>
        </div>
      </div>

      {scanState === "scanning" && <div className="scan-wave" />}
    </div>
  );
}
