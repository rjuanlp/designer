# Building Designer

## Windows prerequisites

- Windows x64 and a compute-capable GPU for the graphics probe.
- Visual Studio 2022 Build Tools: Desktop development with C++, MSVC and Windows SDK.
- Rust MSVC toolchain, including `rustfmt` and `clippy`.
- Git and enough disk space for Rust, SDKs and GPU dependencies.

After installing the prerequisites, open a fresh PowerShell terminal at the
repository root. Do not rely on a developer's absolute paths.

```powershell
rustup show
rustc --version
cargo --version
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
cargo test --workspace --doc --locked
$env:RUSTDOCFLAGS = '-D warnings'
cargo doc --workspace --no-deps --locked
```

`Cargo.lock` and `rust-toolchain.toml` are checked in and every command uses
`--locked`. There is no installable application or file association yet.

## Troubleshooting

- `link.exe` missing: install the C++ workload, not only the Visual Studio installer.
- `cargo` missing: reopen the terminal after rustup installation.
- Package downloads fail: check network/proxy access to crates.io and static.rust-lang.org.
- GPU unavailable: record the adapter and driver; do not claim a GPU test passed.

GPU validation must run on real hardware. Unit tests alone cannot establish
composition correctness, accessibility, or frame pacing.

## Feasibility probes

```powershell
cargo run --locked -p designer-render --example gpu_probe
cargo run --locked -p designer-render --example gpui_probe
```

The first probe submits 120 offscreen Vello frames on a hardware DX12 adapter.
It is not a frame-rate benchmark. The second opens a GPUI window but does not
composite Vello. Neither alone satisfies the integrated graphics gate.

Run the complete checks with `./scripts/verify.ps1`. Add `-Gpu` only on a machine
with the required hardware. The ordinary CI job does not run a GPU smoke test.
