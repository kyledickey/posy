//! Colors: RGB values, named colors, shading gradients, and the escape codes
//! that put them on a terminal.

use std::fmt::Write;

use serde::{Deserialize, Serialize};

/// A 24-bit color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const WHITE: Rgb = Rgb(255, 255, 255);

    /// Blends toward `other`: `t=0` gives `self`, `t=1` gives `other`.
    pub fn lerp(self, other: Rgb, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        let mix = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Rgb(
            mix(self.0, other.0),
            mix(self.1, other.1),
            mix(self.2, other.2),
        )
    }

    /// Parses `#rrggbb` or a name from [`NAMED`] (case-insensitive).
    pub fn parse(s: &str) -> Option<Rgb> {
        let s = s.trim();
        if let Some(hex) = s.strip_prefix('#') {
            if hex.len() != 6 {
                return None;
            }
            let v = u32::from_str_radix(hex, 16).ok()?;
            return Some(Rgb((v >> 16) as u8, (v >> 8) as u8, v as u8));
        }
        NAMED
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(s))
            .map(|&(_, c)| c)
    }

    /// As floats in 0..1, for mixing and shading.
    pub fn to_col(self) -> Col {
        [
            self.0 as f32 / 255.0,
            self.1 as f32 / 255.0,
            self.2 as f32 / 255.0,
        ]
    }

    pub fn from_col(c: Col) -> Rgb {
        let f = |x: f32| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        Rgb(f(c[0]), f(c[1]), f(c[2]))
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }

    /// The closest color is the xterm 256-color palette, for terminals
    /// without truecolor.
    pub fn to_ansi256(self) -> u8 {
        const LEVELS: [u8; 6] = [0, 95, 135, 175, 215, 255];
        let level = |v: u8| -> usize {
            if v < 48 {
                0
            } else if v < 115 {
                1
            } else {
                ((v - 35) / 40) as usize
            }
        };
        let (ri, gi, bi) = (level(self.0), level(self.1), level(self.2));
        let cube = Rgb(LEVELS[ri], LEVELS[gi], LEVELS[bi]);

        let avg = (self.0 as u32 + self.1 as u32 + self.2 as u32) / 3;
        let gray_index = if avg > 238 {
            23
        } else {
            avg.saturating_sub(3) / 10
        };
        let g = (8 + 10 * gray_index) as u8;

        if self.dist2(Rgb(g, g, g)) < self.dist2(cube) {
            232 + gray_index as u8
        } else {
            (16 + 36 * ri + 6 * gi + bi) as u8
        }
    }

    fn dist2(self, o: Rgb) -> i32 {
        let d = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
        d(self.0, o.0) + d(self.1, o.1) + d(self.2, o.2)
    }
}

/// A color as floats (red, green, blue in 0..1). Meshes and the renderer
/// work in these and convert to [`Rgb`] at the very end.
pub type Col = [f32; 3];

/// Blends `a` toward `b`.
pub fn mix(a: Col, b: Col, t: f32) -> Col {
    let t = t.clamp(0.0, 1.0);
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

/// Multiplies brightness by `k`.
pub fn dim(c: Col, k: f32) -> Col {
    [c[0] * k, c[1] * k, c[2] * k]
}

/// A lit color: `b` runs from deep shadow (0) through the color itself
/// (0.55) to a highlight (1). Shadows lean toward plum and highlights
/// toward warm white, which is what keeps flowers from looking flat.
pub fn lit(base: Col, b: f32) -> Col {
    const PLUM: Col = [0.14, 0.05, 0.17];
    const WARM: Col = [1.0, 0.98, 0.955];
    let b = b.clamp(0.0, 1.0);
    if b < 0.55 {
        mix(mix(dim(base, 0.40), PLUM, 0.22), base, b / 0.55)
    } else {
        mix(base, mix(base, WARM, 0.5), (b - 0.55) / 0.45)
    }
}

/// Hue/saturation/value to RGB. All inputs are 0..1.
pub fn hsv(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(1.0) * 6.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let f = |u: f32| ((u + m) * 255.0).round() as u8;
    Rgb(f(r), f(g), f(b))
}

/// Parses `s` if given, else `fallback`, else white.
pub fn color_or(s: Option<&str>, fallback: &str) -> Rgb {
    s.and_then(Rgb::parse)
        .or_else(|| Rgb::parse(fallback))
        .unwrap_or(Rgb::WHITE)
}

/// Every color you can use by name in a bouquet file or the builder.
pub const NAMED: &[(&str, Rgb)] = &[
    ("red", Rgb(214, 32, 52)),
    ("crimson", Rgb(170, 18, 48)),
    ("burgundy", Rgb(118, 14, 40)),
    ("rose", Rgb(232, 64, 104)),
    ("pink", Rgb(246, 122, 172)),
    ("blush", Rgb(250, 182, 202)),
    ("magenta", Rgb(222, 52, 156)),
    ("coral", Rgb(255, 117, 92)),
    ("peach", Rgb(255, 184, 146)),
    ("apricot", Rgb(255, 166, 86)),
    ("orange", Rgb(250, 132, 32)),
    ("gold", Rgb(236, 176, 34)),
    ("yellow", Rgb(255, 216, 46)),
    ("butter", Rgb(255, 238, 150)),
    ("cream", Rgb(252, 242, 218)),
    ("white", Rgb(246, 246, 250)),
    ("lavender", Rgb(184, 160, 238)),
    ("lilac", Rgb(208, 146, 222)),
    ("purple", Rgb(146, 72, 204)),
    ("violet", Rgb(112, 64, 222)),
    ("plum", Rgb(122, 34, 92)),
    ("periwinkle", Rgb(142, 152, 250)),
    ("blue", Rgb(72, 112, 232)),
    ("sky", Rgb(124, 192, 250)),
    ("teal", Rgb(42, 164, 164)),
    ("mint", Rgb(152, 230, 192)),
    ("sage", Rgb(146, 172, 124)),
    ("green", Rgb(72, 172, 72)),
    ("forest", Rgb(32, 112, 52)),
    ("kraft", Rgb(196, 154, 104)),
    ("brown", Rgb(122, 78, 42)),
    ("chocolate", Rgb(78, 44, 24)),
    ("silver", Rgb(192, 198, 210)),
    ("glass", Rgb(170, 214, 232)),
    ("black", Rgb(46, 44, 52)),
];

/// A dark → base → light ramp. Shading picks a point along it, so one base
/// color gives a whole lit-and-shadowed palette.
#[derive(Clone, Copy, Debug)]
pub struct Gradient {
    shadow: Rgb,
    base: Rgb,
    light: Rgb,
}

impl Gradient {
    pub const fn new(shadow: Rgb, base: Rgb, light: Rgb) -> Self {
        Gradient {
            shadow,
            base,
            light,
        }
    }

    /// Color at brightness `t` (0 = deepest shadow, 1 = brightest highlights).
    pub fn at(&self, t: f32) -> Rgb {
        let t = t.clamp(0.0, 1.0);
        if t < 0.55 {
            self.shadow.lerp(self.base, t / 0.55)
        } else {
            self.base.lerp(self.light, (t - 0.55) / 0.45)
        }
    }
}

/// What the user asked for (config file or `--color`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ColorChoice {
    #[default]
    Auto,
    #[value(name = "truecolor", alias = "true-color")]
    TrueColor,
    #[serde(rename = "256")]
    #[value(name = "256")]
    Ansi256,
}

impl ColorChoice {
    pub fn resolve(self) -> ColorMode {
        match self {
            ColorChoice::TrueColor => ColorMode::TrueColor,
            ColorChoice::Ansi256 => ColorMode::Ansi256,
            ColorChoice::Auto => ColorMode::detect(),
        }
    }
}

/// How colors are actually written to the terminal
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorMode {
    TrueColor,
    Ansi256,
}

impl ColorMode {
    /// Whether the terminal can show any colour, or only a fixed 256.
    /// `$COLORTERM` is the usual sign, but it is often lost over ssh, so
    /// terminals known to have full colour are recognised by name too.
    pub fn detect() -> Self {
        let var = |name: &str| std::env::var(name).unwrap_or_default().to_lowercase();
        let colorterm = var("COLORTERM");
        let (term, program) = (var("TERM"), var("TERM_PROGRAM"));
        const FULL: [&str; 11] = [
            "ghostty",
            "kitty",
            "alacritty",
            "wezterm",
            "foot",
            "iterm",
            "vscode",
            "contour",
            "rio",
            "warp",
            "direct",
        ];
        let known = FULL
            .iter()
            .any(|name| term.contains(name) || program.contains(name));
        if colorterm.contains("truecolor") || colorterm.contains("24bit") || known {
            ColorMode::TrueColor
        } else {
            ColorMode::Ansi256
        }
    }

    pub fn write_fg(self, out: &mut String, c: Rgb) {
        let _ = match self {
            ColorMode::TrueColor => write!(out, "\x1b[38;2;{};{};{}m", c.0, c.1, c.2),
            ColorMode::Ansi256 => write!(out, "\x1b[38;5;{}m", c.to_ansi256()),
        };
    }

    pub fn write_bg(self, out: &mut String, c: Rgb) {
        let _ = match self {
            ColorMode::TrueColor => write!(out, "\x1b[48;2;{};{};{}m", c.0, c.1, c.2),
            ColorMode::Ansi256 => write!(out, "\x1b[48;5;{}m", c.to_ansi256()),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_name_and_hex() {
        assert_eq!(Rgb::parse("#ff8000"), Some(Rgb(255, 128, 0)));
        assert_eq!(Rgb::parse("PINK"), Rgb::parse("pink"));
        assert_eq!(Rgb::parse("not-a-color"), None);
        assert_eq!(Rgb::parse("#fff"), None);
    }

    #[test]
    fn ansi256_extremes() {
        assert_eq!(Rgb(0, 0, 0).to_ansi256(), 16);
        assert_eq!(Rgb(255, 255, 255).to_ansi256(), 231);
        assert_eq!(Rgb(255, 0, 0).to_ansi256(), 196);
    }
}
