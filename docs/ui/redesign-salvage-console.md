# UI: Salvage Console redesign

## Old problem

The previous interface was a fairly generic dark "glassmorphism" dashboard: cool slate-blue
background, a blue-to-violet gradient accent, glow shadows, rounded pill badges, and three
purely decorative animations (removed in an earlier session pass; see
`docs/ui/remove-decorative-animations.md`). It did not read as a purpose-built forensic
recovery tool, and it never told the user the single most important safety fact about data
recovery: stop writing to the target now.

It was also not responsive at all — a fixed two-column layout with a 900px minimum window
width, unusable on anything narrower than a laptop.

## What changed

A full visual and structural redesign, implemented directly in `recovar-gui/src/App.tsx` and
`recovar-gui/src/index.css` (a mockup was shown to the user first as a published Artifact and
approved before implementation):

- **Palette**: warm graphite ground (`#14120E`) instead of cool slate-blue, with a muted
  signal-teal accent (`#4CBBAE`) evoking a scanner/sonar readout instead of the default
  blue-violet SaaS gradient. Semantic colors (success green, warning amber, danger red) are
  kept distinct from the accent hue.
- **Typography**: a serif wordmark (`Iowan Old Style`/Georgia stack) for the app name —
  report-like authority, since this data is treated as evidence — monospace for every data
  value (filenames, offsets, sizes, confidence percentages, drive paths), and system sans for
  controls and labels.
- **Persistent safety banner**: "Reading only. Stop saving or installing anything to
  {target} — every new write can permanently erase a file still waiting to be recovered."
  This did not exist anywhere in the previous UI, despite being the single most important
  thing a user in this situation needs to know.
- **Confidence-grouped triage**: results are split into "High confidence" (≥85%, recommended)
  and "Needs review" (below 85%, shown with why — e.g. "no footer found" for a carved file
  with no known end) groups instead of one flat table. A "Select all high confidence" bulk
  action handles the common case in one click.
- **Instrument-style depth bar**: scan progress is shown as a ticked bar with real byte
  counts (`586 GB / 931 GB scanned`), not a generic animated gradient pill.
- **Recovered-count accuracy carried over** from the Antigravity-driven fix earlier this
  session: the status bar reports the real number of files written, not the number selected.

## Why the new design is better

The previous look was visually competent but generic — nothing about it signaled "recovery
tool" specifically, and the flat results table gave equal visual weight to a 96%-confidence
photo and a 58%-confidence signature match with no footer, which is exactly backwards for a
tool whose job is to help someone triage under time pressure and risk of further data loss.

## Design principles referenced

Restrained visual language, strong typographic hierarchy, minimal decoration, and a
product-specific rather than templated identity — consistent with the direction requested for
this pass. The mockup was used as inspiration and a starting point, not copied verbatim; the
implementation adapts it to the real state machine (idle/scanning/error/complete) and real
IPC-driven data instead of static mockup content.

## Responsive behavior

Three breakpoints, verified down to the width of a folded Z Fold-class phone:

- **≥901px (desktop/laptop)**: fixed 268px control rail beside the results area, as before.
- **561–900px (tablet/narrow desktop)**: the control rail collapses behind a "Scan setup"
  toggle in the title bar, rendered as a drawer above the results area instead of a
  permanent sidebar, reclaiming width for the data that matters.
- **≤560px (phone)**: each result row switches from a grid table row to a stacked card — the
  filename stays prominent, and size/type/method/confidence wrap together underneath via a
  `.r-meta` element that is `display: contents` on wider screens (letting its children behave
  as ordinary grid columns) and `display: flex; flex-wrap: wrap` at this breakpoint, so no
  extra markup is needed for either layout.
- Verified at 280px logical width (a folded Galaxy Z Fold-class cover screen) with zero
  horizontal overflow, both with the results list populated and with the settings drawer
  open at the same time.
- `recovar-gui/src-tauri/tauri.conf.json`'s window `minWidth`/`minHeight` were lowered from
  900×600 to 320×480 so the desktop window itself can actually be resized down to see this.

## Components changed

- `recovar-gui/src/App.tsx`
- `recovar-gui/src/index.css`
- `recovar-gui/src-tauri/tauri.conf.json` (window minimum size)

## Behavior changed

The confidence threshold for "high confidence" grouping (85%) is a new, visible product
decision — previously confidence was shown per-row with no grouping. Everything else
(scan/stop/recover flows, event handling) is unchanged; only presentation and layout changed.

## Verification

`npm run build` (tsc + vite) succeeds. Verified via a live `vite dev` browser preview:

- No console errors at any tested width.
- Zero horizontal overflow (`document.documentElement.scrollWidth === clientWidth`) at
  1100px, 768px, and 280px viewport widths.
- The settings drawer toggle correctly switches `.rail` between `display: none` and
  `display: flex` at narrow widths, confirmed via computed style inspection.
- The result-row `.r-meta` element's computed `display` switches from the desktop grid
  behavior to `flex` under 560px, confirmed via computed style inspection on a synthetic row.

Full IPC-driven behavior (live scan progress, real results) could not be exercised in a plain
browser preview, since `@tauri-apps/api/core`'s `invoke` has no backend outside the actual
Tauri shell; this is the same limitation noted in earlier UI verification this session.
