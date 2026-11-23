//! Ambient effects around the bouquet (hearts, petals, fireflies, snow,
//! stars, sparkles) plus one-off heart bursts.

use std::f32::consts::TAU;

use crate::bouquet::Effect;
use crate::camera::{Camera, Viewport};
use crate::canvas::Canvas;
use crate::color::Rgb;
use crate::math::Vec3;
use crate::rng::Rng;

// Depths for flat, screen-space particles.
const SKY: f32 = -1.0e6;
const BEHIND: f32 = -1.0e5;
const IN_FRONT: f32 = 1.0e5;

const HEART_COLORS: [Rgb; 6] = [
    Rgb(200, 30, 100),
    Rgb(240, 50, 120),
    Rgb(255, 90, 150),
    Rgb(255, 120, 170),
    Rgb(255, 160, 200),
    Rgb(255, 200, 225),
];
const PETAL_CHARS: [char; 4] = [',', '\'', '`', '~'];

/// Something drifting across the view. `x` and `y` run 0..1 across it.
struct Drift {
    x: f32,
    y: f32,
    speed: f32,
    phase: f32,
    color: Rgb,
    front: bool,
}

/// Something hovering in 3D around the bouquet.
struct Orbit {
    radius: f32,
    height: f32,
    angle: f32,
    speed: f32,
    phase: f32,
    blink: f32,
}

/// A heart flung out by a burst, in terminal cells.
struct Spark {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    color: Rgb,
}

pub struct Effects {
    on: [bool; Effect::COUNT],
    rng: Rng,
    time: f32,
    hearts: Vec<Drift>,
    petals: Vec<Drift>,
    snow: Vec<Drift>,
    stars: Vec<Drift>,
    fireflies: Vec<Orbit>,
    sparkles: Vec<Orbit>,
    sparks: Vec<Spark>,
}

/// Fills `v` up to `n` items while the effect is on, and empties it when off.
fn top_up<T>(v: &mut Vec<T>, on: bool, n: usize, rng: &mut Rng, make: fn(&mut Rng) -> T) {
    if !on {
        v.clear();
        return;
    }
    while v.len() < n {
        v.push(make(rng));
    }
}

fn new_heart(rng: &mut Rng) -> Drift {
    Drift {
        x: rng.f32(),
        y: rng.range(0.2, 1.2),
        speed: rng.range(0.03, 0.08),
        phase: rng.range(0.0, TAU),
        color: *rng.pick(&HEART_COLORS),
        front: false,
    }
}

fn new_faller(rng: &mut Rng) -> Drift {
    Drift {
        x: rng.f32(),
        y: rng.range(-1.0, 1.0),
        speed: rng.range(0.04, 0.12),
        phase: rng.range(0.0, TAU),
        color: Rgb::WHITE,
        front: rng.chance(0.5),
    }
}

fn new_star(rng: &mut Rng) -> Drift {
    Drift {
        x: rng.f32(),
        y: rng.f32(),
        speed: rng.range(0.5, 2.0),
        phase: rng.range(0.0, TAU),
        color: Rgb::WHITE,
        front: false,
    }
}

fn new_firefly(rng: &mut Rng) -> Orbit {
    let dir = if rng.chance(0.5) { 1.0 } else { -1.0 };
    Orbit {
        radius: rng.range(0.55, 1.0),
        height: rng.range(-0.1, 0.9),
        angle: rng.range(0.0, TAU),
        speed: dir * rng.range(0.1, 0.35),
        phase: rng.range(0.0, TAU),
        blink: rng.range(1.5, 3.0),
    }
}

fn new_sparkle(rng: &mut Rng) -> Orbit {
    Orbit {
        radius: rng.range(0.75, 0.95),
        height: rng.range(-0.3, 1.0),
        angle: rng.range(0.0, TAU),
        speed: 0.0,
        phase: rng.range(0.0, TAU),
        blink: rng.range(1.5, 4.0),
    }
}

impl Effects {
    pub fn new(seed: u64) -> Self {
        Effects {
            on: [false; Effect::COUNT],
            rng: Rng::new(seed),
            time: 0.0,
            hearts: Vec::new(),
            petals: Vec::new(),
            snow: Vec::new(),
            stars: Vec::new(),
            fireflies: Vec::new(),
            sparkles: Vec::new(),
            sparks: Vec::new(),
        }
    }

    pub fn set(&mut self, list: &[Effect]) {
        self.on = [false; Effect::COUNT];
        for &e in list {
            self.on[e as usize] = true;
        }
    }

    pub fn enable(&mut self, e: Effect) {
        self.on[e as usize] = true;
    }

    /// Flips an effect and returns whether it is now on.
    pub fn toggle(&mut self, e: Effect) -> bool {
        let slot = &mut self.on[e as usize];
        *slot = !*slot;
        *slot
    }

    /// Flings `count` hearts out from a point (in cells).
    pub fn burst(&mut self, x: f32, y: f32, count: usize) {
        for _ in 0..count {
            let a = self.rng.range(0.0, TAU);
            let speed = self.rng.range(4.0, 20.0);
            self.sparks.push(Spark {
                x,
                y,
                vx: a.cos() * speed * 2.0,
                vy: a.sin() * speed - 6.0,
                life: self.rng.range(1.2, 2.8),
                color: *self.rng.pick(&HEART_COLORS),
            });
        }
    }

    /// Moves everything forward by `dt` seconds. `palette` colors the petals.
    pub fn update(&mut self, dt: f32, palette: &[Rgb]) {
        self.time += dt;
        let on = |e: Effect| self.on[e as usize];
        let rng = &mut self.rng;

        top_up(&mut self.hearts, on(Effect::Hearts), 18, rng, new_heart);
        for h in &mut self.hearts {
            h.y -= h.speed * dt;
            if h.y < -0.05 {
                *h = Drift {
                    y: 1.05,
                    ..new_heart(rng)
                };
            }
        }

        let had_petals = self.petals.len();
        top_up(&mut self.petals, on(Effect::Petals), 14, rng, new_faller);
        for (i, p) in self.petals.iter_mut().enumerate() {
            if i >= had_petals {
                p.color = petal_color(rng, palette);
            }
            p.y += p.speed * dt;
            p.x += 0.02 * dt;
            if p.y > 1.02 {
                *p = Drift {
                    y: -0.02,
                    color: petal_color(rng, palette),
                    ..new_faller(rng)
                };
            }
        }

        top_up(&mut self.snow, on(Effect::Snow), 45, rng, new_faller);
        for f in &mut self.snow {
            f.y += f.speed * dt;
            if f.y > 1.02 {
                *f = Drift {
                    y: -0.02,
                    ..new_faller(rng)
                };
            }
        }

        top_up(&mut self.stars, on(Effect::Stars), 70, rng, new_star);
        top_up(
            &mut self.fireflies,
            on(Effect::Fireflies),
            10,
            rng,
            new_firefly,
        );
        top_up(
            &mut self.sparkles,
            on(Effect::Sparkles),
            30,
            rng,
            new_sparkle,
        );
        for f in &mut self.fireflies {
            f.angle += f.speed * dt;
        }

        for s in &mut self.sparks {
            s.vy += 9.0 * dt;
            s.x += s.vx * dt;
            s.y += s.vy * dt;
            s.life -= dt;
        }
        self.sparks.retain(|s| s.life > 0.0);
    }

    pub fn draw(&self, canvas: &mut Canvas, cam: &Camera, view: Viewport) {
        let t = self.time;
        let cell = |x: f32, y: f32| {
            (
                view.x + (x.rem_euclid(1.0) * view.w as f32) as i32,
                view.y + (y * view.h as f32).floor() as i32,
            )
        };
        let depth = |front: bool| if front { IN_FRONT } else { BEHIND };

        for s in &self.stars {
            let v = (t * s.speed + s.phase).sin();
            let ch = if v > 0.92 {
                '*'
            } else if v > 0.55 {
                '+'
            } else if v > -0.3 {
                '.'
            } else {
                continue;
            };
            let (x, y) = cell(s.x, s.y);
            canvas.plot(
                x,
                y,
                SKY,
                ch,
                Rgb(70, 75, 110).lerp(Rgb(230, 230, 255), (v + 1.0) / 2.0),
            );
        }

        for h in &self.hearts {
            let (x, y) = cell(h.x + 0.015 * (t * 1.2 + h.phase).sin(), h.y);
            canvas.plot(x, y, BEHIND, '♥', h.color);
        }

        for p in &self.petals {
            let (x, y) = cell(p.x + 0.03 * (t * 1.1 + p.phase).sin(), p.y);
            let ch = PETAL_CHARS[((t * 2.5 + p.phase) as usize) % PETAL_CHARS.len()];
            canvas.plot(x, y, depth(p.front), ch, p.color);
        }

        for f in &self.snow {
            let (x, y) = cell(f.x + 0.01 * (t * 0.8 + f.phase).sin(), f.y);
            let (ch, color) = if f.speed > 0.09 {
                ('*', Rgb(235, 240, 255))
            } else if f.speed > 0.06 {
                ('·', Rgb(210, 220, 240))
            } else {
                ('.', Rgb(170, 180, 205))
            };
            canvas.plot(x, y, depth(f.front), ch, color);
        }

        for f in &self.fireflies {
            let a = f.angle;
            let pos = Vec3::new(
                f.radius * a.cos(),
                f.height + 0.08 * (t * 1.3 + f.phase).sin(),
                f.radius * a.sin(),
            );
            let glow = (0.5 + 0.5 * (t * f.blink + f.phase).sin()).powi(2);
            let ch = if glow > 0.6 {
                '*'
            } else if glow > 0.25 {
                '+'
            } else if glow > 0.05 {
                '.'
            } else {
                continue;
            };
            let p = cam.project(pos);
            canvas.plot(
                p.x.floor() as i32,
                p.y.floor() as i32,
                p.z,
                ch,
                Rgb(80, 90, 30).lerp(Rgb(235, 255, 130), glow),
            );
        }

        for s in &self.sparkles {
            let pos = Vec3::new(s.radius * s.angle.cos(), s.height, s.radius * s.angle.sin());
            let tw = (t * s.blink + s.phase).sin();
            let (ch, color) = if tw > 0.85 {
                ('*', Rgb(255, 255, 255))
            } else if tw > 0.4 {
                ('+', Rgb(255, 215, 240))
            } else if tw > 0.0 {
                ('·', Rgb(255, 175, 215))
            } else {
                continue;
            };
            let p = cam.project(pos);
            canvas.plot(p.x.floor() as i32, p.y.floor() as i32, p.z, ch, color);
        }

        for s in &self.sparks {
            let color = Rgb(60, 20, 40).lerp(s.color, (s.life / 0.6).min(1.0));
            canvas.plot(s.x.floor() as i32, s.y.floor() as i32, IN_FRONT, '♥', color);
        }
    }
}

fn petal_color(rng: &mut Rng, palette: &[Rgb]) -> Rgb {
    if palette.is_empty() {
        *rng.pick(&HEART_COLORS)
    } else {
        *rng.pick(palette)
    }
}
