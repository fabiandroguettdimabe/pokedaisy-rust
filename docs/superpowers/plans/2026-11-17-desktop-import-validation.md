# Desktop ROM Import Validation Plan

**Goal:** Make multi-file ROM imports reject invalid files and report accepted, duplicate, and rejected counts.

**Architecture:** Add a pure import coordinator over the existing library and bounded header inspector. Keep original file locations; do not extract or copy. Return a compact report through the existing command acknowledgement.

**Tech Stack:** Existing Rust library schema, structural ROM inspection, Wry command acknowledgements.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, T11 WIN-LIB-03 (partial intake foundation).

## Tasks

1. Test accepted valid ROM, invalid ROM rejection, and repeated/canonical path handling; verify RED.
2. Implement import coordinator and run focused tests.
3. Return report counts to the existing UI acknowledgement and preserve its multi-file picker.
4. Run Rust workspace formatting, tests, and check; document remaining ZIP/progress/cancel work.
5. Commit and push the verified checkpoint.
