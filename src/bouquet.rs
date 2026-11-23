//! What a bouquet *is*: its flowers, container, color and effects. This is
//! the data you write in a `.toml` file. `scene` turns it into 3D geometry.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::color::{Rgb, color_or};
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlowerKind {
    Rose,
    Tulip,
    Daisy,
    Sunflower,
    Lily,
    Cosmos,
    Carnation,
    Peony,
    Lavender,
    BabysBreath,
}

impl FlowerKind {
    pub const ALL: [FlowerKind; 10] = [
        FlowerKind::Rose,
        FlowerKind::Tulip,
        FlowerKind::Daisy,
        FlowerKind::Sunflower,
        FlowerKind::Lily,
        FlowerKind::Cosmos,
        FlowerKind::Carnation,
        FlowerKind::Peony,
        FlowerKind::Lavender,
        FlowerKind::BabysBreath,
    ];

    /// Flowers that get a full head (not lavender spikes or baby's breath sprays).
    pub const HEADS: [FlowerKind; 8] = [
        FlowerKind::Rose,
        FlowerKind::Tulip,
        FlowerKind::Daisy,
        FlowerKind::Sunflower,
        FlowerKind::Lily,
        FlowerKind::Cosmos,
        FlowerKind::Carnation,
        FlowerKind::Peony,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FlowerKind::Rose => "rose",
            FlowerKind::Tulip => "tulip",
            FlowerKind::Daisy => "daisy",
            FlowerKind::Sunflower => "sunflower",
            FlowerKind::Lily => "lily",
            FlowerKind::Cosmos => "cosmos",
            FlowerKind::Carnation => "carnation",
            FlowerKind::Peony => "peony",
            FlowerKind::Lavender => "lavender",
            FlowerKind::BabysBreath => "baby's breath",
        }
    }

    pub fn default_color(self) -> &'static str {
        match self {
            FlowerKind::Rose => "red",
            FlowerKind::Tulip => "pink",
            FlowerKind::Daisy => "white",
            FlowerKind::Sunflower => "yellow",
            FlowerKind::Lily => "white",
            FlowerKind::Cosmos => "pink",
            FlowerKind::Carnation => "coral",
            FlowerKind::Peony => "blush",
            FlowerKind::Lavender => "lavender",
            FlowerKind::BabysBreath => "white",
        }
    }

    /// Color of the middle of the folower (seed head, stamens...),
    pub fn default_center(self) -> &'static str {
        match self {
            FlowerKind::Sunflower => "chocolate",
            FlowerKind::Lily => "orange",
            FlowerKind::Rose => "crimson",
            _ => "gold",
        }
    }

    /// Head radius in world units. Spikes and sprays have no head.
    pub fn radius(self) -> f32 {
        match self {
            FlowerKind::Rose => 0.19,
            FlowerKind::Tulip => 0.15,
            FlowerKind::Daisy => 0.18,
            FlowerKind::Sunflower => 0.24,
            FlowerKind::Lily => 0.23,
            FlowerKind::Cosmos => 0.19,
            FlowerKind::Carnation => 0.17,
            FlowerKind::Peony => 0.22,
            FlowerKind::Lavender | FlowerKind::BabysBreath => 0.0,
        }
    }

    pub fn petal_count(self, rng: &mut Rng) -> u32 {
        match self {
            FlowerKind::Daisy => 15 + 2 * rng.below(4) as u32,
            FlowerKind::Sunflower => 22 + rng.below(5) as u32,
            FlowerKind::Cosmos => 8,
            FlowerKind::Lily => 6,
            FlowerKind::Rose => 5,
            FlowerKind::Carnation => 12,
            FlowerKind::Peony => 7,
            FlowerKind::Tulip => 3,
            FlowerKind::Lavender | FlowerKind::BabysBreath => 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum WrapStyle {
    /// Paper cone with a ribbon (like the original)
    #[default]
    Cone,
    /// Curvy glass vase with water
    Vase,
    /// Mason jar with twin
    Jar,
    /// Woven basket with a handle
    Basket,
    /// Just the stems, tied with a ribbon
    Bare,
}

impl WrapStyle {
    pub const ALL: [WrapStyle; 5] = [
        WrapStyle::Cone,
        WrapStyle::Vase,
        WrapStyle::Jar,
        WrapStyle::Basket,
        WrapStyle::Bare,
    ];

    pub fn label(self) -> &'static str {
        match self {
            WrapStyle::Cone => "cone",
            WrapStyle::Vase => "vase",
            WrapStyle::Jar => "jar",
            WrapStyle::Basket => "basket",
            WrapStyle::Bare => "bare",
        }
    }

    pub fn default_color(self) -> &'static str {
        match self {
            WrapStyle::Cone => "cream",
            WrapStyle::Vase | WrapStyle::Jar => "glass",
            WrapStyle::Basket => "kraft",
            WrapStyle::Bare => "green",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum Effect {
    /// Hearts floating up behing the bouquet
    Hearts,
    /// Petals drifting down
    Petals,
    /// Fireflies circling the bouquet
    Fireflies,
    /// Gentle snow
    Snow,
    /// A twinkling starfield
    Stars,
    /// Sparkles aroung the flowers
    Sparkles,
}

impl Effect {
    pub const COUNT: usize = 6;
    pub const ALL: [Effect; Effect::COUNT] = [
        Effect::Hearts,
        Effect::Petals,
        Effect::Fireflies,
        Effect::Snow,
        Effect::Stars,
        Effect::Sparkles,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Effect::Hearts => "hearts",
            Effect::Petals => "petals",
            Effect::Fireflies => "fireflies",
            Effect::Snow => "snow",
            Effect::Stars => "stars",
            Effect::Sparkles => "sparkles",
        }
    }
}

/// How the message under the bouquet moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextEffect {
    /// A soft wave of light running through it
    #[default]
    Shimmer,
    /// Slowly breathing brighter and dimmer
    Glow,
    Rainbow,
    /// Perfectly still
    Plain,
}

impl TextEffect {
    pub const ALL: [TextEffect; 4] = [
        TextEffect::Shimmer,
        TextEffect::Glow,
        TextEffect::Rainbow,
        TextEffect::Plain,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TextEffect::Shimmer => "shimmer",
            TextEffect::Glow => "glow",
            TextEffect::Rainbow => "rainbow",
            TextEffect::Plain => "still",
        }
    }
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

/// Steps through `all` from `current`, wrapping around at either end.
pub fn cycle<T: Copy + PartialEq>(all: &[T], current: T, step: i32) -> T {
    let i = all.iter().position(|&x| x == current).unwrap_or(0) as i32;
    all[(i + step).rem_euclid(all.len() as i32) as usize]
}

/// One line of a boquet: "5 red roses".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlowerEntry {
    pub kind: FlowerKind,
    #[serde(default = "one")]
    pub count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub center: Option<String>,
}

fn one() -> u32 {
    1
}

/// A whole bouquet, exactly as it appears in a `.toml` file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BouquetSpec {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Draw the message in big letters.
    #[serde(skip_serializing_if = "is_false")]
    pub message_big: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_color: Option<String>,
    #[serde(skip_serializing_if = "is_default")]
    pub message_effect: TextEffect,
    /// A smaller line under the message, like "— from me".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Turn the message into a question with two answers to click.
    #[serde(skip_serializing_if = "is_false")]
    pub ask: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yes: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no: Option<String>,
    /// What the message becomes after each answer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub yes_reply: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub no_reply: Option<String>,
    pub wrap: WrapStyle,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wrap_color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ribbon_color: Option<String>,
    pub effects: Vec<Effect>,
    /// Fixes the arrangement. Without it, the name is used as the seed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    // Keep this last: TOML wants plain keys before arrays of tables.
    pub flowers: Vec<FlowerEntry>,
}

impl Default for BouquetSpec {
    fn default() -> Self {
        BouquetSpec {
            name: "untitled".into(),
            message: None,
            message_big: false,
            message_color: None,
            message_effect: TextEffect::Shimmer,
            signature: None,
            ask: false,
            yes: None,
            no: None,
            yes_reply: None,
            no_reply: None,
            wrap: WrapStyle::Cone,
            wrap_color: None,
            ribbon_color: None,
            effects: vec![Effect::Hearts],
            seed: None,
            flowers: vec![FlowerEntry {
                kind: FlowerKind::Rose,
                count: 7,
                color: None,
                center: None,
            }],
        }
    }
}

impl BouquetSpec {
    /// Checks that every color name or hex code actually parses.
    pub fn validate(&self) -> Result<()> {
        let check = |what: &str, color: &Option<String>| -> Result<()> {
            if let Some(c) = color
                && Rgb::parse(c).is_none()
            {
                bail!(
                    "{what}: unknown color \"{c}\" (use a name like \"pink\" or a hex code like \"#ff88aa\")"
                );
            }
            Ok(())
        };
        check("wrap_color", &self.wrap_color)?;
        check("message_color", &self.message_color)?;
        check("ribbon_color", &self.ribbon_color)?;
        for f in &self.flowers {
            check(f.kind.label(), &f.color)?;
            check(f.kind.label(), &f.center)?;
        }
        Ok(())
    }

    /// A one-line description, like "5 rose, 3 baby's breath · cone"
    pub fn summary(&self) -> String {
        let flowers: Vec<String> = self
            .flowers
            .iter()
            .map(|f| format!("{} {}", f.count, f.kind.label()))
            .collect();
        format!("{} · {}", flowers.join(", "), self.wrap.label())
    }

    /// Every petal color in the bouquet (used by the falling-petals effect).
    pub fn petal_colors(&self) -> Vec<Rgb> {
        self.flowers
            .iter()
            .filter(|f| f.kind != FlowerKind::BabysBreath)
            .map(|f| color_or(f.color.as_deref(), f.kind.default_color()))
            .collect()
    }
}

/// Bouquets built into the binary. They are ordinary bouquet files, so they
/// double as examples for writing your own.
pub const PRESETS: &[(&str, &str)] = &[
    ("classic", include_str!("../presets/classic.toml")),
    ("roses", include_str!("../presets/roses.toml")),
    ("wildflower", include_str!("../presets/wildflower.toml")),
    ("sunset", include_str!("../presets/sunset.toml")),
    ("spring", include_str!("../presets/spring.toml")),
    ("blush", include_str!("../presets/blush.toml")),
    ("midnight", include_str!("../presets/midnight.toml")),
];

pub fn preset(name: &str) -> Option<BouquetSpec> {
    let (_, text) = PRESETS.iter().find(|(n, _)| *n == name)?;
    toml::from_str(text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_parse_and_validate() {
        for (name, text) in PRESETS {
            let spec: BouquetSpec = toml::from_str(text).unwrap_or_else(|e| panic!("{name}: {e}"));
            spec.validate().unwrap();
            assert_eq!(&spec.name, name);
        }
    }

    #[test]
    fn spec_round_trips_through_toml() {
        let spec = preset("classic").unwrap();
        let text = toml::to_string_pretty(&spec).unwrap();
        let back: BouquetSpec = toml::from_str(&text).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn cycle_wraps() {
        assert_eq!(cycle(&WrapStyle::ALL, WrapStyle::Bare, 1), WrapStyle::Cone);
        assert_eq!(cycle(&WrapStyle::ALL, WrapStyle::Cone, -1), WrapStyle::Bare);
    }
}
