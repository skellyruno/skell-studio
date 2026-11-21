#pragma once

// The plate-type reverb engine of FL Studio's shared `Shared/Reverb_x64.dll` (clean-room; behaviour described in
// docs/analise/REA_rodada31.md §7.5, REA_rodada36 §1 and REA_rodada37 §1.18): mono input, +10⁻³⁰ anti-denormal,
// one-pole high cut, first-order all-pass low cut, four input diffusers, predelay line, a two-half "figure-eight" tank
// (feedback line × decay, colour shelf, + predelay output, damping one-pole, all-pass, delay × decay, all-pass,
// delay) and 7 output taps per channel, all in float32. The plugins that host it (the Harmless and Harmor racks,
// Ogun) compute the coefficients themselves (each with its own parameter laws) and fill the fields below; the
// per-sample routine is the shared one. (Sytrus keeps its own double-precision copy in SytrusEngine.cpp.)

#include <array>
#include <vector>

namespace lynx
{
struct PlateReverb
{
    static constexpr int lines[12] { 142, 107, 379, 277, 672, 4453, 1800, 3720, 908, 4217, 2656, 3163 };
    static constexpr int tapsL[7] { 266, 2974, 1913, 2496, 1690, 187, 1566 };
    static constexpr int tapsR[7] { 353, 3627, 1728, 3173, 1811, 335, 621 };
    static constexpr double colourFactor[5] { 0.2, 0.6, 1.0, 1.5, 3.0 };

    std::array<std::vector<float>, 12> buf;
    std::array<int, 12> L {}, P {};
    std::array<int, 7> tl {}, tr {};
    std::vector<float> pre;
    int pp = 0;
    // coefficients: high cut, low cut (0 = off), damping, input diffusers, tank all-passes, decay, colour shelf
    float cHc = 1, cLc = 0, cDamp = 1, cIn = 0, c1 = 0, c2 = 0, g = 0, c48 = 0, c44 = 0;
    float s10 = 0, s14 = 0, s18 = 0, s3c = 0, s40 = 0, s4c = 0, s50 = 0, s54 = 0, s58 = 0;

    // after L / pre lengths are set: zeroed lines
    void allocate (int predelay)
    {
        for (std::size_t i = 0; i < 12; ++i)
        {
            buf[i].assign ((std::size_t) L[i], 0.0f);
            P[i] = 0;
        }
        pre.assign ((std::size_t) (predelay < 1 ? 1 : predelay), 0.0f);
        pp = 0;
        s10 = s14 = s18 = s3c = s40 = s4c = s50 = s54 = s58 = 0.0f;
    }

    float ap (int i, float v, float c) noexcept
    {
        auto& p = P[(std::size_t) i];
        auto& b = buf[(std::size_t) i];
        const auto w = b[(std::size_t) p] - c * v;
        b[(std::size_t) p] = c * w + v;
        p = p > 0 ? p - 1 : L[(std::size_t) i] - 1;
        return w;
    }

    float dl (int i, float v) noexcept
    {
        auto& p = P[(std::size_t) i];
        auto& b = buf[(std::size_t) i];
        const auto r = b[(std::size_t) p];
        b[(std::size_t) p] = v;
        p = p > 0 ? p - 1 : L[(std::size_t) i] - 1;
        return r;
    }

    float tap (int i, int off) const noexcept
    {
        const auto k = off + P[(std::size_t) i];
        return buf[(std::size_t) i][(std::size_t) (k >= L[(std::size_t) i] ? k - L[(std::size_t) i] : k)];
    }

    // x: mono input (n samples); writes the wet stereo output (not scaled)
    void process (const float* x, float* outL, float* outR, int n) noexcept
    {
        const auto t1 = 1.0f - g;
        const auto w74 = t1 * 0.6f + g, w6c = t1 * 0.25f + g, w78 = t1 * 0.7f + g, w68 = t1 * 0.2f + g, w70 = t1 * 0.5f + g;
        for (int i = 0; i < n; ++i)
        {
            auto a = ((x[i] + 1.0e-30f) - s10) * cHc + s10;
            s10 = a;
            const auto h = (a - s14) * cLc + s18;
            s18 = a, s14 = h;
            if (cLc != 0.0f)
                a = (a - h) * 0.5f + 1.0e-30f;
            for (int k = 0; k < 4; ++k)
                a = ap (k, a, cIn);
            const auto z = pre[(std::size_t) pp];
            pre[(std::size_t) pp] = a;
            pp = pp > 0 ? pp - 1 : (int) pre.size() - 1;
            // half 1
            auto u = buf[11][(std::size_t) P[11]] * g;
            auto f = (u - s50) * c44 + s4c;
            s4c = u, s50 = f;
            auto v = (((f + u) * c48 + u + z) - s3c) * cDamp + s3c;
            s3c = v;
            auto w = ap (4, v, c1);
            auto r = dl (5, w) * g;
            auto w2 = ap (6, r, c2);
            buf[7][(std::size_t) P[7]] = w2;
            P[7] = P[7] > 0 ? P[7] - 1 : L[7] - 1;
            // half 2
            u = buf[7][(std::size_t) P[7]] * g;
            f = (u - s58) * c44 + s54;
            s54 = u, s58 = f;
            v = (((f + u) * c48 + u + z) - s40) * cDamp + s40;
            s40 = v;
            w = ap (8, v, c1);
            r = dl (9, w) * g;
            w2 = ap (10, r, c2);
            buf[11][(std::size_t) P[11]] = w2;
            P[11] = P[11] > 0 ? P[11] - 1 : L[11] - 1;
            outL[i] = (((tap (9, tl[0]) - w6c * tap (9, tl[1])) - w78 * tap (10, tl[2])) - w68 * tap (11, tl[3]))
                      + w74 * tap (5, tl[4]) + tap (6, tl[5]) + w70 * tap (7, tl[6]);
            outR[i] = ((((w6c * tap (5, tr[1]) + tap (5, tr[0])) - w78 * tap (6, tr[2])) - w68 * tap (7, tr[3]))
                       - w74 * tap (9, tr[4])) + tap (10, tr[5]) + w70 * tap (11, tr[6]);
        }
    }
};
} // namespace lynx
