// SPDX-License-Identifier: GPL-3.0-or-later
//! Offline, stereo-linked spectral noise reduction. Original implementation using Tesla's FFT.
use crate::{
    sample::{Format, Sample},
    spectrum::Fft,
};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc,
};
pub const VISIBILITY: &str = "visible_tesla-cleanup";
pub fn visible() -> bool {
    !os_paths::user_file("settings.txt")
        .and_then(|p| std::fs::read_to_string(p).ok())
        .is_some_and(|s| {
            s.lines()
                .rev()
                .filter_map(|l| l.split_once('='))
                .find(|(k, _)| k.trim() == VISIBILITY)
                .is_some_and(|(_, v)| v.trim() == "0")
        })
}
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub rate: u32,
    pub n: usize,
    pub power: Vec<f32>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub reduction: f32,
    pub sensitivity: f32,
    pub busy: bool,
    pub progress: u32,
    pub message: String,
    pub ready: bool,
    pub linked: bool,
    pub awaiting: bool,
}
impl Default for Panel {
    fn default() -> Self {
        Self {
            reduction: 12.,
            sensitivity: 1.5,
            busy: false,
            progress: 0,
            message: "Learn a noise-only selection, or preview with an automatic estimate.".into(),
            ready: false,
            linked: false,
            awaiting: false,
        }
    }
}
#[derive(Default)]
pub struct Progress {
    pub cancel: AtomicBool,
    pub percent: AtomicU32,
}
fn validate(s: &Sample) -> Result<(), String> {
    if !(8000..=384000).contains(&s.rate)
        || s.channels() == 0
        || s.channels() > 2
        || s.frames() == 0
    {
        return Err("Open an audio sample first (8–384 kHz, mono or stereo).".into());
    }
    if s.frames() > s.rate as usize * 60 * 20 || s.frames() * s.channels() > 120_000_000 {
        return Err("Cleanup supports up to 20 minutes / 120 million samples.".into());
    }
    if s.ch
        .iter()
        .any(|c| c.len() != s.frames() || c.iter().any(|x| !x.is_finite()))
    {
        return Err("This sample contains invalid audio.".into());
    }
    Ok(())
}
fn size(rate: u32) -> usize {
    ((rate as f64 * 0.043) as usize)
        .next_power_of_two()
        .clamp(512, 8192)
}
fn window(n: usize) -> Vec<f32> {
    (0..n)
        .map(|i| 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos())
        .collect()
}
fn frame(s: &Sample, c: usize, start: isize, w: &[f32], re: &mut [f32], im: &mut [f32]) {
    for i in 0..w.len() {
        let at = start + i as isize;
        re[i] = if at >= 0 {
            s.ch[c].get(at as usize).copied().unwrap_or(0.) * w[i]
        } else {
            0.
        };
        im[i] = 0.;
    }
}
pub fn learn(
    s: &Sample,
    range: (usize, usize),
    auto: bool,
    p: &Progress,
) -> Result<Profile, String> {
    validate(s)?;
    let (a, b) = range;
    if a > b || b >= s.frames() {
        return Err("Select some background noise first.".into());
    }
    if !auto && b - a + 1 < s.rate as usize / 10 {
        return Err("Select at least 100 ms of noise without music or speech.".into());
    }
    let n = size(s.rate);
    let fft = Fft::new(n);
    let w = window(n);
    let bins = n / 2 + 1;
    let count = ((b - a + 1) / (n / 4)).clamp(1, 192);
    let mut powers = vec![Vec::with_capacity(count); bins];
    let (mut re, mut im) = (vec![0.; n], vec![0.; n]);
    for j in 0..count {
        if p.cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        let start = a + if b - a + 1 > n {
            j * (b - a + 1 - n) / count.saturating_sub(1).max(1)
        } else {
            0
        };
        let mut sum = vec![0.; bins];
        for c in 0..s.channels() {
            frame(s, c, start as isize, &w, &mut re, &mut im);
            fft.run(&mut re, &mut im);
            for k in 0..bins {
                sum[k] += (re[k] * re[k] + im[k] * im[k]) / s.channels() as f32;
            }
        }
        for k in 0..bins {
            powers[k].push(sum[k]);
        }
    }
    let mut power: Vec<f32> = powers
        .into_iter()
        .map(|mut b| {
            if auto {
                b.sort_by(f32::total_cmp);
                b[b.len() / 5] * 3.5
            } else {
                b.iter().sum::<f32>() / b.len() as f32
            }
        })
        .collect();
    // Automatic estimates must not identify a sustained musical tone as noise.
    // Limit narrow peaks by the neighbouring spectral median; learned profiles retain hum peaks.
    if auto {
        let raw = power.clone();
        for k in 0..bins {
            let lo = k.saturating_sub(8);
            let hi = (k + 9).min(bins);
            let mut local = raw[lo..hi].to_vec();
            local.sort_by(f32::total_cmp);
            power[k] = raw[k].min(local[local.len() / 2] * 1.5);
        }
    }
    Ok(Profile {
        rate: s.rate,
        n,
        power,
    })
}
pub fn render(
    s: &Sample,
    range: (usize, usize),
    profile: Option<&Profile>,
    reduction: f32,
    sensitivity: f32,
    p: &Progress,
) -> Result<Sample, String> {
    validate(s)?;
    let (a, b) = range;
    if a > b || b >= s.frames() {
        return Err("Invalid cleanup selection.".into());
    }
    if !reduction.is_finite()
        || !(0.0..=36.).contains(&reduction)
        || !sensitivity.is_finite()
        || !(0.5..=4.).contains(&sensitivity)
    {
        return Err("Invalid cleanup settings.".into());
    }
    if reduction == 0. {
        return Ok(s.clone());
    }
    let inferred;
    let profile = match profile {
        Some(v) if v.rate == s.rate && v.n == size(s.rate) => v,
        Some(_) => return Err("Learn the noise again for this sample rate.".into()),
        None => {
            inferred = learn(s, (0, s.frames() - 1), true, p)?;
            &inferred
        }
    };
    let n = profile.n;
    let hop = n / 4;
    let bins = n / 2 + 1;
    let w = window(n);
    let fft = Fft::new(n);
    let len = b - a + 1;
    let mut output = vec![vec![0.; len]; s.channels()];
    let mut norm = vec![0.; len];
    let floor = 10f32.powf(-reduction / 20.);
    let mut re = vec![vec![0.; n]; s.channels()];
    let mut im = re.clone();
    let mut gain = vec![1.; bins];
    let mut previous = vec![1.; bins];
    let first = a as isize - n as isize + hop as isize;
    let total = (b as isize - first) / hop as isize + 1;
    for (step, start) in (first..=b as isize).step_by(hop).enumerate() {
        if p.cancel.load(Ordering::Relaxed) {
            return Err("Cancelled".into());
        }
        for c in 0..s.channels() {
            frame(s, c, start, &w, &mut re[c], &mut im[c]);
            fft.run(&mut re[c], &mut im[c]);
        }
        for k in 0..bins {
            let power = (0..s.channels())
                .map(|c| re[c][k] * re[c][k] + im[c][k] * im[c][k])
                .sum::<f32>()
                / s.channels() as f32;
            gain[k] = (1. - profile.power[k] * sensitivity / power.max(1e-20))
                .max(0.)
                .sqrt()
                .max(floor);
        }
        for k in 0..bins {
            let lo = k.saturating_sub(2);
            let hi = (k + 3).min(bins);
            let smooth = gain[k].max(gain[lo..hi].iter().sum::<f32>() / (hi - lo) as f32);
            let speed = if smooth > previous[k] { 0.8 } else { 0.35 };
            previous[k] += (smooth - previous[k]) * speed;
        }
        for c in 0..s.channels() {
            for k in 0..n {
                let g = previous[if k < bins { k } else { n - k }];
                re[c][k] *= g;
                im[c][k] *= -g;
            }
            fft.run(&mut re[c], &mut im[c]);
            for i in 0..n {
                let at = start + i as isize - a as isize;
                if at >= 0 && (at as usize) < len {
                    output[c][at as usize] += re[c][i] / n as f32 * w[i];
                }
            }
        }
        for i in 0..n {
            let at = start + i as isize - a as isize;
            if at >= 0 && (at as usize) < len {
                norm[at as usize] += w[i] * w[i];
            }
        }
        p.percent.store(
            ((step + 1) * 100 / total as usize) as u32,
            Ordering::Relaxed,
        );
    }
    let mut out = s.clone();
    out.format = Format::Float32;
    let fade = (s.rate as usize / 250).min(len / 2);
    for c in 0..s.channels() {
        for j in 0..len {
            let mut wet = 1.;
            if a > 0 && j < fade {
                wet = j as f32 / fade as f32;
            }
            if b + 1 < s.frames() && len - 1 - j < fade {
                wet = wet.min((len - 1 - j) as f32 / fade as f32);
            }
            let v = output[c][j] / norm[j].max(1e-12);
            if !v.is_finite() {
                return Err("Cleanup failed; original audio preserved.".into());
            }
            out.ch[c][a + j] = s.ch[c][a + j] * (1. - wet) + v * wet;
        }
    }
    Ok(out)
}
pub enum Outcome {
    Audio(Sample),
    Profile(Profile),
}
pub struct Job {
    pub rx: std::sync::mpsc::Receiver<Result<Outcome, String>>,
    pub source: Arc<Sample>,
    pub version: u64,
    pub range: (usize, usize),
    pub progress: Arc<Progress>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.progress.cancel.store(true, Ordering::Release);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn noise(n: usize) -> Vec<f32> {
        let mut seed = 23u32;
        (0..n)
            .map(|_| {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                ((seed >> 8) as f32 / 8388608. - 1.) * 0.03
            })
            .collect()
    }
    #[test]
    fn cleanup_reduces_noise_preserves_tone_phase_and_bounds() {
        let rate = 48000;
        let n = rate as usize * 2;
        let hiss = noise(n);
        let pure: Vec<f32> = (0..n)
            .map(|i| {
                if i < 12000 {
                    0.
                } else {
                    (i as f32 * std::f32::consts::TAU * 750. / rate as f32).sin() * 0.25
                }
            })
            .collect();
        let input: Vec<f32> = hiss.iter().zip(&pure).map(|(a, b)| a + b).collect();
        let sample = Sample::from_channels(
            rate,
            vec![input.clone(), input.iter().map(|v| v * -0.5).collect()],
            Format::Float32,
        );
        let p = Progress::default();
        let profile = learn(&sample, (0, 11999), false, &p).unwrap();
        let out = render(&sample, (0, n - 1), Some(&profile), 18., 1.8, &p).unwrap();
        let energy = |x: &[f32]| x.iter().map(|v| (*v as f64).powi(2)).sum::<f64>();
        assert!(
            energy(&out.ch[0][2000..10000]) < energy(&input[2000..10000]) * 0.4,
            "noise reduction"
        );
        let projection = out.ch[0][24000..]
            .iter()
            .zip(&pure[24000..])
            .map(|(a, b)| (*a as f64) * (*b as f64))
            .sum::<f64>()
            / energy(&pure[24000..]);
        assert!(
            projection > 0.85 && projection < 1.05,
            "tone gain {projection}"
        );
        assert!(
            out.ch[0]
                .iter()
                .zip(&out.ch[1])
                .all(|(l, r)| (l * -0.5 - r).abs() < 1e-5),
            "stereo phase/gain linked"
        );
        let automatic = render(&sample, (0, n - 1), None, 12., 1.5, &Progress::default()).unwrap();
        let auto_gain = automatic.ch[0][24000..]
            .iter()
            .zip(&pure[24000..])
            .map(|(a, b)| (*a as f64) * (*b as f64))
            .sum::<f64>()
            / energy(&pure[24000..]);
        assert!(auto_gain > 0.85, "automatic tone protection {auto_gain}");
        assert_eq!(out.frames(), n);
        assert_eq!(out.rate, rate);
        assert!(out.ch.iter().flatten().all(|x| x.is_finite()));
        let partial = render(&sample, (16000, 60000), Some(&profile), 12., 1.5, &p).unwrap();
        assert_eq!(partial.ch[0][..16000], input[..16000]);
        assert_eq!(partial.ch[0][60001..], input[60001..]);
        assert_eq!(
            render(&sample, (0, n - 1), None, 0., 1.5, &p).unwrap(),
            sample
        );
        p.cancel.store(true, Ordering::Relaxed);
        assert!(render(&sample, (0, n - 1), Some(&profile), 12., 1.5, &p).is_err());
    }
    #[test]
    fn cleanup_short_silent_and_rates() {
        for rate in [8000, 44100, 96000, 192000, 384000] {
            let sample = Sample::from_channels(rate, vec![vec![0.; 1024]], Format::Float32);
            let out = render(&sample, (0, 1023), None, 36., 4., &Progress::default()).unwrap();
            assert_eq!(out, sample);
        }
    }
}
