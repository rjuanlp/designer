# Build log

| Date | Surface | Status | Evidence / missing work |
| --- | --- | --- | --- |
| 2026-11-16 | S00 | partial | Workspace and deterministic Vello scene authored. Rust/MSVC installation started. GPU bridge, visual capture and 60 FPS benchmark remain unverified. |
| 2026-11-16 | S00 | partial | Rust 1.100.0/MSVC installed. Fixed `gpu_probe` for wgpu 30 (`apply_limit_buckets`); probe-only crates moved to dev-dependencies. `scripts/verify.ps1` passes locally (fmt, clippy, 5 tests, rustdoc). `gpu_probe` completed 120 offscreen frames on NVIDIA GeForce RTX 4050 Laptop GPU, DX12, driver 32.0.16.2124. `gpui_probe` window opens and responds. Bridge, integrated capture and 60 FPS benchmark still missing; CI not yet run. Known: future-incompat warning in transitive `proc-macro-error2` 2.0.1. |
| 2026-11-16 | S00 | partial | Added `designer-gpu-bridge` and vendored GPUI 0.2.2 with an external-texture patch. `composited_probe` shows Vello inside GPUI with clipping and an overlapping GPUI panel (`replica/clone-screens/S00.png`); 600 frames and a resize/minimize stress run completed with 0 skipped frames. Harder than expected: Direct3D 11 refuses the shared texture unless it also has the render-target flag. Missing: translucent canvas, popup occlusion, DPI change, device loss, hybrid-GPU adapter matching, release pacing, 60 FPS benchmark. |
| 2026-11-16 | Repository | partial | MIT license, English architecture/build/contribution/fork documentation and honest feature matrix established. |
