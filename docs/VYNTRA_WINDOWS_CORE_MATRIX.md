# Vyntra Desktop: matriz de viabilidad de cores en Windows

Actualizado: 2026-11-17. Hito T07 / WIN-CORE-02, spike de compilación en Windows x64.

## Entorno reproducido

- Windows x64, WinLibs GCC 16.2 POSIX/UCRT (no MSYS2), CMake 4.4.3, Ninja 1.13.2 y Python 3.13.15.
- Configuración CMake para GBC, GBA y DS finaliza correctamente usando `-DCMAKE_POLICY_VERSION_MINIMUM=3.5`.
- No se encontraron clang/clang-cl ni DLLs construidas en este spike. Las configuraciones y artefactos de diagnóstico se dejaron en `%LOCALAPPDATA%/Temp/vyntra-t07-*`, fuera del repositorio.

## Resultados

| Core objetivo | Configuración | Primer bloqueo de build | Estado |
|---|---|---|---|
| GBC (mGBA adaptado) | CMake configura | `libretro/pd_libretro.c`: llamada POSIX `mkdir(path, mode)` no coincide con la API Windows de un argumento. El enlace también usa `--version-script` ELF. | No hay DLL ni ABI validada. Requiere capa Windows y exports PE. |
| GBA (armGBA) | CMake configura | `sys/mman.h` no existe en MinGW; el JIT/recompiler usa APIs POSIX. El enlace usa version script ELF. | Portabilidad del JIT no resuelta. La fuente local declara GPL-2; revisar compatibilidad de distribución frente a licencia raíz GPL-3.0-only antes de publicar binarios. |
| DS (DSperate libretro) | CMake configura; JIT desactivado para AMD64 | El código del proyecto ya usa `_mkdir`/`_commit` para settings y `localtime_s` en Windows. El submódulo fijado falla con `sys/mman.h` en `page_table.cpp` y `std::aligned_alloc` en `page_table.h`; la prueba temporal de port del allocator/page table mostró luego APIs POSIX adicionales en `rom_source.cpp` y `zip_cache.cpp`, sin conservar cambios en el submódulo. El enlace también conserva exports ELF `--version-script`. | Portar allocator/memoria mapeada/ROM/caché en una fuente controlada y exports PE; aún no hay DLL. No es melonDS. |

Los scripts `linux/dsperate/gen_data.py` y `gen_guide.py` ahora especifican UTF-8 al leer/escribir. Esto corrige el fallo del generador al leer Kotlin UTF-8 bajo la página de códigos Windows. Ambos generadores se ejecutaron con salidas temporales y finalizaron correctamente; el generador de datos informó que coincide con las declaraciones Kotlin.

## Fuentes y límites

Submódulos observados en los commits fijados del checkout: mGBA `26b7884` (MPL-2.0), armGBA `f83c373` (GPL-2 identificada en su LICENSE local), DSperate-lr `801a2dc` (GPL-3). La presencia de un archivo LICENSE no sustituye revisión de composición, obligaciones de distribución ni procedencia de DLL. No se generaron ni redistribuyeron binarios.

## Comandos de verificación

Las tres configuraciones se realizaron con CMake/Ninja en directorios temporales. Los builds focalizados fueron:

```powershell
cmake --build $env:LOCALAPPDATA/Temp/vyntra-t07-gbc --target pokedaisy_gbc_libretro --parallel 4
cmake --build $env:LOCALAPPDATA/Temp/vyntra-t07-gba --target pokedaisy_gba_libretro --parallel 4
cmake --build $env:LOCALAPPDATA/Temp/vyntra-t07-ds --target pokedaisy_ds_libretro --parallel 4
python linux/dsperate/gen_data.py --output $env:TEMP/vyntra-t07-ds/data_gen.h
python linux/dsperate/gen_guide.py app/src/main/kotlin/com/pokedaisy/app/companion/data $env:TEMP/vyntra-t07-ds/guide_gen.h
```

Una repetición desde el checkout con MinGW x64 (`cmake -S linux/libretro-ds -B desktop-rust/target/t07-ds-win -G Ninja -DCMAKE_SYSTEM_NAME=Windows -DCMAKE_C_COMPILER=x86_64-w64-mingw32-gcc -DCMAKE_CXX_COMPILER=x86_64-w64-mingw32-g++ -DCMAKE_BUILD_TYPE=Release`) confirmó que la configuración termina. El target `pd_ds_presentation` compila y enlaza sus fuentes C/C++ compartidas para Windows. `companion_ui.cpp` y `guide_view.cpp` usan ramas Windows para crear/sincronizar settings y leer hora local. El target final se detiene en el submódulo fijado: `page_table.cpp` (`sys/mman.h`) y `page_table.h` (`std::aligned_alloc`). Una prueba temporal del cambio de allocator/page table dejó ver bloqueos posteriores de ROM/caché POSIX (`rom_source.cpp`, `zip_cache.cpp`); no se guardaron cambios dentro del submódulo externo. El enlace ELF sigue pendiente, así que todavía no hay DLL. El árbol de build está bajo `desktop-rust/target` y no forma parte del repositorio.

T07 queda **HECHO-PARCIAL**: la matriz identifica bloqueos concretos y los generadores ya toleran UTF-8 en Windows. El DoD de DLL reproducible, exports PE y smoke con fuente/procedencia validada sigue pendiente.
