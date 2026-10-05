#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use super::{Config, Figment};
use crate::{Result, implement};

/// Applies the entry point's own contribution to a freshly loaded
/// configuration.
pub type Overrides = dyn Fn(Figment) -> Result<Figment> + Send + Sync;

/// What a configuration is built from besides the environment, retained so a
/// reload reproduces the sources the server started with.
#[derive(Default)]
pub struct Sources {
	pub paths: Vec<PathBuf>,
	pub overrides: Option<Box<Overrides>>,
}

/// Builds a raw configuration from these sources, with `extra` paths layered
/// after them.
#[implement(Sources)]
pub fn load<'a, I>(&'a self, extra: I) -> Result<Figment>
where
	I: Iterator<Item = &'a Path>,
{
	let paths = self
		.paths
		.iter()
		.map(PathBuf::as_path)
		.chain(extra);

	Config::load(paths).and_then(|raw| self.apply(raw))
}

#[implement(Sources)]
fn apply(&self, raw: Figment) -> Result<Figment> {
	match &self.overrides {
		| None => Ok(raw),
		| Some(overrides) => overrides(raw),
	}
}
