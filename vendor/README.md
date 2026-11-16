# Vendored third-party source

## gpui

- Upstream: GPUI 0.2.2 as published on crates.io
  (checksum `979b45cfa6ec723b6f42330915a1b3769b930d02b2d505f9697f8ca602bee707`).
  Its `.cargo_vcs_info.json` reports Zed revision
  `69e2130295c2649963eb639fc70b4f2ee8ea1624` with `dirty: true`, so the crates.io
  checksum, not the Git revision alone, identifies the source.
- License: Apache-2.0, retained in `gpui/LICENSE-APACHE`. Designer's MIT license
  does not apply to this directory.
- Wired in through `[patch.crates-io]` in the workspace `Cargo.toml` and excluded
  from the workspace, so Designer's lints and formatting checks do not treat it
  as project code.

### Local modifications

1. **Removed** `examples/` (about 4.8 MB of sample assets) and the matching
   `[[example]]` entries in `Cargo.toml`. No library code depends on them.
2. **Windows external texture compositing.** Upstream 0.2.2 accepts surface
   primitives on Windows but draws nothing for them. The patch adds:
   - `ExternalTexture` and `ExternalFencePoint` in `src/scene.rs`;
   - `Window::paint_external_texture` in `src/window.rs`;
   - `src/platform/windows/external_texture.rs`, which opens shared textures
     and fences on the window's Direct3D 11 device and caches them;
   - an implementation of `draw_surfaces` in
     `src/platform/windows/directx_renderer.rs` that samples the texture with
     the existing polychrome sprite pipeline.

Every changed region is marked with a `DESIGNER PATCH` comment.
`gpui-0.2.2-designer.patch` is the same change as a unified diff against the
pristine crate, excluding item 1.

### Updating

1. Unpack the new upstream crate over `vendor/gpui` and remove `examples/` and
   its manifest entries again.
2. Apply `gpui-0.2.2-designer.patch`, resolve conflicts, and regenerate the
   patch file with `diff -ruN` against the pristine crate.
3. Update the version, checksum and revision above and in
   `THIRD_PARTY_NOTICES.md`.
4. Run `./scripts/verify.ps1 -Gpu` on hardware and the manual checks in
   `docs/gpu-interop.md`.

Remove this directory once upstream GPUI can composite a GPU texture on Windows.
