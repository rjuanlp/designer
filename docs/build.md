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

## Running the application shell

```powershell
cargo run --locked -p designer
```

The application opens the sample document. Ctrl+N creates an empty document;
Frame, Rectangle and Ellipse draw shapes by clicking or dragging. Move selects
and drags layers. Delete removes the selection. Inspector values remain read-only. A debug build
takes several seconds to show its first frame while the GPU device and Vello
shaders are prepared; the window is empty until then.

Launch options set the initial state, mainly for screenshots:

| Option | Meaning |
| --- | --- |
| `--size 1440x900` | Window size in logical pixels, at least 390x560. |
| `--screen NAME` | `documents`, `design`, `components`, `preview`, `import` or `preferences`. |
| `--page N` | Zero-based page index. |
| `--select ID` | Node id to select on that page. |
| `--panel NAME` | `layers`, `canvas` or `inspector`; used below 760 logical pixels of width. |
| `--timeline` | Start with the timeline expanded. |
| `--light` | Start with the light theme. |
| `--open FILE.dsg` | Open a local document instead of the sample. |

Keyboard: Tab and Shift+Tab move focus, Enter or Space activates the focused
control. Ctrl+S saves, Ctrl+Shift+S saves as, Ctrl+O opens, Ctrl+Z undoes,
Ctrl+Y or Ctrl+Shift+Z redoes, Escape cancels a drag. Ctrl+T toggles the timeline and Ctrl+Shift+L switches theme.

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
cargo run --locked -p designer-gpu-bridge --example composited_probe
```

The first probe submits 120 offscreen Vello frames on a hardware DX12 adapter.
The second opens a plain GPUI window. The third composites animated Vello
content inside a GPUI window through shared GPU textures; pass
`-- --frames 600` to close automatically and print pacing figures for the probe
scene, or `-- --translucent` for the alpha-blending check. None of them is the project benchmark. See
[GPU interop](gpu-interop.md) for what the integrated probe does and does not
validate.

Run the complete checks with `./scripts/verify.ps1`. Add `-Gpu` only on a machine
with the required hardware; it also runs the bridge contract tests and opens
the composited probe window briefly. The ordinary CI job does not run a GPU smoke test.
