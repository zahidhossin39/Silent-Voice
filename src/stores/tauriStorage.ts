import type { StateStorage } from "zustand/middleware";
import { isTauri } from "../services/tauriBridge";

// Persist a zustand store to the Tauri plugin-store file (settings.json) in the
// desktop build, and to localStorage in the browser preview. Each call returns
// an INDEPENDENT storage instance with its own load-gate, so two stores never
// share the "loaded yet?" flag — sharing it would let one store's write slip
// through before the other had read its file (overwriting saved data with
// defaults). Extracted from settingsStore so the stats store reuses the exact
// same mechanism.
export function createTauriStorage(fileName = "settings.json"): StateStorage {
  let storePromise: Promise<any> | null = null;
  let isLoaded = false;

  return {
    getItem: async (name) => {
      if (isTauri()) {
        if (!storePromise) {
          storePromise = (async () => {
            try {
              const { Store } = await import("@tauri-apps/plugin-store");
              const store = await Store.load(fileName);
              // One-time migration: copy from localStorage if store is empty.
              const existing = await store.get(name);
              if (existing === null || existing === undefined) {
                const oldVal = window.localStorage.getItem(name);
                if (oldVal !== null) {
                  await store.set(name, oldVal);
                  await store.save();
                }
              }
              return store;
            } catch (e) {
              console.error("Tauri store load failed", e);
              return null;
            }
          })();
        }
        const store = await storePromise;
        if (store) {
          const val = await store.get(name);
          isLoaded = true;
          return typeof val === "string" ? val : null;
        }
        isLoaded = true;
        return null;
      } else {
        const val = window.localStorage.getItem(name);
        isLoaded = true;
        return val;
      }
    },
    setItem: async (name, value) => {
      // Block write attempts before the existing file state is loaded.
      if (!isLoaded) return;
      if (isTauri()) {
        const store = await storePromise;
        if (store) {
          await store.set(name, value);
          await store.save();
        }
      } else {
        window.localStorage.setItem(name, value);
      }
    },
    removeItem: async (name) => {
      if (isTauri()) {
        const store = await storePromise;
        if (store) {
          await store.delete(name);
          await store.save();
        }
      } else {
        window.localStorage.removeItem(name);
      }
    },
  };
}
