// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: AGPL-3.0-only
// Isolated Fruity Compressor DSP engine from LYNX SHARE; DAW host adapters intentionally excluded.
#pragma once
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdint>

namespace lynx::fldyn {
constexpr int compressorControls = 6;
inline constexpr std::array<int, compressorControls> compressorMin { -600, 4, -300, 0, 1, 0 };
inline constexpr std::array<int, compressorControls> compressorMax { 0, 300, 300, 4000, 4000, 7 };
inline constexpr std::array<int, compressorControls> compressorDefault { 0, 10, 0, 150, 200, 0 };

class CompressorEngine
{
public:
    void prepare (int rate) noexcept
    {
        sampleRate = std::max (rate, 1);
        reset();
    }

    void setControls (const std::array<int, compressorControls>& p) noexcept
    {
        if (p != raw)
        {
            raw = p;
            dirty = true;
        }
    }

    void reset() noexcept // also what the plugin does when it is created
    {
        outPos = gainPos = 0;
        releaseNow = 0.5f;
        gainRead = 1.0f;
        follower = 0.0f;
        smooth = 1.0f;
        limitGain = 1.0f;
        hold = 0;
        gainRing.fill (1.0f);
        outRing.fill (0.0f);
        ap.fill (1e-30f);
        dirty = true;
    }

    // interleaved stereo; side = interleaved side input or nullptr (own signal)
    void process (const float* in, const float* side, float* out, int n) noexcept
    {
        if (dirty)
            setup();
        const auto slopeK = slope;
        const auto inv = (float) (1.0 / (double) (kneeHigh - kneeLow));
        const auto vintageTop = (kneeHigh - lnThreshold) * slope;
        auto mixK = lnThreshold - kneeLow;
        if (mixK > 1.0f)
            mixK = 1.0f;

        for (int i = 0; i < n; ++i)
        {
            const auto xl = (float) ((double) in[2 * i] + 1e-30);
            const auto xr = (float) ((double) in[2 * i + 1] + 1e-30);
            ap[0] = (xl - ap[0]) * apCoef + ap[1];
            ap[1] = xl;
            const auto hl = (float) ((double) (xl - ap[0]) * 0.5);
            ap[2] = (xr - ap[2]) * apCoef + ap[3];
            ap[3] = xr;
            const auto hr = (float) ((double) (xr - ap[2]) * 0.5);

            float a = hl, b = hr;
            if (side != nullptr)
            {
                a = (float) ((double) side[2 * i] + 1e-30);
                b = (float) ((double) side[2 * i + 1] + 1e-30);
            }
            a = std::fabs (a);
            b = std::fabs (b);
            auto m = b > a ? b : a;
            m = (float) ((double) m + 1e-30);
            follower = m + (follower - m) * (m > follower ? fastAttack : fastRelease);

            auto g = 1.0f;
            const auto l = (float) std::log ((double) follower);
            if (l >= kneeLow)
            {
                if (vintage)
                {
                    if (l >= kneeHigh)
                        g = vintageCeiling;
                    else
                    {
                        const auto u = (l - kneeLow) * inv;
                        const auto t = (float) ((double) (u * u) * (3.0 - (double) u * 2.0));
                        const auto w = u + (t - u) * mixK;
                        g = (float) std::exp ((double) (vintageTop * w));
                    }
                }
                else if (l >= kneeHigh)
                    g = (float) std::exp ((double) ((l - lnThreshold) * slopeK));
                else
                {
                    const auto dd = l - kneeLow;
                    g = (float) std::exp ((double) (slopeK * dd * dd * kneeScale));
                }
            }
            g = g + (smooth - g) * (g > smooth ? releaseNow : attack);
            smooth = g;
            peakHold = g + (peakHold - g) * releaseSlow;
            if (g > peakHold)
                peakHold = g;
            auto r = peakHold - g;
            if (vintage)
                r = (float) ((0.707 - (double) g) * 1.414);
            auto rel = releaseBase + (releaseFast - releaseBase) * r;
            if (0.0f > rel)
                rel = 0.0f;
            releaseNow = rel;
            g *= makeup;
            const auto ol = hl * g, orr = hr * g;
            const auto mx = std::max (std::fabs (ol), std::fabs (orr));

            if ((double) mx > 1.0)
            {
                const auto lg = (float) (0.9885531 / (double) mx);
                if (limitGain > lg)
                {
                    limitGain = lg;
                    const auto from = gainRead;
                    if (hold < 1)
                        gainRing.fill (gainRead);
                    hold = 64;
                    limitSpeed = 0.0f;
                    for (int j = 0; j < 128; ++j)
                    {
                        auto idx = gainPos + j;
                        if (idx >= 128)
                            idx -= 128;
                        auto w = (float) ((double) (j + 1) / 64.0);
                        if ((double) w > 1.0)
                            w = 1.0f;
                        gainRead = from + (lg - from) * w * w * (3.0f - w * 2.0f);
                        if (gainRing[(std::size_t) idx] > gainRead)
                            gainRing[(std::size_t) idx] = gainRead;
                    }
                }
                else if (hold <= 0)
                {
                    limitSpeed = (float) ((double) limitSpeed + (0.09 - (double) limitSpeed) * 5e-05);
                    limitGain = limitGain + (lg - limitGain) * limitSpeed;
                }
            }
            else if (hold <= 0)
            {
                limitSpeed = (float) ((double) limitSpeed + (0.09 - (double) limitSpeed) * 5e-05);
                limitGain = (float) ((double) limitGain + (1.0 - (double) limitGain) * (double) limitSpeed);
            }
            if (hold > 0)
                --hold;

            gainRead = gainRing[(std::size_t) gainPos];
            gainRing[(std::size_t) gainPos] = limitGain;
            out[2 * i] = outRing[(std::size_t) outPos] * gainRead;
            out[2 * i + 1] = outRing[(std::size_t) outPos + 1] * gainRead;
            outRing[(std::size_t) outPos] = ol;
            outRing[(std::size_t) outPos + 1] = orr;
            if ((outPos += 2) >= 128)
                outPos = 0;
            if (++gainPos >= 128)
                gainPos = 0;
        }
    }

private:
    float coef (float ms) const noexcept // 0.1^(1/(ms·rate/1000))
    {
        if (ms == 0.0f)
            return 0.0f;
        auto t = ms * (float) sampleRate / 1000.0f;
        t = 1.0f / t;
        return (float) std::pow (0.1, (double) t);
    }

    void setup() noexcept
    {
        static constexpr int kneeDb[4] { 0, 6, 7, 15 };
        const auto [thr, ratio, gainRaw, att, rel, type] = raw;
        peakHold = 1.0f;
        fastAttack = coef (0.01f);
        fastRelease = coef (1.0f);
        attack = coef ((float) ((double) att / 10.0));
        const auto t3 = type & 3;
        vintage = t3 == 2;
        releaseFast = type >= 4 ? coef ((float) ((double) rel * 0.1)) : coef ((float) (double) rel);
        releaseBase = coef ((float) rel);
        releaseSlow = coef ((float) rel);
        threshold = (float) std::pow (10.0, (double) thr / 200.0);
        slope = (float) ((double) ratio / 10.0);
        makeup = (float) std::pow (10.0, (double) gainRaw / 200.0);
        const auto kneeLin = (float) std::pow (10.0, (double) -kneeDb[t3] / 20.0);
        slope = (float) (1.0 / (double) slope + -1.0);
        lnThreshold = (float) std::log ((double) threshold);
        kneeLow = (float) std::log ((double) (threshold * kneeLin));
        kneeHigh = (float) std::log ((double) (threshold / kneeLin));
        kneeScale = (float) (0.5 / (double) (kneeHigh - kneeLow));
        if (vintage)
        {
            kneeHigh = (float) ((double) lnThreshold + std::pow ((double) ratio / 10.0, 0.25) * 1.25);
            kneeScale = (float) (0.5 / (double) (kneeHigh - kneeLow));
            vintageCeiling = (float) std::exp ((double) ((kneeHigh - lnThreshold) * slope));
            kneeHigh = (float) std::log (std::exp ((double) kneeHigh) / (double) kneeLin);
            slope = (float) (std::log ((double) vintageCeiling) / (double) (kneeHigh - lnThreshold));
        }
        limitGain = 1.0f;
        hold = 0;
        limitSpeed = 0.01f;
        const auto t = (float) std::tan (9.42477796076938 / (double) sampleRate); // 1st-order all-pass at 1.5 Hz
        apCoef = (float) (((double) t - 1.0) / ((double) t + 1.0));
        dirty = false;
    }

    int sampleRate = 44100;
    std::array<int, compressorControls> raw = compressorDefault;
    bool dirty = true, vintage = false;
    float fastAttack = 0, fastRelease = 0, attack = 0, releaseFast = 0, releaseBase = 0, releaseSlow = 0;
    float threshold = 1, slope = 0, makeup = 1, lnThreshold = 0, kneeLow = 0, kneeHigh = 0, kneeScale = 0, vintageCeiling = 0;
    float apCoef = 0;
    std::array<float, 4> ap {};
    float follower = 0, smooth = 1, peakHold = 1, releaseNow = 0.5f;
    float limitGain = 1, limitSpeed = 0.01f, gainRead = 1;
    int hold = 0, outPos = 0, gainPos = 0;
    std::array<float, 128> gainRing {}, outRing {};
};

// ===================================================================================================================
} // namespace lynx::fldyn
