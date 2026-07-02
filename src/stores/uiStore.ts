import { create } from "zustand";
import type { RecordingState } from "../types";

interface UiState {
  recordingState: RecordingState;
  lastError: string | null;
  setRecordingState: (s: RecordingState) => void;
  setError: (e: string | null) => void;
}

// Lightweight UI-only state shared between the single pipeline subscription
// (in Dashboard) and views like Home that just need to display status.
export const useUiStore = create<UiState>((set) => ({
  recordingState: "idle",
  lastError: null,
  setRecordingState: (recordingState) => set({ recordingState }),
  setError: (lastError) => set({ lastError }),
}));
