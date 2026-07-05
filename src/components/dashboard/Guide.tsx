import Page from "../shared/Page";
import { useSettingsStore } from "../../stores/settingsStore";

export default function Guide() {
  const hotkey = useSettingsStore((s) => s.settings.hotkey);

  return (
    <Page
      title="How to use Silent Voice"
      subtitle="Master local-first dictation in minutes."
    >
      <div className="max-w-[1400px] w-full space-y-8 pb-12 animate-in fade-in duration-500">
        
        {/* HERO SECTION / QUICK START */}
        <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
          <div className="lg:col-span-2">
            <Card title="Quick Start" className="h-full bg-gradient-to-br from-sv-surface to-sv-bg border-sv-border/80">
               <div className="flex flex-col sm:flex-row items-center justify-between gap-8 py-2">
                  <div className="flex-1 space-y-6">
                    <Step n={1} title="Click any input">
                      Focus on any text field (chat, code, email).
                    </Step>
                    <Step n={2} title={<>Hold <Kbd>{hotkey}</Kbd> and speak</>}>
                      The pill turns orange. Speak naturally.
                    </Step>
                    <Step n={3} title="Release to paste">
                      Your words are instantly typed at the cursor.
                    </Step>
                  </div>
                  <div className="w-full sm:w-auto flex justify-center mt-6 sm:mt-0">
                    <HeroVisual />
                  </div>
               </div>
            </Card>
          </div>

          <div className="lg:col-span-1">
            <Card title="The Floating Pill" className="h-full">
              <p className="mb-6 text-sm text-sv-muted">
                Your always-on-top companion. Drag to move, right-click for options.
              </p>
              <div className="flex flex-col gap-4">
                <PillRow label="Idle — waiting" active={false}>
                  <span className="h-[2px] w-4 rounded-full bg-sv-muted" />
                </PillRow>
                <PillRow label="Recording" active={true}>
                  <MiniBars className="bg-sv-accent" />
                </PillRow>
                <PillRow label="Processing" active={true}>
                  <MiniBars className="bg-sv-muted" />
                </PillRow>
              </div>
            </Card>
          </div>
        </div>

        {/* FEATURES GRID */}
        <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-6">
          
          <Card title="Hands-free Dictation" className="flex flex-col">
            <div className="flex-1">
              <div className="mb-4 flex justify-center py-6 bg-sv-surface-2/20 rounded-xl border border-sv-border/30">
                <KeyboardDoubleTapVisual hotkey={hotkey} />
              </div>
              <p className="text-sm text-sv-muted">
                Tap <Kbd>{hotkey}</Kbd> <strong className="text-sv-text">twice quickly</strong> to lock recording on. Let go of the keyboard and speak freely. Press once more to stop and paste.
              </p>
            </div>
          </Card>

          <Card title="AI Rewrite Modes" className="flex flex-col">
             <div className="flex-1">
               <div className="mb-4 flex justify-center py-6 bg-sv-surface-2/20 rounded-xl border border-sv-border/30">
                 <AIModeVisual />
               </div>
               <p className="text-sm text-sv-muted">
                 In the <strong className="text-sv-text">Modes</strong> tab, have AI instantly format your speech. Turn raw thoughts into formal emails, bullet points, or clean code comments.
               </p>
             </div>
          </Card>

          <Card title="Choosing a Model" className="flex flex-col">
             <div className="flex-1">
               <div className="mb-4 flex justify-center py-6 bg-sv-surface-2/20 rounded-xl border border-sv-border/30">
                 <ModelScaleVisual />
               </div>
               <p className="text-sm text-sv-muted mb-4">
                 Without a dedicated GPU, <strong className="text-sv-text">smaller is faster</strong>.
               </p>
               <ul className="space-y-3 text-sm">
                 <DotRow color="bg-sv-good" text="Recommended for your PC" />
                 <DotRow color="bg-sv-warn" text="Works, but might lag" />
                 <DotRow color="bg-sv-bad" text="Too heavy (needs better GPU)" />
               </ul>
             </div>
          </Card>
        </div>

        {/* POWER FEATURES */}
        <Card title="Power Features (Settings)">
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4 pt-2">
             <FeatureItem title="Custom vocabulary" icon={<BookIcon />}>
               Teach the model names and jargon ("n8n", "Kubernetes") so it never misspells them.
             </FeatureItem>
             <FeatureItem title="Text replacements" icon={<ReplaceIcon />}>
               Say a short trigger to paste full text. E.g. "my email" → you@example.com.
             </FeatureItem>
             <FeatureItem title="Per-app profiles" icon={<AppIcon />}>
               Auto-switch AI modes based on the active window. Formal in Outlook, Raw in VS Code.
             </FeatureItem>
             <FeatureItem title="Smart Numbers" icon={<NumberIcon />}>
               Spoken numbers turn into digits: "twenty five" → 25, "twenty twenty six" → 2026.
             </FeatureItem>
             <FeatureItem title="Corrections that teach" icon={<BrainIcon />}>
               Fix mistakes in the History tab. New words are added to your vocabulary automatically.
             </FeatureItem>
             <FeatureItem title="Input sensitivity" icon={<SliderIcon />}>
               Adjust the slider to filter out background noise (wind, keyboard clicks, fans).
             </FeatureItem>
          </div>
        </Card>

        {/* TROUBLESHOOTING */}
        <Card title="Troubleshooting">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            <TroubleItem q="Nothing gets pasted?">
              Check that your model finished downloading and the correct microphone is selected in Settings.
            </TroubleItem>
            <TroubleItem q="Dashboard closed?">
              The app keeps running in the tray. Click the tray icon (near the clock) to bring it back.
            </TroubleItem>
            <TroubleItem q="Cloud STT errors?">
              Use "Test connection" in API Keys. If cloud fails mid-dictation, it falls back to your local model.
            </TroubleItem>
            <TroubleItem q="Something else broke?">
              Errors are logged to <code className="rounded bg-sv-surface-2 px-1.5 py-0.5 text-xs text-sv-accent">%APPDATA%\SilentVoice\logs</code>.
            </TroubleItem>
          </div>
        </Card>
        
      </div>
    </Page>
  );
}

/* ── UI Components ─────────────────────────────────────────── */

function Card({ title, className = "", children }: { title: string; className?: string; children: React.ReactNode }) {
  return (
    <section className={`rounded-2xl border border-sv-border bg-sv-surface p-6 shadow-sm hover:border-sv-border/80 transition-colors ${className}`}>
      <h2 className="mb-5 text-lg font-semibold text-sv-text">{title}</h2>
      {children}
    </section>
  );
}

function Step({ n, title, children }: { n: number; title: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="flex gap-4 items-start">
      <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-sv-accent/10 text-sm font-bold text-sv-accent ring-1 ring-sv-accent/30 shadow-[0_0_12px_rgba(249,115,22,0.15)]">
        {n}
      </div>
      <div>
        <div className="text-sm font-semibold text-sv-text">{title}</div>
        <div className="mt-1 text-sm text-sv-muted leading-relaxed">{children}</div>
      </div>
    </div>
  );
}

function Kbd({ children }: { children: React.ReactNode }) {
  return (
    <kbd className="rounded-md border border-sv-border border-b-[3px] bg-sv-surface-2 px-2 py-0.5 text-xs font-mono font-bold text-sv-text shadow-sm mx-1">
      {children}
    </kbd>
  );
}

function PillRow({ label, active, children }: { label: string; active: boolean; children: React.ReactNode }) {
  return (
    <div className={`flex items-center justify-between p-3.5 rounded-xl border transition-colors ${active ? 'border-sv-accent/20 bg-sv-accent/5' : 'border-sv-border/50 bg-sv-surface-2/30'}`}>
      <span className="text-sm font-medium text-sv-text">{label}</span>
      <div className="flex h-7 w-20 items-center justify-center gap-1 rounded-full border border-sv-border bg-[#0e1116] shadow-inner">
        {children}
      </div>
    </div>
  );
}

function DotRow({ color, text }: { color: string; text: string }) {
  return (
    <li className="flex items-center gap-3 p-2.5 rounded-lg bg-sv-surface-2/30 border border-sv-border/30">
      <span className={`h-2.5 w-2.5 shrink-0 rounded-full ${color} shadow-[0_0_8px_rgba(0,0,0,0.5)] shadow-${color}/50`} />
      <span className="text-sm text-sv-text">{text}</span>
    </li>
  );
}

function MiniBars({ className }: { className: string }) {
  const heights = [6, 10, 13, 9, 5];
  return (
    <div className="flex items-center gap-[2px]">
      {heights.map((h, i) => (
        <span
          key={i}
          className={`w-[2px] rounded-full ${className}`}
          style={{ height: `${h}px` }}
        />
      ))}
    </div>
  );
}

function FeatureItem({ title, icon, children }: { title: string; icon: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="flex gap-4 p-4 rounded-xl border border-transparent hover:border-sv-border/50 hover:bg-sv-surface-2/30 transition-all cursor-default">
      <div className="mt-1 shrink-0 text-sv-accent/90 flex h-9 w-9 items-center justify-center rounded-lg bg-sv-accent/10 ring-1 ring-sv-accent/20 shadow-sm">
        {icon}
      </div>
      <div>
        <h3 className="text-sm font-semibold text-sv-text mb-1.5">{title}</h3>
        <p className="text-xs text-sv-muted leading-relaxed">{children}</p>
      </div>
    </div>
  );
}

function TroubleItem({ q, children }: { q: string; children: React.ReactNode }) {
  return (
    <div className="p-5 rounded-xl border border-sv-border/50 bg-sv-surface-2/20 relative overflow-hidden">
      <div className="absolute top-0 left-0 w-1 h-full bg-sv-warn/60"></div>
      <h4 className="text-sm font-semibold text-sv-text mb-2 flex items-center gap-2">
        <span className="text-sv-warn font-bold">?</span> {q}
      </h4>
      <p className="text-xs text-sv-muted leading-relaxed">{children}</p>
    </div>
  );
}

/* ── SVG Visualizations ────────────────────────────────────── */

function HeroVisual() {
  return (
    <svg width="240" height="160" viewBox="0 0 240 160" fill="none" xmlns="http://www.w3.org/2000/svg" className="drop-shadow-2xl">
      {/* Background Window */}
      <rect x="20" y="20" width="200" height="120" rx="8" fill="#131722" stroke="#262c3d" strokeWidth="2"/>
      <rect x="20" y="20" width="200" height="24" rx="8" fill="#1b2030" stroke="#262c3d" strokeWidth="1" />
      <circle cx="36" cy="32" r="4" fill="#ef4444" />
      <circle cx="52" cy="32" r="4" fill="#eab308" />
      <circle cx="68" cy="32" r="4" fill="#22c55e" />
      
      {/* Editor Content */}
      <rect x="40" y="64" width="80" height="6" rx="3" fill="#262c3d" />
      <rect x="40" y="84" width="120" height="6" rx="3" fill="#262c3d" />
      <rect x="40" y="104" width="140" height="6" rx="3" fill="#f97316" fillOpacity="0.8" />
      
      {/* Voice Wave */}
      <path d="M170 84 Q 185 64 200 84 T 230 84" stroke="#f97316" strokeWidth="3" strokeLinecap="round" fill="none" className="animate-[pulse_1.5s_ease-in-out_infinite]" />
      
      {/* Cursor */}
      <rect x="185" y="102" width="2" height="10" fill="#f97316" className="animate-[pulse_1s_ease-in-out_infinite]" />
    </svg>
  );
}

function KeyboardDoubleTapVisual({ hotkey }: { hotkey: string }) {
  return (
    <div className="relative flex items-center justify-center h-24 w-full">
       <svg width="120" height="80" viewBox="0 0 120 80" fill="none" xmlns="http://www.w3.org/2000/svg">
         {/* Key */}
         <rect x="30" y="20" width="60" height="40" rx="6" fill="#1b2030" stroke="#262c3d" strokeWidth="3"/>
         <rect x="30" y="50" width="60" height="10" rx="2" fill="#0b0e14" opacity="0.5"/>
         <text x="60" y="45" fill="#e6e9f0" fontSize="14" fontWeight="bold" fontFamily="monospace" textAnchor="middle">{hotkey}</text>
         
         {/* Tap Indicators */}
         <circle cx="60" cy="40" r="25" stroke="#f97316" strokeWidth="2" strokeDasharray="4 4" fill="none" className="animate-[ping_1.5s_ease-in-out_infinite]" />
         <circle cx="60" cy="40" r="15" stroke="#f97316" strokeWidth="2" fill="none" className="animate-[ping_1.5s_ease-in-out_infinite_0.3s]" />
       </svg>
    </div>
  );
}

function AIModeVisual() {
  return (
    <svg width="200" height="100" viewBox="0 0 200 100" fill="none" xmlns="http://www.w3.org/2000/svg">
      {/* Raw Bubble */}
      <rect x="10" y="20" width="70" height="40" rx="8" fill="#1b2030" stroke="#262c3d" strokeWidth="2" />
      <path d="M20 35 H60 M20 45 H45" stroke="#5c6473" strokeWidth="3" strokeLinecap="round" />
      
      {/* Arrow / AI Process */}
      <path d="M90 40 L110 40 M105 35 L112 40 L105 45" stroke="#f97316" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
      <circle cx="100" cy="40" r="16" fill="#f97316" fillOpacity="0.15" className="animate-[pulse_2s_ease-in-out_infinite]" />
      
      {/* Polished Bubble */}
      <rect x="120" y="20" width="70" height="40" rx="8" fill="#f97316" fillOpacity="0.05" stroke="#f97316" strokeWidth="2" strokeOpacity="0.8" />
      <path d="M130 35 H170 M130 45 H160" stroke="#f97316" strokeWidth="3" strokeLinecap="round" />
      
      <text x="45" y="78" fill="#8b93a7" fontSize="10" textAnchor="middle" fontWeight="500">Raw Speech</text>
      <text x="155" y="78" fill="#f97316" fontSize="10" textAnchor="middle" fontWeight="500">Polished Text</text>
    </svg>
  );
}

function ModelScaleVisual() {
  return (
    <svg width="200" height="100" viewBox="0 0 200 100" fill="none" xmlns="http://www.w3.org/2000/svg">
      <path d="M20 80 L180 80" stroke="#262c3d" strokeWidth="2" strokeLinecap="round" />
      <path d="M20 80 L20 20" stroke="#262c3d" strokeWidth="2" strokeLinecap="round" />
      
      {/* Bars */}
      <rect x="40" y="60" width="30" height="20" rx="4" fill="#22c55e" />
      <rect x="90" y="40" width="30" height="40" rx="4" fill="#eab308" />
      <rect x="140" y="20" width="30" height="60" rx="4" fill="#ef4444" />
      
      {/* Labels */}
      <text x="55" y="95" fill="#8b93a7" fontSize="10" textAnchor="middle" fontWeight="500">Tiny</text>
      <text x="105" y="95" fill="#8b93a7" fontSize="10" textAnchor="middle" fontWeight="500">Base</text>
      <text x="155" y="95" fill="#8b93a7" fontSize="10" textAnchor="middle" fontWeight="500">Large</text>
      
      {/* Speed Line */}
      <path d="M55 50 L105 30 L155 10" stroke="#f97316" strokeWidth="2" strokeLinecap="round" strokeDasharray="4 4" className="animate-[pulse_2s_ease-in-out_infinite]" />
      <circle cx="55" cy="50" r="4" fill="#f97316" />
      <circle cx="105" cy="30" r="4" fill="#f97316" />
      <circle cx="155" cy="10" r="4" fill="#f97316" />
    </svg>
  );
}

/* ── Icons ─────────────────────────────────────────────────── */
function BookIcon() {
  return <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M2 3h6a4 4 0 0 1 4 4v14a3 3 0 0 0-3-3H2z"/><path d="M22 3h-6a4 4 0 0 0-4 4v14a3 3 0 0 1 3-3h7z"/></svg>;
}
function ReplaceIcon() {
  return <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="m16 3 4 4-4 4"/><path d="M20 7H4"/><path d="m8 21-4-4 4-4"/><path d="M4 17h16"/></svg>;
}
function AppIcon() {
  return <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><rect x="2" y="4" width="20" height="16" rx="2"/><path d="M10 4v4"/><path d="M2 8h20"/><path d="M6 4v4"/></svg>;
}
function NumberIcon() {
  return <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><line x1="4" x2="20" y1="9" y2="9"/><line x1="4" x2="20" y1="15" y2="15"/><line x1="10" x2="8" y1="3" y2="21"/><line x1="16" x2="14" y1="3" y2="21"/></svg>;
}
function BrainIcon() {
  return <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><path d="M21.42 10.922a2 2 0 0 0-.019-3.838L12.83 4.3a2 2 0 0 0-1.66 0L2.6 7.08a2 2 0 0 0 0 3.832l8.57 2.776a2 2 0 0 0 1.66 0z"/><path d="M22 10v6"/><path d="M6 12.5V16a6 3 0 0 0 12 0v-3.5"/></svg>;
}
function SliderIcon() {
  return <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round"><line x1="21" x2="14" y1="4" y2="4"/><line x1="10" x2="3" y1="4" y2="4"/><line x1="21" x2="12" y1="12" y2="12"/><line x1="8" x2="3" y1="12" y2="12"/><line x1="21" x2="16" y1="20" y2="20"/><line x1="12" x2="3" y1="20" y2="20"/><line x1="14" x2="14" y1="2" y2="6"/><line x1="8" x2="8" y1="10" y2="14"/><line x1="16" x2="16" y1="18" y2="22"/></svg>;
}
