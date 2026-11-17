# T19 Memory Adapter Registry Implementation Plan

> **For agentic workers:** Follow the tasks in order and keep each task independently verifiable.

**Goal:** Establish a bounded, read-only memory adapter boundary keyed by exact game/core identity and reject stale captures after session epochs change.

**Architecture:** Rust registry resolves adapters only by exact system, ROM SHA-256, core SHA-256, and game revision. A memory owner supplies requested ranges; capture copies bytes into owned buffers before decode. Snapshots are accepted only for the active session epoch. No raw pointer or UI-provided address crosses the boundary.

**Tech Stack:** Rust, existing desktop crate, serde-free internal contracts, existing `LatestSnapshot` transport.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, MULTI-05 and T19.

## Global Constraints

- Unknown identities remain unsupported; no fallback to arbitrary core memory.
- Captures are read-only, bounded, and copied into owned buffers.
- UI receives decoded snapshots only, never offsets or pointers.
- Do not claim a real game adapter without a licensed fixture and exposed-memory owner.

## Review Focus

- Similar ROM identity with a different hash never resolves an adapter.
- Range overflow/out-of-bounds and truncated reads fail without partial snapshots.
- Captures over the configured byte limit fail before decode.
- An old epoch is rejected after load/session transition.
- No capture can mutate the memory owner.

## Tasks

- [x] Add failing unit tests for exact identity matching, bounded range capture, truncation/overflow, and stale epoch rejection.
- [x] Implement the registry, range contract, owned capture, and epoch gate with minimal public API.
- [x] Run the focused tests and complete Rust crate suite; format and inspect the diff.
- [x] Mark T19 partial with the missing legal fixture/runtime memory owner, commit, and push the milestone.
- [x] Add a bounded Gen III live-party decoder that validates captured Pokémon checksums, party count, level, and HP using synthetic bytes only.
- [x] Bind the party-count and party ranges to an exact-identity Gen III adapter; test the capture-to-decoder path and reject mismatched identity or missing segments.
- [x] Add a session/epoch/revision `MemoryPoller` that resolves only registered exact identities and emits decoded snapshots; test registered and unsupported identities.
- [x] Align the Libretro environment command with `36 | RETRO_ENVIRONMENT_EXPERIMENTAL`; the AV fixture calls the official value and proves mapped reads work.
- [x] Keep poller revisions monotonic after decoder rejection; test that the next valid capture recovers and publishes.
- [ ] Wire the exact-identity adapter into runtime Player polling and the pipe; verify with a legally redistributable fixture for one concrete revision before advertising live data.


