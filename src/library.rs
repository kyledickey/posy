//! Every bouquet you can flip through: today's daily bouquet, the built-in
//! presets, and bouquets saved in `~/.config/posy/bouquets/`.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};

use crate::bouquet::{self, BouquetSpec};
use crate::daily;

#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Daily,
    Random,
    Preset,
    Saved(PathBuf),
    File(PathBuf),
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub source: Source,
    pub spec: BouquetSpec,
}

pub struct Library {
    pub entries: Vec<Entry>,
    /// Saved files that couldn't be loaded, with the reason.
    pub warnings: Vec<String>,
}

impl Library {
    pub fn load(daily_seed: u64, month: u32, saved_dir: Option<&Path>) -> Library {
        let mut entries = vec![Entry {
            name: "daily".into(),
            source: Source::Daily,
            spec: daily::generate(daily_seed, "daily", month),
        }];
        for (name, _) in bouquet::PRESETS {
            let spec = bouquet::preset(name).expect("built-in presets always parse");
            entries.push(Entry {
                name: name.to_string(),
                source: Source::Preset,
                spec,
            });
        }

        let mut warnings = Vec::new();
        if let Some(dir) = saved_dir {
            let mut paths: Vec<PathBuf> = fs::read_dir(dir)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|ext| ext == "toml"))
                .collect();
            paths.sort();
            for path in paths {
                match load_file(&path) {
                    Ok(spec) => entries.push(Entry {
                        name: spec.name.clone(),
                        source: Source::Saved(path),
                        spec,
                    }),
                    Err(e) => warnings.push(format!("{}: {e:#}", path.display())),
                }
            }
        }
        Library { entries, warnings }
    }

    /// Finds a bouquet by name. Saved bouquets win over presets with the same name.
    pub fn find(&self, name: &str) -> Option<usize> {
        self.entries
            .iter()
            .rposition(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// Finds a bouquet by name, makes a random one for "random", or loads a
    /// `.toml` file from a path.
    pub fn resolve(&mut self, query: &str, random_seed: u64) -> Result<usize> {
        if query.eq_ignore_ascii_case("random") {
            return Ok(self.add_random(random_seed));
        }
        if let Some(i) = self.find(query) {
            return Ok(i);
        }
        let path = Path::new(query);
        if path.is_file() {
            let spec = load_file(path)?;
            self.entries.push(Entry {
                name: spec.name.clone(),
                source: Source::File(path.to_path_buf()),
                spec,
            });
            return Ok(self.entries.len() - 1);
        }
        bail!("no bouquet called \"{query}\" (run `posy list` to see them all)")
    }

    /// Adds (or replaces) the "random" entry and returns its index.
    pub fn add_random(&mut self, seed: u64) -> usize {
        let entry = Entry {
            name: "random".into(),
            source: Source::Random,
            spec: daily::generate(seed, "random", 0),
        };
        match self.entries.iter().position(|e| e.source == Source::Random) {
            Some(i) => {
                self.entries[i] = entry;
                i
            }
            None => {
                self.entries.push(entry);
                self.entries.len() - 1
            }
        }
    }

    /// Records a just-saved bouquet, replacing an older save at the same path.
    pub fn upsert_saved(&mut self, path: PathBuf, spec: BouquetSpec) -> usize {
        let entry = Entry {
            name: spec.name.clone(),
            source: Source::Saved(path.clone()),
            spec,
        };
        match self
            .entries
            .iter()
            .position(|e| e.source == Source::Saved(path.clone()))
        {
            Some(i) => {
                self.entries[i] = entry;
                i
            }
            None => {
                self.entries.push(entry);
                self.entries.len() - 1
            }
        }
    }
}

pub fn load_file(path: &Path) -> Result<BouquetSpec> {
    let text = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let mut spec: BouquetSpec =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    spec.validate()?;
    if (spec.name.is_empty() || spec.name == BouquetSpec::default().name)
        && let Some(stem) = path.file_stem()
    {
        spec.name = stem.to_string_lossy().into_owned();
    }
    Ok(spec)
}

/// Writes `spec` to `<dir>/<name>.toml` and returns the path.
pub fn save(dir: &Path, spec: &BouquetSpec) -> Result<PathBuf> {
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let path = dir.join(format!("{}.toml", slug(&spec.name)));
    let text = toml::to_string_pretty(spec)?;
    fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
    Ok(path)
}

/// "Our Anniversary!" → "our-anniversary"
fn slug(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars().flat_map(char::to_lowercase) {
        if ch.is_alphanumeric() {
            out.push(ch);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "bouquet".into()
    } else {
        out
    }
}
