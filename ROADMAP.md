# Roadmap

1. Reproducible Rust/MSVC build and GPUI–Vello graphics feasibility. Works on
   one machine; DPI change, device loss and hybrid-GPU checks remain open.
2. **Active:** native shell (read-only structure done) and the
   create/draw/animate/save/reopen/MCP/undo vertical slice.
3. Vector tools, typography, layout, components and local styles.
4. Advanced animation, nested sequences and morphing.
5. Interactive states, structured Figma migration, accessibility and complete
   MCP capability coverage.
6. Hardware benchmark, packaging, fork onboarding and release review.

These are milestones, not completed capabilities or promised dates. See
[features](replica/features.csv) for current status. No cloud, developer mode,
code generation, animation export or binary .fig compatibility is planned.

Implementation checkpoint **hito 3** (separate from roadmap item 3): basic
create/draw/save/reopen/undo is implemented. Text, animation UI and MCP in the
full vertical slice remain pending. See the build log for validation.

Implementation checkpoint **hito 4**: keyframe editing, playback, preset
easing curves and nested sequences are implemented. Core path rendering and
same-topology morph evaluation are also implemented, and the core exposes an
atomic `SetPath` operation that preserves topology when morph keys exist.
Canvas path editing, morph-key authoring and direct curve editing remain
pending in the UI. Hito 4 and roadmap item 4 therefore remain partial;
interactive states are tracked in roadmap item 5. See the build log for
validation.

Implementation checkpoint **hito 5** on `codex/hito-5-finish` adds Windows
live GUI/MCP sharing, OS sidecar locks, capability discovery and AccessKit
semantics for every screen. Automated contracts cover session locking, the
real MCP binary over a test pipe, prototype transactions and accessibility
tree integrity. The running app was checked with a UI Automation client and the
real MCP binary (see the build log). Roadmap item 5 stays partial: screen-reader
speech (Narrator/NVDA), per-control accessibility rectangles, arrow-key list
navigation, non-Windows live transport and MCP coverage of model areas that do
not exist yet (components, styles, assets, layout) remain open.
