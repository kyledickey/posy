//! What the stems sit in: a paper cone, glass vase, mason jar, basket, or
//! nothing but a ribbon.

use std::f32::consts::{PI, TAU};

use super::Builder;
use crate::bouquet::WrapStyle;
use crate::color::{Col, dim, mix};
use crate::math::{Vec3, smoothstep};
use crate::mesh::{Mat, P, Patch};

const WHITE: Col = [1.0, 0.98, 0.95];
/// Paper and wicker are drawn lighter than petals.
const PAPER_INK: f32 = 0.55;
const WATER: Col = [0.45, 0.72, 0.95];

/// Stem ends showing below the wrap.
pub struct Tail {
    pub top: f32,
    pub bottom: f32,
    pub r0: f32,
    pub spread: f32,
    pub count: usize,
}

/// Where the rest of the bouquet attaches to the wrap.
pub struct WrapInfo {
    /// Where all the stems meet.
    pub neck: Vec3,
    pub tail: Option<Tail>,
    /// Height and radius of the ring of leaves.
    pub leaf_y: f32,
    pub leaf_r: f32,
    /// Height of the middle of the dome of flowers.
    pub dome_y: f32,
    /// The lowest point of the whole thing.
    pub bottom: f32,
}

impl WrapInfo {
    /// Scales every measurement, to match a wrap that has been resized.
    pub fn scale(&mut self, k: f32) {
        self.neck = self.neck * k;
        self.leaf_y *= k;
        self.leaf_r *= k;
        self.dome_y *= k;
        self.bottom *= k;
        if let Some(tail) = &mut self.tail {
            tail.top *= k;
            tail.bottom *= k;
            tail.r0 *= k;
        }
    }
}

pub fn build(b: &mut Builder, style: WrapStyle, wrap: Col, ribbon: Col) -> WrapInfo {
    match style {
        WrapStyle::Cone => cone(b, wrap, ribbon),
        WrapStyle::Vase => vase(b, wrap, ribbon),
        WrapStyle::Jar => jar(b, wrap, ribbon),
        WrapStyle::Basket => basket(b, wrap, ribbon),
        WrapStyle::Bare => bare(b, ribbon),
    }
}

/// A point at height `y`, distance `r` from the middle, direction `a` around.
fn ring(r: f32, y: f32, a: f32) -> Vec3 {
    Vec3::new(r * a.cos(), y, r * a.sin())
}

/// Pleated paper wrapped into a cone, taller at the back, with a second
/// sheet of pale tissue inside, tied with a bow.
fn cone(b: &mut Builder, paper: Col, ribbon: Col) -> WrapInfo {
    let (y0, y1, r0, r1) = (-0.24f32, 0.33f32, 0.045f32, 0.46f32);
    let tissue = mix(paper, WHITE, 0.6);
    for (layer, color) in [(0, paper), (1, tissue)] {
        let inner = layer == 1;
        let phase = b.rng.range(0.0, TAU);
        let folds = if inner { 11.0 } else { 9.0 };
        b.scene.base.patch(
            Patch::new(14, 72)
                .closed()
                .ink(PAPER_INK)
                .born(0.0, if inner { 0.9 } else { 0.6 }),
            |u, v| {
                let a = v * TAU;
                // A wavy top edge that rises toward the back.
                let rim = 1.0 + 0.07 * (3.0 * a + phase).sin() + 0.04 * (7.0 * a - phase).sin()
                    - 0.10 * a.sin();
                let t = u * if inner { rim * 1.07 } else { rim };
                let crease = (folds * a + phase).sin().abs();
                let r = (r0 + (r1 - r0) * t.powf(0.9)) * (1.0 + 0.05 * (crease - 0.6) * t)
                    + 0.10 * t.powi(4);
                let r = if inner { r * 0.93 } else { r };
                let y = y0 + (y1 - y0) * t;
                P::new(ring(r, y, a), color).shade(0.82 + 0.18 * crease)
            },
        );
    }
    let band_y = -0.11;
    let band_r = r0 + (r1 - r0) * ((band_y - y0) / (y1 - y0)).powf(0.9) + 0.012;
    band(b, band_y, band_r, ribbon, 0.7);
    bow(b, Vec3::new(0.0, band_y, band_r + 0.012), ribbon, 0.8, 1.0);
    WrapInfo {
        neck: Vec3::new(0.0, -0.12, 0.0),
        tail: Some(Tail {
            top: -0.22,
            bottom: -0.52,
            r0: 0.02,
            spread: 1.6,
            count: 7,
        }),
        leaf_y: 0.20,
        leaf_r: 0.22,
        // High enough that the lowest heads clear the paper's edge.
        dome_y: 0.24,
        bottom: -0.52,
    }
}

/// A surface made by spinning the outline `radius(t)` around the middle.
fn turned(
    b: &mut Builder,
    (bottom, top): (f32, f32),
    opts: Patch,
    radius: impl Fn(f32) -> f32,
    color: Col,
) {
    b.scene.base.patch(opts.closed(), |u, v| {
        P::new(ring(radius(u), bottom + (top - bottom) * u, v * TAU), color)
    });
}

/// A flat disc: the bottom of a container, or the surface of the water.
fn disc(b: &mut Builder, y: f32, r: f32, color: Col, mat: Mat) {
    b.scene
        .base
        .patch(Patch::new(2, 20).closed().mat(mat), |u, v| {
            P::new(ring(r * u, y, -v * TAU), color)
        });
}

/// A ring of round cord at height `y`.
fn hoop(
    b: &mut Builder,
    y: f32,
    r: f32,
    thick: f32,
    mat: Mat,
    born: f32,
    color: impl Fn(f32) -> Col,
) {
    let path: Vec<Vec3> = (0..=36)
        .map(|j| ring(r, y, j as f32 / 36.0 * TAU))
        .collect();
    b.scene.base.tube(
        &path,
        5,
        Patch::new(36, 5).mat(mat).born(born, 0.0),
        |_| thick,
        color,
    );
}

/// Curvy glass vase with water. Stems show through the glass.
fn vase(b: &mut Builder, glass: Col, ribbon: Col) -> WrapInfo {
    let (bottom, top) = (-0.55f32, 0.22f32);
    let height = top - bottom;
    // Narrow foot, round belly, slim neck, flared lip.
    let radius = |t: f32| {
        0.09 + 0.15 * (PI * t * 1.1).sin().max(0.0).powf(1.3) + 0.07 * smoothstep(0.75, 1.0, t)
    };
    let level = 0.55;
    let water = mix(glass, WATER, 0.6);
    turned(
        b,
        (bottom + 0.01, bottom + height * level),
        Patch::new(10, 28).mat(Mat::Glass(0.16)).born(0.2, 0.5),
        |t| radius(t * level) - 0.012,
        water,
    );
    disc(
        b,
        bottom + height * level,
        radius(level) - 0.012,
        mix(water, WHITE, 0.3),
        Mat::Glass(0.4),
    );
    turned(
        b,
        (bottom, top),
        Patch::new(20, 32).mat(Mat::Glass(0.07)).born(0.0, 0.6),
        radius,
        glass,
    );
    disc(b, bottom, radius(0.0), glass, Mat::Glass(0.35));
    hoop(b, top, radius(1.0), 0.008, Mat::Glass(0.75), 0.6, |_| {
        mix(glass, WHITE, 0.4)
    });

    let band_y = bottom + height * 0.86;
    band(b, band_y, radius(0.86) + 0.008, ribbon, 0.7);
    bow(
        b,
        Vec3::new(0.0, band_y, radius(0.86) + 0.02),
        ribbon,
        0.8,
        0.85,
    );
    WrapInfo {
        neck: Vec3::new(0.0, 0.06, 0.0),
        tail: Some(Tail {
            top: 0.06,
            bottom: -0.5,
            r0: 0.02,
            spread: 2.5,
            count: 7,
        }),
        leaf_y: 0.20,
        leaf_r: 0.10,
        dome_y: 0.12,
        bottom: -0.55,
    }
}

/// Mason jar: a glass cylinder with screw threads and twine.
fn jar(b: &mut Builder, glass: Col, twine: Col) -> WrapInfo {
    let (bottom, top, r) = (-0.5f32, 0.14f32, 0.2f32);
    let height = top - bottom;
    // Rounded at the bottom, pinched in a little at the mouth.
    let radius = |t: f32| {
        r * (0.8 + 0.2 * smoothstep(0.0, 0.1, t)) * (1.0 - 0.10 * smoothstep(0.82, 0.9, t))
    };
    let level = 0.6;
    let water = mix(glass, WATER, 0.6);
    turned(
        b,
        (bottom + 0.01, bottom + height * level),
        Patch::new(8, 28).mat(Mat::Glass(0.16)).born(0.2, 0.5),
        |t| radius(t * level) - 0.012,
        water,
    );
    disc(
        b,
        bottom + height * level,
        r - 0.012,
        mix(water, WHITE, 0.3),
        Mat::Glass(0.4),
    );
    turned(
        b,
        (bottom, top),
        Patch::new(18, 32).mat(Mat::Glass(0.07)).born(0.0, 0.6),
        radius,
        glass,
    );
    disc(b, bottom, radius(0.0), glass, Mat::Glass(0.35));
    for dy in [0.0f32, -0.028, -0.056] {
        hoop(
            b,
            top + dy,
            radius(1.0) + 0.004,
            0.007,
            Mat::Glass(0.7),
            0.6,
            |_| mix(glass, WHITE, 0.4),
        );
    }
    let twine_y = top - 0.13;
    band(b, twine_y, r + 0.008, twine, 0.7);
    bow(b, Vec3::new(0.0, twine_y, r + 0.02), twine, 0.8, 0.8);
    WrapInfo {
        neck: Vec3::new(0.0, 0.02, 0.0),
        tail: Some(Tail {
            top: 0.02,
            bottom: -0.46,
            r0: 0.02,
            spread: 3.5,
            count: 7,
        }),
        leaf_y: 0.14,
        leaf_r: 0.12,
        dome_y: 0.12,
        bottom: -0.50,
    }
}

/// Woven basket with a braided rim and a handle over the top.
fn basket(b: &mut Builder, wicker: Col, ribbon: Col) -> WrapInfo {
    let (bottom, top) = (-0.42f32, 0.10f32);
    let height = top - bottom;
    let radius = |t: f32| 0.17 + 0.19 * t.sqrt();
    let (dark, light) = (dim(wicker, 0.6), mix(wicker, WHITE, 0.25));
    // Over-under weave: a checkerboard of little bumps.
    let (rows, strands) = (9.0, 26.0);
    b.scene.base.patch(
        Patch::new(36, 104).closed().ink(0.7).born(0.0, 0.6),
        |u, v| {
            let bump = (u * rows * PI).sin() * (v * strands * TAU / 2.0).sin();
            let r = radius(u) * (1.0 + 0.03 * bump);
            let color = mix(dark, light, 0.5 + 0.5 * bump);
            P::new(ring(r, bottom + height * u, v * TAU), color).shade(0.85 + 0.15 * bump)
        },
    );
    disc(b, bottom, radius(0.0), dark, Mat::Solid);

    let braid = move |t: f32| mix(dark, light, 0.5 + 0.5 * (t * 40.0 * TAU).sin());
    hoop(b, top, radius(1.0) + 0.012, 0.022, Mat::Solid, 0.6, braid);

    // Handle arching over the flowers.
    let handle_r = radius(1.0) * 1.02;
    let path: Vec<Vec3> = (0..=28)
        .map(|j| {
            let th = j as f32 / 28.0 * PI;
            Vec3::new(handle_r * th.cos(), top + 0.92 * th.sin(), 0.0)
        })
        .collect();
    b.scene.base.tube(
        &path,
        5,
        Patch::new(28, 5).born(0.6, 0.5),
        |_| 0.017,
        move |t| mix(dark, light, 0.5 + 0.5 * (t * 34.0 * TAU).sin()),
    );
    bow(
        b,
        Vec3::new(0.0, top - 0.06, radius(0.85) + 0.03),
        ribbon,
        0.8,
        1.0,
    );
    WrapInfo {
        neck: Vec3::new(0.0, -0.02, 0.0),
        tail: None,
        leaf_y: 0.10,
        leaf_r: 0.26,
        dome_y: 0.12,
        bottom: -0.42,
    }
}

/// No container: the stems are simply tied together.
fn bare(b: &mut Builder, ribbon: Col) -> WrapInfo {
    let tie_y = -0.14;
    band(b, tie_y, 0.05, ribbon, 0.7);
    bow(b, Vec3::new(0.0, tie_y, 0.065), ribbon, 0.8, 1.0);
    WrapInfo {
        neck: Vec3::new(0.0, -0.1, 0.0),
        tail: Some(Tail {
            top: -0.1,
            bottom: -0.6,
            r0: 0.03,
            spread: 0.8,
            count: 9,
        }),
        leaf_y: 0.04,
        leaf_r: 0.04,
        dome_y: 0.12,
        bottom: -0.60,
    }
}

/// A ribbon wrapped around at height `y`.
fn band(b: &mut Builder, y: f32, r: f32, color: Col, born: f32) {
    b.scene
        .base
        .patch(Patch::new(2, 28).closed().born(born, 0.0), |u, v| {
            let bulge = 0.006 * (u * PI).sin();
            P::new(ring(r + bulge, y - 0.02 + 0.04 * u, v * TAU), color)
        });
}

/// A bow facing +z: two puffy loops, two trailing tails and a knot.
fn bow(b: &mut Builder, knot: Vec3, color: Col, born: f32, size: f32) {
    let (deep, pale) = (dim(color, 0.7), mix(color, WHITE, 0.3));
    for side in [-1.0f32, 1.0] {
        // Each loop is a teardrop fanned out from the knot.
        b.scene
            .base
            .patch(Patch::new(4, 16).closed().born(born + 0.1, 0.3), |u, v| {
                let a = v * TAU;
                let reach = 0.17 * (a / 2.0).sin();
                let x = side * reach;
                let y = 0.075 * a.sin() * (0.35 + 0.65 * (a / 2.0).sin()) + 0.3 * reach;
                let puff = 0.035 * (u * PI).sin();
                let p = knot + Vec3::new(x * u, y * u, 0.008 + puff) * size;
                // Folds gathering toward the knot.
                let fold = 0.5 + 0.5 * (a * 3.0).cos();
                P::new(p, mix(deep, pale, u * 0.8)).shade(0.7 + 0.3 * u - 0.15 * fold * (1.0 - u))
            });
        // A tail with a notch cut in its end.
        b.scene
            .base
            .patch(Patch::new(6, 2).born(born + 0.2, 0.4), |u, v| {
                let mid = Vec3::new(side * (0.02 + 0.07 * u), -0.21 * u, 0.012 * (u * 7.0).sin());
                let half = 0.02 + 0.014 * u;
                let notch = if u > 0.99 {
                    0.035 * (1.0 - v.abs())
                } else {
                    0.0
                };
                let p = knot + (mid + Vec3::new(v * half, notch, 0.006)) * size;
                P::new(p, mix(color, deep, u * 0.5)).shade(0.85 + 0.15 * (u * 9.0).sin())
            });
    }
    b.scene.base.blob(
        knot + Vec3::new(0.0, 0.0, 0.02 * size),
        0.026 * size,
        Vec3::UP,
        1.1,
        pale,
        Patch::new(0, 0).born(born, 0.0),
    );
}
