# Library ROM Relink Implementation Plan

**Goal:** Restore a missing library entry only when a selected file has the same verified ROM content hash.

**Architecture:** Keep the stable library ID and user metadata, update only the source path after SHA-256 equality, and persist through the existing atomic catalog writer. The UI requests a candidate file and reports mismatch errors through the existing command channel.

**Tech Stack:** Existing Rust catalog, SHA-256 helper, Wry file picker and Preact UI.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, T12 WIN-LIB-02.

## Tasks

1. Test exact-hash relink and mismatch preservation; verify RED.
2. Implement relink by stable catalog ID and run focused tests.
3. Add a missing-ROM UI action and publish the refreshed catalog after a successful relink.
4. Run Rust workspace and UI checks; update plan/status.
5. Commit and push the checkpoint.
