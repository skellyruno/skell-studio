// Minimal host-model shim for the isolated LYNX Parametric EQ DSP.
#pragma once
#include <array>
namespace lynx {
constexpr int maxEffectParams = 192;
using EffectParams = std::array<float, maxEffectParams>;
}
