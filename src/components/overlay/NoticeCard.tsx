import { useEffect, useRef, useState } from "react";
import {
  copyDiagnostics,
  copyToClipboard,
  listenEvent,
  noticeCurrent,
  noticeHide,
  noticeOpenSettings,
  type Notice,
} from "../../services/tauriBridge";

// The on-screen problem card (system/notice.rs): bottom-center, above the
// taskbar, gone after a few seconds unless hovered. Opaque window, so the
// card fills it edge to edge and DWM rounds the corners.
const SHOW_MS = 7000;
const BG = "#0b0b0d";

const BUTTON: Record<Notice["action"], string> = {
  mic: "Select microphone",
  settings: "Open settings",
  logs: "Copy logs",
  "": "",
};

export default function NoticeCard() {
  const [n, setN] = useState<Notice | null>(null);
  const [seq, setSeq] = useState(0); // restarts the countdown per notice
  const [paused, setPaused] = useState(false);
  const [copied, setCopied] = useState(false);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    document.documentElement.style.background = BG;
    document.body.style.background = BG;
    const show = (next: Notice | null) => {
      if (!next) return;
      setN(next);
      setCopied(false);
      setSeq((s) => s + 1);
    };
    void noticeCurrent().then(show);
    const un = listenEvent<Notice>("notice://show", show);
    return () => {
      un.then((f) => f());
    };
  }, []);

  useEffect(() => {
    window.clearTimeout(timer.current);
    if (n && !paused) timer.current = window.setTimeout(() => void noticeHide(), SHOW_MS);
    return () => window.clearTimeout(timer.current);
  }, [n, seq, paused]);

  if (!n) return null;

  async function act() {
    if (!n) return;
    if (n.action === "logs") {
      await copyToClipboard(await copyDiagnostics());
      setCopied(true);
      return;
    }
    await noticeOpenSettings();
  }

  const r = 13;
  const len = 2 * Math.PI * r;
  return (
    <div
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      style={{
        height: "100vh",
        boxSizing: "border-box",
        padding: "16px 18px",
        background: BG,
        color: "#fafafa",
        fontFamily: "'Segoe UI', system-ui, sans-serif",
        display: "flex",
        flexDirection: "column",
        justifyContent: "space-between",
        userSelect: "none",
      }}
    >
      <style>{`@keyframes sv-ring { from { stroke-dashoffset: 0 } to { stroke-dashoffset: ${len} } }`}</style>
      <div style={{ display: "flex", alignItems: "flex-start", gap: 10 }}>
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#f87171" strokeWidth="2" style={{ flexShrink: 0, marginTop: 1 }}>
          <circle cx="12" cy="12" r="10" />
          <path d="M12 7v6M12 16.5v.5" strokeLinecap="round" />
        </svg>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontWeight: 600, fontSize: 15 }}>{n.title}</div>
          <div style={{ fontSize: 13, color: "rgba(250,250,250,0.62)", marginTop: 3, lineHeight: 1.35 }}>{n.body}</div>
        </div>
        <button
          aria-label="Close"
          onClick={() => void noticeHide()}
          style={{ position: "relative", width: 32, height: 32, flexShrink: 0, background: "none", border: "none", color: "#fafafa", cursor: "pointer", padding: 0 }}
        >
          <svg width="32" height="32" viewBox="0 0 32 32" style={{ position: "absolute", inset: 0, transform: "rotate(-90deg)" }}>
            <circle cx="16" cy="16" r={r} fill="none" stroke="rgba(255,255,255,0.15)" strokeWidth="2" />
            <circle
              key={seq}
              cx="16"
              cy="16"
              r={r}
              fill="none"
              stroke="rgba(255,255,255,0.7)"
              strokeWidth="2"
              strokeDasharray={len}
              style={{ animation: `sv-ring ${SHOW_MS}ms linear forwards`, animationPlayState: paused ? "paused" : "running" }}
            />
          </svg>
          <span style={{ position: "relative", fontSize: 15 }}>✕</span>
        </button>
      </div>
      {n.action && (
        <div style={{ display: "flex", justifyContent: "flex-end" }}>
          <button
            onClick={() => void act()}
            style={{ background: "#ece9e2", color: "#111", border: "none", borderRadius: 9, padding: "7px 14px", fontSize: 13, fontWeight: 600, cursor: "pointer" }}
          >
            {n.action === "logs" && copied ? "Copied" : BUTTON[n.action]}
          </button>
        </div>
      )}
    </div>
  );
}
