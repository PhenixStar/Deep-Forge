import type { FrameMetrics, SystemMetrics } from "../types";

interface MetricsPanelProps {
  fps: number;
  inferenceMetrics: FrameMetrics | null;
  systemMetrics: SystemMetrics | null;
  gpuProvider: string;
  sourceScore: number | null;
}

function gpuProviderVariant(provider: string): "directml" | "npu" | "cpu" | "auto" {
  const lower = provider.toLowerCase();
  if (lower.includes("directml")) return "directml";
  if (lower.includes("npu") || lower.includes("vitisai")) return "npu";
  if (lower === "cpu") return "cpu";
  return "auto";
}

function GpuProviderBadge({ provider }: { provider: string }) {
  const variant = gpuProviderVariant(provider);
  return (
    <span className={`provider-badge provider-badge--${variant}`}>
      {provider}
    </span>
  );
}

function MetricRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="metric-row">
      <span className="metric-label">{label}</span>
      <span className="metric-value">{value}</span>
    </div>
  );
}

function MetricSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="metric-section">
      <div className="metric-section-label">{title}</div>
      {children}
    </div>
  );
}

export function MetricsPanel({
  fps,
  inferenceMetrics,
  systemMetrics,
  gpuProvider,
  sourceScore,
}: MetricsPanelProps) {
  const na = "—";

  return (
    <aside className="metrics-panel">
      <MetricSection title="INFERENCE">
        <MetricRow label="FPS" value={fps > 0 ? String(fps) : na} />
        <MetricRow
          label="Detect"
          value={inferenceMetrics ? `${inferenceMetrics.detect_ms.toFixed(1)} ms` : na}
        />
        <MetricRow
          label="Swap"
          value={inferenceMetrics ? `${inferenceMetrics.swap_ms.toFixed(1)} ms` : na}
        />
        <MetricRow
          label="Total"
          value={inferenceMetrics ? `${inferenceMetrics.total_ms.toFixed(1)} ms` : na}
        />
        <MetricRow
          label="Faces"
          value={inferenceMetrics ? String(inferenceMetrics.face_count) : na}
        />
        {inferenceMetrics && inferenceMetrics.faces.length > 0 && (
          <MetricRow
            label="Best score"
            value={`${(Math.max(...inferenceMetrics.faces.map((f) => f.score)) * 100).toFixed(0)}%`}
          />
        )}
      </MetricSection>

      <MetricSection title="SYSTEM">
        <MetricRow
          label="CPU"
          value={systemMetrics ? `${systemMetrics.cpu_percent.toFixed(1)}%` : na}
        />
        <MetricRow
          label="RAM"
          value={
            systemMetrics
              ? `${systemMetrics.ram_used_gb.toFixed(1)} / ${systemMetrics.ram_total_gb.toFixed(1)} GB`
              : na
          }
        />
        <div className="metric-row">
          <span className="metric-label">GPU</span>
          {gpuProvider ? (
            <GpuProviderBadge provider={gpuProvider} />
          ) : (
            <span className="metric-value">{na}</span>
          )}
        </div>
      </MetricSection>

      <MetricSection title="SOURCE">
        <MetricRow
          label="Score"
          value={sourceScore !== null ? `${(sourceScore * 100).toFixed(0)}%` : na}
        />
      </MetricSection>
    </aside>
  );
}
