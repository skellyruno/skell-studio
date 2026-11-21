// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: AGPL-3.0-only
// Adapted from LYNX SHARE 0.1.30, commit 88ec555ea2bce9714c92707cc06a5abfd317c900.

#include "core/FruityDelay.h"
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>
#include <new>

namespace
{
struct Delay3Bridge
{
    lynx::fldelay::Delay3Engine engine;
    std::array<int, lynx::fldelay::delay3Controls> raw {};
    double sampleRate = 44100.0;
    double tempo = 120.0;
    bool configured = false;

    explicit Delay3Bridge(double rate)
        : engine(rate), sampleRate(rate)
    {
    }
};
}

extern "C"
{
void* skell_delay3_new(int sampleRate) noexcept
{
    try
    {
        const auto rate = std::clamp(sampleRate, 8000, 384000);
        return new Delay3Bridge(static_cast<double>(rate));
    }
    catch (...)
    {
        return nullptr;
    }
}

void skell_delay3_delete(void* instance) noexcept
{
    delete static_cast<Delay3Bridge*>(instance);
}

void skell_delay3_set(void* instance, const std::int32_t* raw, double bpm) noexcept
{
    if (instance == nullptr || raw == nullptr)
        return;

    auto& bridge = *static_cast<Delay3Bridge*>(instance);
    bridge.tempo = std::isfinite(bpm) && bpm > 1.0 ? std::clamp(bpm, 1.0, 1000.0) : 120.0;
    bridge.engine.setTempo(bridge.tempo);

    bool changed = false;
    for (int i = 0; i < lynx::fldelay::delay3Controls; ++i)
    {
        const auto value = raw[i];
        if (!bridge.configured || bridge.raw[static_cast<std::size_t>(i)] != value)
        {
            bridge.raw[static_cast<std::size_t>(i)] = value;
            bridge.engine.setParameter(i, lynx::fldelay::delay3EngineValue(i, value));
            changed = true;
        }
    }

    if (!bridge.configured)
    {
        // Reach the saved-state targets before the first audio sample, as FL does during initial load.
        bridge.engine.settle();
        bridge.configured = true;
    }
    (void) changed;
}

void skell_delay3_reset(void* instance) noexcept
{
    if (instance == nullptr)
        return;

    auto& bridge = *static_cast<Delay3Bridge*>(instance);
    // setSampleRate() clears the preallocated delay/filter state while retaining the configured controls.
    // It is called by the plugin's reset path at the active sample rate; no buffers need to grow.
    bridge.engine.setSampleRate(bridge.sampleRate);
    bridge.engine.setTempo(bridge.tempo);
    bridge.engine.settle();
}

void skell_delay3_process(void* instance, float* left, float* right, int frames) noexcept
{
    if (instance == nullptr || left == nullptr || right == nullptr || frames <= 0)
        return;
    auto& bridge = *static_cast<Delay3Bridge*>(instance);
    bridge.engine.process(left, right, frames);
}
}
