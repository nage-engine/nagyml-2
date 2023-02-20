use std::{collections::HashMap, fmt::Display};

use anyhow::{Result, anyhow, Context};
use semver::Version;
use serde::{Deserialize, Serialize};

use crate::loading::parse;

use super::{path::Path, choice::{Variables, Notes}, text::{TextSpeed, TextLines}};

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A collection of settings that identify information about the game itself and its authors.
pub struct Metadata {
	name: String,
	authors: Vec<String>,
	version: Version,
	contact: Option<HashMap<String, String>>
}

impl Display for Metadata {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "{} v{} by {}", self.name, self.version, self.authors.join(", "))
	}
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(default, deny_unknown_fields)]
///
pub struct HistorySettings {
	locked: bool,
	size: usize
}

impl Default for HistorySettings {
	fn default() -> Self {
		Self { 
			locked: false,
			size: 5
		}
	}
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
	pub save: bool,
	pub debug: bool,
	pub speed: TextSpeed,
	pub history: HistorySettings
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			save: true,
			debug: false,
			speed: TextSpeed::Delay(5),
			history: HistorySettings::default()
		}
	}
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Entrypoint {
	pub path: Path,
	pub background: Option<TextLines>,
	pub notes: Option<Notes>,
	pub variables: Option<Variables>
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
	pub metadata: Metadata,
	pub settings: Settings,
	pub entry: Entrypoint
}

impl Manifest {
	const FILE: &'static str = "nage.yml";

	pub fn load() -> Result<Self> {
		let content = std::fs::read_to_string(Self::FILE)
			.with_context(|| format!("'{}' doesn't exist!", Self::FILE))?;
		let config: Self = parse(Self::FILE, &content)?;
		config.validate()?;
		Ok(config)
	}

	fn validate(&self) -> Result<()> {
		let size = self.settings.history.size;
		if size == 0 {
			return Err(anyhow!("Failed to validate nage.yml: `settings.history.size` must be non-zero"));
		}
		Ok(())
	}
}