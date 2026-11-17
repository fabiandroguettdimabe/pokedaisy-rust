# Rust Libretro Windows Proof Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver the first runnable Windows Rust vertical slice that builds or loads each supported PokeDaisy libretro core, runs a diagnostic ROM, and presents its video/audio/input through SDL.

**Architecture:** A small Cargo workspace separates the unsafe libretro ABI loader from the SDL desktop shell. The shell loads one core DLL per ROM system, owns the callbacks and frontend lifecycle, and never owns emulator logic. This phase proves the Windows toolchain and core boundary; companion behavior and the later complete feature migration are separate plans.

**Tech Stack:** Rust 2024 stable; SDL2 via `sdl2` 0.38.0 with bundled SDL; `libloading` 0.8.9 for core DLL loading; CMake + MSYS2 MinGW-w64 POSIX for the existing native core DLLs; Python 3 for the repository's original diagnostic ROM generators.

**Spec:** `docs/superpowers/specs/2026-11-15-rust-desktop-design.md`

## Global Constraints

- Windows x86_64 is the first target.
- All PokeDaisy product behavior ultimately belongs in Rust; native emulator engines remain dependencies.
- The existing modified cores may be used in this compatibility proof as a transitional reference.
- Use only original diagnostic ROMs from the repository; do not use commercial ROMs in tests or packaging.
- Existing `.sav` / `.srm` files must not be truncated or replaced until the new core session has loaded successfully and a backup has completed.
- A failing core load or unsupported ROM must return an actionable error and leave save data untouched.
- Keep unsafe FFI operations inside `pokedaisy-libretro`.
- Keep the Rust ABI declarations matched to the pinned `third_party/mgba/src/platform/libretro/libretro.h` contract; assert x86_64 structure sizes and field offsets.

## Review Focus

- **Core DLL missing a required export:** report its path and symbol; unload the library cleanly. Covered by `core_load_reports_missing_required_symbol`.
- **Core requests a pixel format or geometry change:** preserve the requested format and resize buffers safely. Covered by `core_pixel_format_and_geometry_changes_are_applied`.
- **Core sends a null frame to repeat the previous image:** retain the last valid frame without reading a null pointer. Covered by `null_video_callback_reuses_last_frame`.
- **Core sends audio/video callbacks during startup, load failure, or shutdown:** callbacks must not access freed state. Covered by `callbacks_are_inert_after_unload`.
- **ROM extension is unsupported or its core DLL is absent:** show an actionable error and do not create or modify a save. Covered by `unsupported_rom_does_not_touch_saves` and `missing_core_does_not_touch_saves`.

---

### Task 1: Build and identify Windows core DLLs

**Files:**
- Create: `desktop-rust/scripts/build-windows-cores.ps1`
- Create: `desktop-rust/scripts/build-diagnostics.ps1`
- Create: `desktop-rust/.gitignore`
- Modify: `libretro/CMakeLists.txt`
- Modify: `libretro/armgba/CMakeLists.txt`
- Modify: `linux/libretro-ds/CMakeLists.txt`
- Create: `libretro/retro.def`
- Create: `libretro/armgba/retro.def`
- Create: `linux/libretro-ds/retro.def`
- Create: `desktop-rust/README.md`

**Interfaces:**
- Produces: `desktop-rust/build/cores/pokedaisy_gbc_libretro.dll`, `pokedaisy_gba_libretro.dll`, and `pokedaisy_ds_libretro.dll` from the repository's pinned submodules and project sources.
- The script accepts `-BuildDir <path>` and `-Configuration Release`; it fails if any expected DLL is missing.
- The script uses MSYS2's `mingw64` environment (default `C:\msys64\mingw64`, overridable with `-Msys2Root`), CMake, and Python 3; it initializes only the submodules required by the three existing core builds.

- [ ] **Step 1: Add a PowerShell smoke check for the three expected DLL outputs**

Create `desktop-rust/scripts/test-windows-cores.ps1`. It invokes the build script in a temporary build directory, asserts all three DLL paths exist and have non-zero length, then checks each DLL exports the required `retro_*` functions from `libretro/test/check_exports.sh` using MinGW `objdump`.

- [ ] **Step 2: Run the smoke check to verify it fails before the build script exists**

Run: `powershell -NoProfile -File desktop-rust/scripts/test-windows-cores.ps1`
Expected: FAIL because the Windows core build script and DLL outputs do not exist yet.

- [ ] **Step 3: Add Windows shared-library export handling to the three existing core CMake targets**

Use `.def` files to expose the same `retro_*` entry points as the current ELF version scripts. Keep the Linux linker options unchanged. The build script initializes the existing submodules, configures the projects with MSYS2 MinGW-w64 POSIX compilers, builds each named CMake target in Release mode, and copies the resulting DLLs into `desktop-rust/build/cores/`.

- [ ] **Step 4: Run the Windows core smoke check**

Run: `powershell -NoProfile -File desktop-rust/scripts/test-windows-cores.ps1`
Expected: PASS; all three DLLs exist and export the required libretro entry points.

- [ ] **Step 5: Document the local prerequisites and core output folder**

`desktop-rust/README.md` lists Rust stable, CMake, Python 3, Git submodules, and the supported C/C++ toolchain, then gives the exact command to build the core DLLs.
`desktop-rust/.gitignore` excludes `/build/`. `build-diagnostics.ps1` invokes the repository's GB/GBA and DS diagnostic generators from `desktop-rust/build/diagnostics/`, so it does not leave generated ROMs among tracked source files.

### Task 2: Create the Cargo workspace and ROM-to-core selection

**Files:**
- Create: `desktop-rust/Cargo.toml`
- Create: `desktop-rust/Cargo.lock`
- Create: `desktop-rust/crates/pokedaisy-libretro/Cargo.toml`
- Create: `desktop-rust/crates/pokedaisy-libretro/src/lib.rs`
- Create: `desktop-rust/crates/pokedaisy-desktop/Cargo.toml`
- Create: `desktop-rust/crates/pokedaisy-desktop/src/main.rs`
- Create: `desktop-rust/crates/pokedaisy-desktop/src/core_selection.rs`
- Create: `desktop-rust/crates/pokedaisy-desktop/tests/core_selection.rs`

**Interfaces:**
- Produces: `CoreKind::{Gb, Gba, Ds}` and `select_core(rom_path: &Path) -> Result<CoreKind, CoreSelectionError>`.
- Produces: `core_library_filename(kind: CoreKind) -> &'static str` mapping each `CoreKind` to its `.dll` filename.
- `.gb` / `.gbc` select GB/GBC, `.gba` selects GBA, `.nds` selects DS; extension matching is case-insensitive.
- This selects only the system core; game identity and hack/revision detection are a later `pokedaisy-game` deliverable.

- [ ] **Step 1: Create the minimal Cargo workspace and crate manifests**

Define the `pokedaisy-libretro` library and `pokedaisy-desktop` binary packages, with an empty `main.rs` and module root so Cargo can run the first test. Pin the dependencies specified in the plan header.

- [ ] **Step 2: Write `rom_core_is_selected_by_case_insensitive_extension` and `unsupported_extension_returns_error`**

The first test checks `.GB`, `.gbc`, `.GBA`, and `.nds`; the second checks `.zip` and an extensionless path.

- [ ] **Step 3: Run the tests and verify they fail because the API is missing**

Run: `cargo test --manifest-path desktop-rust/Cargo.toml --target x86_64-pc-windows-gnu -p pokedaisy-desktop --test core_selection`
Expected: FAIL because `select_core` and `CoreKind` are not implemented.

- [ ] **Step 4: Implement `select_core` and the remaining workspace modules**

Use only `std::path`; keep core selection independent of filesystem access and UI.

- [ ] **Step 5: Run the selection tests**

Run: `cargo test --manifest-path desktop-rust/Cargo.toml --target x86_64-pc-windows-gnu -p pokedaisy-desktop --test core_selection`
Expected: PASS for all supported suffixes and clear errors for unsupported inputs.

### Task 3: Implement the safe libretro ABI wrapper

**Files:**
- Create: `desktop-rust/crates/pokedaisy-libretro/src/abi.rs`
- Create: `desktop-rust/crates/pokedaisy-libretro/src/core.rs`
- Create: `desktop-rust/crates/pokedaisy-libretro/src/callbacks.rs`
- Create: `desktop-rust/crates/pokedaisy-libretro/src/error.rs`
- Modify: `desktop-rust/crates/pokedaisy-libretro/src/lib.rs`
- Create: `desktop-rust/crates/pokedaisy-libretro/tests/abi_contract.rs`
- Create: `desktop-rust/crates/pokedaisy-libretro/tests/core_smoke.rs`

**Interfaces:**
- `CorePaths { save_dir: PathBuf, system_dir: PathBuf }` supplies the directory callbacks.
- `JoypadState { buttons: u16 }` carries the active libretro joypad bitmask.
- `SystemAvInfo { base_width: u32, base_height: u32, max_width: u32, max_height: u32, aspect_ratio: f32, fps: f64, sample_rate: f64 }` describes the loaded system.
- `CoreFrame { video: Option<VideoFrame>, audio: Vec<i16> }`; `VideoFrame` owns pixels, width, height, pitch, and pixel format.
- `LibretroCore::load(library_path: &Path, paths: CorePaths) -> Result<LibretroCore, CoreError>` resolves the required symbols and performs `retro_set_environment`, `retro_init`, then callback registration.
- `LibretroCore::load_game(&mut self, rom_path: &Path) -> Result<SystemAvInfo, CoreError>` reads the ROM, loads it, and returns the core's base geometry and timing.
- `LibretroCore::run_frame(&mut self, input: JoypadState) -> Result<CoreFrame, CoreError>` runs one frame and returns owned video/audio data plus any core error.
- `LibretroCore::save_state(&mut self) -> Result<Vec<u8>, CoreError>` and `load_state(&mut self, bytes: &[u8]) -> Result<(), CoreError>` use the core's serialize API.
- `LibretroCore::read_save_ram(&mut self) -> Result<Vec<u8>, CoreError>` and `write_save_ram(&mut self, bytes: &[u8]) -> Result<(), CoreError>` use `RETRO_MEMORY_SAVE_RAM` and reject size mismatches.
- `LibretroCore::Drop` unloads a loaded game, calls `retro_deinit`, and releases the DLL in that order.
- The CLI accepts `--rom <path> --core-dir <path> --data-dir <path>`; DLL names come from `core_library_filename`.
- `InputAction` is `Button { button: JoypadButton, pressed: bool } | Quit`; `JoypadButton` includes A, B, Select, Start, Up, Down, Left, Right, X, Y, L, and R.

- [ ] **Step 1: Write ABI layout, callback, and diagnostic-core tests**

`abi_contract` asserts the x86_64 sizes and field offsets of used `repr(C)` structures against the pinned header's ABI. Unit tests are named `core_load_reports_missing_required_symbol`, `null_video_callback_reuses_last_frame`, and `callbacks_are_inert_after_unload`. `core_smoke` defines `gb_diagnostic_runs_and_emits_video`, `gba_diagnostic_runs_and_emits_video`, `ds_diagnostic_runs_and_emits_video`, and `diagnostic_state_round_trip_replays_same_frame`; each core test runs its original diagnostic ROM for 120 frames, asserts non-zero frame dimensions/checksum, then verifies a serialize/unserialize replay produces the same next-frame checksum.

- [ ] **Step 2: Run the tests to verify they fail before the ABI wrapper exists**

Run: `powershell -NoProfile -File desktop-rust/scripts/build-diagnostics.ps1; $env:POKEDAISY_DIAGNOSTIC_DIR='desktop-rust/build/diagnostics'; $env:POKEDAISY_CORE_DIR='desktop-rust/build/cores'; cargo test --manifest-path desktop-rust/Cargo.toml --target x86_64-pc-windows-gnu -p pokedaisy-libretro`
Expected: FAIL because `LibretroCore` and its ABI types/callbacks are not implemented.

- [ ] **Step 3: Implement required ABI types, symbol table, callbacks, and lifecycle**

Limit `unsafe` to `abi.rs`, DLL symbol invocation, and callback pointer conversion. Support the environment commands requested by the three cores for pixel format, options, directories, geometry, logging, input bitmasks, and fast-forward state. Log unknown environment requests and return `false` without dereferencing unrecognized data. Use `libloading` and retain the `Library` for the full lifetime of every loaded symbol.

- [ ] **Step 4: Run ABI and all three diagnostic-core tests**

Run: `$env:POKEDAISY_DIAGNOSTIC_DIR='desktop-rust/build/diagnostics'; $env:POKEDAISY_CORE_DIR='desktop-rust/build/cores'; cargo test --manifest-path desktop-rust/Cargo.toml --target x86_64-pc-windows-gnu -p pokedaisy-libretro`
Expected: PASS; ABI assertions match the pinned header, callback ownership tests pass, and all three cores emit diagnostic frames and round-trip serialized state.

### Task 4: Present core frames and route keyboard/controller input

**Files:**
- Create: `desktop-rust/crates/pokedaisy-desktop/src/app.rs`
- Create: `desktop-rust/crates/pokedaisy-desktop/src/video.rs`
- Create: `desktop-rust/crates/pokedaisy-desktop/src/input.rs`
- Create: `desktop-rust/crates/pokedaisy-desktop/src/audio.rs`
- Create: `desktop-rust/crates/pokedaisy-desktop/tests/video_format.rs`
- Modify: `desktop-rust/crates/pokedaisy-desktop/src/main.rs`
- Modify: `desktop-rust/crates/pokedaisy-desktop/Cargo.toml`

**Interfaces:**
- `DesktopApp::run(core_path: &Path, rom_path: &Path, data_dir: &Path) -> Result<(), DesktopError>` owns SDL initialization and the frame loop.
- `map_sdl_event(event: &Event) -> Option<InputAction>` maps arrows and Z/X/Enter/Backspace to D-pad/A/B/Start/Select, and maps the first recognized SDL game controller using SDL's standardized GameController mapping.
- The frame presenter handles RGB565 and XRGB8888, maintains nearest-neighbor pixel scaling, and retains the last valid frame when a core duplicates output.

- [ ] **Step 1: Write input mapping and frame-rectangle tests**

`keyboard_keys_map_to_libretro_actions` asserts arrows, Z, X, Enter, and Backspace; `rgb565_pixels_convert_to_sdl_rgb565` and `xrgb8888_pixels_convert_to_sdl_argb8888` assert exact channel values; `core_pixel_format_and_geometry_changes_are_applied` verifies the requested pixel format and geometry changes are reflected in the next texture; `aspect_fit_uses_integer_scale_when_available` asserts centered integer scaling and a bounded fractional fallback.

- [ ] **Step 2: Run the desktop unit tests and verify they fail**

Run: `cargo test --manifest-path desktop-rust/Cargo.toml --target x86_64-pc-windows-gnu -p pokedaisy-desktop`
Expected: FAIL because the SDL input and frame geometry functions are not implemented.

- [ ] **Step 3: Implement the SDL2 window, streaming texture, event handling, and queued audio**

Use `sdl2` 0.38.0 with bundled SDL2. The core loop runs `retro_run`, updates the video texture with the callback's pitch, queues interleaved audio, polls SDL events, and runs at the core-reported frame rate. Escape exits cleanly; resizing recomputes the display rectangle without changing the emulated framebuffer.

- [ ] **Step 4: Run the desktop unit tests**

Run: `cargo test --manifest-path desktop-rust/Cargo.toml --target x86_64-pc-windows-gnu -p pokedaisy-desktop`
Expected: PASS for key mapping, pixel conversion, geometry changes, and aspect-fit behavior.

Then run: `cargo run --manifest-path desktop-rust/Cargo.toml --target x86_64-pc-windows-gnu -p pokedaisy-desktop -- --rom desktop-rust/build/diagnostics/diagnostic.gba --core-dir desktop-rust/build/cores --data-dir desktop-rust/build/data`
Expected: an SDL window opens, shows the GBA diagnostic image without stretching/cropping it, plays diagnostic audio, responds to keyboard/controller input, and exits cleanly with Escape.

### Task 5: Persist battery saves safely

**Files:**
- Create: `desktop-rust/crates/pokedaisy-desktop/src/save.rs`
- Create: `desktop-rust/crates/pokedaisy-desktop/tests/save_safety.rs`
- Create: `desktop-rust/crates/pokedaisy-desktop/tests/core_save.rs`
- Create: `desktop-rust/crates/pokedaisy-desktop/tests/common/mod.rs`

**Interfaces:**
- Save paths are `<data_dir>/saves/<complete-rom-filename>.sav`.
- `BatterySave::prepare(save_path: &Path, backup_dir: &Path) -> Result<BatterySave, SaveError>` backs up a changed existing save before a new session writes it, retaining the newest ten backups.
- `BatterySave::restore(&self, core: &mut LibretroCore) -> Result<(), SaveError>` restores the existing save after successful ROM load; `BatterySave::persist(&self, core: &mut LibretroCore) -> Result<(), SaveError>` atomically writes the core's save RAM.
- `TestDir::new(label: &str) -> TestDir` creates a unique directory under `std::env::temp_dir()` using the process id and an atomic counter; `Drop` removes it.

- [ ] **Step 1: Write save safety and GB diagnostic persistence tests**

Add `prepare_backs_up_changed_save_before_writing`, `prepare_keeps_only_ten_backups`, `persist_replaces_save_atomically`, `backup_failure_prevents_save_replacement`, `unsupported_rom_does_not_touch_saves`, `missing_core_does_not_touch_saves`, and `failed_game_load_preserves_existing_save` in `save_safety.rs` using temporary directories and real files. Add `gb_diagnostic_battery_save_is_written_after_successful_load` in `core_save.rs`, using the generated GB diagnostic ROM and its save marker.

- [ ] **Step 2: Run the save safety tests and verify they fail because the API is missing**

Run: `$env:POKEDAISY_CORE_DIR='desktop-rust/build/cores'; $env:POKEDAISY_DIAGNOSTIC_DIR='desktop-rust/build/diagnostics'; cargo test --manifest-path desktop-rust/Cargo.toml --target x86_64-pc-windows-gnu -p pokedaisy-desktop`
Expected: FAIL because `BatterySave` and its atomic persistence logic are not implemented.

- [ ] **Step 3: Implement battery-save backup and persistence**

Read the core's save RAM only through `LibretroCore::read_save_ram` / `write_save_ram`. Back up a changed existing file before replacement; write to a sibling temporary file, flush it, then rename. A backup or rename error must leave the existing save intact.

- [ ] **Step 4: Run the save safety tests**

Run: `$env:POKEDAISY_CORE_DIR='desktop-rust/build/cores'; $env:POKEDAISY_DIAGNOSTIC_DIR='desktop-rust/build/diagnostics'; cargo test --manifest-path desktop-rust/Cargo.toml --target x86_64-pc-windows-gnu -p pokedaisy-desktop`
Expected: PASS for backup-on-change, ten-backup rotation, atomic replacement, failure preservation, no-save-on-failed-launch, and the GB battery marker.

- [ ] **Step 5: Run the full Phase 1 verification suite**

Run: `powershell -NoProfile -File desktop-rust/scripts/test-windows-cores.ps1`
Then run: `powershell -NoProfile -File desktop-rust/scripts/build-diagnostics.ps1`
Then run: `$env:POKEDAISY_CORE_DIR='desktop-rust/build/cores'; $env:POKEDAISY_DIAGNOSTIC_DIR='desktop-rust/build/diagnostics'; cargo test --manifest-path desktop-rust/Cargo.toml --target x86_64-pc-windows-gnu --workspace`
Expected: PASS for core DLL export checks, ROM-to-core selection, ABI/callback behavior, all three diagnostic cores, state round trips, save backup safety, the GB battery marker, and SDL-independent unit tests.

- [ ] **Step 6: Document the runnable command and compatibility limits**

`desktop-rust/README.md` documents build prerequisites, the core DLL folder, diagnostic generation, the `cargo run` invocation, and that this phase does not yet include the Rust companion or full feature parity.

## Scope after this plan

This phase ends with a working Rust/libretro Windows shell and is independently testable. The following plans are separate deliverables: desktop library and host-menu parity; Rust game readers and companion UI; Rust gameplay integrations; Android feature parity and Windows packaging. They must preserve the design's final requirement that no shipped desktop feature depends on PokeDaisy-specific logic inside the emulator cores.
