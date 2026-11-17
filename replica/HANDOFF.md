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
| 2 | Pluma, edición de trazados, claves de morph en la UI, curvas Bézier editables, trazo (`stroke`) | sin empezar |
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

Hecho después (segundo commit de la rama; clippy y pruebas de `designer` en
verde, **`scripts/verify.ps1` completo NO se ha vuelto a ejecutar**):

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
  Ctrl++ y "Alejar" cambian el porcentaje. **Sin probar**: rueda, pan, que un
  arrastre a 200 % mueva exactamente las unidades esperadas, pantalla estrecha.

Problemas abiertos conocidos:

1. **Tab en un campo del inspector puede no confirmar el valor.** Enter sí
   confirma. Se cambió `FieldView::sync` para no descartar texto pendiente,
   pero la última prueba (escribir 77 en Ancho + Tab) no quedó concluyente
   porque la selección había cambiado antes. Repetir la prueba con la capa
   seleccionada y revisar `on_blur` en `text_field/view.rs`.
2. Tras "Ajustar al contenido" la etiqueta de zoom mostraba "—"; se añadió
   `cx.on_next_frame` en `zoom_fit` y **no se ha vuelto a comprobar**.
3. `[ERROR] RefCell already borrowed` aparece una vez al arrancar. **Ya ocurre
   en `main`** (comprobado con un binario de `main`), no es de esta rama. Sin
   diagnosticar; sospecha: reentrada de mensajes de Windows durante el primer
   render (`set_window_title`).
4. En `import.rs` el aviso de `isMask` aún dice "El recorte de contenido y las
   máscaras no se importan"; el recorte ya se importa, corregir el texto.
5. `docs/build.md` y `CHANGELOG.md` no describen todavía iconos ni zoom.

Falta del paquete 1 (decisiones ya tomadas):

- **Flechas**: Arriba/Abajo/Inicio/Fin en capas, documentos recientes, estados
  y filas de la línea de tiempo; Izquierda/Derecha en pestañas.
- **Preferencias**: `<data_dir>/preferences.json` versionado, escritura
  atómica, tema, línea de tiempo abierta, tamaño de ventana, 10 recientes;
  archivo corrupto vuelve a valores por defecto; sección "agentes" real con
  el nombre del pipe y los comandos de `designer-mcp`.
- **Recuperación**: autoguardado de sesiones sucias en
  `<data_dir>/recovery/<id>.dsg` + sidecar JSON, máximo cada 30 s, borrado al
  guardar o cerrar bien, aviso "Recuperar / Descartar" al arrancar, nunca
  borrar en silencio un archivo que no valida.
- Después: `scripts/verify.ps1` en verde, capturas a 1440 y 390 px en
  `replica/clone-screens/`, fila en `replica/build-log.md`, actualizar
  `features.csv`/`ROADMAP.md` con honestidad y fusionar en `main`.

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
