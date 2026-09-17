// Settings: a rail of five topics with search. Each topic page shows the
// settings a beginner needs, then a fold-open "Advanced" card that previews
// what's inside before you open it. Explanations live behind info icons.
// Settings themselves are described in settings/catalog.tsx.
import { useEffect, useState } from "react";
import Page from "../shared/Page";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { useHardwareInfo } from "../../hooks/useHardwareInfo";
import { coeditInstalled, gectorInstalled, getAutostart, listInputDevices, setHotkey } from "../../services/tauriBridge";
import { AUTOMATIC, SETTINGS, type Ctx, type SettingDef } from "./settings/catalog";
import { TOPICS, type TopicId } from "./settings/topics";
import { AdvancedCard, Rail, Rows, SearchResults, TopicHeading, resolveJumpTarget, useJump } from "./settings/parts";
import { WandIcon } from "./settings/icons";

export { Toggle } from "./settings/parts";

// Survive leaving and re-opening the page within a session: come back to the
// same topic, with the same advanced cards open.
let lastTopic: TopicId = "dictation";
const openCards: Partial<Record<TopicId, boolean>> = {};

export default function Settings() {
  const settings = useSettingsStore((s) => s.settings);
  const setSettings = useSettingsStore((s) => s.setSettings);
  const { hardware } = useHardwareInfo();

  const [devices, setDevices] = useState<string[]>([]);
  const [hotkeyError, setHotkeyError] = useState<string | null>(null);

  const [coeditReady, setCoeditReady] = useState(false);
  const [gectorReady, setGectorReady] = useState(false);
  const coeditStatus = useModelStore((s) => s.progress["coedit"]?.status);
  const gectorStatus = useModelStore((s) => s.progress["gector"]?.status);
  useEffect(() => {
    coeditInstalled().then(setCoeditReady);
  }, [coeditStatus]);
  useEffect(() => {
    gectorInstalled().then(setGectorReady);
  }, [gectorStatus]);

  useEffect(() => {
    listInputDevices().then(setDevices);
    // The registry is the truth for "Start with Windows": sync the toggle to it
    // so the page can't show ON while no Run-key entry actually exists.
    getAutostart().then((real) => {
      if (useSettingsStore.getState().settings.auto_start !== real) setSettings({ auto_start: real });
    });
  }, [setSettings]);

  const [topic, setTopicState] = useState<TopicId>(lastTopic);
  const setTopic = (t: TopicId) => {
    lastTopic = t;
    setTopicState(t);
  };
  const [open, setOpenState] = useState<Partial<Record<TopicId, boolean>>>(() => ({ ...openCards }));
  const setOpen = (t: TopicId, v: boolean) => {
    openCards[t] = v;
    setOpenState((o) => ({ ...o, [t]: v }));
  };
  const [query, setQuery] = useState("");
  const [flash, setFlash] = useJump();

  const ctx: Ctx = {
    s: settings,
    set: setSettings,
    onHotkey: async (accelerator) => {
      setHotkeyError(null);
      setSettings({ hotkey: accelerator });
      try {
        await setHotkey(accelerator);
      } catch {
        setHotkeyError("Another app or Windows already uses that key. Try a different combination.");
      }
    },
    hotkeyError,
    devices,
    gpuName: hardware?.gpu_vram_gb && hardware.gpu_vram_gb >= 1 ? hardware.gpu_name ?? "your GPU" : null,
    coeditReady,
    setCoeditReady,
    gectorReady,
    setGectorReady,
    toggleRule: (rule, enabled) => {
      const rest = settings.proofread_disabled_rules.filter((r) => r !== rule);
      setSettings({ proofread_disabled_rules: enabled ? rest : [...rest, rule] });
    },
  };

  const shown = (x: SettingDef) => !x.visible || x.visible(ctx);
  const basic = SETTINGS.filter((x) => x.topic === topic && x.tier === "basic" && shown(x));
  const advanced = SETTINGS.filter((x) => x.topic === topic && x.tier === "advanced" && shown(x));
  const topicLabel = TOPICS.find((t) => t.id === topic)!.label;

  const jumpTo = (def: SettingDef) => {
    // A hidden setting can't be shown until its parent is on, so land on the
    // parent instead of changing anything for the user.
    const target = resolveJumpTarget(def, SETTINGS, ctx);
    setQuery("");
    setTopic(target.topic);
    if (target.tier === "advanced") setOpen(target.topic, true);
    setFlash(target.id);
  };

  return (
    <Page title="Settings">
      <div className="flex flex-col gap-5 lg:flex-row lg:gap-8">
        <Rail query={query} onQuery={setQuery} current={topic} onSelect={setTopic} />
        <div className="@container min-w-0 max-w-3xl flex-1 pb-16">
          {query.trim() ? (
            <SearchResults ctx={ctx} list={SETTINGS} query={query} onPick={jumpTo} />
          ) : (
            <section key={topic}>
              <TopicHeading id={topic} />
              <div className="mt-6">
                <Rows ctx={ctx} items={basic} flash={flash} />
                <AdvancedCard
                  ctx={ctx}
                  topicLabel={topicLabel}
                  items={advanced}
                  open={!!open[topic]}
                  onToggle={() => setOpen(topic, !open[topic])}
                  flash={flash}
                  panelId={`advanced-${topic}`}
                />
              </div>
              {topic === "system" && <AutomaticNote />}
            </section>
          )}
        </div>
      </div>
    </Page>
  );
}

function AutomaticNote() {
  return (
    <details className="group mt-8 text-[13px]">
      <summary className="flex w-fit cursor-pointer list-none items-center gap-2 rounded-lg px-2 py-1.5 text-sv-muted transition-colors duration-150 hover:bg-sv-surface-2 hover:text-sv-text focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-sv-accent">
        <WandIcon />
        What Silent Voice sets up for you
      </summary>
      <ul className="mt-2 max-w-[62ch] space-y-2.5 pl-8">
        {AUTOMATIC.map((a) => (
          <li key={a.label}>
            <span className="text-sv-text">{a.label}.</span> <span className="text-sv-muted">{a.detail}</span>
          </li>
        ))}
      </ul>
    </details>
  );
}
