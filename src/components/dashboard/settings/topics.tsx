import type { ReactNode } from "react";
import type { SettingDef } from "./catalog";
import { BookIcon, MicIcon, MonitorIcon, PenIcon, SpeakerIcon } from "./icons";

export type TopicId = "dictation" | "writing" | "readaloud" | "words" | "system";

export const TOPICS: { id: TopicId; label: string; blurb: string; Icon: () => ReactNode }[] = [
  { id: "dictation", label: "Dictation", blurb: "Your hotkey, microphone and language", Icon: MicIcon },
  { id: "writing", label: "Writing help", blurb: "Spelling, grammar and writing styles", Icon: PenIcon },
  { id: "readaloud", label: "Read aloud", blurb: "Hear selected text spoken", Icon: SpeakerIcon },
  { id: "words", label: "Words", blurb: "Names, jargon and shortcuts", Icon: BookIcon },
  { id: "system", label: "System", blurb: "Startup, updates, memory and graphics card", Icon: MonitorIcon },
];

// Every word of the query must appear in the setting's label, explanation or
// keywords. Keywords carry the old names ("input sensitivity") and jargon
// ("stt", "gpu") so people who remember the previous page still find things.
export function matches(def: Pick<SettingDef, "label" | "keywords"> & { info?: unknown }, query: string) {
  const info = typeof def.info === "string" ? def.info : "";
  const hay = `${def.label} ${info} ${def.keywords ?? ""}`.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((w) => hay.includes(w));
}
