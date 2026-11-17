# Changelog

## Unreleased

- Add a pure single-line editing state with UTF-16 replacement ranges,
  composition replacement, selection, deletion, commit and cancellation tests.
  Connect it to a GPUI input handler and editable inspector, with clipboard,
  composition, commit/cancel, validation and session undo support.
- Add per-user preferences persistence (`<data_dir>/preferences.json`), a
  versioned JSON schema, recent-document tracking and automatic crash recovery
  under `<data_dir>/recovery/`, with startup recovery/discard prompts and
  periodic snapshots for dirty documents.
- Connect recent documents to the Documents screen and show the live MCP pipe
  and client command forms in Preferences.
- Add layer visibility/lock/order controls, locked-selection deletion protection,
  prototype-state property overrides and reset controls, and AccessKit SetValue.
  Graphical IME and screen-reader validation remains pending.
- Extend the operation serialization round-trip fixture to cover all five
  new editor operations.
- Centralize application identity and per-user data-directory resolution.
- Add undoable corner-radius, visibility, locking, sibling-order and frame-clip
  operations, including MCP capabilities and transaction support.
- Write DSG schema 4, retaining readers for versions 1 to 3 and defaulting
  legacy frames to unclipped. Render ancestor frame clips and honor them in hits.
- Honor Figma frame clipping and enable clipping for newly drawn frames.
- Add screen/document transform round trips, bounded pointer-anchor zoom math,
  interactive zoom/pan, arrow navigation across layers, recent documents,
  prototype states and timeline sequence/keyframe rows, per-user preferences
  and crash recovery snapshots.

- Add Windows live MCP attachment with `--connect`/`--pipe`, capability discovery,
  bounded local pipe clients and drag/prompt mutation rejection.
- Hold OS advisory sidecar locks for file-backed sessions, including headless MCP.
- Extract a pure AccessKit tree builder, add all-screen semantics, focus actions
  and unchanged-tree caching. Manual screen-reader validation remains pending.

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
  keyboard focus.

- Add in-process sessions with atomic commands and bounded undo/redo history.
- Add validated `.dsg` save/open and native file dialogs with unsaved-change prompts.
- Add frame/rectangle/ellipse drawing, hit testing, moving and deleting layers.
- Keep the status row fixed so command messages do not shift the canvas during drags.
- Add basic timeline playback, keyframe editing, duration controls, and
  linear/constant interpolation with undo/redo and `.dsg` persistence.
- Add ease-in, ease-out and ease-in-out interpolation presets with a timeline
  curve preview.
- Add nested animation sequences with local clocks, one-shot playback, and
  held end states.
- Add normalized line/cubic path geometry, rendering, hit testing, and
  same-topology morph keyframes on page and sequence timelines.
- Write the updated `.dsg` path/morph schema as version 2 while continuing to
  read version 1 documents.
- Write the interactive prototype-state model as `.dsg` schema version 3 while
  retaining readers for versions 1 and 2.
- Add hover/click prototype states and a transition preview in S05.
- Add a structured Figma JSON importer with explicit loss reports and safe
  handling of unsupported or over-limit content.
- Add a headless `designer-mcp` stdio server with revision-checked atomic
  transactions, save/import/history tools and evaluated still snapshots.

There is no released editor binary.
