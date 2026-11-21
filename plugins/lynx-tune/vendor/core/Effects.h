#pragma once

#include <memory>
#include <string>
#include <vector>
#include "core/Song.h"

namespace lynx {
struct EffectParamInfo {
    const char* name;
    float defaultValue;
};
struct EffectInfo {
    const char* name;
    std::vector<EffectParamInfo> params;
};
class Effect {
public:
    virtual ~Effect() = default;
    virtual void prepare(double sampleRate) noexcept = 0;
    virtual void reset() noexcept = 0;
    virtual void process(float* left, float* right, int frames, const EffectParams& params, double bpm) noexcept = 0;
    virtual void setHeldNotes(const NoteMask* channels, int count) noexcept { (void) channels; (void) count; }
    virtual bool followsHeldNotes() const noexcept { return false; }
    virtual bool followsHeldNotesWith(const EffectParams&) const noexcept { return followsHeldNotes(); }
    virtual float controlOutput(int output) const noexcept { (void) output; return 0.0f; }
    virtual int latencySamples() const noexcept { return 0; }
};
} // namespace lynx
