// SPDX-License-Identifier: GPL-3.0-or-later
// Lemon Hall adapter. Routing/initialization adapted from Dragonfly Hall DSP:
// Copyright (c) 2018 Michael Willis, Rob van den Berg (GPL-3.0-or-later).
// Freeverb3 authors and licence: dragonfly/UPSTREAM.txt and original headers.
#include <algorithm>
#include <cmath>
#include <vector>
#include "freeverb/earlyref.hpp"
#include "freeverb/zrev2.hpp"
// Freeverb's legacy macros must not leak into the adapter.
#undef isfinite
#undef isnormal
#undef fpclassify

struct LemonHall {
    fv3::earlyref_ early;
    fv3::zrev2_ late;
    std::vector<float> delayL, delayR;
    size_t write = 0;
    float rate;
    float previous[10];
    double el[64], er[64], il[64], ir[64], ll[64], lr[64];
    explicit LemonHall(float sr):delayL(size_t(sr*.251f)+4,0),delayR(delayL.size(),0),rate(sr) {
        early.setMuteOnChange(false); late.setMuteOnChange(false);
        early.loadPresetReflection(FV3_EARLYREF_PRESET_1);
        early.setdryr(0); early.setwet(0); early.setwidth(1); early.setLRDelay(.3f);
        early.setLRCrossApFreq(750,4); early.setDiffusionApFreq(150,4);
        early.setSampleRate(sr); early.setRSFactor(3.2f);
        late.setdryr(0); late.setwet(0); late.setwidth(1); late.setSampleRate(sr);
        late.setRSFactor(.4f); late.setPreDelay(.1f);
        late.setxover_low(500); late.setxover_high(std::min(5500.f,sr*.4f));
        late.setrt60_factor_low(1.25f); late.setwander(16); late.setspin(2.4f);
        std::fill(previous,previous+10,-1.f);
    }
    void reset() {
        early.mute(); late.mute();
        std::fill(delayL.begin(),delayL.end(),0.f); std::fill(delayR.begin(),delayR.end(),0.f); write=0;
    }
    // All exposed setters alter coefficients only. Size, wander and native
    // predelay (which resize buffers) are fixed during initialization.
    void set(const float* v) {
        if(v[0]!=previous[0]) late.setrt60(v[0]);
        if(v[3]!=previous[3]) { late.setidiffusion1(v[3]/140.f); late.setapfeedback(v[3]/140.f); }
        if(v[4]!=previous[4]) { early.setoutputhpf(v[4]); late.setoutputhpf(v[4]); }
        if(v[5]!=previous[5]) { float hz=std::min(v[5],rate*.45f); early.setoutputlpf(hz); late.setoutputlpf(hz); }
        if(v[6]!=previous[6]) late.setrt60_factor_high(v[6]);
        if(v[7]!=previous[7]) { late.setspinfactor(v[7]*.005f); late.setlfofactor(v[7]*.005f); }
        std::copy(v,v+10,previous);
    }
    void process(float* l,float* r,int n) {
        for(int start=0;start<n;start+=64) {
            int count=std::min(n-start,64);
            const float offset=previous[1]*rate*.001f;
            const size_t whole=size_t(offset); const float frac=offset-float(whole);
            for(int i=0;i<count;++i) {
                delayL[write]=l[start+i]; delayR[write]=r[start+i];
                size_t a=(write+delayL.size()-whole)%delayL.size(), b=(a+delayL.size()-1)%delayL.size();
                il[i]=delayL[a]+frac*(delayL[b]-delayL[a]); ir[i]=delayR[a]+frac*(delayR[b]-delayR[a]);
                write=(write+1)%delayL.size();
            }
            early.processreplace(il,ir,el,er,count);
            for(int i=0;i<count;++i) { il[i]+=.2f*el[i]; ir[i]+=.2f*er[i]; }
            late.processreplace(il,ir,ll,lr,count);
            const float mix=previous[9]*.01f, earlyGain=previous[2]*.01f, lateGain=1.f-earlyGain;
            for(int i=0;i<count;++i) {
                // Continue the reverb state at dry=100%, but preserve exact dry samples.
                if(mix==0) continue;
                const float wl=earlyGain*el[i]+lateGain*ll[i], wr=earlyGain*er[i]+lateGain*lr[i];
                const float mid=(wl+wr)*.5f, side=(wl-wr)*.5f*previous[8]*.01f;
                l[start+i]=(1-mix)*l[start+i]+mix*(mid+side);
                r[start+i]=(1-mix)*r[start+i]+mix*(mid-side);
            }
        }
    }
};
extern "C" {
void* lemon_hall_create(float rate) noexcept { try { return new LemonHall(rate); } catch(...) { return nullptr; } }
void lemon_hall_destroy(void* p) noexcept { delete static_cast<LemonHall*>(p); }
void lemon_hall_reset(void* p) noexcept { static_cast<LemonHall*>(p)->reset(); }
void lemon_hall_set(void* p,const float* v) noexcept { static_cast<LemonHall*>(p)->set(v); }
bool lemon_hall_process(void* p,float* l,float* r,int n) noexcept {
    try { static_cast<LemonHall*>(p)->process(l,r,n); return true; } catch(...) { return false; }
}
}
