# Build log

| Date | Surface | Status | Evidence / missing work |
| --- | --- | --- | --- |
| 2026-11-16 | S00 | partial | Workspace and deterministic Vello scene authored. Rust/MSVC installation started. GPU bridge, visual capture and 60 FPS benchmark remain unverified. |
| 2026-11-16 | S00 | partial | Rust 1.100.0/MSVC installed. Fixed `gpu_probe` for wgpu 30 (`apply_limit_buckets`); probe-only crates moved to dev-dependencies. `scripts/verify.ps1` passes locally (fmt, clippy, 5 tests, rustdoc). `gpu_probe` completed 120 offscreen frames on NVIDIA GeForce RTX 4050 Laptop GPU, DX12, driver 32.0.16.2124. `gpui_probe` window opens and responds. Bridge, integrated capture and 60 FPS benchmark still missing; CI not yet run. Known: future-incompat warning in transitive `proc-macro-error2` 2.0.1. |
| 2026-11-16 | Repository | partial | MIT license, English architecture/build/contribution/fork documentation and honest feature matrix established. |
