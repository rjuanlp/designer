# Changelog

## Unreleased

- Establish an MIT-licensed Rust workspace and English project documentation.
- Add deterministic Vello feasibility-scene construction and input tests.
- Record the required graphics gate and explicitly unimplemented editor features.
- Add `designer-gpu-bridge`: shared Direct3D 12 render targets with fence-based
  hand-over, and a probe that composites Vello inside a GPUI window.
- Vendor GPUI 0.2.2 with a Windows external-texture patch (`vendor/README.md`).
- Verify translucent canvas content and deferred GPUI elements over the canvas.
- Add `designer-core`: typed document tree, validation and a sample document.
- Add page display lists and Vello scenes to `designer-render`.
- Add the `designer` application shell: navigation, read-only layers, canvas
  and properties, timeline placeholder, light/dark themes, narrow layout and
  keyboard focus. It cannot create, edit, save or open documents.

There is no released editor binary.
