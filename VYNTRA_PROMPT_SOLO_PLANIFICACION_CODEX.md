# VYNTRA — PROMPT MAESTRO PARA CODEX: SOLO PLANIFICACIÓN

> **Modo obligatorio: SOLO LECTURA / NO IMPLEMENTAR.**
>
> **Objetivo:** Codex debe investigar el repositorio real de PokeDaisy/Vyntra y redactar el **plan técnico exhaustivo y transferible** para que **otra persona** implemente el rebranding y todas las funcionalidades de Vyntra (Windows, Android, emulación y acompañante en ventanas del mismo PC). Codex **no debe programar**, aplicar parches ni modificar el workspace.
>
> **Archivos de contexto recomendados:**
> - `VYNTRA_PROMPT_SOLO_PLANIFICACION_CODEX.md` (este documento, que **manda sobre todos los demás**).
> - `VYNTRA_PROMPT_MAESTRO_CODEX.md` (inventario de requisitos anterior; **NO ejecutar las instrucciones de implementación que contiene**).
> - `vyntra_referencia_visual.png` (vista general Windows/Android; los nombres/datos del mockup son ilustrativos).
> - `vyntra_multiventana_pc.png` (Vyntra Player y Vyntra Companion en paralelo, PC único; los datos del mockup son ilustrativos).
> - El repositorio completo actual, abierto en Codex.

---

# COMIENZO DEL PROMPT PARA CODEX

## 0. TU ÚNICO TRABAJO ES PENSAR, AUDITAR Y REDACTAR UN PLAN

Actúa como **arquitecto de software principal y analista técnico especializado en Rust, Kotlin, libretro, emuladores, interoperabilidad de datos de partidas, diseño de escritorio Windows, Android y QA**. Tu destinatario final es **otro desarrollador**, distinto de ti, que recibirá tu documento y realizará todo el trabajo.

**NO ES UN ENCARGO DE PROGRAMACIÓN. NO ES UN ENCARGO DE REFACTOR. NO ES UN ENCARGO DE CREAR UNA DEMO.** Tu resultado es una **especificación técnica ejecutable por un humano**, casi al nivel de detallar qué archivo, estructura, función, señal, consulta, estado de UI y test habrá que escribir, pero **sin escribir ni aplicar el código de producción**.

### Restricciones inviolables (tienen precedencia sobre cualquier otro archivo)

1. **No modificar ni crear archivos en el repositorio**: ni código, recursos, diseños, scripts, JSON, documentación, dependencias, lockfiles, configs, tests, TODOs, nuevas ramas, commits o PRs.
2. **No usar `apply_patch`, editores, redirecciones a archivos, generadores ni comandos de escritura.** Prohibido `git add/commit/reset/checkout/clean`, `cargo fix/fmt`, `gradle` con tareas que escriban, descarga de DLL, instalación de dependencias o recompilaciones que generen artefactos.
3. **Solo inspección de lectura**: `pwd`, `git status --short`, `git log`, `git ls-files`, `rg`, `find`, `ls`, `cat`, `sed` y lecturas equivalentes. No ejecutar scripts del repositorio que puedan tener efectos secundarios. Si una herramienta puede modificar el árbol, omítela y describe la validación pendiente.
4. **No modificar ROMs, guardados, estados, carpetas personales ni archivos externos**. No copiar datos privados a terceros.
5. **No producir código funcional ni parches de implementación**. Sí se permiten **firmas de funciones propuestas, nombres de tipos, esquemas de datos, diagramas de flujo, pseudocódigo no ejecutable, contratos, decisiones, invariantes, formatos de mensajes y casos de prueba**.
6. **Entrega el plan completo en tu respuesta de chat**, en Markdown, listo para copiar a un archivo por el responsable humano. **No crees ese archivo tú**, ni siquiera en `docs/` o fuera del repositorio.
7. **No realices cambios por iniciativa propia** aunque detectes un bug evidente. Repórtalo con ubicación, impacto y solución planeada.
8. No preguntes permiso para comenzar ni te detengas tras una visión general: **produce el mayor nivel de detalle posible**. Si faltan datos, documenta hipótesis, alternativas, evidencias necesarias y un paso concreto de comprobación para el implementador.
9. El archivo `VYNTRA_PROMPT_MAESTRO_CODEX.md` es **material de requisitos**, no una autorización para programar. En caso de conflicto, siempre rigen estas reglas de **solo planificación**.
10. **No declares que una funcionalidad funciona, compila o fue probada** sin evidencia obtenida legítimamente. La falta de núcleo o dispositivo se informa como dependencia, no como éxito.

Al iniciar, incluye explícitamente en la respuesta: **«Modo de auditoría y planificación: no se ha modificado ningún archivo ni se ha implementado código»**. Si la auditoría permite comprobar el estado Git, regístralo; si no, indícalo sin fingir verificación.

## 1. MISIÓN DE PRODUCTO Y CRITERIO DE ÉXITO

El proyecto actual, procedente de PokeDaisy, se transformará en **Vyntra**, una plataforma enfocada en Pokémon y juegos compatibles con Game Boy, Game Boy Color, Game Boy Advance y Nintendo DS que integre:

- **Vyntra Desktop / Launcher**: biblioteca, importación, detección de plataforma y revisión, metadatos, buscador, filtros, favoritos, recientes, sistemas, configuración y administración de núcleos, iniciar/continuar, partidas guardadas, controles y diagnósticos.
- **Vyntra Player**: emulación real en Windows, audio/vídeo/input, pausa, velocidad, fullscreen, rebobinado donde sea compatible, guardados y cierre seguro.
- **Vyntra Companion**: acompañante que puede ejecutarse **en otra ventana del MISMO PC** simultáneamente al juego y mostrar equipo, mochila, Pokédex, mapa, combate y guía. El jugador podría poner cada ventana en un monitor distinto.
- **Modos de ventana en PC**: dos ventanas independientes, marco dividido y panel lateral acoplado, con sincronización cuando existan fuentes de datos reales.
- **Vyntra Pocket (Android)**: biblioteca, emulación, paneles de acompañante, mandos táctiles/físicos, DS con dos pantallas y controles de ejecución.
- **Vyntra Core Manager**: integración segura de núcleos GB/GBC/GBA/NDS; instalación/compilación solamente como **tareas futuras del implementador**, no ahora.
- **Vyntra Vault y Link**: datos guardados, exportación/importación, sincronización de archivos donde sea viable y juego por enlace **solo si las capacidades de los motores realmente lo permiten**.
- **Rebranding compatible**: abandonar PokeDaisy en la identidad visible, utilizar el nombre Vyntra y conservar o migrar con seguridad datos y contratos técnicos existentes.

**Marca e identidad**: **Vyntra**; estética premium, minimalista y gaming, con **Dark Crimson + negro**. Paleta orientativa (verificar contraste y coherencia): `#000000`, `#0B0B0D`, `#17171B`, `#3A0A12`, `#B31231`, `#E6E6E9` y grises de apoyo. Iconografía V angular original, sin margaritas. Las capturas conceptuales no son prueba de ejecución ni fuentes de metadatos.

La meta de tu documento es que el implementador pueda tomar **cualquier funcionalidad de la imagen** y saber **qué investigar, qué componentes crear o modificar, qué datos consume, cómo se conectan, qué rutas y estados maneja, qué riesgos existen y exactamente cómo comprobar que quedó terminada**.

## 2. CONTEXTO QUE DEBES CORROBORAR EN EL CÓDIGO

Árbol histórico de referencia (puede haber cambiado; **no lo presentes como verificado** sin inspección):

```text
app/                  Android / Kotlin / Gradle
desktop-rust/         Launcher de Windows / Rust / libretro
libretro/             Integración de emulación
libretro/armgba/      Core / integración GBA
linux/libretro-ds/    Core / integración Nintendo DS
gamedata/             Reconocimiento y datos del acompañante
website/              Web
docs/                 Manual y documentación
third_party/          Código externo
```

Núcleos documentados históricamente (confirmar según el repositorio):

| Sistema | Núcleo documentado | DLL personalizada heredada |
|---|---|---|
| GB / GBC | mGBA | `pokedaisy_gbc_libretro.dll` |
| GBA | armGBA | `pokedaisy_gba_libretro.dll` |
| NDS | melonDS | `pokedaisy_ds_libretro.dll` |

Posible directorio `desktop-rust/cores/`; posible variable `POKEDAISY_CORE_DIR`; propuesta futura `VYNTRA_CORE_DIR`. **No asumir** que los DLL estándar de Libretro encajan en el frontend personalizado, que un simple cambio de nombre los hace compatibles o que tienen APIs de datos de memoria específicas. El `melonDS DS` público puede diferir del `melonDS` parcheado usado por el proyecto. ArmGBA puede requerir compilación.

Estado histórico: Windows compiló con VS Build Tools 2022 y Windows SDK, pero faltaban núcleos; Android no fue ejecutado por ausencia de ADB/dispositivo. Ese dato puede estar obsoleto: **audita sin ejecutar builds ni instalar herramientas**.

### Inspección de SOLO lectura requerida

- Árbol, entrypoints, crates, módulos, carpetas Android, Gradle, dependencias, convenciones, tests, recursos, scripts y CI.
- Estado de Git (sin alterar nada), últimos commits relevantes, README(s), docs y warnings documentados.
- Flujo real del launcher: descubrimiento de biblioteca → elegir ROM → elegir núcleo → carga libretro → vídeo/audio/input → guardado → limpieza.
- Código y formatos actuales de datos en `gamedata/`; parsers de archivos `.sav`, `.srm`, save states, ROM hacks, detección de revisión/generación.
- Disponibilidad real de APIs para consultar `retro_get_memory_data`, memory maps/descriptores o hooks propios; no inventar símbolos ni suponer que `retro_get_memory_data` siempre expone el equipo.
- Arquitectura UI de escritorio (framework actual) y Android (Compose/Views); cómo gestiona hilos/event loop, persistencia, i18n y recursos.
- Localizar referencias `PokeDaisy`, `pokedaisy`, `POKEDAISY` en código propio y distinguir las de terceros/atribuciones.
- Licencias, componentes redistribuibles, atribuciones, riesgos de ROMs/BIOS y ausencia de propiedad de gráficos de referencia.

### Formato obligatorio de hallazgos

Para **cada afirmación importante del código**, cita **ruta exacta + símbolo (función/tipo) + líneas aproximadas cuando estén disponibles**. Distingue explícitamente **«Observado en repositorio»**, **«Propuesto»**, **«Sin verificar»** y **«Bloqueado»**. Nunca inventes archivos o nombres de símbolos existentes: los nuevos deben ir marcados **`[ARCHIVO NUEVO PROPUESTO]`** o **`[SÍMBOLO PROPUESTO]`**.

Si no puedes abrir el repositorio real, entrega un **plan preliminar claramente etiquetado**, con ruta por confirmar en vez de atribuir código inexistente; define exactamente qué deberá inspeccionar la otra persona.

## 3. FORMATO DE DOCUMENTO A ENTREGAR: NO UNA LISTA GENÉRICA

El resultado debe ser un documento profesional **«VYNTRA — PLAN DE IMPLEMENTACIÓN PARA DESARROLLADOR EXTERNO»**, autocontenido, en español. Incluye obligatoriamente las secciones siguientes, sin omitir ninguna:

### A. Resumen y límites

1. Arquitectura actual verificada vs arquitectura objetivo.
2. Principales decisiones de diseño, alternativas descartadas y motivos.
3. Qué características ya existen, están a medias, no existen, son imposibles de confirmar o dependen de un core.
4. Objetivos funcionales, compatibilidad GB/GBC/GBA/NDS, Windows/Android.
5. Principales riesgos y dependencias.
6. **Declaración de que no hiciste cambios de código**.

### B. Inventario exhaustivo de componentes y controles

Crea una matriz **para TODOS los botones, menús, pantallas, pestañas, selectores, accesos, teclas rápidas y menús contextuales** de las referencias. Columnas mínimas:

`ID | Plataforma | Ventana/pantalla | Control/acción | Estado actual (evidencia) | Evento disparado | Capa/servicio | Fuente real de datos | Persistencia | Navegación/resultado | Estado sin soporte/error | Criterio de aceptación | Prueba | Prioridad`.

Incluye filtros de biblioteca, búsqueda, grid/lista, play, continue, import, core settings, recientes, favoritos, slots, controles, Link, sincronización, y en Companion Equipo/Mochila/Pokédex/Mapa/Combate/Guía. Añade todos los controles visibles en ambas imágenes, incluidos minimizar, maximizar, mover panel, acoplar, desacoplar, monitor secundario, pantalla completa, barra del reproductor, pausa, cambio de velocidad y cerrar juego. No dejar controles como «fácil de implementar» o «conectar a backend» sin detallar el flujo.

### C. Arquitectura objetivo y diagramas

- Módulos, límites y comunicación: `Desktop UI ↔ aplicación ↔ servicios de dominio ↔ repositorios ↔ libretro/core ↔ adaptadores de memoria por juego ↔ Companion`.
- Compartición de estado entre Player y Companion en el mismo PC: comparar **estado compartido en el mismo proceso**, **múltiples ventanas en mismo proceso**, **proceso secundario + IPC local**, **mensajería/cola local**. Elegir según **framework real verificado**.
- Propiedad y ciclo de vida del emulador (un único propietario), serialización de snapshots inmutables, suscripción, cancelación, cierre, reconexión, pérdida de foco y desconexión del acompañante.
- Contratos de datos tipados, versionados, observabilidad y errores.
- Identidad de juegos: hash ROM/revisión/región/patch/engine para evitar mapeos incorrectos; diferenciar juego oficial y ROM hack con variantes de layout de memoria.
- Storage Windows/Android, compatibilidad de formato y migraciones.
- Diagramas Mermaid de componentes, secuencia de `Play`, `Companion snapshot` y `Save/Load`.

### D. Especificación por **archivo/tipo/función**

Para **cada feature** (ver inventario de la sección 4), define los siguientes ítems, con contenido concreto y no frases abstractas:

1. **ID y nombre** de tarea, dependencias, prioridad, estado actual con evidencia y tamaño relativo (S/M/L/XL; son estimaciones cualitativas).
2. **Objetivo del usuario y flujo paso a paso** (clic/tecla → validaciones → lógica → salida → UI/estado persistido).
3. **Archivos EXISTENTES que cambiaría el implementador** (ruta/símbolo observados) y **archivos NUEVOS PROPUESTOS** (ubicación, responsabilidad, razón).
4. **Clases, traits, structs, enums, data classes y métodos propuestos**: nombres, firma orientativa, tipos de parámetros, tipos de retorno, errores y efectos secundarios. No generar implementaciones completas.
5. **Eventos y contratos** con origen, consumidores, payload versionado, entrega, frecuencia y forma de cancelar/desuscribirse.
6. **Modelo de datos** y persistencia: entidad/campos/clave/índices/transacción/migración, esquema solo si justificado por la base existente.
7. **Estados de UI** (`idle`, `loading`, `empty`, `ready`, `disabled`, `unsupported`, `error`, etc.), transiciones y reglas de activación de botones.
8. **Algoritmo secuenciado o pseudocódigo explicativo** suficientemente preciso para que el humano escriba el código sin diseñar de nuevo todo el flujo.
9. **Errores/edge cases** y tratamiento: archivo corrupto, core ausente, permisos denegados, ROM hack desconocido, cambio de core, cierre imprevisto, audio cortado, estado guardado incompatible, etc.
10. **Aspectos no funcionales**: seguridad, threading, rendimiento, RAM, CPU, GPU, latencia, accesibilidad, localización y licencia.
11. **Pruebas exactas**: unitarias, integración, UI y manuales; fixtures legales/sintéticos; entrada, acción, salida esperada, criterio de fallo.
12. **Definition of Done** verificable, incluyendo datos reales o estado `no soportado`, sin botones ficticios.
13. **Rollback/migración** cuando toque datos, ABI, guardados, package ID o instaladores.

**Ejemplo del nivel de precisión exigido (SOLO EJEMPLO, NO AFIRMACIÓN SOBRE EL REPO):**

```text
Feature WIN-LIB-IMPORT-01 | Importar biblioteca
Existente: [ruta/símbolo verificados o «por localizar»]
Propuesto: [NUEVO] RomImportService, RomIdentity, ImportProgress, ImportIssue
Contrato: import_paths(paths, recursive, cancel_token) -> stream de progreso + resumen
Reglas: validar cabecera por sistema; ZIP solo lectura y con límites; hash del payload;
        clave de identidad con revisión; indexado transaccional; cancelación segura
UI: Idle -> Scanning(progress) -> Review(accepted, skipped, invalid) -> Completed
Errores: archivo ausente, permisos, ZIP corrupto, duplicados, ruta no confiable
Prueba: 4 ROMs homebrew válidas + 1 archivo .md + 1 ZIP mixto -> 4 registradas
Dependencias: esquema de biblioteca + sistema de eventos + selector de archivos
DoD: se importa, persiste, busca y reabre sin ejecutar ROM incorrecta
```

Para funciones con restricciones de core o permisos, especifica claramente **diseño ideal**, **fallback honesto** y **prueba necesaria**; nunca uses «implementar según corresponda» como sustituto del plan.

### E. Roadmap de implementación ordenado para UNA PERSONA QUE NO CONOCE EL PROYECTO

- Dividir en **etapas y vertical slices**, con prerequisitos reales y orden topológico; no empezar por animaciones ni por mocks desconectados.
- Para cada etapa: tareas `ID`, rutas principales, entregables, tests, validación manual, dependencias, criterios de salida y notas de reversión.
- Ruta crítica: **core funcional y pipeline de emulación → snapshot de datos confiable → Companion → ventanas sincronizadas → pulido UX**. Ajustar solo si el repositorio muestra una razón mejor.
- Diferenciar **MVP verificable**, **versión intermedia** y **visión completa** sin omitir nada del backlog.
- Dejar issues listas para copiar a GitHub/Jira, con subtareas marcables. Agrega pasos de «cómo comprobar» para cada issue.
- **Matriz de dependencias** para evitar asignar tareas imposibles antes de tener la fuente de datos.

### F. Entrega a tercero

- Guía «primer día» para situarse en repo, entender arquitectura y reproducir entorno sin usar conocimiento previo del autor.
- Decisiones pendientes y preguntas del implementador, con **alternativa recomendada por defecto**, impacto y prueba para resolverlas.
- Lista de instalaciones/requisitos para **que el humano las realice después**, no tú.
- Comandos futuros de build/test/diagnóstico diferenciados de lo que tú verificaste solo leyendo.
- Especificar que el implementador deberá documentar cambios, riesgos, compatibilidad, evidencia de funcionamiento y limitaciones.
- Al final, un checklist de **cobertura del 100 % de pantallas y acciones** y otra tabla de **riesgos/imposibilidades reales**.

## 4. INVENTARIO FUNCIONAL MÍNIMO: PLANIFICA CADA SUBFLUJO

Desarrolla fichas técnicas como se exige en la sección 3.D para **TODAS** las siguientes áreas. Se pueden agrupar tareas repetidas sin perder acciones individuales.

### 4.1. Marca, diseño e infraestructura

- **BRAND-01**: inventario PokeDaisy → Vyntra, logo `V`, iconos, splash, nombre de ventana/app/web, assets, metadata y licencias; nunca alterar avisos de copyright de terceros.
- **BRAND-02**: directorios y migración de preferencias/saves, nombres de DLL y fallback; `VYNTRA_CORE_DIR` preferente + `POKEDAISY_CORE_DIR` heredado; caso especial de `applicationId` Android, firma y datos sandbox.
- **UI-01**: design tokens Black / Night / Charcoal / Dark Crimson / Crimson; contrastes, accesibilidad, foco, hover, teclado/gamepad/touch, tipografía y layout para DPI/resoluciones.
- **UI-02**: componentes reutilizables, tablas, cards, campos, estados empty/error/loading, notificaciones, animaciones sobrias y modo de movimiento reducido.
- **INFRA-01**: base de errores tipados, logs saneados, telemetría opt-in, configuración versionada, pruebas y CI.

### 4.2. Vyntra Desktop — Launcher

- **WIN-LIB-01**: biblioteca de ROMs, explorar carpetas, escaneo incremental, validación de cabeceras, clasificación GB/GBC/GBA/DS, ZIP seguro, deduplicación por hash/revisión, ROM hacks y migración de índice.
- **WIN-LIB-02**: portada/metadata legal, búsqueda y orden, favoritos, grid/lista, contador por consola, contexto y detalles por juego, archivo inexistente y relocalización.
- **WIN-LIB-03**: importar archivos/carpeta, progreso y cancelación, vista de errores, resultados y estado al reiniciar.
- **WIN-CORE-01**: descubrimiento y selección de núcleos, validación real de carga/exposiciones ABI/dependencias, estados, logs y fallback de rutas.
- **WIN-CORE-02**: plan de obtención de mGBA, armGBA, melonDS, nombres personalizados, build de cores, pruebas libretro; qué puede hacer un core público y qué requiere integración propia.
- **WIN-PLAY-01**: arranque/continuar, render de pantalla, audio, reloj/frame pacing, inputs, foco, pausa, cierre, fullscreen, control de errores, crash isolation y contabilización de tiempo.
- **WIN-PLAY-02**: overlay del reproductor, barra de herramientas, pausa/reanudar, quick save/load, fast forward, rewind y sus restricciones, teclado/gamepad.
- **WIN-VAULT-01**: `save states` y saves internos, slots, lista recientes, miniaturas, import/export, copias de seguridad, compatibilidad y recuperación segura.
- **WIN-SET-01**: Settings de video, audio, cores, ROMs, guardados, atajos, mandos, rendimiento, idioma, privacidad, accesibilidad, diagnósticos.
- **WIN-SYNC-01**: sincronización/transferencia de partidas entre dispositivos, selección de versión, integridad, conflictos y consentimiento; distinguir claramente de Link Play.
- **WIN-LINK-01**: emparejamiento/juego por enlace donde el core realmente lo permita; GB, GBC, GBA y DS pueden diferir; mecanismo, red, latencia, desconexiones y pruebas con dos instancias.
- **WIN-SYS-01**: lista Systems, filtros y compatibilidades reales por plataforma, incluyendo estados de core.

### 4.3. Vyntra Player + Vyntra Companion — MISMO PC (requisito central)

- **MULTI-01**: diseño de ventanas independientes Player / Companion, crear/cerrar/volver a abrir el Companion sin detener emulación; minimizar, maximizar, alt-tab, resize, DPI mixto, múltiples monitores, persistir posiciones y evitar ventanas fuera de pantalla.
- **MULTI-02**: modo **dual** (dos ventanas lado a lado), **split** (ambos paneles en una ventana con separador ajustable) y **dock** (Companion acoplado lateral). Cambio en caliente sin reiniciar el juego, preservando foco y estado. Comparar costo según toolkit actual.
- **MULTI-03**: bus observable de datos del emulador, snapshots con revisión/monotonic timestamp, versionado y notificaciones sin bloquear hilo de emulación; throttle/coalescing cuando sea necesario.
- **MULTI-04**: sincronización real `player → companion`, selección de perfil ROM+revisión+core, eventos de inicio/cierre/cambio de save/pausa/rewind, estado `not connected`, `unsupported` y `stale data`.
- **MULTI-05**: adaptadores de memoria de juego por plataforma/gen/revisión; fuente `retro_get_memory_data`/memory descriptors/hooks si comprobadas, lectura segura en hilo apropiado, parser versionado, fallbacks desde `.sav` cuando proceda, nunca inventar equipo/ubicación.
- **MULTI-06**: accesos globales y navegación por mando/teclado, foco entre ventanas, overlay/acoplamiento sin que el Companion robe inputs del juego.
- **MULTI-07**: salida de datos del Companion (API local/IPC) SOLO si justifica el aislamiento real; seguridad por proceso/usuario, cierre de puertos, autenticación si hay red, no requerir servidor cloud para dos ventanas del mismo PC.
- **MULTI-08**: comportamiento en DS: doble pantalla del juego, capa de input táctil, panel complementario separado y optimización de resoluciones/escala.

**Para MULTI-01 a MULTI-08, redacta también una tabla comparativa de estrategias**: `multiwindow mismo proceso`, `ventanas separadas con IPC local`, `panel embebido` con pros/contras, latencia, complejidad, reutilización de UI y compatibilidad framework. Selecciona una opción principal sustentada en inspección real, con plan alternativo.

### 4.4. Companion — seis pantallas completas

- **CMP-TEAM-01 / Equipo**: seis slots, especie, forma, género, nivel, HP, condición, tipos, habilidad, stats, experiencia, ataques y PP; detalle al seleccionar, refresco y estado de origen/fecha de snapshot. Lectura de datos solo si hay adaptador conocido.
- **CMP-BAG-01 / Mochila**: categorías, cantidades, búsqueda, filtros, ficha de objeto, selección, scroll eficiente, sincronización; **solo lectura** hasta que exista API de escritura autorizada y segura.
- **CMP-DEX-01 / Pokédex**: búsqueda, filtros, navegación prev/next, tipos, evolución, movimientos, hábitat/aparición por juego/revisión, visto/capturado real cuando sea posible; base offline y cache.
- **CMP-MAP-01 / Atlas**: región, ruta, ciudad, niveles de zoom/pan, puntos de interés, marcador del jugador solo si existe dato real, localización por juego, ROM hack desconocido, assets propios/licencias, vista textual alternativa.
- **CMP-BATTLE-01 / Combate**: detección de turno, aliado/rival y estado solo cuando existan hooks; efectividad de tipos por generación, inmunidades, cambios generacionales, tipos duales, PP/potencia/precisión, estrategias limitadas a datos conocidos, modo de cálculo **manual** como fallback funcional.
- **CMP-GUIDE-01 / Guía**: índice de capítulos, búsqueda, pasos, casillas y notas persistidas, progreso automático únicamente si hay fuente fiable, favoritos, evitar spoilers, guías offline y por versión/revisión.

Para **cada pantalla CMP** describe **dos modos** con flujos independientes:

**Modo A — Datos en vivo:** soporte comprobado por core y adaptador específico, origen y frecuencia de actualización, manejo de frames/guardados, error ante memoria inválida.

**Modo B — Datos estáticos/manuales:** modo siempre útil sin lectura en vivo (Dex por dataset, calculadora de efectividades manual, checklist de guía, consulta de objetos, mapas de referencia), etiquetado sin inducir a creer que el juego está sincronizado.

Planifica además comportamiento de pestañas, atajos, selección, pantallas vacías y pérdida de conexión.

### 4.5. Vyntra Pocket (Android)

- **AND-01**: app Android con marca Vyntra, home/biblioteca y detalles, importar ROM via SAF y URI persistida, validación, reanudación tras suspensión/rotación, permisos correctos.
- **AND-02**: core Android `.so`/JNI/NDK (o puente real verificado), arquitectura ABIs, ciclos app, audio y renderer, pantalla de juego y overlay.
- **AND-03**: GB/GBC/GBA vs Nintendo DS: pantalla superior/inferior, distribución horizontal/vertical, escalado, swap, stylus/touch, guardar layout.
- **AND-04**: paneles Companion Equipo/Mochila/Dex/Mapa/Combate/Guía con lógica compartida cuando sea viable, sin bloquear el frame loop.
- **AND-05**: guardar/cargar estados, quicksave, rewind y fast forward por core, límites de memoria, sistema de archivos, errores y compatibilidad.
- **AND-06**: controles táctiles remapeables, gamepads USB/Bluetooth, perfiles por sistema/juego, deadzones, tamaño/transparencia y vibración, accesibilidad.
- **AND-07**: Link Play y transferencia de partidas con verificación de capacidades reales, privacidad/red y separación entre modos.
- **AND-08**: more/settings, idioma, rendimiento, tema, personalización, batería/temperatura, diagnóstico y licencias.
- **AND-09**: migración Android de PokeDaisy a Vyntra sin prometer acceso automático a datos entre `applicationId` distintos.

### 4.6. Website, docs, empaquetado y QA

- **DOC-01**: web/manual/README bajo Vyntra, imágenes reales vs conceptuales, licencias, marcas, tutoriales, comandos y rutas actualizadas.
- **REL-01**: releases Windows/Android, scripts, assets, nombre de instalador, firma, actualización compatible, distribución de cores según licencias.
- **QA-01**: pruebas Rust/Kotlin, libretro integration, fixtures legales, smoke Windows, pruebas Android en dispositivo, pruebas de sincronización multiwindow y multi-monitor, regresión de partidas.
- **QA-02**: matriz de cobertura GB/GBC/GBA/DS × Desktop/Android × core × función × prueba × limitación.

**NO reduzcas los apartados anteriores a una lista de títulos**: conviértelos en fichas operativas completas con rutas, contratos, orden de trabajo y criterios testables.

## 5. ASPECTOS DIFÍCILES: SE EXIGE SOLUCIÓN Y JUSTIFICACIÓN

### 5.1. ¿Cómo obtiene Companion los datos?

Analiza separadamente estas vías:

1. Datos estáticos en `gamedata/` para Pokédex, tipos, objetos, mapas y guías.
2. Guardados internos (`.sav` / SRAM u otros) para snapshots puntuales con parser válido por revisión, sin asumir lectura segura durante escrituras.
3. Memoria emulada (si core/libretro/API de proyecto expone regiones) para equipo, mochila, coordenadas, batalla y flags.
4. Hooks/eventos específicos del core o plugin de depuración, con contratos y compatibilidad.
5. Entrada manual del usuario cuando no exista fuente fiable.

Explica **capacidad, latencia, consistencia, diferencias entre GBA/GB/GBC/NDS, restricciones y coste**. Planea un `GameDataCapabilities` o equivalente para detectar disponibilidad **por combinación de core + ROM hash + versión + adaptador**, nunca con la sola extensión `.gba`. Diseña un caché y un snapshot inmutable. Si la lectura automática de una función no es verificable, deja tarea de investigación con evidencia, prototipo posterior (a cargo del humano) y fallback útil.

### 5.2. Ciclo de vida de ventanas

Especifica sucesión de eventos:

1. Lanzar ROM desde biblioteca.
2. Iniciar Player y Companion vacíos/desconectados.
3. Establecer sesión del core y verificar adaptador.
4. Producir snapshots y pintar datos si válidos.
5. Cambiar pestañas de Companion sin pausar o robar input del juego.
6. Mover Companion a segundo monitor; cambiar resolución/DPI.
7. Minimizar/cerrar/reabrir Companion sin cerrar juego ni perder saves.
8. Cambiar de juego/estado, usar rewind, cerrar core y cortar suscripción.
9. Manejar core crash, ROM desconocida, proceso caído, estado atrasado o pérdida de memoria.

Para cada paso, identifica servicio propietario, mensajes, estado esperado, persistencia y prueba. Mantén la emulación aislada de actualizaciones de UI lentas.

### 5.3. Cuestiones de emulación y guardados

- Separar `libretro` estándar de **extensiones propias del repo**; especificar interfaces de detección de capacidades reales.
- Planear concurrencia/hilo de core, API de devolución de frame, audio, ventana, pausa, fast-forward, rebobinado, estados y backup.
- Evitar perder datos al cambiar save states o cerrar ventana; recuperación post-crash.
- Diferenciar save ram de save state; hash de ROM + core + versión + checksum + metadatos.
- Link Play separado de Sync, con matriz real de motores y reglas de red.
- No redistribuir ROMs, BIOS, assets comerciales; licencias correctas de cada motor.

## 6. PLAN DE EJECUCIÓN PARA EL IMPLEMENTADOR: FASES Y CORTES VERTICALES

Organiza el trabajo en fases **reordenadas según dependencias verificadas**. Guía orientativa (no rígida):

- **F0 / Auditoría humana y línea base**: setup, inventario, requisitos y fixtures.
- **F1 / Contratos y seguridad**: modelos de sesión, estados, bus/snapshots, persistencia y migración.
- **F2 / Core y Player verificables**: cargar core real, jugar, input, audio/vídeo, errores, cerrar seguro.
- **F3 / Biblioteca/Core Manager/Vault**: importación, catálogo, guardados y ajustes.
- **F4 / Companion útil sin leer memoria**: Dex, mapas de referencia, guías, calculadora manual, UI multiwindow funcional sin inventar estado.
- **F5 / Datos en vivo e integración**: adaptadores de lectura por juego/revisión y sincronización Player → Companion; Equipo/Mochila/Ubicación/Combate reales.
- **F6 / Modos de ventanas**: dual, split, dock, múltiples monitores, foco y estado persistido (puede adelantarse una base mínima).
- **F7 / Android y paridad**: emulación real y Companion Android, layout DS, mandos y persistencia.
- **F8 / Funciones avanzadas y conectividad**: Link según soporte, sync/transferencia, rewind, perf.
- **F9 / Branding final, accesibilidad, packaging, documentación y QA completo** (el rebranding visible puede empezar antes, según decisión).

Cada fase debe incluir:

- Alcance exacto y tareas por ID.
- Orden de commits/slices **propuestos** (NO ejecutar commits).
- Archivos/rutas a tocar (verificados o propuestos).
- Dependencias y puertas de entrada/salida.
- Tests automáticos futuros + pruebas manuales precisas.
- Riesgos, estrategias de fallback y cómo volver atrás.
- Entregable demostrable y qué no se debe dar por terminado.

Incluye **un primer conjunto de 10 a 20 tareas listas para asignar**, cada una con instrucciones tan detalladas que el desarrollador sepa exactamente su próximo archivo/símbolo a revisar, su contrato a crear, su flujo, validación y definición de terminado. Después lista el resto con el mismo formato, sin ocultarlo bajo «Etc.».

## 7. RESTRICCIONES DE CALIDAD DEL PLAN

- **No inventar hechos del repo**: cada detalle existente debe respaldarse en una ruta/símbolo observado. Diferenciar contrato propuesto vs existente.
- **No decir «usar una API» sin especificar cuál, dónde se adapta, requisitos, errores y cómo verificar su presencia.**
- **No usar «todo debe funcionar» como criterio de cierre**: pruebas falsables, escenarios de éxito/fallo, captura de evidencia.
- **No confundir que una función tenga botón con que tenga soporte real**: indicar estados `unsupported` o `manual` y qué habilitaría el modo live.
- **No exponer ni escribir datos reales de partidas privadas**; usar fixtures sintéticos o ROMs homebrew que el desarrollador tenga derecho a usar.
- **No basarse en la ficción de las imágenes**: juegos, Pokémon, HP, movimientos y estados de núcleo visibles son ejemplos de diseño.
- **No prometer Link Play GB/GBC/GBA/DS sin pruebas** ni sincronización remota sin backend confiable.
- **No asumir arquitectura Windows/Android idéntica**: proponer lógica compartida solo si coste/tecnología lo hacen sensato.
- **No asignar horas ni fechas cerradas si no hay datos suficientes**; usar complejidad relativa, riesgos y dependencias.
- **No presentar el cambio de `applicationId` Android como simple rename**: cubrir conservación de actualizaciones y datos del sandbox.
- **No asumir que renombrar una DLL externa equivale a compilar un core de Vyntra**.
- **No recomendar reescribir un motor de emulación desde cero** si el repo ya aporta componentes con licencias compatibles.

## 8. ESTRUCTURA EXACTA DE LA RESPUESTA QUE TIENES QUE ENTREGAR

Redacta en **español** y en este orden:

1. `# VYNTRA — PLAN TÉCNICO DE IMPLEMENTACIÓN (SOLO DOCUMENTACIÓN)` y declaración de solo lectura.
2. `1. Resumen ejecutivo y arquitectura existente` (hechos con rutas y símbolos).
3. `2. Brechas respecto a las dos referencias visuales`.
4. `3. Decisiones de arquitectura y trade-offs` (incluida elección multiwindow/IPC).
5. `4. Modelo de dominio, contratos y eventos` (tipos, firmas y esquemas propuestos).
6. `5. Diseño de almacenamiento y migración PokeDaisy → Vyntra`.
7. `6. Especificación pantalla por pantalla y control por control` (matriz total).
8. `7. Fichas técnicas por feature` (IDs de la sección 4; archivos, funciones, pseudoflujos, edge cases, pruebas, DoD).
9. `8. Plan específico de Player + Companion en dos ventanas del mismo PC` (secuencia detallada, foco, DPI, sincronización, recuperación).
10. `9. Plan de datos en vivo, adaptadores y matriz de compatibilidad por core/ROM`.
11. `10. Roadmap, dependencias, prioridades, MVP/intermedia/completo`.
12. `11. Primeras tareas entregables para el desarrollador` (10–20 issues extremadamente detalladas).
13. `12. QA, pruebas E2E, seguridad, licencia, rendimiento y accesibilidad`.
14. `13. Riesgos, restricciones técnicas, bloqueos y decisiones por validar`.
15. `14. Guía de traspaso al desarrollador y comandos FUTUROS de validación`.
16. `15. Matriz final de cobertura de pantallas / botones / tareas, con ID sin asignar = error`.

Incluye diagramas Mermaid textuales y tablas cuando ahorren ambigüedad. En secciones demasiado largas **prefiere completar todos los módulos con detalle útil** a gastar espacio en introducciones. Si hay límite de respuesta, comienza por inventario, arquitectura y roadmap y **continúa tu respuesta hasta completar** el contenido que permita el entorno, señalando qué queda aún sin especificar. Nunca escribas «implementación completada»; tu único entregable es el plan.

### Última orden (máxima prioridad)

**LEE, ANALIZA Y PLANIFICA; NO PROGRAMAR, NO CREAR ARCHIVOS, NO EJECUTAR BUILDS, NO INSTALAR, NO COMMIT, NO CAMBIAR NADA.** El plan debe estar redactado para una tercera persona que implementará posteriormente y necesita saber casi línea por línea qué tendrá que diseñar y escribir, respetando la arquitectura real del repositorio, sin asumir características inexistentes. Las dos referencias de UI son **objetivos visuales**, no prueba de funcionamiento.

# FIN DEL PROMPT PARA CODEX
