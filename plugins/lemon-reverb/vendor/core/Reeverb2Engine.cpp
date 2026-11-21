#include "core/Reeverb2.h"

// Fruity Reeverb 2 (docs/analise/REA_rodada49.md). Every value below is computed in the precision the plugin uses
// (single-precision state and coefficients, double-precision intermediates where the plugin converts), because the
// effect is verified sample by sample against the installation's DLL. Fusing a·b + c into one FMA would change the
// results: contraction stays off.
#if defined(__clang__)
#pragma clang fp contract(off)
#elif defined(__GNUC__)
#pragma GCC optimize ("fp-contract=off")
#endif

#include <algorithm>
#include <cmath>
#include <cstdio>
#include <cstring>

#include "core/PlateReverb.h"

namespace lynx
{
namespace reeverb2
{
// (float) std::sin ((double) ph · 2π) for the LFO, faster (round 190kz, owner decision 4f) and exact: an odd polynomial
// to r^23 on [−π/2, π/2] (error < 1e-15) gives v; when every value within 1e-14 of v rounds to the same float, that float
// is what std::sin's (≤ 1 ulp) result rounds to, else std::sin itself. Checked on every float phase in [0, 1): all
// 1,065,353,216 identical to the plain expression (agents/dsp/f/lfosin.txt of round 190kz); 2.5x faster on a phase walk.
float lfoSine (float ph) noexcept
{
    const double x = (double) ph * 6.283185307179586;
    if (! (x >= 0.0 && x < 6.283185307179586))
        return (float) std::sin (x);
    const double pi = 3.141592653589793, halfPi = 1.5707963267948966;
    double r = x > pi ? x - 6.283185307179586 : x; // (−π, π]
    r = r > halfPi ? pi - r : r < -halfPi ? -pi - r : r;
    const double r2 = r * r;
    static constexpr double c[12] = { 1.0, -1.0 / 6.0, 1.0 / 120.0, -1.0 / 5040.0, 1.0 / 362880.0, -1.0 / 39916800.0, 1.0 / 6227020800.0,
                                      -1.0 / 1307674368000.0, 1.0 / 355687428096000.0, -1.0 / 121645100408832000.0,
                                      1.0 / 51090942171709440000.0, -1.0 / 25852016738884976640000.0 };
    double q = c[11];
    for (int k = 10; k >= 0; --k)
        q = q * r2 + c[k];
    const double v = q * r;
    const float lo = (float) (v - 1.0e-14), hi = (float) (v + 1.0e-14);
    return lo == hi ? lo : (float) std::sin (x);
}

// FL's control ranges (REA_rodada7 §3.2) and the two option bytes
const std::array<Range, reeverb2ParamCount> ranges { {
    { 19, 3000 }, { 5, 221 }, { 0, 384 }, { 1, 100 }, { 0, 100 }, { 1, 200 }, { 5, 221 }, { 20, 300 }, { 25, 2000 },
    { -64, 64 }, { 0, 160 }, { 0, 160 }, { 0, 160 }, { 0, 2048 }, { 0, 2048 }, { 0, 1 }, { 0, 1 } } };

Settings fromParams (const EffectParams& params) noexcept
{
    Settings s;
    auto raw = [&] (int k) {
        const auto r = ranges[(std::size_t) k];
        return r.lo + (int) std::nearbyint (std::clamp (params[(std::size_t) k], 0.0f, 1.0f) * (float) (r.hi - r.lo));
    };
    for (int k = 0; k < 15; ++k)
        s.p[(std::size_t) k] = raw (k);
    s.tempoPredelay = raw (15) != 0;
    s.midSide = raw (16) != 0;
    return s;
}

void toParams (const Settings& s, EffectParams& params) noexcept
{
    auto put = [&] (int k, int v) {
        const auto r = ranges[(std::size_t) k];
        params[(std::size_t) k] = (float) (std::clamp (v, r.lo, r.hi) - r.lo) / (float) (r.hi - r.lo);
    };
    for (int k = 0; k < 15; ++k)
        put (k, s.p[(std::size_t) k]);
    put (15, s.tempoPredelay ? 1 : 0);
    put (16, s.midSide ? 1 : 0);
}

namespace
{
// line proportions of the plate (in 1/20000 of the room-scaled rate) and output taps: the shared plate's; the
// early-reflection taps (scaled by (room + 3)·fs·1.5 / (44100·103))
constexpr const int* lineN = PlateReverb::lines;
constexpr const int* tapLN = PlateReverb::tapsL;
constexpr const int* tapRN = PlateReverb::tapsR;
constexpr int erLN[8] { 1102, 2904, 3784, 4873, 5878, 7470, 9062, 9263 };
constexpr int erRN[8] { 837, 1423, 2865, 3938, 5278, 5978, 7532, 9263 };
constexpr int modSize = 4096; // each tank half's modulated delay line

int roundEven (double x) noexcept { return (int) std::nearbyint (x); } // the plugin's Round (current rounding mode)
float f32 (double x) noexcept { return (float) x; }
double power025 (double x) noexcept { return std::exp (0.25 * std::log (x)); }

// FL's level law for a control value (scale 1/128 for dry, 1/256 for early reflections and wet)
float levelGain (int v, float scale) noexcept
{
    const float x = (float) v * scale;
    if (x == 1.0f)
        return 1.0f;
    return f32 ((std::exp ((double) (x * 2.39789534f)) - 1.0) * (double) 0.1f);
}

// The output gain matrix (outL += m0·L + m2·R, outR += m1·R + m3·L) for a gain and separation
std::array<float, 4> gainMatrix (float gain, float sep, bool midSide) noexcept
{
    std::array<float, 4> m {};
    if (sep == 0.0f)
        m = { gain, gain, 0.0f, 0.0f };
    else if (gain == 0.0f)
        m = { 0.0f, 0.0f, 0.0f, 0.0f };
    else
    {
        // pan 0 through the plugin's sine/cosine law, in extended precision
        const long double a = (1.0L + 0.0L) * (long double) 0.7853981633974483;
        const long double g = (long double) (gain * 1.41421354f);
        m[0] = (float) (cosl (a) * g);
        m[1] = (float) (sinl (a) * g);
        float k = sep;
        if (sep > 0.0f)
            k = (float) (std::exp ((double) sep * std::log (5.0)) - 1.0) * 0.25f;
        const float fa = std::min (0.0f + k, 1.0f), fb = std::min (0.0f + k, 1.0f);
        m[2] = m[0] * fa * 0.5f;
        m[0] = m[0] * (1.0f - fa * 0.5f);
        m[3] = m[1] * fb * 0.5f;
        m[1] = m[1] * (1.0f - fb * 0.5f);
    }
    if (midSide)
        m[1] *= -1.0f, m[3] *= -1.0f;
    return m;
}

// FL's per-block ramp of a smoothed value: returns the start value and the per-sample increment
struct Ramp
{
    float start = 0.0f, inc = 0.0f;
};

Ramp ramp (float& current, float target, int n) noexcept
{
    constexpr float maxStep = 0.001f;
    Ramp r;
    r.start = current;
    r.inc = target - r.start;
    if (r.inc != 0.0f)
    {
        current = target;
        r.inc = r.inc / (float) n;
        if (std::abs (r.inc) > maxStep)
        {
            r.inc = std::copysign (maxStep, r.inc);
            current = r.start + r.inc * (float) n;
        }
    }
    return r;
}
} // namespace

struct Engine::Impl
{
    // --- the reverb object (Shared/Reverb_x64.dll's parameter block) ---
    int fs = 44100;
    float sHc = 0, sLcH = 0, sLcX = 0, cLc = 0, g = 0, cT1 = 0, cT2 = 0, cIn = 0, cHc = 1, cDamp = 1;
    float dampA = 0, dampB = 0, c44 = 0, c48 = 0, lfA1 = 0, lfA2 = 0, lfB1 = 0, lfB2 = 0;
    std::array<std::vector<float>, 12> line;
    std::array<int, 12> L {}, P {};
    std::vector<float> pre, er;
    int preP = 0, erP = 0, erSize = 1;
    std::array<int, 7> tl {}, tr {};
    std::array<int, 8> erL {}, erR {};
    std::array<float, 8> erWL {}, erWR {};
    int storedRoom = 0;
    std::array<std::vector<float>, 2> mod;
    std::array<int, 2> modW {};
    float modSpeed = 0, modDepth = 0, lfoPhase = 0, modAmount = 0;

    // --- the Delphi wrapper's fields ---
    float lowCut = 0, highCut = 0, damping = 0, decay = 0, bass = 1, xover = 500;
    int predelay = 1, room = 50, diffusion = 100;
    float speedIn = 0, depthIn = 0;
    bool dirty = true;

    // --- the plugin ---
    Settings set;
    float bpm = 120.0f;
    float dry = 1, dryNow = 0, sep = 0;
    std::array<float, 4> mEr {}, mErNow {}, mWet {}, mWetNow {};
    std::vector<float> mono, wet, erBuf;

    // FUN_006d1d00: (re)allocation at the rate or a predelay change; clears everything
    void reinit()
    {
        for (int k = 0; k < 12; ++k)
        {
            P[(std::size_t) k] = 0;
            L[(std::size_t) k] = (int) ((long long) lineN[k] * fs / 20000);
            line[(std::size_t) k].assign ((std::size_t) std::max (1, L[(std::size_t) k]), 0.0f);
        }
        preP = erP = 0;
        pre.assign ((std::size_t) std::max (1, predelay), 0.0f);
        int r = roundEven ((double) fs * 0.3163265306122449), bits = 0;
        while (r > 0)
            r >>= 1, ++bits;
        erSize = 1 << (bits + 1);
        er.assign ((std::size_t) erSize, 0.0f);
        for (int k = 0; k < 7; ++k)
            tl[(std::size_t) k] = (int) ((long long) tapLN[k] * fs / 20000), tr[(std::size_t) k] = (int) ((long long) tapRN[k] * fs / 20000);
        for (auto& m : mod)
            if (m.size() != (std::size_t) modSize)
                m.assign ((std::size_t) modSize, 0.0f);
        clear();
    }

    // FUN_006d1c60: zero the lines and the filter states
    void clear() noexcept
    {
        for (auto& b : line)
            std::fill (b.begin(), b.end(), 0.0f);
        std::fill (pre.begin(), pre.end(), 0.0f);
        std::fill (er.begin(), er.end(), 0.0f);
        sHc = sLcH = sLcX = 0;
        dampA = dampB = lfA1 = lfA2 = lfB1 = lfB2 = 0;
        for (auto& m : mod)
            std::fill (m.begin(), m.end(), 0.0f);
    }

    // FUN_006d1f30: coefficients and line lengths for the current settings
    void update()
    {
        modSpeed = speedIn, modDepth = depthIn;
        const float fsF = (float) fs;
        const float inv = f32 (1.0 / (double) fs);
        const float nyq = f32 ((double) fsF * 0.5);
        auto onePole = [&] (float f) {
            if (f == 0.0f)
                return 1.0f;
            if (nyq <= f)
                f = nyq;
            const float y = f32 (2.0 - std::cos ((double) f * 628.3185307179587 * (double) inv));
            const double s = std::sqrt ((double) (y * y) - 1.0);
            return f32 (1.0 - ((double) y - s));
        };
        if (lowCut == 0.0f)
            cLc = 0.0f;
        else
        {
            const float f = nyq <= lowCut ? nyq : lowCut;
            const float t = f32 (std::tan ((double) f * 3.141592653589793 * (double) inv));
            cLc = f32 (((double) t - 1.0) / ((double) t + 1.0));
        }
        cHc = onePole (highCut);

        const int R = (int) ((long long) fs * (room + 15) / 115);
        for (int k = 0; k < 12; ++k)
        {
            auto& len = L[(std::size_t) k];
            len = std::max (1, (int) ((long long) lineN[k] * R / 20000));
            P[(std::size_t) k] %= len;
        }
        for (int k = 0; k < 7; ++k)
            tl[(std::size_t) k] = (int) ((long long) tapLN[k] * R / 20000), tr[(std::size_t) k] = (int) ((long long) tapRN[k] * R / 20000);
        if (room != storedRoom)
        {
            storedRoom = room;
            for (int k = 0; k < 12; ++k)
                std::fill (line[(std::size_t) k].begin(), line[(std::size_t) k].begin() + L[(std::size_t) k], 0.0f);
            std::fill (er.begin(), er.end(), 0.0f);
        }

        const double ln1e3 = std::log (0.001);
        const float d = f32 ((double) diffusion * 0.01);
        const float h = f32 ((double) inv * 0.5);
        const float a0 = (float) (L[0] + L[1]) * h, a2 = (float) (L[4] + L[8]) * h, a3 = (float) (L[6] + L[10]) * h;
        if (d == 0.0f)
            cIn = cT1 = cT2 = 0.0f;
        else
        {
            cIn = f32 (std::exp (ln1e3 * (double) a0 / ((double) d * 0.07)));
            cT1 = f32 (std::exp (ln1e3 * (double) a2 / ((double) d * 0.35)));
            cT2 = f32 (std::exp (ln1e3 * (double) a3 / ((double) d * 0.51)));
        }
        const float sum = (float) (L[5] + L[7] + L[9] + L[11]);
        g = f32 (power025 (std::exp (ln1e3 * (double) sum / (double) decay)));
        cDamp = onePole (damping);
        float rho = f32 (power025 (std::exp (ln1e3 * (double) sum / (double) (decay * bass)))) / g;
        c48 = f32 (((double) rho - 1.0) * 0.5);
        if ((double) rho > 1.0)
            rho = 1.0f;
        const float t = f32 (std::tan ((double) xover * 3.141592653589793 * (double) inv));
        c44 = (t - rho) / (t + rho);

        const float x6 = f32 ((double) (fs * (room + 3)) * 3.3022917905026087e-07);
        for (int k = 0; k < 8; ++k)
        {
            // the numerator is reset to 0 on every pass of the plugin's loop: only the first tap of each side sounds
            erL[(std::size_t) k] = roundEven ((double) ((float) erLN[k] * x6));
            erR[(std::size_t) k] = roundEven ((double) ((float) erRN[k] * x6));
            const float m = k == 0 ? (float) std::min (erL[0], erR[0]) : 0.0f;
            erWL[(std::size_t) k] = f32 ((double) m * 0.5 / (double) erL[(std::size_t) k]);
            erWR[(std::size_t) k] = f32 ((double) m * 0.36 / (double) erR[(std::size_t) k]);
        }
        dirty = false;
    }

    static float allPass (std::vector<float>& b, int& p, int len, float x, float c) noexcept
    {
        const float y = b[(std::size_t) p] - c * x;
        b[(std::size_t) p] = c * y + x;
        if (--p < 0)
            p = len - 1;
        return y;
    }

    float modRead (int k, float dly) const noexcept
    {
        const auto& b = mod[(std::size_t) k];
        const int i = modW[(std::size_t) k] - (int) dly;
        const float fr = dly - (float) (int) dly;
        return (1.0f - fr) * b[(std::size_t) ((i - 1) & (modSize - 1))] + fr * b[(std::size_t) ((i - 2) & (modSize - 1))];
    }

    void modWrite (int k, float v) noexcept
    {
        mod[(std::size_t) k][(std::size_t) modW[(std::size_t) k]] = v;
        modW[(std::size_t) k] = (modW[(std::size_t) k] + 1) & (modSize - 1);
    }

    float tap (int k, int off) const noexcept
    {
        int i = off + P[(std::size_t) k];
        if (i >= L[(std::size_t) k])
            i -= L[(std::size_t) k];
        return line[(std::size_t) k][(std::size_t) i];
    }

    // Shared/Reverb_x64.dll's full process (FUN_180003270): mono in, wet (and early reflections) out, interleaved
    void reverb (const float* x, float* wetOut, float* erOut, int n) noexcept
    {
        if (dirty)
            update();
        const float fsF = (float) fs;
        const float t1 = 1.0f - g;
        const float w25 = t1 * 0.25f + g, w60 = t1 * 0.6f + g, w70 = t1 * 0.7f + g, w20 = t1 * 0.2f + g, w50 = t1 * 0.5f + g;
        const float target = modDepth * 0.004535147454589605f * fsF;
        const float inc = (modSpeed * modSpeed * modSpeed * 9.89999962f + 0.100000001f) / fsF;
        const float smooth = 1.0f - std::pow (0.01f, f32 (1.0 / ((double) fsF * 0.1)));
        const int erMask = erSize - 1;

        // The loop runs on local copies of the state, positions, coefficients and buffer pointers: the float
        // members could otherwise alias the float lines being written and be re-read on every access. Same operations,
        // same order; the state is written back after the block.
        std::array<int, 12> p = P;
        const std::array<int, 12> len = L;
        std::array<float*, 12> ln {};
        for (int k = 0; k < 12; ++k)
            ln[(std::size_t) k] = line[(std::size_t) k].data();
        float* const erBuf = er.data();
        float* const preBuf = pre.data();
        const int preLast = (int) pre.size() - 1, erLast = erSize - 1;
        int erPos = erP, prePos = preP;
        float* const md[2] = { mod[0].data(), mod[1].data() };
        int mw[2] = { modW[0], modW[1] };
        const auto tL = tl, tR = tr;
        const auto eL = erL, eR = erR;
        const auto ewL = erWL, ewR = erWR;
        const float kHc = cHc, kLc = cLc, kIn = cIn, kT1 = cT1, kT2 = cT2, kG = g, k44 = c44, k48 = c48, kDamp = cDamp;
        float phase = lfoPhase, amount = modAmount, hc = sHc, lcH = sLcH, lcX = sLcX;
        float a1 = lfA1, a2 = lfA2, b1 = lfB1, b2 = lfB2, dA = dampA, dB = dampB;

        auto allPassAt = [&] (int k, float in, float c) {
            float* const b = ln[(std::size_t) k];
            int& q = p[(std::size_t) k];
            const float y = b[q] - c * in;
            b[q] = c * y + in;
            if (--q < 0)
                q = len[(std::size_t) k] - 1;
            return y;
        };
        auto tapAt = [&] (int k, int off) {
            int j = off + p[(std::size_t) k];
            if (j >= len[(std::size_t) k])
                j -= len[(std::size_t) k];
            return ln[(std::size_t) k][j];
        };
        auto modReadAt = [&] (int k, float dly) {
            const float* const b = md[k];
            const int j = mw[k] - (int) dly;
            const float fr = dly - (float) (int) dly;
            return (1.0f - fr) * b[(j - 1) & (modSize - 1)] + fr * b[(j - 2) & (modSize - 1)];
        };
        auto modWriteAt = [&] (int k, float v) {
            md[k][mw[k]] = v;
            mw[k] = (mw[k] + 1) & (modSize - 1);
        };

        for (int i = 0; i < n; ++i)
        {
            // modulation: sine LFO on the phase before its advance, smoothed depth
            const float ph = phase;
            float nx = ph + inc;
            nx = nx - (float) (int) nx;
            phase = nx;
            const float diff = target - amount;
            amount = std::abs ((double) diff) < 0.01 ? target : diff * smooth + amount + 1.0e-30f;
            // with no modulation depth the product is a zero of the amount's sign for any s in [−1, 1]: the sine is
            // only computed when it can change the delay
            const float s = amount != 0.0f ? lfoSine (ph) : 0.0f;
            const float dly = amount * ((s + 1.0f) * 0.5f);

            // input: high cut one-pole, low cut through a first-order all-pass
            float a = ((x[i] + 1.0e-30f) - hc) * kHc + hc;
            hc = a;
            const float hh = (a - lcH) * kLc + lcX;
            lcX = a;
            lcH = hh;
            if (kLc != 0.0f)
                a = (a - hh) * 0.5f + 1.0e-30f;

            if (erOut != nullptr)
            {
                auto e = [&] (int off) { return erBuf[(off + erPos) & erMask]; };
                erOut[2 * i] = ((((((e (eL[0]) * ewL[0] + e (eL[1]) * ewL[1]) - e (eL[2]) * ewL[2]) + e (eL[3]) * ewL[3])
                                  + e (eL[4]) * ewL[4])
                                 + e (eL[5]) * ewL[5])
                                + e (eL[6]) * ewL[6]);
                erOut[2 * i + 1] = ((((((e (eR[0]) * ewR[0] - e (eR[1]) * ewR[1]) + e (eR[2]) * ewR[2]) + e (eR[3]) * ewR[3])
                                      - e (eR[4]) * ewR[4])
                                     + e (eR[5]) * ewR[5])
                                    + e (eR[6]) * ewR[6])
                                   - e (eR[7]) * ewR[7];
            }

            // input diffusion; the early-reflection line takes the first diffuser's output
            const float y0 = allPassAt (0, a, kIn);
            const float y1 = allPassAt (1, y0, kIn);
            erBuf[erPos] = y0;
            if (--erPos < 0)
                erPos = erLast;
            const float y2 = allPassAt (2, y1, kIn);
            const float y3 = allPassAt (3, y2, kIn);
            float z = preBuf[prePos];
            preBuf[prePos] = y3;
            if (--prePos < 0)
                prePos = preLast;
            z = z + 1.0e-20f;

            // tank, half A
            float u = ln[11][p[11]] * kG;
            float f = (u - a2) * k44 + a1;
            a1 = u, a2 = f;
            float v = ((((f + u) * k48 + u) + z) - dA) * kDamp + dA;
            dA = v;
            modWriteAt (0, v);
            float w = allPassAt (4, modReadAt (0, dly), kT1);
            float r = ln[5][p[5]];
            ln[5][p[5]] = w;
            if (--p[5] < 0)
                p[5] = len[5] - 1;
            w = allPassAt (6, r * kG, kT2);
            ln[7][p[7]] = w;
            if (--p[7] < 0)
                p[7] = len[7] - 1;

            // tank, half B
            u = ln[7][p[7]] * kG;
            f = (u - b2) * k44 + b1;
            b1 = u, b2 = f;
            v = ((((f + u) * k48 + u) + z) - dB) * kDamp + dB;
            dB = v;
            modWriteAt (1, v);
            w = allPassAt (8, modReadAt (1, dly), kT1);
            r = ln[9][p[9]];
            ln[9][p[9]] = w;
            if (--p[9] < 0)
                p[9] = len[9] - 1;
            w = allPassAt (10, r * kG, kT2);
            ln[11][p[11]] = w;
            if (--p[11] < 0)
                p[11] = len[11] - 1;

            wetOut[2 * i] = ((((tapAt (9, tL[0]) - w25 * tapAt (9, tL[1])) - w70 * tapAt (10, tL[2])) - w20 * tapAt (11, tL[3]))
                             + w60 * tapAt (5, tL[4]) + tapAt (6, tL[5]))
                            + w50 * tapAt (7, tL[6]);
            wetOut[2 * i + 1] = (((((w25 * tapAt (5, tR[1]) + tapAt (5, tR[0])) - w70 * tapAt (6, tR[2])) - w20 * tapAt (7, tR[3]))
                                  - w60 * tapAt (9, tR[4]))
                                 + tapAt (10, tR[5]))
                                + w50 * tapAt (11, tR[6]);
        }

        P = p;
        erP = erPos, preP = prePos;
        modW = { mw[0], mw[1] };
        lfoPhase = phase, modAmount = amount, sHc = hc, sLcH = lcH, sLcX = lcX;
        lfA1 = a1, lfA2 = a2, lfB1 = b1, lfB2 = b2, dampA = dA, dampB = dB;
    }

    // FUN_006d4e10: the wrapper hands the controls to the reverb (a predelay change re-allocates and clears it)
    void applyReverbParams()
    {
        const int iv = set.p[2] * fs;
        const float pd = set.tempoPredelay ? f32 ((double) iv * 0.3125 / (double) bpm) : (float) iv / 384.0f;
        const int old = predelay;
        predelay = roundEven ((double) pd);
        if (predelay != old)
            reinit();
        dirty = true;
        decay = (float) (set.p[5] * fs) * 0.100000001f;
        room = set.p[3];
        diffusion = set.p[4];
        bass = (float) set.p[7] * 0.00999999978f;
        xover = (float) set.p[8];
        lowCut = set.p[0] != 19 ? (float) set.p[0] : 0.0f;
        highCut = set.p[1] != 221 ? (float) set.p[1] : 0.0f;
        damping = set.p[6] != 221 ? (float) set.p[6] : 0.0f;
        speedIn = f32 ((double) set.p[13] / 2048.0);
        depthIn = f32 ((double) set.p[14] / 2048.0);
    }

    void applyGains()
    {
        dry = levelGain (set.p[10], 0.0078125f);
        sep = (float) set.p[9] * 0.015625f;
        mEr = gainMatrix (levelGain (set.p[11], 0.00390625f), sep, set.midSide);
        mWet = gainMatrix (levelGain (set.p[12], 0.00390625f), sep, set.midSide);
    }

    void change (const Settings& s, double newBpm)
    {
        const auto old = set;
        const auto oldBpm = bpm;
        set = s;
        bpm = (float) newBpm;
        bool rev = false;
        for (int k : { 0, 1, 2, 3, 4, 5, 6, 7, 8, 13, 14 })
            rev = rev || old.p[(std::size_t) k] != s.p[(std::size_t) k];
        rev = rev || old.tempoPredelay != s.tempoPredelay || (s.tempoPredelay && oldBpm != bpm);
        if (rev)
            applyReverbParams();
        applyGains();
    }

    void flush() noexcept
    {
        dryNow = dry;
        mErNow = mEr;
        mWetNow = mWet;
        clear();
    }

    // FUN_005df6b0: dst += src × matrix, with the matrix ramped per sample
    static void addMatrix (const float* src, float* dst, int n, const std::array<float, 4>& m0, const std::array<Ramp, 4>& rp) noexcept
    {
        const bool ramping = rp[0].inc != 0.0f || rp[1].inc != 0.0f || rp[2].inc != 0.0f || rp[3].inc != 0.0f;
        std::array<float, 4> m = m0;
        if (! ramping)
        {
            if (m[2] == 0.0f && m[3] == 0.0f)
            {
                if (m[0] == 0.0f && m[1] == 0.0f)
                    return;
                for (int i = 0; i < n; ++i)
                {
                    dst[2 * i] = src[2 * i] * m[0] + dst[2 * i];
                    dst[2 * i + 1] = src[2 * i + 1] * m[1] + dst[2 * i + 1];
                }
                return;
            }
            for (int i = 0; i < n; ++i)
            {
                const float a = src[2 * i], b = src[2 * i + 1];
                dst[2 * i] = a * m[0] + b * m[2] + dst[2 * i];
                dst[2 * i + 1] = b * m[1] + a * m[3] + dst[2 * i + 1];
            }
            return;
        }
        int i = 0;
        for (; i + 1 < n; i += 2)
        {
            std::array<float, 4> m1;
            for (int k = 0; k < 4; ++k)
                m1[(std::size_t) k] = m[(std::size_t) k] + rp[(std::size_t) k].inc;
            float a = src[2 * i], b = src[2 * i + 1];
            dst[2 * i] = a * m[0] + b * m[2] + dst[2 * i];
            dst[2 * i + 1] = b * m[1] + a * m[3] + dst[2 * i + 1];
            a = src[2 * i + 2], b = src[2 * i + 3];
            dst[2 * i + 2] = a * m1[0] + b * m1[2] + dst[2 * i + 2];
            dst[2 * i + 3] = b * m1[1] + a * m1[3] + dst[2 * i + 3];
            for (int k = 0; k < 4; ++k)
                m[(std::size_t) k] = m1[(std::size_t) k] + rp[(std::size_t) k].inc;
        }
        if (i < n)
        {
            const float a = src[2 * i], b = src[2 * i + 1];
            dst[2 * i] = a * m[0] + b * m[2] + dst[2 * i];
            dst[2 * i + 1] = b * m[1] + a * m[3] + dst[2 * i + 1];
        }
    }

    // FUN_006d5a90: Eff_Render on an interleaved stereo buffer, in place
    void render (float* io, int n)
    {
        if ((int) mono.size() < n)
            mono.resize ((std::size_t) n), wet.resize (2 * (std::size_t) n), erBuf.resize (2 * (std::size_t) n);
        const bool erOn = std::any_of (mEr.begin(), mEr.end(), [] (float v) { return v != 0.0f; })
                          || std::any_of (mErNow.begin(), mErNow.end(), [] (float v) { return v != 0.0f; });
        for (int i = 0; i < n; ++i)
            mono[(std::size_t) i] = set.midSide ? io[2 * i] - io[2 * i + 1] : io[2 * i] + io[2 * i + 1];
        reverb (mono.data(), wet.data(), erOn ? erBuf.data() : nullptr, n);

        // dry (FUN_005db3c0): constant gain, or the ramp in pairs of samples as the plugin's unrolled loop does
        const auto dr = ramp (dryNow, dry, n);
        if (dr.inc == 0.0f)
        {
            if (dr.start == 0.0f)
                std::fill (io, io + 2 * n, 0.0f);
            else if (dr.start != 1.0f)
                for (int i = 0; i < 2 * n; ++i)
                    io[i] = io[i] * dr.start;
        }
        else
        {
            float even = dr.start + 0.0f, odd = dr.start + dr.inc;
            const float inc2 = dr.inc + dr.inc;
            int i = 0;
            for (; i + 8 <= n; i += 8)
                for (int j = 0; j < 8; j += 2)
                {
                    io[2 * (i + j)] *= even, io[2 * (i + j) + 1] *= even;
                    io[2 * (i + j + 1)] *= odd, io[2 * (i + j + 1) + 1] *= odd;
                    even = even + inc2, odd = odd + inc2;
                }
            for (; i < n; ++i)
            {
                io[2 * i] *= even, io[2 * i + 1] *= even;
                even = even + dr.inc;
            }
        }

        if (erOn)
        {
            std::array<Ramp, 4> rp;
            std::array<float, 4> st;
            for (int k = 0; k < 4; ++k)
                rp[(std::size_t) k] = ramp (mErNow[(std::size_t) k], mEr[(std::size_t) k], n), st[(std::size_t) k] = rp[(std::size_t) k].start;
            addMatrix (erBuf.data(), io, n, st, rp);
        }
        std::array<Ramp, 4> rp;
        std::array<float, 4> st;
        for (int k = 0; k < 4; ++k)
            rp[(std::size_t) k] = ramp (mWetNow[(std::size_t) k], mWet[(std::size_t) k], n), st[(std::size_t) k] = rp[(std::size_t) k].start;
        addMatrix (wet.data(), io, n, st, rp);
    }

    std::vector<float> inter;
};

Engine::Engine() : d (std::make_unique<Impl>()) {}
Engine::~Engine() = default;

void Engine::prepare (int sampleRate, const Settings& s, double bpm, bool flushed)
{
    auto& D = *d;
    D.fs = std::max (1, sampleRate);
    D.set = s;
    D.bpm = (float) bpm;
    D.predelay = 1;
    D.storedRoom = 0;
    D.lfoPhase = D.modAmount = 0.0f;
    D.modW = { 0, 0 };
    D.reinit();
    D.applyReverbParams();
    D.applyGains();
    if (flushed)
        D.flush();
}

void Engine::reserve (int sampleRate, int maxBlock)
{
    auto& D = *d;
    const auto fs = std::max (1, sampleRate);
    for (int k = 0; k < 12; ++k)
        D.line[(std::size_t) k].reserve ((std::size_t) std::max (1, (int) ((long long) lineN[k] * fs / 20000)));
    D.pre.reserve ((std::size_t) (2 * fs + 2));
    int r = roundEven ((double) fs * 0.3163265306122449), bits = 0; // (reinit's early-reflection size)
    while (r > 0)
        r >>= 1, ++bits;
    D.er.reserve ((std::size_t) 1 << (bits + 1));
    for (auto& m : D.mod)
        m.reserve ((std::size_t) modSize);
    const auto block = (std::size_t) std::max (1, maxBlock);
    D.mono.reserve (block);
    D.wet.reserve (2 * block);
    D.erBuf.reserve (2 * block);
    D.inter.reserve (2 * block);
}

void Engine::flush() noexcept { d->flush(); }

void Engine::set (const Settings& s, double bpm)
{
    if (! (s == d->set) || (float) bpm != d->bpm)
        d->change (s, bpm);
}

void Engine::process (float* left, float* right, int n) noexcept
{
    auto& D = *d;
    if ((int) D.inter.size() < 2 * n)
        D.inter.resize (2 * (std::size_t) n);
    for (int i = 0; i < n; ++i)
        D.inter[2 * (std::size_t) i] = left[i], D.inter[2 * (std::size_t) i + 1] = right[i];
    D.render (D.inter.data(), n);
    for (int i = 0; i < n; ++i)
        left[i] = D.inter[2 * (std::size_t) i], right[i] = D.inter[2 * (std::size_t) i + 1];
}
} // namespace reeverb2
} // namespace lynx
