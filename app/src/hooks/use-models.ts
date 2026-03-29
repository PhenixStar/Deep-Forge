import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { ModelInfo } from "../types";

const API_BASE = "http://localhost:8008";

const MODEL_URLS: Record<string, string> = {
  "buffalo_l/buffalo_l/det_10g.onnx":
    "https://huggingface.co/hacksider/deep-live-cam/resolve/main/buffalo_l/buffalo_l/det_10g.onnx",
  "buffalo_l/buffalo_l/w600k_r50.onnx":
    "https://huggingface.co/hacksider/deep-live-cam/resolve/main/buffalo_l/buffalo_l/w600k_r50.onnx",
  "inswapper_128.onnx":
    "https://huggingface.co/hacksider/deep-live-cam/resolve/main/inswapper_128_fp16.onnx",
  "gfpgan-1024.onnx":
    "https://huggingface.co/hacksider/deep-live-cam/resolve/main/gfpgan-1024.onnx",
  "GPEN-BFR-256.onnx":
    "https://huggingface.co/hacksider/deep-live-cam/resolve/main/GPEN-BFR-256.onnx",
  "GPEN-BFR-512.onnx":
    "https://huggingface.co/hacksider/deep-live-cam/resolve/main/GPEN-BFR-512.onnx",
};

export function hasDownloadUrl(file: string): boolean {
  return file in MODEL_URLS;
}

interface DownloadProgressEvent {
  name: string;
  downloaded: number;
  total: number;
}

export function useModels(): {
  models: ModelInfo[];
  downloading: Record<string, number>;
  downloadModel: (model: ModelInfo) => void;
  refresh: () => void;
} {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [downloading, setDownloading] = useState<Record<string, number>>({});
  const unlistenRef = useRef<UnlistenFn | null>(null);

  const fetchModels = useCallback(() => {
    fetch(`${API_BASE}/models/status`)
      .then((r) => r.json())
      .then((data: { models: ModelInfo[] }) => setModels(data.models))
      .catch(() => {});
  }, []);

  useEffect(() => {
    fetchModels();

    let active = true;

    listen<DownloadProgressEvent>("model_download_progress", (event) => {
      if (!active) return;
      const { name, downloaded, total } = event.payload;
      const pct = total > 0 ? Math.round((downloaded / total) * 100) : 0;
      setDownloading((prev) => ({ ...prev, [name]: pct }));
    }).then((fn) => {
      unlistenRef.current = fn;
    });

    return () => {
      active = false;
      unlistenRef.current?.();
    };
  }, [fetchModels]);

  const downloadModel = useCallback(
    async (model: ModelInfo) => {
      const url = MODEL_URLS[model.file];
      if (!url) return;

      setDownloading((prev) => ({ ...prev, [model.name]: 0 }));

      try {
        // Resolve absolute path via Tauri command
        const modelsDir = await invoke<string>("get_models_dir");
        const dest = modelsDir + "/" + model.file.replace(/\//g, "/");

        await invoke<void>("download_model", {
          name: model.name,
          url,
          dest,
        });

        setDownloading((prev) => {
          const next = { ...prev };
          delete next[model.name];
          return next;
        });
        fetchModels();
      } catch {
        setDownloading((prev) => {
          const next = { ...prev };
          delete next[model.name];
          return next;
        });
      }
    },
    [fetchModels],
  );

  return { models, downloading, downloadModel, refresh: fetchModels };
}
