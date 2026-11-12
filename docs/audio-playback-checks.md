# Playback release checks

Offline checks help isolate failures but do not certify a physical audio driver or another computer.

## Installed instrument and sampler diagnostic

`transport::tests::installed_vst_three_track_queue_probe` is an ignored, opt-in test. Set
`LEMON_STRESS_PLUGIN` to an installed VST3 instrument and `LEMON_STRESS_SECONDS` to the
duration per scenario (default 10 seconds, capped at 120). Use an isolated
`OPENSTUDIO_USER_DIR` and an E: temporary directory on the Windows development machine.
Run the normal storage guard before compiling.

```text
cargo test --locked --release -p os-app installed_vst_three_track_queue_probe -- --ignored --nocapture --test-threads=1
```

The test renders a synthetic 44.1 kHz sample through two actual hosted stock
samplers at 48 kHz, with and without a four-note chord in the selected instrument.
Each scenario creates a fresh engine and runs the real feeder, render-ahead queue,
and paced synthetic callback at 48, 192, 512, and 1024 frames. It reports underruns,
silent blocks, nonfinite samples, callback lateness, callback execution time, and
peak published render load. It never opens an editor, microphone or audio device.
The plugin uses its initial preset. This is not the reporter's project or preset.

Do not run compilation or other timing tests concurrently. Callback pacing uses
the local scheduler and does not emulate ASIO, an older CPU, or driver buffer
switches. Sampler output can mask an instrument-specific silent gap, so also use
`installed_vst_audio_probe` to check that the instrument itself produces audio.

## Hardware acceptance

For a reproducible dropout report, retain the exact build, project and samples,
plugin format/version/preset, OS/CPU, driver version, sample rate and buffer size.
Compare a copy with the suspect instrument removed. Hold the buffer fixed, reset
the counter, and measure a continuous run separately from changing buffer sizes.
Capture CPU/render load and actual audio behavior. Where possible compare the
same workload and driver configuration with another host.

The existing Underruns total combines output-queue shortages, hosted-plugin misses
and driver overload notifications. It does not identify the source by itself or
count unique audible incidents. `OS_DEBUG=1` logs queue depth and callback spacing
every two seconds during playback; collect logs locally without private project
data in public reports. Monitor dropouts have a separate counter.

An unresolved user-reported failure stays open even when these local tests pass.
Do not call a release audio-verified until the affected setup passes its acceptance
run, or the owner explicitly changes the release decision with the limitation known.
