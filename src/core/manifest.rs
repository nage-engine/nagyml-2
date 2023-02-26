use anyhow::{Result, anyhow, Context};
use semver::Version;
use serde::Deserialize;

use crate::loading::parse;

use super::{path::Path, choice::{Variables, Notes}, text::{TextSpeed, TextLines}};

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
/// A collection of settings that identify information about the game itself and its authors.
pub struct Metadata {
	name: String,
	authors: Vec<String>,
	version: Version,
	pub contact: Option<Vec<String>>
}

impl Metadata {
	pub fn global_variable(&self, var: &str) -> Option<String> {
		match var {
			"nage.game_name" => Some(self.name.clone()),
			"nage.game_authors" => Some(self.authors.join(", ")),
			"nage.game_version" => Some(self.version.to_string()),
			_ => None
		}
	}
}

#[derive(Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct HistorySettings {
	pub locked: bool,
	pub size: usize
}

impl Default for HistorySettings {
	fn default() -> Self {
		Self { 
			locked: false,
			size: 5
		}
	}
}

#[derive(Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
	pub save: bool,
	pub debug: bool,
	pub speed: TextSpeed,
	pub history: HistorySettings,
	pub lang: Option<String>
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			save: true,
			debug: false,
			speed: TextSpeed::Delay(5),
			history: HistorySettings::default(),
			lang: None
		}
	}
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Entrypoint {
	pub path: Path,
	pub background: Option<TextLines>,
	pub notes: Option<Notes>,
	pub variables: Option<Variables>
}

#[derive(Deserialize, Debug)]
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