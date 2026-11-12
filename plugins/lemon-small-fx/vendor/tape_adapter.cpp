// MIT: Airwindows ToTape6 audio processor with a Lemon-owned host adapter.
#include "tape_adapter.h"
#include <algorithm>
#define __ToTape6_H
#include "airwindows/ToTape6Proc.cpp"
extern "C" {
void* lemon_tape_create(double rate) {return new(std::nothrow) ToTape6(rate);}
void lemon_tape_destroy(void* p) {delete static_cast<ToTape6*>(p);}
void lemon_tape_reset(void* p,double rate) {static_cast<ToTape6*>(p)->reset(rate);}
void lemon_tape_set(void* p,const float* v) {auto& s=*static_cast<ToTape6*>(p);s.A=v[0];s.B=v[1];s.C=v[2];
 // Keep the upstream 500-frame flutter line in bounds even above 192 kHz.
 s.D=std::min(v[3],static_cast<float>(std::sqrt(450.0/(s.sample_rate/44100.0*70.0))));s.E=v[4];s.F=v[5];}
void lemon_tape_process(void* p,float* left,float* right,int32_t count) {float* io[]={left,right};static_cast<ToTape6*>(p)->processReplacing(io,io,count);}
}
