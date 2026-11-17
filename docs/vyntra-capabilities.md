# Matriz de capacidades y evidencia de Vyntra (baseline)

Actualizado: 2026-11-17. Este documento inicia la ejecución del plan técnico (`docs/VYNTRA_PLAN_TECNICO.md`, T01/QA-02). Es una matriz de **evidencia**, no una promesa de soporte.

## Convenciones

- **Detectado / presente:** el artefacto, código o ruta existe en el checkout o fue observado.
- **Probado / verificado:** existe evidencia de ejecución reproducible para un `build/commit/coreHash/ROM identity o fixtureId/testId/fecha` concretos. Sin esa evidencia, el estado es **no verificado** o **pendiente**.
- **Fuente:** referencia a E01–E18 del plan técnico, rutas de código o resultado de comando registrado.
- Una capacidad de emulación estándar, una extensión propia y un adaptador Companion son dimensiones distintas y no se infieren entre sí.

## Estado global (checkout inspeccionado)

| Área | Detectado/presente | Probado en esta sesión | Evidencia | Notas / bloqueo |
|---|---|---|---|---|
| Plan técnico y estado | Sí | n/a (documental) | `docs/VYNTRA_PLAN_TECNICO.md`, `docs/VYNTRA_IMPLEMENTATION_STATUS.md` | Punto de partida para Fase 0/F1. |
| Desktop Rust (launcher + player SDL/libretro) | Sí | Parcial (compilación/tests crate) | E01–E06, `desktop-rust/crates/pokedaisy-desktop`, `desktop-rust/crates/pokedaisy-libretro` | `cargo fmt --check`, `cargo check --workspace` y `cargo test -p vyntra-desktop` OK (2026-11-16); `cargo test --workspace` no completó (timeout). Runtime/emulación/UI no probados. |
| UI web (Preact/Tailwind) embebida en shell | Sí | No | E05, `desktop-rust/ui/` | `npm.cmd` 11.19.0 disponible, pero `npm.ps1` bloqueado por ExecutionPolicy en PowerShell; Node v24.20.0 y Python 3.12.10 presentes. `npm test && npm run build` pendientes. |
| Núcleos libretro locales (DLL) | Sí (6 archivos) | No (carga/ABI/runtime no acreditados aquí) | E01.4 / hashes en plan, `desktop-rust/cores/` | Identidad binaria por hash ≠ procedencia, seguridad ni compatibilidad. GBA propio ausente. |
| Android (Compose/JNI, emulación, companion) | Sí | No | E07–E13, `app/` | `gradlew.bat` no observado en raíz del checkout inspeccionada; toolchain/dispositivo no acreditados en esta sesión. |
| Gamedata / esquemas / generadores | Sí (parcial) | No | E14, `gamedata/`, `scripts/gen_gamedata.py` | Fuente TOML; salidas generadas no deben editarse a mano. |
| Website/manual y pipeline docs | Sí | No | `website/`, `docs/manual/` | `npm.cmd test && npm.cmd run build` futuros para baseline. |
| Referencia visual 2 (multiventana) | No localizada | No | Plan §2 | Bloqueada hasta recibir archivo original. |

## Toolchains / entorno observado (2026-11-16, máquina de trabajo)

| Herramienta | Observado | Nota |
|---|---|---|
| Rust / Cargo | `rustc 1.100.0`, `cargo 1.100.0` | Verificación Desktop parcial realizada en `desktop-rust/`. |
| Node.js | `v24.20.0` | Disponible. |
| npm | `npm.cmd` 11.19.0 | `npm` (ps1) bloqueado por ExecutionPolicy en PowerShell 5.1; preferir `npm.cmd`. |
| Python | `3.12.10` | Disponible para scripts/generadores. |
| Gradle wrapper (raíz) | No observado (`gradlew.bat`) | No impide inspección Android, pero baseline de build/test Android queda pendiente de entorno. |

## Checks baseline documentados (pendientes de ejecución/evidencia)

| Área | Comando futuro (según plan) | Estado en esta sesión |
|---|---|---|
| UI Desktop | `npm.cmd ci; npm.cmd run build; npm.cmd test` en `desktop-rust/ui` | **OK (2026-11-16)**: `npm ci` (33 paquetes), `npm test` 1/1, `npm run build` OK. `dist/` regenerado y conservado por el cambio de `src/app.jsx` (contexto de sesión en comandos). |
| Rust Desktop | `cargo fmt --all --check; cargo check --workspace; cargo test --workspace` en `desktop-rust` | **OK (2026-11-16)**: fmt/check OK; `cargo test --workspace` = 19 tests `vyntra-desktop` (0 fallos), 0 en `vyntra-libretro`, 0 doc-tests. Warnings `dead_code` de scaffolds pendientes de integrar. |
| Android | `gradlew.bat :app:testDebugUnitTest --stacktrace; :app:assembleDebug --stacktrace` | No ejecutado (wrapper/toolchain no verificado) |
| Web/manual | `npm.cmd test; npm.cmd run build` en `website/` (y pipeline manual si aplica) | **Parcial (2026-11-16)**: `npm ci` OK (229 paquetes); `npm run build` OK (146 páginas) tras corregir un ancla rota del rebrand; `npm test` 81/82 (fallo preexistente de guías de instalación desactualizadas; arreglo: `node website/scripts/install-guides.mjs`). |
| Generadores gamedata | `python scripts/gen_gamedata.py` (+ validación de no diff esperado) | Ejecutó y regeneró salidas; los diffs generados se revirtieron (sin validar equivalencia funcional). |

## Matriz por plataforma / sistema / núcleo (baseline inicial)

| Plataforma | Sistema | Núcleo/ruta observada | Arranque A/V/input | SRAM/estados/FF/rewind | Companion / datos live | Link | Evidencia | Próxima verificación |
|---|---|---|---|---|---|---|---|---|
| Windows | GB | DLL pública/alias local (mGBA grupo) | No verificado | Parcial en código (SRAM/state/FF); rewind pendiente | No integrado en Desktop | No probado | E02,E03,E06, cores locales | T06/T08/T09 + fixture/homebrew legal |
| Windows | GBC | DLL pública/alias local (mGBA grupo) | No verificado | Parcial en código | No integrado en Desktop | No probado | Igual base | No asumir GB=GBC probado |
| Windows | GBA | armGBA (fuente) / DLL propia ausente | Bloqueado | No verificado | Bloqueado | No probado | E15 | T07/T06 + artefacto/probe |
| Windows | DS | melonDS DS público local; ruta Linux distinta (DSperate) | No verificado | No verificado (BIOS/opciones/estados por motor) | No integrado en Desktop | No probado | E06,E15 | Acreditar motor exacto y capacidades |
| Android | GB/GBC | mGBA JNI | Parcial (código) | Parcial (código) | Parcial por identidad (lectores) | No demostrado | E07–E12 | Dispositivo/ABI + fixtures |
| Android | GBA | mGBA JNI; configs Gen3 | Parcial (código) | Parcial (código) | Parcial/extenso por juego | Especializado con flags (E16) | E07–E13,E16 | No asumir armGBA Android |
| Android | DS | melonDS y DSperate en JNI/CMake | Parcial (código) | Parcial (segregado por core) | Parcial Gen4/5 por versión | Pendiente | E07,E10–E12,E16 | Validación hardware + core exacto |

## Capacidades Desktop observadas en código (no runtime)

| Capacidad | Estado observado | Fuente | Riesgo / nota (plan) |
|---|---|---|---|
| Escaneo/importación básica por extensión | Parcial (mejorado) | `main.rs` (`scan_games`, `supported`), `shell.rs` import, `library.rs` | Catálogo nuevo con `id`/`content_hash`/`title`/`system`/`last_seen_unix` (carga compatible con array legacy de rutas, guardado atómico); saves/states por hash de contenido en `by-id/`. Faltan ZIP, validación fuerte y migración/compactación del catálogo. |
| Selección de núcleo por nombre/dirs/alias legado | Parcial | `main.rs` (`find_core`, `core_names`), `settings.rs` | Probe/ABI no acreditados al detectar archivo. |
| Carga ROM/core, SRAM, savestates F2/F4, FF (Space) | Parcial (mejorado) | `main.rs`, `vyntra-libretro` | Saves/states aislados por hash de ROM (`by-id/<hash16>/`) con fallback de lectura legacy, escritura atómica, logging de errores SRAM y manifiestos por tipo (`manifest.json` SRAM, `state.manifest.json` estado) con bytes/sha256/`core_hash`/`state_format`. Falta sesión inmutable end-to-end y validación runtime. |
| Proceso player separado + shell WebView (Windows) | Parcial (mejorado) | `shell.rs`, `protocol.rs`, `ui/src/app.jsx` | Parsing de acciones vía `parse_legacy_action`, emisión de `UiAck` (`window.vyntraAck`), contexto de sesión/epoch/revisión en estado publicado, validación de `session_id`/`epoch` en comandos, UI que adjunta esos campos y consume ACKs fallidos. Falta sesión continua player↔shell y ACK con resultado de dominio. |
| Ajustes persistentes JSON + backup `.bak` | Parcial | `settings.rs` | No transaccional para catálogo/saves; schema básico. |

## Baseline adicional ejecutado (2026-11-16)

| Área | Comando | Resultado | Nota |
|---|---|---|---|
| Generadores gamedata | `python scripts/gen_gamedata.py` | Ejecutó y reportó generación de configs/registros/salidas | Modificó archivos generados en working tree; se revirtieron tras la prueba para no introducir diffs no solicitados. Equivalencia de contenido no validada. |

## Registro de ejecuciones

Las filas siguientes son verificaciones adicionales posteriores al baseline del 2026-11-16. El commit identifica el checkout evaluado; los resultados negativos o incompletos también se conservan para evitar inferir soporte.

| Fecha | Build/commit | Plataforma | Sistema/core hash | ROM identity o fixtureId | Prueba (testId/acción) | Resultado | Limitación |
|---|---|---|---|---|---|---|---|
| 2026-11-17 | `87f51fc` (código T19: límites `select`/`disconnect` y espacios de dirección) | Windows x86_64 GNU / Desktop Rust | Core sintético `av_core_fixture.c` | Party sintética GBA, identidad exacta de fixture | `synthetic_libretro_map_flows_through_gen3_party_adapter_and_poller` (rechazo `select`/`disconnect` y selección exacta de `addrspace`); `cargo fmt --manifest-path desktop-rust/Cargo.toml --all -- --check`; `cargo test --manifest-path desktop-rust/Cargo.toml --workspace --target x86_64-pc-windows-gnu --message-format short` | Formato OK; suite Desktop 171 passed, 0 failed, 4 ignored; `core_probe` 5 passed/1 ignored; Libretro A/V 1/1; `vyntra-libretro` 4/4 | Prueba contrato y decoder con bytes sintéticos; no acredita perfil de juego ni datos live. Registro de producción vacío. |
| 2026-11-17 | `6c02a60` | Web/manual | No aplica | Corpus de pruebas del sitio | `node --test test/*.test.js` en `website/` | 82/82 passed | No incluye build en esta ejecución; el build anterior generó 146 páginas. |
| 2026-11-17 | `b7dfa45` | Docs/QA | No aplica | Matriz `docs/vyntra-capabilities.md` | `ready capabilities require a reproducible test ID and commit`; `a malformed capability row cannot hide a ready status`; `the checked-in capability matrix has no ready claim without test evidence`; `each capability column accepts evidence scoped to its own cell`; `node --test test/*.test.js` | Suite web 86/86 passed; los fixtures verifican evidencia por celda, rechazan evidencia de otra celda y filas malformadas; se exige `cell=<capacidad>`, `testId` y commit. | Solo valida estados de capacidades; aún falta evidencia granular por celda. |
| 2026-11-17 | `6c02a60` | Android Pocket | NDK 28.2.13676358 | `RomIntakeTest`, `SaveStatesTest` | `:app:testDebugUnitTest --no-daemon` | No ejecutado: Gradle detuvo la configuración en `LicenceNotAcceptedException` | No se aceptó la licencia solicitada; sin resultado de compilación Kotlin, firma ni validación en dispositivo. |

## Próximos pasos inmediatos (alineados al plan)

1. **T01/QA-02:** mantener el inventario por feature y añadir una fila por ejecución reproducible; el test integrado rechaza capacidades Ready sin `testId`+commit, pero falta evidencia granular por celda.
2. **T02/INFRA-01:** introducir contratos de dominio/protocolo versionados en Desktop (`domain.rs`, `protocol.rs`) sin romper el flujo actual.
3. **T03/T05/T08:** abordar identidad por hash, guardados atómicos/aislados y sesión inmutable (P0) con fixtures sintéticos antes de ampliar UI.
4. **Baseline de checks:** el sitio web y Desktop Rust tienen verificaciones recientes registradas; Android sigue pendiente de licencia NDK y entorno de hardware/firma, y el baseline completo de generadores aún requiere repetir equivalencia sin dejar diffs.
