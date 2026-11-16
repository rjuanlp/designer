# Third-party notices

Original Designer code and documentation use MIT. Dependencies remain under their
own terms; Designer's license does not relicense them.

Direct graphics dependencies:
- GPUI 0.2.2 — Apache-2.0; https://github.com/zed-industries/zed
- Vello 0.12.0 — Apache-2.0 OR MIT; https://github.com/linebender/vello

Before distributing a binary, generate the complete dependency inventory from
Cargo.lock, verify licenses of transitive dependencies and embedded fonts/assets,
and include the required license texts and notices in the package. This initial
list is not a completed binary-distribution audit.

A patched copy of GPUI 0.2.2 is vendored in `vendor/gpui` under its original
Apache-2.0 license (`vendor/gpui/LICENSE-APACHE`). `vendor/README.md` identifies
the upstream source and lists every local modification; changed regions are
marked `DESIGNER PATCH` in the source.
