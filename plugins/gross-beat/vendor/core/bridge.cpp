#include "core/GrossBeat.h"

#include <algorithm>
#include <array>
#include <atomic>
#include <optional>
#include <utility>
#include <cmath>
#include <cstddef>
#include <cstdio>
#include <cstdint>
#include <cstring>
#include <limits>
#include <memory>
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
    lynx::grossbeat::State* state = nullptr; // owned by the audio thread after load
    std::atomic<lynx::grossbeat::State*> pendingState { nullptr }; // parsed on the editor thread
    // Intrusive, unbounded retired-state stack: no allocations, locks, or frees
    // on the audio callback, and no dropped pointers after 64 rapid edits.
    std::atomic<lynx::grossbeat::State*> retiredHead { nullptr };
    lynx::grossbeat::MidiSlots midi;

    void retire(lynx::grossbeat::State* old) noexcept
    {
        if (old == nullptr) return;
        auto* head = retiredHead.load(std::memory_order_relaxed);
        do
        {
            old->nextRetired = head;
        } while (!retiredHead.compare_exchange_weak(
            head, old, std::memory_order_release, std::memory_order_relaxed));
    }

    // Called by the control thread (or during destruction), never from process().
    void reclaimRetired() noexcept
    {
        auto* head = retiredHead.exchange(nullptr, std::memory_order_acquire);
        while (head != nullptr)
        {
            auto* next = head->nextRetired;
            delete head;
            head = next;
        }
    }

    ~Bridge()
    {
        delete pendingState.exchange(nullptr, std::memory_order_acq_rel);
        delete state;
        reclaimRetired();
    }
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
        auto parsed = lynx::grossbeat::parseState(data, size);
        if (!parsed)
            return false;
        auto next = std::make_unique<lynx::grossbeat::State>(std::move(*parsed));
        for (auto& slot : next->slots)
            if (slot.points.empty() || slot.points.size() > static_cast<std::size_t>(lynx::grossbeat::maxEditPoints))
                return false;
        b.ppq = 96;
        if (!b.engine.load(*next, b.ppq))
            return false;
        delete b.state;
        b.state = next.release();
        delete b.pendingState.exchange(nullptr, std::memory_order_acq_rel);
        b.reclaimRetired();
        b.midi.clear();
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
    if (b.loaded && b.state != nullptr && index >= 0 && index < lynx::grossbeat::paramCount)
    {
        b.state->params[static_cast<std::size_t>(index)] = raw;
        b.engine.setParam(index, raw);
    }
}

int lynx_gb_note(void* instance, bool on, int channel, int note) noexcept
{
    if (instance == nullptr)
        return -1;
    auto& b = *static_cast<Bridge*>(instance);
    if (!b.loaded || b.state == nullptr || !b.state->midiTrigger || channel < 0 || channel > 15 || note < 0 || note > 127)
        return -1;

    const std::array<int, 2> current { b.state->params[0], b.state->params[1] };
    const auto selected = b.midi.note(on, channel, note, current, [&b](int kind, int slot) noexcept {
        const auto index = static_cast<std::size_t>(kind * lynx::grossbeat::slotsPerKind + slot);
        return b.state->slots[index].keyHeld;
    });
    if (selected.first < 0 || selected.second < 0)
        return -1;

    const auto parameter = selected.first;
    b.state->params[static_cast<std::size_t>(parameter)] = selected.second;
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
    if (auto* next = b.pendingState.exchange(nullptr, std::memory_order_acq_rel))
    {
        if (b.engine.update(*next))
        {
            for (int i = 0; i < lynx::grossbeat::paramCount; ++i)
                b.engine.setParam(i, next->params[static_cast<std::size_t>(i)]);
            auto* old = b.state;
            b.state = next;
            b.retire(old);
        }
        else
            b.retire(next);
    }
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

// Stateless envelope readers/editors. These are called by the editor thread and only
// serialize the existing FL state image; no audio-thread object is touched.
int lynx_gb_state_point_count(const std::uint8_t* data, std::size_t size, int slot) noexcept
{
    try
    {
        if (data == nullptr || slot < 0 || slot >= 72) return -1;
        const auto state = lynx::grossbeat::parseState(data, size);
        if (!state) return -1;
        const auto points = lynx::grossbeat::editPoints(state->slots[static_cast<std::size_t>(slot)]);
        return static_cast<int>(points.size());
    }
    catch (...) { return -1; }
}

bool lynx_gb_state_get_point(const std::uint8_t* data, std::size_t size, int slot, int index,
    double* x, float* y, float* tension, int* type) noexcept
{
    try
    {
        if (data == nullptr || slot < 0 || slot >= 72 || index < 0 || x == nullptr || y == nullptr || tension == nullptr || type == nullptr)
            return false;
        const auto state = lynx::grossbeat::parseState(data, size);
        if (!state) return false;
        const auto points = lynx::grossbeat::editPoints(state->slots[static_cast<std::size_t>(slot)]);
        if (static_cast<std::size_t>(index) >= points.size()) return false;
        const auto& point = points[static_cast<std::size_t>(index)];
        *x = point.x; *y = point.y; *tension = point.tension; *type = static_cast<int>(point.type);
        return true;
    }
    catch (...) { return false; }
}

float lynx_gb_state_level(const std::uint8_t* data, std::size_t size, int slot, double beat) noexcept
{
    try
    {
        if (data == nullptr || slot < 0 || slot >= 72) return 0.0f;
        const auto state = lynx::grossbeat::parseState(data, size);
        if (!state) return 0.0f;
        return lynx::grossbeat::levelAt(state->slots[static_cast<std::size_t>(slot)], std::clamp(beat, 0.0, 4.0));
    }
    catch (...) { return 0.0f; }
}

int lynx_gb_state_render_slot(const std::uint8_t* data, std::size_t size, int slot, float* out, int count) noexcept
{
    try
    {
        if (data == nullptr || slot < 0 || slot >= 72 || out == nullptr || count <= 0) return -1;
        const auto state = lynx::grossbeat::parseState(data, size);
        if (!state) return -1;
        const auto values = lynx::grossbeat::renderSlot(state->slots[static_cast<std::size_t>(slot)], count);
        if (values.size() != static_cast<std::size_t>(count)) return -1;
        std::copy(values.begin(), values.end(), out);
        return count;
    }
    catch (...) { return -1; }
}

int lynx_gb_state_midi_trigger(const std::uint8_t* data, std::size_t size) noexcept
{
    try
    {
        if (data == nullptr) return -1;
        const auto state = lynx::grossbeat::parseState(data, size);
        return state ? static_cast<int>(state->midiTrigger) : -1;
    }
    catch (...) { return -1; }
}

static int write_state_result(const lynx::grossbeat::State& state, std::uint8_t* out, int capacity) noexcept
{
    try
    {
        const auto bytes = lynx::grossbeat::writeState(state);
        if (bytes.size() > static_cast<std::size_t>(std::numeric_limits<int>::max())) return -1;
        const auto needed = static_cast<int>(bytes.size());
        if (out == nullptr || capacity < needed) return needed;
        std::memcpy(out, bytes.data(), bytes.size());
        return needed;
    }
    catch (...) { return -1; }
}

// operation: 0 move, 1 add, 2 remove, 3 set the selected point's curve mode/tension.
int lynx_gb_state_edit(const std::uint8_t* data, std::size_t size, int operation, int slot, int index,
    double x, float y, int type, float tension, std::uint8_t* out, int capacity, int* resulting_index) noexcept
{
    try
    {
        if (data == nullptr || slot < 0 || slot >= 72 || !std::isfinite(x) || !std::isfinite(y) || !std::isfinite(tension))
            return -1;
        auto parsed = lynx::grossbeat::parseState(data, size);
        if (!parsed) return -1;
        auto points = lynx::grossbeat::editPoints(parsed->slots[static_cast<std::size_t>(slot)]);
        int selected = index;
        switch (operation)
        {
            case 0:
                if (index < 0 || static_cast<std::size_t>(index) >= points.size()) return -1;
                lynx::grossbeat::movePoint(points, index, x, std::clamp(y, 0.0f, 1.0f));
                break;
            case 1:
                if (points.size() >= static_cast<std::size_t>(lynx::grossbeat::maxEditPoints)) return -1;
                selected = lynx::grossbeat::addPoint(points, x, std::clamp(y, 0.0f, 1.0f),
                    static_cast<std::uint8_t>(std::clamp(type, 0, 12)), std::clamp(tension, -1.0f, 1.0f));
                break;
            case 2:
                if (!lynx::grossbeat::removePoint(points, index)) return -1;
                selected = std::clamp(index, 0, static_cast<int>(points.size()) - 1);
                break;
            case 3:
                if (index < 0 || static_cast<std::size_t>(index) >= points.size() || type < 0 || type > 12) return -1;
                points[static_cast<std::size_t>(index)].type = static_cast<std::uint8_t>(type);
                points[static_cast<std::size_t>(index)].tension = std::clamp(tension, -1.0f, 1.0f);
                break;
            default: return -1;
        }
        lynx::grossbeat::setPoints(parsed->slots[static_cast<std::size_t>(slot)], points);
        if (resulting_index != nullptr) *resulting_index = selected;
        return write_state_result(*parsed, out, capacity);
    }
    catch (...) { return -1; }
}

int lynx_gb_state_set_midi_trigger(const std::uint8_t* data, std::size_t size, bool enabled,
    std::uint8_t* out, int capacity) noexcept
{
    try
    {
        if (data == nullptr) return -1;
        auto parsed = lynx::grossbeat::parseState(data, size);
        if (!parsed) return -1;
        parsed->midiTrigger = enabled;
        return write_state_result(*parsed, out, capacity);
    }
    catch (...) { return -1; }
}

// Batched editor-only thumbnail/slot-title renderer. One state parse per frame,
// following LYNX's 4x9 Gross Beat grids; never called from the audio thread.
int lynx_gb_state_thumbnails(const std::uint8_t* data, std::size_t size,
    float* out, int sampleCount, char* names, int nameBytes) noexcept
{
    if (data == nullptr || out == nullptr || names == nullptr ||
        sampleCount < 8 || sampleCount > 128 || nameBytes < 8 || nameBytes > 96)
        return -1;
    try
    {
        const auto parsed = lynx::grossbeat::parseState(data, size);
        if (!parsed) return -1;
        for (int i = 0; i < 72; ++i)
        {
            const auto& slot = parsed->slots[static_cast<std::size_t>(i)];
            const auto curve = lynx::grossbeat::renderSlot(slot, sampleCount);
            if (curve.size() != static_cast<std::size_t>(sampleCount)) return -1;
            // Some valid LYNX curve modes overshoot the normalized display rail.
            // Clamp preview pixels only; do not change the source state or audio DSP.
            auto* preview = out + static_cast<std::size_t>(i) * sampleCount;
            for (int j = 0; j < sampleCount; ++j)
            {
                const float level = curve[static_cast<std::size_t>(j)];
                if (!std::isfinite(level)) return -1; // nonfinite remains a real error
                preview[j] = std::clamp(level, 0.0f, 1.0f);
            }
            auto* dest = names + static_cast<std::size_t>(i) * nameBytes;
            std::memset(dest, 0, static_cast<std::size_t>(nameBytes));
            const auto length = std::min(slot.name.size(), static_cast<std::size_t>(nameBytes - 1));
            std::memcpy(dest, slot.name.data(), length);
        }
        return 72;
    }
    catch (...) { return -1; }
}

// Called from the editor thread. Parsing/allocation occurs here; the audio thread only
// consumes the completed snapshot and calls Engine::update(), whose slot storage is reserved.
bool lynx_gb_queue_state(void* instance, const std::uint8_t* data, std::size_t size) noexcept
{
    if (instance == nullptr || data == nullptr || size == 0) return false;
    auto& b = *static_cast<Bridge*>(instance);
    try
    {
        b.reclaimRetired();
        auto parsed = lynx::grossbeat::parseState(data, size);
        if (!parsed) return false;
        for (const auto& slot : parsed->slots)
            if (slot.points.empty() || slot.points.size() > static_cast<std::size_t>(lynx::grossbeat::maxEditPoints))
                return false;
        auto* next = new lynx::grossbeat::State(std::move(*parsed));
        auto* superseded = b.pendingState.exchange(next, std::memory_order_acq_rel);
        delete superseded;
        return true;
    }
    catch (...) { return false; }
}
}
