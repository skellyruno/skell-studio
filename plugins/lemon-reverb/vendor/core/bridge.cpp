#include "core/Reeverb2.h"
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <new>
namespace {
struct SkellReverb2 {
    lynx::reeverb2::Engine engine;
    int rate, maxBlock;
    SkellReverb2(int sampleRate, int blockSize)
        : rate(std::clamp(sampleRate, 8000, 384000)), maxBlock(std::clamp(blockSize, 1, 65536)) {
        engine.reserve(rate, maxBlock);
        lynx::reeverb2::Settings defaults {};
        engine.prepare(rate, defaults, 120.0, true);
    }
};
}
extern "C" {
void* skell_reeverb2_new(int rate, int block) noexcept {
    try { return new SkellReverb2(rate, block); } catch (...) { return nullptr; }
}
void skell_reeverb2_delete(void* p) noexcept { delete static_cast<SkellReverb2*>(p); }
void skell_reeverb2_set(void* p, const std::int32_t* raw, int flags, double bpm) noexcept {
    if (!p || !raw) return;
    auto& b = *static_cast<SkellReverb2*>(p);
    lynx::reeverb2::Settings s {};
    for (int i=0;i<15;++i) s.p[static_cast<std::size_t>(i)] = raw[i];
    s.tempoPredelay = (flags & 1) != 0;
    s.midSide = (flags & 2) != 0;
    const double safeBpm = std::isfinite(bpm) && bpm > 1.0 ? std::clamp(bpm, 1.0, 1000.0) : 120.0;
    b.engine.set(s, safeBpm);
}
void skell_reeverb2_reset(void* p) noexcept {
    if (p) static_cast<SkellReverb2*>(p)->engine.flush();
}
void skell_reeverb2_process(void* p, float* l, float* r, int frames) noexcept {
    if (!p || !l || !r || frames <= 0) return;
    auto& b = *static_cast<SkellReverb2*>(p);
    int offset = 0;
    while (offset < frames) {
        const int count = std::min(b.maxBlock, frames - offset);
        b.engine.process(l + offset, r + offset, count);
        offset += count;
    }
}
}
