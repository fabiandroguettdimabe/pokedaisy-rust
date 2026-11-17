# Desktop AV Clock Implementation Plan

**Goal:** Pace Desktop frames from the active core timing and keep core audio at the SDL device rate without unbounded queued latency.

**Architecture:** Add a small pure timing module in the desktop crate for frame intervals and stereo linear resampling. Integrate it at the existing SDL loop boundary, retaining the current last-frame behavior for duplicate video callbacks and bounding queued samples to 120 ms.

**Tech Stack:** Rust 2024, SDL2 audio queue, existing libretro `SystemAvInfo`.

**Spec:** `docs/VYNTRA_PLAN_TECNICO.md`, ticket T09 / WIN-PLAY-01 AV.

## Tasks

1. Add tests for 50/59.94/60 Hz frame intervals and stereo resampling from 22,050 to 44,100 Hz; verify RED.
2. Implement pure helpers and run focused tests; verify GREEN.
3. Use active core FPS in the player loop, resample each frame's stereo audio to the opened SDL frequency, and skip queueing when it would exceed the 120 ms budget. Keep fast-forward audio muted and clear its queue.
4. Run `cargo fmt --all --check`, `cargo test --workspace`, and `cargo check --workspace`; update T09 status with explicit runtime limits.
5. Commit and push the verified milestone.
