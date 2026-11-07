use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, bail};
use livekit_wakeword::WakeWordModel;
use log::info;

use crate::audio::RATE;

/// The .onnx models in a directory, named by file stem.
pub struct Models {
    paths: BTreeMap<String, PathBuf>,
}

impl Models {
    /// Loads each model once, so a bad file fails at startup.
    pub fn scan(dir: &Path) -> anyhow::Result<Self> {
        let mut paths = BTreeMap::new();
        for entry in std::fs::read_dir(dir).with_context(|| format!("reading {}", dir.display()))? {
            let path = entry?.path();
            if path.extension().is_some_and(|e| e == "onnx")
                && let Some(name) = path.file_stem().and_then(|s| s.to_str())
            {
                paths.insert(name.to_owned(), path);
            }
        }
        if paths.is_empty() {
            bail!("no .onnx models in {}", dir.display());
        }

        let models = Self { paths };
        for name in models.names() {
            let started = Instant::now();
            models.load(name)?;
            info!("loaded {name} in {:.2?}", started.elapsed());
        }
        Ok(models)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.paths.keys().map(String::as_str)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.paths.contains_key(name)
    }

    pub fn load(&self, name: &str) -> anyhow::Result<WakeWordModel> {
        let path = &self.paths[name];
        WakeWordModel::new(&[path], RATE).with_context(|| format!("loading {}", path.display()))
    }
}
