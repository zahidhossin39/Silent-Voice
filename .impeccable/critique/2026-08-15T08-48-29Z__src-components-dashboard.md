---
target: dashboard UI/UX
total_score: 30
max_score: 40
na_heuristics: 
p0_count: 0
p1_count: 3
timestamp: 2026-08-15T08-48-29Z
slug: src-components-dashboard
---
# Critique — Silent Voice dashboard (Operate mode)

Method: dual-agent (A: unanchored design review · B: detector + DOM evidence)

## Design Health Score

| # | Heuristic | Score | Key Issue |
|---|-----------|-------|-----------|
| 1 | Visibility of System Status | 4 | Status card, pill states, per-download %/indeterminate bars, "Ready — hold X" reward all excellent |
| 2 | Match System / Real World | 3 | Jargon leaks: quant labels (Q4_K_M), "Staff Pick", `wer`; Guide names a "Dictation" category that doesn't exist |
| 3 | User Control and Freedom | 3 | Esc closes editors/sheets, pause/cancel downloads; no undo on delete-mode, no duplicate-mode |
| 4 | Consistency and Standards | 2 | Staff-pick rows show metric bars, HF rows show an empty spacer; H1→H3 heading skip on 2 tabs; two parallel theme systems |
| 5 | Error Prevention | 3 | ConfirmDialog on destructive acts, ≥800MB download warning, hotkey-conflict catch, Save disabled until named |
| 6 | Recognition Rather Than Recall | 3 | Fit dots + RAM inline everywhere; but the toggle the Guide sends you to is under a different category name |
| 7 | Flexibility and Efficiency | 3 | Quick controls mirror Settings, pins, per-app profiles; no keyboard nav between tabs, no bulk/duplicate |
| 8 | Aesthetic and Minimalist | 3 | Clean token system; Model Store STT tab stacks ~6 controls before the first model |
| 9 | Error Recovery | 3 | Colored dismissible banners, in-place retranscribe errors, humane "No speech detected" |
| 10 | Help and Documentation | 3 | Strong Guide + InfoTips + inline hints; undercut by the stale category reference and hover-only tooltips |
| **Total** | | **30/40** | **Good** |

## Design Specificity Verdict

**Authored, not interchangeable — with pockets of generic dashboard chrome.** The product is organized around the one question local-AI users actually have — "will this run on MY machine" — and it answers it everywhere: green/warn/bad fit dots + RAM estimates on Home, every model row, the quant picker, and the Modes dropdown, all computed from real detected VRAM. The dictation vocabulary is voice-native ("Hold to speak, release to drop the text at your cursor"), and the Settings serif-italic section descriptions are an ownable editorial signature.

Where it slides generic: the Model Store rows are a fintech-style data table, and the HF trending rows are visibly less-authored than the staff picks (empty metric column).

**Deterministic scan:** detector returned exactly **1 finding** — `side-tab` (thick colored left border) at `Settings.tsx:960`. This is the intentional masthead accent bar on section headers using semantic per-category color tokens — a real structural match but a deliberate, low-severity choice, not slop.

## Overall Impression

A genuinely well-built Operate-mode utility that scores its lowest on **consistency**: the same information (model metrics, headings, theme scope, focus states) is presented one way in one place and another way — or not at all — a few pixels over. Nothing is broken enough to block a task, but the seams show. Biggest single opportunity: make the second-class surfaces (HF rows, keyboard focus, LLM/TTS captions) match the first-class ones the team already proved they can build.

## What's Working

1. **Hardware-aware fit signaling is the product's spine, and it's consistent.** The same fit dot + RAM estimate at every decision point answers the one question that matters for local AI, from real detected specs — the difference between a voice app and a generic model browser.
2. **The overlay pill's state system is excellent.** GPU-composited CSS bars encoding recording/processing/TTS by color *and* motion, with a `prefers-reduced-motion` path that parks bars at *distinguishable* static states instead of killing the distinction — care most teams skip.
3. **Failure-recovery honesty as a theme.** AI failure → raw text, cloud-STT → local fallback, "No speech detected" instead of empty, diagnostics-copy button. The app consistently says what breaks and what happens next.

## Priority Issues

**[P1] Keyboard focus is invisible on the primary sidebar nav.**
- Why it matters: computed `outline: none` with no `focus-visible` class on the nav buttons (B, confirmed in DOM) — keyboard and screen-magnifier users get zero visible focus on the app's main navigation. Compounded by History's blur-until-hover cards, which use `group-hover:blur-none` only (no `focus-within`), so keyboard users see permanently blurred transcripts they can't reveal. This is modality exclusion, not polish.
- Fix: add `focus-visible:ring-2 ring-sv-accent` to nav items (the pattern already exists on TranscriptCard's IconButton); add a `group-focus-within:blur-none` variant to History cards.

**[P1] HF trending rows are second-class citizens vs. staff picks.**
- Why it matters: `SttRow`/`LlmRow` render accuracy/speed `MetricBar`s; `HfRow` renders an empty `w-[300px]` spacer (`HfBrowser.tsx:841`) and no metrics. You can't compare a trending model against a staff pick on the axes the store exists to compare — and the dead column reads as a rendering bug.
- Fix: populate HfRow with param-derived quality/speed estimates (you already compute `estimateFitFromParams`/`params_b`), or drop the empty column and share one honest layout.

**[P1] The Guide points to a Settings category that doesn't exist.**
- Why it matters: `Guide.tsx:93` says "Settings → **Dictation** → Inline proofreading" — there is no Dictation category; the toggle lives under **Grammar**. A user following documented steps for a privacy-relevant toggle hits a dead end. (Related: the Grammar section's CoEdIT help text still says the model "isn't used on your pasted text" — now false after this session's wiring.)
- Fix: correct to "Settings → Grammar" and sweep the Guide for other stale category names + the CoEdIT copy.

**[P2] Real contrast failures on placeholder + preview text.**
- Why it matters: Home "None selected" voice-model placeholder is hardcoded `#6d6d6d` = **3.73:1** (fails AA); Theme light-mode preview swatches measure **2.72** and **3.07**; pin stars/defaults render at `sv-muted/40`, well under 3:1. The main `sv-muted` token itself passes (5.2–6.3) — these are one-off hardcoded grays and low-opacity interactive marks, not the system.
- Fix: replace hardcoded grays with the `sv-muted` token; floor interactive icon opacity at full `sv-muted`, reserve /40 for decoration only.

**[P2] Two "theme" systems on one page with no scope disambiguation.**
- Why it matters: Theme.tsx sets both the whole-app accent and a separate Grammar-popup accent (5 palettes × 2 layouts); both are named by color words, nothing signals they're independent scopes, so users set one expecting the other.
- Fix: a scope chip per section ("Affects: whole app" / "Affects: the grammar popup only"), or nest the popup controls in a bordered card that visibly owns them.

**[P3] Model Store STT tab exceeds the working-memory budget on open.**
- Why it matters: ~6 filter/sort controls (track tabs, category, language, search, sort, show-incompatible) + 2 section headers before the first model — choice paralysis at the exact step a first-timer just wants "the recommended one."
- Fix: collapse filters behind one disclosure; lead with the hardware-recommended staff pick pre-selected.

## Persona Red Flags

**Alex (power user):**
- Can't compare trending HF models by accuracy/speed (no bars on HfRow) — must expand each and eyeball params.
- No duplicate-mode action; iterating on prompts means rebuilding or destroying the original.
- Quant picker gives no quality guidance beyond size+fit — the actual selection criterion (Q4_K_M vs Q5_K_M) is unstated.
- No keyboard path between dashboard tabs; everything is mouse-to-sidebar.

**Sam (accessibility / keyboard):**
- No visible focus on primary nav (see P1).
- Blur-until-hover History can't be revealed without a mouse (see P1).
- LLM/TTS metric bars have no resting caption and no `role="meter"`/`aria-valuenow`; values hide behind mouse-only hover-expand.
- InfoTip tooltips are hover-gated; keyboard `onClick` toggle closes on `onBlur`, so tab-then-read is fragile.
- Two tabs skip H1→H2 (Model Store, Writing styles), breaking screen-reader outline.
- Credit: the TranscriptCard audio scrubber (`role="slider"`, full keyboard, `aria-valuetext`) and the ModelPicker listbox are exemplary — the gaps are inconsistency, not incapacity.

## Minor Observations

- No page-level horizontal scroll at 1280/900/375 (B verified) — responsive containment is solid.
- `mode_id` slug leaks to users in History (`· clean_up`).
- Onboarding first-dictation failure ("try again, a little louder") offers no link to mic settings for a genuinely dead device.
- `EMPTY` mode hardcodes `model_id: "llama-3.2-1b-instruct-q4"` — a new mode may reference an un-downloaded model.
- Emoji glyphs as controls (`❚❚ ■ ▶ ✕`) render inconsistently vs. the clean SVG icon set.
- `sv-on-accent` (#1a1200 on orange) is a genuinely good, documented contrast decision.

## Questions to Consider

1. If the product is organized around "will this run on my machine," why wade through filters and a trending feed before seeing *the one model you'd pick for my exact hardware*? Where's the "just use the best one for me" button?
2. Do users ever knowingly want the app accent and grammar-popup accent *different*, or does the second system exist because it was buildable rather than needed?
3. Staff-pick rows get metric bars and HF rows get nothing — are you quietly telling users "only trust the curated list"? Then why surface trending at all?
4. Blur-until-hover protects a shoulder-surfed screen but breaks for keyboard users — is "privacy for mouse users only" a real privacy feature?
