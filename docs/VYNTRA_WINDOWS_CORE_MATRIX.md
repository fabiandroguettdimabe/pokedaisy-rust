# Vyntra Desktop: matriz de viabilidad de cores en Windows

Actualizado: 2026-11-17. Hito T07 / WIN-CORE-02, build y smoke GBC en Windows x64.

## Entorno reproducido

- Windows x64, WinLibs GCC 16.2 POSIX/UCRT/SEH (no MSYS2), CMake 4.4.3, Ninja 1.13.2 y Python 3.13.15.
- Configuración CMake para GBC, GBA y DS finaliza correctamente usando `-DCMAKE_POLICY_VERSION_MINIMUM=3.5`.
- No se encontraron clang/clang-cl. El build GBC y sus artefactos temporales están en `desktop-rust/target/t07-gbc-win`, ignorado por Git.

## Resultados

| Core objetivo | Configuración | Primer bloqueo de build | Estado |
|---|---|---|---|
| GBC (mGBA adaptado) | Build Release `pokedaisy_gbc_libretro.dll` con WinLibs GCC 16.2.0 / CMake 4.4.3 / Ninja 1.13.2; mGBA fijado en `26b7884bc25a5933960f3cdcd98bac1ae14d42e2`. | Adaptación `_mkdir`, exports PE explícitos y guard Windows de fallos mediante vectored exception handler. Probe Rust: `loadable`, sistema GBC, ABI 1, 22 exports requeridos y ninguno ausente. `configured_local_gbc_core_runs_generated_diagnostic_rom`: 120 frames, video/audio, SRAM marker `0x5a` y serialize/unserialize. Core SHA-256 `5b2c7819a12d9cee28eada6c91f69e72377cff469c4d55df9f2c211d2d4359fd`; ROM diagnóstica original de `linux/make_diagnostic.py`, SHA-256 `90d49d61eb62c365b355d5c4182da9e88f89fa0d773e0365c83db7bcb15a1000`; verificado en commit `6f8c1e6b373660057c00fa09315136f7a081b56d`. | DLL PE cargable y smoke API aprobados. No prueba Player SDL/input en ventana ni distribución del binario. |
| GBA (mGBA dentro de la DLL PokeDaisy compartida) | `pokedaisy_gbc_libretro.dll` se descubre como fallback GBA tras los nombres GBA dedicados; selección y apertura verificadas con `VYNTRA_CORE_DIR`. | Probe: `loadable`, sistema GBA, x86_64, API 1, 22 exports. `configured_local_core_runs_generated_gba_diagnostic_rom`: carga, geometría mínima, video por 120 cuadros y serialize/unserialize. Player SDL `--emulator` abre la ROM y muestra el patrón GBA en ventana 1016×699; `Responding=True`; al cerrar quedan SRAM de 32 KiB y manifiesto `committed` con hashes de ROM/core. Core SHA-256 `5b2c7819a12d9cee28eada6c91f69e72377cff469c4d55df9f2c211d2d4359fd`; ROM SHA-256 `5d43b3e4a9954f7b5b9bdac2dd954e9f01defeadfc39355562b593dcdf319582`; test `462c412440eaba1f89847a077e20b7d2e19b25db`; descubrimiento `8c38c946866048505f7042e5ba88d78bc5a9c86d`. | ROM homebrew verificada en API y Player SDL. No valida input, Companion/lectura GBA live, cambios de SRAM escritos por el juego ni armGBA. No se empaqueta una DLL duplicada. |
| GBA (armGBA) | CMake configura con `ARMGBA_JIT=OFF` | El build se detiene en `third_party/armgba/src/gba.c:16`: `fatal error: sys/mman.h: No such file or directory`. Apagar JIT no elimina el mapeo POSIX usado para abrir ROMs. También hay usos `mmap`/`munmap` en `cpu/jit.c` y dependencias libretro-common (VFS y `archive_file.c`); aún no se evaluaron todos los siguientes errores de compilación. El enlace usa version script ELF. | La ruta armGBA Windows no está construida. La fuente local declara GPL-2; revisar compatibilidad de distribución frente a licencia raíz GPL-3.0-only antes de publicar binarios. |
| DS (DSperate libretro) | CMake configura; JIT desactivado para AMD64 | Rebuild del checkout actual falla en `page_table.cpp:10` por `sys/mman.h` y en `page_table.h:54` porque `std::aligned_alloc` no está disponible en este toolchain. El código del proyecto usa `_mkdir`/`_commit` para settings y `localtime_s` en Windows. Una prueba temporal de port del allocator/page table mostró luego APIs POSIX adicionales en `rom_source.cpp` y `zip_cache.cpp`, sin conservar cambios en el submódulo. El enlace también conserva exports ELF `--version-script`. | Portar allocator/memoria mapeada/ROM/caché en una fuente controlada y exports PE; aún no hay DLL. No es melonDS. |

Los scripts `linux/dsperate/gen_data.py` y `gen_guide.py` ahora especifican UTF-8 al leer/escribir. Esto corrige el fallo del generador al leer Kotlin UTF-8 bajo la página de códigos Windows. Ambos generadores se ejecutaron con salidas temporales y finalizaron correctamente; el generador de datos informó que coincide con las declaraciones Kotlin.

## Fuentes y límites

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

T07 queda **HECHO-PARCIAL**: la misma DLL PE mGBA/PokeDaisy está verificada por API para GBC y GBA con ROMs diagnósticas originales; GBC además cubre audio y SRAM, y GBA cubre video, estados y ventana Player SDL con SRAM comprometida. No se compila ni distribuye una copia duplicada bajo nombre GBA. armGBA separado sigue bloqueado por POSIX y licencia GPL; DS por POSIX en el submódulo. Faltan input Player, Companion GBA, cerrar DS y verificar distribución/procedencia; no afirmar soporte general de los tres sistemas ni distribuir esta DLL aún.
