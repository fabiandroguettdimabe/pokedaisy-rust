# PokeDaisy Desktop (Rust)

This is the first Windows desktop slice: a game library and native libretro frontend. It reuses PokeDaisy's existing libretro cores, game recognition, companion, HUD, quick menu, and project font. The frontend accepts every GB, GBC, GBA, or NDS ROM and does not require a hack name whitelist; the core decides which companion features it can provide for the loaded game.

## Run

Place the Windows core DLLs in `cores/` beside the executable:

- `pokedaisy_gbc_libretro.dll` for GB and GBC
- `pokedaisy_gba_libretro.dll` for GBA
- `pokedaisy_ds_libretro.dll` for NDS

Then run `cargo run --release`. Use **Open a game** to choose a ROM, or drag one onto the window. Games in `data/roms` are scanned recursively; start a specific ROM with `cargo run --release -- path/to/game.gba`. Set `POKEDAISY_CORE_DIR` to use a different core folder. Saves go in `data/saves`.

This checkout does not yet build the Windows core DLLs. The existing Linux build recipes in [`../linux/README.md`](../linux/README.md) are the reference while the Windows build is being brought up.

## Controls

- Library: Up/Down or J/K to select, Enter to start, double-click a game to start, F5 to rescan.
- Game: arrows for the D-pad, Z/X for A/B, F/Tab for Y/X (the core's quick menu/companion controls), Enter/Backspace for Start/Select, Q/E for L/R, 1/2 for L2/R2, Space for fast-forward, F2/F4 for save/load state, Escape to return, F11 for fullscreen.
- SDL GameController compatible gamepads are read when connected.

The desktop scanner does not filter by hack title, so Radical Red, Unbound, Gaia, and other GB/GBC/GBA/NDS ROM hacks can all be launched. Companion panels still depend on the existing core's game profiles, listed in the [supported-games table](../README.md#supported-games); this first desktop slice has not added new game profiles yet. The next compatibility increment is to carry those shared profiles into the Rust-owned layer and broaden them without making the launcher reject unfamiliar hacks.
