# Sync Type Chart Selection Across Windows

**Goal:** Keep Companion windows on the same saved per-ROM chart after one window changes it.

**Architecture:** Listen for same-origin `storage` events scoped to the active ROM's stable `storage_id`. Reload and validate the persisted dataset ID, reset the stale calculated matchup, and fall back to the base chart when the saved value is invalid or removed.

**Files:** `desktop-rust/ui/src/app.jsx`, `desktop-rust/ui/test/ui.test.mjs`, `docs/VYNTRA_PLAN_TECNICO.md`.

## Tasks

- [x] Add a UI assertion that fails when a matching storage event does not refresh the active chart.
- [x] Add a scoped storage listener and clear the previous calculated result.
- [x] Run all UI tests with two pages in one browser context; verify the secondary page updates the primary through a same-origin storage event.
