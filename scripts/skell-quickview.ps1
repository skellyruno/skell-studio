# Skell Studio QuickView: real native UI captures, no GUI automation or mockups.
# First use: .\scripts\skell-quickview.ps1 -Mode Auto -Open
# Repeat captures without rebuilding: .\scripts\skell-quickview.ps1 -Mode Render -Open
# After source edits: .\scripts\skell-quickview.ps1 -Mode BuildRender -Open
[CmdletBinding()]
param(
    [ValidateSet("Auto", "Build", "Render", "BuildRender")]
    [string]$Mode = "Auto",

    [ValidateSet("all", "playlist", "rack", "piano", "mixer", "plugins")]
    [string]$View = "all",

    [ValidatePattern('^[1-9][0-9]{2,3}x[1-9][0-9]{2,3}$')]
    [string]$Size = "1440x900",

    [ValidateRange(1, 16)]
    [int]$Jobs = 3,

    [string]$Exe = "",

    [ValidatePattern('^[A-Za-z0-9_-]+$')]
    [string]$Tag = "latest",

    [switch]$Open,
    [switch]$Plan
)
$ErrorActionPreference = "Stop"
$repo = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$targetDir = if ($env:CARGO_TARGET_DIR) {
    if ([IO.Path]::IsPathRooted($env:CARGO_TARGET_DIR)) { $env:CARGO_TARGET_DIR }
    else { Join-Path $repo $env:CARGO_TARGET_DIR }
} else {
    Join-Path $repo "target"
}
$previewExe = Join-Path $targetDir "skell-preview/skell-studio.exe"
$workDir = Join-Path $repo ".skell-quickview"
$profileDir = Join-Path $workDir "isolated-settings"
$outDir = Join-Path $workDir $Tag

$scenes = @(
    [pscustomobject]@{ Key = "playlist"; Label = "Playlist"; Args = @("--view", "playlist", "--no-rack") },
    [pscustomobject]@{ Key = "rack"; Label = "Playlist + Channel Rack"; Args = @("--view", "playlist", "--rack-all") },
    [pscustomobject]@{ Key = "piano"; Label = "Piano Roll"; Args = @("--view", "piano", "--no-rack") },
    [pscustomobject]@{ Key = "mixer"; Label = "Mixer"; Args = @("--view", "mixer", "--no-rack", "--insert", "1") },
    [pscustomobject]@{ Key = "plugins"; Label = "Plugin Palette"; Args = @("--view", "playlist", "--no-rack", "--plugin-menu", "all") }
)
$selected = @($scenes | Where-Object { $View -eq "all" -or $_.Key -eq $View })
if ($selected.Count -eq 0) { throw "Unknown capture view $View" }

$mustBuild = $Mode -in @("Build", "BuildRender")
$mustRender = $Mode -ne "Build"
if ($Mode -eq "Auto" -and -not $Exe -and -not (Test-Path $previewExe)) {
    $candidates = @(
        (Join-Path $targetDir "debug/skell-studio.exe"),
        (Join-Path $targetDir "release/skell-studio.exe")
    )
    if (-not ($candidates | Where-Object { Test-Path $_ })) { $mustBuild = $true }
}
if ($Mode -eq "Auto" -and -not $Exe -and -not (Test-Path $previewExe)) {
    if (@((Join-Path $targetDir "debug/skell-studio.exe"), (Join-Path $targetDir "release/skell-studio.exe")) |
        Where-Object { Test-Path $_ }) {
        $mustBuild = $false
    }
}

if ($Plan) {
    Write-Host "Skell QuickView | mode=$Mode | build=$mustBuild | render=$mustRender | jobs=$Jobs"
    Write-Host "Isolated settings: $profileDir"
    Write-Host "Native output: $outDir"
    Write-Host "Compiled preview: $previewExe"
    foreach ($scene in $selected) { Write-Host "Scene: $($scene.Label) --> $($scene.Key)-$Size.png" }
    return
}

New-Item -ItemType Directory -Force -Path $profileDir | Out-Null
$env:OPENSTUDIO_USER_DIR = $profileDir  # Never migrate/write the user's normal Lemon/Skell Studio settings.

Push-Location $repo
try {
    if ($mustBuild) {
        if ($Exe) { throw "Remove -Exe when building; the build always targets the local source checkout." }
        if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
            throw "Rust/Cargo is missing. Install the Rust toolchain (rustup.rs), then rerun QuickView."
        }
        if (-not (Test-Path (Join-Path $repo "Cargo.toml"))) { throw "This is not a Skell Studio source checkout." }
        $cargoConfig = Get-Content (Join-Path $repo "Cargo.toml") -Raw
        if ($cargoConfig -notmatch '\[profile\.skell-preview\]') {
            throw "The fast profile is missing. Use the Skell fast-preview source branch or update your checkout."
        }
        $env:OPENSTUDIO_NO_PLUGIN_BUILD = "1"
        if (-not $env:LIBCLANG_PATH -and (Test-Path "C:\Program Files\LLVM\bin")) {
            $env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
        }
        # Do not force CARGO_INCREMENTAL=0: the skell-preview profile enables local incremental builds.
        Write-Host "Building fast UI-only Skell Studio profile ($Jobs compile jobs; optimized Release builds are separate)."
        & cargo build --locked --profile skell-preview -p os-app --bin skell-studio -j $Jobs
        if ($LASTEXITCODE -ne 0) { throw "Cargo fast-preview build failed (exit $LASTEXITCODE)." }
        if (-not (Test-Path $previewExe)) { throw "Build did not produce $previewExe" }
    }
    if (-not $mustRender) {
        Write-Host "Built: $previewExe"
        return
    }

    if ($Exe) {
        if (-not (Test-Path $Exe)) { throw "EXE not found: $Exe" }
        $chosenExe = (Resolve-Path $Exe).Path
    } elseif (Test-Path $previewExe) {
        $chosenExe = (Resolve-Path $previewExe).Path
    } else {
        $fallback = @(
            (Join-Path $targetDir "debug/skell-studio.exe"),
            (Join-Path $targetDir "release/skell-studio.exe")
        ) | Where-Object { Test-Path $_ } | Select-Object -First 1
        if (-not $fallback) {
            throw "No local Skell Studio EXE found. Use -Mode BuildRender, or pass -Exe <path-to-skell-studio.exe>."
        }
        $chosenExe = (Resolve-Path $fallback).Path
    }

    New-Item -ItemType Directory -Force -Path $outDir | Out-Null
    $files = @()
    foreach ($scene in $selected) {
        $name = "$($scene.Key)-$Size.png"
        $png = Join-Path $outDir $name
        $renderArgs = @("--screenshot", $png, "--design", "Skell", "--size", $Size) + $scene.Args
        Write-Host "Native renderer: $($scene.Label) -> $name"
        & $chosenExe @renderArgs
        if ($LASTEXITCODE -ne 0) { throw "Skell native renderer failed for $name (exit $LASTEXITCODE)." }
        if (-not (Test-Path $png)) { throw "Native renderer produced no $name" }
        $bytes = [IO.File]::ReadAllBytes($png)
        $sig = @(137, 80, 78, 71, 13, 10, 26, 10)
        if ($bytes.Length -lt 4096) { throw "PNG unexpectedly small ($($bytes.Length) bytes): $png" }
        for ($i = 0; $i -lt $sig.Count; $i++) {
            if ($bytes[$i] -ne $sig[$i]) { throw "Invalid native PNG signature: $png" }
        }
        $w = ([int]$bytes[16] -shl 24) -bor ([int]$bytes[17] -shl 16) -bor ([int]$bytes[18] -shl 8) -bor [int]$bytes[19]
        $h = ([int]$bytes[20] -shl 24) -bor ([int]$bytes[21] -shl 16) -bor ([int]$bytes[22] -shl 8) -bor [int]$bytes[23]
        $dims = $Size.Split('x')
        if ($w -ne [int]$dims[0] -or $h -ne [int]$dims[1]) {
            throw "Screenshot dimensions $w x $h do not match requested $Size for $png"
        }
        $files += [pscustomobject]@{ Label = $scene.Label; Name = $name; Bytes = $bytes.Length }
        Write-Host "Captured actual native paint output: $w x $h ($($bytes.Length) bytes)"
    }
    $cards = ($files | ForEach-Object {
        '<article><a href="' + $_.Name + '" target="_blank"><img src="' + $_.Name +
        '" alt="' + $_.Label + '"></a><h2>' + $_.Label + '</h2><p>' +
        [string]$_.Bytes + ' bytes &middot; native renderer</p></article>'
    }) -join [Environment]::NewLine
    $html = @"
<!doctype html>
<html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Skell Studio / QuickView</title>
<style>
:root{color-scheme:dark}body{font:14px/1.5 "Segoe UI",system-ui,sans-serif;background:#080c08;color:#e4f2e5;margin:0;padding:25px}
h1{font-size:25px;margin:0 0 4px;letter-spacing:.05em}header p{color:#86b68a;margin:0 0 24px}
main{display:grid;grid-template-columns:repeat(auto-fit,minmax(min(100%,500px),1fr));gap:18px}
article{background:#131b14;border:1px solid #335e38;border-radius:12px;overflow:hidden}
article img{display:block;width:100%;height:auto;max-height:480px;object-fit:contain;background:#000}
h2{font-size:16px;margin:10px 14px 0;color:#9aff82}article p{color:#9ab49b;margin:0 14px 13px}
small{color:#8ca08d}
</style></head><body><header><h1>SKELL <span style="color:#9aff82">STUDIO</span> / QUICKVIEW</h1>
<p>Actual Skell Studio renderer captures &mdash; $Size &mdash; $(Get-Date -Format "yyyy-MM-dd HH:mm")</p>
</header><main>$cards</main><p><small>These are real offscreen UI frames, not an OS window capture. Interactivity and audio are not tested here.</small></p></body></html>
"@
    $index = Join-Path $outDir "index.html"
    [IO.File]::WriteAllText($index, $html, [Text.Encoding]::UTF8)
    Write-Host "QuickView captured $($files.Count) view(s). Open $index"
    if ($Open) { Start-Process $index }
}
finally {
    Pop-Location
}
