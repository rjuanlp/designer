# Design tokens

All reusable visual constants live in typed Rust token modules. The initial
feasibility scene defines background, paper, accent, inset, radius, amplitude,
period and output dimensions in `designer_render::tokens`.

The editor will add semantic light/dark colors, type scale, spacing, panel widths,
focus ring and motion durations. Components must not embed raw visual constants.
Geometry values authored by users are document data, not UI theme tokens.
