import { useCallback, useEffect, useRef, useState } from "react";

const ROW_H = 26; // px per number row
const COLLAPSED_H = 36; // matches the height of a <select> in the same row
const HALF = 2; // rows rendered above/below the centre (visible span = 5)
const BUFFER = 8; // extra rows kept off-screen so a slide never shows a gap
const WHEEL_STEP = 34; // px of wheel travel per number

// A deep mechanical detent — the muted thunk of a ratchet, not a UI blip.
// Two layers: a low body tone that pitch-drops fast, plus a tiny filtered
// noise burst for the mechanical "knock" transient. Everything is lowpassed
// so nothing bright survives. Built lazily — an AudioContext created before
// any user gesture is blocked.
let audioCtx: AudioContext | null = null;
let noiseBuffer: AudioBuffer | null = null;

function tick() {
  try {
    audioCtx ??= new AudioContext();
    const ctx = audioCtx;
    if (ctx.state === "suspended") void ctx.resume();
    const t = ctx.currentTime;

    const lowpass = ctx.createBiquadFilter();
    lowpass.type = "lowpass";
    lowpass.frequency.setValueAtTime(320, t);
    lowpass.connect(ctx.destination);

    // Body: 130Hz collapsing to 45Hz in 35ms reads as a solid, weighty click.
    const osc = ctx.createOscillator();
    osc.type = "triangle";
    osc.frequency.setValueAtTime(130, t);
    osc.frequency.exponentialRampToValueAtTime(45, t + 0.035);
    const oscGain = ctx.createGain();
    oscGain.gain.setValueAtTime(0.16, t);
    oscGain.gain.exponentialRampToValueAtTime(0.0001, t + 0.055);
    osc.connect(oscGain).connect(lowpass);
    osc.start(t);
    osc.stop(t + 0.06);

    // Knock transient: 12ms of lowpassed noise gives it a mechanical edge.
    if (!noiseBuffer) {
      const len = Math.floor(ctx.sampleRate * 0.012);
      noiseBuffer = ctx.createBuffer(1, len, ctx.sampleRate);
      const data = noiseBuffer.getChannelData(0);
      for (let i = 0; i < len; i++) {
        data[i] = (Math.random() * 2 - 1) * (1 - i / len);
      }
    }
    const noise = ctx.createBufferSource();
    noise.buffer = noiseBuffer;
    const noiseGain = ctx.createGain();
    noiseGain.gain.setValueAtTime(0.06, t);
    noiseGain.gain.exponentialRampToValueAtTime(0.0001, t + 0.02);
    noise.connect(noiseGain).connect(lowpass);
    noise.start(t);
  } catch {
    // Audio is a nicety — never let it break the picker.
  }
}

interface Props {
  value: number;
  onChange: (value: number) => void;
  min: number;
  max: number;
  width?: number;
  sound?: boolean;
}

export default function ScrollNumberPicker({
  value,
  onChange,
  min,
  max,
  width = 64,
  sound = true,
}: Props) {
  const [open, setOpen] = useState(false);
  const [dragging, setDragging] = useState(false);
  // The strip is laid out around `anchor` and translated by (value - anchor)
  // rows. Keeping the anchor still is what makes the numbers visibly SLIDE
  // instead of just swapping in place.
  const [anchor, setAnchor] = useState(value);
  const [animate, setAnimate] = useState(true);

  const wrapRef = useRef<HTMLDivElement>(null);
  const accum = useRef(0);
  const valueRef = useRef(value);
  valueRef.current = value;

  const clamp = useCallback(
    (v: number) => Math.max(min, Math.min(max, v)),
    [min, max]
  );

  const commit = useCallback(
    (next: number) => {
      const v = clamp(next);
      if (v === valueRef.current) return;
      if (sound) tick();
      onChange(v);
    },
    [clamp, onChange, sound]
  );

  // Re-anchor once the strip has slid far enough that it would run out of
  // rendered rows. Done with animation off so the jump is invisible.
  useEffect(() => {
    if (Math.abs(value - anchor) <= BUFFER - HALF) return;
    setAnimate(false);
    setAnchor(value);
    const id = requestAnimationFrame(() =>
      requestAnimationFrame(() => setAnimate(true))
    );
    return () => cancelAnimationFrame(id);
  }, [value, anchor]);

  useEffect(() => {
    if (open) setAnchor(valueRef.current);
  }, [open]);

  // React's onWheel is passive, so preventDefault there is ignored and the
  // page scrolls behind the picker. A native non-passive listener is the only
  // way to actually keep the wheel local to this control.
  useEffect(() => {
    const el = wrapRef.current;
    if (!el || !open) return;
    function onWheel(e: WheelEvent) {
      e.preventDefault();
      e.stopPropagation();
      accum.current += e.deltaY;
      const steps = Math.trunc(accum.current / WHEEL_STEP);
      if (steps !== 0) {
        accum.current -= steps * WHEEL_STEP;
        commit(valueRef.current + steps);
      }
    }
    el.addEventListener("wheel", onWheel, { passive: false });
    return () => el.removeEventListener("wheel", onWheel);
  }, [open, commit]);

  // Click-away closes.
  useEffect(() => {
    if (!open) return;
    function onDown(e: MouseEvent) {
      if (!wrapRef.current?.contains(e.target as Node)) setOpen(false);
    }
    window.addEventListener("mousedown", onDown);
    return () => window.removeEventListener("mousedown", onDown);
  }, [open]);

  // Drag to spin.
  useEffect(() => {
    if (!dragging) return;
    const startY = dragStart.current.y;
    const startValue = dragStart.current.value;
    function onMove(e: MouseEvent) {
      commit(startValue - Math.round((e.clientY - startY) / ROW_H));
    }
    function onUp() {
      setDragging(false);
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    return () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
  }, [dragging, commit]);

  const dragStart = useRef({ y: 0, value });

  if (!open) {
    return (
      <button
        onClick={() => setOpen(true)}
        style={{ width, height: COLLAPSED_H }}
        className="rounded-lg border border-sv-border bg-sv-bg text-sm font-semibold tabular-nums text-sv-text transition hover:border-sv-accent/60"
        title="Click to change"
      >
        {value}
      </button>
    );
  }

  const rows: number[] = [];
  for (let n = anchor - BUFFER; n <= anchor + BUFFER; n++) {
    if (n >= min && n <= max) rows.push(n);
  }
  const height = ROW_H * (HALF * 2 + 1);

  return (
    <div style={{ width, height: COLLAPSED_H }} className="relative">
      <div
        ref={wrapRef}
        role="spinbutton"
        tabIndex={0}
        aria-valuenow={value}
        aria-valuemin={min}
        aria-valuemax={max}
        autoFocus
        onMouseDown={(e) => {
          dragStart.current = { y: e.clientY, value };
          setDragging(true);
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowUp") {
            e.preventDefault();
            commit(value + 1);
          } else if (e.key === "ArrowDown") {
            e.preventDefault();
            commit(value - 1);
          } else if (e.key === "Enter" || e.key === "Escape") {
            setOpen(false);
          }
        }}
        style={{
          width,
          height,
          top: -(height - COLLAPSED_H) / 2,
          // Fades the top/bottom rows out instead of cutting them off.
          maskImage:
            "linear-gradient(to bottom, transparent, #000 22%, #000 78%, transparent)",
          WebkitMaskImage:
            "linear-gradient(to bottom, transparent, #000 22%, #000 78%, transparent)",
        }}
        className={`absolute left-0 z-20 overflow-hidden rounded-xl border border-sv-accent/40 bg-sv-surface shadow-lg outline-none ring-1 ring-sv-accent/20 ${
          dragging ? "cursor-grabbing" : "cursor-grab"
        }`}
      >
        <div
          className="pointer-events-none absolute inset-x-1 rounded-md bg-sv-accent/10"
          style={{ top: HALF * ROW_H, height: ROW_H }}
        />
        <div
          style={{
            transform: `translateY(${
              HALF * ROW_H - (value - anchor) * ROW_H
            }px)`,
            transition: animate
              ? "transform 180ms cubic-bezier(0.22, 1, 0.36, 1)"
              : "none",
          }}
        >
          {rows.map((n) => {
            const d = Math.abs(n - value);
            return (
              <div
                key={n}
                onClick={(e) => {
                  if (n !== value) {
                    e.stopPropagation();
                    commit(n);
                  }
                }}
                style={{
                  position: "absolute",
                  top: (n - anchor) * ROW_H,
                  height: ROW_H,
                  left: 0,
                  right: 0,
                  fontSize: d === 0 ? 17 : d === 1 ? 13 : 11,
                  opacity: d === 0 ? 1 : d === 1 ? 0.5 : d === 2 ? 0.22 : 0,
                  filter: d === 0 ? "none" : `blur(${d * 0.7}px)`,
                  transition:
                    "font-size 180ms ease, opacity 180ms ease, filter 180ms ease",
                }}
                className={`flex items-center justify-center tabular-nums ${
                  d === 0 ? "font-semibold text-sv-text" : "text-sv-muted"
                }`}
              >
                {n}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
