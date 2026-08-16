# UI: removed purely decorative animations

## Old problem

The GUI's stylesheet (`recovar-gui/src/index.css`) included three animations that carried no
information and only ran for their own sake, matching the "unnecessary animation" and
"attention-seeking effect" anti-patterns called out for this review:

- A shimmer sweep on the progress bar (`.progress-fill.active::after`), redundant with the
  percentage text and phase label already shown next to it.
- A perpetual up/down float on the empty-state emoji icon, running even while idle.
- A full-width "scan wave" overlay that swept across the entire window on a 2.5s loop while
  scanning, on top of an already-present progress bar.

## What changed

Removed all three `@keyframes` blocks and their usages: `shimmer`, `float`, and `scanWave`,
along with the `.scan-wave` div in `App.tsx` and the `active` shimmer/`float` rules in
`index.css`.

## Why the new version is better

None of the three carried state information beyond what the progress bar, phase text, and
status dot already show. Removing constant motion also reduces visual noise and CPU/GPU
churn while a scan is running, when the UI should be easy to scan at a glance.

State-communicating animations were kept: the `fade-in` transition when new UI sections
appear, the status-bar dot pulse while scanning, and the progress bar's width transition.
These convey a state change, not decoration.

## Design principles referenced

Restrained visual language and no motion without function, consistent with the "minimal
decoration" and "no unnecessary animations" guidance for this pass. Not based on copying the
supplied mockups directly.

## Components changed

- `recovar-gui/src/index.css`
- `recovar-gui/src/App.tsx`

## Behavior changed

None. These were visual-only; no state, data flow, or command wiring changed.

## Verification

`npm run build` (tsc + vite) completes cleanly. Ran the frontend standalone via `vite dev` in
a browser preview (Tauri IPC calls are unavailable outside the Tauri shell, so drive
listing/scanning could not be exercised there); the layout renders correctly and the browser
console shows no errors. Full IPC-connected verification requires running the actual
`recovar-gui.exe` binary, which was built successfully but not interactively driven in this
session (see `docs/testing/verify-cli-scan-and-list.md` for what was exercised).
