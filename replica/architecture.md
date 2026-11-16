# Architecture

## Graphics gate (ADR-001)

GPUI is the only UI framework. Vello is the canvas renderer. The Windows bridge
shares Direct3D 12 textures with GPUI's Direct3D 11 renderer through NT handles,
with retained resource lifetimes and fence-based synchronization. D3D11On12 was
the original plan; shared handles replaced it because GPUI and wgpu link
different `windows` crate versions and cannot exchange COM types (see
[GPU interop](../docs/gpu-interop.md)). A CPU-readback image upload is not an accepted
interactive rendering implementation.

The upstream external compositor is experimental:
https://github.com/zed-industries/zed/discussions/60572

Before S01-S07, validate clipping, alpha, popup occlusion, resize, DPI changes,
minimize/restore, device loss and no per-frame readback. Record an exact GPUI
revision, every local patch, upstream license, and GPU evidence. The bridge
(`designer-gpu-bridge` plus the patch in `vendor/gpui`) passes clipping,
in-window and deferred-element occlusion, translucent content, resize,
minimize/restore and no-readback checks on one machine. DPI changes, device
loss, hybrid-GPU adapter matching and the 60 FPS benchmark are still
unvalidated, so the gate stays open.

## Target process model (ADR-002)

`designer` is the GPUI client; `designer-engine` owns document state/history;
`designer-mcp` adapts rmcp stdio to the engine. Windows named pipes are restricted
to the current user. All clients address the same document instance.

Commands include document ID and expected revision. The engine validates a whole
transaction before committing, publishes deltas, and returns a revision plus
affected IDs. Conflicts never silently overwrite another client's work.
GUI animation evaluates immutable snapshots using the shared pure animation
library; playback does not create history entries. Queues are bounded.

## Current crates

- `designer-core`: typed document tree (pages, nodes, colors, validation) and
  the built-in sample document. No GPUI or GPU dependency.
- `designer-engine`: in-process document sessions, bounded transaction history and
  validated ZIP/JSON `.dsg` persistence. No UI or GPU dependency.
- `designer-render`: turns a page into a display list and a Vello scene. The
  display list is plain data and is tested without a GPU.
- `designer-gpu-bridge`: shared-texture bridge, the only crate with `unsafe`.
- `designer`: the GPUI shell. Interface tokens live in `theme.rs`, every
  user-visible string in `strings.rs` (Spanish, one place to translate), the
  on-demand canvas renderer in `canvas.rs` and the views in `shell.rs`.

The shell renders the canvas only when the page, selection or size changes,
not on every frame. The GUI uses the in-process engine. Multi-client revision
checks, IPC, recovery and `designer-mcp` remain targets.

## Target module boundaries

Document, geometry/layout, commands/history, animation, persistence/import,
rendering, GPUI interface, IPC and MCP remain independent modules. The core does
not depend on GPUI or GPU availability. Unsafe code is forbidden outside
`designer-gpu-bridge`, where every unsafe block must document its invariant.

## Delivery order

S00 graphics gate; S01/S02 shell; core vertical slice; S03; S04/S05; S06/S07;
performance, accessibility, packaging. Documentation follows implementation.
