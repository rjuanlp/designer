# Design tokens

All reusable visual constants live in typed Rust token modules. The initial
feasibility scene defines background, paper, accent, inset, radius, amplitude,
period and output dimensions in `designer_render::tokens`.

The application shell keeps its tokens in `crates/designer/src/theme.rs`:

- `Palette` with `DARK` and `LIGHT`: app, panel, border, text, muted text,
  hover, selected, selected text and focus ring. Unit tests check WCAG contrast
  for text, muted text and selected text in both themes.
- `metrics`: window sizes, the 760 px narrow breakpoint, bar/panel/row sizes,
  a four-step spacing scale, one control radius and a three-step type scale.
- The canvas selection outline is in `designer_render::document::overlay_tokens`.

Components must not embed raw visual constants. Geometry and colors authored by
users, including the sample document, are document data, not UI theme tokens.
Motion durations are not defined yet because the shell has no animation.
