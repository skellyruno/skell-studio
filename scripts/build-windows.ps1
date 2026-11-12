$ErrorActionPreference = "Stop"
Write-Host "Skell Studio — Windows development build (source baseline)"
Write-Host "Requires Rust/Cargo, Visual Studio C++ Build Tools and LLVM/Clang."
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw "Rust/Cargo was not found. Install rustup." }
$env:OPENSTUDIO_NO_PLUGIN_BUILD = "1"
if (-not $env:LIBCLANG_PATH) { $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin" }
cargo build --locked --release -p os-app --bin skell-studio
if ($LASTEXITCODE -ne 0) { throw "DAW build failed; inspect logs." }
Write-Host "Build complete: target\release\skell-studio.exe"
