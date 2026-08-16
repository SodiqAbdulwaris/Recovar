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

const HIGH_CONFIDENCE = 0.85;

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
  const [railOpen, setRailOpen] = useState(false);

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
    setRailOpen(false);
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
      const savedCount = await invoke<number>("recover_files", { indices, outputDir });
      setStatusMsg(`Saved ${savedCount} of ${selected.size} file(s) to ${outputDir}`);
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

  const selectHighConfidence = () => {
    const highConfidence = results.filter(r => r.confidence >= HIGH_CONFIDENCE).map(r => r.index);
    const allHighSelected = highConfidence.length > 0 && highConfidence.every(i => selected.has(i));
    setSelected(allHighSelected ? new Set() : new Set(highConfidence));
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

  const getConfClass = (c: number) => c >= HIGH_CONFIDENCE ? "high" : c >= 0.65 ? "mid" : "low";

  const getMethodLabel = (m: string) => {
    const map: Record<string, string> = {
      NtfsMft: "MFT record",
      Fat32Directory: "FAT32 entry",
      DiskCarving: "Sector carve",
      AdbPull: "ADB pull",
    };
    return map[m] ?? m;
  };

  const targetLabel = target === "laptop"
    ? (selectedDrive || "the selected drive")
    : (selectedDevice ? `device ${selectedDevice}` : "your phone");

  const highConfidenceResults = results.filter(r => r.confidence >= HIGH_CONFIDENCE);
  const reviewResults = results.filter(r => r.confidence < HIGH_CONFIDENCE);
  const selectedSize = results.filter(r => selected.has(r.index)).reduce((sum, r) => sum + r.size, 0);
  const allHighSelected = highConfidenceResults.length > 0 && highConfidenceResults.every(r => selected.has(r.index));

  const renderRow = (file: RecoveredFile) => {
    const isSelected = selected.has(file.index);
    const conf = getConfClass(file.confidence);
    const typeClass = getTypeBadgeClass(file.file_type);
    const displayName = file.name ?? `recovered_${String(file.index).padStart(4, "0")}.${file.file_type}`;
    const subtext = file.original_path
      ?? (file.disk_offset > 0 ? `signature match, offset 0x${file.disk_offset.toString(16).toUpperCase()}` : null);

    return (
      <div
        key={file.index}
        className={`result-row ${isSelected ? "selected" : ""}`}
        onClick={() => toggleSelect(file.index)}
        role="row"
      >
        <div className="chk">{isSelected && <span className="chk-check">✓</span>}</div>
        <div className="r-name">
          <div className="fname">{displayName}</div>
          {subtext && <div className="fpath">{subtext}</div>}
        </div>
        <div className="r-meta">
          <div className="r-size">{formatSize(file.size)}</div>
          <span className={`type-chip ${typeClass}`}>
            <span className={`sw ${typeClass}`} />
            {file.file_type.toUpperCase()}
          </span>
          <div className="r-method">{getMethodLabel(file.recovery_method)}</div>
          <div className="conf">
            <div className="conf-track"><div className={`conf-fill ${conf}`} style={{ width: `${Math.round(file.confidence * 100)}%` }} /></div>
            <span className={`conf-num ${conf}`}>{Math.round(file.confidence * 100)}%</span>
          </div>
        </div>
      </div>
    );
  };

  return (
    <div className="app">
      <div className="titlebar">
        <div className="titlebar-logo">
          <span className="wordmark">Recov<em>ar</em></span>
          <span className="titlebar-sub">SALVAGE CONSOLE</span>
        </div>
        <button className="rail-toggle" onClick={() => setRailOpen(o => !o)} aria-expanded={railOpen}>
          <span className="label">{railOpen ? "Hide" : "Scan"} setup</span>
          <span>{railOpen ? "▴" : "▾"}</span>
        </button>
        <span className="device-tag">
          <span className={`dot ${scanState === "scanning" ? "" : "idle"}`} />
          {target === "laptop"
            ? (selectedDrive || "No drive selected")
            : (selectedDevice || "No device selected")}
        </span>
      </div>

      <div className="safety-banner">
        <span className="glyph">!</span>
        <span className="msg">
          <strong>Reading only.</strong> Stop saving or installing anything to {targetLabel} — every new write can permanently erase a file still waiting to be recovered.
        </span>
      </div>

      <div className="body">
        <aside className={`rail ${railOpen ? "open" : ""}`}>
          <div>
            <div className="rail-label">Recovery Target</div>
            <div className="seg">
              <button
                className={target === "laptop" ? "active" : ""}
                onClick={() => handleTargetChange("laptop")}
              >
                This PC
              </button>
              <button
                className={target === "android" ? "active" : ""}
                onClick={() => handleTargetChange("android")}
              >
                Android
              </button>
            </div>
          </div>

          {target === "laptop" ? (
            <div className="field">
              <div className="rail-label">Drive</div>
              <select
                className="select"
                value={selectedDrive}
                onChange={e => setSelectedDrive(e.target.value)}
                onClick={() => { if (drives.length === 0) loadDrives(); }}
              >
                {drives.length === 0 && <option value="">Click to load drives…</option>}
                {drives.map(d => (
                  <option key={d.path} value={d.path}>{d.label} [{d.filesystem}]</option>
                ))}
              </select>
              <span className="field-cap">Or type a drive path</span>
              <input
                className="input"
                placeholder="e.g. D:\\"
                value={selectedDrive}
                onChange={e => setSelectedDrive(e.target.value)}
              />
            </div>
          ) : (
            <div className="field">
              <div className="rail-label">Device</div>
              <select
                className="select"
                value={selectedDevice}
                onChange={e => setSelectedDevice(e.target.value)}
                onClick={() => { if (devices.length === 0) loadDevices(); }}
              >
                {devices.length === 0 && <option value="">Click to scan for devices…</option>}
                {devices.map(d => (
                  <option key={d.serial} value={d.serial}>{d.model} ({d.serial})</option>
                ))}
              </select>
            </div>
          )}

          <div>
            <div className="rail-label">Scan depth</div>
            <div className="mode-list">
              {([
                { key: "quick", label: "Quick", desc: "Reads filesystem records. Seconds to minutes." },
                { key: "deep",  label: "Deep",  desc: "Scans every sector for file signatures. Slower, finds more." },
                { key: "both",  label: "Both",  desc: "Quick first, then deep. Recommended." },
              ] as const).map(m => (
                <button
                  key={m.key}
                  className={`mode-row ${scanMode === m.key ? "active" : ""}`}
                  onClick={() => setScanMode(m.key)}
                >
                  <span className="mode-radio" />
                  <span>
                    <div className="mode-title">{m.label}</div>
                    <div className="mode-desc">{m.desc}</div>
                  </span>
                </button>
              ))}
            </div>
          </div>

          <div className="field">
            <div className="rail-label">Save recovered files to</div>
            <input
              className="input"
              placeholder="./recovered"
              value={outputDir}
              onChange={e => setOutputDir(e.target.value)}
            />
          </div>

          <div className="rail-spacer" />

          <button className={`start-btn ${scanState === "scanning" ? "scanning" : ""}`} onClick={startScan}>
            {scanState === "scanning" ? "■ Stop scan" : "▶ Start scan"}
          </button>
        </aside>

        <section className="main">
          {progress && (
            <div className="scan-status fade-in">
              <div className="scan-status-row">
                <div className="scan-phase">{progress.phase}</div>
                <div className="scan-stats">
                  <span>Found <b>{progress.files_found}</b></span>
                  <span>Progress <b>{pct}%</b></span>
                </div>
              </div>
              <div className="depth">
                <div className="depth-fill" style={{ width: `${pct}%` }} />
                <div className="depth-ticks" />
              </div>
              <div className="depth-caption">
                <span>0 B</span>
                <span>{formatSize(progress.bytes_scanned)} / {formatSize(progress.bytes_total)} scanned</span>
              </div>
              {progress.warning && (
                <div style={{ marginTop: 6, fontSize: 11, color: "var(--color-warning)" }}>⚠ {progress.warning}</div>
              )}
            </div>
          )}

          {results.length === 0 ? (
            <div className="empty-state">
              <div className="empty-state-glyph">{scanState === "scanning" ? "…" : "⌕"}</div>
              <div className="empty-state-title">
                {scanState === "scanning" ? "Scanning…" : scanState === "error" ? "Scan failed" : "No results yet"}
              </div>
              <div className="empty-state-desc">
                {scanState === "idle" ? (
                  "Choose a target and scan depth, then start a scan. Recoverable files will be grouped by how likely they are to be intact."
                ) : scanState === "scanning" ? (
                  "Searching for recoverable files. This can take a while for a deep scan of a large drive."
                ) : error ? (
                  error
                ) : (
                  "No recoverable files were found on the selected target."
                )}
              </div>
            </div>
          ) : (
            <>
              <div className="triage">
                {highConfidenceResults.length > 0 && (
                  <div className="results-list">
                    <div className="group-head">
                      <span className="group-chip high">High confidence</span>
                      <span className="group-count">{highConfidenceResults.length} file{highConfidenceResults.length !== 1 ? "s" : ""} — recommended, data intact</span>
                      <div className="group-line" />
                    </div>
                    <div className="list-header" role="row">
                      <span></span><span>File</span><span>Size</span><span>Type</span><span>Method</span><span>Confidence</span>
                    </div>
                    {highConfidenceResults.map(renderRow)}
                  </div>
                )}

                {reviewResults.length > 0 && (
                  <div className="results-list">
                    <div className="group-head">
                      <span className="group-chip review">Needs review</span>
                      <span className="group-count">{reviewResults.length} file{reviewResults.length !== 1 ? "s" : ""} — partial data, verify before relying on these</span>
                      <div className="group-line" />
                    </div>
                    <div className="list-header" role="row">
                      <span></span><span>File</span><span>Size</span><span>Type</span><span>Method</span><span>Confidence</span>
                    </div>
                    {reviewResults.map(renderRow)}
                  </div>
                )}
              </div>

              <div className="bulk-bar fade-in">
                <span className="bulk-count"><b>{selected.size}</b> selected{selected.size > 0 ? ` · ${formatSize(selectedSize)}` : ""}</span>
                <div className="bulk-spacer" />
                {highConfidenceResults.length > 0 && (
                  <button className="ghost-btn" onClick={selectHighConfidence}>
                    {allHighSelected ? "Deselect high confidence" : "Select all high confidence"}
                  </button>
                )}
                <button className="primary-btn" disabled={selected.size === 0} onClick={recoverSelected}>
                  ↓ Recover selected
                </button>
              </div>
            </>
          )}

          <div className="statusbar">
            <div className={`statusbar-dot ${scanState === "scanning" ? "scanning" : scanState === "error" ? "error" : "ready"}`} />
            <span className="statusbar-msg">{statusMsg}</span>
            {error && <span style={{ color: "var(--color-danger)" }}>⚠ {error}</span>}
            <span className="statusbar-spacer" />
            <span className="statusbar-version">Recovar v0.1.0</span>
          </div>
        </section>
      </div>
    </div>
  );
}
