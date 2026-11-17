# VYNTRA — PLAN TÉCNICO DE IMPLEMENTACIÓN (SOLO DOCUMENTACIÓN)

Fecha de auditoría: 2026-11-16. Destinatario: desarrollador o equipo que implementará posteriormente.

**Modo de auditoría y planificación: no se ha modificado ningún archivo ni se ha implementado código durante la inspección técnica.** Después de esa inspección, el usuario autorizó expresamente guardar este informe en Markdown y subir los cambios pendientes a GitHub. Esa autorización permite este documento y la publicación del trabajo previo; no convierte las propuestas en implementación ni autoriza a afirmar pruebas que no se ejecutaron.

Se leyó el documento VYNTRA_PROMPT_PLANIFICACION_V3_EXPANDIDA_CODEX.md y se inspeccionaron fuentes, manifiestos, pruebas, CI, Git e imágenes disponibles. No se ejecutaron compilaciones, tests, instalaciones, generadores ni cargas de núcleos durante esta auditoría. Los comandos de validación de este informe son futuros.

Convenciones: **[EXISTENTE-VERIFICADO]** significa encontrado en código, no demostrado en ejecución; **[EXISTENTE-PARCIAL]**, integración incompleta; **[PROPUESTO]**, trabajo futuro; **[NO-VERIFICADO]**, evidencia insuficiente; **[BLOQUEADO]**, dependencia pendiente; **[DEUDA-TÉCNICA]**, riesgo del diseño observado. Las especificaciones, contratos, diagramas y criterios de aceptación son propuestas, salvo indicación expresa de evidencia existente. Todos los nombres nuevos de módulos y funciones se consideran **[ARCHIVO NUEVO PROPUESTO]** y **[SÍMBOLO PROPUESTO]**, respectivamente, y se identifican en el catálogo de rutas.

## 1. Resumen ejecutivo y arquitectura existente

Vyntra no necesita una reescritura completa. Desktop dispone de un launcher Wry/Tao con Preact/Tailwind, un reproductor SDL/libretro en proceso secundario y configuración persistente. Android tiene una base considerablemente más avanzada: Compose, JNI, emulación integrada, lectores de memoria por juego, datos generados, guardados, controles y funciones de acompañante. La estrategia es estabilizar el circuito de emulación y guardados de Desktop, reutilizar especificaciones y fixtures de Android, y compartir contratos/datasets antes de intentar compartir toda la implementación.

### 1.1 Estado Git y alcance de la evidencia

- Rama al empezar: main; último commit observado: 2a5bb2b, Expand Rust port roadmap with quality improvements.
- Árbol con numerosos cambios anteriores a esta auditoría: branding, launcher, recursos Android, web, documentación y archivos nuevos. No se atribuyen a este informe ni se consideran validados por publicarlos.
- El estado de implementación documentado anteriormente puede estar desactualizado. Por ejemplo, ya existen DLL en cores y la UI actual usa Preact/Tailwind; algunos documentos describen una etapa anterior.
- Las capturas docs/vyntra-desktop-native.png y docs/vyntra-settings-native.png son archivos de evidencia visual histórica. No prueban el estado actual de los núcleos ni de una sesión de emulación.

### 1.2 Índice de evidencia y rutas observadas

Los alias siguientes abrevían rutas en las fichas; no representan módulos nuevos.

| Evidencia | Ruta y símbolos observados | Conclusión documental |
|---|---|---|
| E01 [EXISTENTE-VERIFICADO] | desktop-rust/Cargo.toml; crates/pokedaisy-desktop/Cargo.toml | Workspace Rust 2024; paquetes vyntra-desktop y vyntra-libretro; carpetas técnicas antiguas conservadas. SDL2, fontdue, rfd, serde; Wry/Tao en Windows. |
| E02 [EXISTENTE-PARCIAL] | desktop-rust/crates/pokedaisy-desktop/src/main.rs:718 open_game; :748 find_core; :812 scan_games; :859 save_battery; state_path | Escaneo por extensión; carga core/ROM; SRAM y estados asociados a nombre de archivo. No hay identidad persistente por hash. |
| E03 [EXISTENTE-PARCIAL] | desktop-rust/crates/pokedaisy-desktop/src/shell.rs:16 run; :71 with_ipc_handler; :145 rama play; :215 save_files; :230 publish | UI WebView, IPC JSON por action, proceso secundario, selección de carpetas/DLL, exportación de copia. Sin protocolo de sesión continuo Player–Companion. |
| E04 [EXISTENTE-PARCIAL] | desktop-rust/crates/pokedaisy-desktop/src/settings.rs, Settings::{load,save,core} | JSON schema 1, backup .bak, rutas, volumen, filtro, fullscreen, FF, autoguardado, teclado y reduced_motion. |
| E05 [EXISTENTE-PARCIAL] | desktop-rust/ui/src/app.jsx, App, Button, Panel, Toggle; src/styles.css; build.mjs; test/ui.test.mjs | Biblioteca, filtros, grid/lista, ajustes y Vault; Companion es cobertura/limitación, no seis módulos implementados. Prueba UI con bridge simulado. |
| E06 [EXISTENTE-PARCIAL] | desktop-rust/crates/pokedaisy-libretro/src/lib.rs:339 Api; :402 callback_state; :553 LibretroCore; :859 environment_inner; :1073 input_state | Carga dinámica, ABI, callbacks, serialize/unserialize y SRAM. Contexto global único; input joypad; no contrato Desktop de memoria del acompañante. |
| E07 [EXISTENTE-VERIFICADO] | app/build.gradle.kts; app/src/main/AndroidManifest.xml | Kotlin/Compose, JDK 17, SDK 34, minSdk 26, NDK 28.2.13676358, CMake 3.22.1. applicationId/namespace com.pokedaisy.app. ABI arm64 por defecto, otras por flags. |
| E08 [EXISTENTE-PARCIAL] | app/src/main/kotlin/com/pokedaisy/app/LibraryActivity.kt; RomIdentity.kt; RomIntake.kt; RomArchive.kt | SAF y flujos de rutas; comprobaciones de cabecera y límites de copia; ZIP/7z; selección automática de una entrada de archivo. También MANAGE_EXTERNAL_STORAGE. |
| E09 [EXISTENTE-VERIFICADO] | app/src/main/kotlin/com/pokedaisy/app/EmulatorEngine.kt, EmulatorEngine; MgbaCore.kt; EmulatorView.kt; PokeDaisyActivity.kt:2956 onResume, :3445 onPause, :3459 onStop | Propietario de emulación, solicitudes entre frames, audio/vsync, suspensión, rewind/FF y lifecycle. Existencia no sustituye pruebas en dispositivo. |
| E10 [EXISTENTE-PARCIAL] | app/src/main/cpp/CMakeLists.txt; pokedaisy_jni.c; pk_rewind.c; nds/pk_nds*.cpp | Android integra mGBA, melonDS y una ruta DSperate. No se debe trasladar automáticamente la asociación Desktop armGBA a Android. |
| E11 [EXISTENTE-VERIFICADO] | app/src/main/kotlin/com/pokedaisy/app/companion/data/MemoryReader.kt:10 MemoryReader, :20 InProcessReader, RamSnapshot; Poller.kt:16 TelemetrySampler; companion/TelemetryStore.kt:21 TelemetryStore; data/SnapshotView.kt:181 SnapshotView | Lectores, detección, snapshots y StateFlow existentes. Datos no se limitan al esquema inicial del prompt. |
| E12 [EXISTENTE-PARCIAL] | CompanionSupport.kt, verdict/isSupported; Poller.kt, detect/detectDs; Gen1Reader.kt, Gen2Reader.kt, Gen4Reader.kt, Gen5Reader.kt; TypeChart*.kt | Soporte por juego/revisión/hash, hacks y best effort. Debe separarse confianza estricta de coincidencias parciales. |
| E13 [EXISTENTE-VERIFICADO] | SaveStates.kt, latestResumeSource; SaveBackups.kt; GameSaves.kt; companion/data/SnapshotCache.kt; companion/data/save/Gen3SaveFile.kt, parse/checksum/readSlot | Slots Android, copia Companion, parser Gen3 con sectores/checksum/slot activo. No asumir compatibilidad de estados entre motores/plataformas. |
| E14 [EXISTENTE-PARCIAL] | gamedata/README.md; gamedata/schema; gen3/retail, hacks, records; nds/ports.toml; scripts/gen_gamedata.py | TOML genera Kotlin y C; cobertura parcial. No es todavía un catálogo universal de identidad ni proveedor Rust. No editar salidas generadas manualmente. |
| E15 [EXISTENTE-PARCIAL] | libretro/armgba/CMakeLists.txt; linux/libretro-ds/CMakeLists.txt; .github/workflows/libretro.yml | armGBA tiene integración; linux/libretro-ds actualmente usa dsperate-lr/DSperate. CI observado construye Linux x86/aarch64, no acredita DLL Windows. |
| E16 [EXISTENTE-PARCIAL] | CoopGuards.kt; CoopBattles.kt; companion/coop; companion/data/coop; app/src/main/cpp/pk_coop_link*.c; server/coop-relay | Hay implementación de co-op especializada y feature gates. No equivale a Link universal ni a prueba con dos instancias. |
| E17 [EXISTENTE-VERIFICADO] | app/src/test; desktop-rust/ui/test; .github/workflows/ci.yml; website/package.json; website/test | Amplia base de pruebas Android/datos/co-op y web. No se ejecutó ninguna en esta auditoría. |
| E18 [EXISTENTE-VERIFICADO] | LICENSE; NOTICE; third_party/*/LICENSE; desktop-rust/assets/vyntra-*.svg; app/src/main/res/values/strings.xml | Atribuciones y marca parcialmente migrada. Mantener autores, términos adicionales y nombres de terceros. |

Árbol útil observado: app, desktop-rust, libretro, linux, gamedata, website, docs, third_party, scripts, server, native-capture, ui-preview, 3ds y .github/workflows.

### 1.3 Hallazgos prioritarios y verificación posterior

| Estado / severidad | Evidencia → inferencia | Incertidumbre y prueba futura |
|---|---|---|
| [HECHO-PARCIAL] P0 | E02 save_battery/state_path usaban file_name → dos ROM distintas con el mismo nombre compartían destino; la escritura SRAM ignoraba errores. Ahora hay clave por hash de contenido, escritura atómica y logging. | Probar homónimos y fallo de disco; falta mostrar el error en UI y backup restaurable (T05). |
| [DEUDA-TÉCNICA] P0 | main.rs:207 DropFile cambia selected/screen sin cerrar necesariamente core; cierre/autosave toma games[selected]. | Riesgo de guardar memoria del juego A bajo nombre de B; reproducir con core de prueba. La sesión debe poseer identidad/ruta inmutable. |
| [DEUDA-TÉCNICA] P0 | E03 acepta índices de lista en play; escaneo puede cambiar la colección. | Sustituir índices por GameId y revision del catálogo; prueba de escaneo concurrente con clic. |
| [DEUDA-TÉCNICA] P1 | main.rs:109 audio a 32768; :586–588 periodo fijo 16667 μs; E06 proporciona AV info. | Medir con core de prueba a distintas tasas y geometría dinámica; adoptar tasas reales y resampling, no asumir 60 FPS. |
| [DEUDA-TÉCNICA] P1 | E06 no incluye retro_get_system_info en Api; load_game lee la ROM completa. | Implementar need_fullpath/block_extract/extensiones y límites antes de probar cores públicos. |
| [DEUDA-TÉCNICA] P1 | E06 reconoce SET_MEMORY_MAPS/INPUT_DESCRIPTORS sin conservar un contrato utilizable; input_state solo joypad. | Rechazar capacidades no implementadas; fixtures de descriptors y stylus DS. |
| [EXISTENTE-PARCIAL] P1 | E03 supervisa Child con try_wait al solicitar acciones; Escape en SDL vuelve a su biblioteca. | El launcher puede considerar ocupada una sesión terminada; probar cerrar juego y volver a jugar sin cerrar manualmente ventana SDL. |
| [DEUDA-TÉCNICA] P1 | E08 RomIdentity.looksLikeRom usa firmas parciales; RomArchive.extract copia stream y verifica tamaño al terminar. | Añadir validación estructural y límite durante descompresión; probar tamaño declarado falso y contenedor ambiguo. |
| [DEUDA-TÉCNICA] P1 | E11 RamSnapshot delega lecturas fuera de regiones a InProcessReader, incluso en decodificación separada. | Auditar que solo lea ROM inmutable; reemplazar fallback libre por vista ROM capturada/validada, probar bloqueo de direcciones mutables. |
| [EXISTENTE-PARCIAL] P1 | E07/E08 permiso de acceso amplio y flujos SAF conviven. | Migrar flujo ordinario a SAF sin retirar de golpe acceso a bibliotecas existentes. |
| [EXISTENTE-PARCIAL] P1 | E12 algunos casos DS se reconocen por código/revisión; best effort es deliberado. | Código de cabecera no prueba identidad de hack; exigir firmas adicionales para live estricto, conservar manual/referencia. |
| [DEUDA-TÉCNICA] P2 | E04 backups/configuración y E13 caché no constituyen una transacción completa entre catálogo, payload y manifest. | Fault injection en cada fase, reinicio e integridad; nunca mostrar guardado confirmado antes de verificar. |

### 1.4 Núcleos locales y dependencias

**[EXISTENTE-VERIFICADO]** Se observaron seis DLL. Los hashes demuestran identidad binaria dentro de cada grupo, no procedencia, seguridad ni compatibilidad:

| Archivos | SHA-256 | Estado |
|---|---|---|
| mgba_libretro.dll, pokedaisy_gbc_libretro.dll, vyntra_gbc_libretro.dll | D775ECEF6DA1DA5489FF2E0A37C13826B510C73711F89E78CB8D757C6F8C1426 | [NO-VERIFICADO] Carga/ABI y origen no comprobados. |
| melondsds_libretro.dll, pokedaisy_ds_libretro.dll, vyntra_ds_libretro.dll | 6BE427125CAE551DA75680F85609CFB1CBAAFAFEF1CEBEBCC5E0E55A85E0FF41 | [NO-VERIFICADO] No asumir equivalencia con el core personalizado del proyecto. |
| Núcleo GBA con nombre propio/armGBA | No encontrado en la carpeta observada | [BLOQUEADO] Obtener/buildar y validar; un mGBA público podría ser alternativa de reproducción si supera el contrato, no sustituto automático de extensiones. |

Dependencias declaradas: SDL2 0.38, libloading 0.8.9, fontdue 0.9.3, rfd 0.15, Wry 0.54, Tao 0.34; Preact ^10.29, Tailwind ^4.2, Lucide ^0.577, esbuild ^0.27 y playwright-core ^1.65; versiones resueltas en lockfiles. NOTICE declara mGBA/partes armGBA MPL-2.0, melonDS GPL-3.0, DSperate GPL-3.0-or-later y licencias de dependencias. Verificar composición exacta por artefacto; no borrar avisos ni incluir Gambatte GPL-2-only en el ensamblado excluido por el proyecto. No se acreditó la procedencia de las DLL locales para redistribuirlas.

### 1.5 Estado de ejecución del plan (actualizado 2026-11-16)

**[HECHO-PARCIAL]** Esta sección registra lo ya implementado y verificado. El detalle por archivo y las limitaciones viven en `docs/VYNTRA_IMPLEMENTATION_STATUS.md`; la matriz de evidencia, en `docs/vyntra-capabilities.md`. Ninguna fila implica emulación/runtime probado.

| Tarea | Estado | Implementado y verificado | Falta para el DoD del ticket |
|---|---|---|---|
| T01 / QA-02 | HECHO-PARCIAL | Matriz de evidencia creada; baselines ejecutados: Rust (`cargo test --workspace`, 19 tests), UI (`npm test` 1/1, build OK), website (`npm run build` 146 páginas; `npm test` 81/82). | Fixtures/testId por celda, imagen de referencia 2 y registro por ROM/core en cada prueba. |
| T02 / INFRA-01 | HECHO-PARCIAL | `domain.rs`/`protocol.rs` con `UiCommandEnvelope`, `UiAck`, `AppError`; IPC legacy parseado con límite de payload, ACK por `window.vyntraAck` y errores por `window.vyntraError`; tests de payload sobredimensionado, acción legacy y ACK. | Doble envío/ACK perdido, logs saneados y contrato consumido por todas las acciones. |
| T03 / WIN-LIB-01 | HECHO-PARCIAL | `inspect.rs` valida estructura básica GB/GBC/GBA/DS con lectura acotada y confianza parcial; el catálogo usa título/código/revisión y el Play revalida el hash actual contra el ID publicado. | Corpus de headers por variante/región, checksum GBA/GB, deduplicación visible por bytes y ZIP. |
| T04 / BRAND-02 | HECHO-PARCIAL | `migration.rs` y Vault UI ofrecen vista previa, journal versionado, staging, hash y aplicación de saves no ambiguos; conflictos homónimos y estados legacy de compatibilidad desconocida se conservan. | Falta descubrimiento de ubicaciones heredadas fuera de las carpetas configuradas y fault injection de interrupciones/espacio insuficiente. |
| T05 / WIN-VAULT-01 | HECHO-PARCIAL | Saves/states en `by-id/<hash16>/` con escritura atómica y manifiestos; F4 valida hash de payload, ROM, core, formato y estado committed antes de unserialize. Migración segura SRAM no ambigua disponible desde Vault. | Backup restaurable, fault injection (ENOSPC, permiso, rename, corte, CRC), compatibilidad de SRAM legacy y prueba en runtime. |
| T06 / WIN-CORE-01 | HECHO-PARCIAL | `probe_pe.rs` valida PE/arquitectura/lista de exports sin ejecutar; `probe.rs` hace carga y lectura de API/system_info en proceso hijo con timeout de 8 s y reporte en Core Manager. | Fixture DLL loadable, dependencias/ABI y timeout/crash E2E; no hay un core local con procedencia verificada para smoke real. |
| T08 / WIN-PLAY-01 | HECHO-PARCIAL | Shell envía `storage_id` estable para Play; el launcher resuelve por ID; SDL conserva ROM y hash del core activos para guardar/cargar SRAM y estados aunque cambie la selección. DropFile conserva la pantalla Playing mientras hay core activo. | Protocolo de sesión end-to-end con player, cierre seguro al importar/solicitar otra partida, supervisor de crash y prueba E2E A/B con cores. |

Verificación común de esta ejecución: `cargo fmt --all --check` OK, `cargo test --workspace` OK (19 tests), UI build/test OK y website build OK. Runtime/emulación, Android y la imagen de referencia 2 siguen **[NO-VERIFICADO]**.

## 2. Brechas respecto a las dos referencias visuales

**[EXISTENTE-VERIFICADO] Referencia 1:** imagen adjunta general de Vyntra: teléfono a la izquierda; launcher arriba; paneles Combate, Equipo/Inventario y Mapa/Guía abajo; tarjeta de marca y tokens. Se inspeccionó visualmente. Las tres imágenes de clipboard localizadas son duplicados con el mismo hash, no tres referencias distintas.

**[NO-VERIFICADO] Referencia 2:** no se localizó vyntra_multiventana_pc.png. Los requisitos de dos ventanas, Dual/Split/Dock y dos monitores se especifican desde el texto. La aceptación de semejanza visual con esa segunda imagen queda pendiente de que el implementador reciba el archivo; no se sustituyó por una captura de la aplicación.

| Elemento | Brecha observada / objetivo [PROPUESTO] | Ticket |
|---|---|---|
| Jerarquía | La captura actual tiene grandes espacios vacíos y tarjetas inferiores de cobertura. Crear densidad adaptable, encabezado compacto, biblioteca dominante y paneles con contenido útil. | UI-01/02, WIN-LIB-02 |
| Profundidad | Conservar superficies carbón, borde superior iluminado, degradados discretos, sombra corta y selección crimson. Evitar neón global y paneles transparentes sobre ilustraciones. | UI-01 |
| Tipografía | Texto de interfaz antialias, tamaño legible, dos pesos. Pixel art solo en contenido de juego y assets legítimos; no en labels, menús, estadísticas ni texto largo. | BRAND-01, UI-01, AND-08 |
| Logo | Reutilizar los SVG V originales existentes; completar variante mono, ICO/adaptive icons, favicon, splash y tamaños pequeños. No copiar la ilustración protegida de la maqueta. | BRAND-01 |
| Biblioteca | Maqueta tiene selección, búsqueda, vistas, contador, acciones, guardados y núcleos. Persistir selección/vista y conectar Continuar a manifest compatible. | WIN-LIB-02, WIN-VAULT-01 |
| Companion | Sustituir tarjetas de limitación por módulos de referencia/manual útiles; live solo por adaptador. | CMP-* |
| Pocket | Conservar Compose y navegación actual; nueva cabecera, accesos y panel dentro de actividad, objetivos táctiles 48 dp y texto escalable. | AND-01/04/08 |
| DS | Dos pantallas del juego no son Player + Companion. Definir tres superficies conceptuales independientes. | MULTI-08, AND-03 |
| Multiventana | No existe sesión observable Desktop ni segunda WebView Companion integrada. | MULTI-01 a MULTI-07 |

Tokens propuestos centralizados: Black #000000, Night #0B0B0D, Charcoal #17171B, DarkCrimson #3A0A12, Crimson #B31231, Primary #E6E6E9, Muted #A5A5AF, Border #39343C. Añadir semánticos success/warning/error, foco, elevación, espaciado, radio, tipografía y motion; validar contraste, nunca usar color como único estado. Animaciones 120–250 ms, anulables. La UI no debe gastar GPU con blur/partículas durante emulación.

## 3. Decisiones de arquitectura y trade-offs

### 3.1 Arquitectura objetivo [PROPUESTO]

~~~mermaid
flowchart LR
  UI[Wry / Preact / Tailwind] --> APP[Casos de uso Rust]
  UI2[Companion WebView] --> APP
  APP --> REPO[Catálogo / Vault / Settings]
  APP <--> IPC[IPC local autenticado]
  IPC <--> WORKER[Proceso Player]
  WORKER --> OWNER[Único propietario del core]
  OWNER --> CORE[Libretro o adaptador propio]
  OWNER --> SNAP[Captura acotada entre frames]
  SNAP --> DEC[Decodificador de datos]
  DEC --> IPC
  DATA[gamedata versionado] --> DEC
  DATA --> REF[Referencia offline]
  REF --> APP
  AND[Compose / Kotlin] --> MODEL[Contratos equivalentes]
  MODEL --> JNI[EmulatorEngine / JNI existente]
~~~

Principios: una sesión posee ROM, core y destinos de guardado inmutables; UI no recibe punteros ni direcciones crudas; comandos críticos tienen confirmación; snapshots son inmutables y reemplazables; persistencia y exportación no dependen de nombres de tarjeta; proveedor de referencias no implica lector live; consultas Companion no autorizan modificar memoria.

### 3.2 Estrategias de ventanas [PROPUESTO]

| Estrategia | Ventajas | Coste / riesgo | Decisión |
|---|---|---|---|
| Core y todas las ventanas en mismo proceso | Menos transporte; estado compartido sencillo | DLL defectuosa derriba launcher; contexto global libretro limita múltiples cores | Descartada como arquitectura principal. |
| Launcher y Companion Wry en mismo proceso; Player SDL aislado | Reutiliza E03; compartir modelo UI sencillo; crash isolation | Requiere IPC y supervisión; Split/Dock no resulta gratis | Principal para MVP Dual. |
| Worker de core aislado + superficies nativas de vídeo hospedadas por shell | Permite Split/Dock real y múltiples pantallas sin reiniciar core | Spike Win32/Wry/SDL, transporte de frames y sincronización DPI; complejidad XL | Objetivo V1 condicionado al spike MULTI-02. |
| Reparentar HWND del SDL existente | Menor cambio inicial | Foco, fullscreen, DPI, accesibilidad y ciclo de vida frágiles | Solo alternativa si pruebas demuestran fiabilidad; no decidir por una demo estática. |
| Player/Companion como apps separadas por servidor TCP local | Aislamiento UI adicional | Puerto, autenticación y duplicación innecesaria en mismo PC | No requerido; reservar red para función LAN opt-in. |

IPC recomendado Windows: named pipes con ACL del usuario, nonce de sesión y framing limitado. Canales lógicos separados para comandos fiables, eventos de sesión y latest snapshot. Framebuffer no debe viajar serializado en JSON; para Split/Dock evaluar memoria compartida con buffers de tamaño validado, índices/secuencia y protocolo de propiedad. Audio permanece fuera del hilo UI. El fallback de un spike fallido es Dual funcional, con Split/Dock deshabilitado y motivo; no llamar Dock a dos ventanas simplemente alineadas.

### 3.3 Persistencia y reutilización

Conservar JSON para preferencias y manifests portables. Proponer SQLite para catálogo, sesiones, índices y notas: transacciones, búsquedas y miles de entradas justifican una dependencia nueva, su versión/licencia se fijarán tras revisión. Alternativa MVP: repositorio JSON con transacciones por archivo, limitado a corpus pequeño; no mantener dos fuentes activas de verdad. Compartir datasets, esquema y fixtures entre Rust y Kotlin; no introducir Kotlin multiplatform o Rust JNI para todo Android sin benchmark de coste y compatibilidad.

## 4. Modelo de dominio, contratos y eventos

Todo este apartado es **[PROPUESTO]**. Las firmas son orientativas, no código ejecutable.

### 4.1 Entidades e invariantes

| Tipo | Campos esenciales / clave | Invariante |
|---|---|---|
| GameIdentity | platform, title, gameCode, revision, region/idioma opcionales, sha256, legacySha1/CRC, identityConfidence | Hash de bytes descomprimidos; desconocido no equivale a retail. |
| RomSource / RomVariant | sourceId, locator/SAF URI, archiveEntry, tamaño, mtime, baseHash, patchHash, outputHash | Múltiples orígenes de mismos bytes; variantes separadas, nunca saves compartidos implícitamente. |
| LibraryEntry | gameId estable, identityId, título usuario, favoritos, tags, sourceIds, lastPlayed | Selección/IPC por ID, no índice de array. |
| CoreDescriptor | coreId, binaryHash, engineName/version, arquitectura, ABI, origin, licenseRefs, options | Nombre de DLL no identifica el motor ni su confianza. |
| CapabilityReport | feature, declared/detected/verified, scope, reason, evidenceId, verifiedAt | Distinción emulación estándar / extensión / frontend / adaptador de juego. |
| SessionIdentity | sessionId, gameId, romHash, coreHash, adapterId, epoch | Cada cambio de juego reinicia sesión; load/rewind incrementa epoch. |
| CompanionSnapshot | schema, session, revision, monotonicTime, wallTime, origin, capabilities, módulos opcionales | Ausente distinto de vacío. No mezclar revisiones ni fuentes sin indicarlo. |
| ProvenancedValue | value, source(live/save/reference/manual), observedAt, adapterVersion, confidence | Un dato manual nunca sustituye silenciosamente el live. |
| SaveManifest | saveId, kind, romHash, coreHash/version, stateFormat, slot, bytes, sha256, parentId, createdAt, status | SRAM, state y copia Companion son tipos distintos. |
| WindowLayoutProfile | profileId, mode, monitorId, bounds lógicos, dpi, splitRatio, tab, selection, zoom | Revalidar contra monitores disponibles y área de trabajo. |
| GameProfile | gameId, overrides, schema, inheritedRevision | Global → sistema → juego; permitir quitar override. |
| SyncConflict | objectId, commonAncestor, localHash, remoteHash, candidateVersions, resolution | Comparar ancestros/hashes; timestamps no resuelven divergencia. |
| CompanionNote / ChecklistProgress | gameId, datasetId/version, itemId, usuario/manual o observado, revision | No marcar progreso automático sin evidencia; conservar datos al actualizar catálogo. |

~~~mermaid
erDiagram
  GAME_IDENTITY ||--o{ ROM_SOURCE : localizado_en
  GAME_IDENTITY ||--o{ ROM_VARIANT : relacionado
  GAME_IDENTITY ||--o{ SESSION : ejecutado
  CORE_DESCRIPTOR ||--o{ SESSION : usa
  SESSION ||--o{ SAVE_MANIFEST : genera
  SESSION ||--o{ SNAPSHOT : publica
  GAME_IDENTITY ||--o{ GAME_PROFILE : configura
  GAME_IDENTITY ||--o{ COMPANION_NOTE : documenta
  SAVE_MANIFEST ||--o{ SYNC_CONFLICT : diverge
~~~

### 4.2 Contratos de aplicación

| Servicio / firma conceptual | Resultado / errores / efectos |
|---|---|
| RomInspector.inspect(source, cancellation) → InspectionResult | Identidad + avisos o InvalidHeader/Truncated/TooLarge/PermissionDenied; solo lectura. |
| LibraryService.importSources(request, cancellation) → ImportJobId | Progreso, reporte por archivo; indexación transaccional, nunca borra fuente. |
| CoreProbe.probe(candidate, policy, timeout) → CoreProbeReport | StaticRejected/LoadFailed/TimedOut/Crashed/AbiMismatch/ConfigurationRequired; ejecuta solo candidato elegido/autorizado. |
| SessionService.start(gameId, profileRevision, resumePolicy) → SessionId | Valida fuentes/core/manifest; adquiere lock, inicia worker, confirma Playing tras frame/audio/input inicializados. |
| SessionService.command(sessionId, epoch, commandId, command) → Ack | Rechaza StaleSession/Unsupported/Busy/InvalidTransition. Idempotencia por commandId en operaciones persistentes. |
| SaveRepository.commit(snapshotBytes, metadata, expectedRevision) → SaveId | Temporario, hash, flush, publicación atómica y manifest; DiskFull/Conflict/IntegrityError. |
| SaveService.restore(saveId, session, policy) → RestoreResult | Preflight, backup, pausa, carga, epoch nuevo, rollback o cierre seguro si core queda inconsistente. |
| GameStateProvider.detect(identity, memoryCapabilities) → AdapterMatch | Strict/Partial/Unsupported; matriz explícita. |
| GameStateProvider.read(capturedMemory, request) → SnapshotModules | Límites, checksum/invariantes; datos inválidos no se presentan como reales. |
| GameReferenceProvider.query(datasetId, query, cancellation) → Page | Referencias offline paginadas; DatasetMissing/VersionMismatch. |
| WorkspaceService.transition(session, targetMode, layout) → LayoutResult | Conserva core; reversión de vistas ante fallo. |
| TransferService.preview(bundle) / apply(decision) → TransferResult | Allowlist, integridad, compatibilidad y conflicto antes de escribir. |

### 4.3 Eventos y garantías

Envelope común: protocolVersion, eventType, eventId, requestId opcional, sessionId/epoch opcionales, revision, payload, timestamp monotónico local; fechas UTC para persistencia. El reloj monotónico no se compara entre dispositivos.

| Evento | Productor → consumidores | Entrega / frecuencia / cancelación |
|---|---|---|
| ImportProgress / ImportCompleted | worker IO → Library | Coalescing de progreso, resultado fiable; JobId y token cancelable. |
| CoreProbeCompleted | probe supervisor → Core Manager | Una respuesta por solicitud; cancelación mata solo probe, no Player. |
| SessionStarted/Paused/Stopped/Crashed | owner/supervisor → todas las vistas | Ordenado y fiable; sesión cerrada invalida suscripciones. |
| SnapshotPublished | decoder → store → Companion | Latest-only, cola acotada; frecuencia por módulo/budget, backpressure; suscripción con lease. |
| EpochChanged | owner → decoder/store | Antes de publicar nuevos datos; descartar trabajos de epoch anterior. |
| SaveCommitted/RestoreFailed | Vault → Library/Companion | Confirmación tras persistencia, no después del clic. |
| SettingsApplied | servicio ajustes → vistas/worker | requestId + revision efectiva; aplicado ahora, siguiente inicio o rechazado. |
| FocusOwnershipChanged/LayoutChanged | window manager → input/UI | No perder key-up; liberar teclas al salir del contexto de juego. |
| TransferConflictDetected | transferencia → centro de conflictos | Persistente hasta decisión; no sobrescritura automática. |

## 5. Diseño de almacenamiento y migración PokeDaisy → Vyntra

**[PROPUESTO]** Windows: raíz elegida explícitamente o directorio de datos por usuario Vyntra; portable mode conserva raíz junto al ejecutable solo si es escribible. Configuración, catálogo, saves, states, backups, notas, media, logs y cachés separados. No buscar indiscriminadamente en todo el perfil del usuario. Descubrir solo ubicaciones heredadas conocidas, argumentos y carpeta elegida.

Algoritmo migración: (1) abrir instalación vieja solo lectura; (2) inventariar con tamaño/hash y mapa de IDs; (3) detectar conflictos/espacio insuficiente; (4) escribir journal versionado; (5) copiar a staging con límites; (6) verificar cada hash; (7) publicar catálogo nuevo; (8) marcar migración completada; (9) mantener originales y ruta de recuperación. Un reinicio retoma por journal, una segunda ejecución es idempotente. Si hay dos versiones distintas de un save, ambas se conservan y se pide elección en Vault. No inferir juego únicamente del nombre del save.

Esquema catálogo propuesto: games(sha256 único), sources(locator+archiveEntry), variants(base/output), profiles, sessions, saves, notes, checklist, migration_journal. Índices por sistema, favorito, título normalizado y lastPlayed; relaciones por ID estable. Operaciones de archivo y base no forman una sola transacción del SO: usar staging + journal/reconciliación, publicar manifest al final y recuperar huérfanos de forma no destructiva.

Android mantiene applicationId, firma y namespace inicialmente. Alias SHA-1/CRC32 antiguos apuntan al nuevo GameId; no mover estados CRC a ciegas. Copiar/exportar dentro del sandbox y permisos SAF; entre applicationId distintos solo transferencia explícita por documento/Share/provider autorizado, nunca promesa de acceso directo. Respetar freshBootFile y separación de estados por core ya existentes en SaveStates.

| Anterior → objetivo | Estado / motivo / retirada |
|---|---|
| Marca PokeDaisy → Vyntra Desktop/Pocket/Companion/Library/Dex/Atlas/Vault/Link/Core Manager | [EXISTENTE-PARCIAL] Completar UI/web/metadatos; no reemplazo global de URLs reales. |
| POKEDAISY_CORE_DIR → VYNTRA_CORE_DIR | [EXISTENTE-PARCIAL] Precedencia ya en find_core; añadir pruebas y diagnóstico. Alias legado con warning solo desarrollo. |
| pokedaisy_*_libretro → vyntra_*_libretro en builds propios | [PROPUESTO] Mantener descubrimiento legado. DLL de terceros conserva identidad/origen; alias no implica adaptación. |
| com.pokedaisy.app → mismo ID interno | [EXISTENTE-VERIFICADO] Preservar actualizaciones, sandbox y firma; nombre visible Vyntra Pocket. |
| libpokedaisy/JNI/clases/rutas técnicas → conservar inicialmente | [PROPUESTO] Renombrar solo con migración de símbolos/cargador y prueba de upgrade; no necesario para marca visible. |
| saves por filename / states por CRC → GameId/hash con alias | [PROPUESTO] Copia verificable, conflictos explícitos, formatos intactos. |
| gamedata y salidas NativeConfigsGen/GameDataRecordsGen | [EXISTENTE-VERIFICADO] Mantener fuente TOML; actualizar generador si cambia esquema. |
| URLs de repositorios, cuentas, Firebase, copyright, NOTICE de terceros | [EXISTENTE-VERIFICADO] Conservar hasta migración real del servicio; no inventar dominios Vyntra. |
| claves de prefs/flags pokedaisy.*, magic PDSC y formatos heredados | [PROPUESTO] Readers compatibles y alias documentados; no modificar bytes por estética. |

Rollback: seleccionar raíz anterior intacta; no degradar en sitio un schema futuro. Backups de catálogo/config y payloads antes de migrar. Retención configurable; nunca purgar primera copia de migración automáticamente.

## 6. Especificación pantalla por pantalla y control por control

**[PROPUESTO]** Estados comunes: inicial, cargando, vacío, listo, desactualizado, no compatible, sin configurar, error, cancelado. Cada control tiene nombre accesible, foco visible y motivo textual si está deshabilitado. Los destinos duplicados comparten router/store, no copias de estado.

| Pantalla / controles | Acción / fuente / estados específicos | Aceptación futura | IDs |
|---|---|---|---|
| Primer inicio: elegir raíz, detectar legado, importar, omitir | MigrationService y configuración; preview de copia y conflictos | Interrumpir en cada paso y reabrir sin perder originales | BRAND-02, NEW-LIB-09 |
| Library: buscar, filtro GB/GBC/GBA/DS, favoritos, recientes, ordenar, grid/lista, tamaño | Consultar catálogo; persistir preferencias de vista/selección | Reinicio conserva vista; contadores coinciden con entradas reales | WIN-LIB-01/02, WIN-SYS-01 |
| Tarjeta: seleccionar, doble clic/Jugar, Continuar, menú contextual | GameId; Continuar con manifest compatible; menú Propiedades/controles/ubicación/Vault/quitar índice | Archivo ausente bloquea Play y ofrece relocalizar; quitar no borra ROM | WIN-LIB-02, WIN-PLAY-01 |
| Importar: archivos, carpeta, recursión, cancelar, detalles, reintentar | Job IO; reporte aceptados/duplicados/rechazados/omitidos | Cancelación preserva importaciones ya confirmadas y originales | WIN-LIB-03 |
| Sistemas: fila/conteo, abrir colección, core y capacidades | Catálogo + CapabilityReport | No mostrar Listo solo por DLL existente | WIN-SYS-01, WIN-CORE-01 |
| Core Manager: carpeta, DLL, probar, opciones, logs, volver a versión previa | Probe aislado; No instalado/Sin verificar/Listo para…/Incompatible/Error/Configurar | DLL inválida no cierra launcher; origen y versión visibles | WIN-CORE-01/02, NEW-PLY-12 |
| Player: pausa, reanudar, fullscreen, cerrar, velocidad, overlay | SessionService + AV real | Input/audio/vídeo y cierre con guardado coherente | WIN-PLAY-01/02 |
| Player: F2/F4, slots, rewind, mandos, Companion | Vault/capacidades; bloquear rewind durante Link cuando requerido | Atajos no actúan al escribir en búsqueda; epoch correcto tras carga | WIN-VAULT-01, MULTI-06 |
| Vault: recientes, filtro tipo, guardar/cargar, renombrar, borrar, importar/exportar, recuperar | Manifests + archivos verificados; borrar exige confirmación | Incompatible no carga; exportación no toca original; restauración deja backup | WIN-VAULT-01, NEW-SAVE-* |
| Sync: dispositivos, exportar/importar paquete, previsualizar, conflictos | Transporte local primero; red opt-in | Hash corrupto rechazado, dos versiones conservadas | WIN-SYNC-01 |
| Link: crear/unirse/desconectar/diagnóstico | Capacidad específica de core/juego; no compatible explicado | Dos instancias reales para habilitar soporte publicado | WIN-LINK-01, AND-07 |
| Ajustes: general/idioma/tema/motion, biblioteca/carpetas, núcleos, AV, input, rendimiento, privacidad/logs, datos/legal | Settings con validación, alcance global/sistema/juego, estado Aplicando/Guardado/Requiere reinicio/Error | Cambios alteran comportamiento; restablecer no borra partidas | WIN-SET-01, AND-08 |
| Companion inicio: juego, modo fuente, seis accesos, última actualización | Session + referencia + capacidades | No mostrar dato cacheado como En vivo | NEW-CMP-01/17 |
| Equipo: seis plazas, detalle, refrescar, abrir Dex, manual/live | Snapshot validado; vacío distinto de sin lector | HP/PP/nivel inválidos → error de lectura, no inventar valores | CMP-TEAM-01 |
| Mochila: categorías, búsqueda, cantidad, ficha, refrescar | Snapshot y catálogo compatible; sin botón Usar por defecto | Referencias manuales no afirman pertenencia a la mochila | CMP-BAG-01 |
| Dex: búsqueda/filtros, anterior/siguiente, evolución/movimientos/encuentros, visto/capturado | ReferenceProvider; progreso solo con fuente de partida | Cambiar ROM cambia dataset sin mezclar generaciones | CMP-DEX-01 |
| Combate: aliado, movimiento, rival/tipos, afinidades, detalle, modo | Tabla de reglas por juego; fuente live/save/manual visible | Inmunidad y doble tipo correctos; no inferir estadísticas desconocidas | CMP-BATTLE-01 |
| Atlas: zoom ±, pan, zona, punto, centrar, capas, pestañas | Mapa permitido o vista textual; marcador solo observado | Zoom/foco persisten; DS touch no se confunde con mapa | CMP-MAP-01 |
| Guía/Quests/Notas: índice, búsqueda, favorito, spoiler, check/uncheck, deshacer | Dataset versionado + progreso local manual separado | Checklist persiste y undo restaura; sin spoilers en snippets | CMP-GUIDE-01 |
| Ventanas: abrir/cerrar Companion, Dual/Split/Dock/Compact, monitor, pin, restaurar | WorkspaceService; fuente única sesión | Cerrar Companion no cierra Player; desenchufar monitor recupera ventana | MULTI-01/02/04/06 |
| Pocket Home/Game: elegir ROM, jugar/continuar, pausa/salir, menú | SAF + motor JNI + lifecycle | Rotar/bloquear/volver conserva identidad y no deja botones pulsados | AND-01/02 |
| Pocket accesos Team/Bag/Map/Dex/Battle/Guide/States/Rewind/FF/Controls/Link/More | Destinos del mismo estado; bottom Home/Team/Bag/Guide/Settings y Atrás coherente | Ningún acceso abre una página aislada sin contexto | AND-03 a AND-08 |
| DS Player: superior/inferior, swap, rotación, layout, stylus | Geometría y transform inversa | Esquinas/centro llegan a coordenadas DS correctas en todos los layouts | MULTI-08, AND-03 |
| Información/ayuda/legal: versión, licencias, cobertura, exportar diagnóstico | Build metadata + licencias + matriz + redacción | Reporte excluye secretos/rutas personales por defecto | INFRA-01, DOC-01, NEW-DEV-05/08 |

## 7. Fichas técnicas por feature

### 7.1 Catálogo de archivos propuestos y reglas comunes

Prefijos: D = desktop-rust/crates/pokedaisy-desktop/src; L = desktop-rust/crates/pokedaisy-libretro/src; U = desktop-rust/ui/src; A = app/src/main/kotlin/com/pokedaisy/app; C = A/companion/data. E01–E18 indican archivos existentes. Los siguientes módulos son **[ARCHIVO NUEVO PROPUESTO]**, no se han creado:

- D/domain.rs: identidad, sesión, capabilities, errores; D/protocol.rs y U/bridge.js: contratos IPC y ACK.
- D/library/{inspect,import,repository,patches}.rs: validación, trabajos, catálogo y variantes.
- D/cores/{catalog,probe,options}.rs: descubrimiento, validación aislada, opciones.
- D/session/{supervisor,worker,input,timing,transport}.rs: lifecycle, propietario de core, input, pacing e IPC.
- D/storage/{migration,atomic,catalog}.rs y D/vault/{manifest,service,transfer}.rs: persistencia y guardados.
- D/companion/{model,registry,reference,snapshot,team,bag,dex,battle,atlas,guide}.rs: dominio Companion sin UI.
- D/workspace/{windows,layout,focus,ds}.rs: ventanas y superficies; D/diagnostics.rs.
- U/components/{Button,Panel,Status,Dialog,DataTable}.jsx; U/screens/{Library,Import,CoreManager,Player,Vault,Settings,Companion,Team,Bag,Dex,Battle,Atlas,Guide,Sync,Link}.jsx; U/theme.css; U/i18n/es.json.
- A/vyntra/{GameIdentity,CapabilityReport,SaveManifest,Migration,SettingsSchema,TransferService}.kt: adaptación gradual a Kotlin; A/companion/ui/vyntra/{Theme,Navigation,Capabilities}.kt.
- gamedata/schema/{identity,capabilities,reference_pack}.toml; gamedata/targets/rust.toml; contracts/vyntra/v1/*.schema.json; tests/fixtures/vyntra/: esquemas/fixtures nuevos, sin ROMs comerciales.
- docs/vyntra-capabilities.md, docs/adr/vyntra-*.md, scripts/build-vyntra-cores-windows.ps1 y .github/workflows/vyntra-desktop.yml: documentación y pipeline futuros.

Herencia obligatoria para TODAS las fichas: IO/hashes/compresión en worker cancelable; FFI solo propietario de core; persistencia con revisión esperada y backup; comandos validados en Rust/Kotlin además de UI; mensajes localizados; datos de terceros con procedencia; pruebas positivas, negativas, adversariales y regresión; rollback descrito en sección 18. DoD exige evidencia ejecutada por implementador, no basta un botón visible.

### 7.2 Marca, UI e infraestructura

| ID / prioridad / tamaño / responsable | Estado, evidencia y trabajo asignable | Contrato / flujo / prueba de aceptación |
|---|---|---|
| BRAND-01 P1 M UI+Android | [EXISTENTE-PARCIAL] E18, E05. Auditar strings/metadata/assets propios en app, U, website, docs, scripts/logo_icon.mjs; usar SVG existentes, completar iconos. | BrandAssetManifest propuesto con usos/licencia; inventario → clasificar visible/técnico/tercero → sustituir solo propios → inspección 16–512 px. Prueba: ningún nombre viejo visible salvo atribución histórica explicada; URLs de terceros siguen intactas. |
| BRAND-02 P0 L Rust+Android | [EXISTENTE-PARCIAL] E02/E04/E07/E13. D/storage/migration.rs, A/vyntra/Migration.kt. Depende INFRA-01. | MigrationPlan.preview/apply/recover; journal y hashes. Casos: disco lleno, conflicto, interrupción y segunda ejecución; originales byte-idénticos. |
| UI-01 P1 M UI | [EXISTENTE-PARCIAL] E05; U/theme.css y Theme.kt; sin cambio de framework. | ThemeTokens, escala y contraste; aplicar tokens → validar responsive/DPI/texto. Prueba teclado, 200% texto, motion off; tipografía UI no pixelada. |
| UI-02 P1 M UI | [EXISTENTE-PARCIAL] E05; extraer componentes de app.jsx y navegación. Depende UI-01/INFRA-01. | ComponentState y RouteContext; load/error/disabled con motivo y focus restoration. Prueba diálogos, Escape, gamepad, error al persistir; no toast de éxito prematuro. |
| INFRA-01 P0 L Rust+Kotlin | [HECHO-PARCIAL 2026-11-16] E03/E04/E17. D/domain.rs y protocol.rs implementados (envelope, ACK, AppError, límite de payload); D/diagnostics.rs y SettingsSchema.kt pendientes. | AppError(code,reason,retryable), Command/Ack versionados; límite de payload, correlación y logs saneados. Fixtures desconocido/malformado/demasiado grande; ninguna ruta arbitraria desde WebView. |

### 7.3 Desktop

| ID / prioridad / tamaño / responsable | Estado, archivos, dependencias | Operación/algoritmo, fallos y aceptación |
|---|---|---|
| WIN-LIB-01 P0 L Rust | [HECHO-PARCIAL 2026-11-16] E02 scan_games; D/identity.rs y D/library.rs (catálogo con hash) implementados; parser inspect.rs, dedup por bytes y ZIP pendientes; INFRA-01, BRAND-02 | inspect identifica cabecera/estructura, calcula SHA256 por streaming y cachea por source metadata; hash se revalida al cambio/Play. Deduplicar bytes, mantener revisiones/ubicaciones. ZIP: selección, paths seguros, cuotas y cancelación. Fixture válido/truncado/falsa extensión/homónimo/duplicado; exactamente los válidos quedan indexados tras reinicio. |
| WIN-LIB-02 P1 M UI+Rust | [EXISTENTE-PARCIAL] E05/E03; U/screens/Library.jsx; WIN-LIB-01 | queryLibrary(filter,sort,page), updateEntry, relink(expectedHash), removeIndex. Persistir vista/favoritos; cubierta etiquetada si falta licencia. Quitar índice no borra archivo; cambio de ruta solo si hash coincide; captura de error accesible. |
| WIN-LIB-03 P1 M Rust+UI | [EXISTENTE-PARCIAL] E03; D/library/import.rs, U/screens/Import.jsx; WIN-LIB-01 | Elegir archivos/carpetas → crear JobId → inspección → confirmación por lote → reporte. Cancelación cooperativa en límites de bloque, no solo descartar resultado; PermissionDenied por entrada, no abortar toda carpeta. Prueba cancelación durante ZIP y reinicio sin entradas parciales. |
| WIN-CORE-01 P0 L Rust/FFI | [HECHO-PARCIAL 2026-11-17] `probe_pe.rs`, `probe.rs`, `CoreSystemInfo` y Core Manager; INFRA-01 | Inspección PE/arquitectura/exports antes del child load/version/system_info, timeout/crash acotados y reporte visible. Falta fixture DLL loadable y prueba real con dependencias/core por arquitectura. |
| WIN-CORE-02 P1 XL C/C++ build | [HECHO-PARCIAL T07; bloqueado para DLL] `docs/VYNTRA_WINDOWS_CORE_MATRIX.md`; commits fijados y LICENSEs revisados; WIN-CORE-01 | Configuración Windows verificada para mGBA/armGBA/DSperate; fallos concretos: `mkdir` y version script ELF (GBC), `sys/mman.h` y version script ELF (GBA), `aligned_alloc` y version script ELF (DS). Generadores DS fuerzan UTF-8. Pendiente port Windows, exports PE, revisión de distribución armGBA y smoke con procedencia verificada. DSperate no es melonDS DS. |
| WIN-PLAY-01 P0 XL Rust/FFI | [HECHO-PARCIAL 2026-11-16] E02/E03/E06; SessionIdentity publicada y comandos validados por epoch en shell; D/session/* y consumo por player pendientes; WIN-LIB-01, WIN-CORE-01, WIN-VAULT-01 base | start captura identidad/paths, lock, probe vigente, load SRAM validada, iniciar A/V con timing real, emitir Playing; pause/stop entre frames, flush y cierre. Child exit supervisado, nunca guardar según selección UI. Prueba 2 ROM homónimas/drop durante juego/crash y segunda apertura. |
| WIN-PLAY-02 P1 L Rust+UI | [EXISTENTE-PARCIAL] F2/F4/FF en E02; D/session/input.rs, U/screens/Player.jsx; WIN-PLAY-01 | Overlay opera sobre Command/Ack; save/load/FF/rewind según capability, foco libera teclas. Rewind invalida snapshots/epoch y limpia audio; FF no cambia reloj de observaciones a wall clock ficticio. Prueba teclado, mando, texto en Companion y core sin estados. |
| WIN-VAULT-01 P0 L Rust+Kotlin | [HECHO-PARCIAL 2026-11-16] E02/E03/E13; aislado por hash + escritura atómica + manifiestos en main.rs; restauración/fault injection pendientes; INFRA-01, identidad | commit/restore/export/import con manifest, hash, tipo/core/ROM. Captura en owner, escritura fuera, backup previo; cargar incompatible bloqueado. SRAM sin header requiere tamaño y perfil, no adivinar por extensión. Prueba corrupción, power loss, disco lleno y rollback. |
| WIN-SET-01 P1 L Rust+UI | [EXISTENTE-PARCIAL] E04/E05; settings.rs, U/screens/Settings.jsx, profiles | SettingsPatch(expectedRevision,scope,changes); validar duplicados/paths/valores; aplicar ahora o marcar próximo inicio. Tabs todas de sección 6; buscador y reset por sección. Prueba conflicto dos ventanas, cambio fallido y restablecer conservando saves. |
| WIN-SYNC-01 P1 L Rust+Android | [EXISTENTE-PARCIAL] Solo export copia E03; D/vault/transfer.rs, A/vyntra/TransferService.kt; Vault e identidad | Export manifest allowlist → validar import → preview → resolver divergencia → copia nueva. LAN después; nada de nube ficticia. Prueba hash incorrecto, paquete con ROM/BIOS, identidad distinta y ambos dispositivos modificados. |
| WIN-LINK-01 P2 XL C/C+++red | [BLOQUEADO] E16 no demuestra Desktop Link; D/session/link.rs propuesto; core capability+sesión+Vault | Spike por engine: cable/hook/transporte, lock de estados/rewind, handshake versión/ROM y disconnect. Mantener deshabilitado con motivo hasta 2 instancias verificadas; no emular conexión con UI. |
| WIN-SYS-01 P1 S UI+Rust | [EXISTENTE-PARCIAL] E05; Library/CoreManager; WIN-LIB-01, WIN-CORE-01 | SystemSummary calculado de catálogo y capability; fila filtra juegos, botón configura motor. Prueba colección vacía y GB/GBC con flags reales de cabecera, sin conteos duplicados por alias de DLL. |

### 7.4 Multiventana

| ID / prioridad / tamaño / responsable | Estado y archivos | Operación / aceptación |
|---|---|---|
| MULTI-01 P1 M Desktop | [PROPUESTO] E03 base; D/workspace/windows.rs, shell.rs; INFRA-01/MULTI-03 | WindowRegistry crea Companion con viewId; subscribe/release; guardar bounds lógicos y monitor; cerrar y reabrir mantiene contexto. Prueba 1/2 monitores y ventana fuera de pantalla. |
| MULTI-02 P2 XL Desktop/GPU | [BLOQUEADO] D/workspace/layout.rs; MULTI-01/03 y spike superficie | WorkspaceTransition conserva core/SessionId. Preparar nueva superficie → transferir view state → mostrar → retirar vieja; fallo revierte vista. Prueba 30 cambios Dual/Split/Dock con audio y frame continuo; no reiniciar ROM. |
| MULTI-03 P0 L Rust | [PROPUESTO] D/protocol.rs, session/transport.rs, companion/snapshot.rs; INFRA-01 | Bus de snapshot latest-only y eventos fiables; subscribe(session,modules), unsubscribe; límites y versionado. Prueba consumidor lento: memoria acotada y comando Stop nunca detrás de cola ilimitada. |
| MULTI-04 P1 M Rust+UI | [PROPUESTO] companion/model.rs, U/bridge.js; MULTI-03 | Store acepta solo session/epoch vigente y revision creciente. Cache muestra Desde guardado; timeout muestra Desactualizado con fecha, nunca En vivo. Prueba load/rewind/juego nuevo con decode atrasado. |
| MULTI-05 P1 XL datos/FFI | [EXISTENTE-PARCIAL] E11–14 reutilizables; companion/registry.rs y L memoria; identidad, MULTI-03 | Captura owner → buffers propios → decode worker → invariantes → snapshot. No punteros retenidos ni offsets en UI. Prueba ROM parecida desconocida rechazada y segmentos fuera de rango. |
| MULTI-06 P1 M Desktop/input | [PROPUESTO] workspace/focus.rs, session/input.rs; MULTI-01 | FocusLease y ámbitos juego/UI/global opt-in; edición texto inhibe acciones; liberar teclas al perder foco/desconectar mando. Prueba keydown→AltTab→keyup y retorno sin tecla pegada. |
| MULTI-07 P0 M Rust/seguridad | [PROPUESTO] session/transport.rs; INFRA-01 | Named pipe por sesión/usuario, nonce, ACL, frame length, timeout, schema; allowlist de comandos, sin escuchar red. Prueba cliente ajeno, replay, bytes excesivos y cierre/reconexión. |
| MULTI-08 P1 L Desktop/FFI | [EXISTENTE-PARCIAL] E06 sin pointer; workspace/ds.rs, L/lib.rs; WIN-PLAY-01 | DsSurfaceDescriptor + transform inversa para stylus; layout solo si sabemos regiones del framebuffer/core. No cortar imagen arbitrariamente. Prueba esquinas, swap, DPI, bordes fuera de táctil y zoom. |

### 7.5 Companion: seis fichas operativas

**CMP-TEAM-01 — Equipo. [EXISTENTE-PARCIAL], P1/L, responsable datos + UI, MVP manual/referencia; V1 live.** Evidencia E11/E12 y PartyScreen.kt; Desktop U/app.jsx solo indica limitación. Nuevos companion/team.rs y U/screens/Team.jsx; adaptar UI Android existente, no rehacer lector. Dependencias identidad, ReferenceProvider, MULTI-04/05 para live. Tipos propuestos PartySnapshot, PartyMember, MoveSlot, PartySelection; readParty(capture) → Supported/Unsupported/DecodeError; selectMember(stableSlotId). Flujo: elegir fuente → verificar identidad/adapter → leer copia → validar count 0..6 y campos por generación → publicar → seleccionar miembro → detalle/Dex. HP desconocido se omite; HP máximo cero/inconsistente invalida el campo y deja motivo. Estados Sin partida, Leyendo equipo, Equipo vacío, Desde guardado, En vivo, Desactualizado, No disponible para este juego. Persistir solo selección y equipos manuales separados; cache con manifest/epoch. Tests: fixture 6 miembros, 0 miembros válido, conteo 255, checksum malo, nick Unicode, evolución/cambio slot, regreso de decode anterior tras load. DoD datos/origen coinciden fixture; detalle y Dex preservan contexto; ninguna escritura a memoria/save. Rollback desactivar adaptador conservando manual.

**CMP-BAG-01 — Mochila. [EXISTENTE-PARCIAL], P1/M, datos + UI.** Reutilizar SnapshotView/items, SaveBagScreen.kt, Gen3SaveFile/BagPocket; nuevos companion/bag.rs y Bag.jsx. Contratos BagSnapshot(pockets,entries,source), ItemReference, queryBag(category,query,page). Flujo captura consistente → seleccionar layout por identity → decodificar cantidades/claves cuando aplique → validar índices/cantidades → agrupar → buscar y ficha desde dataset exacto. Sin adaptador ofrecer Catálogo de objetos, nunca Mochila real vacía. Cantidad cero, objeto desconocido y bolsillo ausente son distintos. UI consulta; antiguas funciones de uso/edición Android se conservan separadas y explícitas, no disparadas por esta vista. Pruebas categorías, cantidad corrupta, ID desconocido, búsqueda con acentos, listas grandes y actualización mientras ficha abierta. Persistencia filtros/selección; no inventario manual mezclado. DoD acciones solo lectura verificadas con hash de save antes/después.

**CMP-DEX-01 — Dex. [EXISTENTE-PARCIAL], P1/L, datos + UI, MVP offline.** Evidencia Gen1Dex/Gen2Dex/Gen4Dex/Gen5DexBlack, DexDetails.kt, gamedata registros; nuevos companion/dex.rs/reference.rs y Dex.jsx. Tipos SpeciesKey(datasetId,speciesId,form), DexProgress(known/unknown), EncounterReference; searchSpecies(query,filters,cursor), getSpecies(key). Flujo seleccionar juego/dataset → índice normalizado → buscar/paginar → ficha → enlaces evolución/movimientos/Atlas/Guía con RouteContext. Visto/capturado solo si observado; referencia no se marca no capturada. No mezclar ids entre hacks/generaciones. Dataset faltante muestra cobertura y selector manual. Tests tipos con acentos, especies con mismo id en dos packs, forma/evolución condicional, progreso unknown, dataset actualizado que elimina entrada. Persistir favoritos por dataset/key y resolver huérfanos sin borrar. DoD consulta offline y navegación contextual sin llamadas al core durante render.

**CMP-MAP-01 — Atlas. [EXISTENTE-PARCIAL], P1/L, datos + UI.** Evidencia Gen4TownMap*, Gen5TownMap*, RegionMapModel, GuideAreas; nuevos companion/atlas.rs y Atlas.jsx. Tipos MapDefinition(version,bounds,license,coordinateSystem), AreaId, PointOfInterest, PlayerLocation(provenance); getMap(identity,region), locate(snapshot), route(from,to,rules). MVP mapa esquemático original/vista textual donde faltan recursos permitidos; rutas automáticas solo con grafo de conexiones conocido. Flujo dataset → cobertura/versión → cargar asset legal o texto → fit bounds → zoom/pan/clamp → seleccionar área → guía/notas. Posición live no inferida de última área manual. Progreso bloqueado/descubierto conserva unknown. Tests mapas sin asset, coordenadas fuera de límite, zoom teclado/touch, cambio de ROM y de DPI, spoiler en POI. Persistir viewport y marcadores por mapVersion; invalidar transform si cambia dataset. DoD todas las pestañas reales y fallback textual accesible.

**CMP-BATTLE-01 — Combate. [EXISTENTE-PARCIAL], P1/L, datos + UI, MVP manual.** Evidencia TypeChart*.kt, BattleInfoScreen.kt, Poller/SnapshotView; nuevos companion/battle.rs y Battle.jsx. Tipos BattleSnapshot, TypeRulesetId, MatchupRequest, MatchupResult(factor,assumptions,source), DamageInput opcional. typeEffectiveness(rules,attackType,targetTypes,knownModifiers) devuelve producto racional con inmunidad prioritaria y notas; no asumir habilidades/clima observados. Modo manual pide generación/juego, tipos y movimiento; live requiere adapter Battle, invalida rival al terminar combate. Tabla Gen1 conserva peculiaridades de esa generación, Gen2–5 y hacks usan ruleset propio; tipado por ID de dataset, no posición global. Pruebas Fuego→Planta 2x, Normal→Fantasma 0, Tierra→Volador/Acero 0, ataque contra dos debilidades 4x, cambios de Fantasma/Psíquico Gen1 frente a Gen2 y de resistencias de Acero antes/después Gen6 si se amplía. Añadir fixtures oficiales de reglas obtenidos legalmente. No estimar daño exacto sin stats/IV/EV/nivel/modificadores; mostrar intervalo condicionado o pedir datos. DoD cálculo manual útil, origen visible, cero inyección de botones al juego.

**CMP-GUIDE-01 — Guía/Quests/Notas. [EXISTENTE-PARCIAL], P1/L, datos + UI.** Evidencia Guide.kt/GuideAreas/GuideGen4/GuideGen5 y tablas generadas; nuevos companion/guide.rs y Guide.jsx. Tipos GuidePack, Chapter, Objective, ManualProgress, ObservedProgress, SpoilerPolicy; searchGuide(dataset,query,policy), toggleManual(objective,expectedRevision), observeFlags(snapshot). Flujo elegir versión/idioma → índice local → aplicar spoiler antes de snippets/resultados → capítulo → marcar manual/deshacer → persistir → enlazar área. Detección de flag verifica adapter/revisión y no modifica casilla manual. Sin guía compatible mostrar notas/checklist personal funcional, no objetivos inventados. Tests persistencia/reinicio, undo, cambios de dataset con IDs huérfanos, filtro de spoiler en búsqueda y notificaciones, flag desconocido, conflicto dos ventanas. DoD cuatro tabs Mapa/Objetivos/Guía/Notas abren vistas útiles, con fuente y cobertura. Rollback pack previo y conservar notas.

### 7.6 Android, documentación y distribución

| ID / prioridad / tamaño / responsable | Estado, archivos y dependencias | Flujo / aceptación futura |
|---|---|---|
| AND-01 P0 L Kotlin | [EXISTENTE-PARCIAL] E08, LibraryActivity/LaunchActivity/RomIntake; identidad/migración | SAF document/tree con permisos persistidos; copiar cuando motor exige path, límites durante IO; URI revocada → volver a autorizar. Mantener biblioteca vieja legible; fixture provider y intents hostiles. |
| AND-02 P0 L Kotlin+C++ | [EXISTENTE-PARCIAL] E09/E10, EmulatorEngine/MgbaCore/EmulatorView; INFRA-01 | Inventario por ABI/core; enlazar capability real al selector; validación engine/ROM, audio focus, errores JNI y cierre. DoD homebrew por ABI objetivo, input/audio/vídeo y suspensión en dispositivo; compilación sola insuficiente. |
| AND-03 P1 M Kotlin/GPU | [EXISTENTE-PARCIAL] nds/* y EmulatorView; AND-02 | DsLayoutProfile, transform de touch, swap/orientación/escala; persistir por juego. Prueba rotación con dedo activo cancela contacto y esquinas correctas. |
| AND-04 P1 L Kotlin+datos | [EXISTENTE-PARCIAL] E11–14 y companion/ui; CMP seis módulos | Adaptar Theme/Navigation/Capabilities y procedencia al StateFlow; panel dentro actividad y selección común. Auditar RamSnapshot fallback. Prueba panel abre/cierra durante play sin perder estado ni ejecutar IO en recomposición. |
| AND-05 P0 L Kotlin+C++ | [EXISTENTE-PARCIAL] SaveStates/SaveBackups/EmulatorEngine/pk_rewind.c; Vault | Extender manifests manteniendo slots/CRC, fresh boot y undo; memoria rewind y FF real; bloquear modos incompatibles. Pruebas undo/save/load, core cambiado, process death, espacio lleno. |
| AND-06 P1 L Kotlin/input | [EXISTENTE-PARCIAL] Prefs, EmulatorView, entrada existente; AND-02/UI-01 | InputProfile visual: tamaño/posición/opacidad/vibración/deadzone y USB/BT; conflictos, preview/restaurar, safe insets. DoD mando desconectado libera input y perfil persiste tras rotación. |
| AND-07 P2 XL Kotlin+red | [EXISTENTE-PARCIAL] E16 y GameSaves; WIN-SYNC-01/Link spike | Separar transferencia de archivo de co-op existente; mantener feature flags/restricciones, handshake y consentimiento. E2E dos dispositivos obligatorio para etiqueta compatible. |
| AND-08 P1 L Kotlin/UI | [EXISTENTE-PARCIAL] Prefs, Settings existentes, i18n; UI-01/02 | Catálogo tipado de settings con alcance/capability, español inicial, búsqueda, modo bajo consumo, privacidad/logs/legal. No perder preferencias avanzadas ya existentes. Prueba cada ajuste cambia efecto o explica reinicio. |
| AND-09 P0 M Kotlin/release | [EXISTENTE-PARCIAL] E07/E13; BRAND-02 | Conservar ID/certificado; schema prefs y alias de datos, preview y backup. Upgrade de versión previa en mismo dispositivo conserva ROM refs y saves; distinto firmante falla con explicación, no borrar app. |
| DOC-01 P1 M docs/web | [EXISTENTE-PARCIAL] README, docs/manual, website; QA-02 | Inventario nombres/enlaces/sources; distinguir capturas reales vs fixtures; generar manual desde fuente existente, actualizar matriz. Test anchors/links y coherencia de nombres de APK reales; no cambiar URL por simple branding. |
| REL-01 P1 L release | [EXISTENTE-PARCIAL] E17/E18 workflows; core builds/QA | Paquetes Windows/Android con versionado, hashes, firma y notices; instalador nunca borra datos; WebView2 requisito y SDL fallback documentados. Upgrade/rollback con fixtures; no publicar release si matriz no acredita mínimo. |
| QA-01 P0 L QA | [EXISTENTE-PARCIAL] E17; tests/fixtures/vyntra y workflow Desktop | Contratos, fuzz parsers, fault injection saves, E2E nativo + Android real, regresión de features existentes. Separar mock UI de core smoke. |
| QA-02 P0 M QA+docs | [HECHO-PARCIAL 2026-11-16] docs/vyntra-capabilities.md creado y baselines ejecutados; falta tabla en apps y registro por ROM/core | Registro por plataforma/sistema/core hash/ROM adapter/función/build/evidence; sin prueba queda Pendiente. UI lee mismo formato que documentación, evita drift. |

## 8. Plan específico de Player + Companion en dos ventanas del mismo PC

**[PROPUESTO]** Implementación incremental sin sustituir Preact ni Compose:

1. Shell crea SessionId, adquiere lock de juego/save y arranca worker con canal local privado; no enviar secretos por logs/argumentos.
2. Worker completa preflight, carga ROM y SRAM, reporta versión, capacidades y SessionStarted. Player conserva ventana SDL inicial.
3. Shell abre otra ventana Tao/Wry con role=Companion y viewId propio. Comparte ApplicationStore, pero cada vista conserva su scroll/tab/selección.
4. La nueva vista recibe snapshot completo, luego deltas/versiones o latest snapshots. No necesita servidor ni cuenta.
5. Cerrar Companion cancela suscripciones y guarda layout; Player continúa. Cerrar Player inicia flush/Stop; Companion pasa a Sesión finalizada y mantiene referencias/notas.
6. Si worker falla, supervisor conserva launcher y muestra error recuperable; snapshot pasa stale, Vault no marca como válido un save incompleto.
7. Reconexión exige mismo SessionId/epoch/nonce; si no coincide, limpiar datos viejos antes de mostrar nuevo juego.
8. Mover ventana conserva coordenadas lógicas por monitor, recalcula DPI y clamps a work area. Monitor desconectado devuelve ventana a monitor disponible sin alterar core.
9. AltTab/editar nota enruta teclado a UI. Gamepad tiene destinatario explícito; hotkey de cambio de foco libera botones. Auto-pausa por foco es opt-in y considera el grupo Player/Companion.
10. MULTI-02 prepara superficie hospedable en spike; solo después implementar Split y Dock con transacción visual. Mantener audio/core en worker y frame transport acotado. Si falla, volver a Dual sin reiniciar partida.

Prueba de aceptación: abrir homebrew, mover Companion a monitor 150% y Player a 100%; editar nota, alternar layouts, desconectar monitor, cerrar/reabrir Companion, guardar/cargar y cerrar Player. No hay teclas retenidas, sesión duplicada, datos de otro epoch ni guardado cruzado. Comparación estética de referencia 2 queda bloqueada por imagen ausente.

## 9. Plan de datos en vivo, adaptadores y matriz de compatibilidad por core/ROM

### 9.1 Capas separadas [PROPUESTO]

1. **Frontend:** reloj, audio, input, archivos, UI, rewind por serialización.
2. **ABI libretro:** exports/version/system_info, vídeo/audio, joypad/pointer, SRAM y serialize según respuesta real.
3. **Extensión propia:** mapas de memoria/hook versionado, lectura de bus y señales; no asumir presencia en RetroArch DLL pública.
4. **Juego:** identidad estricta, revisión, lenguaje, cifrado/checksum/offsets y reglas de datos.
5. **Evidencia:** detected no implica verified; pruebas registradas por build/ROM legal/core hash.

Registro adaptadores: GameDataAdapter.describe/match/captureRequirements/decode/validate. Manifest con identity predicates, regiones permitidas, tamaños máximos, schema, capacidades por módulo y licencia/procedencia. Match desconocido → referencia/manual. Un usuario puede elegir dataset de referencia para un hack; esa elección no habilita automáticamente offsets live.

Capturar memoria solo entre frames; copiar regiones necesarias a buffer propio; decodificar fuera; límite de un decode pendiente por sesión y reemplazo de solicitud vieja. SRAM no equivale a RAM activa. retro_get_memory_data(SAVE_RAM) puede contener solo guardado, no equipo actual. SET_MEMORY_MAPS requiere traducir select/disconnect/offset/addrspace y comprobar rangos; si no se implementa fielmente, no afirmar soporte. API propia propuesta de lectura debe ser versionada y solo lectura, con capability discovery antes de invocarla.

Reusar Gen3SaveFile.parse como especificación/fixtures para Rust: 14 sectores por slot, checksum, firmas, IDs sin duplicados, contador/wrap, slot válido y bytes copiados. No activar formato por extensión. Confirmar cobertura party/bag/dex por formato; un save puede no contener rival/combate actual. En Android separar consultas del Poller de efectos nativos/automatismos existentes, preservando estos últimos detrás de su configuración explícita.

### 9.2 Matriz documental inicial

| Plataforma/sistema | Core/ruta observada | Arranque/A-V/input | SRAM/estados/rewind/FF | Companion/Link |
|---|---|---|---|---|
| Windows GB | Alias mGBA público local; E06 | [NO-VERIFICADO] Código host, no ejecución auditada | [EXISTENTE-PARCIAL] SRAM/state/FF; rewind pendiente | [BLOQUEADO] Adapter Desktop no integrado; Link no probado |
| Windows GBC | Mismo grupo DLL GB/GBC | [NO-VERIFICADO] No inferir GB=GBC probado | [EXISTENTE-PARCIAL] Mismas limitaciones | [BLOQUEADO] Por juego/core |
| Windows GBA | armGBA fuente; DLL propia ausente | [BLOQUEADO] Core a buildar o alternativa pública a validar | [NO-VERIFICADO] ABI no garantiza formato | [BLOQUEADO] Extensiones/adapter pendientes |
| Windows DS | melonDS DS público local; ruta Linux DSperate distinta | [NO-VERIFICADO] Joypad host; stylus pendiente | [NO-VERIFICADO] Estados/BIOS/options por motor | [BLOQUEADO] No hay integración Desktop de datos/Link |
| Android GB | mGBA JNI, lectores Gen1 | [EXISTENTE-PARCIAL] Implementación; dispositivo pendiente | [EXISTENTE-PARCIAL] Código; soporte por modo | [EXISTENTE-PARCIAL] Por identidad exacta, no toda ROM |
| Android GBC | mGBA JNI, lectores Gen2 | [EXISTENTE-PARCIAL] Igual criterio | [EXISTENTE-PARCIAL] Verificar por core/build | [EXISTENTE-PARCIAL] Datos por juego; Link específico no demostrado |
| Android GBA | mGBA JNI; configs Gen3 | [EXISTENTE-PARCIAL] No asumir armGBA Android | [EXISTENTE-PARCIAL] Slots/rewind/FF existentes | [EXISTENTE-PARCIAL] Lectores extensos; co-op especializado con flags |
| Android DS | melonDS y DSperate en CMake/JNI | [EXISTENTE-PARCIAL] Render/touch existentes a validar | [EXISTENTE-PARCIAL] Estados segregados por core; no intercambio asumido | [EXISTENTE-PARCIAL] Gen4/5 por versión; Link pendiente de prueba |

Matriz final requerida: cada celda incluye acción, plataforma, ABI, coreHash, ROM identity o fixtureId, adapterVersion, fecha, testId, resultado y limitación. No cambiarla a soportado por presencia de código o DLL.

## 10. Roadmap, dependencias, prioridades, MVP/intermedia/completo

Todo el roadmap es **[PROPUESTO]**; tamaños relativos, sin fechas inventadas.

~~~mermaid
flowchart TD
  F0[QA-02 inventario y fixtures] --> INF[INFRA-01 contratos]
  INF --> MIG[BRAND-02 migración]
  INF --> ID[WIN-LIB-01 identidad]
  INF --> CORE[WIN-CORE-01 probe]
  ID --> VAULT[WIN-VAULT-01 transacciones]
  MIG --> VAULT
  CORE --> PLAY[WIN-PLAY-01 loop real]
  VAULT --> PLAY
  PLAY --> BUS[MULTI-03 y MULTI-07]
  BUS --> DUAL[MULTI-01 y MULTI-04 Dual]
  ID --> DATA[NEW-DEV-01 referencia]
  DATA --> OFF[CMP seis módulos offline/manual]
  OFF --> LIVE[MULTI-05 adaptadores live]
  DUAL --> LIVE
  DUAL --> DOCK[MULTI-02 spike y Dock]
  VAULT --> AND[AND-01 a AND-09]
  AND --> SYNC[WIN-SYNC-01 y NEW-SAVE]
  PLAY --> LINK[WIN-LINK-01 spike]
  LIVE --> QA[QA-01 integración]
  DOCK --> QA
  SYNC --> QA
  QA --> REL[REL-01 release]
~~~

| Fase | Cortes/orden de commits propuestos y archivos | Puerta de salida / fallback |
|---|---|---|
| F0 auditoría humana · [HECHO-PARCIAL 2026-11-16] | QA-02 inventario → QA-01 fixtures → confirmar toolchains e imágenes. docs/matriz y tests futuros | Evidencias anteriores separadas de nuevas; segunda imagen y hardware faltantes registrados. No declarar baseline verde sin correrla. |
| F1 contratos/seguridad · [HECHO-PARCIAL 2026-11-16] | INFRA-01 → identidad → BRAND-02 → Vault atomic; D/domain/protocol/storage, contratos Kotlin | Migración interrumpida recupera; datos originales intactos. Fallback raíz vieja solo lectura. |
| F2 Player verificable | WIN-CORE-01 → system_info/callbacks → WIN-PLAY-01 → input/AV → guardado/cierre | Demo legal ROM→core→frame→input→save→cerrar. Un sistema no acreditado queda pendiente. |
| F3 biblioteca/ajustes | WIN-LIB-01/03/02 → WIN-SYS-01 → WIN-SET-01; U screens y catálogo | Import/reinicio/relocalizar/core ausente/estado válido. No añadir catálogo remoto para cerrar MVP. |
| F4 Companion offline | NEW-DEV-01 referencia → Dex/Battle → Team planner/Bag catálogo → Atlas/Guide/notas → UI-01/02 | Seis módulos útiles sin inventar datos. Fallback textual y manual explícito. |
| F5 live | MULTI-03/07/04 → un adapter con fixtures → seis módulos según capacidades | Una ROM/revisión acreditada antes de ampliar; rollback desactiva pack/adapter. |
| F6 ventanas | MULTI-01/06 → spike MULTI-02 → Split/Dock → MULTI-08 DS | Cambios de vista sin reinicio, DPI y desconexión monitor. Dual como fallback. |
| F7 Android | AND-09/01 → AND-02/05 → AND-04/08/06/03 | Upgrade/lifecycle/SAF/core tests en hardware; conservar UI y servicios útiles actuales. |
| F8 avanzadas | NEW-SAVE transferencia → conflicto/LAN → rewind optimizado → Link solo con spike | Integridad y dos dispositivos; si Link no viable, no bloquea resto. |
| F9 release | BRAND-01 final → DOC-01 → QA-01/02 → REL-01 | Matriz verificable, notices, firmas, rollback y capturas reales. No empaquetar binarios sin origen acreditado. |

MVP útil: biblioteca segura, al menos un sistema de reproducción demostrado, Vault robusto, configuración real, Companion offline/manual y Dual básico. Excluye Link, nube, plugins, grabación y live generalizado. No liberar si hay riesgo de pérdida/cross-save, core crash que cierre biblioteca o importador sin límites.

Intermedia/V1: adaptadores live por revisiones comprobadas, Dual sólido, Split/Dock si spike aprueba, controles/DS, Android migrado y transferencia local con conflictos. No vender paridad total si un core carece de capacidad.

Completa condicionada/V2: más adapters, LAN seguro, DS avanzado, optimización, nuevos módulos seleccionados y Link únicamente donde haya prueba de dos instancias. Lo experimental no es condición de entrega del producto base.

## 11. Primeras tareas entregables para el desarrollador

Todas heredan reglas de sección 7.1 y Ready/Done de sección 18. Las tareas con spike no autorizan construir integración final antes de resolver la incógnita. **Estado 2026-11-16:** T01/T02/T03/T05/T08 tienen ejecución parcial registrada en §1.5; el resto sigue **[PROPUESTO]** salvo que su fila indique otra cosa.

| Issue / prioridad / tamaño / dueño / hito | Dependencias, próximo archivo y contrato | Pasos concretos / prueba falsable / riesgo y cierre |
|---|---|---|
| T01 QA-02 P0 S QA F0 · [HECHO-PARCIAL 2026-11-16] | Ninguna; leer E01–18, docs status, core hashes | Crear matriz de evidencia separando detectado/probado y registrar imagen faltante. Fixture/testId por celda; falla si hay Ready sin evidencia. Riesgo documental, rollback simple del registro. |
| T02 INFRA-01 P0 M Rust F1 · [HECHO-PARCIAL 2026-11-16] | T01; shell.rs Action/publish → D/domain/protocol y U/bridge | Definir Envelope, AppError, CommandId/Ack, límite y esquema; adaptar una acción de lectura y una setting. Probar mensaje desconocido, truncado, doble envío y ACK perdido; no interpretar rutas arbitrarias. |
| T03 WIN-LIB-01 P0 M Rust F1 · [HECHO-PARCIAL 2026-11-16] | T02; main.rs scan_games → inspect.rs | Diseñar GameIdentity y SourceId; parser acotado GB/GBC/GBA/DS, hashes streaming, región unknown. Fixtures válidos/truncados y falsa firma; no indexar solo por extensión. |
| T04 BRAND-02 P0 L Rust F1 | T03; settings.rs → storage/migration | Preview/journal/copia/verificación/commit; mapear saves homónimos como conflicto. Cortar después de cada write; nueva ejecución recupera sin sobrescribir original. Requiere corpus sintético legado. |
| T05 WIN-VAULT-01 P0 L Rust F1 · [HECHO-PARCIAL 2026-11-16] | T03/04; main.rs save_battery/state_path, shell save_files | SaveManifest y AtomicWrite; aislar por hash/kind/core; persistir errores. Pruebas ENOSPC, permiso denegado, rename fallido, corte y CRC alterado. Cierre solo cuando backup restaurable. |
| T06 WIN-CORE-01 P0 M FFI F2 | T02; L Api::load/LibretroCore::load → cores/probe | Enumerar exportaciones requeridas, PE machine, ABI; añadir reporte system_info y probe aislado con timeout. DLL de texto y core fixture incompatible producen reporte, launcher vivo. No autoejecutar cualquier DLL encontrada. |
| T07 WIN-CORE-02 P1 L build F2 | T06; libretro CMake, armgba CMake, linux/libretro-ds CMake | Spike Windows por motor: fuentes, compilador, POSIX/JIT/deps, licencia y exports. Entregar matriz viable/bloqueado y primer artefacto reproducible futuro; no confundir DSperate con melonDS DS. |
| T08 WIN-PLAY-01 P0 L Rust F2 · [HECHO-PARCIAL 2026-11-16] | T05/06; main.rs open_game/DropFile y shell play | Introducir SessionIdentity inmutable y supervisor; DropFile solo importa, no retarget save. Probar A/B homónimos, Escape y segundo Play, child crash. No guardar jamás por selected. |
| T09 WIN-PLAY-01 AV P0 L Rust F2 | T08; main audio freq/frame delay, L av_info | Reloj del core, audio resample/backpressure, geometría dinámica y callbacks de dupe frame. Core fixture 50/59.x/60 Hz y tasas distintas; medir deriva/underrun. Audio error visible, no cola infinita. |
| T10 MULTI-03/07 P0 L Rust F2 | T08/T02; session/transport | Named pipe ACL/nonce, comandos fiables y latest-only snapshots; disconnect/cancel/reconnect. Inyectar mensaje viejo, oversize y consumidor lento; Stop sigue respondiendo y memoria acotada. |
| T11 WIN-LIB-03 P1 M Rust+UI F3 | T03/04; shell import, U Import | Job cancelable, progreso, varios roots, ZIP cuotas y selección múltiple explícita; persistir resultados confirmados. Fixture archive traversal/bomb/múltiples ROM/Unicode y permiso denegado por entrada. |
| T12 WIN-LIB-02/SYS P1 M UI F3 | T11/T06; app.jsx → Library | IDs estables, filtros/favoritos/recientes/contexto, missing/relink y vista persistida; prueba escanear mientras se hace clic, quitar índice conserva archivo. |
| T13 WIN-SET-01 P1 M UI+Rust F3 | T02/05; Settings::load/save, Settings.jsx | Schema patch por scope, herencia y validation; guardado efectivo confirmado, tecla conflictiva, reset sección. Test dos ventanas modifican revisión antigua: conflicto, no pérdida. |
| T14 NEW-DEV-01 P1 L datos F4 | T03; gamedata README/generador → targets/rust y schema packs | Exportar datos de referencia mínimos con licencia/version; fixtures equivalentes Kotlin/Rust; no tocar outputs a mano. Dataset faltante conserva manual y rechaza ids cruzados. |
| T15 CMP-DEX/BATTLE P1 L datos+UI F4 | T14; TypeChart*, DexDetails → reference/dex/battle | Dex offline, reglas generación explícitas, calculadora manual; Golden fixtures inmunidades/doble tipo/cambio generación, búsqueda offline y navegación contexto. |
| T16 CMP-TEAM/BAG P1 L datos+UI F4/F5 | T14/T10; SnapshotView/Gen3SaveFile → team/bag | Separar Party real de planner y Bag real de catálogo; implementar fuente save estricta primero donde parser válido. Fixture checksum, slots, count inválido y cache stale; hashes save sin cambios. |
| T17 CMP-MAP/GUIDE P1 L datos+UI F4 | T14; Guide*/RegionMap → atlas/guide | Mapa permitido o textual, notas/checklist por dataset y spoiler antes de búsqueda. Test vista sin mapas, undo persistido, cambiar dataset y conservar notas huérfanas. |
| T18 MULTI-01/04/06 P1 L Desktop F5/F6 | T10/T15–17; shell.run → workspace | Segunda WebView, store por sesión/epoch y focus lease; 2 monitores/DPI mixto/desconexión/tecla sostenida; cerrar Companion no detiene Player. |
| T19 MULTI-05 P1 XL datos/FFI F5 | T10/T14/T16; MemoryReader/Poller → adapter registry | Elegir UNA revisión con fixture legal y memoria expuesta; capture→decode→validate; no fallback libre al core fuera del owner. Inyectar estado atrasado tras load: se descarta. |
| T20 AND-09/01/05 P0 L Android F7 | T03–05; Gradle ID, RomIntake, LibraryActivity, SaveStates | Upgrade mismo ID/firma, permisos SAF revocados, manifests/legacy CRC y lifecycle. Requiere dispositivo o emulador ABI válido; no marcar DoD con solo unit tests. |

## 12. QA, pruebas E2E, seguridad, licencia, rendimiento y accesibilidad

**[PROPUESTO]** La validación futura se organiza por riesgos, no por número de botones.

| Suite | Fixtures / acciones | Resultado exigido y señal de fallo |
|---|---|---|
| ROM/archivo | Headers sintéticos válidos y truncados; extensión falsa; GBC dual-mode; DS rangos ARM9/ARM7 inválidos; tamaños con overflow | Identidad correcta o rechazo explicado; ninguna lectura fuera de rango ni hash ilimitado en UI. Homebrew legítimo no se rechaza solo por no ser retail. |
| Compresión | ZIP traversal, absoluto/UNC, ADS, device names, symlink, ratio extremo, tamaño declarado falso, entrada duplicada, contenedor anidado y múltiples ROM | Sin escape de staging, cuotas durante lectura, cancelación y selección explícita. Rechazar formatos no soportados; no ejecutar contenido. |
| Duplicados | Misma ROM en 2 rutas, mismo nombre con bytes distintos, revisión distinta, archivo cambiado mientras se analiza | Un GameId por bytes; sources separados; variantes no mezcladas; revalidación TOCTOU antes de Play. |
| Core | Fixtures ABI incorrecta, exports faltantes, architecture distinta, init colgado/crash, core con need_fullpath y con estados no disponibles | Probe aislado, timeout, reporte saneado; no Ready por existencia ni serialize_size diferente de cero solamente. |
| A/V/input | Core sintético con patrones de vídeo/audio, distintas tasas, geometry changes, frame duplication, pointer y joypad | Pixels y muestras esperados; pacing medido; no desborde acumulado de audio; entradas liberadas al cambiar foco. |
| Vault/migración | Guardados sintéticos, dos slots Gen3, wrap contador, checksum inválido, homónimos, destino lleno, corte por fase | Originales intactos; último commit válido recuperable; no restaurar ROM/core incompatible ni dar éxito en write fallido. |
| Companion | Capturas de memoria sintéticas, ids extremos, strings inválidos, fuente manual/save/live y epoch atrasado | Validación por módulo; unknown preservado; cero modificación de memoria/save; no mezcla de sesiones. |
| Reglas | Matrices de tipos por generación/hack, evolución y movimientos por dataset | Golden vectors independientes de implementación; inmunidades/dual type y supuestos visibles. |
| UI | Playwright mock para componentes + automatización nativa Windows separada; Compose/Paparazzi + instrumentación | Mock no acredita IPC ni emulación. Teclado/gamepad/lector, estados vacío/error/disabled y acciones reales. |
| Android | Upgrade mismo certificado, URI revocada, rotation/background/process death/audio focus, mando unplug | Datos conservados, reanudación consistente, no teclas/touch pegados, no reiniciar core por recomposición. |
| Multiwindow | Dos monitores/DPI, cerrar/reabrir, AltTab, hot layout change, desconexión worker | Fuente única, memoria acotada, foco correcto y core no reiniciado por mover una vista. |
| Transferencia/Link | Bundles corruptos/hostiles, ancestro común/divergente, peer no autorizado, desconexión | Consentimiento, integridad, copias en conflicto y cierre limpio; Link exige prueba real independiente por motor. |

Seguridad: limitar también cantidad total de callbacks/audio por frame, dimensiones, pitch, tamaño de state y mensajes IPC; límites distintos por dominio, no reutilizar MAX_FRAME_BYTES como regla universal de archivos. DLL nativa es código con permisos del proceso: aislamiento de crash no es sandbox. No afirmar que validar exports hace segura una DLL maliciosa. Instalar/probar solo motores de origen verificable y elegidos por usuario. DLL search path controlada y dependencias resueltas sin descargar nombres encontrados automáticamente.

WebView mantiene recursos locales y CSP restrictiva; no navegar contenido de ROM/guía como HTML ejecutable. Sanitizar Markdown/URLs de packs y no exponer comandos de filesystem arbitrarios. Backups/exportaciones usan allowlists y límites; no incluir ROM/BIOS/credenciales sin selección explícita compatible con producto. Logs sin contenido de memoria, tokens, nombres privados ni rutas completas por defecto. Telemetría off; reporte con preview y consentimiento.

Accesibilidad: texto normal ≥4.5:1 y UI esencial/grande ≥3:1 como objetivos de diseño a medir; foco visible no oculto, estados con icono+texto, nombres accesibles, orden de tab coherente, Escape/back predecible, objetivos Android ≥48 dp, escala de texto 200%, Windows 100/125/150/200%, reduced motion. No animar HP con interpolación que aparenta muestras inexistentes: animación visual termina en valor observado y se etiqueta antigüedad.

Licencias: un manifiesto de recursos por artefacto incluirá fuente, commit/version, licencia, modificaciones, texto de atribución y obligaciones de distribución. Capturas de videojuegos no sustituyen autorización para redistribuir sprites/mapas. Priorizar recursos originales y extracción local cuando legalmente proceda; no publicar contenido privado del usuario. El NOTICE y términos adicionales existentes deben conservarse. No cambiar derechos/autores por rebranding.

## 13. Riesgos, restricciones técnicas, bloqueos y decisiones por validar

| ID / estado / prioridad | Riesgo | Acción futura y criterio de desbloqueo |
|---|---|---|
| R01 [DEUDA-TÉCNICA] P0 | Colisión/retarget de guardado Desktop | T05/T08: sesión inmutable, hash, atomic write; pruebas A/B y DropFile. |
| R02 [NO-VERIFICADO] P0 | DLL públicas no prueban extensiones ni procedencia | T06/T07: obtener fuente/hash verificable, licencias, probe y homebrew; no publicar DLL locales ahora. |
| R03 [BLOQUEADO] P1 | Segunda referencia visual ausente | Recibir archivo original y registrar viewport; revisar solo diferencias visuales sin rediseñar contratos. |
| R04 [BLOQUEADO] P1 | armGBA Windows y ruta DS propia no acreditadas | Spike CMake/toolchain/architecture/JIT; alternativa público play-only con capabilities honestas. |
| R05 [DEUDA-TÉCNICA] P1 | Pacing/audio fijo y cola sin presupuesto de extremo a extremo | T09 instrumenta y prueba con core sintético; errores visibles. |
| R06 [BLOQUEADO] P1 | Split/Dock real con SDL/Wry | Spike nativo con foco/DPI/fullscreen y no reinicio; fallback Dual explícito. |
| R07 [EXISTENTE-PARCIAL] P1 | Android madura tiene escrituras/cheats/co-op no cubiertos por Companion read-only | Separar capacidades/acciones y preservar opciones consentidas; no borrarlas ni activarlas desde consultas. |
| R08 [NO-VERIFICADO] P1 | Identidad por código/revisión insuficiente para ciertos hacks | Fingerprints y adapter manifests; desconocidos solo referencia/manual. |
| R09 [BLOQUEADO] P1 | Dispositivo/hardware/corpus legal no demostrado en esta auditoría | Inventario futuro del implementador y tests físicos; no asumir que ADB sigue ausente. |
| R10 [DEUDA-TÉCNICA] P1 | MANAGE_EXTERNAL_STORAGE en Android | SAF como flujo ordinario, migración gradual y casos de permiso revocado. |
| R11 [NO-VERIFICADO] P1 | Datos/licencias de mapas/carátulas/guías incompletos | Manifest de procedencia; placeholder/texto antes que scraping o arte de mockup. |
| R12 [EXISTENTE-PARCIAL] P2 | Docs/branding previos pueden haber cambiado enlaces reales o nombres de APK | DOC-01 verifica anchors y destinos reales; no hacer renombrado textual indiscriminado. |
| R13 [NO-VERIFICADO] P2 | Versiones de contrato Rust/Kotlin divergen | Golden schemas/fixtures CI y compatibilidad N/N-1 explícita; consumidores rechazan major incompatible. |
| R14 [BLOQUEADO] P2 | Link local/remoto depende de API/determinismo/juego | Spike por core, dos instancias y recuperación tras disconnect; no prometerlo en MVP. |

Decisiones aún abiertas: dependencia SQLite concreta; formato/codec de captura; estrategia de superficie nativa; arquitectura armGBA Windows; primeros juegos/revisiones con evidencia legal; política de distribución de cores; umbrales medidos de stale/performance. Cada una tiene fallback definido, no exige detener la biblioteca básica.

## 14. Guía de traspaso al desarrollador y comandos FUTUROS de validación

**No ejecutados durante esta auditoría.** El desarrollador deberá preservar los cambios previos y crear cortes revisables. Primero registrar toolchains y ejecutar la línea base antes de adjudicar fallos a la migración. No compartir logs con secretos o rutas personales sin saneamiento.

| Ubicación | Comando futuro | Propósito / limitación |
|---|---|---|
| desktop-rust/ui | npm ci; npm run build; npm test | Instalar desde lock, empaquetar UI y pruebas existentes. Necesita navegador que espera test/ui.test.mjs; mock no demuestra emulador. |
| desktop-rust | cargo fmt --all --check; cargo check --workspace; cargo test --workspace | Validación Rust y formatos. No ejecutar fmt sin --check durante una auditoría. |
| desktop-rust | cargo clippy --workspace --all-targets; cargo build --release | Lint/build Windows con toolchain MSVC/SDK adecuados y recurso UI generado. Registrar warnings previos. |
| desktop-rust | cargo run --release; cargo run --release -- --sdl | Smoke launcher nativo y fallback SDL. Abrir solo fixture/homebrew legal, en raíz de datos de prueba. |
| raíz | ./gradlew :app:testDebugUnitTest --stacktrace; ./gradlew :app:assembleDebug --stacktrace | En Windows usar gradlew.bat. JDK17, SDK34, NDK/CMake declarados; no acredita hardware ni release firmado. |
| raíz | ./gradlew :app:connectedDebugAndroidTest | Solo después de añadir/configurar instrumentación y dispositivo/ABI; documentar si tarea/corpus no existe todavía. |
| raíz | python scripts/gen_gamedata.py --check; python scripts/gen_nds_ports.py --check | Verificar que fuentes y salidas generadas coinciden; no generar a mano durante revisión. |
| raíz | python scripts/check_translations.py | Consistencia i18n; revisar comportamiento antes de invocarlo. |
| website | npm ci; npm test; npm run build | Manual/web; scripts prebuild pueden sincronizar/generar archivos, no son solo lectura. |
| futuros core builds | scripts/build-vyntra-cores-windows.ps1 | Archivo todavía propuesto; no es comando ejecutable disponible. Crear tras T07, registrar fuentes y licencias. |

Validación nativa futura: elegir datos temporales separados, no la carpeta real del usuario; capturar build/commit/coreHash/fixtureId, vídeo del flujo y reportes de pruebas; marcar resultados pendientes/skipped explícitamente. Una prueba con ROM privada puede registrarse por fixture/identidad anonimizada sin subir ROM ni save.

## 15. Matriz final de cobertura de pantallas / botones / tareas / nuevas funciones

**[PROPUESTO]** Esta tabla enlaza la especificación de controles de sección 6, fichas de sección 7, primeras issues y backlog de sección 20. Ningún control del inventario queda sin responsable de feature; semejanza con imagen 2 permanece no verificable.

| Área / controles agrupados | IDs base | Nuevas capacidades asociadas | Evidencia / prueba futura |
|---|---|---|---|
| Marca, splash, iconos, títulos, tema/foco/motion | BRAND-01/02, UI-01/02 | NEW-LIB-09, NEW-AND-08 | E05/E18; QA visual/DPI/i18n/upgrade |
| Biblioteca/import/búsqueda/filtros/grid/lista/selección/contexto/relink | WIN-LIB-01/02/03, WIN-SYS-01 | NEW-LIB-01 a NEW-LIB-09 | E02/E03/E05; T03/11/12, E2E-02 |
| Núcleos/ruta/opciones/probar/logs/rollback | WIN-CORE-01/02 | NEW-PLY-09/12, NEW-DEV-05/07 | E06/E15; T06/07, E2E-03/10 |
| Play/Continue/pausa/salir/fullscreen/input/FF/rewind/overlay | WIN-PLAY-01/02 | NEW-PLY-01 a NEW-PLY-10 | E02/E06; T08/09, E2E-03/07/11 |
| DS pantallas/swap/táctil/layout | MULTI-08, AND-03 | NEW-PLY-11 | E06/E10; E2E-09 |
| Ventanas/monitor/Dual/Split/Dock/Compact/foco | MULTI-01 a MULTI-07 | NEW-CMP-02/03 | E03; T10/18, E2E-04/05 |
| Home Companion/origen/estado/refresh | MULTI-04/05, AND-04 | NEW-CMP-01/04/16/17 | E11; T19, E2E-05/07 |
| Equipo/detalle/Dex/planificador | CMP-TEAM-01 | NEW-CMP-05/06 | E11/E12; T16 |
| Mochila/categorías/buscador/ficha | CMP-BAG-01 | NEW-CMP-14/17 | E11/E13; T16 |
| Dex/filtros/evoluciones/movimientos/encuentros | CMP-DEX-01 | NEW-CMP-08/13/14 | E12/E14; T14/15, E2E-06 |
| Battle/selección/modo/manual/afinidades | CMP-BATTLE-01 | NEW-CMP-07/08/09 | E12; T15, E2E-05 |
| Atlas/zoom/pan/zona/marcadores/guía | CMP-MAP-01 | NEW-CMP-12 | E14; T17, E2E-06 |
| Guide/Quests/notas/spoilers/checklist | CMP-GUIDE-01 | NEW-CMP-10/11/15/18 | E14; T17, E2E-06/12 |
| Vault/slots/save/load/import/export/backup/recuperar | WIN-VAULT-01, AND-05 | NEW-SAVE-01/02/03/04/07/08, NEW-PLY-05 | E13; T05/20, E2E-01/07/08 |
| Sync/perfiles/dispositivos/conflictos | WIN-SYNC-01, AND-07 | NEW-SAVE-05/06/08, NEW-AND-03/07 | E03/E16; E2E-08 |
| Link/crear/unirse/desconectar | WIN-LINK-01, AND-07 | EXP-03 | E16; prueba dos instancias por core |
| Pocket Home/menú/bottom/Atrás/controles/settings/legal | AND-01 a AND-09 | NEW-AND-01 a NEW-AND-08 | E07–E13; T20, lifecycle y accesibilidad |
| Ayuda/diagnóstico/releases/web/manual | INFRA-01, DOC-01, REL-01, QA-01/02 | NEW-DEV-01 a NEW-DEV-08 | E14/E17/E18; contratos/licencias/build futuro |
| Extensiones deliberadamente no base | Tickets de sección 20 | EXP-01 a EXP-08 | Gates/spikes; no botones prometidos sin evidencia |

## 16. Registro de decisiones ADR, criterios y alternativas descartadas

Cada ADR es **[PROPUESTO]**, pendiente de aprobación técnica del implementador con evidencia del spike indicado.

| ADR | Contexto / opciones | Recomendación / consecuencias / reversibilidad / reconsiderar si… |
|---|---|---|
| ADR-01 Ventanas e IPC | E03 ya separa Player; comparar core en shell, child+pipe y apps por TCP | Shell comparte store entre WebViews; core aislado, pipe privado. Dual primero, superficie hospedable condicionada. Contratos preservan cambio de transporte. Reconsiderar si spike Wry/SDL incumple latencia/foco/DPI. |
| ADR-02 Cores | E06 ABI parcial; DLL públicas y fuentes personalizadas diferentes | Descriptor por hash/origen, probe aislado, libretro estándar para play donde basta, extensiones separadas para datos. No renombrar terceros como propios. Reconsiderar al existir API upstream equivalente y probada. |
| ADR-03 Companion | Android tiene lectores y datos generados; Desktop carece de integración | Captura inmutable + adapters por identity + reference provider offline. Portar reglas/fixtures antes que UI o offsets aislados. Packs/adapters desactivables sin romper juego. Reconsiderar si parser compartido FFI reduce mantenimiento demostrado. |
| ADR-04 Storage | JSON básico y filenames frente a catálogo creciente/transacciones | JSON settings/manifests, SQLite catálogo sujeto licencia/benchmark; blobs en archivos con journal. Imports copy-first. Reversible mediante export portable y raíz vieja intacta. Reconsiderar SQLite si coste despliegue supera corpus real. |
| ADR-05 Paridad | Kotlin/Compose/JNI maduros frente a Rust/Wry/SDL | Equivalencia de contratos y fixtures, implementaciones nativas; no reescribir Android en Rust ni UI en otro framework. Reconsiderar solo con medidas de drift/coste y API compartida pequeña. |
| ADR-06 Offline/live | Datos desconocidos no autorizan inferencias | Manual/referencia producto válido; live exige adapter verificado; provenance siempre visible, stale explícito. Reversible por feature gate; reconsiderar cadencia por mediciones, no retirar procedencia. |
| ADR-07 Recursos/licencias | Mockup no otorga derechos; NOTICE y motores externos | Manifest de origen/atribución por asset/core, originales o datos locales legítimos, sin ROM/BIOS comerciales. Descarga externa opt-in después de revisión. Reconsiderar proveedor al cambiar licencia/servicio. |

ADRs adicionales recomendados: scopes de input y atajos; formato de bundle/consentimiento; límites de plugin SDK; política de firma/actualización; separación consulta/edición del juego; modelo de epoch para rewind y timeline.

## 17. Catálogo de contratos y máquinas de estados propuestos, sin código ejecutable

**[PROPUESTO]** Versionar esquemas en contracts/vyntra/v1 y crear fixtures equivalentes para Rust/Kotlin/JS. Major incompatible se rechaza con mensaje; campos opcionales nuevos tolerados según schema. No serializar rutas nativas sin tipo locator; no usar enteros sin límites para tamaños o índices.

| Contrato | Límite / ownership / consumidor | Condición de rechazo |
|---|---|---|
| UiCommand/UiAck | Shell valida action, scope, session, revision; UI no escribe filesystem | Schema desconocido, payload excesivo, commandId repetido con distinto contenido, revisión vieja |
| WorkerHello/SessionConfig | Supervisor fija core/ROM/hash/paths; nonce único | ABI protocolo distinta, identidad cambiada, core sin autorización |
| InputFrame | Secuencia monotónica, joypad/stylus normalizados, FocusLease | Lease vencida, coordenada fuera de superficie, sesión cerrada |
| MemoryCapture | Buffers propios, rangos declarados, epoch/frame, sin punteros persistentes | Rangos fuera de mapa, datos mutables fuera del owner, captura demasiado grande |
| SnapshotEnvelope | Último valor, modules opcionales, provenance por campo/módulo | epoch/session distinto, revisión anterior, validación semántica fallida |
| SaveManifest/TransferManifest | Límites por kind, hashes de payloads y schema | ROM/core/format incompatible, tamaño falso, path traversal, entrada no permitida |
| ReferencePackManifest | dataset identity/language/version/license/contentHashes | Licencia ausente, schema incompatible, IDs cruzados o contenido ejecutable |
| SettingsPatch/LayoutProfile | ExpectedRevision, ámbito, campos portable/local | Conflicto, ruta no autorizada, monitor inexistente sin fallback, tecla reservada |
| DiagnosticsExport | Preview/consentimiento, allowlist de campos, truncado de hash | Token/ruta privada/contenido memoria sin autorización explícita |

~~~mermaid
stateDiagram-v2
  [*] --> Idle
  Idle --> Preflight: Jugar
  Preflight --> Starting: ROM/core/save válidos
  Preflight --> Error: validación falla
  Starting --> Playing: AV/input listos
  Starting --> Error: load falla
  Playing --> Paused: pausa o política foco
  Paused --> Playing: reanudar
  Playing --> Restoring: cargar estado
  Paused --> Restoring: cargar estado
  Restoring --> Paused: éxito y epoch nuevo
  Restoring --> Recovery: fallo
  Recovery --> Paused: rollback válido
  Recovery --> Error: core inconsistente
  Playing --> Stopping: salir
  Paused --> Stopping: salir
  Stopping --> Idle: flush confirmado
  Stopping --> SaveError: persistencia falla
  SaveError --> Stopping: reintentar o exportar copia
  Playing --> Crashed: worker termina
  Crashed --> Idle: informe y recuperación
~~~

~~~mermaid
stateDiagram-v2
  [*] --> Reference
  Reference --> Manual: usuario elige
  Reference --> Detecting: sesión disponible
  Manual --> Detecting: solicitar live
  Detecting --> Live: identity y adapter válidos
  Detecting --> Unsupported: no hay adapter
  Detecting --> Cached: snapshot desde save
  Cached --> Live: primera lectura válida
  Live --> Stale: timeout o core perdido
  Live --> Detecting: epoch cambió
  Stale --> Live: snapshot vigente
  Stale --> Reference: sesión cerrada
  Unsupported --> Manual: cálculo/consulta manual
~~~

~~~mermaid
stateDiagram-v2
  [*] --> Discovered
  Discovered --> StaticRejected: arquitectura/formato inválido
  Discovered --> Unverified: candidato válido
  Unverified --> Probing: usuario prueba
  Probing --> Loadable: ABI y init correctos
  Probing --> Failed: crash/timeout/dependencias
  Loadable --> NeedsConfiguration: BIOS/opción requerida
  Loadable --> VerifiedForProfile: smoke de perfil acreditado
  VerifiedForProfile --> Unverified: hash/opciones relevantes cambian
~~~

Importación: Idle→Scanning→Inspecting→Committing→Completed, con CancelRequested→Cancelled y ReportPartial; no rollback de entradas ya confirmadas salvo acción explícita de quitar índice. Migración: Discovered→Previewed→Copying→Verifying→Committed; cualquier corte→Recovering usando journal. Transferencia: Preview→Validated→Conflict/Ready→Applying→Committed; fallo→preservar candidatas y reporte.

## 18. Paquete de entrega a tercero: Definition of Ready/Done, checklist y riesgos por ticket

**[PROPUESTO] Definition of Ready común:** ID/prioridad/dueño; evidencia E y ruta exacta; objetivo observable; contrato y límites; dependencias resueltas o spike explícito; fuente/fixture legal; estados/errores; pruebas positivas/negativas/adversariales/regresión; política de datos/rollback. No Ready para integración final si faltan APIs, hardware o licencia; Ready solo para el spike correspondiente.

**[PROPUESTO] Definition of Done común:** implementación integrada, pruebas ejecutadas y reportes vinculados, formatter/lint/build adecuados, UI accesible y español, persistencia reiniciable, errores visibles, sin datos ficticios ni cambios silenciosos de saves, matriz actualizada por core/ROM/build, documentación y revisión humana. No basta screenshot, unit test espejo de implementación o endpoint sin consumidor.

| Familia de ticket | Riesgo heredado / pruebas mínimas | Rollback / responsable de revisar |
|---|---|---|
| BRAND/UI/DOC | Rebrand rompe URL/atribución; accesibilidad/contraste/escala | Revertir recursos propios manteniendo migración compatible; UI+legal/documentación |
| LIB/PATCH | Entrada hostil, TOCTOU, identidad equivocada | Desindexar sin borrar original; variantes en copia; Rust+QA seguridad |
| CORE/PLAY/MULTI | FFI crash, race, foco, timing, ABI o superficie | Desactivar motor/feature, conservar último save y Dual; FFI+QA nativo |
| VAULT/SAVE/MIGRATION/SYNC | Pérdida, corrupción, sobrescritura, conflicto | Journal, backup y versiones separadas; no downgrade en sitio; storage+QA fault injection |
| CMP/DEV datos | Adaptador equivocado, licencia o dato presentado sin fuente | Desactivar adapter/pack, volver a referencia/manual y conservar notas; datos+QA |
| AND | Sandbox/firmante/ABI/SAF/lifecycle | Mismo appId, backup/export previo y feature flags; Android+dispositivo QA |
| LINK/EXP | Dependencia externa o capacidad no demostrada | Off por defecto y motivo claro; nunca bloquear juego básico; especialista+QA dos instancias |

Cada fila de sección 20 hereda estas obligaciones según familia. La columna de prueba define su caso falsable específico; el fallback define comportamiento si no se satisface. El ticket final debe adjuntar evidencia, no convertir estimación de viabilidad en soporte.

Checklist por PR futuro: alcance y controles; contrato actualizado; schema migration; cancelación; límites; licencia; test/fixture; no secrets; no ROM/BIOS; log saneado; benchmark si toca loop; downgrade/fallback; matriz; captura real si UI; validación Android si cruza lifecycle; no cambios ajenos mezclados. La publicación de trabajo pendiente solicitada por el usuario es un checkpoint, no una PR de release aprobada.

## 19. Autoevaluación del plan: 12 controles de calidad, faltantes y límites explícitos

| Control | Estado de la planificación |
|---|---|
| 1 Cobertura de UI | [PROPUESTO] Inventario de controles mapeado en 6/15; referencia 2 visual pendiente. |
| 2 Cero implementación en auditoría | [EXISTENTE-VERIFICADO] Solo lecturas durante auditoría; después autorización específica para Markdown/commit/push, sin nuevas features. |
| 3 Hechos grounded | [EXISTENTE-VERIFICADO] E01–18; incertidumbres etiquetadas, código no equivale a runtime. |
| 4 Transferibilidad | [PROPUESTO] Catálogo de rutas, 20 primeras issues, contratos, backlog y pruebas; spikes señalados. |
| 5 No funciones ficticias | [PROPUESTO] Capabilities por core/juego; manual y live separados; Link condicionado. |
| 6 Fuente única | [PROPUESTO] SessionId/epoch/revision y owner; settings/catalog repositorios únicos. |
| 7 Guardados | [PROPUESTO] Copy-first, atomic commit, manifests, backups, conflictos y fault injection. |
| 8 Rendimiento/threading | [PROPUESTO] Core owner, capturas acotadas, decode fuera, latest-only y presupuestos a medir. |
| 9 Android | [PROPUESTO] SAF/ABI/JNI/lifecycle/firma/DS incluidos; pruebas físicas no realizadas. |
| 10 Legalidad/privacidad | [PROPUESTO] Licencias/orígenes, no ROM/BIOS, no secretos, exports allowlist. |
| 11 QA ejecutable después | [PROPUESTO] Casos falsables, comandos futuros y 12 secuencias; no resultados supuestos. |
| 12 Viabilidad | [PROPUESTO] MVP independiente de Link/nube/plugins; dependencia y fallback por idea. |

Escala de planificación: 0 ausente, 1 bosquejo, 2 especificada con validaciones pendientes, 3 especificación transferible sin incógnita mayor en el alcance evaluado. No puntúa la implementación actual.

| Dimensión | Puntuación / justificación |
|---|---|
| UI | 2/3: controles/tokens/estados, falta segunda imagen y validación visual completa Android. |
| Arquitectura | 2/3: decisiones/contracts claros; superficie Split/Dock requiere spike. |
| Datos live | 2/3: captura/adapters/confianza definidos; primeras revisiones y hooks por core a acreditar. |
| Cores | 2/3: inventario/ABI/probe/build gates; DLL no cargadas y Windows build propio pendiente. |
| Desktop | 2/3: ruta y tareas detalladas; A/V/multiwindow necesitan prueba física. |
| Android | 2/3: reutilización real identificada; gran superficie antigua necesita inventario granular durante implementación. |
| Migración | 3/3 de diseño: copia/journal/rollback/conflictos/ID y firma especificados; pruebas siguen futuras. |
| Riesgos | 3/3 de diseño: riesgos críticos con mitigación/gates, sin afirmar eliminados. |
| QA | 2/3: suites/secuencias definidas; corpus/hardware y umbrales pendientes. |
| Handoff | 2/3: backlog asignable y dependencias; XL/externas son spikes, no tickets de build totalmente Ready. |

Cobertura funcional implementada: **no cuantificable** con una auditoría documental. Cobertura del plan frente a apariencia de imagen 2: **no cuantificable**. No se anuncia 100% de producto terminado.

## 20. Nuevas funcionalidades: evaluación de valor, alcance, factibilidad y orden

Todas las filas son **[PROPUESTO]**; símbolos y archivos nuevos siguen sección 7.1. Cuando una función ya tiene antecedente Android, se evalúa como evolución/paridad, no como ausencia absoluta. M = MVP, V1 = intermedia, V2 = completa condicionada, X = experimental, D = descartada para el alcance actual. P0 integridad/seguridad; P1 producto base; P2 valor posterior; P3 exploración. Tamaño S/M/L/XL. Cada fila incluye dependencia, implementación semántica, fallback y prueba propia; hereda Ready/DoD/rollback de 18. Todas las capacidades externas permanecen [NO-VERIFICADO] hasta el spike descrito.

### 20.1 Biblioteca y ROM hacks

| ID / alcance / prioridad / tamaño / dueño | Dependencia; archivo/tipo/operación propuesta; flujo y validación | Fallback / prueba futura |
|---|---|---|
| NEW-LIB-01 M P0 L Rust | WIN-LIB-01; inspect.rs, GameIdentity/RomVariant, fingerprint(source). Header estructural → hash descomprimido → región/revisión opcional → relación base/patch solo acreditada. | Identidad parcial explícita; mismo filename con hashes distintos crea dos juegos; cabecera retail en hack no habilita live. |
| NEW-LIB-02 V1 P2 L Rust | NEW-LIB-01/VAULT; patches.rs, PatchDescriptor, previewPatch/applyCopy. IPS/BPS/UPS por parser/licencia acreditados, límites y checksums según formato; IPS sin hash base exige selección/validación adicional. | Aplicación externa/manual; fixture patch truncado/overflow/output gigante y base incorrecta no modifica original ni hereda save sin consentimiento. |
| NEW-LIB-03 V1 P2 M Rust+UI | WIN-LIB-02; repository.rs/Library.jsx, Collection/Tag/PersonalProgress, updateCollection/query. Watcher debounced → invalida cache → reinspection cancelable. | Escaneo manual; 10k entradas, eventos duplicados y rename masivo no duplican juegos; progreso es del usuario. |
| NEW-LIB-04 V1 P2 M UI+Rust | NEW-LIB-01/08; Library.jsx, VariantGroup, compareVariants. Agrupar por relación probada/manual etiquetada, mostrar hashes/región/core. | Lista independiente; dos revisiones nunca comparten state automáticamente. |
| NEW-LIB-05 M P1 M Rust | WIN-LIB-01; repository.rs, IntegrityReport, verify/relink. Archivo faltante → buscar solo roots elegidos → hash candidato → confirmar locator. | Reimportar sin borrar índice; archivo mismo nombre/bytes distintos no relocaliza silenciosamente. |
| NEW-LIB-06 V2 P2 L integración | NEW-DEV-01, consentimiento/licencia proveedor; library/metadata.rs nuevo, MetadataRecord, fetch/cache. Consultar identificador consentido, timeout, atribución, edición manual separada. | Cubierta Vyntra y metadatos locales; servicio caído/asset sin licencia no bloquea biblioteca ni sube ROM. |
| NEW-LIB-07 M ZIP / V1 7z P0 L Rust | WIN-LIB-03, parser permitido; import.rs, ArchivePolicy, enumerate/select/extractBounded. Rechazar rutas hostiles, quota por entrada/total, sin anidados por defecto. | ROM sin comprimir; archivo multi-ROM pide elección; tamaños falsos/cancelación no dejan import válido parcial. |
| NEW-LIB-08 V1 P1 M Rust+UI | WIN-SET-01/identidad; profiles.rs nuevo, GameProfile, resolveEffectiveProfile. Global→sistema→juego, preview de origen de cada valor, rutas no sincronizables. | Config global; override core eliminado vuelve a estado sin configurar, no motor aleatorio. Cheats solo opción existente/consentida y capacidad conocida. |
| NEW-LIB-09 M P1 M UI+Rust | BRAND-02/WIN-CORE-01; Onboarding.jsx nuevo, SetupProgress, advance/back/resume. Raíz/legado/core/controles con skip y resultados reales. | Biblioteca vacía con ayuda; cerrar en paso 3 retoma sin copiar dos veces ni descargar ROM/BIOS. |

### 20.2 Player avanzado

| ID / alcance / prioridad / tamaño / dueño | Dependencia; archivo/tipo/operación propuesta; flujo y validación | Fallback / prueba futura |
|---|---|---|
| NEW-PLY-01 M básico / V1 shaders P1 M GPU | WIN-PLAY-01/SET; session/video.rs nuevo, ConsolePreset, applyVideoProfile. Aspect/integer/filter, sampler adecuado y shaders solo con licencia/renderer soporte. | Escala simple; patrón cuadrícula demuestra aspect/integer y no recorta DS. |
| NEW-PLY-02 V1 P1 M Rust | T09; diagnostics.rs, PerformanceSample, sampleCounters. Distinguir frames core/presentados, percentiles/underruns; ring acotado, no rutas privadas. | Overlay off; core sintético con retraso controlado eleva métrica correcta, no FPS inventado. |
| NEW-PLY-03 V1 P2 M GPU+UI | WIN-PLAY-01; media/capture.rs y Gallery.jsx nuevos, ScreenshotManifest, captureFrame. Copia frame completo → encode worker → atomic file → miniatura. | Sin galería, export puntual; imagen excluye overlays por defecto y error ENOSPC no detiene core. |
| NEW-PLY-04 V2/X P3 XL multimedia | NEW-PLY-03, codec/licencia/benchmark; media/record.rs nuevo, RecordingSession, start/stop. Cola limitada, timestamps A/V, dropped frames y espacio mínimo. | Capturas; spike codec/software/hardware y prueba cierre forzado recupera archivo o informa truncado. Desactivar si degrada juego. |
| NEW-PLY-05 M P0 L Rust | WIN-VAULT-01/PLAY; session/suspend.rs nuevo, ResumeCandidate, checkpoint/resume. Snapshot antes cierre, manifest final, seleccionar compatible y pedir recuperación tras crash. | SRAM/arranque limpio; state incompatible nunca carga por ser el más reciente. |
| NEW-PLY-06 M teclado / V1 mandos P1 L input | WIN-SET-01/MULTI-06; session/input.rs, InputProfile, bind/resolveConflict. Contextos y reserved keys; perfiles por dispositivo/sistema/juego. | Mapa seguro por defecto; remap de tecla sistema rechaza/explica y desconexión libera todas entradas. |
| NEW-PLY-07 V1 P2 L Rust | Estados verificados/PLAY; session/rewind.rs nuevo, RewindBudget, capture/step/clear. Cap bytes, frecuencia por costo, epoch nuevo, audio flush, bloqueo Link. | Deshabilitado por capacidad; state grande reduce segundos reales sin exceder RAM, GC/eviction correcto. |
| NEW-PLY-08 M P1 M Rust | T09/PLAY; timing.rs, SpeedPolicy, setSpeed. 1x/1.5x/2x/3x según presupuesto; mute/time-stretch solo si implementado, held/toggle y retorno inmediato. | 1x; medir frames emulados vs tiempo, no solo label; UI lenta no acumula snapshots. |
| NEW-PLY-09 V1 P2 M FFI | WIN-CORE-01/SET; cores/options.rs, CoreOptionProfile, validate/apply/reset. Opciones declaradas por core, defaults persistidos por hash/version, riesgo experimental visible. | Defaults probados; opción retirada en upgrade no se envía como válida y rollback restaura perfil. |
| NEW-PLY-10 M P0 M Rust | T08/Vault; supervisor.rs, SessionLock, start/close/recover. Lock por juego/save entre procesos, foco opt-in, ACK de cierre y recuperación. | Una sesión por perfil; doble clic/proceso simultáneo no abre dos escritores del mismo save. |
| NEW-PLY-11 V1 P1 L GPU+input | MULTI-08/AND-03; workspace/ds.rs, DsLayoutProfile, transformPointer. Editor muestra regiones conocidas, swap/rotate/zoom y monitor por pantalla opcional. | Layout verificado del core; 9 puntos por pantalla bajo DPI/rotación correctos; no enviar stylus a superior. |
| NEW-PLY-12 M P1 M UI+FFI | WIN-CORE-01/QA-02; CoreManager.jsx, CapabilityComparison, compare. Detectado vs probado por ROM/build; recomendación basada evidencia, no nombre. | Selección manual; core con serialize export pero tamaño cero aparece sin estados. |

### 20.3 Companion 2.0

| ID / alcance / prioridad / tamaño / dueño | Dependencia; archivo/tipo/operación propuesta; flujo y validación | Fallback / prueba futura |
|---|---|---|
| NEW-CMP-01 M P1 M UI | MULTI-04/CMP seis; Companion.jsx, SessionOverview, selectModule. Identidad/duración real/fuente/antigüedad y accesos. | Modo offline; sesión cerrada conserva referencias, no etiqueta Conectado. |
| NEW-CMP-02 V2 P2 L Desktop | MULTI-01/06; workspace/windows.rs, WidgetProfile, pin/setOpacity/escapeClickThrough. Opt-in, hotkey segura y control de recuperación siempre accesible. | Ventana normal; al activar click-through se puede desactivar sin mouse sobre widget; no robar foco. |
| NEW-CMP-03 V1 P2 L Desktop | MULTI-01/02; layout.rs, WorkspacePreset, save/apply/clamp. Dual/Split/Dock/Compact/Second Monitor por juego y topología. | Dual; desconectar monitor devuelve bounds visibles y no pierde tab/zoom. |
| NEW-CMP-04 V1 P2 M datos | MULTI-03/04; companion/timeline.rs nuevo, TimelineEvent, appendObserved/query. Dedup session/epoch/revision/event; registrar solo observado, retención. | Eventos sesión/guardado sin inferencias; rewind crea rama/epoch, no duplica logros ni inventa evolución no observada. |
| NEW-CMP-05 V1 P2 M datos+UI | CMP-TEAM-01/Vault; companion/compare.rs nuevo, PartyDiff, compareSnapshots. Identidad y fuente compatibles, diff por miembro identificado y campos conocidos. | Comparación manual separada; dos slots reordenados no se interpretan como criaturas nuevas sin evidencia. |
| NEW-CMP-06 V1 P2 L datos+UI | CMP-DEX/BATTLE; companion/planner.rs y Planner.jsx nuevos, TeamPlan, validateCoverage. Seis slots teóricos, reglas dataset, edición y persistencia local. | Tipos manuales; movimiento inexistente para versión se rechaza/etiqueta, nunca modifica save ni promete legalidad competitiva universal. |
| NEW-CMP-07 M tipos / V2 daño P1 L datos | CMP-BATTLE-01; battle.rs, DamageRuleset/DamageInput, calculateRange. Implementar Gen1–5 por módulos con redondeos/orden y supuestos explícitos; hack unknown sin cálculo exacto. | Efectividad de tipos; fixture de daño por reglas independientes, stats faltantes pide entrada y no inventa IV/EV. |
| NEW-CMP-08 V1 P1 M datos | CMP-DEX/BATTLE; companion/moves.rs nuevo, MoveAnalysis, inspectMove. Potencia/precisión/PP/categoría/aprendizaje por versión, excepciones y cobertura. | Ficha parcial con desconocidos; categoría física/especial cambia entre Gen3/4 correctamente. |
| NEW-CMP-09 V1 P2 M datos+UI | CMP-BATTLE/NEW-CMP-08; battle.rs, ReadOnlyAdvice, explainMatchup. Ordenar riesgos conocidos y mostrar supuestos sin actuar sobre emulador. | Tabla manual; sin rival fiable no anuncia recomendación live ni probabilidad fabricada. |
| NEW-CMP-10 V1 P2 M datos | CMP-GUIDE/MULTI-05; guide.rs, ObjectiveEvidence, observe/toggleManual/undo. Mantener manual vs flag automático y origen. | Checklist manual; flag unknown no desmarca trabajo del usuario; undo persiste. |
| NEW-CMP-11 M P1 M UI+datos | CMP-GUIDE; guide.rs/Guide.jsx, SpoilerPolicy, filterContent. Sin spoilers/pistas/solución aplicados a lista, búsqueda, mapas y avisos. | Sin spoilers por defecto; buscar jefe futuro no filtra nombre mediante snippet. |
| NEW-CMP-12 V2 P2 L mapas | CMP-MAP/NEW-DEV-01; atlas.rs, MapLayer/RouteGraph, findRoute/annotate. Grafo legal, restricciones conocidas, marcadores privados, capas y minimapa. | Vista textual/puntos sin ruta; desconexión o edge desconocida dice Sin ruta verificada. |
| NEW-CMP-13 V1 P2 L datos | CMP-DEX/pack versionado; dex.rs, DexFilter/EncounterKey, queryExtended. Evolución/encuentros exactos por versión y progreso conocido. | Dex referencia base identificada; filtrar capturado no trata unknown como false. |
| NEW-CMP-14 V1 P2 M búsqueda | NEW-DEV-01/notes; companion/search.rs nuevo, SearchDocument, searchAcross. Índice local por dataset/tipo/origen, cancelación, ranking y spoiler antes de render. | Búsqueda por módulo; resultados de otro hack no aparecen mezclados; consulta vieja cancelada no reemplaza nueva. |
| NEW-CMP-15 M P1 M storage+UI | Identidad/storage; companion/notes.rs nuevo, CompanionNote, autosave/updateBookmark. Revisión, debounce, backup y privacidad local. | Export texto manual; cierre inesperado recupera última escritura confirmada, dos ventanas muestran conflicto en vez de perder edición. |
| NEW-CMP-16 V1 P2 M UI | MULTI-03/timeline; companion/notifications.rs nuevo, NoticePolicy, enqueue/dismiss/mute. Eventos reales, cooldown/dedup y no spoilers. | Solo errores críticos/save; bucle de mismo evento no satura ni roba foco del juego. |
| NEW-CMP-17 M P0 M datos+UI | QA-02/MULTI-04; Capabilities.jsx nuevo, SourceStatus, explainUnavailable. En vivo/Desde guardado/Referencia/Manual/Sin soporte y fecha/evidencia. | Razón textual; cache recién cargada nunca se marca live hasta lectura válida. |
| NEW-CMP-18 V1 P1 M storage | Notes/guide/profiles, WIN-SYNC; transfer.rs, CompanionBundle, preview/import/export. Manifiesto versionado, identidad, allowlist y merge por revisión. | JSON local validado; payload con archivo .sav o ruta absoluta se rechaza en bundle Companion, save permanece byte-idéntico. |

### 20.4 Vyntra Vault y sincronización

| ID / alcance / prioridad / tamaño / dueño | Dependencia; archivo/tipo/operación propuesta; flujo y validación | Fallback / prueba futura |
|---|---|---|
| NEW-SAVE-01 M P0 L storage | WIN-VAULT-01; manifest.rs, SaveHistory, commit/listVersions. Hash, tipo, ROM/core/format, parent y origen, índice consistente. | Import legacy como compatibilidad desconocida; dos saves mismo nombre no se fusionan. |
| NEW-SAVE-02 M P0 L storage+UI | NEW-SAVE-01; service.rs/Vault.jsx, RestorePreview, preflight/restore/rollback. Mostrar destino, backup y evidencia antes de tocar core. | Export copia; error unserialize deja sesión pausada/recuperada o cerrada segura, no sigue con core inconsistente. |
| NEW-SAVE-03 V1 P1 M storage | NEW-SAVE-01; vault/backup.rs nuevo, RetentionPolicy, snapshot/prunePreview. Quotas y versiones fijadas; borrar solo backup propio aprobado, conservar último válido. Cifrado opcional requiere diseño de claves separado. | Backup manual; disco lleno y retención no eliminan única copia; clave perdida no permite falsa promesa de recuperación. |
| NEW-SAVE-04 M P1 M storage | WIN-SYNC/manifest; transfer.rs, PortableBundle, exportAllowlist. Manifiesto+payload seleccionado, hashes y licencia datos; ninguna ROM/BIOS implícita. | Export archivo individual; bundle con traversal/mismatch se rechaza antes de escribir. |
| NEW-SAVE-05 V2 P2 XL red | NEW-SAVE-04/08, AND-07; vault/lan.rs nuevo, PairedDevice/TransferSession. Pairing explícito, canal cifrado, chunks hashes, resume autenticado, límites y revocación. | Carpeta/USB; peer incorrecto o chunk alterado no aplica nada; prueba desconexión y reanudar conserva integridad. |
| NEW-SAVE-06 X P3 XL servicios | NEW-SAVE-05/conflictos, proveedor/coste/privacy; vault/cloud.rs solo propuesta | Posponer: primero medir necesidad y modelo claves/consentimiento. Fallback export/LAN; spike debe demostrar offline-first, borrado de cuenta y conflicto sin pérdida antes de aprobar. |
| NEW-SAVE-07 V1 P1 L datos+Android | NEW-SAVE-01/04, AND-05; transfer.rs/TransferService.kt, CompatibilityDecision. Comparar formato SRAM/state, core build y ROM; conversiones solo explícitas y probadas. | SRAM compatible o no transferir; state mGBA nativo vs libretro no se acepta solo por nombre de motor. |
| NEW-SAVE-08 M P0 M storage+UI | NEW-SAVE-01/04; vault/conflicts.rs y Conflicts.jsx nuevos, SyncConflict, choose/keepBoth. Ancestro/hash, no last-write-wins por reloj. | Conservar ambas; relojes invertidos/igual timestamp con bytes distintos no sobrescriben silenciosamente. |

### 20.5 Android ampliado

| ID / alcance / prioridad / tamaño / dueño | Dependencia; archivo/tipo/operación propuesta; flujo y validación | Fallback / prueba futura |
|---|---|---|
| NEW-AND-01 V1 P1 M Kotlin | AND-04/03; A/companion/ui/vyntra/Overlay.kt nuevo, OverlayState, expand/collapse/move. Panel actividad con insets, lifecycle y snapshot compartido. | Drawer fijo; rotación con panel abierto no reinicia core ni superpone controles esenciales. |
| NEW-AND-02 V1 perfiles / X macros P2 L input | AND-06/SET; A/vyntra/InputProfile.kt nuevo, edit/export/import/reset. Editor touch/mando y conflictos; macros excluidas inicialmente de modo competitivo/link. | Perfil básico; import con botones inválidos o solapamiento irresoluble se rechaza; macro experimental nunca se activa sola. |
| NEW-AND-03 V2 P2 M Kotlin+sync | NEW-SAVE-04/05, AND-01; TransferService.kt, MetadataLibraryBundle, previewMerge. Favoritos/notas por hash sin ROM, resolver SAF local. | Export metadata offline; URI remota no se convierte en permiso local ni se copia ROM implícitamente. |
| NEW-AND-04 V1 P1 M Kotlin | AND-02/08, métricas; A/vyntra/PowerPolicy.kt nuevo, observe/apply. Thermal/battery APIs disponibles, bajar polling/animación antes de juego, mostrar perfil. | Manual Bajo consumo; API ausente no muestra temperatura inventada; benchmark mide batería/frametime. |
| NEW-AND-05 M P0 L Kotlin | AND-01/02/05; PokeDaisyActivity/EmulatorEngine, SessionLifecycle propuesto. Pausa/audio focus/suspend entre frames; detectar process death; limpiar touch/keys. | Arranque desde último save válido; llamada/lock/rotación/multiventana no duplican core ni corrompen save. |
| NEW-AND-06 V1 P2 M Kotlin | NEW-PLY-03/NEW-DEV-05; A/vyntra/ShareExport.kt nuevo, SharePreview, createGrant/revoke. FileProvider/temporario permitido, Sharesheet, saneamiento/consentimiento. | Guardar local; recipient no accede otras rutas y grant temporal no filtra credenciales. |
| NEW-AND-07 V2 P2 M Kotlin+Rust | NEW-LIB-08/NEW-SAVE-04; SettingsSchema.kt, PortableSettings, mergeSupported. Allowlist idioma/notas/motion, no rutas GPU/teclado/core binario. | Ajustes independientes; opción Windows-only no invalida preferencias Android. |
| NEW-AND-08 M P1 M accesibilidad | UI-01/02/AND-08; Theme/Navigation y componentes existentes. Semantics, touch targets, contraste, foco, texto y reduced motion. | Layout simplificado; TalkBack y 200% texto permiten todos los flujos sin tap oculto. |

### 20.6 Datos, herramientas y modularidad

| ID / alcance / prioridad / tamaño / dueño | Dependencia; archivo/tipo/operación propuesta; flujo y validación | Fallback / prueba futura |
|---|---|---|
| NEW-DEV-01 M P1 L datos | E14/identidad; schemas/targets/rust, ReferencePackManifest, validate/loadPack. Fuentes TOML→salidas versionadas, licencias, checksums y compatibilidad; JSON portable si se justifica. | Dataset embebido conocido; pack incompatible/corrupto no sustituye el anterior. |
| NEW-DEV-02 V1 P1 L datos+FFI | MULTI-05/NEW-DEV-01; registry.rs, GameDataAdapter, match/requirements/decode/validate. Rangos/acceso read-only, capabilities por módulo, errores tipados. | Manual/referencia; adapter no puede solicitar direcciones fuera de allowlist ni modificar memoria. |
| NEW-DEV-03 D actual / X futuro P3 XL arquitectura | NEW-DEV-02, threat model y necesidad medida; posible D/plugins solo si ADR aprobada. PluginManifest/PermissionSet, sandbox/revoke sería contrato futuro, no loader nativo abierto. | Packs declarativos revisados; spike debe demostrar aislamiento real CPU/RAM/FS/red y actualización firmada. Descartar ejecutar scripts/DLL arbitrarias como SDK MVP. |
| NEW-DEV-04 M P0 L QA | QA-01/contratos; tests/fixtures/vyntra, FixtureManifest, replayCapture. Memoria/save sintéticos y homebrew redistribuible con licencia; test host determinista. | Fixtures unitarios si no hay ROM legal; distribución de corpus verifica ausencia ROM comercial y tests fallan ante fixture alterado. |
| NEW-DEV-05 M P1 M Rust+Kotlin | INFRA-01/QA-02/NEW-PLY-02; diagnostics.rs y Diagnostics.jsx, DiagnosticsSnapshot, preview/export. Core/ABI/hash truncado/source/stale/log saneado. | Copiar mensaje de error; export con paths/tokens ficticios los elimina, no incluye dumps. |
| NEW-DEV-06 V1 P1 L datos | NEW-LIB-01/NEW-DEV-01/02; registry/identity packs, HackCompatibility, matchStrict/selectReference. Firma/hash y layout versionado; perfil manual no habilita live. | Referencia elegida/manual; ROM con header idéntico pero bytes diferentes permanece unsupported salvo regla probada. |
| NEW-DEV-07 V2 P2 XL release | REL-01/migración, AppUpdater existente; updater propio futuro, SignedReleaseManifest, stage/verify/activate/rollback. Separar app/core/pack, proteger downgrade y conservar firma Android. | Actualización manual verificada; paquete con firmante distinto o migración fallida no reemplaza instalación/datos. |
| NEW-DEV-08 M P1 M UX+docs | DOC-01/QA-02; Help.jsx y ayuda Android existente, HelpTopic, explainCapability. Ayuda offline contextual: ausente vs incompatible vs no probado; enlaces seguros. | Manual local; botón de core ausente abre instrucciones del motor real, no descarga ficticia. |

### 20.7 Exploración: evaluar, no prometer

| ID / decisión / prioridad / tamaño / dueño | Dependencia / spike y contrato futuro | Fallback / evidencia mínima |
|---|---|---|
| EXP-01 V2/X P3 L datos | MULTI-05/timeline; companion/achievements.rs propuesto, LocalAchievementRule, evaluateObserved. Reusar separación de RetroAchievements Android existente; reglas locales con origen. | Checklist manual; observación desconocida nunca desbloquea, rewind no duplica logro; licencia del catálogo. |
| EXP-02 X P3 XL emulación | States/core determinism/T09; replay.rs propuesto, ReplayManifest, recordInputs/verifyReplay. Fijar coreHash/ROM/settings/RNG/RTC, comprobar hashes periódicos. | Grabación de vídeo separada; repetir 3 runs sintéticas sin divergencia antes de afirmar determinismo; no reescribir SRAM al reproducir. |
| EXP-03 X por core P2 XL FFI+red | WIN-LINK-01/AND-07/CoopGuards; LinkCapability/Handshake. Auditar cable/local wireless/hooks y latencia; no usar libretro netplay como equivalencia automática a intercambio. | No compatible; dos instancias y saves intactos tras desconexión, reconexión y versiones distintas. |
| EXP-04 X P3 L web+seguridad | MULTI-07/NEW-SAVE-05; LanCompanionSession propuesto. Servidor opt-in autenticado, pairing, permisos read-only, origins, rate limits; nada expuesto por defecto. | Companion nativo local; cliente sin pair no lee datos y cerrar sesión cierra puerto. |
| EXP-05 V2/X P3 L integración | NEW-LIB-06/proveedor legal; MetadataConnector propuesto. Licencia, atribución, credenciales, cuotas y cache; aprovechar integraciones Android existentes si aptas. | Manual/offline; prueba sin credenciales/red y sin transmitir ROM/save. |
| EXP-06 posponer X P3 XL producto | NEW-DEV-01/privacidad/proveedor coste; GroundedExplanation propuesto solo tras necesidad demostrada. Explicación de datos citados, opt-in, límites y sin instrucciones al core. | Reglas deterministas/ayuda local; no enviar memoria/ROM/save por defecto; no prometer precisión ni acceso live. |
| EXP-07 V2/X P3 XL Desktop | ADR-01/REL-01, port Linux existente E15; PlatformHost propuesto. Auditar Wry/SDL/pipe alternativo y packaging por distro; reutilizar linux actual. | Mantener port Linux existente; no afirmar que hoy no hay Linux ni sustituirlo con launcher incompleto. |
| EXP-08 posponer X P3 XL emulación | QA-02/CoreAdapter/identity; PlatformDefinition propuesto. Elegir nueva consola solo con demanda, core/licencia, fixtures y controles definidos. | GB/GBC/GBA/DS; ninguna consola se anuncia soportada por aceptar su extensión. |

### 20.8 Validación de cobertura de nuevas ideas

| Familia | IDs clasificados | Dependencia / fallback / prueba | Límite |
|---|---|---|---|
| NEW-LIB | 01,02,03,04,05,06,07,08,09 | Cada fila de 20.1 incluye los tres | Parches/proveedor/7z sujetos a parser y licencia |
| NEW-PLY | 01,02,03,04,05,06,07,08,09,10,11,12 | Cada fila de 20.2 incluye los tres | Codec, rewind y DS dependen capacidad/benchmark |
| NEW-CMP | 01,02,03,04,05,06,07,08,09,10,11,12,13,14,15,16,17,18 | Cada fila de 20.3 incluye los tres | Live no inferido de modo manual; mapas/licencias pendientes |
| NEW-SAVE | 01,02,03,04,05,06,07,08 | Cada fila de 20.4 incluye los tres | Nube pospuesta y estados multiplataforma condicionados |
| NEW-AND | 01,02,03,04,05,06,07,08 | Cada fila de 20.5 incluye los tres | Hardware/SAF/termal reales pendientes |
| NEW-DEV | 01,02,03,04,05,06,07,08 | Cada fila de 20.6 incluye los tres | SDK abierto descartado en alcance actual |
| EXP | 01,02,03,04,05,06,07,08 | Cada fila de 20.7 incluye los tres | Exploración, no compromiso de soporte |

## 21. Plan de integración de nuevas capacidades con pantallas existentes y prototipos de flujo

**[PROPUESTO]** Conservar navegación principal corta. Nuevas capacidades aparecen donde se usan, no como decenas de entradas de sidebar.

- **Library → Propiedades → Identidad/Versiones/Perfil/Integridad.** Importar parche abre preview base/output y crea variante; no lanza inmediatamente ni copia saves sin decisión. Tags/colecciones van en filtros, no en otra biblioteca aislada.
- **Core Manager → candidato → diagnóstico → probar → perfil.** Mostrar motor real, procedencia, arquitectura y capacidades verificadas; diferencias entre versiones y rollback. El menú de instalación solo aparece cuando existe pipeline/origen acreditado.
- **Player → overlay → sesión/velocidad/guardado/captura/Companion.** Diagnóstico opcional y grabación futura en herramientas; la pantalla de juego conserva controles mínimos. La galería recibe framebuffer del Player, no captura de escritorio con datos privados.
- **Companion Inicio → fuente → módulo.** En manual, Team conduce a planificador y Bag a catálogo; en live muestra snapshot con hora. Timeline/avisos/capacidades en panel secundario. Fuente no cambia sin señal visible.
- **Dex → especie → encuentros → Atlas → área → Guía → nota.** RouteContext mantiene gameId/datasetVersion/species/area/query/scroll/zoom; Atrás restaura contexto en la misma ventana o después de moverla.
- **Battle → reglas → aliado/rival → movimiento → afinidad/explicación.** Campos desconocidos permanecen vacíos; calculadora de daño futura solicita datos faltantes. No botones de actuación automática.
- **Vault → versión → previsualizar → backup → restaurar.** Exportación e importación convergen en preview/compatibilidad; Sync muestra diferencias y Keep both, no una nube falsa.
- **Settings → buscar ajuste → ámbito y origen → modificar → confirmación efectiva.** Opciones avanzadas por motor colapsadas; las incompatibles explican razón. Rutas, audio, vídeo, input, DS, rendimiento, idioma, privacidad, datos, backups y accesibilidad se pueden configurar aquí.
- **Pocket → Home/Game + drawer Companion + bottom navigation.** Estados compartidos y lifecycle actual, overlay dentro actividad, back colapsa panel antes de salir. Share incluye preview y grant limitado; sync de preferencias excluye paths y opciones GPU no portables.

## 22. Flujos E2E obligatorios: 12 secuencias con fuentes y rutas de error

Todos los diagramas son **[PROPUESTO]**. Las pruebas descritas serán ejecutadas por el implementador; no representan resultados de esta auditoría.

### E2E-01 Primer inicio y migración

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant UI as Primer inicio
  participant M as MigrationService
  participant Old as Datos heredados
  participant New as Staging y catálogo
  U->>UI: Elegir instalación anterior
  UI->>M: preview(source,target)
  M->>Old: Leer inventario y hashes
  M-->>UI: Copias, espacio y conflictos
  U->>UI: Confirmar copia
  UI->>M: apply(planId)
  M->>New: Journal y copias verificadas
  alt Todo válido
    M->>New: Publicar catálogo y completed
    M-->>UI: Migración completada, origen conservado
  else Interrupción o falta espacio
    M-->>UI: Recuperación pendiente, originales intactos
    UI->>M: recover(journal) al reiniciar
  end
~~~

Fuente E04/E13; contratos MigrationPlan/SaveManifest. Prueba: cortar después de cada fase; hashes originales iguales, sin pérdida y segunda ejecución idempotente.

### E2E-02 Carpeta con ROM, ZIP, falso y hack/parche

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant UI as Importador
  participant I as ImportJob
  participant P as Inspector y archivo seguro
  participant DB as Catálogo
  U->>UI: Elegir carpeta y recursión
  UI->>I: importSources + cancellation
  I->>P: Enumerar, límites, header y hash
  P-->>I: Válida, duplicada, falsa o ambigua
  alt ZIP múltiples ROM o parche separado
    I-->>UI: Solicitar elección/preview de variante
    U->>UI: Seleccionar fuente y base
    UI->>I: Resolver elección
  end
  I->>DB: Confirmar entradas válidas por lote
  I-->>UI: Reporte por archivo
  alt Cancelar o archivo hostil
    UI->>I: cancel(JobId)
    I-->>UI: Parcial/cancelado, original preservado
  end
~~~

Fuente E02/E08; GameIdentity/ImportReport. Prueba: conteo exacto de válidos, rechazo falso/traversal, hack desconocido sin adapter live, persistencia tras reiniciar.

### E2E-03 Core, ROM, SRAM, A/V, cierre

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant L as Launcher
  participant P as Probe aislado
  participant W as Worker owner
  participant V as Vault
  U->>L: Jugar GameId
  L->>P: Validar candidato y ABI
  alt Probe correcto
    L->>W: Start SessionConfig
    W->>V: Leer SRAM compatible
    W->>W: Load game, AV info, input, frames
    W-->>L: SessionStarted y capacidades
    U->>W: Cerrar
    W->>V: Snapshot y commit verificado
    W-->>L: SessionStopped
  else DLL/dependencia/ABI falla
    P-->>L: Error saneado y solución
  end
~~~

Fuente E02/E06; CoreProbeReport/SessionIdentity. Prueba homebrew con patrón A/V/input, error de dependencia y ENOSPC al cerrar; launcher sigue utilizable.

### E2E-04 Companion segundo monitor, Dock y Dual

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant WM as WindowManager
  participant Store as ApplicationStore
  participant W as Player worker
  U->>WM: Abrir Companion en monitor 2
  WM->>Store: subscribe(viewId,session)
  Store-->>WM: Contexto y snapshot vigente
  U->>WM: Cambiar a Dock
  WM->>WM: Preparar superficie y transferir estado de vista
  alt Host nativo disponible
    WM->>W: Asociar superficie, misma SessionId
    WM-->>U: Dock activo sin reinicio
    U->>WM: Volver a Dual
    WM->>WM: Restaurar ventanas y bounds válidos
  else Fallo de superficie
    WM-->>U: Dual conservado, motivo del fallo
  end
~~~

Fuente E03; WindowLayoutProfile/WorkspaceTransition. Prueba SessionId constante, secuencia frame continua y monitor desconectado recuperable; bloquear liberación de Dock si spike falla.

### E2E-05 Equipo manual → live → stale

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant C as Companion
  participant R as AdapterRegistry
  participant W as Owner y decoder
  U->>C: Consultar equipo manual
  C-->>U: Plan manual etiquetado
  U->>C: Usar datos de partida
  C->>R: match(identity,coreCapabilities)
  alt Adapter estricto disponible
    R->>W: captureRequirements
    W-->>C: Snapshot validado, origen live
    W-->>C: Core perdido / timeout
    C-->>U: Desactualizado y última lectura
  else No compatible
    R-->>C: UnsupportedReason
    C-->>U: Manual conservado, sin inventar equipo
  end
~~~

Fuente E11/E12; PartySnapshot/Provenance. Prueba identidad parecida desconocida, snapshot anterior a epoch y pérdida core; manual no sobrescrito.

### E2E-06 Dex → Atlas → Guía

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant Dex as Dex
  participant Ref as ReferenceProvider
  participant Map as Atlas
  participant Guide as Guía
  U->>Dex: Seleccionar especie y versión
  Dex->>Ref: getEncounters(SpeciesKey)
  Ref-->>Dex: Áreas con origen/dataset
  U->>Dex: Abrir ubicación
  Dex->>Map: Navigate(RouteContext)
  U->>Map: Zoom y abrir guía del área
  Map->>Guide: Navigate(context,spoilerPolicy)
  alt Guía compatible
    Guide-->>U: Capítulo y notas
  else Sin cobertura
    Guide-->>U: Cobertura ausente y notas personales
  end
  U->>Map: Atrás
  Map-->>U: Área y zoom anteriores
~~~

Fuente E14; SpeciesKey/RouteContext. Prueba cambio de dataset, mapa ausente, spoiler en snippet y recuperación del viewport.

### E2E-07 Pausa, quicksave, FF, rewind y load

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant UI as Player UI
  participant W as Owner
  participant V as Vault
  participant C as Companion
  U->>UI: Pausa y quicksave
  UI->>W: Commands con session y epoch
  W->>V: Serialize entre frames, commit
  V-->>UI: SaveCommitted
  U->>UI: Reanudar y FF
  UI->>W: SetSpeed
  W-->>C: Último snapshot coalescido
  U->>UI: Rewind o Load
  alt Capacidad y compatibilidad válidas
    W->>W: Restaurar, flush audio, epoch++
    W-->>C: EpochChanged y snapshot nuevo
    C->>C: Descartar decode atrasado
  else No soportado o state inválido
    W-->>UI: Rechazo sin sustituir estado válido
  end
~~~

Fuente E02/E09/E13; SessionCommand/SaveManifest. Prueba memoria rewind limitada, FF real, Link bloquea operaciones incompatibles y timeline no mezcla epochs.

### E2E-08 Backup, Android y conflicto

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant PC as Vault PC
  participant T as TransferService
  participant A as Vault Android
  U->>PC: Backup y exportar selección
  PC->>T: Bundle manifest y hashes
  T->>A: Preview import opt-in
  A->>A: Comparar identidad, formato y ancestro
  alt Local y remoto divergieron
    A-->>U: Conflicto con dos versiones
    U->>A: Conservar ambas o elegir con backup
    A->>A: Publicar copia nueva y resolución
  else Integridad/compatibilidad falla
    A-->>U: Rechazo, originales intactos
  else Compatible sin conflicto
    A->>A: Importar copia verificada
  end
~~~

Fuente E03/E13; PortableBundle/SyncConflict. Transporte inicial carpeta/SAF; LAN opcional posterior. Prueba relojes distintos, hash corrupto y state no portable.

### E2E-09 DS layouts y stylus

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant UI as DS Layout
  participant X as Transformador input
  participant W as Core owner
  W-->>UI: DsSurfaceDescriptor verificado
  U->>UI: Swap/rotar/zoom
  UI->>X: Matriz inversa y región táctil
  U->>UI: Tocar pantalla inferior visual
  UI->>X: Coordenada ventana y DPI
  alt Dentro de región táctil
    X->>W: Stylus coordenada DS normalizada
  else Fuera o layout desconocido
    X-->>UI: No enviar contacto / deshabilitar con motivo
  end
  U->>UI: Rotación durante contacto
  UI->>W: Release stylus antes de nueva matriz
~~~

Fuente E06/E10; DsLayoutProfile/InputFrame. Prueba esquinas/centro/bordes bajo 100/150/200% y dos orientaciones; sin asumir mitad de framebuffer.

### E2E-10 Actualizar core y rollback

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant CM as Core Manager
  participant P as Probe
  participant S as Core store
  U->>CM: Seleccionar nueva versión
  CM->>S: Guardar candidato separado y verificar origen/hash
  CM->>P: probe con perfil de prueba
  alt ABI/dependencia incompatible
    P-->>CM: Incompatible
    CM-->>U: Versión anterior sigue seleccionada
  else Validado
    CM->>S: Activar para próxima sesión
    CM-->>U: Compatibilidad de estados a revisar
    U->>CM: Volver a versión previa
    CM->>S: Restaurar selección/perfil previo
  end
~~~

Fuente E06/E15; CoreDescriptor/ProbeReport. No reemplazar DLL cargada. Prueba hash cambiado invalida verificación y state de versión nueva no contamina carpeta anterior.

### E2E-11 Gamepad y conflicto de atajo

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant UI as Editor controles
  participant I as InputService
  participant P as ProfileRepository
  U->>UI: Capturar botón por juego
  UI->>I: BeginCapture, suspender comandos de juego
  I-->>UI: Botón detectado
  UI->>I: validateBinding(scope,target)
  alt Conflicto o tecla reservada
    I-->>UI: Explicación y opciones permitidas
    U->>UI: Cancelar o resolver asignación
  else Válido
    UI->>P: save(expectedRevision)
    P-->>I: Perfil efectivo actualizado
    I-->>UI: Confirmación y preview
  end
  I->>I: Al desconectar liberar entradas
~~~

Fuente E04/E09; InputProfile/FocusLease. Prueba texto Companion no activa juego, perfil persiste y reset no borra perfiles ajenos.

### E2E-12 Exportar capturas/notas sin ROM/BIOS

~~~mermaid
sequenceDiagram
  actor U as Usuario
  participant UI as Galería y notas
  participant E as ExportService
  participant V as Validador bundle
  U->>UI: Seleccionar capturas y notas
  UI->>E: previewExport(allowlist)
  E->>V: Revisar tipos, paths y tamaño
  alt Solo contenido permitido
    V-->>UI: Preview exacto del paquete
    U->>UI: Confirmar destino
    UI->>E: Escribir copia y verificar hashes
    E-->>UI: Exportación confirmada
  else ROM/BIOS/ruta ajena encontrada
    V-->>UI: Rechazo o exclusión explicada
  end
~~~

Fuente E03/E18; CompanionBundle/ScreenshotManifest. Prueba selección manipulada con path traversal/archivo de core/ROM; inspeccionar bundle y asegurar que solo contiene lo consentido.

## 23. Roadmap de capacidad y presupuesto de rendimiento, condicionado a benchmarks futuros

Todos los valores siguientes son **candidatos**, no resultados actuales ni garantías. Hardware de referencia aún por fijar: Windows x64 con CPU/GPU/RAM/refresh anotados, dos monitores DPI mixto; Android arm64 de gama media y dispositivo limitado, emulador x86_64 para pruebas funcionales donde se builda esa ABI. Añadir DS renderer/core concreto y corpus legal por sistema.

| Atributo | Objetivo candidato / instrumentación | Prueba futura / regresión / degradar primero |
|---|---|---|
| Frame pacing | Usar fps real core; registrar core frame, present y deadline, percentiles | Sesión 10–30 min por sistema; umbrales por fijar tras baseline. Degradar efectos UI/shaders y Companion antes de reloj del juego. |
| Audio | Contadores de underrun/overrun, profundidad cola y deriva reloj; no crecimiento ilimitado | Patrón sintético + juego legal; regresión frente baseline por hardware; reducir recording/FF audio avanzado. |
| Input | Timestamp captura→owner→frame; latencia visual real requiere cámara/medición externa | Candidato ≤1 frame de cola frontend en carga nominal; no confundir con input-to-photon. Desactivar trabajo accesorio si se excede. |
| Captura Companion | Candidato coste de captura p95 <5% del presupuesto de frame; decode fuera del owner | Instrumentar tamaño/copia/decodificación por adapter, más caro DS. Reducir cadencia/regiones y pausar módulos ocultos. |
| Cadencia/stale | Inicial 1 Hz para party/bag/map donde no hay eventos; batalla configurable según coste; stale tras varias muestras esperadas, distinguir pausa | Core parado/decoder lento; stale aparece con fecha. Umbral exacto por adapter/benchmark, no universal. |
| Backpressure | Un latest snapshot pendiente por consumidor y cola control acotada con prioridad Stop | Consumidor suspendido 60 s: RAM estable, sin reproducir backlog viejo al volver. |
| Rewind | Cap configurable; candidato Desktop 128–256 MiB y Android menor según memory class, no defaults definitivos | Memoria = estados retenidos × tamaño + overhead medido; segundos ofrecidos calculados. Reducir segundos/cadencia antes de exceder cap. |
| Library | Corpus candidato 10k entradas; búsqueda local p95 <100 ms, UI no bloqueada por hashing | Dataset sintético en SSD/HDD; medir startup/primer render/consulta; paginar y carga perezosa de cubiertas. |
| Import | Streaming con cancelación comprobada en límites de bloque; cuotas archivo/archivo comprimido/total | Bomba y disco lento, memoria acotada; procesar pocos workers, no saturar IO del Player. |
| Vault | Después de ACK de commit, última versión verificable debe sobrevivir fallo según garantías FS | Fault injection por paso y reinicio; no publicar manifest antes payload; conservar original si FS no permite atomic replace. |
| Ventanas | 100/125/150/200% DPI, monitor removido, resize y fullscreen | Prueba continua sin reinicio core ni región táctil errónea; fallback Dual si composición falla. |
| Android thermal | Registrar API thermal/battery disponibles y frame cost, sin telemetría remota | Sesión prolongada hardware real; bajar polling/animación/recording antes de jugar más lento sin aviso. |
| A11y/motion | Contraste y foco verificados, 200% texto, navegación completa y 48 dp Android | Lectores de pantalla/manual y checks automáticos; layout simple si panel complejo pierde accesibilidad. |

Política de degradación: recorder/shaders opcionales → animaciones → thumbnails/metadata IO → refresco Companion oculto/no crítico → frecuencia snapshots con aviso stale. Nunca corromper saves, omitir validaciones de entrada ni alterar velocidad nativa silenciosamente para lograr una métrica.

## 24. Matriz de evaluación de ideas: construir, posponer o descartar, con motivos

**[PROPUESTO]** Valor no significa prioridad automática; las ideas XL necesitan evidencia de demanda y spike antes de comprometer producto.

| Decisión | Capacidades | Valor / coste / alternativa sencilla / gate |
|---|---|---|
| Construir primero | Identidad, Vault transaccional, core probe, sesión inmutable, importer seguro, settings reales | Evitan pérdida y desbloquean todo; coste M–L. Gate fixtures e integridad, no depende de cloud. |
| Construir MVP | Biblioteca, ayuda/capacidades, Dex y Battle manual, notas/guías, Companion Dual | Valor offline inmediato; usar texto y datos permitidos cuando no hay assets/adapters. |
| Construir V1 condicionado | Live por revisión, perfiles, controles, DS, comparación, backups y transferencia portable | Alto valor; no generalizar fuera matriz; fallback manual/SRAM/Dual. |
| Spike antes de construir | Split/Dock nativo, armGBA Windows, DS core propio, LAN y Link por motor | XL o dependencia externa; evidencia mínima: loop real, latencia/foco/rollback y dos instancias cuando aplica. |
| Posponer V2 | Mapas/rutas avanzados, recording, catálogo remoto, updater multiplataforma y workspace studio | Complejidad/licencias; alternativa mapa textual/captura/manual update. Desactivar si consume recursos del Player. |
| Posponer experimental | Nube, AI, replay determinista, LAN web Companion, consolas adicionales | Necesidad/privacidad/coste/determinismo no probados; producto base no depende de ellos. |
| Descartar alcance actual | SDK que ejecute scripts/DLL arbitrarios; reescritura total UI/motores; nube obligatoria | Riesgo/mantenimiento no compensados; packs declarativos y motores existentes cubren necesidad. |
| Descartar siempre como comportamiento | Datos ficticios presentados live, Ready por filename, sync silenciosa que sobrescribe, renombrar terceros como adaptación | Contradice integridad del producto; no es fallback aceptable. |

## 25. Declaración final de integridad: auditoría, documentación y publicación

La auditoría técnica fue de lectura: no ejecutó la aplicación, pruebas, compilaciones, instalaciones, descargas de herramientas ni núcleos. No modificó ROMs, BIOS, guardados, preferencias privadas o motores. Este documento describe trabajo futuro; ninguna propuesta se declara implementada por estar aquí.

Después de la auditoría, el usuario pidió: «dejalo todo en un md y sube todo, hasta lo pendiente a github». En respuesta se autorizó crear este Markdown y publicar el trabajo pendiente preexistente como checkpoint. La documentación conserva explícitamente los pendientes y no certifica ese código como release. No se crea una implementación nueva para completar casillas del plan.

Alcance de publicación previsto: cambios propios de código/UI/branding/documentación ya pendientes, prompts de requisitos, recursos originales, lockfiles y capturas existentes; este plan. Exclusiones: datos privados/ignorados, node_modules/builds locales, ROMs/BIOS/saves, credenciales y las seis DLL locales sin origen/distribución acreditados. Los hashes y los pasos pendientes de validación de esos cores quedan en sección 1.4; sus archivos permanecen locales intactos.

Referencia visual 1 inspeccionada; segunda referencia no localizada. Git estaba sucio antes de la auditoría. No se emitieron resultados nuevos de build/tests ni cifras de rendimiento. La publicación en GitHub debe comprobarse por el SHA remoto y anunciar rama/commit y exclusiones al usuario. La implementación completa y la aprobación de release siguen pendientes de las tareas y gates definidos en este documento.

