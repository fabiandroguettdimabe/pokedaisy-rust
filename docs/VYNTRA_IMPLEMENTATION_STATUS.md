# Estado de implementación de Vyntra

Actualizado: 2026-11-16. Este documento registra el comportamiento comprobado del checkout; una función no se considera disponible por aparecer en una imagen de referencia o una etiqueta de interfaz.

## Cambios implementados en este checkout

- Desktop se muestra como **Vyntra Desktop** y usa la paleta Dark Crimson + Black en su interfaz SDL. El símbolo V se dibuja como vector en SDL; las variantes SVG original, reducida y monocromática están en `desktop-rust/assets/`.
- Android muestra **Vyntra Pocket** como nombre de aplicación. Se conservan `applicationId` y `namespace` `com.pokedaisy.app` para no romper actualización in-place ni acceso a los datos existentes.
- Los paquetes Rust propios se llaman `vyntra-desktop` y `vyntra-libretro`; las rutas en disco aún conservan `crates/pokedaisy-*`.
- Desktop prefiere `VYNTRA_CORE_DIR`; conserva `POKEDAISY_CORE_DIR` como alias legado y solo escribe el aviso de deprecación en builds debug. Busca primero nombres `vyntra_*_libretro` y mantiene los nombres `pokedaisy_*_libretro` como fallback.
- Ajustes distingue `No instalado` de `Archivo detectado · sin probar`. La DLL se carga al iniciar un juego mediante `LibretroCore::load`; errores de exportaciones/API/carga se notifican en ese flujo. Un archivo presente no se anuncia como listo.
- La pantalla Companion de Desktop informa explícitamente qué paneles aún no están implementados.

## Matriz de pantallas y controles

| Área | Estado actual | Código / fuente | Verificación disponible |
|---|---|---|---|
| Desktop · Biblioteca | Parcial: recorre `data/roms`, reconoce por extensión GB/GBC/GBA/NDS, permite diálogo de ROM, arrastrar, selección por teclado, doble clic y reescaneo. Sin indexador, hash, importación ZIP, filtros ni persistencia de preferencias. | `desktop-rust/crates/pokedaisy-desktop/src/main.rs` | Build release; revisión del flujo |
| Desktop · Sistemas | Parcial: sistema derivado de extensión y se puede iniciar el juego seleccionado; no hay filtros ni contadores por sistema. | `desktop-rust/crates/pokedaisy-desktop/src/main.rs` | Revisión del código |
| Desktop · Core Manager | Parcial: muestra ausente/detectado sin ejecutar la DLL; el intento real ocurre al iniciar un juego y verifica ABI/exportaciones/API libretro. No hay selector de ruta ni botón de diagnóstico independiente. | `desktop-rust/crates/pokedaisy-desktop/src/main.rs`, `desktop-rust/crates/pokedaisy-libretro/src/lib.rs` | Pruebas unitarias de nombres y precedencia; sin DLL instalada para integración |
| Desktop · Play | Parcial: libretro carga ROM, entrega vídeo/audio/input y puede persistir RAM de guardado; no se probó con una ROM/core en este entorno. | `desktop-rust/crates/pokedaisy-desktop/src/main.rs`, `desktop-rust/crates/pokedaisy-libretro/src/lib.rs` | Build release; juego E2E no probado |
| Desktop · Estados / FF | Parcial: atajos F2/F4 y Space existen; compatibilidad depende del core. No hay vista Vault ni metadatos de compatibilidad. | `desktop-rust/crates/pokedaisy-desktop/src/main.rs`, `desktop-rust/crates/pokedaisy-libretro/src/lib.rs` | Revisión del código; sin core para E2E |
| Desktop · Acompañante | No disponible; la pantalla informa el estado real y no presenta datos simulados. | `desktop-rust/crates/pokedaisy-desktop/src/main.rs` | Revisión de la interfaz |
| Desktop · Ajustes | Parcial: muestra rutas de datos/núcleos y estado de detección; no persiste preferencias ni configura audio/vídeo/gamepad. | `desktop-rust/crates/pokedaisy-desktop/src/main.rs` | Revisión del código |
| Desktop · Sync / Link | No disponible en este frontend. | Sin implementación Rust correspondiente | No probado |
| Pocket · Biblioteca / Setup | Implementado en Android con selector SAF, biblioteca y persistencia existente. | `app/src/main/kotlin/com/pokedaisy/app/LibraryActivity.kt`, `SetupScreen.kt` | APK pendiente: compilación Kotlin/Compose activa; motores nativos arm64 compilados |
| Pocket · Emulación | Código existente integra motores nativos mGBA, armGBA y melonDS; no hay dispositivo ADB disponible aquí. | `app/src/main/kotlin/com/pokedaisy/app/EmulatorEngine.kt`, `MgbaCore.kt`, `app/src/main/cpp/` | Build y prueba en dispositivo no disponibles |
| Pocket · Equipo / Mochila / Combate / Dex / Atlas / Guía | Pantallas y adaptadores existentes por juego; la cobertura depende de la ROM y sus datos. No se volvió a verificar en runtime en este checkout. | `app/src/main/kotlin/com/pokedaisy/app/companion/ui/` y `companion/data/` | Pruebas y cobertura existentes por revisar; runtime pendiente |
| Pocket · Estados / Controles / DS / Link | Implementaciones existentes para estados, entradas, pantalla dual y co-op Android; capacidades varían por core y configuración. | `app/src/main/kotlin/com/pokedaisy/app/` y `companion/` | No revalidado en hardware durante esta sesión |
| Web / documentación | Parcial: web con logo Vyntra, paleta oscura y textos visibles traducidos; manual/repositorio aún conservan abundantes referencias históricas y algunos enlaces/IDs intencionales. | `website/src/`, `website/functions/`, `docs/manual/`, `website/scripts/` | 82/82 pruebas web y build estático de 146 páginas correctos |

## Compatibilidad de cores declarada por el árbol

| Sistema | Motor usado por el proyecto | DLL Desktop esperada | Estado en este entorno |
|---|---|---|---|
| GB/GBC | mGBA adaptado por el proyecto | `vyntra_gbc_libretro.dll` o alias `pokedaisy_gbc_libretro.dll` | No instalado |
| GBA | armGBA adaptado por el proyecto | `vyntra_gba_libretro.dll` o alias `pokedaisy_gba_libretro.dll` | No instalado |
| DS | melonDS adaptado por el proyecto | `vyntra_ds_libretro.dll` o alias `pokedaisy_ds_libretro.dll` | No instalado |

Los DLL estándar de RetroArch no se renombran ni se anuncian como compatibles. Los adaptadores desktop se deben compilar y validar por separado para Windows. En Android, los núcleos están integrados como bibliotecas nativas del APK.

## Builds y pruebas realizadas

- `cargo fmt --check` en `desktop-rust/`: inicialmente mostró formato pendiente; se ejecutó `cargo fmt`.
- `cargo test --workspace` en `desktop-rust/`: 2 pruebas pasaron para nombres nuevos/legados de core y prioridad de directorios.
- `cargo run --release` en `desktop-rust/`, desde Visual Studio Developer Command Prompt: compiló y ejecutó `vyntra-desktop.exe`.
- `:app:assembleDebug` con JDK 17: SDK 34, Build Tools 34, NDK 28.2 y CMake 3.22 instalados; submódulos nativos inicializados según sus commits fijados. Los motores C/C++ para `arm64-v8a` compilaron; Kotlin/Compose sigue en la fase de lowering IR, sin APK final aún. El daemon Kotlin ha usado alrededor de 6 GB y lleva más de 20 minutos en esta compilación, por lo que el resultado de Android queda pendiente.
- `node --test test/*.test.js` en `website/`: 82 pruebas pasaron. `npm run build`: 146 páginas generadas correctamente. La suite detectó y se corrigió el manejo de CRLF en front matter para Windows en `website/src/lib/docs.js` y `website/scripts/install-guides.mjs`; las guías derivadas se regeneraron.
- `adb` no está instalado y no hay dispositivo Android conectado; no se pudo realizar prueba de ejecución en hardware. No hay DLL de núcleos en `desktop-rust/cores/`.

## Referencias históricas conservadas

| Anterior | Nuevo | Estado | Motivo |
|---|---|---|---|
| `com.pokedaisy.app` (applicationId y namespace) | Visible: Vyntra Pocket | Conservado internamente | Mantiene actualización Android in-place y acceso al sandbox actual. |
| `POKEDAISY_CORE_DIR` | `VYNTRA_CORE_DIR` | Alias temporal | Compatibilidad con configuración Desktop existente; advertencia solo en build debug. |
| `pokedaisy_*_libretro.dll` | `vyntra_*_libretro.dll` | Fallback temporal | Instalar una versión nueva no exige renombrar los núcleos existentes. Se validan cargándolos al iniciar juego. |
| `crates/pokedaisy-*` | Paquetes Cargo `vyntra-*` | Rutas conservadas | Evita movimientos de archivos innecesarios; los nombres de paquete son internos. |
| `libpokedaisy` y módulos Kotlin `com.pokedaisy.app` | Pendiente de evaluar | Conservado internamente | Renombrar bibliotecas y paquetes puede romper JNI, migraciones o actualización de datos; la marca de interfaz ya puede cambiar por separado. |
