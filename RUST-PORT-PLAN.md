# PokeDaisy Rust Desktop Port

This file is the project checkpoint and continuation guide for the standalone Rust port in this repository.

## Where we are

**Checkpoint: 2026-11-16**

- The initial Rust desktop slice is in `desktop-rust/`: an SDL window, a libretro core loader, a game library, ROM picker, and drag and drop.
- The library screen has been redesigned around PokeDaisy's existing identity and resources.
- The launcher does not use a hack-name whitelist. It accepts GB, GBC, GBA, and NDS file types and chooses a core by system.
- `cargo check --workspace` and `cargo build --release --workspace` both completed successfully on the development machine.
- **The game runtime has not yet been verified on Windows:** the required Windows core DLLs are not built or packaged, and no end-to-end game session has been confirmed.
- Companion behavior and game-specific data have not yet been ported to Rust. Existing recognition and companion support remain limited to profiles in the original cores.
- ROM hacks such as Radical Red and Unbound are not blocked by their names in the launcher, but their companion features are not yet implemented or newly supported by this port.

## Continue here

### Next task: build and run the Windows cores

1. Inspect the existing core CMake builds and confirm the Windows compiler and library requirements for GB/GBC, GBA, and NDS.
2. Add a reproducible Windows build script that produces the three DLLs expected by `desktop-rust`.
3. Build diagnostic or homebrew ROMs and launch one for each core through the desktop frontend.
4. Fix any DLL exports, video, audio, input, or shutdown issues found in those runs.
5. Document the exact prerequisites and commands in `desktop-rust/README.md`.

**This task is complete when** a clean Windows setup can build the cores and play a diagnostic ROM for each supported system, with saves preserved across successful and failed launches.

## After the Windows runtime works

### Move compatibility data into Rust

- Port game recognition, version/revision checks, and per-game feature capabilities from the original project.
- Keep game launch independent from companion profile matching: an unknown hack can still be opened, with unsupported companion features clearly indicated.
- Add verified profiles incrementally, prioritizing Radical Red and Unbound, then other high-demand hacks and existing PokeDaisy-supported titles.
- Record the exact ROM hashes/builds and the companion features verified for each profile.

### Port the companion

- Start with read-only party and bag data, then battle, map, Pokédex, guide, and PC boxes.
- Port the HUD, quick menu, and actions that modify game data after the readers are reliable.
- Reuse the base project's licensed fonts, icons, maps, and other assets where applicable.
- Preserve battery saves and create backups before any feature writes to game data.

### Prepare a desktop release

- Complete library management, settings, resizing, keyboard navigation, and gamepad remapping.
- Package a reproducible Windows build with clear core setup and compatibility information.
- Verify a clean install, ROM selection, controls, gameplay, and save preservation.

## Further improvements

These improvements build on the core port milestones and can be prioritized as the Windows runtime becomes stable.

### Compatibility and core management

- [ ] Add a compatibility view per game showing verified ROM version/hash and support for each companion feature.
- [ ] Provide a useful status for unknown games and a simple way to export a local diagnostic report for improving compatibility.
- [ ] Pin and display core versions, verify expected DLLs at startup, and explain missing or mismatched cores.
- [ ] Let contributors add reviewed game profiles without changing ROM-launch rules.

### Library and per-game setup

- [ ] Add search, favorites, system filters, duplicate detection, and optional cover art.
- [ ] Store per-game controls, core options, display layout, scaling, and audio settings.
- [ ] Import and export library settings and profile data so a reinstall does not require configuring everything again.

### Save safety and portability

- [ ] Keep battery saves in standard `.sav` / `.srm` formats where the selected core supports them; make backup and restore actions visible.
- [ ] Explain that save states can depend on the core and its version; identify the core used to create each state before offering to load it.
- [ ] Add recovery guidance for interrupted writes and provide an easy way to open the save folder.

### Performance and display quality

- [ ] Measure frame pacing, dropped frames, and audio underruns on representative hardware before tuning buffers.
- [ ] Keep audio and video synchronized during normal speed, fast-forward, and slow motion.
- [ ] Support high-DPI displays, window movement between monitors, integer scaling, and resizable companion layouts.
- [ ] Offer optional shaders and overlays after the baseline renderer is stable.

### Input, accessibility, and support

- [ ] Detect gamepads connected or disconnected while the app is running; offer standard mappings and per-game remapping.
- [ ] Support keyboard-only navigation, scalable text, clear focus states, and readable contrast.
- [ ] Add searchable local logs and a copyable diagnostic summary; keep ROM paths and user data out of reports unless the user chooses to include them.
- [ ] Reuse available project translations and make missing strings easy to identify.

### Broaden platform support

- [ ] Keep platform-specific core loading and packaging isolated so Linux and macOS can follow after the Windows release is reliable.
- [ ] Track platform-specific core availability separately from game and companion compatibility.

## Technical references

- [Libretro core development](https://docs.libretro.com/development/cores/developing-cores/) describes the frontend/core boundary, fixed-rate audio and video expectations, input callbacks, and save-state serialization considerations.
- [SDL2 controller device events](https://wiki.libsdl.org/SDL2/SDL_ControllerDeviceEvent) document controller connect, disconnect, and remap notifications.
- [SDL high-DPI window guidance](https://wiki.libsdl.org/SDL2/SDL_CreateWindow) explains the difference between window coordinates and drawable pixels when high-DPI support is enabled.
- [SDL audio device documentation](https://wiki.libsdl.org/SDL2/SDL_OpenAudioDevice) documents the latency and underrun tradeoff when choosing audio buffer sizes.

## Compatibility rule

The launcher should not reject a ROM because its title is unfamiliar. The emulator core determines whether it can run the file; a separate Rust game profile determines which companion features are available. Profile support must be stated per game and per feature.
