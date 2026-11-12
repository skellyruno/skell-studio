//! Exact Blackman-windowed sinc taps, stepping trigonometric phases instead of recomputing
//! three transcendental functions per tap. No lookup quantization or quality reduction.
pub struct BlackmanSinc {
    first: f64, index: usize, cut: f64, half: f64,
    wave: (f64,f64), window: (f64,f64), wave_step: (f64,f64), window_step: (f64,f64),
}
impl BlackmanSinc {
    pub fn new(first: f64, cut: f64, half: f64) -> Self {
        let a=std::f64::consts::PI*cut;let b=std::f64::consts::PI/half;
        Self{first,index:0,cut,half,wave:(a*first).sin_cos(),window:(b*first).sin_cos(),wave_step:a.sin_cos(),window_step:b.sin_cos()}
    }
    #[inline]
    fn rotate(value:(f64,f64),step:(f64,f64))->(f64,f64) {
        (value.0*step.1+value.1*step.0,value.1*step.1-value.0*step.0)
    }
}
impl Iterator for BlackmanSinc {
    type Item=f64;
    #[inline]
    fn next(&mut self)->Option<f64> {
        let t=self.first+self.index as f64;self.index+=1;
        // Around zero, recurrence roundoff divided by a tiny t is amplified.
        // Evaluate the removable singularity with its stable Taylor series.
        let x=std::f64::consts::PI*self.cut*t;
        let sinc=if x.abs()<1e-4 {self.cut*(1.-x*x/6.+x*x*x*x/120.)}else{self.wave.0/(std::f64::consts::PI*t)};
        let c=self.window.1;
        let w=if t.abs()>=self.half {0.}else{sinc*(0.42+0.5*c+0.08*(2.*c*c-1.))};
        self.wave=Self::rotate(self.wave,self.wave_step);self.window=Self::rotate(self.window,self.window_step);
        Some(w)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_kernel_matches_direct_formula() {
        for half in [4.,13.5,64.,256.,768.] {for cut in [1.,0.91875,0.5,0.1] {for phase in [0.,1e-13,0.001,0.31,0.999] {
            let first=-half+phase;
            for (i,w) in BlackmanSinc::new(first,cut,half).take((2.*half) as usize+2).enumerate() {
                let t=first+i as f64;let x=std::f64::consts::PI*t;
                let expected=if t.abs()>=half {0.}else{(if t==0. {cut}else{(cut*x).sin()/x})*(0.42+0.5*(x/half).cos()+0.08*(2.*x/half).cos())};
                assert!((w-expected).abs()<1e-10,"{half} {cut} {phase} {i}: {w} != {expected}");
            }
        }}}
    }
}
