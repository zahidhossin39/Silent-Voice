import { useCallback, useEffect, useRef, useState } from "react";

const ROW_H = 28; // px per number row

interface Props {
  value: number;
  onChange: (value: number) => void;
  min: number;
  max: number;
  width?: number;
}

// iOS-style scroll wheel: the current value sits in the center row, with the
// neighbors faded above/below. Scroll (wheel or drag) to change; click a
// neighbor row to jump straight to it. Replaces the old up/down-arrow
// <input type="number"> spinner.
export default function ScrollNumberPicker({ value, onChange, min, max, width = 64 }: Props) {
  const clamp = useCallback((v: number) => Math.max(min, Math.min(max, v)), [min, max]);
  const [dragging, setDragging] = useState(false);
  const dragStartY = useRef(0);
  const dragStartValue = useRef(value);
  const accumulated = useRef(0);

  const commit = useCallback(
    (next: number) => onChange(clamp(next)),
    [onChange, clamp]
  );

  const onWheel = (e: React.WheelEvent) => {
    e.preventDefault();
    // One notch per ~40px of scroll — small trackpad scrolls don't overshoot.
    accumulated.current += e.deltaY;
    const step = Math.trunc(accumulated.current / 40);
    if (step !== 0) {
      accumulated.current -= step * 40;
      commit(value + step);
    }
  };

  useEffect(() => {
    if (!dragging) return;
    function onMove(e: MouseEvent) {
      const dy = e.clientY - dragStartY.current;
      const steps = Math.trunc(dy / ROW_H);
      commit(dragStartValue.current - steps);
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

  const startDrag = (e: React.MouseEvent) => {
    dragStartY.current = e.clientY;
    dragStartValue.current = value;
    setDragging(true);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "ArrowUp") {
      e.preventDefault();
      commit(value + 1);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      commit(value - 1);
    }
  };

  const rows = [value - 1, value, value + 1];

  return (
    <div
      role="spinbutton"
      tabIndex={0}
      aria-valuenow={value}
      aria-valuemin={min}
      aria-valuemax={max}
      onWheel={onWheel}
      onMouseDown={startDrag}
      onKeyDown={onKeyDown}
      style={{ width, height: ROW_H * 3 }}
      className={`relative select-none overflow-hidden rounded-lg border border-sv-border bg-sv-bg outline-none focus:ring-1 focus:ring-sv-accent ${
        dragging ? "cursor-grabbing" : "cursor-grab"
      }`}
    >
      {/* Selection band behind the center row */}
      <div
        className="pointer-events-none absolute inset-x-0 rounded-md bg-sv-surface-2"
        style={{ top: ROW_H, height: ROW_H }}
      />
      {rows.map((v, i) => {
        const inRange = v >= min && v <= max;
        const isCenter = i === 1;
        return (
          <div
            key={i}
            onClick={(e) => {
              // Neighbor rows jump straight to that value; center row does
              // nothing (already selected).
              if (!isCenter && inRange) {
                e.stopPropagation();
                commit(v);
              }
            }}
            style={{ top: i * ROW_H, height: ROW_H }}
            className={`absolute inset-x-0 flex items-center justify-center text-sm tabular-nums transition-opacity ${
              isCenter
                ? "font-semibold text-sv-text"
                : inRange
                ? "cursor-pointer text-sv-muted opacity-60 hover:opacity-90"
                : "opacity-0"
            }`}
          >
            {inRange ? v : ""}
          </div>
        );
      })}
    </div>
  );
}
