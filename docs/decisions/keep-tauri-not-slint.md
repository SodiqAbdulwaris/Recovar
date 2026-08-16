# Decision: keep Tauri, do not migrate the frontend to Slint

## Question

Would rewriting `recovar-gui` from Tauri (Rust backend + React/TypeScript webview frontend)
to Slint (a Rust-native declarative UI toolkit with no webview) be a good idea?

## Recommendation

No. Keep Tauri. The reasons below are specific to this project's actual state, not a general
argument against Slint.

## Reasoning

**What would actually change.** Slint replaces the webview and the HTML/CSS/TypeScript
frontend with a `.slint` markup language compiled to native Rust, rendered without a webview
runtime. `recovar-core` and `recovar-cli` are unaffected either way; only `recovar-gui` is in
scope.

**Arguments for Slint, taken seriously:**
- No webview dependency: on Linux, Tauri pulls in webkit2gtk, which is a real, sometimes
  fragile system dependency across distros. Slint has none of that.
- Smaller binary and lower idle memory, since there's no embedded browser engine.
- Everything in one language (Rust) instead of Rust + TypeScript + CSS.

**Why it's not worth it here:**
- **The actual problem this session was asked to solve — cross-platform disk access — has
  nothing to do with the UI framework.** It lived entirely in `recovar-core`'s `disk` module
  and has been fixed there (see `docs/architecture/cross-platform-disk-backend.md`). Neither
  Tauri nor Slint changes what's hard about this app: reading raw block devices safely with
  elevated privileges.
- **The GUI is small and just got substantially reworked this session** (see
  `docs/ui/redesign-salvage-console.md`): a full visual redesign, confidence-grouped triage,
  a responsive layout down to 280px, and the state/IPC wiring for start/stop/recover. A
  framework migration would mean re-doing all of that in Slint's markup language and its very
  different styling model, for a webview dependency that is not currently causing any real
  problem in this codebase.
- **Slint's web tooling is far less mature than React's.** This project already ships a
  working responsive layout (CSS Grid, media queries, `display: contents`) that a design
  agency's worth of prior art exists for. Slint's `.slint` layout language can do responsive
  layouts, but the ecosystem, examples, and debugging tools for it are a fraction of what
  exists for CSS.
- **Team/maintenance reality.** A solo/small-team Rust project gets more mileage from widely
  known web tech (HTML/CSS/React) that any contributor can pick up, versus a DSL specific to
  one UI framework.
- The core engineering principle for this whole engagement was: fix root causes, don't
  rewrite working software for the sake of rewriting it. The GUI works, builds, and now looks
  intentional. There is no concrete problem here that a framework swap would solve.

## When Slint would actually make sense

If a future requirement genuinely needs it — for example, targeting a headless embedded
Linux device with no webview available, or a hard requirement to eliminate the webkit2gtk
dependency for a specific deployment target — that would be a real, scoped reason to
reconsider. That is not the situation today.

## Notes

This is advice, not a decision to act on. No code changes were made as a result of this
question.
