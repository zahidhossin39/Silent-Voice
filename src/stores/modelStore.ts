import { create } from "zustand";
import type { DownloadProgress } from "../types";
import {
  listDownloadedModels,
  downloadModel as bridgeDownload,
  deleteModel as bridgeDelete,
  listDownloadedLlm,
  downloadLlmModel as bridgeDownloadLlm,
  deleteLlmModel as bridgeDeleteLlm,
  listDownloadedTts,
  downloadTtsModel as bridgeDownloadTts,
  deleteTtsModel as bridgeDeleteTts,
} from "../services/tauriBridge";
import {
  STT_MODELS,
  WHISPER_BASE_URL,
  LLM_MODELS,
  TTS_MODELS,
} from "../services/catalog";

interface ModelState {
  downloaded: Set<string>; // STT (whisper) model ids
  downloadedLlm: Set<string>; // LLM (GGUF) model ids
  downloadedTts: Set<string>; // TTS (Piper) voice ids
  progress: Record<string, DownloadProgress>;
  refresh: () => Promise<void>;
  download: (modelId: string) => Promise<void>;
  remove: (modelId: string) => Promise<void>;
  downloadLlm: (modelId: string) => Promise<void>;
  removeLlm: (modelId: string) => Promise<void>;
  downloadTts: (voiceId: string) => Promise<void>;
  removeTts: (voiceId: string) => Promise<void>;
}

function startProgress(
  set: (fn: (s: ModelState) => Partial<ModelState>) => void,
  modelId: string,
  totalBytes: number
) {
  set((s) => ({
    progress: {
      ...s.progress,
      [modelId]: {
        model_id: modelId,
        downloaded_bytes: 0,
        total_bytes: totalBytes,
        status: "downloading",
      },
    },
  }));
}

export const useModelStore = create<ModelState>((set) => ({
  downloaded: new Set<string>(),
  downloadedLlm: new Set<string>(),
  downloadedTts: new Set<string>(),
  progress: {},

  refresh: async () => {
    const [stt, llm, tts] = await Promise.all([
      listDownloadedModels(),
      listDownloadedLlm(),
      listDownloadedTts(),
    ]);
    set({
      downloaded: new Set(stt),
      downloadedLlm: new Set(llm),
      downloadedTts: new Set(tts),
    });
  },

  download: async (modelId) => {
    const model = STT_MODELS.find((m) => m.id === modelId);
    if (!model) return;
    startProgress(set, modelId, model.size_mb * 1024 * 1024);
    try {
      const downloadUrl = model.url ?? WHISPER_BASE_URL + model.file;
      await bridgeDownload(modelId, downloadUrl, model.file);
      set((s) => {
        const downloaded = new Set(s.downloaded);
        downloaded.add(modelId);
        return {
          downloaded,
          progress: {
            ...s.progress,
            [modelId]: { ...s.progress[modelId], status: "downloaded" },
          },
        };
      });
    } catch (e) {
      set((s) => ({
        progress: {
          ...s.progress,
          [modelId]: { ...s.progress[modelId], status: "error", error: String(e) },
        },
      }));
    }
  },

  remove: async (modelId) => {
    await bridgeDelete(modelId);
    set((s) => {
      const downloaded = new Set(s.downloaded);
      downloaded.delete(modelId);
      const progress = { ...s.progress };
      delete progress[modelId];
      return { downloaded, progress };
    });
  },

  downloadLlm: async (modelId) => {
    const model = LLM_MODELS.find((m) => m.id === modelId);
    if (!model) return;
    startProgress(set, modelId, model.size_mb * 1024 * 1024);
    try {
      await bridgeDownloadLlm(modelId, model.url);
      set((s) => {
        const downloadedLlm = new Set(s.downloadedLlm);
        downloadedLlm.add(modelId);
        return {
          downloadedLlm,
          progress: {
            ...s.progress,
            [modelId]: { ...s.progress[modelId], status: "downloaded" },
          },
        };
      });
    } catch (e) {
      set((s) => ({
        progress: {
          ...s.progress,
          [modelId]: { ...s.progress[modelId], status: "error", error: String(e) },
        },
      }));
    }
  },

  removeLlm: async (modelId) => {
    await bridgeDeleteLlm(modelId);
    set((s) => {
      const downloadedLlm = new Set(s.downloadedLlm);
      downloadedLlm.delete(modelId);
      const progress = { ...s.progress };
      delete progress[modelId];
      return { downloadedLlm, progress };
    });
  },

  downloadTts: async (voiceId) => {
    const voice = TTS_MODELS.find((v) => v.id === voiceId);
    if (!voice) return;
    startProgress(set, voiceId, voice.size_mb * 1024 * 1024);
    try {
      await bridgeDownloadTts(voiceId, voice.url_onnx, voice.url_json);
      set((s) => {
        const downloadedTts = new Set(s.downloadedTts);
        downloadedTts.add(voiceId);
        return {
          downloadedTts,
          progress: {
            ...s.progress,
            [voiceId]: { ...s.progress[voiceId], status: "downloaded" },
          },
        };
      });
    } catch (e) {
      set((s) => ({
        progress: {
          ...s.progress,
          [voiceId]: { ...s.progress[voiceId], status: "error", error: String(e) },
        },
      }));
    }
  },

  removeTts: async (voiceId) => {
    await bridgeDeleteTts(voiceId);
    set((s) => {
      const downloadedTts = new Set(s.downloadedTts);
      downloadedTts.delete(voiceId);
      const progress = { ...s.progress };
      delete progress[voiceId];
      return { downloadedTts, progress };
    });
  },
}));

// Allow the Rust backend to push live download progress via events.
export function applyDownloadProgress(p: DownloadProgress) {
  useModelStore.setState((s) => ({
    progress: { ...s.progress, [p.model_id]: p },
  }));
  // When a download completes, make sure it lands in the right downloaded set.
  if (p.status === "downloaded") {
    void useModelStore.getState().refresh();
  }
}
