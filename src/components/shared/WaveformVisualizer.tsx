// Lightweight animated waveform used in the recording overlay and Home status.
// `barClass` overrides the bar color — the overlay pill uses the theme text
// color so the red recording dot stands out (red bars-adjacent-to-orange
// clashed); Home keeps the default orange accent.
export default function WaveformVisualizer({
  active,
  bars = 5,
  barClass = "bg-sv-accent",
}: {
  active: boolean;
  bars?: number;
  barClass?: string;
}) {
  return (
    <div className="flex h-5 items-center gap-[3px]">
      {Array.from({ length: bars }).map((_, i) => (
        <span
          key={i}
          className={`w-[3px] rounded-full ${barClass} ${
            active ? "sv-bar" : ""
          }`}
          style={{
            height: active ? "100%" : "30%",
            animationDelay: `${i * 0.12}s`,
          }}
        />
      ))}
    </div>
  );
}
