// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: AGPL-3.0-only
#include "core/FruityCompressor.h"

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <new>

namespace {
struct Compressor {
    lynx::fldyn::CompressorEngine engine;
    bool lookahead = true;
    explicit Compressor(int rate) { engine.prepare(std::max(rate, 1)); }
};
}

extern "C" {
void* lynx_fc_new(int rate) noexcept {
    try { return new Compressor(rate); } catch (...) { return nullptr; }
}
void lynx_fc_delete(void* p) noexcept { delete static_cast<Compressor*>(p); }
void lynx_fc_prepare(void* p, int rate) noexcept {
    if (p) static_cast<Compressor*>(p)->engine.prepare(std::max(rate, 1));
}
void lynx_fc_set(void* p, const int* values, bool lookahead) noexcept {
    if (!p || !values) return;
    auto& c = *static_cast<Compressor*>(p);
    std::array<int, lynx::fldyn::compressorControls> raw{};
    std::copy_n(values, raw.size(), raw.begin());
    c.engine.setControls(raw);
    c.lookahead = lookahead;
}
void lynx_fc_reset(void* p) noexcept {
    if (p) static_cast<Compressor*>(p)->engine.reset();
}
void lynx_fc_process(void* p, const float* input, const float* sidechain, float* output, int frames) noexcept {
    if (p && input && output && frames > 0)
        static_cast<Compressor*>(p)->engine.process(input, sidechain, output, frames);
}
int lynx_fc_latency(void* p) noexcept {
    return p && static_cast<Compressor*>(p)->lookahead ? 64 : 0;
}
}
