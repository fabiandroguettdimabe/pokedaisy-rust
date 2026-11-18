# T02 Async Action ACKs Implementation Plan

> **For agentic workers:** Follow the steps in order and keep the existing Launcher process and personal data untouched.

**Goal:** Correlate the terminal result of every asynchronous UI action with the originating `command_id` and `request_id`.

**Architecture:** Carry command/request IDs through worker events. At completion, emit the existing versioned `UiAck` terminal response; continue using existing progress and state notifications. A failed core probe returns a structured error ACK, while its detailed probe report remains available in app state.

**Tech Stack:** Rust, serde_json, existing Vyntra UI IPC and cargo tests.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, T02 / INFRA-01.

## Global Constraints

- Keep the envelope version, action payload contracts, and initial accepted ACK behavior unchanged.
- Preserve existing import progress/toast and core probe state updates.
- Correlate by the exact command and request IDs supplied by the UI.
- Do not alter the already-running user Launcher or install personal content.

## Review Focus

- Import and folder import completion, including cancellation and rejected entries, must return one correlated terminal ACK.
- Core probe load success and failure must return a correlated terminal ACK without hiding detailed diagnostics.
- Stale worker generations must not deliver an ACK to a newer operation.
- Existing nonterminal progress callbacks and settings/library refreshes must remain intact.

### Task 1: Correlate asynchronous action completions

**Files:**
- Modify: `desktop-rust/crates/pokedaisy-desktop/src/shell.rs`
- Modify: `desktop-rust/ui/src/app.jsx` only if terminal import results need visible handling
- Modify: `docs/VYNTRA_PLAN_TECNICO.md`
- Modify: `docs/VYNTRA_IMPLEMENTATION_STATUS.md`

**Interfaces:**
- `ImportFinished` carries `command_id` and optional `request_id` in addition to generation, entries, and summary.
- `CoreProbed` carries `command_id` and optional `request_id` in addition to generation and report.
- Existing `UiAck::terminal` is used for both operations.

- [x] Write unit tests asserting import success/cancellation summary and core probe success/failure preserve both IDs.
- [x] Run the targeted tests and observe expected failures before implementation.
- [x] Pass the originating IDs through import and probe worker events and emit terminal ACKs only for the active generation.
- [x] Run Rust formatting, the full Windows GNU workspace suite, UI tests, and relevant launcher smoke checks.
- [x] Update T02 evidence and remaining gaps, review the diff, then commit and push the milestone.
