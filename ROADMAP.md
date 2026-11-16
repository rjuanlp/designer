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
same-topology morph evaluation are also implemented, but path editing,
morph-key authoring and direct curve editing remain pending in the UI. Hito 4
and roadmap item 4 therefore remain partial; interactive states are tracked in
roadmap item 5. See the build log for validation.
