#pragma once

#include <array>
#include <cstdint>

namespace lynx {
constexpr int maxEffectParams = 192;
using EffectParams = std::array<float, maxEffectParams>;
using NoteMask = std::array<std::uint64_t, 2>;
struct Song;
} // namespace lynx
