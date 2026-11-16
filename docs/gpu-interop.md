# Windows GPU interop investigation

Status: unresolved implementation gate, not a successful compositor.

## Reproducible source evidence

Inspected the published gpui 0.2.2 crate pinned by Cargo.lock. Its VCS metadata
reports revision 69e2130295c2649963eb639fc70b4f2ee8ea1624 with dirty=true;
therefore the crates.io package checksum, not that Git SHA alone, identifies the
exact source used here.

- src/platform/windows/directx_renderer.rs: draw_surfaces returns Ok without drawing.
- src/window.rs: paint_surface accepts a macOS CVPixelBuffer.
- src/elements/surface.rs: the native surface element is macOS-specific.

The existing image path is not an acceptable replacement if it requires a CPU
readback per animation frame.

## Interop constraints found in the pinned sources

- gpui 0.2.2 builds against `windows` 0.61 and wgpu-hal 30.0.2 against
  `windows` 0.62. COM interface types cannot cross that boundary; the bridge
  must exchange an NT shared handle or raw pointers.
- wgpu 30.0.2 exposes the needed DX12 escape hatches (`Device::as_hal`,
  `raw_device`, `raw_queue`, `texture_from_raw`, `create_texture_from_hal`).
  All are `unsafe` and belong in the isolated bridge crate.
- GPUI selects the first DXGI adapter that supports Direct3D 11, while the
  Vello probe requests the high-performance adapter. On hybrid-GPU machines
  these can differ, so the bridge must match adapters by LUID.

## Required bridge work

1. Define an owned Windows GPU-surface primitive with format, size, adapter
   identity, content generation and a resource lifetime lease.
2. Establish one DX12 device/queue for Vello and a compatible D3D11On12 path for
   GPUI. Explicitly match adapter identity.
3. Wrap/share the output texture, acquire/release resources in the correct state,
   order writes before reads and retain them until GPU completion.
4. Sample the texture in GPUI primitive order with clipping, premultiplied-alpha
   semantics and overlay occlusion. No second swapchain on the same HWND.
5. Invalidate cached wrappers on resize/device generation changes. Avoid blocking
   the UI thread on unbounded GPU waits.
6. Test repeated resize, clipping, DPI, menus, minimization, device recreation,
   resource cleanup and long-running rendering on hardware.

This work must be implemented and tested before the document editor expands.
No vendored or modified GPUI source is currently included.

## Evidence requirements

Record adapter/driver, toolchain, package checksums, integrated screenshots and
CPU/GPU timings. An offscreen Vello dispatch and a separate GPUI window are useful
smoke tests but do not satisfy this gate. The target 10,000-node benchmark remains
unimplemented and unmeasured.

