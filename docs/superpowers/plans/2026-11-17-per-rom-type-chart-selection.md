# Per-ROM Type Chart Selection

**Goal:** Restore the type chart manually selected for each ROM in Companion.

**Architecture:** Store only the chart dataset ID in browser local storage under the ROM's stable `storage_id`. Validate the saved ID against the bundled reference manifest when loading; invalid or unavailable values fall back to the Gen III base chart. Header identity remains advisory and does not auto-select a chart.

**Tech Stack:** Preact, browser local storage, Node test runner, Playwright UI test.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, T14 and T15.

## Global Constraints

- Keep chart selection editable and scoped to a stable ROM identity.
- Do not infer a retail chart from partial header identity.
- Invalid saved profiles must not break Companion.

## Review Focus

- Unknown dataset IDs fall back to the base chart.
- Different ROM identities keep separate choices.
- Unavailable local storage leaves current-session selection usable.

## Files

- Modify `desktop-rust/ui/src/dex.js` for storage helpers.
- Modify `desktop-rust/ui/src/app.jsx` to load and save the selected chart.
- Modify `desktop-rust/ui/test/dex.test.mjs` and `ui.test.mjs` for isolation and restore behavior.
- Modify `docs/VYNTRA_PLAN_TECNICO.md` to record the delivered behavior and remaining strict-identity blocker.

## Tasks

- [x] Add a failing unit test for per-ROM storage and invalid IDs.
- [x] Implement guarded load/save helpers.
- [x] Load the saved selection when the selected ROM changes and save manual/Dex-driven selection.
- [x] Verify switching ROMs restores each selection in the UI test.
- [x] Build the UI and run the complete UI test suite.
