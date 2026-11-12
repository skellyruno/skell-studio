// MIT: derived from Airwindows ToTape6 (Chris Johnson). See vendor/airwindows/LICENSE.
#pragma once
#define _USE_MATH_DEFINES
#include <cmath>
#include <cstdint>
#include <cstring>
#include <new>
#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif
using VstInt32 = int32_t;
class ToTape6 {
public:
 double sample_rate;
 double getSampleRate() const { return sample_rate; }
 void processReplacing(float**,float**,VstInt32);
 void processDoubleReplacing(double**,double**,VstInt32);
 ToTape6(double rate) {reset(rate);}
 void reset(double rate) {std::memset(this,0,sizeof(*this));sample_rate=rate;A=B=C=D=E=0.5f;F=1;rateof=nextmax=0.5;sweep=M_PI;fpdL=0x12345678;fpdR=0x87654321;}
	double dL[502];
	double dR[502];
	int gcount;
	double rateof;
	double sweep;
	double nextmax;
	
	double iirMidRollerAL;
	double iirMidRollerBL;
	double iirHeadBumpAL;
	double iirHeadBumpBL;
	
	double iirMidRollerAR;
	double iirMidRollerBR;
	double iirHeadBumpAR;
	double iirHeadBumpBR;
	double biquadAL[9];
	double biquadBL[9];
	double biquadCL[9];
	double biquadDL[9];

	double biquadAR[9];
	double biquadBR[9];
	double biquadCR[9];
	double biquadDR[9];
	bool flip;
	
	double lastSampleL;
	double lastSampleR;
	
	uint32_t fpdL;
	uint32_t fpdR;
	//default stuff
	
    float A;
    float B;
    float C;
    float D;
    float E;
    float F;
};
