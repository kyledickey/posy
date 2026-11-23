//! Command-line arguments.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::bouquet::{Effect, WrapStyle};
use crate::color::ColorChoice;
use crate::raster::Style;

/// A little 3D bouquet for your terminal
#[derive(Parser, Debug)]
#[command(name = "posy", version, about)]
pub struct Cli {
    /// Bouquet to show: a preset, a saved bouquet, "daily", "random", or a .toml file
    #[arg(short, long, global = true)]
    pub bouquet: Option<String>,

    /// Message under the bouquet (repeat it to rotate through several)
    #[arg(short, long, global = true)]
    pub message: Vec<String>,

    /// Who it's for (fills {name} in messages)
    #[arg(long, global = true)]
    pub name: Option<String>,

    /// Who it's from (fills {from} and adds a signature)
    #[arg(long, global = true)]
    pub from: Option<String>,

    /// Arrangement seed: the same seed always gives the same layout
    #[arg(long, global = true)]
    pub seed: Option<u64>,

    /// What the stems sit in
    #[arg(short, long, value_enum, global = true)]
    pub wrap: Option<WrapStyle>,

    /// Ambient effects, comma separated (e.g. hearts,snow)
    #[arg(short, long, value_enum, value_delimiter = ',', global = true)]
    pub effects: Option<Vec<Effect>>,

    /// Turn off all effects
    #[arg(long, conflicts_with = "effects", global = true)]
    pub no_effects: bool,

    /// Color output
    #[arg(long, value_enum, global = true)]
    pub color: Option<ColorChoice>,

    /// What to draw with: letters, dots, or coloured blocks
    #[arg(long, value_enum, global = true)]
    pub style: Option<Style>,

    /// Frames per second
    #[arg(long)]
    pub fps: Option<u32>,

    /// Spin speed in radians per second (negative spins the other way)
    #[arg(long, allow_negative_numbers = true)]
    pub speed: Option<f32>,

    /// Skip the growing animation
    #[arg(long, global = true)]
    pub no_grow: bool,

    /// Hide the buttons along the bottom (h brings them back)
    #[arg(long, global = true)]
    pub clean: bool,

    /// Open the "make your own" menu right away
    #[arg(long, alias = "builder")]
    pub menu: bool,

    /// Pretend today is this date (MM-DD or YYYY-MM-DD), to test special dates
    #[arg(long, global = true)]
    pub date: Option<String>,

    /// Use this config file instead of the usual one (`posy init` shows where that is)
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List every bouquet you can show
    List,
    /// Print a bouquet as TOML (copy it, change it, save it as your own)
    Show {
        /// Bouquet name or path
        bouquet: String,
    },
    /// Write an example config file, and say where it went
    Init {
        /// Overwrite an existing config
        #[arg(long)]
        force: bool,
    },
    /// Render a single frame to a file (.html, .ans or .txt)
    Snapshot {
        /// Output file
        out: PathBuf,
        /// Size in characters, WIDTHxHEIGHT
        #[arg(long, default_value = "100x48")]
        size: String,
        /// Seconds into the animation
        #[arg(long, default_value_t = 6.0)]
        at: f32,
        /// Keys to press first, comma separated (down,enter,x ...)
        #[arg(long, value_delimiter = ',', hide = true)]
        keys: Vec<String>,
        /// Save this many frames in a row (out-000.ans, out-001.ans ...)
        #[arg(long, default_value_t = 1, hide = true)]
        frames: usize,
        /// Seconds between those frames
        #[arg(long, default_value_t = 0.1, hide = true)]
        every: f32,
    },
    /// Get the newest posy (or just see whether there is one)
    Update {
        /// Only look; don't change anything
        #[arg(long)]
        check: bool,
    },
    /// Time how long frames take
    #[command(hide = true)]
    Bench {
        /// Size in characters, WIDTHxHEIGHT
        #[arg(long, default_value = "170x58")]
        size: String,
    },
    /// Every kind of flower side by side (for tuning how they look)
    #[command(hide = true)]
    Garden {
        /// Output file
        out: PathBuf,
        /// Size in characters, WIDTHxHEIGHT
        #[arg(long, default_value = "150x64")]
        size: String,
        /// How far open the flowers are, 0 to 1
        #[arg(long, default_value_t = 1.0)]
        open: f32,
    },
}
