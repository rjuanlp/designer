param([switch]$Gpu)
$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    $commands = @(
        @('fmt', '--all', '--', '--check'),
        @('clippy', '--workspace', '--all-targets', '--all-features', '--locked', '--', '-D', 'warnings'),
        @('test', '--workspace', '--all-targets', '--all-features', '--locked'),
        @('test', '--workspace', '--doc', '--locked'),
        @('doc', '--workspace', '--no-deps', '--locked')
    )
    $previousRustdocFlags = $env:RUSTDOCFLAGS
    try {
        $env:RUSTDOCFLAGS = '-D warnings'
        foreach ($arguments in $commands) {
            & cargo @arguments
            if ($LASTEXITCODE -ne 0) { throw "cargo $arguments failed ($LASTEXITCODE)" }
        }
        if ($Gpu) {
            & cargo run --locked -p designer-render --example gpu_probe
            if ($LASTEXITCODE -ne 0) { throw 'GPU smoke test failed.' }
            & cargo test --locked -p designer-gpu-bridge -- --ignored
            if ($LASTEXITCODE -ne 0) { throw 'GPU bridge contract tests failed.' }
            & cargo run --locked -p designer-gpu-bridge --example composited_probe -- --frames 300
            if ($LASTEXITCODE -ne 0) { throw 'Composited probe failed.' }
        }
    } finally { $env:RUSTDOCFLAGS = $previousRustdocFlags }
} finally { Pop-Location }

