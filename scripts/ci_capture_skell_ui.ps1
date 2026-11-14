# Real Skell Studio renderer capture: the same Canvas/App paint path as the DAW.
# Runs skell-studio.exe in its built-in --screenshot mode; never creates mockups.
param(
    [string]$Exe = "target/skell-preview/skell-studio.exe"
)
$ErrorActionPreference = "Stop"
if (-not (Test-Path $Exe)) { throw "Preview executable missing: $Exe" }
New-Item -ItemType Directory -Force -Path "captures" | Out-Null

$views = @(
    @{Name="playlist-1920x1080"; View="playlist"; Size="1920x1080"; Extra=@("--no-rack")},
    @{Name="playlist-with-rack-1920x1080"; View="playlist"; Size="1920x1080"; Extra=@("--rack-all")},
    @{Name="piano-1920x1080"; View="piano"; Size="1920x1080"; Extra=@("--no-rack")},
    @{Name="mixer-1920x1080"; View="mixer"; Size="1920x1080"; Extra=@("--no-rack","--insert","1")},
    @{Name="plugin-palette-1920x1080"; View="playlist"; Size="1920x1080"; Extra=@("--no-rack","--plugin-menu","all")},
    @{Name="playlist-1280x720"; View="playlist"; Size="1280x720"; Extra=@("--no-rack")},
    @{Name="mixer-2560x1440-150pct"; View="mixer"; Size="2560x1440"; Extra=@("--no-rack","--scale","1.5")}
)

$manifest = @("SKELL STUDIO | NATIVE UI RENDERER CAPTURES",
    "Source: executable built from this GitHub Actions checkout",
    "Method: Skell Studio built-in --screenshot; renders the real App::paint UI without an OS window",
    "Not an interactive Windows session; real mouse interactions, plugin editors and audio remain unverified",
    "Build: fast skell-preview (non-release, not for DSP/CPU measurements)", "")
foreach($v in $views) {
    $file = "captures/$($v.Name).png"
    $args = @("--screenshot", $file, "--design", "Skell", "--view", $v.View, "--size", $v.Size) + $v.Extra
    Write-Host "Rendering $file from actual Skell Studio source..."
    & $Exe @args
    if ($LASTEXITCODE -ne 0) { throw "Native renderer returned exit code $LASTEXITCODE for $file" }
    if (-not (Test-Path $file)) { throw "Renderer failed to write $file" }
    $bytes = [IO.File]::ReadAllBytes((Resolve-Path $file))
    $sig = @(137,80,78,71,13,10,26,10)
    if ($bytes.Length -lt 16000) { throw "Tiny/possibly blank PNG ($($bytes.Length) bytes): $file" }
    for ($i = 0; $i -lt $sig.Length; $i++) {
        if ($bytes[$i] -ne $sig[$i]) { throw "Invalid PNG signature: $file" }
    }
    $w = ([uint32]$bytes[16] -shl 24) -bor ([uint32]$bytes[17] -shl 16) -bor ([uint32]$bytes[18] -shl 8) -bor [uint32]$bytes[19]
    $h = ([uint32]$bytes[20] -shl 24) -bor ([uint32]$bytes[21] -shl 16) -bor ([uint32]$bytes[22] -shl 8) -bor [uint32]$bytes[23]
    $dims = $v.Size -split "x"
    if ($w -ne [int]$dims[0] -or $h -ne [int]$dims[1]) {
        throw "Wrong PNG dimensions $w x $h; wanted $($v.Size): $file"
    }
    $manifest += "$file | $w x $h | $($bytes.Length) bytes"
    Write-Host "PASS: native PNG $w x $h, $($bytes.Length) bytes"
}
$manifest | Set-Content "captures/CAPTURE_MANIFEST.txt" -Encoding utf8
