# Vyntra Desktop (Rust)

## Interfaz de escritorio

Windows abre ahora una ventana nativa Wry/WebView2 con Preact, Tailwind CSS y
Lucide. El HTML, CSS y JavaScript se empaquetan en el ejecutable; no se usa CDN
ni se cargan páginas remotas. WebView2 Runtime debe estar instalado en Windows.
El frontend SDL se conserva como alternativa con `--sdl`, y la emulación usa
un proceso SDL/libretro separado (`--emulator`). Esto evita que un fallo del
núcleo cierre la biblioteca.

La biblioteca permite importar archivos con diálogo nativo, buscar, filtrar
por sistema y alternar tarjetas/lista. Las importaciones quedan indexadas en
`data/vyntra-library.json`, con copia anterior `.bak`. Los contadores provienen
del índice real; las cubiertas geométricas Vyntra son marcadores originales.

Configuración permite cambiar carpetas, DLL por consola, volumen, filtro de
vídeo, pantalla completa, avance rápido, autoguardado, animaciones y teclado.
Las preferencias se guardan en `data/vyntra-settings.json` con copia `.bak`.
Cambiar carpetas no traslada ni elimina partidas. Los ajustes de reproducción
se aplican al siguiente proceso de emulación. Vyntra Vault lista archivos reales
y permite exportar copias con diálogo nativo.

Companion y Link no tienen integración Desktop; la interfaz indica esa limitación.
Los estados aún no tienen metadatos suficientes para una restauración automática
segura. Los mandos físicos mantienen el mapa SDL existente; aún no hay editor de mandos.

Para reconstruir la interfaz tras modificar `ui/src/`:

```powershell
cd ui
npm ci
npm run build
cd ..
cargo build --release
```

`ui/dist/index.html` es el recurso generado que Rust incorpora al ejecutable.
Wry/Tao y serde: MIT/Apache-2.0; Preact/Tailwind: MIT; Lucide: ISC.
Las versiones exactas se fijan en `Cargo.lock` y `ui/package-lock.json`.

Vyntra Desktop is a Windows game library and native libretro frontend. It accepts GB, GBC, GBA and NDS ROM files. Companion data is available only when a loaded game and core provide a verified adapter; the desktop companion screens are not implemented yet.

## Run

Place compatible Vyntra core DLLs in `cores/` beside the executable, or set `VYNTRA_CORE_DIR` to another core folder:

- `vyntra_gbc_libretro.dll` for GB and GBC
- `vyntra_gba_libretro.dll` for GBA
- `vyntra_ds_libretro.dll` for NDS

Then run `cargo run --release`. Choose **Buscar ROM** or drag a ROM onto the window. Games in `data/roms` are scanned recursively; start a specific ROM with `cargo run --release -- path/to/game.gba`. Saves go in `data/saves`.

Existing installations can continue using the legacy `pokedaisy_*_libretro.dll` names and `POKEDAISY_CORE_DIR`; Vyntra-named files and `VYNTRA_CORE_DIR` take precedence. The old environment variable emits a deprecation notice in debug builds.

This checkout does not yet build the Windows core DLLs. Standard RetroArch DLLs are not assumed compatible: the frontend dynamically loads a core and attempts to load a selected ROM before starting emulation. The Linux build recipes in [`../linux/README.md`](../linux/README.md) document the project adapters and engine sources. Preserve third-party license notices when building or distributing cores.

## Controls

- Library: Up/Down or J/K to select, Enter to start, double-click a game to start, F5 to rescan.
- Game: arrows for the D-pad, Z/X for A/B, F/Tab for Y/X, Enter/Backspace for Start/Select, Q/E for L/R, 1/2 for L2/R2, Space for fast-forward, F2/F4 for save/load state, Escape to return, F11 for fullscreen.
- SDL GameController compatible gamepads are read when connected.

The scanner currently accepts supported file extensions and does not filter hack titles. Persistent library indexes, ZIP import, Vyntra Vault, Sync, Link, companion panels and game metadata recognition are not implemented in this desktop build. Android remains the source of the existing supported-game matrix in the [project README](../README.md#supported-games).
