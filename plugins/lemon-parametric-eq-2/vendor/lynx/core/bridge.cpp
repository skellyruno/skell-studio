// Copyright (C) 2026 Skelly Audio
// SPDX-License-Identifier: AGPL-3.0-only
// LYNX SHARE 0.1.30 Fruity Parametric EQ 2, pinned at 88ec555ea2bce9714c92707cc06a5abfd317c900.
#include "core/ParametricEqFl.h"
#include "core/ParametricEqLinear.h"
#include <algorithm>
#include <cmath>
#include <cstdint>
namespace {
using lynx::peqfl::Engine;
using lynx::peqfl::Law;
using lynx::peqfl::Processing;
using lynx::peqfl::Settings;
Settings makeSettings(const int* raw,int hq,int flags,int quality,int fade,int mask) noexcept {
    Settings s;s.law=Law::peq2;
    for(int band=0;band<7;++band){const auto k=static_cast<std::size_t>(band);
        s.gain[k]=raw[band];s.freq[k]=std::clamp(raw[7+band],0,65536);
        s.width[k]=std::clamp(raw[14+band],0,65536);s.type[k]=std::clamp(raw[21+band],0,7);
        s.order[k]=std::clamp(raw[28+band],-3,3);}
    s.main=std::clamp(raw[35],-1800,1800);s.mask=static_cast<std::uint32_t>(mask)&0x7fu;
    s.linearPhase=(flags&1)!=0;s.linearQuality=std::clamp(quality,0,7);s.linearFade=std::clamp(fade,0,3);
    const bool enabled=(hq&1)!=0;const int design=(hq>>1)&0xff;
    if(enabled&&design==0)s.processing=(flags&2)!=0?Processing::hqExternal:Processing::hq;
    else if(enabled&&design==1)s.processing=Processing::hqAlternative;
    else s.processing=Processing::plain;
    return s;
}
bool sameSettings(const Settings&a,const Settings&b) noexcept {
    return a.law==b.law&&a.gain==b.gain&&a.freq==b.freq&&a.width==b.width&&a.type==b.type&&a.order==b.order
        &&a.main==b.main&&a.processing==b.processing&&a.linearPhase==b.linearPhase
        &&a.linearQuality==b.linearQuality&&a.linearFade==b.linearFade&&a.mask==b.mask;
}
struct Instance{Engine engine;Settings settings{};int fixedLinearQuality=2;double sampleRate=44100.;bool configured=false;};
void apply(Instance& instance,const int* raw,int hq,int flags,int quality,int fade,int mask) noexcept {
    auto next=makeSettings(raw,hq,flags,quality,fade,mask);
    next.linearQuality=instance.fixedLinearQuality;
    if(!instance.configured||!sameSettings(instance.settings,next)){
        instance.settings=next;instance.engine.set(instance.settings);instance.configured=true;
    }
}
}
extern "C" {
void* skell_lynx_eq_new(int sampleRate,const int* raw,int hq,int flags,int quality,int fade,int mask) noexcept {
    if(raw==nullptr)return nullptr;
    try{
        auto* instance=new Instance();
        instance->sampleRate=std::clamp(sampleRate,8000,384000);
        const auto initial=makeSettings(raw,hq,flags,quality,fade,mask);
        instance->fixedLinearQuality=initial.linearQuality;
        instance->engine.set(initial);            // select the exact saved LIN quality first
        instance->engine.prepare(instance->sampleRate); // reserve all FFT buffers before audio starts
        instance->settings=initial;instance->configured=true;
        return instance;
    }catch(...){return nullptr;}
}
void skell_lynx_eq_delete(void* opaque) noexcept{delete static_cast<Instance*>(opaque);}
void skell_lynx_eq_set(void* opaque,const int* raw,int hq,int flags,int quality,int fade,int mask) noexcept{
    if(opaque&&raw)apply(*static_cast<Instance*>(opaque),raw,hq,flags,quality,fade,mask);
}
void skell_lynx_eq_reset(void* opaque) noexcept{if(opaque)static_cast<Instance*>(opaque)->engine.reset();}
int skell_lynx_eq_latency(void* opaque) noexcept{return opaque?static_cast<Instance*>(opaque)->engine.latencySamples():0;}
void skell_lynx_eq_process(void* opaque,float* left,float* right,int frames) noexcept{
    if(opaque&&left&&right&&frames>0)static_cast<Instance*>(opaque)->engine.process(left,right,frames);
}
double skell_lynx_eq_response(void* opaque,double hz) noexcept{
    if(!opaque||!std::isfinite(hz))return 0.;const auto& p=*static_cast<Instance*>(opaque);
    return lynx::peqfl::responseDb(p.settings,p.sampleRate,std::clamp(hz,1.,p.sampleRate*.499));
}
bool skell_lynx_eq_stages_match(std::uint32_t seed) noexcept{return lynx::peqfl::stagesMatchReference(seed);}
}
