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

## Compatibility rule

The launcher should not reject a ROM because its title is unfamiliar. The emulator core determines whether it can run the file; a separate Rust game profile determines which companion features are available. Profile support must be stated per game and per feature.
