import { create } from "zustand";

// A single polite screen-reader live region for the whole app. Anything that
// finishes silently for sighted users (a dictation landing, "learned N words",
// a download completing, an error) calls announce() so assistive tech speaks
// it. The message is rendered once, in the app shell.
interface AnnounceState {
  message: string;
  announce: (message: string) => void;
}

export const useAnnounceStore = create<AnnounceState>((set) => ({
  message: "",
  // Blank first, then set on the next tick, so re-announcing the same text
  // still registers as a DOM change and gets spoken again.
  announce: (message) => {
    set({ message: "" });
    setTimeout(() => set({ message }), 60);
  },
}));
