# Generation Dex Type Profiles

**Goal:** Add offline National Dex type profiles for Generations IV–IX to the existing chart selector.

**Architecture:** Join the existing National Dex names to pinned PokeAPI CSV snapshots by National Dex number. Use default Pokémon records and historical type rows effective at each generation, translating type names to each chart's local IDs. Attach source hashes, pinned commit, attribution, license, and scope notes to each generated profile.

**Tech Stack:** Python CSV/JSON exporter, PokeAPI static CSV snapshot, Preact offline datasets.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, T15.

## Constraints and Limits

- Profiles cover species introduced through a generation, not a title's regional availability list.
- Default forms only; regional and alternate forms remain outside the profile.
- Do not infer a ROM's chart from header codes.
- Preserve offline operation and existing source licensing metadata.

## Files

- Add pinned source CSVs and license under `scripts/data/pokeapi/`.
- Modify `scripts/export_desktop_reference.py` to parse historical typing and enrich standard profiles.
- Modify `scripts/tests/test_export_desktop_reference.py` to verify history resolution and generation coverage.
- Regenerate `desktop-rust/ui/src/data/type-charts-reference.json` and `desktop-rust/ui/dist/index.html`.
- Modify `desktop-rust/ui/test/ui.test.mjs` and `docs/VYNTRA_PLAN_TECNICO.md`.

## Tasks

- [x] Test historical type selection against synthetic CSV inputs.
- [x] Implement the parser and enrich standard generation profiles.
- [x] Regenerate offline data and test Dex typing for IV, V, VII, VIII, and IX.
- [x] Build UI and run Python/UI test suites plus export check.
