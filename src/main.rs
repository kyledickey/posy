//! posy: a little 3D bouquet for your terminal.

mod app;
mod bigtext;
mod bouquet;
mod camera;
mod canvas;
mod cli;
mod color;
mod commands;
mod config;
mod daily;
mod effects;
mod export;
mod flowers;
mod garden;
mod glyphs;
mod hud;
mod library;
mod math;
mod menu;
mod mesh;
mod octants;
mod raster;
mod render;
mod rng;
mod scene;
mod sha256;
mod update;

use anyhow::Result;
use clap::Parser;

use crate::cli::{Cli, Command};
use crate::config::Config;

fn main() -> Result<()> {
    // Windows consoles only understand colour codes once asked to.
    #[cfg(windows)]
    crossterm::ansi_support::supports_ansi();
    let cli = Cli::parse();
    let config = Config::load(cli.config.as_deref())?;

    match &cli.command {
        None => app::run(&cli, config),
        Some(Command::List) => commands::list(),
        Some(Command::Show { bouquet }) => commands::show(bouquet),
        Some(Command::Init { force }) => commands::init(*force),
        Some(Command::Snapshot {
            out,
            size,
            at,
            keys,
            frames,
            every,
        }) => app::snapshot(&cli, config, out, size, *at, keys, (*frames, *every)),
        Some(Command::Update { check }) => update::run(*check),
        Some(Command::Bench { size }) => app::bench(&cli, config, size),
        Some(Command::Garden { out, size, open }) => {
            let (w, h) = app::parse_size(size)?;
            garden::run(
                out,
                w,
                h,
                *open,
                app::DEFAULT_BG,
                cli.style.unwrap_or_default(),
            )
        }
    }
}
