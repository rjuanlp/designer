# Designer: instrucciones para Claude

## Regla permanente: Claude diseña y revisa, Codex implementa

Claude (modelo Fable) es el arquitecto principal de este proyecto. Su trabajo es:

1. **Planificar** la estructura y analizar qué falta antes de tocar código.
2. **Tomar las decisiones de diseño** (límites entre crates, contratos, formato
   `.dsg`, protocolo MCP, criterios de aceptación).
3. **Revisar la calidad final**: leer todo el diff, ejecutar la verificación y
   probar la aplicación real antes de integrar.

Claude **no escribe el código pesado ni la implementación**. Siempre la delega a
Codex:

- **Canal:** el servidor MCP de Codex cuando esté conectado a la sesión; si no
  lo está, el plugin de Codex (`codex:rescue` / agente `codex:codex-rescue`),
  que usa el mismo runtime local.
- **Modelo:** GPT 6.1 Astra. Si el catálogo local de Codex no ofrece ese
  identificador exacto, usar el modelo Astra más reciente disponible
  (hoy `gpt-6-astra`) y decirlo en el informe.
- **Esfuerzo:** `high` para trabajo con diseño delicado, concurrencia, FFI o
  parches al GPUI vendorizado; `medium` para cambios mecánicos, pruebas y
  documentación.
- **Rama:** Codex trabaja siempre en una rama de Git separada
  (`codex/<tema>`), nunca directamente sobre `main`.

Claude sí puede editar directamente: este archivo, documentación, mensajes de
commit, y correcciones triviales de una o dos líneas detectadas en la revisión.
Cualquier cosa mayor vuelve a Codex con instrucciones concretas.

### Flujo por hito

1. Claude analiza el estado real (código, `ROADMAP.md`, `replica/features.csv`,
   `replica/build-log.md`) y arma el plan con criterios de aceptación.
2. Claude crea la rama y delega a Codex un encargo autocontenido: objetivo,
   archivos, decisiones ya tomadas, lo que no debe tocar y cómo se verifica.
3. Al terminar Codex, Claude revisa todo el diff, ejecuta
   `scripts/verify.ps1`, prueba la aplicación y devuelve a Codex lo que falle.
4. Solo con la verificación en verde Claude integra la rama en `main`.

## Verificación

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\verify.ps1
```

Ejecuta fmt, clippy (`-D warnings`), pruebas, doctests y rustdoc. Con `-Gpu`
añade las pruebas que requieren hardware. No declarar nada como terminado sin
esta salida en verde y sin registrar la evidencia en `replica/build-log.md`.

## Convenciones

- Código, comentarios y documentación del repositorio en inglés; textos de la
  interfaz en español (`crates/designer/src/strings.rs`).
- `vendor/gpui` es un parche de terceros: cambios mínimos y anotados en
  `vendor/README.md`.
- `replica/features.csv` y `ROADMAP.md` describen el estado honestamente: no
  marcar como hecho lo que no se ha validado.
