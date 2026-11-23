//! The non-animated subcommands: `list`, `show` and `init`.

use std::fs;

use anyhow::{Result, bail};
use chrono::{Datelike, Local};

use crate::config;
use crate::library::{Library, Source};
use crate::rng::Rng;

fn library() -> Library {
    let today = Local::now().date_naive();
    Library::load(
        crate::app::daily_seed(today),
        today.month(),
        config::bouquets_dir().as_deref(),
    )
}

pub fn list() -> Result<()> {
    let lib = library();
    let width = lib
        .entries
        .iter()
        .map(|e| e.name.chars().count())
        .max()
        .unwrap_or(0);
    for (title, wanted) in [("today", 0), ("presets", 1), ("saved", 2)] {
        let entries: Vec<_> = lib
            .entries
            .iter()
            .filter(|e| match e.source {
                Source::Daily => wanted == 0,
                Source::Preset => wanted == 1,
                Source::Saved(_) => wanted == 2,
                _ => false,
            })
            .collect();
        if entries.is_empty() {
            continue;
        }
        println!("{title}:");
        for e in entries {
            println!("  {:<width$}  {}", e.name, e.spec.summary());
        }
        println!();
    }
    if let Some(dir) = config::bouquets_dir() {
        println!("saved bouquets live in {}", dir.display());
    }
    for w in &lib.warnings {
        eprintln!("warning: {w}");
    }
    Ok(())
}

pub fn show(query: &str) -> Result<()> {
    let mut lib = library();
    let i = lib.resolve(query, Rng::from_time().next_u64())?;
    print!("{}", toml::to_string_pretty(&lib.entries[i].spec)?);
    Ok(())
}

pub fn init(force: bool) -> Result<()> {
    let Some(path) = config::config_path() else {
        bail!("couldn't find a config directory on this system");
    };
    if path.exists() && !force {
        bail!(
            "{} already exists (use --force to overwrite it)",
            path.display()
        );
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    fs::write(&path, config::EXAMPLE)?;
    println!("wrote {}", path.display());
    println!("edit it to add her name, your messages and special dates ♥");
    Ok(())
}
