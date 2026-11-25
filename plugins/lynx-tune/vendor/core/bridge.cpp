#include "core/LynxTune.h"
#include "core/Effects.h"

#include <algorithm>
#include <array>
#include <cstdint>
#include <memory>
#include <new>

namespace {
struct TuneInstance {
    lynx::EffectParams params{};
    std::unique_ptr<lynx::Effect> effect;
    std::array<lynx::NoteMask, 16> midiChannels{};
    lynx::NoteMask held{};
};
void mergeHeld(TuneInstance& instance) noexcept {
    instance.held = {};
    for (const auto& channel : instance.midiChannels) {
        instance.held[0] |= channel[0];
        instance.held[1] |= channel[1];
    }
}
}

extern "C" {
void skell_lynx_tune_set(void* opaque, const int* values, int count) noexcept;

void* skell_lynx_tune_new(int sampleRate, const int* values, int count) noexcept {
    try {
        auto instance = std::make_unique<TuneInstance>();
        const auto& info = lynx::lynxtune::info();
        for (std::size_t i = 0; i < info.params.size() && i < instance->params.size(); ++i)
            instance->params[i] = info.params[i].defaultValue;
        // Host MIDI from all 16 input channels is merged into the first held-note source.
        instance->params[lynx::lynxtune::midiChannel] = 1.0f / 128.0f;
        // Set restored controls before construction/prepare so the voice range
        // determines the host's initial PDC, not only the first process block.
        skell_lynx_tune_set(instance.get(), values, count);
        instance->effect = lynx::lynxtune::make(&instance->params);
        if (!instance->effect)
            return nullptr;
        instance->effect->prepare(static_cast<double>(std::clamp(sampleRate, 8000, 192000)));
        return instance.release();
    } catch (...) {
        return nullptr;
    }
}

void skell_lynx_tune_delete(void* opaque) noexcept {
    delete static_cast<TuneInstance*>(opaque);
}

void skell_lynx_tune_prepare(void* opaque, int sampleRate) noexcept {
    auto* instance = static_cast<TuneInstance*>(opaque);
    if (instance && instance->effect)
        instance->effect->prepare(static_cast<double>(std::clamp(sampleRate, 8000, 192000)));
}

void skell_lynx_tune_set(void* opaque, const int* values, int count) noexcept {
    auto* instance = static_cast<TuneInstance*>(opaque);
    if (!instance || !values || count < 8)
        return;
    auto& p = instance->params;
    p[lynx::lynxtune::key] = static_cast<float>(std::clamp(values[0], 0, 11)) / 11.0f;
    p[lynx::lynxtune::scale] = static_cast<float>(std::clamp(values[1], 0, 13)) / 13.0f;
    p[lynx::lynxtune::retune] = static_cast<float>(std::clamp(values[2], 0, 1000)) / 1000.0f;
    p[lynx::lynxtune::humanize] = static_cast<float>(std::clamp(values[3], 0, 1000)) / 1000.0f;
    p[lynx::lynxtune::formantShift] = static_cast<float>(std::clamp(values[4], -700, 700) + 700) / 1400.0f;
    p[lynx::lynxtune::transpose] = static_cast<float>(std::clamp(values[5], -12, 12) + 12) / 24.0f;
    p[lynx::lynxtune::mix] = static_cast<float>(std::clamp(values[6], 0, 1000)) / 1000.0f;
    p[lynx::lynxtune::range] = static_cast<float>(std::clamp(values[7], 0, 3)) / 3.0f;
}

void skell_lynx_tune_note(void* opaque, bool on, int channel, int note) noexcept {
    auto* instance = static_cast<TuneInstance*>(opaque);
    if (!instance || channel < 0 || channel >= static_cast<int>(instance->midiChannels.size()) || note < 0 || note > 127)
        return;
    auto& word = instance->midiChannels[static_cast<std::size_t>(channel)][static_cast<std::size_t>(note >> 6)];
    const auto bit = std::uint64_t{1} << (note & 63);
    if (on)
        word |= bit;
    else
        word &= ~bit;
}

void skell_lynx_tune_reset(void* opaque) noexcept {
    auto* instance = static_cast<TuneInstance*>(opaque);
    if (!instance || !instance->effect)
        return;
    instance->midiChannels = {};
    instance->held = {};
    instance->effect->reset();
}

void skell_lynx_tune_process(void* opaque, float* left, float* right, int frames) noexcept {
    auto* instance = static_cast<TuneInstance*>(opaque);
    if (!instance || !instance->effect || !left || !right || frames <= 0)
        return;
    mergeHeld(*instance);
    instance->effect->setHeldNotes(&instance->held, 1);
    instance->effect->process(left, right, frames, instance->params, 120.0);
}

int skell_lynx_tune_latency(const void* opaque) noexcept {
    const auto* instance = static_cast<const TuneInstance*>(opaque);
    return instance && instance->effect ? std::max(0, instance->effect->latencySamples()) : 0;
}
}
