# Vyntra Desktop: matriz de viabilidad de cores en Windows

Actualizado: 2026-11-17. Hito T07 / WIN-CORE-02, build reproducible y smokes GBC/GBA/DS en Windows x64.

## Entorno reproducido

- Windows x64, WinLibs GCC 16.2 POSIX/UCRT/SEH (no MSYS2), CMake 4.4.3, Ninja 1.13.2 y Python 3.13.15.
- Configuración CMake para GBC, GBA y DS finaliza correctamente usando `-DCMAKE_POLICY_VERSION_MINIMUM=3.5`.
- No se encontraron clang/clang-cl. El build GBC y sus artefactos temporales están en `desktop-rust/target/t07-gbc-win`, ignorado por Git.

## Resultados

| Core objetivo | Configuración | Primer bloqueo de build | Estado |
|---|---|---|---|
| GBC (mGBA adaptado) | Build Release `pokedaisy_gbc_libretro.dll` con WinLibs GCC 16.2.0 / CMake 4.4.3 / Ninja 1.13.2; mGBA fijado en `26b7884bc25a5933960f3cdcd98bac1ae14d42e2`. | Adaptación `_mkdir`, exports PE explícitos y guard Windows de fallos mediante vectored exception handler. Probe Rust: `loadable`, sistema GBC, ABI 1, 22 exports requeridos y ninguno ausente. `configured_local_gbc_core_runs_generated_diagnostic_rom`: 120 frames, video/audio, SRAM marker `0x5a` y serialize/unserialize. Core SHA-256 `5b2c7819a12d9cee28eada6c91f69e72377cff469c4d55df9f2c211d2d4359fd`; ROM diagnóstica original de `linux/make_diagnostic.py`, SHA-256 `90d49d61eb62c365b355d5c4182da9e88f89fa0d773e0365c83db7bcb15a1000`; verificado en commit `6f8c1e6b373660057c00fa09315136f7a081b56d`. | DLL PE cargable y smoke API aprobados. No prueba Player SDL/input en ventana ni distribución del binario. |
| GBA (mGBA dentro de la DLL PokeDaisy compartida) | `pokedaisy_gbc_libretro.dll` se descubre como fallback GBA tras los nombres GBA dedicados; selección y apertura verificadas con `VYNTRA_CORE_DIR`. | Probe: `loadable`, sistema GBA, x86_64, API 1, 22 exports. `configured_local_core_runs_generated_gba_diagnostic_rom`: carga, geometría mínima, video por 120 cuadros y serialize/unserialize. Player SDL `--emulator` abre la ROM y muestra el patrón GBA en ventana 1016×699; `Responding=True`; al cerrar quedan SRAM de 32 KiB y manifiesto `committed` con hashes de ROM/core. Core SHA-256 `5b2c7819a12d9cee28eada6c91f69e72377cff469c4d55df9f2c211d2d4359fd`; ROM SHA-256 `5d43b3e4a9954f7b5b9bdac2dd954e9f01defeadfc39355562b593dcdf319582`; test `462c412440eaba1f89847a077e20b7d2e19b25db`; descubrimiento `8c38c946866048505f7042e5ba88d78bc5a9c86d`. | ROM homebrew verificada en API y Player SDL; E2E de teclado Win32 registrado abajo. Companion/lectura GBA live, cambios de SRAM escritos por el juego ni armGBA. No se empaqueta una DLL duplicada. |
| GBA (armGBA) | CMake configura con `ARMGBA_JIT=OFF` | El build se detiene en `third_party/armgba/src/gba.c:16`: `fatal error: sys/mman.h: No such file or directory`. Apagar JIT no elimina el mapeo POSIX usado para abrir ROMs. También hay usos `mmap`/`munmap` en `cpu/jit.c` y dependencias libretro-common (VFS y `archive_file.c`); aún no se evaluaron todos los siguientes errores de compilación. El enlace usa version script ELF. | La ruta armGBA Windows no está construida. La fuente local declara GPL-2; revisar compatibilidad de distribución frente a licencia raíz GPL-3.0-only antes de publicar binarios. |
| DS (DSperate libretro) | DLL `pokedaisy_ds_libretro.dll` compilada con MinGW x86_64, JIT/GLES/Vulkan desactivados; overlays Windows están en `linux/libretro-ds/windows_compat` y el submódulo DSperate permanece intacto. | Probe Rust: `loadable`, sistema DS, x86_64, API 1, exports libretro completos; build reproducible nuevo SHA-256 `f83a197bb8f716eaf5f259d80af34fb3adec1852a944c3e3216adbf553c055f3`. CTest Windows pasó 9/9: contrato libretro con ROM diagnóstica (video, SRAM, RAM 4 MiB, serialize/unserialize), settings, layouts, Companion, battle strip/parity y mmap. Player SDL abrió la ROM con `--emulator`, mostró el framebuffer verde de diagnóstico en `Vyntra Desktop`, se mantuvo `Responding=True` y cerró por WM_CLOSE; captura local ignorada en `desktop-rust/target/t07-ds-win/player-window.png`. ROM SHA-256 `841806d5e59c8088fe4f02ad2813378083ddd37b931c96ce50a868639a73f9b9`. Las pruebas de settings usan rutas temporales portables; el reemplazo atómico usa `MoveFileExA` en Windows. | DSperate es el backend DS integrado, no melonDS. El Player smoke no valida input; se usó ROM diagnóstica generada localmente. Falta procedencia/licencias de artefactos para distribución. GLES/Vulkan permanecen apagados en este port Windows. |

Los scripts `linux/dsperate/gen_data.py` y `gen_guide.py` ahora especifican UTF-8 al leer/escribir. Esto corrige el fallo del generador al leer Kotlin UTF-8 bajo la página de códigos Windows. Ambos generadores se ejecutaron con salidas temporales y finalizaron correctamente; el generador de datos informó que coincide con las declaraciones Kotlin.

Evidencia adicional GBA input (commit `b295334bd5e4e313332de9099d855c5660d620c1`): `configured_local_gba_core_receives_a_button_from_libretro_input` ejecuta `linux/make_diagnostic_gba_input.py` contra la DLL local y obtiene el pixel rojo `[0,248]` sin A y verde `[192,7]` con A; `configured_keyboard_bindings_map_to_libretro_buttons` verifica que Z se asigna a A. Core SHA-256 `5b2c7819a12d9cee28eada6c91f69e72377cff469c4d55df9f2c211d2d4359fd`; ROM input SHA-256 `11d8dcec6d21371a8754d1d0a0682dc5da6fbbd20af928123395651ee2d855`. Esto comprueba el core por API y la función de mapeo, pero no una tecla real dentro de la ventana SDL.

E2E de teclado Player (commit `d7002b9a668a3c67e5989661b6f47cd0bdb07bae`): `desktop-rust/scripts/test-gba-player-input.ps1` genera la ROM, entrega mensajes `WM_KEYDOWN/UP` para Z a la ventana SDL, cierra Player y valida que SRAM de 32 KiB contiene el marcador `0xA5` escrito por el juego y que el manifiesto committed coincide con hashes de payload, ROM y DLL. Ejecución local pasó; core SHA-256 `5b2c7819a12d9cee28eada6c91f69e72377cff469c4d55df9f2c211d2d4359fd`, ROM SHA-256 `c28ffeb7537151dc6115e0e3fdec46c8947d41ca2170ce7c3bf4aceb790cd7a3`, save SHA-256 `bcfecfa4991900f13858e91d9c83de221819ea3a17d553fcd2483e416bfea384`. Los mensajes se inyectan en HWND; no es una pulsación física de teclado.

## Fuentes y límites

## Pipeline reproducible y hashes actuales

`desktop-rust/scripts/build-vyntra-cores-windows.ps1` (commit `9845c758b2d6d131918859a20f6ae3805c23041f`) construye los tres targets en `desktop-rust/target/windows-cores`, ejecuta CTest y prueba los DLL con `--probe-core`. Toolchain: WinLibs GCC 16.2.0, CMake 4.4.3, Ninja 1.13.2; CTest DS 9/9 y los probes GBC/GBA/DS pasaron. Fuentes fijadas: mGBA `26b7884bc25a5933960f3cdcd98bac1ae14d42e2`, wrapper DS `801a2dc2bd9ddc6ec66eefd7b8f621d1d4b8adb6`, DSperate `99618eb956c66083618395dbae9bb5cbf0241d16`.

Los DLL de esta ejecución: GBC/GBA SHA-256 `fee4752b7c1e1469c78d1e8b1bb538a5f752221e07ed0879290181d22f6374bd`; DS SHA-256 `f83a197bb8f716eaf5f259d80af34fb3adec1852a944c3e3216adbf553c055f3`. La ROM GBA de input pasó por API y Player SDL; el juego escribió SRAM marcador `0xA5`, verificada junto con el manifiesto committed. Se genera `provenance.json` en el directorio local ignorado. Los binarios no se suben ni se consideran listos para distribución; el detalle de procedencia está en `docs/VYNTRA_CORE_PROVENANCE.md`.

Submódulos observados en los commits fijados del checkout: mGBA `26b7884` (MPL-2.0), armGBA `f83c373` (GPL-2 identificada en su LICENSE local), DSperate-lr `801a2dc` (GPL-3). La presencia de un archivo LICENSE no sustituye revisión de composición, obligaciones de distribución ni procedencia de DLL. La DLL GBC se generó solo bajo `desktop-rust/target`; no se añadió al repositorio ni se redistribuyó.

## Comandos de verificación

La configuración GBA/DS se realizó con CMake/Ninja en directorios ignorados bajo `desktop-rust/target`. El diagnóstico GBA con JIT desactivado:

```powershell
cmake -S libretro/armgba -B desktop-rust/target/t07-gba-win -G Ninja -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_C_COMPILER=x86_64-w64-mingw32-gcc -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-g++ '-DCMAKE_POLICY_VERSION_MINIMUM=3.5' -DARMGBA_JIT=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build desktop-rust/target/t07-gba-win --target armgba_libretro --parallel 8
```

La configuración termina; el build reproducido falla en `gba.c:16` por la cabecera `sys/mman.h` ausente en MinGW. Esta prueba no cambia el submódulo ni sus archivos fuente.

Para el smoke visual GBA, desde `desktop-rust/` con la DLL ya compilada:

```powershell
$env:VYNTRA_CORE_DIR = (Resolve-Path target/t07-gbc-win).Path
target/debug/vyntra-desktop.exe --emulator --data target/t07-gba-player-smoke-3 diagnostic.gba
```

La ventana SDL mostró `diagnostic.gba` (captura local ignorada `target/t07-gba-player-window.png`) y el proceso respondió durante la ejecución. Al salir, `data/saves/by-id/5d43b3e4a9954f7b/manifest.json` quedó `committed`, 32 KiB, con los SHA-256 de ROM y core indicados. Este es un smoke GBA standalone, sin pipe Launcher/Companion ni prueba de mandos/teclado.

Reproducción del build DS en el checkout actual:

```powershell
cmake -S linux/libretro-ds -B desktop-rust/target/t07-ds-win -G Ninja -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_C_COMPILER=x86_64-w64-mingw32-gcc -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-g++ -DCMAKE_BUILD_TYPE=Release
cmake --build desktop-rust/target/t07-ds-win --target pokedaisy_ds_libretro --parallel 8
python linux/dsperate/gen_data.py --output $env:TEMP/vyntra-t07-ds/data_gen.h
python linux/dsperate/gen_guide.py app/src/main/kotlin/com/pokedaisy/app/companion/data $env:TEMP/vyntra-t07-ds/guide_gen.h
```

El build se detiene primero en la memoria del submódulo (`page_table.cpp`, `sys/mman.h`; `page_table.h`, `std::aligned_alloc`). El log completo de esta ejecución quedó en `desktop-rust/target/t07-ds-win-build.log`, ignorado por Git.

Build GBC reproducible en el checkout actual:

```powershell
cmake -S libretro -B desktop-rust/target/t07-gbc-win -G Ninja -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_C_COMPILER=x86_64-w64-mingw32-gcc '-DCMAKE_POLICY_VERSION_MINIMUM=3.5' -DCMAKE_BUILD_TYPE=Release
cmake --build desktop-rust/target/t07-gbc-win --target pokedaisy_gbc_libretro --parallel 4
cmake --build desktop-rust/target/t07-gbc-win --target pd_fault_guard_test --parallel 2
desktop-rust/target/t07-gbc-win/pd_fault_guard_test.exe
```

Desde `desktop-rust/`, `cargo run -p vyntra-desktop -- --probe-core target/t07-gbc-win/pokedaisy_gbc_libretro.dll GBC` comprueba ABI y exports. El smoke requiere `VYNTRA_CORE_SMOKE_PATH` con ruta absoluta al DLL y `VYNTRA_CORE_SMOKE_ROM` con ruta absoluta a `diagnostic.gb`, seguido de `cargo test -p vyntra-desktop --test libretro_av configured_local_gbc_core_runs_generated_diagnostic_rom -- --ignored --nocapture`.

Una repetición desde el checkout con MinGW x64 (`cmake -S linux/libretro-ds -B desktop-rust/target/t07-ds-win -G Ninja -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_C_COMPILER=x86_64-w64-mingw32-gcc -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-g++ -DCMAKE_BUILD_TYPE=Release`) confirmó que la configuración termina. El target `pd_ds_presentation` compila y enlaza sus fuentes C/C++ compartidas para Windows. `companion_ui.cpp` y `guide_view.cpp` usan ramas Windows para crear/sincronizar settings y leer hora local. El target final se detiene en el submódulo fijado: `page_table.cpp` (`sys/mman.h`) y `page_table.h` (`std::aligned_alloc`). Una prueba temporal del cambio de allocator/page table dejó ver bloqueos posteriores de ROM/caché POSIX (`rom_source.cpp`, `zip_cache.cpp`); no se guardaron cambios dentro del submódulo externo. El enlace ELF sigue pendiente, así que todavía no hay DLL. El árbol de build está bajo `desktop-rust/target` y no forma parte del repositorio.

T07 queda **HECHO-PARCIAL**: el pipeline local reproduce builds mGBA GBC/GBA y DSperate DS; los tres pasaron probes y Player SDL, y DS pasó CTest 9/9. GBC/GBA usan la misma DLL y el flujo GBA de input comprobó tecla Z→A hasta el marcador SRAM `0xA5`. armGBA separado sigue bloqueado por POSIX y requiere revisión GPL-2. Faltan Companion GBA, auditoría de avisos/licencias y procedencia para distribución; no afirmar soporte general ni distribuir estos DLL aún.
