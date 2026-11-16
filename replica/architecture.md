# Architecture

## Graphics gate (ADR-001)

GPUI is the only UI framework. Vello is the canvas renderer. The Windows bridge
is planned as DX12/D3D11On12 texture composition, with retained resource lifetimes
and explicit synchronization. A CPU-readback image upload is not an accepted
interactive rendering implementation.

The upstream external compositor is experimental:
https://github.com/zed-industries/zed/discussions/60572

Before S01-S07, validate clipping, alpha, popup occlusion, resize, DPI changes,
minimize/restore, device loss and no per-frame readback. Record an exact GPUI
revision, every local patch, upstream license, and GPU evidence. The current
repository does not contain that bridge or a claim that it works.

## Target process model (ADR-002)

`designer` is the GPUI client; `designer-engine` owns document state/history;
`designer-mcp` adapts rmcp stdio to the engine. Windows named pipes are restricted
to the current user. All clients address the same document instance.

Commands include document ID and expected revision. The engine validates a whole
transaction before committing, publishes deltas, and returns a revision plus
affected IDs. Conflicts never silently overwrite another client's work.
GUI animation evaluates immutable snapshots using the shared pure animation
library; playback does not create history entries. Queues are bounded.

## Target module boundaries

Document, geometry/layout, commands/history, animation, persistence/import,
rendering, GPUI interface, IPC and MCP remain independent modules. The core does
not depend on GPUI or GPU availability. Unsafe code is forbidden outside a
future isolated native graphics bridge whose invariants must be documented.

## Delivery order

S00 graphics gate; S01/S02 shell; core vertical slice; S03; S04/S05; S06/S07;
performance, accessibility, packaging. Documentation follows implementation.
