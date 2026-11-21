// Copyright (C) 2026 ItsAlphaNeon
// SPDX-License-Identifier: AGPL-3.0-only
// Adapted from LYNX SHARE 0.1.30, commit 88ec555ea2bce9714c92707cc06a5abfd317c900. The LimiterEngine body and DSP helpers are from the pinned source.

#include <algorithm>
#include <array>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <initializer_list>
#include <limits>
#include <vector>

namespace lynx::fldyn
{
constexpr int limiterControls = 18;
const std::array<int, limiterControls> limiterMin { 0, 0, 0, 0, 1, 0, 1, 0, 0, -1000, -1000, 0, 0, 1, 0, 0, 0, 0 };
const std::array<int, limiterControls> limiterMax { 2000, 1000, 2000, 10000, 8, 10000, 8, 10000, 1000, 1000, 1000, 10000, 10000, 8, 10000, 1000, 1000, 10000 };
const std::array<int, limiterControls> limiterDefault { 1000, 1000, 1000, 1115, 3, 5000, 3, 2882, 1000, 0, 0, 0, 6666, 1, 650, 1000, 0, 4187 };

namespace
{
std::int32_t i32 (const std::vector<std::uint8_t>& d, std::size_t at)
{
    std::int32_t v = 0;
    std::memcpy (&v, d.data() + at, 4);
    return v;
}

inline std::int32_t fbits (float f) noexcept
{
    std::int32_t b;
    std::memcpy (&b, &f, 4);
    return b;
}

inline int roundEven (double x) noexcept { return (int) std::nearbyint (x); } // cvtsd2si, round to nearest even

const double ln501 = std::log (501.0);
const double lnHundredth = std::log (0.01);
const float ln7f = (float) std::log (7.0);
const float ln3f = (float) std::log (3.0);

// FL's normalised exponential laws on a raw control (scale 1000): (k^x − 1)/(k − 1)
float law7 (int v) noexcept
{
    const auto x = (float) v * 0.001f;
    if (x == 1.0f)
        return 1.0f;
    return (float) ((std::exp ((double) (x * ln7f)) - 1.0) * (double) (1.0f / 6.0f));
}

float law3 (int v) noexcept
{
    const auto x = (float) v * 0.001f;
    if (x == 1.0f)
        return 1.0f;
    return (float) ((std::exp ((double) (x * ln3f)) - 1.0) * (double) 0.5f);
}

// Limiter time law in samples: 2·(501^(v/10000) − 1) ms
double timeSamples (int v, int sampleRate) noexcept
{
    const auto e = std::exp ((double) ((float) v * 1e-4f) * ln501);
    return (double) (float) (e - 1.0) * 2.0 * 0.001 * (double) sampleRate;
}

// --- one-pole curve stage (attack coefficient when rising, release when falling) ------------------------------------

struct Stage
{
    double attack = 0.0, release = 0.0, state = 0.0;
    bool single = false; // both coefficients equal: one coefficient for both directions

    void set (double a, double b) noexcept
    {
        attack = a <= 0.0 ? 0.0 : std::exp (lnHundredth / std::max (a, 1.0));
        release = b <= 0.0 ? 0.0 : std::exp (lnHundredth / std::max (b, 1.0));
        single = attack == release;
    }

    void run (float* buf, int n) noexcept
    {
        const long double a = attack, r = release;
        long double y = state;
        for (int i = 0; i < n; ++i)
        {
            const long double x = buf[i];
            const auto c = (single || x >= y) ? a : r;
            y = x + (y - x) * c;
            buf[i] = (float) y;
        }
        state = std::fabs (y) > (long double) 5.960464477539063e-08f ? (double) y : 0.0;
    }
};

// A chain of `count` stages over the block, the same as running them one after another (each stage's float output is
// the next one's input), but sample by sample: the stages' recurrences overlap instead of running back to back.
template <int count>
void runChainN (Stage* st, float* buf, int n) noexcept
{
    // coefficient table per stage: [falling, rising] (both the attack for a single-coefficient stage), picked by
    // index rather than by a branch on the signal
    long double y[count];
    double c[count][2];
    for (int s = 0; s < count; ++s)
    {
        y[s] = st[s].state;
        c[s][0] = st[s].single ? st[s].attack : st[s].release;
        c[s][1] = st[s].attack;
    }
    for (int i = 0; i < n; ++i)
    {
        float v = buf[i];
        for (int s = 0; s < count; ++s)
        {
            const long double x = v;
            const long double k = c[s][x >= y[s] ? 1 : 0];
            y[s] = x + (y[s] - x) * k;
            v = (float) y[s];
        }
        buf[i] = v;
    }
    for (int s = 0; s < count; ++s)
        st[s].state = std::fabs (y[s]) > (long double) 5.960464477539063e-08f ? (double) y[s] : 0.0;
}

// A block whose samples all equal every stage's state (a gain curve resting at a constant gain) is a fixed point of
// the chain: each step gives y = x + (x − x)·c = x, the float output is the input and the states stay as they are.
bool chainAtRest (const Stage* st, int count, const float* buf, int n) noexcept
{
    if (n <= 0)
        return false;
    const float v = buf[0];
    if (! (std::fabs ((long double) v) > (long double) 5.960464477539063e-08f)) // (the states would be flushed to 0)
        return false;
    for (int s = 0; s < count; ++s)
        if (! ((long double) st[s].state == (long double) v))
            return false;
    const auto bits = fbits (v);
    for (int i = 1; i < n; ++i)
        if (fbits (buf[i]) != bits)
            return false;
    return true;
}

void runChain (Stage* st, int count, float* buf, int n) noexcept
{
    if (count > 0 && chainAtRest (st, count, buf, n))
        return;
    switch (count)
    {
        case 0: return;
        case 1: st[0].run (buf, n); return;
        case 2: runChainN<2> (st, buf, n); return;
        case 3: runChainN<3> (st, buf, n); return;
        case 4: runChainN<4> (st, buf, n); return;
        default:
            runChainN<4> (st, buf, n);
            runChain (st + 4, count - 4, buf, n);
            return;
    }
}

// --- limiter peak window: running maximum over the window, kept on the bit patterns ---------------------------------

struct PeakWindow
{
    std::vector<std::int32_t> ring, queue;
    int size = 0, pos = 0, head = 1, tail = 0, count = 0;

    void allocate (int maxSize)
    {
        ring.assign ((std::size_t) std::max (maxSize, 1), 0);
        queue.assign (ring.size(), 0);
    }

    void setSize (int w) noexcept
    {
        w = std::clamp (w, 1, (int) ring.size());
        if (w == size)
            return;
        size = w;
        std::fill (ring.begin(), ring.begin() + w, 0); // fresh memory
        std::fill (queue.begin(), queue.begin() + w, 0);
        reset();
    }

    void reset() noexcept
    {
        ring[0] = 0;
        pos = 0;
        head = 1;
        tail = 0;
        count = 0;
    }

    // (round 190kr: the positions and counters as locals, written back after the block: the int members could otherwise
    // alias the int ring / queue being written and be stored / re-read at every sample. The same steps.)
    void run (float* buf, int n) noexcept
    {
        const int w = size;
        if (w < 2)
            return;
        std::int32_t* const rg = ring.data();
        std::int32_t* const qu = queue.data();
        int ps = pos, hd = head, tl = tail, ct = count;
        for (int i = 0; i < n; ++i)
        {
            const auto x = fbits (buf[i]);
            const int prev = ps;
            if (++ps >= w)
                ps = 0;
            const auto ref = ct > 0 ? rg[qu[hd]] : rg[prev];
            if (x > rg[prev])
            {
                while (ct > 0)
                {
                    if (x > rg[qu[tl]])
                    {
                        --ct;
                        if (--tl < 0)
                            tl = w - 1;
                        continue;
                    }
                    const auto t = qu[hd] - 1;
                    if (ps == t || ps == t + w)
                    {
                        --ct;
                        if (++hd >= w)
                            hd = 0;
                    }
                    break;
                }
            }
            else
            {
                if (++tl >= w)
                    tl = 0;
                qu[tl] = prev;
                const auto t = qu[hd] - 1;
                if (ps == t || ps == t + w)
                {
                    if (++hd >= w)
                        hd = 0;
                }
                else
                    ++ct;
            }
            rg[ps] = x;
            const auto y = x < ref ? ref : x;
            std::memcpy (buf + i, &y, 4);
        }
        pos = ps, head = hd, tail = tl, count = ct;
    }
};

// --- limiter compression detector: RMS of L² + R² over the window -------------------------------------------------

struct RmsWindow
{
    std::vector<float> ring;
    int size = 0, pos = 0;
    float scale = 1.0f;
    double sum = 0.0;

    void allocate (int maxSize) { ring.assign ((std::size_t) std::max (maxSize, 1), 0.0f); }

    void setSize (int w) noexcept
    {
        w = std::clamp (w, 1, (int) ring.size());
        if (w == size)
            return;
        size = w;
        scale = (float) (1.0 / (double) (w * 2));
        reset();
    }

    void reset() noexcept
    {
        std::fill (ring.begin(), ring.begin() + std::max (size, 1), 0.0f);
        pos = 0;
        sum = 0.0;
    }

    void run (const float* stereo, float* out, int n) noexcept
    {
        long double s = sum;
        const long double k = scale;
        for (int i = 0; i < n; ++i)
        {
            const auto l = stereo[2 * i], r = stereo[2 * i + 1];
            const auto sq = l * l + r * r;
            if (++pos >= size)
                pos = 0;
            s = (s - (long double) ring[(std::size_t) pos]) + (long double) sq;
            ring[(std::size_t) pos] = sq;
            const auto v = (float) (k * s);
            out[i] = std::sqrt (v < 0.0f ? 0.0f : v);
        }
        sum = (double) s;
    }
};

// ===================================================================================================================
// Fruity Limiter

class LimiterEngine
{
public:
    static constexpr int maxBlock = 512;

    void prepare (int rate)
    {
        sampleRate = std::max (rate, 1);
        const auto maxWindow = sampleRate + 2; // the time law tops at one second of samples
        peak.allocate (maxWindow);
        rms.allocate (maxWindow);
        delay.assign ((std::size_t) maxWindow * 2, 0.0f);
        level.assign (maxBlock, 0.0f);
        gain.assign (maxBlock, 0.0f);
        detect.assign (maxBlock, 0.0f);
        work.assign (maxBlock * 2, 0.0f);
        side.assign (maxBlock * 2, 0.0f);
        lookahead = -1;
        peak.size = rms.size = 0;
        configured = false;
    }

    void setControls (const std::array<int, limiterControls>& p, bool exact) noexcept
    {
        if (configured && p == raw && exact == exactCeiling)
            return;
        // ProcessParam groups: gains / times / release & compressor curve / threshold / ratio / knee
        const auto changed = [&] (std::initializer_list<int> idx) {
            if (! configured)
                return true;
            for (auto i : idx)
                if (p[(std::size_t) i] != raw[(std::size_t) i])
                    return true;
            return false;
        };
        const auto gainsChanged = changed ({ 0, 1, 2, 15, 16 }) || exact != exactCeiling;
        const auto timesChanged = changed ({ 3, 4, 7, 14 });
        const auto curvesChanged = changed ({ 5, 6, 11, 12, 13, 17 });
        const auto thresholdChanged = changed ({ 8 });
        const auto ratioChanged = changed ({ 9 });
        const auto kneeChanged = changed ({ 10 });
        raw = p;
        exactCeiling = exact;
        if (gainsChanged)
            setGains();
        if (thresholdChanged)
            setThreshold();
        if (ratioChanged)
            setRatio();
        if (kneeChanged)
            setKnee();
        if (timesChanged || ! configured)
            setTimes(); // includes the curves
        else if (curvesChanged)
            setCurves();
        if (! configured)
            reset();
        configured = true;
    }

    void reset() noexcept
    {
        for (int i = 0; i < gainStages; ++i)
            gainCurve[(std::size_t) i].state = 1.0;
        for (int i = 0; i < releaseStages; ++i)
            releaseCurve[(std::size_t) i].state = 0.0;
        for (int i = 0; i < compStages; ++i)
            compCurve[(std::size_t) i].state = 0.0;
        gateGain = 1.0f;
        std::fill (delay.begin(), delay.end(), 0.0f);
        delayPos = 0;
        silentFor = std::numeric_limits<int>::min() / 2 + 1; // 0xC0000001
    }

    int latency() const noexcept { return std::max (lookahead, 0); }

    // interleaved stereo in place; sideIn = interleaved side input or nullptr (own input)
    void process (float* io, const float* sideIn, int n) noexcept
    {
        // silence: after `holdSamples` of input at or below 1e-6 the plugin passes the input through untouched
        bool silent = true;
        for (int i = 0; i < 2 * n && silent; ++i)
            silent = std::fabs (io[i]) <= 1e-6f; // NaN counts as signal
        if (silent)
            silentFor -= n;
        else
            silentFor = holdSamples;
        if (silentFor <= -n)
        {
            if (silentFor > minSilent)
                reset();
            silentFor = minSilent;
            return;
        }

        auto* lv = level.data();
        for (int i = 0; i < n; ++i)
            lv[i] = std::max (std::fabs (io[2 * i]), std::fabs (io[2 * i + 1]));
        peak.run (lv, n);

        std::fill (gain.begin(), gain.begin() + n, gainLin);
        if (compOn)
        {
            rms.run (sideIn != nullptr ? sideIn : io, detect.data(), n);
            runChain (compCurve.data(), compStages, detect.data(), n);
            const auto thr = fbits (compThreshold);
            for (int i = 0; i < n; ++i)
                if (fbits (detect[(std::size_t) i]) > thr)
                    gain[(std::size_t) i] = compressorGain (detect[(std::size_t) i]);
        }

        // look-ahead: the audio is delayed by the attack time
        if (lookahead > 0)
            for (int i = 0; i < n; ++i)
            {
                std::swap (io[2 * i], delay[(std::size_t) delayPos * 2]);
                std::swap (io[2 * i + 1], delay[(std::size_t) delayPos * 2 + 1]);
                if (++delayPos >= lookahead)
                    delayPos = 0;
            }

        runChain (releaseCurve.data(), releaseStages, lv, n);

        const auto limitBits = fbits (limitLevel), gateBits = fbits (gateThreshold);
        for (int i = 0; i < n; ++i)
        {
            const auto l = lv[i];
            const auto g = gain[(std::size_t) i];
            auto r = fbits (l * g) > limitBits ? limitLevel / l : g;
            if (fbits (l) < gateBits)
                r = gateFloor + (gateGain - gateFloor) * gateRelease;
            gateGain = r;
            lv[i] = r;
        }
        runChain (gainCurve.data(), gainStages, lv, n);

        for (int i = 0; i < n; ++i)
        {
            io[2 * i] *= lv[i];
            io[2 * i + 1] *= lv[i];
        }

        if (saturate)
        {
            const long double t = satThreshold, r = satRange, k = satScale;
            const auto tb = fbits (satThreshold);
            for (int i = 0; i < 2 * n; ++i)
            {
                const auto a = std::fabs (io[i]);
                if (fbits (a) <= tb)
                    continue;
                const auto e = std::exp ((t - (long double) a) * k);
                const auto y = (float) ((1.0L - e) * r + t);
                io[i] = std::signbit (io[i]) ? -y : y;
            }
        }
        else
            for (int i = 0; i < 2 * n; ++i)
                if (std::fabs (io[i]) > ceiling)
                    io[i] = std::signbit (io[i]) ? -ceiling : ceiling;
    }

    std::vector<float> work, side;

    // Skell adapter: the plugin keeps the original gate/sleep status available without touching internal state elsewhere.
    bool asleep() const noexcept { return silentFor <= minSilent; }

private:
    static constexpr int minSilent = std::numeric_limits<int>::min() / 2 + 1;

    float compressorGain (float level) const noexcept
    {
        const auto v = level > compClamp ? compClamp : level;
        auto over = (float) std::log10 ((double) v);
        over *= 20.0f;
        over += -compThresholdDb;
        float d;
        if (knee != 0)
        {
            auto c = over + 1.0f;
            if (knee > 0)
            {
                c *= kneeAmount;
                c += 1.0f;
                c = (float) std::log ((double) c);
                c *= kneeScale;
            }
            else
            {
                c *= kneeScale;
                c = (float) std::exp ((double) c);
                c += -1.0f;
                c *= kneeInverse;
            }
            c += -1.0f;
            c *= ratioFactor;
            d = c - over;
        }
        else
            d = over * ratioSlope;
        const auto e = d * 0.115129255f; // ln 10 / 20
        const auto g = (float) std::exp ((double) e) * gainLin;
        // (a negative knee with a tiny amount overflows the curve to inf; at Gain −inf that was inf · 0 = NaN: round 111)
        return std::isfinite (g) ? g : (gainLin > 0.0f ? std::numeric_limits<float>::max() : 0.0f);
    }

    void setGains() noexcept
    {
        gainLin = law7 (raw[0]);
        ceiling = law3 (raw[2]);
        limitLevel = ceiling * (exactCeiling ? 1.0f : 0.975f);
        gateFloor = law7 (raw[15]);
        const auto gateOpen = law7 (raw[16]);
        gateThreshold = limitLevel <= gateOpen ? limitLevel : gateOpen;
        const auto sat = law3 (raw[1]);
        saturate = (double) sat < 1.0;
        if (saturate)
        {
            const auto c = ceiling < 1.0f ? ceiling : 1.0f;
            satThreshold = sat;
            satRange = c - sat;
            if (! (satRange > 0.0f))
                satRange = 1.52587890625e-05f;
            satScale = 1.0f / satRange;
        }
    }

    void setThreshold() noexcept
    {
        compOn = raw[8] < 1000;
        if (compOn)
        {
            const auto t = law7 (raw[8]);
            compThreshold = t <= 0.001f ? 0.001f : t; // floor −60 dB
        }
        else
            compThreshold = 100.0f;
        compThresholdDb = fbits (compThreshold) > 0 ? (float) (std::log10 ((double) compThreshold) * 20.0)
                                                     : -std::numeric_limits<float>::infinity();
        compClamp = compThreshold * 100.0f;
    }

    void setRatio() noexcept
    {
        const auto v = raw[9];
        const auto e = std::exp ((double) ((float) std::abs (v) * 0.001f) * std::log (51.0));
        ratioFactor = v < 1 ? (float) ((double) (float) (e - 1.0) * 0.06 + 1.0) : (float) (1.0 / ((double) (float) (e - 1.0) * 0.38 + 1.0));
        ratioSlope = ratioFactor - 1.0f;
    }

    void setKnee() noexcept
    {
        const auto v = raw[10];
        knee = v == 0 ? 0 : (v > 0 ? 1 : -1);
        const auto e = std::exp ((double) std::fabs ((float) v * 0.001f) * std::log (201.0));
        if (v < 1)
        {
            kneeAmount = (float) ((double) (float) (e - 1.0) * 0.001);
            kneeScale = (float) std::log ((double) (kneeAmount + 1.0f));
            kneeInverse = 1.0f / kneeAmount;
        }
        else
        {
            kneeAmount = (float) ((double) (float) (e - 1.0) * 0.005);
            kneeScale = (float) (1.0 / std::log ((double) (kneeAmount + 1.0f)));
        }
    }

    void setTimes() noexcept
    {
        const auto la = std::clamp (roundEven (timeSamples (raw[3], sampleRate)), 0, (int) delay.size() / 2);
        if (la != lookahead)
        {
            lookahead = la;
            std::fill (delay.begin(), delay.end(), 0.0f);
            delayPos = 0;
        }
        const auto window = std::max ({ la, 1, roundEven (timeSamples (raw[7], sampleRate)) });
        peak.setSize (window);
        rms.setSize (std::max (roundEven (timeSamples (raw[14], sampleRate)), 1));
        if (la < 1)
            gainStages = 0;
        else
        {
            const auto count = std::clamp (raw[4], 1, 8);
            if (count != gainStages)
            {
                gainStages = count;
                for (int i = 0; i < count; ++i)
                    gainCurve[(std::size_t) i].state = 1.0;
            }
            const auto c = (double) ((float) la * 0.8f) / std::sqrt ((double) count);
            for (int i = 0; i < count; ++i)
                gainCurve[(std::size_t) i].set (c, c);
        }
        setCurves();
        holdSamples = lookahead + roundEven ((double) sampleRate * 1.5);
    }

    void setCurves() noexcept
    {
        const auto release = timeSamples (raw[5], sampleRate);
        if (release <= 0.0)
            releaseStages = 0;
        else
        {
            const auto count = std::clamp (raw[6], 1, 8);
            if (count != releaseStages)
            {
                releaseStages = count;
                for (int i = 0; i < count; ++i)
                    releaseCurve[(std::size_t) i].state = 0.0;
            }
            const auto b = release * 2.0 / std::sqrt ((double) count);
            for (int i = 0; i < count; ++i)
                releaseCurve[(std::size_t) i].set (0.0, b);
        }
        const auto count = std::clamp (raw[13], 1, 8);
        if (count != compStages)
        {
            compStages = count;
            for (int i = 0; i < count; ++i)
                compCurve[(std::size_t) i].state = 0.0;
        }
        const auto k = 1.0 / std::sqrt ((double) count);
        const auto a = timeSamples (raw[11], sampleRate) * k;
        const auto b = timeSamples (raw[12], sampleRate) * 2.0 * k;
        for (int i = 0; i < count; ++i)
            compCurve[(std::size_t) i].set (a, b);
        const auto gate = std::max (timeSamples (raw[17], sampleRate), 1.0) * 2.0;
        gateRelease = (float) std::exp (lnHundredth / gate);
        gateGain = 1.0f;
    }

    int sampleRate = 44100;
    std::array<int, limiterControls> raw = limiterDefault;
    bool exactCeiling = false, configured = false;

    float gainLin = 1.0f, ceiling = 1.0f, limitLevel = 0.975f, gateFloor = 1.0f, gateThreshold = 0.0f;
    bool saturate = false;
    float satThreshold = 1.0f, satRange = 1.0f, satScale = 1.0f;
    bool compOn = false;
    float compThreshold = 100.0f, compThresholdDb = 40.0f, compClamp = 1e4f;
    float ratioFactor = 1.0f, ratioSlope = 0.0f;
    int knee = 0;
    float kneeAmount = 0.0f, kneeScale = 0.0f, kneeInverse = 0.0f;
    float gateRelease = 0.0f, gateGain = 1.0f;

    int lookahead = -1, delayPos = 0, holdSamples = 0, silentFor = minSilent;
    PeakWindow peak;
    RmsWindow rms;
    std::array<Stage, 8> gainCurve {}, releaseCurve {}, compCurve {};
    int gainStages = 0, releaseStages = 0, compStages = 0;
    std::vector<float> delay, gain, detect, level;
};


} // namespace
} // namespace lynx::fldyn

namespace
{
struct SkellLimiterBridge
{
    lynx::fldyn::LimiterEngine engine;
    explicit SkellLimiterBridge(int sampleRate)
    {
        engine.prepare(std::clamp(sampleRate, 8000, 384000));
    }
};
}

extern "C"
{
void* skell_limiter_new(int sampleRate) noexcept
{
    try { return new SkellLimiterBridge(sampleRate); }
    catch (...) { return nullptr; }
}

void skell_limiter_delete(void* instance) noexcept
{
    delete static_cast<SkellLimiterBridge*>(instance);
}

void skell_limiter_set(void* instance, const std::int32_t* raw, bool exactCeiling) noexcept
{
    if (instance == nullptr || raw == nullptr) return;
    std::array<int, lynx::fldyn::limiterControls> values {};
    for (int i = 0; i < lynx::fldyn::limiterControls; ++i)
        values[static_cast<std::size_t>(i)] = raw[i];
    static_cast<SkellLimiterBridge*>(instance)->engine.setControls(values, exactCeiling);
}

void skell_limiter_reset(void* instance) noexcept
{
    if (instance != nullptr) static_cast<SkellLimiterBridge*>(instance)->engine.reset();
}

int skell_limiter_latency(void* instance) noexcept
{
    if (instance == nullptr) return 0;
    return static_cast<SkellLimiterBridge*>(instance)->engine.latency();
}

bool skell_limiter_asleep(void* instance) noexcept
{
    return instance == nullptr || static_cast<SkellLimiterBridge*>(instance)->engine.asleep();
}

// Input and optional sidechain are non-interleaved at the Rust/NIH-plug boundary. The engine owns
// two interleaved scratch buffers from prepare(), so this conversion never allocates.
void skell_limiter_process(void* instance, float* left, float* right,
                           const float* sideLeft, const float* sideRight, int frames) noexcept
{
    if (instance == nullptr || left == nullptr || right == nullptr || frames <= 0) return;
    auto& engine = static_cast<SkellLimiterBridge*>(instance)->engine;
    float* io = engine.work.data();
    float* side = engine.side.data();
    int offset = 0;
    while (offset < frames)
    {
        const int count = std::min(512, frames - offset);
        for (int i = 0; i < count; ++i)
        {
            io[2 * i] = left[offset + i];
            io[2 * i + 1] = right[offset + i];
            if (sideLeft != nullptr && sideRight != nullptr)
            {
                side[2 * i] = sideLeft[offset + i];
                side[2 * i + 1] = sideRight[offset + i];
            }
        }
        engine.process(io, sideLeft != nullptr && sideRight != nullptr ? side : nullptr, count);
        for (int i = 0; i < count; ++i)
        {
            left[offset + i] = io[2 * i];
            right[offset + i] = io[2 * i + 1];
        }
        offset += count;
    }
}
}
