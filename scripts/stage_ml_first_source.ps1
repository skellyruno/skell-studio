# Stages the full ML-first Skell Studio source package in a NEW Git branch.
# Never overwrites main. The ZIP comes from the completed ChatGPT ML-first source package.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$SourceZip,
    [string]$Branch="feature/skell-ml-first",
    [switch]$Push
)
$ErrorActionPreference="Stop"
$root=(Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$git=Get-Command git -ErrorAction SilentlyContinue
if (-not $git) { throw "Git is required." }
if (-not (Test-Path $SourceZip)) { throw "ML-first source ZIP does not exist: $SourceZip" }
$zip=(Resolve-Path $SourceZip).Path
Push-Location $root
$temp=Join-Path ([IO.Path]::GetTempPath()) ("skell-ml-first-stage-"+[guid]::NewGuid().ToString("N"))
try {
    $inside=& git rev-parse --show-toplevel
    if ($LASTEXITCODE -ne 0 -or [IO.Path]::GetFullPath($inside) -ne [IO.Path]::GetFullPath($root)) {
        throw "Run from a standalone Skell Studio Git checkout, not an unrelated directory."
    }
    $changes=@(& git status --porcelain)
    if ($changes.Count -gt 0) { throw "Git checkout has uncommitted changes. Commit/stash them first; nothing was altered." }
    if ($Branch -in @("main","master","production","release") -or $Branch -match '(^|/)(main|production|release)(/|$)') {
        throw "Refusing to stage into a protected branch: $Branch"
    }
    New-Item -ItemType Directory -Path $temp -Force | Out-Null
    Expand-Archive -LiteralPath $zip -DestinationPath $temp -Force
    $source=Join-Path $temp "skell-studio"
    foreach ($path in @(
        "Cargo.lock","Cargo.toml","crates/os-app/Cargo.toml",
        "crates/os-app/src/main.rs",
        "crates/os-engine/src/engine/mixer_graph.rs",
        "crates/os-app/src/visualizer.rs",
        "crates/os-app/src/notation.rs",
        "crates/os-app/src/mix_assistant.rs",
        "crates/os-app/src/recording_merge.rs",
        "THIRD-PARTY-NOTICES.txt",
        "SKELL_ML_FIRST_README.md",
        "SKELL_ML_UPGRADE_MANIFEST.json"
    )) {
        if (-not (Test-Path (Join-Path $source $path))) { throw "Archive is missing $path; checkout unchanged." }
    }
    $cargo=Get-Content (Join-Path $source "crates/os-app/Cargo.toml") -Raw
    if ($cargo -notmatch 'name = "skell-studio"') { throw "ZIP lacks the Skell Studio binary target; checkout unchanged." }
    if ($Branch -notmatch '^[A-Za-z0-9/_-]+$') { throw "Unsafe branch name." }
    & git fetch origin main
    if ($LASTEXITCODE -ne 0) { throw "Could not fetch current main." }
    & git switch -c $Branch origin/main
    if ($LASTEXITCODE -ne 0) { throw "Could not create staging branch. No source overwritten." }
    # Overlay ML-first files but preserve repository-specific instructions,
    # original references, and any plugin files not in ML's workspace. Cargo's
    # workspace manifest owns the actual build graph.
    foreach ($item in Get-ChildItem -LiteralPath $source -Force) {
        if ($item.Name -eq ".git") { continue }
        Copy-Item -LiteralPath $item.FullName -Destination $root -Force -Recurse
    }
    & git add -A
    if ($LASTEXITCODE -ne 0) { throw "Could not stage ML-first source." }
    & git diff --cached --stat
    & git commit -m "feat: stage ML-first Skell Studio engine, editor, visualizer and tools"
    if ($LASTEXITCODE -ne 0) { throw "Git commit failed; source remains in staging branch for inspection." }
    Write-Host "Staged ML-first source on $Branch. NOT compiled or validated by this script."
    if ($Push) {
        & git push --set-upstream origin $Branch
        if ($LASTEXITCODE -ne 0) { throw "Push failed; local commit is preserved." }
        Write-Host "Pushed staging branch. Open a draft PR and run Rust/Windows/UI tests before merging."
    }
}
finally {
    Pop-Location
    if (Test-Path $temp) { Remove-Item -LiteralPath $temp -Recurse -Force }
}