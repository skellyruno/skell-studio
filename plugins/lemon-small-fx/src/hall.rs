// SPDX-License-Identifier: GPL-3.0-or-later
//! Lemon Hall: Dragonfly/Freeverb3 early reflections and modulated FDN tail.
use super::*;
use crate::params::leading_number;
use std::ffi::c_void;

unsafe extern "C" {
    fn lemon_hall_create(rate: f32) -> *mut c_void;
    fn lemon_hall_destroy(p: *mut c_void);
    fn lemon_hall_reset(p: *mut c_void);
    fn lemon_hall_set(p: *mut c_void, v: *const f32);
    fn lemon_hall_process(p: *mut c_void, l: *mut f32, r: *mut f32, n: i32) -> bool;
}
struct Hall {
    p: std::ptr::NonNull<c_void>,
    rate: u32,
    target: [f32; 10],
    current: [f32; 10],
}
// One native instance is owned exclusively by this Sound. No shared native state.
unsafe impl Send for Hall {}
impl Drop for Hall {
    fn drop(&mut self) {
        unsafe { lemon_hall_destroy(self.p.as_ptr()) }
    }
}
fn values(v: &[i32]) -> [f32; 10] {
    std::array::from_fn(|i| {
        let s = &SETTINGS[i];
        let n = v
            .get(i)
            .copied()
            .unwrap_or(s.start)
            .clamp(s.range.0, s.range.1) as f32;
        n / match i {
            0 => 100.,
            1 | 2 | 3 | 7 | 8 | 9 => 10.,
            6 => 1000.,
            _ => 1.,
        }
    })
}
impl Sound for Hall {
    fn set(&mut self, v: &[i32]) {
        self.target = values(v);
    }
    fn set_sample_rate(&mut self, rate: u32) {
        let rate = rate.clamp(8000, 384000);
        if rate != self.rate {
            let p = std::ptr::NonNull::new(unsafe { lemon_hall_create(rate as f32) })
                .expect("Hall allocation");
            unsafe { lemon_hall_destroy(self.p.as_ptr()) };
            self.p = p;
            self.rate = rate;
        }
        self.reset();
    }
    fn reset(&mut self) {
        self.current = self.target;
        unsafe {
            lemon_hall_reset(self.p.as_ptr());
            lemon_hall_set(self.p.as_ptr(), self.current.as_ptr());
        }
    }
    fn process(&mut self, l: &mut [f32], r: &mut [f32]) {
        for (l, r) in l.chunks_mut(64).zip(r.chunks_mut(64)) {
            let n = l.len().min(r.len());
            let retain = (-(n as f32) / (self.rate as f32 * 0.02)).exp();
            for i in 0..10 {
                self.current[i] = self.target[i] + (self.current[i] - self.target[i]) * retain;
                if (self.current[i] - self.target[i]).abs() < 1e-4 {
                    self.current[i] = self.target[i];
                }
            }
            for x in l.iter_mut().chain(r.iter_mut()) {
                if !x.is_finite() {
                    *x = 0.;
                } else {
                    *x = x.clamp(-64., 64.);
                }
            }
            let ok = unsafe {
                lemon_hall_set(self.p.as_ptr(), self.current.as_ptr());
                lemon_hall_process(self.p.as_ptr(), l.as_mut_ptr(), r.as_mut_ptr(), n as i32)
            };
            if !ok || l.iter().chain(r.iter()).any(|x| !x.is_finite()) {
                l.fill(0.);
                r.fill(0.);
                self.reset();
            }
        }
    }
}
fn sound(rate: u32, v: &[i32]) -> Box<dyn Sound> {
    let rate = rate.clamp(8000, 384000);
    let target = values(v);
    let p =
        std::ptr::NonNull::new(unsafe { lemon_hall_create(rate as f32) }).expect("Hall allocation");
    let mut s = Hall {
        p,
        rate,
        target,
        current: target,
    };
    s.reset();
    Box::new(s)
}
const SETTINGS: [Setting; 10] = [
    Setting {
        id: "decay",
        name: "Decay",
        range: (20, 1200),
        start: 240,
        magnet: None,
    },
    Setting {
        id: "predelay",
        name: "Pre-delay",
        range: (0, 2500),
        start: 180,
        magnet: None,
    },
    Setting {
        id: "early",
        name: "Early balance",
        range: (0, 1000),
        start: 250,
        magnet: None,
    },
    Setting {
        id: "diffusion",
        name: "Diffusion",
        range: (0, 1000),
        start: 850,
        magnet: None,
    },
    Setting {
        id: "low_cut",
        name: "Low cut",
        range: (20, 1000),
        start: 120,
        magnet: None,
    },
    Setting {
        id: "high_cut",
        name: "High cut",
        range: (1000, 20000),
        start: 8200,
        magnet: None,
    },
    Setting {
        id: "damping",
        name: "HF decay ratio",
        range: (100, 1000),
        start: 450,
        magnet: None,
    },
    Setting {
        id: "modulation",
        name: "Modulation",
        range: (0, 1000),
        start: 300,
        magnet: None,
    },
    Setting {
        id: "width",
        name: "Width",
        range: (0, 1000),
        start: 1000,
        magnet: None,
    },
    Setting {
        id: "mix",
        name: "Mix",
        range: (0, 1000),
        start: 250,
        magnet: None,
    },
];
const CONTROLS: [Control; 10] = [
    Control::Knob {
        id: "decay",
        cx: 66.,
        cy: 186.,
        r: 25.,
        caption: "",
        bipolar: false,
    },
    Control::Knob {
        id: "predelay",
        cx: 198.,
        cy: 186.,
        r: 25.,
        caption: "",
        bipolar: false,
    },
    Control::Knob {
        id: "early",
        cx: 330.,
        cy: 186.,
        r: 25.,
        caption: "",
        bipolar: false,
    },
    Control::Knob {
        id: "diffusion",
        cx: 462.,
        cy: 186.,
        r: 25.,
        caption: "",
        bipolar: false,
    },
    Control::Knob {
        id: "low_cut",
        cx: 594.,
        cy: 186.,
        r: 25.,
        caption: "",
        bipolar: false,
    },
    Control::Knob {
        id: "high_cut",
        cx: 66.,
        cy: 300.,
        r: 25.,
        caption: "",
        bipolar: false,
    },
    Control::Knob {
        id: "damping",
        cx: 198.,
        cy: 300.,
        r: 25.,
        caption: "",
        bipolar: false,
    },
    Control::Knob {
        id: "modulation",
        cx: 330.,
        cy: 300.,
        r: 25.,
        caption: "",
        bipolar: false,
    },
    Control::Knob {
        id: "width",
        cx: 462.,
        cy: 300.,
        r: 25.,
        caption: "",
        bipolar: false,
    },
    Control::Knob {
        id: "mix",
        cx: 594.,
        cy: 300.,
        r: 25.,
        caption: "",
        bipolar: false,
    },
];
fn text(k: usize, v: i32) -> String {
    match k {
        0 => format!("{:0.2} s", v as f32 / 100.),
        1 => format!("{:0.1} ms", v as f32 / 10.),
        4 | 5 => format!("{v} Hz"),
        6 => format!("{:0.3}x", v as f32 / 1000.),
        _ => format!("{:0.1}%", v as f32 / 10.),
    }
}
fn parse(k: usize, s: &str) -> Option<i32> {
    let n = leading_number(s)?;
    if !n.is_finite() {
        return None;
    }
    let s = &SETTINGS[k];
    let multiplier = match k {
        0 => 100.,
        4 | 5 => 1.,
        6 => 1000.,
        _ => 10.,
    };
    Some(
        (n * multiplier)
            .round()
            .clamp(s.range.0 as f64, s.range.1 as f64) as i32,
    )
}
fn space(c: &mut Canvas, r: Rect, v: &[i32]) {
    let gold = Color::rgb(0xffd94a);
    let dim = Color::rgb(0x91a2a8);
    let blue = Color::rgb(0x6daab4);
    c.fill_rrect(r, 6., Color::rgb(0x141b20));
    c.text(
        os_ui::Font::new(18),
        r.x + 16.,
        r.y + 12.,
        gold,
        "HALL SPACE",
    );
    c.text(
        os_ui::Font::new(10),
        r.x + 16.,
        r.y + 40.,
        dim,
        "REFLECTIONS + MODULATED TAIL",
    );
    c.text(
        os_ui::Font::new(10),
        r.x + 16.,
        r.y + 59.,
        dim,
        "DRAGONFLY / FREEVERB3",
    );
    let x0 = r.x + 245.;
    let y = r.bottom() - 13.;
    let width = r.right() - x0 - 16.;
    c.line(x0, y, r.right() - 16., y, 1., Color::rgb(0x435259));
    let delay = v[1] as f32 / 2500. * 45.;
    for i in 0..8 {
        let x = x0 + delay + i as f32 * 4.;
        let h = (1. - i as f32 / 10.) * (15. + v[2] as f32 * 0.025);
        c.line(x, y, x, y - h, 2., blue);
    }
    let start = x0 + delay + 34.;
    let length = (width - delay - 34.).max(1.);
    let mut last = (start, y - 50.);
    for i in 1..100 {
        let t = i as f32 / 99.;
        let decay = 2. + 6. * (1. - v[0] as f32 / 1200.);
        let point = (start + t * length, y - 50. * (-t * decay).exp());
        c.line(last.0, last.1, point.0, point.1, 2., gold);
        last = point;
    }
}
pub const SPEC: &Spec = &Spec {
    name: "ML Hall",
    word: "HALL",
    about: "Hall reverb powered by Dragonfly / Freeverb3 — GPL-3.0-or-later",
    settings: &SETTINGS,
    version: Some(1),
    any_version: false,
    fields: &[],
    tail: &[],
    size: (660, 390),
    cards: &[
        Card {
            x0: 8.,
            x1: 652.,
            y0: 18.,
            y1: 121.,
            heading: "SPACE",
        },
        Card {
            x0: 8.,
            x1: 652.,
            y0: 137.,
            y1: 238.,
            heading: "TIMING & REFLECTIONS",
        },
        Card {
            x0: 8.,
            x1: 652.,
            y0: 251.,
            y1: 352.,
            heading: "TONE & OUTPUT",
        },
    ],
    controls: &CONTROLS,
    display: Some(Display {
        r: Rect::new(16., 26., 628., 87.),
        draw: space,
    }),
    mark: Mark {
        x: 642.,
        y: 373.,
        anchor: 1.,
        stacked: false,
    },
    hint: Some(Rect::new(12., 362., 475., 20.)),
    labels: &[
        (66., 147., "DECAY"),
        (198., 147., "PRE-DELAY"),
        (330., 147., "EARLY BALANCE"),
        (462., 147., "DIFFUSION"),
        (594., 147., "LOW CUT"),
        (66., 261., "HIGH CUT"),
        (198., 261., "HF DECAY"),
        (330., 261., "MODULATION"),
        (462., 261., "WIDTH"),
        (594., 261., "MIX"),
    ],
    text,
    parse,
    sound,
    rings: true,
    blank: None,
    locate: None,
};
pub struct HallKind;
impl Kind for HallKind {
    const SPEC: &'static Spec = SPEC;
    const CLAP_ID: &'static str = "org.openstudio.lemon-hall";
    const CLAP_DESCRIPTION: &'static str =
        "Hall reverb with early reflections and a modulated tail; Dragonfly / Freeverb3";
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Reverb,
        ClapFeature::Stereo,
    ];
    const VST3_CLASS_ID: [u8; 16] = *b"LemonStudioHall1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Reverb];
}
pub type LemonHall = SmallFx<HallKind>;

#[cfg(test)]
mod tests {
    use super::*;
    fn impulse(rate: u32, v: &[i32], seconds: usize) -> (Vec<f32>, Vec<f32>) {
        let mut s = sound(rate, v);
        let mut l = vec![0.; rate as usize * seconds];
        let mut r = l.clone();
        l[0] = 0.5;
        r[0] = 0.5;
        s.process(&mut l, &mut r);
        (l, r)
    }
    #[test]
    fn hall_decay_predelay_width_and_dry() {
        let mut v = SPEC.start();
        v[9] = 1000;
        v[1] = 0;
        v[7] = 0;
        let (base, _) = impulse(48000, &v, 4);
        v[1] = 1000;
        let (delayed, _) = impulse(48000, &v, 4);
        assert!(delayed[..4800].iter().all(|x| x.abs() < 1e-10));
        assert!(base[..4800].iter().any(|x| x.abs() > 1e-5));
        // With modulation off, wet predelay is an exact timeline shift.
        assert!(base[..48000]
            .iter()
            .zip(&delayed[4800..])
            .all(|(a, b)| (a - b).abs() < 1e-5));
        v[1] = 0;
        v[0] = 60;
        let (short, _) = impulse(48000, &v, 4);
        v[0] = 600;
        let (long, right) = impulse(48000, &v, 4);
        let energy = |s: &[f32]| s[96000..].iter().map(|x| (*x as f64).powi(2)).sum::<f64>();
        assert!(energy(&long) > energy(&short) * 10.);
        assert!(long.iter().zip(&right).any(|(a, b)| (a - b).abs() > 1e-5));
        v[8] = 0;
        let (l, r) = impulse(48000, &v, 1);
        assert!(l.iter().zip(&r).all(|(a, b)| (a - b).abs() < 1e-5));
        v[9] = 0;
        let (l, r) = impulse(48000, &v, 1);
        assert_eq!(l[0], 0.5);
        assert_eq!(r[0], 0.5);
        assert!(l[1..].iter().chain(&r[1..]).all(|x| *x == 0.));
    }
    #[test]
    fn hall_rates_extremes_automation_reset() {
        for rate in [8000, 22050, 44100, 48000, 96000, 192000, 384000] {
            let mut s = sound(rate, &SPEC.start());
            for (max, late_only) in [(false, false), (true, false), (true, true), (false, true)] {
                let mut v: Vec<i32> = SETTINGS
                    .iter()
                    .map(|s| if max { s.range.1 } else { s.range.0 })
                    .collect();
                if late_only {
                    v[2] = 0;
                    v[9] = 1000;
                }
                s.set(&v);
                let mut l = vec![0.; rate as usize / 2];
                let mut r = l.clone();
                l[0] = 1.;
                r[0] = 0.5;
                s.process(&mut l, &mut r);
                assert!(
                    l.iter().chain(&r).all(|x| x.is_finite() && x.abs() < 16.),
                    "rate {rate}, max setting {max}, peak {}",
                    l.iter().chain(&r).map(|x| x.abs()).fold(0.0_f32, f32::max)
                );
            }
            s.reset();
            let mut l = vec![0.; 8192];
            let mut r = l.clone();
            s.process(&mut l, &mut r);
            assert!(l.iter().chain(&r).all(|x| x.abs() < 1e-10));
            s.set_sample_rate(48000);
            s.process(&mut l, &mut r);
            assert!(l.iter().chain(&r).all(|x| x.is_finite()));
        }
    }
    #[test]
    fn hall_parameters_and_panel() {
        for (k, s) in SETTINGS.iter().enumerate() {
            for v in s.range.0..=s.range.1 {
                assert_eq!(parse(k, &text(k, v)), Some(v));
            }
        }
        let mut logic = editor::Logic::new(
            SPEC,
            Arc::new(params::FxParams::new(SPEC)),
            Arc::new(Shared::default()),
            None,
        );
        let mut c = Canvas::new(660, 390, 1.).unwrap();
        logic.paint(&mut c, &os_plugkit::palette());
        assert_eq!(
            logic
                .hits
                .iter()
                .filter(|h| matches!(h.t, view::Target::Knob(_)))
                .count(),
            10
        );
        if let Some(dir) = std::env::var_os("OS_SHOT_DIR") {
            c.pixmap
                .save_png(std::path::PathBuf::from(dir).join("lemon-hall.png"))
                .unwrap();
        }
    }
}
