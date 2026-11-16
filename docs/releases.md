# Release procedure

No application release can be produced from the current feasibility milestone.

Before the first release: complete the graphics gate and vertical slice; freeze
the .dsg schema; verify MCP examples; audit dependencies/assets; test a clean
Windows checkout and fork configuration; run the documented hardware benchmark.
Publish measured results rather than an unqualified 60 FPS claim.

The future manual packaging workflow must bundle the three executables, license
texts, required notices, version metadata and SHA-256 checksums. Keep GPU tests
separate from hosted CI. Enable private security reporting before publication.
Do not publish empty binaries or a probe as the finished application.
