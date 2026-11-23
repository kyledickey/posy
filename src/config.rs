//! The config file: who it's for, messages, special dates and secret
//! words. It is `config.toml` in posy's folder, which is `~/.config/posy`
//! on Linux, `~/Library/Application Support/posy` on a Mac and
//! `%APPDATA%\posy` on Windows.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

use crate::bouquet::Effect;
use crate::color::{ColorChoice, Rgb};
use crate::raster::Style;

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Who the bouquet is for. Fills `{name}` in messages.
    pub name: Option<String>,
    /// Who it's from. Fills `{from}` and shows as a signature.
    pub from: Option<String>,
    /// Messages to rotate through under the bouquet.
    pub messages: Vec<String>,
    /// Seconds each message stays up.
    pub message_interval: f32,
    /// Bouquet to open with. Leave it out for the daily bouquet.
    pub bouquet: Option<String>,
    pub color: ColorChoice,
    /// What to draw with. Leave it out to use the best your terminal can do.
    pub style: Option<Style>,
    /// Paints the whole terminal background, e.g. "#1a1b26".
    pub background: Option<String>,
    /// Frames per second. Left out, it is 60, or 30 over ssh.
    pub fps: Option<u32>,
    pub spin_speed: f32,
    /// Play the growing/blooming animation.
    pub grow: bool,
    /// Show the buttons along the bottom.
    pub buttons: bool,
    /// Look for a newer posy now and then, and mention it on the way out.
    pub update_check: bool,
    /// Where `x` saves snapshots. Defaults to the current directory.
    pub export_dir: Option<PathBuf>,
    pub dates: Vec<SpecialDate>,
    pub secrets: Vec<Secret>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            name: None,
            from: None,
            messages: Vec::new(),
            message_interval: 8.0,
            bouquet: None,
            color: ColorChoice::Auto,
            style: None,
            background: None,
            fps: None,
            spin_speed: 0.25,
            grow: true,
            buttons: true,
            update_check: true,
            export_dir: None,
            dates: Vec::new(),
            secrets: Vec::new(),
        }
    }
}

/// A day that gets its own message, bouquet or effects.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecialDate {
    /// "MM-DD" for every year, or "YYYY-MM-DD" for one day only.
    pub date: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub bouquet: Option<String>,
    /// The year it started, so `{years}` can count up.
    #[serde(default)]
    pub since: Option<i32>,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

/// A word you can whisper (press `/`) to unlock something.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Secret {
    pub word: String,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub bouquet: Option<String>,
    #[serde(default)]
    pub burst: bool,
}

impl Config {
    /// Loads `explicit` if given (it must exist), else the default path if it
    /// exists, else the built-in defaults.
    pub fn load(explicit: Option<&Path>) -> Result<Config> {
        let path = match explicit {
            Some(p) => p.to_path_buf(),
            None => match config_path() {
                Some(p) if p.exists() => p,
                _ => return Ok(Config::default()),
            },
        };
        let text =
            fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        let config: Config =
            toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
        config
            .validate()
            .with_context(|| format!("checking {}", path.display()))?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if let Some(bg) = &self.background
            && Rgb::parse(bg).is_none()
        {
            bail!("background: unknown color \"{bg}\"");
        }
        for d in &self.dates {
            When::parse(&d.date)?;
        }
        Ok(())
    }

    /// The special date matching `day`, if any. One-off dates win over yearly ones.
    pub fn occasion(&self, day: NaiveDate) -> Option<&SpecialDate> {
        let matching = |once: bool| {
            self.dates.iter().find(move |d| match When::parse(&d.date) {
                Ok(When::Once(date)) => once && date == day,
                Ok(When::Yearly(m, dd)) => !once && day.month() == m && day.day() == dd,
                Err(_) => false,
            })
        };
        matching(true).or_else(|| matching(false))
    }
}

enum When {
    Yearly(u32, u32),
    Once(NaiveDate),
}

impl When {
    fn parse(s: &str) -> Result<When> {
        let parts: Vec<&str> = s.trim().split('-').collect();
        let num = |p: &str| {
            p.parse::<u32>()
                .with_context(|| format!("bad date \"{s}\""))
        };
        match parts.as_slice() {
            [m, d] => {
                let (m, d) = (num(m)?, num(d)?);
                // 2024 is a leap year, so "02-29" is accepted.
                if NaiveDate::from_ymd_opt(2024, m, d).is_none() {
                    bail!("bad date \"{s}\" (use MM-DD or YYYY-MM-DD)");
                }
                Ok(When::Yearly(m, d))
            }
            [_, _, _] => NaiveDate::parse_from_str(s.trim(), "%Y-%m-%d")
                .map(When::Once)
                .with_context(|| format!("bad date \"{s}\" (use MM-DD or YYYY-MM-DD)")),
            _ => bail!("bad date \"{s}\" (use MM-DD or YYYY-MM-DD)"),
        }
    }
}

/// Parses `--date`: "YYYY-MM-DD", or "MM-DD" in the current year.
pub fn parse_date_arg(s: &str, today: NaiveDate) -> Result<NaiveDate> {
    match When::parse(s)? {
        When::Once(d) => Ok(d),
        When::Yearly(m, d) => NaiveDate::from_ymd_opt(today.year(), m, d)
            .with_context(|| format!("{s} doesn't exist this year")),
    }
}

/// Replaces `{name}`, `{from}` and `{years}` in a message.
pub fn fill(template: &str, name: Option<&str>, from: Option<&str>, years: Option<i32>) -> String {
    template
        .replace("{name}", name.unwrap_or("you"))
        .replace("{from}", from.unwrap_or(""))
        .replace("{years}", &years.map(|y| y.to_string()).unwrap_or_default())
}

pub fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("posy"))
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("config.toml"))
}

pub fn bouquets_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join("bouquets"))
}

/// Expands a leading `~/` to the home directory.
pub fn expand_tilde(path: &Path) -> PathBuf {
    match (path.strip_prefix("~"), dirs::home_dir()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path.to_path_buf(),
    }
}

/// The commented example written by `posy init`.
pub const EXAMPLE: &str = include_str!("../examples/config.toml");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_config_parses() {
        let config: Config = toml::from_str(EXAMPLE).unwrap();
        config.validate().unwrap();
        assert!(!config.dates.is_empty());
    }

    #[test]
    fn occasions_match() {
        let config: Config = toml::from_str(
            r#"
            [[dates]]
            date = "02-14"
            message = "yearly"
            [[dates]]
            date = "2027-02-14"
            message = "once"
            "#,
        )
        .unwrap();
        let day = |y, m, d| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        assert_eq!(
            config
                .occasion(day(2026, 2, 14))
                .unwrap()
                .message
                .as_deref(),
            Some("yearly")
        );
        assert_eq!(
            config
                .occasion(day(2027, 2, 14))
                .unwrap()
                .message
                .as_deref(),
            Some("once")
        );
        assert!(config.occasion(day(2026, 2, 15)).is_none());
    }

    #[test]
    fn fills_placeholders() {
        assert_eq!(
            fill("hi {name}, {years} years", Some("sam"), None, Some(3)),
            "hi sam, 3 years"
        );
        assert_eq!(fill("hi {name}", None, None, None), "hi you");
    }
}
