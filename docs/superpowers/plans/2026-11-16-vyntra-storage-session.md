# Vyntra Desktop Storage and Session Plan

**Goal:** Safely migrate legacy saves and ensure each running player owns an immutable ROM identity for all save and state operations.

**Architecture:** Keep the SDL player as the sole owner of its core and active `Game`. Add a small storage migration service using an explicit preview, versioned journal, staged copies and hash verification; never delete or overwrite legacy files. Finish the related catalog and Vault gaps needed to resolve identity and legacy conflicts.

**Tech Stack:** Rust 2024, existing serde/serde_json/sha2, SDL2 player and Wry/Tao shell.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, tasks T03–T05 and T08; sections 5, 7.3, 11 and 17.

## Global Constraints

- Legacy files remain untouched; conflicting candidates are surfaced rather than selected by filename.
- Save ownership comes from the active session, never the mutable library selection.
- An artifact is committed only after write and hash verification.
- Preserve existing JSON and on-disk compatibility where safe; do not claim runtime core validation.

## Review Focus

- Same-named legacy saves for different ROM contents remain separate candidates.
- Interrupted migration resumes without replacing verified destination data.
- ROM replacement after catalog scan is detected before launch and cannot inherit a stale identity.
- Drop/import during play never changes the current session's save destination.
- Save/state write failures remain visible and do not publish a committed manifest.

---

### Task 1: Catalog inspection and identity refresh

Add bounded GB/GBC/GBA/DS header inspection and revalidate the content hash at launch. Keep unknown or malformed ROMs identifiable by content without claiming a game match. Add tests for truncated headers, false extensions, changed bytes and duplicate content.

### Task 2: Legacy storage migration service

Create a migration module with a preview report, versioned journal, staging copies and per-file SHA-256 verification. Only migrate unambiguous files; preserve homonymous conflicts in place and report them. Make preview read-only and recovery idempotent. Test interruption at copy/verify/publish boundaries and rerun behavior.

### Task 3: Vault recovery and conflict handling

Validate existing save manifests against payload hashes and identity. Add explicit backup/restore copies and reject incompatible save-state metadata. Ensure a failed payload or manifest write cannot appear committed. Test corrupt payloads, conflicting legacy candidates and failed writes.

### Task 4: Immutable player session ownership

Keep an `ActiveSession` in the SDL process containing the selected `Game`, session identity and core hash. Route autosave, manual save, state load/save and shutdown through that object. DropFile only updates the library after safely closing or retaining the active session. Test selection/drop changes while active and two same-named ROMs.

### Task 5: Shell lifecycle and IPC convergence

Validate the requested stable game ID and current content hash before spawning. Track child exit and clear the matching session only; reject stale commands and publish actionable errors. Exercise changed catalog order, stale command epochs and child exit behavior.

### Task 6: Verification and status update

Run formatting and focused plus full Rust workspace tests. Update `docs/VYNTRA_IMPLEMENTATION_STATUS.md` and the plan's execution matrix to distinguish completed code paths from runtime checks that still require a core/device.
