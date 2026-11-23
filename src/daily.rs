//! The daily bouquet: a fresh arrangement generated from a seed. With the
//! date as the seed, it's different every day but the same all day.

use crate::bouquet::{BouquetSpec, Effect, FlowerEntry, FlowerKind, WrapStyle};
use crate::rng::Rng;

/// A set of colors that look good together.
struct Theme {
    petals: &'static [&'static str],
    ribbons: &'static [&'static str],
    papers: &'static [&'static str],
}

const THEMES: &[Theme] = &[
    Theme {
        petals: &["blush", "pink", "cream", "white", "peach"],
        ribbons: &["pink", "blush"],
        papers: &["white", "cream", "kraft"],
    },
    Theme {
        petals: &["orange", "coral", "yellow", "apricot", "red"],
        ribbons: &["kraft", "gold"],
        papers: &["cream", "kraft"],
    },
    Theme {
        petals: &["crimson", "plum", "magenta", "pink", "burgundy"],
        ribbons: &["blush", "silver"],
        papers: &["cream", "black", "blush"],
    },
    Theme {
        petals: &["white", "yellow", "lavender", "sky", "butter"],
        ribbons: &["sky", "sage"],
        papers: &["kraft", "white"],
    },
    Theme {
        petals: &["purple", "violet", "periwinkle", "lilac", "white"],
        ribbons: &["silver", "lilac"],
        papers: &["plum", "white", "lilac"],
    },
    Theme {
        petals: &["red", "crimson", "pink", "white", "rose"],
        ribbons: &["red", "pink"],
        papers: &["blush", "white", "cream"],
    },
    Theme {
        petals: &["yellow", "orange", "gold", "white", "butter"],
        ribbons: &["sage", "gold"],
        papers: &["kraft", "cream"],
    },
    Theme {
        petals: &["blush", "lilac", "mint", "peach", "butter"],
        ribbons: &["mint", "lilac"],
        papers: &["white", "blush"],
    },
];

/// Builds a random bouquet from `seed`. `month` (1–12, or 0 if unknown) lets
/// winter bouquets sometimes get snow.
pub fn generate(seed: u64, name: &str, month: u32) -> BouquetSpec {
    let mut rng = Rng::new(seed);
    let theme = rng.pick(THEMES);

    // 2–4 kinds of flower, 8–12 heads in total, at least one of each kind.
    let mut kinds = FlowerKind::HEADS.to_vec();
    rng.shuffle(&mut kinds);
    let n_kinds = 2 + rng.below(3);
    let total = 8 + rng.below(5);
    let mut counts = vec![1u32; n_kinds];
    for _ in n_kinds..total {
        counts[rng.below(n_kinds)] += 1;
    }

    let mut flowers = Vec::new();
    for (&kind, count) in kinds.iter().zip(counts) {
        let color = rng.pick(theme.petals).to_string();
        flowers.push(FlowerEntry {
            kind,
            count,
            color: Some(color),
            center: None,
        });
    }
    if rng.chance(0.55) {
        let count = 2 + rng.below(3) as u32;
        flowers.push(FlowerEntry {
            kind: FlowerKind::BabysBreath,
            count,
            color: None,
            center: None,
        });
    }
    if rng.chance(0.35) {
        let count = 2 + rng.below(2) as u32;
        let color = rng.pick(&["lavender", "purple", "lilac"]).to_string();
        flowers.push(FlowerEntry {
            kind: FlowerKind::Lavender,
            count,
            color: Some(color),
            center: None,
        });
    }

    let wrap = *rng.pick(&WrapStyle::ALL);
    let wrap_color = match wrap {
        WrapStyle::Cone => Some(rng.pick(theme.papers).to_string()),
        _ => None,
    };
    let ribbon_color = Some(rng.pick(theme.ribbons).to_string());

    let mut effects = vec![*rng.pick(&[Effect::Hearts, Effect::Petals, Effect::Sparkles])];
    if matches!(month, 12 | 1 | 2) && rng.chance(0.5) {
        effects.push(Effect::Snow);
    } else if rng.chance(0.4) {
        effects.push(*rng.pick(&[Effect::Fireflies, Effect::Stars, Effect::Sparkles]));
    }
    effects.dedup();

    BouquetSpec {
        name: name.to_string(),
        message: None,
        wrap,
        wrap_color,
        ribbon_color,
        effects,
        seed: Some(seed),
        flowers,
        ..BouquetSpec::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn many_seeds_make_valid_bouquets() {
        for seed in 0..500 {
            let spec = generate(seed, "daily", 1);
            spec.validate().unwrap();
            assert!(!spec.flowers.is_empty());
        }
    }

    #[test]
    fn same_seed_same_bouquet() {
        assert_eq!(
            generate(20261119, "daily", 11),
            generate(20261119, "daily", 11)
        );
    }
}
