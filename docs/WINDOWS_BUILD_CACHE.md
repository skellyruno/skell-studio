# Skell Studio Windows build caching

## Why this was necessary

The Windows verification run 43319457934 reported **No cache found** and `CACHE_ON_FAILURE: false`. The checkout had to download/compile large Rust dependencies again, even though the final error came from an inherited bundled plugin. It took 36 minutes before failing.

## Current change (draft PR #3)

`.github/workflows/skell-studio-verify.yml`, windows-check:

1. Restore Cargo registries and compiled dependency artifacts using `Swatinem/rust-cache@v2`, with a stable `shared-key: skell-windows-check`.
2. Use `cache-on-failure: true` to store reusable outputs even when an unrelated later Rust source error stops the DAW check.
3. Start `mozilla-actions/sccache-action@v0.0.11` for Rust compiler outputs; set `RUSTC_WRAPPER=sccache` and `SCCACHE_GHA_ENABLED=true`.
4. On every run, print `sccache --show-stats` (compilation requests, hits, misses, reasons).
5. Test on the isolated `dev/skell-fast-windows-cache` branch before merging.

**Important:** The first run of a new cache key will usually be slow. The action needs one completed run to save reusable data, and a *second* run using the same toolchain and flags to measure hits. Cache keys reflect Rust version and lockfile/manifests, and GitHub may evict old caches. We have not verified any speedup yet.

The existing build still checks all essential bundled instruments. No changes to FLP parsing, DSP, audio-host code, binaries or packaging are made by this caching patch.

## How to verify efficacy

1. Open [Windows cache trial run](https://github.com/skellyruno/skell-studio/actions/runs/43327322225).
2. Look for `Restore Rust dependencies`, `Enable shared compiler-output cache`, and `Show cache hits and misses` steps.
3. Confirm the cache is **saved** in job post-steps even when `cargo check` reports a Rust compiler error. If the job is canceled, save is not guaranteed.
4. Start another run on the same branch **after the previous run completes**.
5. Compare Rust compile duration, cache hit rate, and overall wall-clock duration. A warm cache does not guarantee the full native C/C++ toolchain build will be fast, since some native builders don't route through `RUSTC_WRAPPER`.
6. Merge PR #3 only after the action setup, compilation and post-run cache steps behave as expected.

## Notes

- The first-party Windows build check runs with `OPENSTUDIO_NO_PLUGIN_BUILD=1`, which skips packaging plugins but **does not** avoid compiling Rust plugin source dependencies from `os-app`.
- `RUSTC_WRAPPER` and the GitHub Actions `sccache` cache are CI-only. Do not write them into the developer's local project config: local Windows builds should work without having `sccache` installed.
- Consider pinning the Rust toolchain for reproducibility after the first clean compile; changing `stable` may invalidate the cache.
- Do not manually cache the whole `target` tree with `actions/cache` on top of `rust-cache` because it duplicates data and may exhaust the cache quota.

## Monitoring

The separate Skell Studio Verified Upgrade task runs hourly (the available scheduler's maximum supported frequency), checks current GitHub Actions results and only proceeds to the larger upgrade on confirmed passing verification. A UI countdown timer by itself cannot inspect GitHub or trigger an action.
