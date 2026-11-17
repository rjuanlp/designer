# Traspaso: dónde quedó el trabajo y qué sigue

Escrito el 2026-11-17 por Claude (Fable 5.2) a petición del usuario, para que
otra IA o sesión pueda continuar. Leer junto con `CLAUDE.md`, `ROADMAP.md`,
`replica/features.csv` y `replica/build-log.md`.

## Encargo del usuario

"Termina todas esas implementaciones que hacen falta y haz el hito 6."
Instrucciones posteriores del usuario, todas vigentes:

1. Si Codex se queda sin tokens, **Claude implementa directamente**. Codex
   agotó su cuota el 2026-11-17 ~02:46 UTC (decía renovarse a las 4:15 AM).
   Mientras tenga cuota se le sigue delegando (regla de `CLAUDE.md`).
2. **Los botones son iconos, no palabras**; el nombre aparece como tooltip al
   pasar el ratón. Pestañas de navegación, nombres de páginas/capas/estados y
   valores siguen siendo texto (interpretación de Claude, no confirmada).

## Plan en cinco paquetes

| # | Paquete | Estado |
| --- | --- | --- |
| 1 | Editor base: identidad, operaciones nuevas, recorte de marcos, inspector editable, iconos, zoom/pan, flechas, preferencias, recuperación | **en curso** (ver abajo) |
| 2 | Pluma, edición de trazados, claves de morph en la UI, curvas Bézier editables, trazo (`stroke`) | **en curso** |
| 3 | Tipografía (render de texto con parley + fuentes del sistema, herramienta de texto) e imágenes (assets PNG/JPEG dentro del `.dsg`) | sin empezar |
| 4 | Auto layout y restricciones, componentes/variantes/instancias, estilos locales, pantalla S03, cobertura MCP e importación Figma de todo ello | sin empezar |
| 5 | **Hito 6**: benchmark de hardware, empaquetado, onboarding de forks, revisión de release | sin empezar |

Los encargos redactados para Codex (paquete 1 completo y la sección del
inspector) están en el directorio temporal de la sesión y pueden perderse; su
contenido esencial está resumido aquí.

## Paquete 1: estado exacto

Rama: `codex/editor-foundations` (sale de `main` en `f9ab639`).

Hecho y verificado con `scripts/verify.ps1` en verde (commit `31ec11f`):

- `designer_engine::identity` (nombre, directorio de datos, prefijo del pipe,
  `DESIGNER_DATA_DIR`).
- Operaciones `set_corner_radius`, `set_visible`, `set_locked`,
  `reorder_node`, `set_clips_content`, expuestas por MCP.
- `NodeKind::Frame { clips_content }`, recorte en render y hit-test, esquema
  `.dsg` v4 (lee v1 a v3).

Hecho después (commits `77d39d5` y `8421e94`; `scripts/verify.ps1` completo
en verde sobre `8421e94`):

- Inspector editable (`inspector.rs`, `shell/inspector_view.rs`,
  `text_field.rs`, `text_field/view.rs`): nombre, X, Y, ancho, alto, radio,
  opacidad, relleno (`#RRGGBB[AA]`, `#` opcional), visible, bloqueada, recortar.
  Probado en la app real: escribir ancho + Enter redimensiona, valor inválido
  se revierte con mensaje, pegar color funciona, Ctrl+Z deshace.
- Overrides por estado de prototipo desde el inspector (pruebas unitarias; **no
  probado en la app**).
- Iconos (`icons.rs`, 46 SVG propios servidos por `AssetSource`), `icon_button`
  y `command` con tooltip en `shell.rs`. Vistos en captura: barra de
  herramientas, capas, inspector y línea de tiempo.
- Lista de capas en una línea con ojo/candado; subir/bajar en la cabecera.
- Zoom y desplazamiento (`canvas.rs`: `zoom_by`, `zoom_to`, `pan`, `fit`;
  `shell.rs`: Ctrl+rueda, rueda, botón central, Espacio+arrastre, Ctrl+0/1/+/-,
  controles en la barra y en el árbol de accesibilidad). Probado en la app:
  Ctrl++ y "Alejar" cambian el porcentaje. Exactitud probada: a 195 % un
  arrastre de 250×125 px crea un rectángulo de 128×64 unidades. **Sin
  probar**: Ctrl+rueda, pan con botón central y Espacio+arrastre, mover a
  zoom distinto de 100 %, pantalla estrecha.
- Tab confirma los campos del inspector (blur diferido con `window.defer`);
  probado con Ancho=77 + Tab. Tras "Ajustar" la etiqueta de zoom muestra el
  porcentaje correcto (probado: 100 %).

Cambios sin commit posteriores a `8421e94`:

- Preferencias versionadas en `<data_dir>/preferences.json`, con tema, estado
  de la línea de tiempo, tamaño de ventana y recientes conectados al shell.
  La pantalla Documentos ahora muestra los recientes; Preferencias muestra
  el pipe de sesión y los comandos de `designer-mcp`.
- Up/Down/Home/End navega capas, recientes, estados del prototipo y secuencias/
  claves de timeline después de seleccionar la lista. Left/Right cambia de
  pestaña. Falta validar estos recorridos en la GUI.
- `RecoveryStore` está conectado a la ventana: snapshots al detectar dirty,
  como máximo cada 30 s; limpieza al guardar, descartar/cerrar o reemplazar;
  prompts explícitos al arrancar. La GUI todavía requiere prueba manual.
- Aviso `isMask` corregido y documentación de build/MCP/changelog actualizada.
- `scripts/verify.ps1` pasó tras los cambios de recuperación, preferencias,
  recientes, agentes y navegación de listas: 141 pruebas pasaron, 3 de
  hardware quedaron ignoradas;
  fmt, clippy, doctests y rustdoc pasaron. La GUI necesita validación manual.

Problemas abiertos conocidos:

1. ~~Tab no confirma~~ resuelto en `8421e94`.
2. ~~Zoom "—" tras ajustar~~ resuelto en `8421e94`.
3. `[ERROR] RefCell already borrowed` aparece una vez al arrancar. **Ya ocurre
   en `main`** (comprobado con un binario de `main`), no es de esta rama. Sin
   diagnosticar; sospecha: reentrada de mensajes de Windows durante el primer
   render (`set_window_title`).
4. El texto de `isMask` ya se corrigió para indicar que las máscaras no se
   importan, pero el recorte de contenido sí se respeta.
5. La documentación y el changelog se han actualizado para reflejar iconos,
   zoom/pan, y las operaciones nuevas.

Falta del paquete 1 (decisiones ya tomadas):

- **Flechas**: Arriba/Abajo/Inicio/Fin en documentos recientes, estados y
  filas de la línea de tiempo. Arriba/Abajo en capas e Izquierda/Derecha en
  pestañas ya están conectadas, pendiente validarlas en la GUI.
- **Preferencias pendientes**: conectar la lista de documentos recientes a la
  pantalla de Documentos y sustituir el texto temporal de Agentes por el pipe
  y los comandos reales de `designer-mcp`.
- **Recuperación**: el motor y la shell ya escriben snapshots periódicos de
  sesiones sucias, limpian al guardar/cerrar/reemplazar y presentan decisiones
  explícitas al arrancar. La GUI todavía no se ha probado; los snapshots
  inválidos se conservan si no se elige descartarlos.
- **Validación GUI**: probar Ctrl+rueda, pan con botón central y con
  Espacio+arrastre, overrides de estado del inspector, layout estrecho
  (390 px), tema claro con iconos, mover a zoom distinto de 100 %, teclas
  muertas/IME en los campos, flechas y el flujo de recuperación.
- Después: `scripts/verify.ps1` en verde, capturas a 1440 y 390 px en
  `replica/clone-screens/`, fila en `replica/build-log.md`, actualizar
  `features.csv`/`ROADMAP.md` con honestidad y fusionar en `main`.

## Paquete 2: primer corte

- `Operation::SetPath` reemplaza la geometría normalizada de un nodo Path sin
  cambiar sus bounds. La validación transaccional conserva la atomicidad,
  rechaza otros tipos de nodo y evita cambios de topología incompatibles con
  claves morph existentes. El discriminador aparece en la lista de capacidades
  usada por MCP.
- `cargo test --locked -p designer-core`: 27 pruebas pasaron.
- `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify.ps1`:
  pasó; 142 pruebas, 3 pruebas GPU ignoradas por requerir hardware; fmt,
  clippy, doctests y rustdoc pasaron.
- Sigue pendiente: herramienta pluma y edición en canvas, conversión de formas,
  `Node.stroke` y renderizado de trazos, autoría UI de claves morph y edición
  directa de curvas Bézier. No se ha hecho validación GUI de este paquete.

## Decisiones de diseño para los paquetes siguientes

- **2**: `Node.stroke: Option<{color,width}>`; `SetPath` y `ConvertToPath`;
  pluma con clic (línea) y arrastre (cúbica); con claves de morph se bloquean
  los cambios de topología; `Easing::CubicBezier{x1,y1,x2,y2}` editable
  arrastrando dos manejadores.
- **3**: `Text { content, style }` (familia, tamaño, peso, cursiva,
  interlineado, tracking, alineación, modo de tamaño); medición fuera de
  `designer-core` y límites resultantes guardados en la operación; imágenes
  como entradas `assets/<sha256>.<ext>` del ZIP, solo PNG/JPEG, con límites.
- **4**: los componentes son nodos normales en las páginas (`Component`,
  `ComponentSet`) y las instancias los referencian con overrides por id de
  nodo origen, para reutilizar todas las operaciones existentes; auto layout
  resuelto dentro de la transacción (los límites guardados ya están
  resueltos); estilos locales a nivel de documento.
- **5 (hito 6)**: benchmark por la ruta real de composición con escenas de
  100/1.000/5.000 nodos, publicando media/p95/p99/máximo y fotogramas
  perdidos con adaptador y driver, sin afirmar "60 FPS" a secas;
  `scripts/package.ps1` (zip con ejecutables, licencias, avisos de terceros,
  SHA-256, sin consola en release); guía de fork basada en `identity`; revisión
  de release sin publicar ni etiquetar nada.

## Cómo trabajar aquí

- Verificación: `powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify.ps1`.
- Codex: `codex-companion.mjs task --background --write --fresh --model gpt-6-astra --effort high --prompt-file <archivo>`.
  Lecciones de esta sesión: lanzarlo siempre con `--background` (en primer
  plano se corta a los 10 minutos), darle **una sección por encargo** (con
  encargos grandes devuelve trabajo parcial), y avisarle de que su sandbox no
  puede abrir ventanas. No hace commits; los hace Claude.
- Probar la app: compilar `cargo build --locked -p designer`, lanzar
  `target\debug\designer.exe --size 1440x900 --select 3` con
  `DESIGNER_DATA_DIR` apuntando a una carpeta temporal, y manejarla con UI
  Automation (`System.Windows.Automation`) y capturas de la ventana.
- Queda un worktree temporal de `main` registrado en Git dentro del directorio
  temporal de la sesión; `git worktree prune` lo limpia si la carpeta ya no
  existe.
