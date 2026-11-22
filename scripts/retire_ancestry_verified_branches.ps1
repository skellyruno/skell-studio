# Copyright (C) 2026 Skell Studio contributors
# Safe-by-default historical branch retirement: does not touch main, release, or unmerged work.
# -Apply requires deliberate human invocation. No CI workflow invokes it.
param(
    [switch]$Apply,
    [switch]$ConfirmArchiveAndRetire,
    [string]$Remote = "origin"
)
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$project = (& git remote get-url $Remote 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $project -notmatch '(github[.]com[:/])skellyruno/skell-studio([.]git)?$') {
    throw "Remote must point to skellyruno/skell-studio; got: $project"
}
$candidate = "feature/skell-release-candidate-20261121"
if ($Apply -and -not $ConfirmArchiveAndRetire) {
    throw "Refusing destructive branch operations without -ConfirmArchiveAndRetire"
}

# This is the exact SHA inventory observed 2026-11-22 before reconciliation.
# Do not silently add arbitrary branches, change these tips, or use a wildcard.
$verified = [ordered]@{
    "feature/skell-beat-curves-midi-20261121" = "ec933330aae4bb8a074b163e975fdc730ffda3c5"
    "feature/skell-fl-fst-presets-lynx-audit-20261120" = "0f56c7ae4f0488b73649a1c0ecd6efd6ad965bc9"
    "feature/skell-lynx-delay3-in-place-20261121" = "5b7592638e575edffd7aa13269e9033659e7c0da"
    "feature/skell-lynx-limiter-in-place-20261121" = "2b6d436b7c0d1a3014aaf881b6ea598a747f0ae5"
    "feature/skell-lynx-reeverb2-in-place-20261121" = "64f9e34feb423bf8132577459cf6f5bb0710727a"
    "feature/skell-small-fx-skull-branding-20261121" = "3f8c5268c48a7005a26311b898bfd45dd5d49d82"
    "dev/skell-smooth-ui-branding-20261122" = "bace2eeaeff2520932d61ca59a6ec39c6289c2e9"
}

Write-Host "Inspecting remote branch refs. Apply=$Apply"
# Fetch only updates local tracking refs and never pushes, deletes, or merges.
& git fetch --no-tags $Remote "+refs/heads/*:refs/remotes/$Remote/*"
if ($LASTEXITCODE -ne 0) { throw "Unable to verify remote ancestry" }

$candidateSha = (& git rev-parse "refs/remotes/$Remote/$candidate" 2>&1 | Out-String).Trim()
if ($LASTEXITCODE -ne 0 -or $candidateSha -notmatch '^[0-9a-f]{40}$') { throw "Candidate branch missing" }

$approvedCount = 0
foreach ($branch in $verified.Keys) {
    if ($branch -match '(^main$|production|(^|/)release($|/))' -or $branch -eq $candidate) {
        throw "Protected/refuse branch: $branch"
    }
    $remoteRef = "refs/heads/$branch"
    $line = (& git ls-remote --heads $Remote $remoteRef 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { throw "Remote branch check failed: $branch" }
    if (-not $line) {
        Write-Host "ALREADY RETIRED: $branch"
        continue
    }
    $tip = ($line -split '\s+')[0]
    if ($tip -ne $verified[$branch]) {
        Write-Warning "SKIP changed branch $branch (expected $($verified[$branch]), got $tip)"
        continue
    }
    & git merge-base --is-ancestor $tip $candidateSha
    if ($LASTEXITCODE -ne 0) {
        Write-Warning "SKIP not fully merged by ancestry: $branch"
        continue
    }
    $archiveTag = "archive/skell-studio/verified/$branch"
    $existing = (& git ls-remote --tags $Remote "refs/tags/$archiveTag" 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -ne 0) { throw "Archive tag lookup failed: $archiveTag" }
    if ($existing) {
        $archivedSha = ($existing -split '\s+')[0]
        if ($archivedSha -ne $tip) { throw "Conflicting archive tag for $branch" }
    }
    if (-not $Apply) {
        Write-Host "PLAN: archive $branch ($tip) as $archiveTag, recheck, then delete with exact-ref lease"
        $approvedCount++
        continue
    }
    if (-not $existing) {
        & git tag $archiveTag $tip
        if ($LASTEXITCODE -ne 0) { throw "Could not create local archive tag" }
        & git push $Remote "refs/tags/$archiveTag:refs/tags/$archiveTag"
        if ($LASTEXITCODE -ne 0) { throw "Could not push archive tag; branch retained" }
    }
    $remoteTag = (& git ls-remote --tags $Remote "refs/tags/$archiveTag" 2>&1 | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or (($remoteTag -split '\s+')[0] -ne $tip)) {
        throw "Archive tag was not verified; branch retained"
    }
    # Git refuses deletion if any other process changed the branch SHA.
    & git push "--force-with-lease=${remoteRef}:$tip" $Remote ":$remoteRef"
    if ($LASTEXITCODE -ne 0) { throw "Remote branch changed or deletion denied; archive tag retained" }
    Write-Host "ARCHIVED + RETIRED: $branch (exact SHA $tip)"
    $approvedCount++
}
if (-not $Apply) {
    Write-Host "Dry run completed: $approvedCount ancestry-contained branches eligible for archive + retirement."
    Write-Host "All diverged historical branches remain protected for full content reconciliation."
    Write-Host "To perform later: -Apply -ConfirmArchiveAndRetire (requires repository write permission)."
} else { Write-Host "Archived and retired $approvedCount ancestry-contained branches." }
