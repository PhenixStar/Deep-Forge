import { useState, useEffect, type ChangeEvent } from "react";
import type { Status, Camera, Enhancers, Resolution } from "../types";

const API_BASE = "http://localhost:8008";

const RESOLUTIONS: Resolution[] = [
  { width: 640,  height: 480,  label: "480p (640x480)"   },
  { width: 1280, height: 720,  label: "720p (1280x720)"  },
  { width: 1920, height: 1080, label: "1080p (1920x1080)" },
];

interface ServerModeInfo {
  remote_mode: boolean;
  bind_address: string;
  api_token?: string;
}

interface ControlsPanelProps {
  status: Status;
  cameras: Camera[];
  selectedCamera: number;
  enhancers: Enhancers;
  sourceImage: string | null;
  sourceScore: number | null;
  showDebugOverlay: boolean;
  onConnect: () => void;
  onDisconnect: () => void;
  onCameraChange: (e: ChangeEvent<HTMLSelectElement>) => void;
  onEnhancerToggle: (key: keyof Enhancers, checked: boolean) => void;
  onSourceUpload: (e: ChangeEvent<HTMLInputElement>) => void;
  onToggleDebug: () => void;
}

const ENHANCER_LABELS: { key: keyof Enhancers; label: string }[] = [
  { key: "face_enhancer",      label: "GFPGAN"   },
  { key: "face_enhancer_gpen256", label: "GPEN-256" },
  { key: "face_enhancer_gpen512", label: "GPEN-512" },
];

export function ControlsPanel({
  status,
  cameras: initialCameras,
  selectedCamera,
  enhancers,
  sourceImage,
  sourceScore,
  showDebugOverlay,
  onConnect,
  onDisconnect,
  onCameraChange,
  onEnhancerToggle,
  onSourceUpload,
  onToggleDebug,
}: ControlsPanelProps) {
  const [cameras, setCameras] = useState<Camera[]>(initialCameras);
  const [refreshing, setRefreshing] = useState(false);
  const [resolution, setResolution] = useState<Resolution>(RESOLUTIONS[0]);
  const [serverMode, setServerMode] = useState<ServerModeInfo | null>(null);
  const [tokenCopied, setTokenCopied] = useState(false);

  // Sync if parent updates cameras (initial load)
  useEffect(() => {
    if (initialCameras.length > 0) {
      setCameras(initialCameras);
    }
  }, [initialCameras]);

  // Fetch server mode info from /health on mount
  useEffect(() => {
    fetch(`${API_BASE}/health`)
      .then((res) => res.json())
      .then((data: { remote_mode?: boolean; bind_address?: string }) => {
        if (data.remote_mode !== undefined) {
          setServerMode({
            remote_mode: data.remote_mode,
            bind_address: data.bind_address ?? "127.0.0.1:8008",
          });
        }
      })
      .catch(() => {});
  }, []);

  const handleRefreshCameras = async () => {
    setRefreshing(true);
    try {
      const res = await fetch(`${API_BASE}/cameras/refresh`, { method: "POST" });
      if (res.ok) {
        const data = (await res.json()) as { cameras: Camera[] };
        setCameras(data.cameras);
      }
    } catch {
      // Silently ignore — cameras list stays as-is
    } finally {
      setRefreshing(false);
    }
  };

  const handleResolutionChange = async (e: ChangeEvent<HTMLSelectElement>) => {
    const selected = RESOLUTIONS.find((r) => r.label === e.target.value);
    if (!selected) return;
    setResolution(selected);
    try {
      await fetch(`${API_BASE}/settings`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          resolution_width:  selected.width,
          resolution_height: selected.height,
        }),
      });
    } catch {
      // Best-effort; local state already updated
    }
  };

  const handleCopyToken = async () => {
    if (!serverMode?.api_token) return;
    try {
      await navigator.clipboard.writeText(serverMode.api_token);
      setTokenCopied(true);
      setTimeout(() => setTokenCopied(false), 2000);
    } catch {
      // Clipboard not available
    }
  };

  return (
    <section className="controls">
      <div className="source-face">
        <label>Source Face</label>
        {sourceImage ? (
          <img src={sourceImage} alt="source" className="face-preview" />
        ) : (
          <div className="placeholder">No face selected</div>
        )}
        {sourceScore !== null && (
          <div className="source-score">
            Detection: {(sourceScore * 100).toFixed(0)}%
          </div>
        )}
        <input type="file" accept="image/*" onChange={onSourceUpload} />
      </div>

      <div className="camera-select">
        <div className="camera-select-header">
          <label>Camera</label>
          <button
            className="btn-refresh"
            onClick={handleRefreshCameras}
            disabled={refreshing}
            title="Refresh camera list"
          >
            {refreshing ? "..." : "Refresh"}
          </button>
        </div>
        <select value={selectedCamera} onChange={onCameraChange}>
          {cameras.map((c) => (
            <option key={c.index} value={c.index}>
              {c.name}
            </option>
          ))}
        </select>
      </div>

      <div className="resolution-select">
        <label>Resolution</label>
        <select value={resolution.label} onChange={handleResolutionChange}>
          {RESOLUTIONS.map((r) => (
            <option key={r.label} value={r.label}>
              {r.label}
            </option>
          ))}
        </select>
      </div>

      <div className="enhancers">
        <label>Face Enhancers</label>
        {ENHANCER_LABELS.map(({ key, label }) => (
          <label key={key} className="toggle">
            <input
              type="checkbox"
              checked={enhancers[key]}
              onChange={(e) => onEnhancerToggle(key, e.target.checked)}
            />
            {label}
          </label>
        ))}
      </div>

      <div className="debug-toggle-row">
        <label className="toggle">
          <input
            type="checkbox"
            className="debug-toggle"
            checked={showDebugOverlay}
            onChange={onToggleDebug}
          />
          Debug Overlay
        </label>
      </div>

      <div className="actions">
        {status === "disconnected" ? (
          <button className="btn primary" onClick={onConnect}>
            Start Live
          </button>
        ) : (
          <button className="btn danger" onClick={onDisconnect}>
            Stop
          </button>
        )}
      </div>

      {serverMode && (
        <div className="server-mode">
          <label>Server Mode</label>
          {serverMode.remote_mode ? (
            <div className="server-mode-info">
              <div className="server-mode-row">
                <span className="server-mode-badge remote">Remote</span>
              </div>
              <div className="server-mode-row">
                <span className="server-mode-label">Bind</span>
                <span className="server-mode-value">{serverMode.bind_address}</span>
              </div>
              {serverMode.api_token && (
                <div className="server-mode-row">
                  <span className="server-mode-label">Token</span>
                  <span className="server-mode-value token">
                    {serverMode.api_token}
                  </span>
                  <button className="btn-copy" onClick={handleCopyToken}>
                    {tokenCopied ? "Copied" : "Copy"}
                  </button>
                </div>
              )}
            </div>
          ) : (
            <div className="server-mode-info">
              <div className="server-mode-row">
                <span className="server-mode-badge local">Local only</span>
              </div>
              <p className="server-mode-hint">
                Start with <code>--remote</code> flag to enable LAN access.
              </p>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
