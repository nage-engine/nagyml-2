use anyhow::{Result, anyhow, Context};
use semver::Version;
use serde::Deserialize;

use crate::loading::parse;

use super::{choice::{Variables, Notes}, text::{TextSpeed, TextLines, TemplatableValue}, player::PathEntry, resources::UnlockedInfoPages};

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
/// A collection of settings that identify information about the game itself and its authors.
pub struct Metadata {
	pub name: String,
	pub authors: Vec<String>,
	pub version: Version,
	pub contact: Option<Vec<String>>
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
	pub lang: Option<String>,
	pub channels: Option<Vec<String>>
}

impl Default for Settings {
	fn default() -> Self {
		Self {
			save: true,
			debug: false,
			speed: TextSpeed::Delay(TemplatableValue::value(5)),
			history: HistorySettings::default(),
			lang: None,
			channels: None
		}
	}
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Entrypoint {
	pub path: PathEntry,
	pub background: Option<TextLines>,
	pub notes: Option<Notes>,
	pub variables: Option<Variables>,
	#[serde(rename = "info")]
	pub info_pages: Option<UnlockedInfoPages>,
	pub log: Option<Vec<String>>
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