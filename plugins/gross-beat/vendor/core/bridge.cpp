#include "core/GrossBeat.h"

#include <algorithm>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <limits>
#include <new>
#include <vector>

namespace
{
struct Bridge
{
    lynx::grossbeat::Engine engine;
    int sampleRate = 44100;
    int maxBlock = 4096;
    int ppq = 96;
    float tempo = 120.0f;
    double ticks = 0.0;
    double expectedTicks = 0.0;
    bool loaded = false;
    bool started = false;
    lynx::grossbeat::State state;
    lynx::grossbeat::MidiSlots midi;
};

constexpr int kLatencySamples = 15;
}

extern "C"
{
int lynx_gb_factory_state(std::uint8_t* out, int capacity) noexcept
{
    try
    {
        const auto bytes = lynx::grossbeat::writeState(lynx::grossbeat::factoryState());
        if (bytes.size() > static_cast<std::size_t>(std::numeric_limits<int>::max()))
            return -1;
        const int needed = static_cast<int>(bytes.size());
        if (out == nullptr || capacity < needed)
            return needed;
        std::memcpy(out, bytes.data(), bytes.size());
        return needed;
    }
    catch (...)
    {
        return -1;
    }
}

void* lynx_gb_new(int sampleRate, int maxBlock) noexcept
{
    try
    {
        auto* b = new Bridge();
        b->sampleRate = std::clamp(sampleRate, 8000, 384000);
        b->maxBlock = std::clamp(maxBlock, 1, 65536);
        return b;
    }
    catch (...)
    {
        return nullptr;
    }
}

void lynx_gb_delete(void* instance) noexcept
{
    delete static_cast<Bridge*>(instance);
}

bool lynx_gb_load(void* instance, const std::uint8_t* data, std::size_t size) noexcept
{
    if (instance == nullptr || data == nullptr || size == 0)
        return false;
    auto& b = *static_cast<Bridge*>(instance);
    try
    {
        const auto state = lynx::grossbeat::parseState(data, size);
        if (!state)
            return false;
        b.ppq = 96;
        b.state = *state;
        b.midi.clear();
        if (!b.engine.load(b.state, b.ppq))
            return false;
        b.engine.prepare(b.sampleRate, b.maxBlock);
        b.tempo = 120.0f;
        b.ticks = 0.0;
        b.expectedTicks = 0.0;
        b.started = false;
        b.loaded = true;
        return true;
    }
    catch (...)
    {
        b.loaded = false;
        return false;
    }
}

void lynx_gb_prepare(void* instance, int sampleRate, int maxBlock) noexcept
{
    if (instance == nullptr)
        return;
    auto& b = *static_cast<Bridge*>(instance);
    b.sampleRate = std::clamp(sampleRate, 8000, 384000);
    b.maxBlock = std::clamp(maxBlock, 1, 65536);
    if (b.loaded)
        b.engine.prepare(b.sampleRate, b.maxBlock);
    b.started = false;
    b.ticks = 0.0;
    b.expectedTicks = 0.0;
}

void lynx_gb_set_param(void* instance, int index, int raw) noexcept
{
    if (instance == nullptr)
        return;
    auto& b = *static_cast<Bridge*>(instance);
    if (b.loaded && index >= 0 && index < lynx::grossbeat::paramCount)
    {
        b.state.params[static_cast<std::size_t>(index)] = raw;
        b.engine.setParam(index, raw);
    }
}

int lynx_gb_note(void* instance, bool on, int channel, int note) noexcept
{
    if (instance == nullptr)
        return -1;
    auto& b = *static_cast<Bridge*>(instance);
    if (!b.loaded || !b.state.midiTrigger || channel < 0 || channel > 15 || note < 0 || note > 127)
        return -1;

    const std::array<int, 2> current { b.state.params[0], b.state.params[1] };
    const auto selected = b.midi.note(on, channel, note, current, [&b](int kind, int slot) noexcept {
        const auto index = static_cast<std::size_t>(kind * lynx::grossbeat::slotsPerKind + slot);
        return b.state.slots[index].keyHeld;
    });
    if (selected.first < 0 || selected.second < 0)
        return -1;

    const auto parameter = selected.first;
    b.state.params[static_cast<std::size_t>(parameter)] = selected.second;
    b.engine.setParam(parameter, selected.second);
    return parameter * lynx::grossbeat::slotsPerKind + selected.second;
}

void lynx_gb_set_transport(void* instance, double tempo, double positionBeats, bool hasPosition, bool playing) noexcept
{
    if (instance == nullptr)
        return;
    auto& b = *static_cast<Bridge*>(instance);
    if (!b.loaded)
        return;

    if (std::isfinite(tempo) && tempo > 0.0)
    {
        b.tempo = static_cast<float>(std::clamp(tempo, 1.0, 1000.0));
        b.engine.setTempo(b.tempo);
    }

    const double requested = hasPosition && std::isfinite(positionBeats)
        ? positionBeats * static_cast<double>(b.ppq)
        : b.expectedTicks;

    if (!playing)
    {
        if (b.started)
            b.engine.stop();
        b.started = false;
        b.ticks = requested;
        b.expectedTicks = requested;
        return;
    }

    if (!b.started)
    {
        b.engine.start(requested);
        b.started = true;
    }
    else if (hasPosition && std::fabs(requested - b.expectedTicks) > 1.0)
    {
        b.engine.jump(requested);
    }

    b.ticks = requested;
}

void lynx_gb_process(void* instance, float* left, float* right, int frames) noexcept
{
    if (instance == nullptr || left == nullptr || right == nullptr || frames <= 0)
        return;
    auto& b = *static_cast<Bridge*>(instance);
    if (!b.loaded)
        return;
    b.engine.process(left, right, frames, b.ticks);
    if (b.started)
    {
        b.expectedTicks = b.ticks
            + static_cast<double>(frames) * static_cast<double>(b.tempo)
                / 60.0 * static_cast<double>(b.ppq) / static_cast<double>(b.sampleRate);
    }
}

void lynx_gb_reset(void* instance) noexcept
{
    if (instance == nullptr)
        return;
    auto& b = *static_cast<Bridge*>(instance);
    if (b.loaded && b.started)
        b.engine.stop();
    b.started = false;
    b.ticks = 0.0;
    b.expectedTicks = 0.0;
}

int lynx_gb_latency() noexcept
{
    return kLatencySamples;
}
}
