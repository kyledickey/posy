//! Draws triangles into a picture several times finer than the terminal,
//! then turns each cell of that picture into the one character whose shape
//! fits it best.
//!
//! That second step is what makes outlines crisp: a cell that is only lit
//! along its bottom becomes `_`, one cut by a diagonal becomes `/`, and a
//! fully lit one becomes something dense like `@`.

use serde::{Deserialize, Serialize};

use crate::canvas::Canvas;
use crate::color::{Col, ColorMode, Rgb, dim, mix};
use crate::glyphs::GLYPHS;
use crate::octants::OCTANTS;

/// What the bouquet is drawn with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Style {
    /// Letters and symbols
    #[default]
    Ascii,
    /// Little coloured blocks; works in any terminal with full colour
    Blocks,
    /// Smaller coloured blocks; the sharpest picture, needs a recent terminal
    Fine,
}

impl Style {
    pub const ALL: [Style; 3] = [Style::Ascii, Style::Blocks, Style::Fine];

    pub fn label(self) -> &'static str {
        match self {
            Style::Ascii => "letters",
            Style::Blocks => "blocks",
            Style::Fine => "fine blocks",
        }
    }

    /// The best style this terminal is likely to draw well.
    pub fn detect(mode: ColorMode) -> Style {
        if mode != ColorMode::TrueColor {
            return Style::Ascii;
        }
        let var = |name: &str| std::env::var(name).unwrap_or_default().to_lowercase();
        let (term, program) = (var("TERM"), var("TERM_PROGRAM"));
        // The smallest blocks are new characters only some terminals draw.
        let draws_fine = ["ghostty", "kitty"]
            .iter()
            .any(|name| term.contains(name) || program.contains(name));
        if draws_fine {
            Style::Fine
        } else {
            Style::Blocks
        }
    }

    /// Blocks paint colour rather than characters, like a picture.
    pub fn is_picture(self) -> bool {
        matches!(self, Style::Blocks | Style::Fine)
    }
}

/// Samples per terminal cell, across and down. Cells are about twice as
/// tall as wide, so this makes the samples square.
pub const SX: usize = 4;
pub const SY: usize = 8;
const N: usize = SX * SY;

/// How much nearer (in world units) something must be to cast a shadow line.
const EDGE_GAP: f32 = 0.006;
/// Sharpens the difference between the lit and unlit parts of a cell.
const CONTRAST: f32 = 1.7;

/// Characters for filled areas, sparse to dense. A short, even ramp reads
/// as shading; using every character there is reads as noise.
const RAMP: &str = ".:-=+*o#%&@$";
/// Characters for outlines, where a cell is only partly covered.
const EDGE_GLYPHS: &str = ".,_-~'\"`^/\\|()<>;:=+*oO#%@&$";
/// The fullest a filled cell gets, as a fraction of the densest character.
const DENSEST: f32 = 0.85;

/// A triangle corner, already projected: `x`/`y` in samples, `z` toward
/// the viewer.
#[derive(Clone, Copy, Debug)]
pub struct Pv {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub color: Col,
    /// Brightness 0..1. Decides how dense a character is used.
    pub lum: f32,
    /// Opacity (1 for everything but glass).
    pub alpha: f32,
}

/// A triangle ready to be filled.
#[derive(Clone, Copy, Debug)]
pub struct Shape {
    pub corners: [Pv; 3],
    pub part: u16,
}

/// A horizontal strip of the picture, which one thread fills on its own.
struct Band<'a> {
    w: usize,
    /// The picture row this strip starts at, and how many rows it has.
    top: usize,
    rows: usize,
    z: &'a mut [f32],
    color: &'a mut [Col],
    lum: &'a mut [f32],
    alpha: &'a mut [f32],
    part: &'a mut [u16],
}

impl Band<'_> {
    fn triangle(&mut self, shape: &Shape, glass: bool) {
        let [a, b, c] = shape.corners;
        let (top, bottom) = (self.top as f32, (self.top + self.rows) as f32);
        let (lowest, highest) = (a.y.min(b.y).min(c.y), a.y.max(b.y).max(c.y));
        if highest < top || lowest >= bottom {
            return;
        }
        let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        if area.abs() < 1e-5 {
            return;
        }
        let x0 = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
        let x1 = (a.x.max(b.x).max(c.x).ceil() as isize).min(self.w as isize - 1);
        if x1 < 0 {
            return;
        }
        let y0 = lowest.floor().max(top) as usize;
        let y1 = (highest.ceil() as usize).min(self.top + self.rows - 1);
        let inv = 1.0 / area;
        for py in y0..=y1 {
            let y = py as f32 + 0.5;
            let row = (py - self.top) * self.w;
            for px in x0..=x1 as usize {
                let x = px as f32 + 0.5;
                // How much of each corner this sample is made of.
                let wa = ((b.x - x) * (c.y - y) - (b.y - y) * (c.x - x)) * inv;
                let wb = ((c.x - x) * (a.y - y) - (c.y - y) * (a.x - x)) * inv;
                let wc = 1.0 - wa - wb;
                if wa < 0.0 || wb < 0.0 || wc < 0.0 {
                    continue;
                }
                let z = wa * a.z + wb * b.z + wc * c.z;
                let i = row + px;
                if z <= self.z[i] && !(glass && self.alpha[i] < 1.0) {
                    continue;
                }
                let color = [
                    wa * a.color[0] + wb * b.color[0] + wc * c.color[0],
                    wa * a.color[1] + wb * b.color[1] + wc * c.color[1],
                    wa * a.color[2] + wb * b.color[2] + wc * c.color[2],
                ];
                let lum = wa * a.lum + wb * b.lum + wc * c.lum;
                if glass {
                    let k = (wa * a.alpha + wb * b.alpha + wc * c.alpha).clamp(0.0, 1.0);
                    let under = self.alpha[i];
                    if under > 0.0 {
                        self.color[i] = mix(self.color[i], color, k);
                        self.lum[i] += (lum - self.lum[i]) * k;
                    } else {
                        self.color[i] = color;
                        self.lum[i] = lum;
                    }
                    self.alpha[i] = under + (1.0 - under) * k;
                    if under == 0.0 {
                        self.part[i] = shape.part;
                    }
                    self.z[i] = self.z[i].max(z);
                } else {
                    self.z[i] = z;
                    self.color[i] = color;
                    self.lum[i] = lum;
                    self.alpha[i] = 1.0;
                    self.part[i] = shape.part;
                }
            }
        }
    }
}

pub struct Frame {
    cols: usize,
    rows: usize,
    w: usize,
    h: usize,
    z: Vec<f32>,
    color: Vec<Col>,
    lum: Vec<f32>,
    alpha: Vec<f32>,
    part: Vec<u16>,
    /// Parts from this number on belong to flowers, `per_flower` each.
    pub flower_parts: u16,
    pub per_flower: u16,
    pub style: Style,
    /// A guess at the terminal's background, for seeing through glass.
    pub back: Col,
    /// For each of the six zones of a cell, how much each sample counts.
    zones: [[f32; N]; 6],
    /// Characters allowed along outlines, with their ink patterns.
    glyphs: Vec<(char, [f32; 6])>,
    /// Characters for filled areas, with how dense each one is.
    ramp: Vec<(char, f32)>,
}

impl Frame {
    pub fn new() -> Frame {
        Frame {
            cols: 0,
            rows: 0,
            w: 0,
            h: 0,
            z: Vec::new(),
            color: Vec::new(),
            lum: Vec::new(),
            alpha: Vec::new(),
            part: Vec::new(),
            flower_parts: u16::MAX,
            per_flower: 1,
            style: Style::Ascii,
            back: [0.0; 3],
            zones: zone_weights(),
            glyphs: GLYPHS
                .iter()
                .filter(|(c, _)| EDGE_GLYPHS.contains(*c))
                .cloned()
                .collect(),
            ramp: GLYPHS
                .iter()
                .filter(|(c, _)| RAMP.contains(*c))
                .map(|(c, ink)| (*c, ink.iter().sum::<f32>() / 6.0))
                .collect(),
        }
    }

    /// Sizes the picture for a `cols` x `rows` terminal and wipes it.
    pub fn begin(&mut self, cols: usize, rows: usize) {
        (self.cols, self.rows) = (cols, rows);
        (self.w, self.h) = (cols * SX, rows * SY);
        let n = self.w * self.h;
        self.z.clear();
        self.z.resize(n, f32::NEG_INFINITY);
        self.color.clear();
        self.color.resize(n, [0.0; 3]);
        self.lum.clear();
        self.lum.resize(n, 0.0);
        self.alpha.clear();
        self.alpha.resize(n, 0.0);
        self.part.clear();
        self.part.resize(n, 0);
    }

    /// Fills every triangle: the solid ones first, which hide what is
    /// behind them, then the glass ones (given farthest first), which tint
    /// it. The picture is cut into horizontal bands, one per thread.
    pub fn fill(&mut self, solid: &[Shape], glass: &[Shape]) {
        if self.w == 0 || self.h == 0 {
            return;
        }
        // Thin strips, handed out as threads come free: the bouquet sits
        // in the middle rows, so equal shares would leave most threads idle.
        let rows = 2 * SY;
        let (w, chunk) = (self.w, rows * self.w);
        let z = self.z.chunks_mut(chunk);
        let color = self.color.chunks_mut(chunk);
        let lum = self.lum.chunks_mut(chunk);
        let alpha = self.alpha.chunks_mut(chunk);
        let part = self.part.chunks_mut(chunk);
        let bands = z.zip(color).zip(lum).zip(alpha).zip(part).enumerate().map(
            |(k, ((((z, color), lum), alpha), part))| Band {
                w,
                top: k * rows,
                rows: z.len() / w,
                z,
                color,
                lum,
                alpha,
                part,
            },
        );
        share(bands, |mut band| {
            for shape in solid {
                band.triangle(shape, false);
            }
            for shape in glass {
                band.triangle(shape, true);
            }
        });
    }

    /// Darkens whatever sits just behind the edge of something nearer:
    /// a soft line where one petal overlaps the next, and a wider, deeper
    /// shadow around each whole flower so it stands clear of its neighbours.
    pub fn shadow_edges(&mut self) {
        const NEAR: [(isize, isize); 4] = [(2, 0), (-2, 0), (0, 2), (0, -2)];
        const FAR: [(isize, isize); 8] = [
            (5, 0),
            (-5, 0),
            (0, 5),
            (0, -5),
            (3, 3),
            (-3, 3),
            (3, -3),
            (-3, -3),
        ];
        let (w, h) = (self.w as isize, self.h as isize);
        // Characters get sparser in shadow; a picture has only colour to
        // darken, so it darkens more.
        let depth = if self.style.is_picture() { 0.62 } else { 0.3 };
        let chunk = 2 * SY * self.w;
        let (zs, parts, alphas) = (&self.z, &self.part, &self.alpha);
        let (flower_parts, per_flower) = (self.flower_parts, self.per_flower);
        let thing = |part: u16| {
            if part < flower_parts {
                part
            } else {
                flower_parts + (part - flower_parts) / per_flower
            }
        };
        let strips = self
            .lum
            .chunks_mut(chunk)
            .zip(self.color.chunks_mut(chunk))
            .enumerate();
        share(strips, |(k, (lum, color))| {
            let top = (k * 2 * SY) as isize;
            for (n, (lum, color)) in lum.iter_mut().zip(color.iter_mut()).enumerate() {
                let (x, y) = (n as isize % w, top + n as isize / w);
                let i = (y * w + x) as usize;
                if alphas[i] == 0.0 {
                    continue;
                }
                let (part, z) = (parts[i], zs[i] + EDGE_GAP);
                let nearer = |dx: isize, dy: isize, whole: bool| {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w || ny >= h {
                        return false;
                    }
                    let j = (ny * w + nx) as usize;
                    let other = parts[j];
                    zs[j] > z
                        && if whole {
                            thing(other) != thing(part)
                        } else {
                            other != part
                        }
                };
                let petal = NEAR
                    .iter()
                    .filter(|(dx, dy)| nearer(*dx, *dy, false))
                    .count();
                let whole = FAR.iter().filter(|(dx, dy)| nearer(*dx, *dy, true)).count();
                let k =
                    (1.0 - 0.16 * petal as f32).max(0.5) * (1.0 - 0.14 * whole as f32).max(0.22);
                if k < 1.0 {
                    *lum *= k;
                    *color = dim(*color, 1.0 - depth + depth * k);
                }
            }
        });
    }

    /// Writes the raw picture as a PPM image, for checking the models
    /// without the characters in the way.
    pub fn dump(&self, path: &str) {
        let mut out = format!("P6 {} {} 255\n", self.w, self.h).into_bytes();
        for i in 0..self.w * self.h {
            let c = Rgb::from_col(dim(self.color[i], self.alpha[i]));
            out.extend([c.0, c.1, c.2]);
        }
        let _ = std::fs::write(path, out);
    }

    /// Turns the picture into characters on `canvas`.
    pub fn resolve(&self, canvas: &mut Canvas) {
        if let Ok(path) = std::env::var("POSY_DUMP") {
            self.dump(&path);
        }
        for cy in 0..self.rows {
            for cx in 0..self.cols {
                match self.style {
                    Style::Ascii => self.resolve_cell(canvas, cx, cy),
                    Style::Blocks => self.block_cell(canvas, cx, cy, 2),
                    Style::Fine => self.block_cell(canvas, cx, cy, 4),
                }
            }
        }
    }

    /// Paints one cell as a tiny picture: the cell is cut into squares
    /// (2 across, `down` down), sorted into two colours, and drawn with the
    /// block character that has that pattern. Where the bouquet stops, the
    /// rest of the cell is left as the terminal's own background.
    fn block_cell(&self, canvas: &mut Canvas, cx: usize, cy: usize, down: usize) {
        let tall = SY / down;
        let n = 2 * down;
        let mut sub = [[0.0f32; 3]; 8];
        let mut covered = [0u32; 8];
        let mut nearest = f32::NEG_INFINITY;
        for sy in 0..SY {
            let row = (cy * SY + sy) * self.w + cx * SX;
            for sx in 0..SX {
                let i = row + sx;
                let a = self.alpha[i];
                if a == 0.0 {
                    continue;
                }
                nearest = nearest.max(self.z[i]);
                // Glass shows what is behind it, which we can only guess.
                let c = mix(self.back, self.color[i], a);
                let k = (sy / tall) * 2 + sx / (SX / 2);
                covered[k] += 1;
                for j in 0..3 {
                    sub[k][j] += c[j];
                }
            }
        }
        let size = (tall * SX / 2) as u32;
        let total: u32 = covered.iter().sum();
        if total == 0 {
            return;
        }
        // Two rows of squares are drawn with the four-row characters.
        let glyph = |mask: u32| {
            let pattern = if down == 4 {
                mask as usize
            } else {
                (0..4).fold(0usize, |m, q| {
                    if mask >> q & 1 == 1 {
                        let (col, row) = (q % 2, q / 2);
                        m | 1 << (row * 4 + col) | 1 << (row * 4 + 2 + col)
                    } else {
                        m
                    }
                })
            };
            OCTANTS[pattern]
        };
        if total < size * n as u32 {
            // On the outline: one colour, in the squares the bouquet fills.
            let (mut mask, mut sum, mut count) = (0u32, [0.0f32; 3], 0);
            for k in 0..n {
                if covered[k] * 2 >= size {
                    mask |= 1 << k;
                    count += covered[k];
                    for j in 0..3 {
                        sum[j] += sub[k][j];
                    }
                }
            }
            if mask != 0 {
                let fg = coarse(dim(sum, 1.0 / count as f32));
                canvas.plot(cx as i32, cy as i32, nearest, glyph(mask), fg);
            }
            return;
        }
        let per = 1.0 / (tall * SX / 2) as f32;
        let sub = &mut sub[..n];
        for c in sub.iter_mut() {
            *c = dim(*c, per);
        }

        // Start by splitting along whichever of red, green, blue varies most.
        let mut split = (0, 0.0, 0.0);
        for j in 0..3 {
            let lo = sub.iter().map(|c| c[j]).fold(f32::INFINITY, f32::min);
            let hi = sub.iter().map(|c| c[j]).fold(f32::NEG_INFINITY, f32::max);
            if hi - lo > split.2 - split.1 {
                split = (j, lo, hi);
            }
        }
        let (j, lo, hi) = split;
        let mean = |mask: u32, on: bool| -> Col {
            let (mut sum, mut count) = ([0.0f32; 3], 0.0);
            for (k, c) in sub.iter().enumerate() {
                if (mask >> k & 1 == 1) == on {
                    for j in 0..3 {
                        sum[j] += c[j];
                    }
                    count += 1.0;
                }
            }
            dim(sum, 1.0 / f32::max(count, 1.0))
        };
        let all = (1u32 << n) - 1;
        if hi - lo < 0.015 {
            canvas.paint(cx, cy, nearest, ' ', Rgb::WHITE, coarse(mean(all, true)));
            return;
        }
        let mut mask = 0u32;
        for (k, c) in sub.iter().enumerate() {
            if c[j] > (lo + hi) / 2.0 {
                mask |= 1 << k;
            }
        }
        // Then let each square join whichever colour it is closer to.
        for _ in 0..2 {
            let (fg, bg) = (mean(mask, true), mean(mask, false));
            let dist = |a: Col, b: Col| (0..3).map(|j| (a[j] - b[j]).powi(2)).sum::<f32>();
            let mut next = 0u32;
            for (k, c) in sub.iter().enumerate() {
                if dist(*c, fg) < dist(*c, bg) {
                    next |= 1 << k;
                }
            }
            if next == 0 || next == all {
                break;
            }
            mask = next;
        }
        let (fg, bg) = (mean(mask, true), mean(mask, false));
        let (x, y) = (cx, cy);
        canvas.paint(x, y, nearest, glyph(mask), coarse(fg), coarse(bg));
    }

    fn resolve_cell(&self, canvas: &mut Canvas, cx: usize, cy: usize) {
        let mut shape = [0.0f32; 6];
        let mut covered = 0.0;
        let mut gaps = 0;
        let mut nearest = f32::NEG_INFINITY;
        // The few parts that show in this cell: (part, samples, color sum).
        let mut seen: [(u16, f32, Col); 4] = [(0, 0.0, [0.0; 3]); 4];

        for sy in 0..SY {
            let row = (cy * SY + sy) * self.w + cx * SX;
            for sx in 0..SX {
                let i = row + sx;
                let a = self.alpha[i];
                if a == 0.0 {
                    gaps += 1;
                    continue;
                }
                let s = sy * SX + sx;
                let ink = self.lum[i] * a;
                for (zone, weights) in shape.iter_mut().zip(&self.zones) {
                    *zone += weights[s] * ink;
                }
                covered += a;
                nearest = nearest.max(self.z[i]);
                let part = self.part[i];
                let slot = seen
                    .iter()
                    .position(|e| e.0 == part || e.1 == 0.0)
                    .unwrap_or(3);
                let e = &mut seen[slot];
                e.0 = part;
                e.1 += a;
                for k in 0..3 {
                    e.2[k] += self.color[i][k] * a;
                }
            }
        }
        if covered < 0.5 {
            return;
        }

        // Glass counts as covering a cell even though it is faint.
        let full = gaps == 0;
        let ch = if full {
            // Solidly covered: pick by brightness from a clean ramp.
            let want = shape.iter().sum::<f32>() / 6.0 * DENSEST;
            self.ramp
                .iter()
                .min_by(|a, b| (a.1 - want).abs().total_cmp(&(b.1 - want).abs()))
                .map_or(' ', |g| g.0)
        } else {
            // Exaggerate the shape within the cell so edges come out clean.
            let peak = shape.iter().cloned().fold(0.0, f32::max);
            if peak > 1e-4 {
                for v in &mut shape {
                    *v = (*v / peak).powf(CONTRAST) * peak;
                }
            }
            best_glyph(&self.glyphs, &shape)
        };
        if ch == ' ' {
            return;
        }
        // Color: whichever part fills most of the cell. Averaging two
        // flowers together would only make mud.
        let main = seen
            .iter()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("four slots");
        let color = dim(main.2, 1.0 / main.1);
        canvas.plot(cx as i32, cy as i32, nearest, ch, Rgb::from_col(color));
    }
}

/// A colour rounded to the nearest of 32 levels per channel. The eye can't
/// tell, but far fewer cells change from one frame to the next, so there
/// is much less to send to the terminal.
fn coarse(c: Col) -> Rgb {
    let f = |x: f32| ((x.clamp(0.0, 1.0) * 31.0).round() * 255.0 / 31.0).round() as u8;
    Rgb(f(c[0]), f(c[1]), f(c[2]))
}

/// Runs `work` on every item, spread over the machine's cores.
fn share<T: Send>(items: impl Iterator<Item = T> + Send, work: impl Fn(T) + Sync) {
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
    let queue = std::sync::Mutex::new(items);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let next = queue.lock().expect("no thread panics holding it").next();
                    match next {
                        Some(item) => work(item),
                        None => break,
                    }
                }
            });
        }
    });
}

/// The character whose ink pattern is closest to `shape`.
fn best_glyph(glyphs: &[(char, [f32; 6])], shape: &[f32; 6]) -> char {
    let mut best = (' ', shape.iter().map(|v| v * v).sum::<f32>());
    for (ch, ink) in glyphs {
        let d: f32 = ink.iter().zip(shape).map(|(a, b)| (a - b) * (a - b)).sum();
        if d < best.1 {
            best = (*ch, d);
        }
    }
    best.0
}

/// The six zones are overlapping circles, two across and three down, the
/// same ones the glyph table was measured with.
fn zone_weights() -> [[f32; N]; 6] {
    const CENTERS: [(f32, f32); 6] = [
        (0.27, 0.19),
        (0.73, 0.19),
        (0.27, 0.50),
        (0.73, 0.50),
        (0.27, 0.81),
        (0.73, 0.81),
    ];
    const RADIUS: f32 = 0.30;
    const FINE: usize = 4;
    let mut zones = [[0.0; N]; 6];
    for (zone, (zx, zy)) in zones.iter_mut().zip(CENTERS) {
        let mut total = 0.0;
        for (s, weight) in zone.iter_mut().enumerate() {
            let (sx, sy) = (s % SX, s / SX);
            let mut inside = 0;
            for fy in 0..FINE {
                for fx in 0..FINE {
                    let x = (sx as f32 + (fx as f32 + 0.5) / FINE as f32) / SX as f32;
                    let y = (sy as f32 + (fy as f32 + 0.5) / FINE as f32) / SY as f32;
                    // The cell is twice as tall as wide.
                    let (dx, dy) = (x - zx, (y - zy) * 2.0);
                    if dx * dx + dy * dy < RADIUS * RADIUS {
                        inside += 1;
                    }
                }
            }
            *weight = inside as f32;
            total += inside as f32;
        }
        for v in zone.iter_mut() {
            *v /= total;
        }
    }
    zones
}
