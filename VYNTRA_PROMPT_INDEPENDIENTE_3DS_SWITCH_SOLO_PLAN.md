# VYNTRA — PROMPT INDEPENDIENTE DE ARQUITECTURA 3DS + SWITCH

> **Tipo de encargo:** planificación técnica exclusivamente. **Producto:** Vyntra. **Prioridad:** incorporar las capacidades complementarias para juegos Nintendo 3DS y Nintendo Switch en el proyecto Vyntra existente.
>
> **Este es un encargo NUEVO y AUTÓNOMO:** no es una continuación ni una modificación de otro prompt o planificación. No presupongas que se haya implementado nada de los planes anteriores.

## PROMPT PARA CODEX (COPIAR COMPLETO)

Eres **arquitecto principal de software, especialista en emulación, frontend de videojuegos, sistemas multiplataforma y herramientas de acompañamiento de juegos**. Te entregaré el repositorio del proyecto **Vyntra** (antes PokeDaisy). Tu trabajo es **examinarlo en modo solo lectura** y redactar **un plan nuevo, independiente, exhaustivo y ejecutable por otra persona** para extender Vyntra con un ecosistema de acompañamiento de juegos de **Nintendo 3DS** y **Nintendo Switch**.

### REGLA INNEGOCIABLE: SOLO PLANIFICACIÓN

- **NO programes ni implementes nada**. NO crees, edites, elimines, renombres ni formatees archivos. NO generes ramas, commits, parches, PR ni cambios de configuración.
- NO ejecutes scripts, builds, pruebas, linters, migraciones, generadores, instaladores, comandos que cambien el estado del repositorio ni descargas de dependencias. NO instales software o cores.
- Permite únicamente la **inspección de solo lectura**: árbol de directorios, búsqueda de referencias, lectura de código y documentación, y consulta de fuentes públicas cuando estén disponibles y sea necesario contrastar afirmaciones. Si la política del entorno requiere autorización para alguna lectura, respétala.
- NO sustituyas el plan por prototipos, código de ejemplo listo para ejecutar, mockups, capturas o artefactos de diseño. Puedes incluir **firmas propuestas, esquemas textuales, pseudocódigo descriptivo, contratos, diagramas Mermaid y nombres de archivos**, siempre como especificaciones y **sin escribir código en el repositorio**.
- Entrega **todo el plan en tu respuesta**, en Markdown; si no cabe en una sola respuesta, entrégalo en partes numeradas y conserva un índice y referencias entre ellas. No crees archivos de planificación en el repositorio.
- No afirmes que una función está implementada, es compatible, se ejecuta o funciona si no está demostrado por los archivos inspeccionados o fuentes verificables.

### CONTEXTO Y LÍMITES DEL PROYECTO

Vyntra es una plataforma de ejecución, biblioteca y acompañamiento de juegos, centrada originalmente en Pokémon. Usa una identidad visual **negro + dark crimson**, interfaz moderna para Windows y Android, y separa el juego de paneles de información complementaria.

Estructura conocida del proyecto original, **a verificar** en el repositorio real: `desktop-rust/` (lanzador Windows y frontend Rust), `app/` (Android Kotlin/Gradle), `libretro/`, `libretro/armgba/`, `linux/libretro-ds/` (integraciones de emulación), `gamedata/` (reconocimiento de juegos y datos), `website/`, `docs/` y `third_party/`. Existe un legado de núcleos GB/GBC, GBA y DS; **no elimines ni desestabilices el soporte anterior**. No presupongas que el rebranding de PokeDaisy a Vyntra ya terminó: indícalo como dependencia cuando corresponda, sin convertir este encargo en otro plan completo de rebranding.

**Ámbito de este plan**: crear un **complemento funcional** para juegos 3DS y Switch dentro de Vyntra, con integración de biblioteca, ejecución mediante soluciones compatibles cuando sea viable, Companion separado del juego, datos por título, guardados, perfiles, control, interacción entre ventanas, diagnóstico y extensibilidad. **La prioridad funcional es el acompañante**, no escribir un emulador nuevo desde cero.

**Alcance legal y de seguridad**: considera únicamente juegos y copias obtenidos legítimamente, software homebrew y materiales que el usuario tenga derecho a usar; no incluyas ROMs, BIOS, firmware, claves, certificados, descifradores, técnicas de evasión de DRM ni instrucciones para obtener material de terceros. Analiza requisitos de usuario y licencia sin suministrar ni automatizar distribución de esos materiales. No dependas de servicios no oficiales, repositorios dudosos o scraping prohibido.

### OBJETIVO FINAL — EXPERIENCIA DE PRODUCTO

El usuario abre Vyntra, importa o detecta un juego 3DS/Switch compatible de su propia biblioteca, ve su ficha, selecciona un motor disponible, lo inicia cuando es viable y dispone de un **Vyntra Companion** en otra ventana o monitor. La interfaz complementaria ofrece, **según el juego y la calidad de integración**: equipo, estadísticas, inventario, mapa, objetivos, Pokédex/enciclopedia, encuentros, combates, registro de sesión, guía contextual, guardados y herramientas de análisis. Cuando no exista lectura en tiempo real, el Companion debe funcionar con **datos estáticos, actualización manual o importación de guardados autorizada**, indicando claramente el nivel de compatibilidad.

Diseña esto con **adaptadores intercambiables**: `GameProfile`, `EmulatorAdapter`, `TelemetryProvider`, `SaveAdapter`, `InputAdapter`, `ScreenLayoutAdapter`, `CompanionModule` y un catálogo de **capacidades** negociadas explícitamente. Los nombres son orientativos; ajústalos a la realidad del proyecto.

## FASE 0 — AUDITORÍA OBLIGATORIA (SOLO LECTURA)

Antes de decidir tecnología:

1. Construye un **mapa real del repositorio**: archivos, módulos, dependencias, tecnología UI de Windows, interop con núcleos, arquitectura Android, formatos y fuentes de `gamedata`, guardados, biblioteca, eventos y comunicación de estado.
2. Identifica qué parte de Vyntra ya puede reutilizarse y qué parte debe construirse; demuestra hallazgos con **rutas de archivo, símbolos y referencias**. No inventes rutas ni funciones existentes.
3. Separa **hechos confirmados / inferencias / hipótesis por validar / decisiones pendientes**.
4. Identifica límites de licencia, distribución, rendimiento, seguridad, riesgo de regresión y compatibilidad Windows/Android.
5. Evalúa si es razonable empezar con **Windows** y dejar **Android** en un segundo hito, con criterios claros (API, GPU, memoria, drivers, fuentes disponibles, distribución y licencias).
6. Registra los **datos y acciones que debe proporcionar el usuario** y evita bloquear las partes que no dependen de ellos.

**Entregable A:** inventario auditado con tabla `Ruta | Estado | Evidencia | Reutilizable | Gap | Riesgo | Tarea propuesta`.

## FASE 1 — INVESTIGACIÓN Y SELECCIÓN DE MOTORES

Haz una matriz comparativa **separada para 3DS y Switch**, sin dar por hecho que ambos usan libretro.

**Nintendo 3DS:** investiga y contrasta **Azahar** (incluido su core libretro cuando corresponda), alternativas legítimas actuales y las posibilidades de integración directa, mediante proceso externo y por IPC. Revisa licencias, Windows/Android, render, doble pantalla, controles táctiles, guardados, API/capacidades, estabilidad y disponibilidad verificable de binarios. Distingue claramente soporte real de los requisitos del proyecto. Las capacidades de rebobinado, netplay, memoria, estados y cámara/sensores requieren verificación específica: no prometas soporte universal.

**Nintendo Switch:** estudia opciones de integración con emuladores actualmente disponibles y jurídicamente utilizables, **sin asumir un core libretro de Switch para PC ni una DLL `vyntra_switch_libretro.dll`**. Examina compatibilidad de integración por lanzador externo, argumentos documentados, gestión de proceso, IPC cuando el motor lo ofrezca, telemetría con autorización, observación de ventana, lectura de guardados y modo **Companion standalone** que no dependa de emulación. Justifica cualquier motor propuesto con fuente, fecha, licencia y estado. Si no existe interfaz pública confiable, especifica un adaptador de funcionalidad degradada en lugar de inyección, hooking invasivo o capacidades inventadas.

Para cada motor o integración candidata especifica:
- Compatibilidad Windows / Android, requisitos mínimos y recomendados, fuente oficial, licencia y mantenimiento.
- Mecanismo de ejecución, rutas y gestión de configuración, logs, errores y retorno a Vyntra.
- Posibilidad de capturar vídeo/audio/input y **restricciones por API**, no solo ideas teóricas.
- Estados guardados, guardados normales, compatibilidad entre versiones, almacenamiento y backup seguro.
- Disponibilidad y legitimidad de información de juego, comunicación de estado y lectura de datos.
- Riesgos de distribución, seguridad, mantenimiento, actualizaciones y bloqueo por proveedor.
- Decisión final **Recomendado / Alternativa / Experimental / No viable ahora**, con explicación verificable.

**Entregable B:** ADRs (Architecture Decision Records) separados para 3DS y Switch con alternativas descartadas y plan de reversión.

## FASE 2 — CAPAS FUNCIONALES QUE DEBES PLANIFICAR

### 2.1 Biblioteca y reconocimiento 3DS/Switch

- Exploración incremental de carpetas, detección segura por cabeceras/metadatos cuando sean accesibles, hash opcional, deduplicación, regiones, idiomas, versiones, actualizaciones y DLC **solo como metadatos o archivos legítimos aportados por el usuario**.
- Formatos a evaluar (no confundir identificación con capacidad de ejecutar): 3DS `.3ds`, `.cci`, `.cxi`, `.3dsx`, `.cia`; Switch `.nsp`, `.xci`, `.nca`, `.nro` y formatos pertinentes. Especifica validación, límites, seguridad de archivos y soporte según motor; no atribuyas soporte automático a todas las extensiones.
- Biblioteca unificada con las consolas existentes, carátulas mediante fuentes autorizadas, listas, búsquedas, perfiles, indicadores de compatibilidad y rutas independientes del motor.
- Si el formato no se puede inspeccionar sin material confidencial o requisitos ajenos al alcance, define un estado **No identificado / Requiere configuración del usuario** sin tratar de descifrarlo.

### 2.2 Lanzamiento y ciclo de vida de sesiones

- Selección explícita del motor disponible, comprobación no intrusiva de capacidades, perfiles por juego y validación previa.
- Inicio, pausa si el motor la admite, foco, cierre normal, recuperación tras fallo, logs y detección de cierre externo.
- IPC autenticado local, heartbeats, reconexión, gestión de múltiples instancias y prevención de pérdida de datos.
- Modo launcher integrado, modo emulador externo y modo **solo Companion**.
- No mezcles el estado de la sesión del emulador con el estado de módulos del Companion.

### 2.3 Vyntra Companion para Pokémon 3DS

Planifica módulos específicos para Pokémon X/Y, Omega Ruby/Alpha Sapphire, Sun/Moon y Ultra Sun/Ultra Moon, distinguiendo **perfiles por versión, región, idioma y actualización**; añade extensiones para otros títulos 3DS si aportan utilidad.

Funcionalidades:
- Equipo: especies, nivel, naturaleza, habilidad, tipos, movimientos, estado, objetos, stats y cálculos disponibles con su procedencia.
- Mochila: inventario y favoritos; consulta estática/manual o lectura autorizada cuando sea verificable.
- Pokédex: búsqueda, formas, evoluciones, métodos, tipos, movimientos, capturas registradas y comparaciones.
- Combate: tabla de tipos, estimador de daño con supuestos visibles, matchup, velocidades, clima, terrenos y cambios de mecánicas según generación.
- Atlas: regiones, rutas, marcadores, encuentros, objetivos, cuevas, zonas y progreso; mapa interactivo contextual.
- Guía: hitos, misiones, coleccionables, consejos y sistema antiespoilers.
- Recolección y registro: eventos, equipo histórico, log de sesión y progreso manual, sin afirmar que un evento fue observado cuando no exista telemetría.
- 3DS doble pantalla: panel primario/secundario intercambiables, escalado independiente, ventana secundaria, touch con mouse y control, y modos de ventana/monitor.
- Características especiales: funcionalidad 3D estereoscópica solo si está soportada, sensores/cámara/micrófono solo si el motor y permisos lo permiten.

### 2.4 Vyntra Companion para Pokémon Switch

Planifica perfiles independientes para Let's Go Pikachu/Eevee, Sword/Shield, Brilliant Diamond/Shining Pearl, Legends: Arceus, Scarlet/Violet y **otros juegos Pokémon de Switch que se confirmen en el inventario de contenido**. No des por sentado que comparten estructura de guardados, combate, mapa o telemetría.

- Experiencia de pantalla única en alta resolución, docking del Companion, overlay no intrusivo y segunda pantalla externa.
- Equipo, bolsa, Pokédex, mapa, tareas, misiones, clima/tiempo cuando tenga sentido, progresión, objetivos y log.
- **Perfiles de juego y generaciones**: combate clásico, cambios en Legends, exploración de mundo abierto, mecánicas individuales, objetos y evolución.
- Guía contextual opcional por juego, seguimiento manual de objetivos, rutas de exploración, listas de tareas y coleccionables.
- Analizador de combates solo si los datos están disponibles y el modelo matemático corresponde al título.
- Guardados e importación: planifica parsing **solo si formato, permisos y exactitud están verificados**. Nunca modifiques un guardado por defecto.
- Planifica degradación elegante cuando se use el juego en una consola física y Vyntra solo haga acompañamiento manual o por datos aportados voluntariamente.

### 2.5 Conectividad y modos complementarios

Define experiencias separadas:
1. **Mismo PC: Vyntra Player + Companion** en dos ventanas con ventanas desacopladas, always-on-top opcional, snap, redimensionamiento y segundo monitor.
2. **Un PC: pantalla de juego + múltiples widgets** (equipo, mapa, Pokédex, combate) con layouts guardados.
3. **Juego en Switch/3DS física + Companion en PC o Android**, sin prometer acceso automático al estado de la consola.
4. **PC + Android como pantalla complementaria** mediante protocolo local seguro si es factible, con consentimiento, pairing y fallback manual.
5. **Juego no ejecutable + consulta offline**: biblioteca, Pokédex, guías, mapa, tareas, planificación de equipos, historial y Vault funcionan sin emulador.

Para cada modo, especifica descubrimiento, emparejamiento, reconexión, latencia objetivo, seguridad, ancho de banda y estados de error.

### 2.6 Vyntra Vault y datos persistentes

- Guardados normales frente a save states; diferencias por motor y por juego.
- Backup con escrituras atómicas, snapshots, política de retención, checksum, compatibilidad por juego/versión y restauración segura.
- Carpetas por usuario, consola, juego, edición, región, versión, motor y perfil para evitar colisiones.
- Migración desde datos GB/GBC/GBA/NDS sin pérdida ni renombrados peligrosos.
- Exportación/importación consentida de perfiles, progreso manual y configuración.
- Nunca garantices compatibilidad de partidas entre motores sin evidencia.

### 2.7 Core Manager / Emulator Manager

- Distingue **core libretro** de **motor externo** y **módulo Companion**; separa la interfaz y la lógica para evitar que Switch se modele falsamente como una DLL.
- Detectar instalado, compatible, incompatible, no encontrado, experimental, desactualizado, sin permisos, licencia no verificada.
- Proponer flujos de instalación manual segura, verificación de integridad/origen y selección de ruta; el plan no debe realizar descargas.
- Diseñar configuración por motor, pruebas de humo para el desarrollador y fallbacks.

### 2.8 UI/UX — DARK CRIMSON + BLACK

Usa como identidad de Vyntra negro profundo y crimson oscuro, con contraste WCAG, estados claros y animación moderada. Planifica estas pantallas:

- Home / Dashboard multiplataforma con últimos juegos, actividad y acceso rápido.
- Biblioteca con tabs GB, GBC, GBA, NDS, 3DS, Switch, filtros y etiquetas.
- Game Detail 3DS y Switch: portada, edición, integraciones disponibles, estado de telemetría, compatibilidad y acción adecuada.
- Setup Wizard para 3DS y Switch: dependencias legales, selección de motor, configuración y prueba guiada **a implementar en el futuro**.
- Vyntra Player para 3DS: dos pantallas, layouts, controles táctiles y perfiles.
- Vyntra Player / External Launcher para Switch: estado de sesión y límites reales.
- Companion: Team, Bag, Dex, Battle, Atlas, Guide, Goals, Timeline, Vault y Settings.
- Layout Designer multiventana: docking, segundo monitor, widgets y presets.
- Diagnostics & Compatibility: capacidades verificadas, registro de errores y reporte reproducible.

Para **cada pantalla** describe: jerarquía visual, componentes y tamaños propuestos, acciones por botón, navegación, estados de carga/error/vacío, accesibilidad, teclado/gamepad, persistencia, eventos, datos y pruebas. Explica cómo se adapta de Windows a Android sin obligar a replicar exactamente la misma disposición.

### 2.9 Funcionalidades nuevas para superar la idea original

Aporta **al menos 15 capacidades adicionales** específicas de 3DS/Switch, además de lo anterior, sin agregar relleno. Considera, por ejemplo:
- Comparador de equipos por juego y generación.
- Planificador de objetivos e itinerarios de captura.
- Historial de evoluciones y progreso manual o trazable.
- Recomendaciones basadas en equipo propio con modo offline y justificación.
- Exportación de fichas del equipo y registros personales.
- Presets de layouts por monitor y juego.
- Perfiles de control para 3DS y Switch, mando y ratón.
- Modo streaming sin datos sensibles y con overlays opcionales.
- Privacidad para estadísticas de partida y datos locales.
- Paquetes de contenido versionados y revisables por terceros.
- Asistente de compatibilidad y resolución guiada de errores.
- Visualización de compatibilidad de partidas por edición/actualización.
- Personalización de módulos por título.
- Modo no conectado, sin depender de APIs de terceros.
- Modo accesible con navegación por teclado, lector de pantalla y contraste reforzado.

Para cada nueva idea asigna impacto, complejidad, riesgos, dependencia, evidencia, prioridad y criterio de descarte. No conviertas hipótesis en promesas.

## FASE 3 — ARQUITECTURA IMPLEMENTABLE POR OTRA PERSONA

Propón una arquitectura concreta, compatible con lo observado:

- Mapa de módulos: Rust Windows, Kotlin Android, `gamedata`, adaptadores por motor, perfiles de juego, Companion, almacenamiento y seguridad.
- Diagramas Mermaid: componentes, ciclo de vida de una sesión, flujo de datos, contrato de capacidades, migración y modo degradado.
- Contratos de interfaces y modelos: nombre propuesto, propósito, campos con tipos, invariantes, error codes, serialización, versionado y ownership.
- Eventos del sistema: por ejemplo `GameDetected`, `SessionStarted`, `SessionStopped`, `CapabilityChanged`, `CompanionStateUpdated`, `SaveObserved`, `WindowLayoutChanged`; usar nombres coherentes con el código auditado.
- Políticas de comunicación: IPC local para Windows, flujo Android, permisos mínimos, validación de entradas y autenticación si hay conexión entre dispositivos.
- API de módulos: cómo registrar una nueva consola, juego, perfil, motor, fuente de datos y pantalla, sin añadir condicionales globales por título.
- Catálogo de capacidades por proveedor: **estática**, **manual**, **archivo de guardado**, **API del emulador**, **telemetría en tiempo real**, **no disponible**. Toda UI debe poder mostrar el nivel de confianza y origen.
- Seguridad: lectura de archivos no confiables, limites de tamaño, sandbox/aislamiento cuando aplique, contraseñas/tokens, protección de procesos, logs sin secretos.
- Rendimiento: presupuestos de CPU, RAM, GPU, memoria compartida, frecuencia de polling solo cuando corresponda, caché, FPS del Companion y consumo energético Android.
- Compatibilidad con arquitectura actual: estrategia incremental, feature flags, migraciones reversibles y no-regresión de GB/GBC/GBA/NDS.

**Nivel de precisión obligatorio:** en cada módulo detalla rutas de archivos existentes confirmadas, rutas nuevas **propuestas** claramente etiquetadas, responsabilidades, firmas conceptuales, parámetros, entradas/salidas, estados, errores, validaciones, algoritmos paso a paso en lenguaje natural y puntos de integración. **No escribas implementación ejecutable**.

## FASE 4 — CONTRATOS POR JUEGO (MATRIZ)

Crea una tabla para los títulos 3DS/Switch analizados con columnas:

`Consola | Juego | Edición/versión | Perfil de datos | Team | Bag | Dex | Battle | Map | Guide | Save import | Tiempo real | Origen de información | Riesgos | Nivel de confianza`.

Usa valores explícitos: **Verificado / Requiere investigación / Manual / No aplicable / No disponible**. Nunca pongas “Verificado” por intuición.

Incluye una estrategia de validación por título: corpus de datos de prueba obtenidos legalmente, fixtures sintéticos cuando sea posible, versión, región, idioma, integridad y pruebas de regresión.

## FASE 5 — BACKLOG DETALLADO PARA EL DESARROLLADOR

Produce un backlog de épicas e historias técnicas **atómicas**. Para **cada tarea** usa esta plantilla y rellénala completamente:

- **ID y título** únicos (p. ej. `VYN-3DS-001`, `VYN-SW-001`, `VYN-CORE-001`, `VYN-COMP-001`).
- **Objetivo observable por el usuario** y resultado verificable.
- **Estado real**: existe / parcial / ausente / no auditado.
- **Evidencias**: rutas/símbolos del repositorio auditado o enlaces a documentación consultada.
- **Archivos existentes a revisar y modificar por el desarrollador**, con motivo.
- **Archivos nuevos propuestos**, con lugar, responsabilidad y contenido esperado.
- **Modelos, interfaces, métodos y firmas conceptuales** a implementar; entradas, salidas, precondiciones y invariantes.
- **Algoritmo y flujo paso a paso**, eventos, UI, errores, concurrencia, persistencia y seguridad.
- **Dependencias y bloqueos**, incluidas decisiones y datos externos.
- **Tamaño estimado** en esfuerzo relativo y rol recomendado (sin prometer plazos reales).
- **Criterios de aceptación** en formato Dado/Cuando/Entonces, con casos negativos.
- **Pruebas** unitarias, integración, UI, manuales, regresión y rendimiento; cuáles son ejecutables sin ROMs comerciales.
- **Definition of Done**, documentación y consideraciones de rollback.

Las tareas deben estar **ordenadas en un DAG** con identificadores reales de dependencia. Evita historias gigantes o frases abstractas como “implementar soporte Switch”. Divide cada historia hasta que otra persona pueda programarla sin redescubrir requisitos fundamentales.

**Prioridad sugerida**, que debes confirmar tras auditoría:
1. Catálogo de capacidades y contrato de adaptadores.
2. Biblioteca y perfiles de juegos 3DS/Switch.
3. Companion offline/manual 3DS/Switch sin motor.
4. Integración 3DS validada (Azahar u otra opción justificable).
5. Ventanas y layouts 3DS.
6. Adaptador Switch con motor externo **solo si es viable**; si no, solo Companion.
7. Importación segura y opcional de guardados, cuando sea viable.
8. Datos contextuales por juegos, diagnóstico y UI refinada.
9. Android y experiencia entre dispositivos.
10. Funcionalidades experimentales con gates verificables.

## FASE 6 — CALIDAD, VALIDACIÓN Y DECISIONES

Diseña una estrategia de pruebas para entregar al desarrollador, **sin ejecutar ninguna**:

- Matriz Windows x64, Android, múltiples GPU/resoluciones, monitores y gamepads; distingue requisitos reales de aspiracionales.
- Pruebas de detección con archivos sintéticos, malformados, duplicados y rutas largas; sin distribuir contenido protegido.
- Pruebas de sesiones: cierre, reconexión, cambio de foco, emulador no instalado, permisos, corrupción, versión desconocida y múltiples instancias.
- Pruebas funcionales por módulo y por perfil de juego: datos incorrectos, regiones, idiomas, cambios de versiones, modo manual y ausencia de telemetría.
- Pruebas de persistencia: backup, restauración, colisiones, fallos de escritura, migración y compatibilidad.
- Tests de seguridad, consumo de recursos, accesibilidad, offline y errores recuperables.
- Criterios de Go/No-Go para cada hito, señalización de riesgos y bloqueos no resueltos.

## FORMATO OBLIGATORIO DE ENTREGA (MARKDOWN)

Genera un **documento completo de planificación**, con estas secciones **en este orden**:

1. **Resumen ejecutivo y definición del alcance de este encargo independiente**.
2. **Restricciones: solo planificación, no implementación**.
3. **Auditoría del repositorio con evidencia**.
4. **Mapa de capacidades actuales y brechas**.
5. **Investigación de motores 3DS y Switch, fechas, fuentes, licencias y ADRs**.
6. **Arquitectura objetivo y diagramas**.
7. **Contratos de datos, interfaces, eventos, seguridad y compatibilidad**.
8. **Especificación funcional detallada por consola y por juego**.
9. **Pantallas UI/UX y multiventana con acciones, estados y datos**.
10. **Nuevas funcionalidades propuestas, justificación y viabilidad**.
11. **Backlog completo de épicas y tareas por archivo**.
12. **Dependencias, diagrama de orden de ejecución e hitos MVP/V1/V2/Experimental**.
13. **Plan de QA y criterios de aceptación verificables**.
14. **Riesgos, decisiones pendientes, preguntas bloqueantes y alternativas**.
15. **Checklist de entrega para la persona que implementará**.
16. **Registro final de hechos verificados, hipótesis y cosas que NO se han hecho**.

No me respondas solo con un resumen, brainstorming o sugerencias sueltas. Necesito la profundidad de un **documento de implementación para un desarrollador ajeno a la autoría**, casi función por función, sin pedirle que invente la arquitectura.

### VALIDACIONES FINALES DE TU PROPIA RESPUESTA

Antes de entregar, revisa que:
- No hayas creado ni modificado archivos, ejecutado pruebas, instalado nada ni escrito implementación.
- El plan sea **independiente** de cualquier prompt anterior y trate **el mismo proyecto Vyntra**.
- 3DS y Switch tengan **planes separados**, funciones compartidas y adaptadores distintos.
- No hayas supuesto que hay un core libretro Switch ni garantizado lectura de memoria universal.
- El Companion funcione en modo **offline/manual** incluso sin emulador.
- Se incluyan los juegos Pokémon relevantes por plataforma sin igualar sus mecánicas.
- Todas las pantallas tengan flujos reales especificados, no solo su apariencia.
- Las rutas de archivo observadas se distingan claramente de rutas nuevas propuestas.
- Existan criterios de aceptación claros y tareas con responsables técnicos sugeridos.
- La aplicación existente GB/GBC/GBA/NDS no se haya eliminado del diseño.
- Cada hipótesis importante esté etiquetada y tenga un método de validación posterior.

**Empieza ahora por auditar el repositorio en modo solo lectura y después entrega el plan. No comiences el desarrollo.**
