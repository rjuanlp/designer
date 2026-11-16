# Windows GPU interop

Status: **the bridge composites Vello output inside a GPUI window on one
tested machine. The graphics gate is partially validated, not closed.** See
[What is not validated](#what-is-not-validated).

## Why a bridge is needed

In the published gpui 0.2.2 crate:

- `src/platform/windows/directx_renderer.rs`: `draw_surfaces` returns `Ok`
  without drawing.
- `src/window.rs`: `paint_surface` accepts only a macOS `CVPixelBuffer`.
- `src/elements/surface.rs`: the native surface element is macOS-specific.

Uploading an image per frame would require a GPU-to-CPU readback of the Vello
target, which is not an accepted interactive rendering path.

## Design

```text
Vello (wgpu, Direct3D 12)                 GPUI (Direct3D 11)
  render into shared texture  ──────────►  open by NT handle, sample in
  signal "ready" fence                     scene order, clipped
  poll "released" fence       ◄──────────  wait "ready", signal "released"
```

- **`designer-gpu-bridge`** creates a ring of three textures in shared
  Direct3D 12 heaps, wraps them as wgpu textures and exposes their NT handles.
  It is the only crate that may contain `unsafe`; each block documents its
  invariant.
- **The GPUI patch** (see [`vendor/README.md`](../vendor/README.md)) adds
  `Window::paint_external_texture`. The Windows renderer opens the handle with
  `ID3D11Device1::OpenSharedResource1` and draws it with the existing
  polychrome sprite pipeline, so it obeys paint order and the content mask.
  Bounds are snapped to whole device pixels so texels map one to one.
- **No CPU readback and no second swap chain.** Pixel data never leaves the GPU.

### Why shared handles instead of D3D11On12

gpui 0.2.2 links `windows` 0.61 and wgpu-hal 30.0.2 links `windows` 0.62, so
COM interface types cannot be passed between them. An NT handle is a plain
integer and crosses that boundary. It also leaves GPUI's device creation
untouched, which keeps the patch small.

### Texture requirements

The shared texture is `R8G8B8A8_UNORM` with unordered access (Vello writes it
from a compute shader), **render target** and simultaneous access. On the
tested driver, Direct3D 11 rejects the handle with `E_INVALIDARG` when the
render-target flag is missing, even though nothing renders to it as a target.

### Alpha

Vello writes straight (non-premultiplied) alpha into the target, which is what
GPUI's sprite blend (`SRC_ALPHA` / `INV_SRC_ALPHA`) expects. This was measured,
not assumed: with a premultiplied blend a 50% white band saturated to pure
white; with the sprite blend it matches the expected value (see Evidence).

### Synchronization

Two shared fences, both created on the Direct3D 12 device:

- **ready**: wgpu signals generation *n* right after the command lists of the
  frame's submission (`add_signal_fence`). GPUI queues
  `ID3D11DeviceContext4::Wait` before sampling. This is a GPU-side wait; the UI
  thread does not block.
- **released**: GPUI queues `Signal` with generation *n* after its draw call.
  The bridge only polls the completed value.

A target is reused only when it is not the latest frame and GPUI has completed
drawing a newer generation. If no target is free (for example nothing is being
presented), `SharedCanvas::render` returns `None` without rendering. Work is
therefore bounded and never blocks on the compositor.

### Lifetimes

GPUI keeps its own references to opened textures and fences, keyed by
process-unique ids rather than handle values, and releases them after 120
frames without use or on device loss. On resize the caller creates a new
`SharedCanvas` and keeps the old one alive for a few frames so that frames
already handed over can still be opened.

## Evidence

Machine: NVIDIA GeForce RTX 4050 Laptop GPU, driver 32.0.16.2124, the only
display adapter enabled; Windows 11; Rust 1.100.0. Debug build unless a row
says release.

| Check | Result |
| --- | --- |
| Vello content visible inside the GPUI window | Yes, [`S00.png`](../replica/clone-screens/S00.png) |
| GPUI element painted above the canvas, translucent | Yes, same capture |
| Canvas clipped by an `overflow_hidden` parent | Yes, same capture |
| 600 frames, fixed size | 600 rendered, 0 skipped, no renderer errors |
| 40 programmatic resizes, then 3 minimize/restore cycles | 970 rendered, 0 skipped, 42 canvases created, window responsive, no renderer errors while open |
| Backpressure without a compositor (`gpu_contract` test) | Stops after the unused targets, no blocking |
| Translucent canvas over a GPUI backdrop `(31,111,92)` | Transparent band `(31,111,92)`, 50% white band `(143,183,174)`, opaque band `(105,87,235)`: all equal the expected values, [`S00-translucent.png`](../replica/clone-screens/S00-translucent.png) |
| GPUI `deferred` element (the pass used by menus and tooltips) above the canvas | Yes, same capture |
| Release build, 3,000 frames | 0 skipped; frame interval mean 6.96 ms, p95 8.42 ms, p99 9.18 ms; UI-thread canvas work mean 1.36 ms, p95 2.23 ms |
| Release build, 4 minute run, 28,025 frames | Private memory 636 MB at start, 649 to 651 MB from 30 s on; handles 641 to 667; always responsive. One 27.3 s gap in painting with 24 skipped frames, then normal pacing (see below) |

Pacing of the probe scene in the debug fixed-size run: frame interval mean
9.44 ms, p95 12.65 ms, p99 13.73 ms; UI-thread canvas work p95 10.09 ms. The
release intervals follow the display refresh rate. All figures are for two
shapes; they say nothing about the 10,000-node target, which remains
unimplemented and unmeasured.

Reproduce with:

```powershell
cargo run --locked -p designer-gpu-bridge --example composited_probe -- --frames 600
cargo run --locked -p designer-gpu-bridge --example composited_probe -- --translucent
cargo run --release --locked -p designer-gpu-bridge --example composited_probe -- --frames 3000
cargo test --locked -p designer-gpu-bridge -- --ignored
```

## What is not validated

- **The 27.3 s painting gap in the 4 minute run.** The window was not in the
  foreground and the machine was in use, so occlusion or display state is a
  plausible cause, but it was not reproduced or diagnosed. The bridge skipped
  24 frames instead of blocking and recovered on its own.
- **Hybrid-GPU machines.** GPUI picks the first Direct3D 11 capable adapter and
  the probe requests the high-performance one. If they differ, opening the
  handle fails and the surface is skipped with a logged error.
  `SharedCanvas::adapter_luid` exists, but nothing matches adapters yet.
- **Device loss.** The patch clears its cache on GPUI device recreation, and
  the bridge ignores the removed-device fence value, but no loss was provoked.
- **DPI changes** and moving between monitors.
- **Real popup menus and tooltips.** An in-window element and a `deferred`
  element were tested, not GPUI's menu or tooltip components.
- **Stability over hours.** The longest run is 4 minutes.
- **The 60 FPS benchmark** on the 10,000-node scene.
- **Other drivers and vendors** (AMD, Intel) and older Windows versions. Shared
  fences need Windows 10 1703 or later.
## Known upstream noise

When the window is closed by the system, GPUI 0.2.2 logs `window not found`
and two invalid-window-handle errors (`0x80040102`, `0x80070578`). They come
from unpatched upstream code: `src/platform/windows/window.rs` revokes drag and
drop and destroys a window that is already destroyed, and a pending frame
callback targets the closed window. They appear after the probe's final report
and do not involve the bridge.
