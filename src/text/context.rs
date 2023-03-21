use rlua::{Context, Table};

use crate::core::{manifest::Manifest, choice::{Notes, Variables}, scripts::Scripts, audio::Audio, resources::Resources};

use super::display::TranslationFile;

/// A wrapper for all data relevant for filling in [`TemplatableString`]s.
/// 
/// This struct must own copies of mutable player data (notes and variables).
/// Immutable resource data must be referenced.
/// 
/// A set of global "nage" variables consistent between both templating and scripts are derived from this context.
/// They are as follows:
/// - `game_name`: The metadata's `name` key
/// - `game_authors`: The metadata's `authors` key, represented as a sequence
/// - `game_version`: The metadata's `version` key
/// - `lang`: The currently loaded language key
pub struct TextContext<'a> {
	pub config: &'a Manifest,
	pub notes: Notes,
	pub variables: Variables,
	pub lang: String,
	pub lang_file: Option<&'a TranslationFile>,
	pub scripts: &'a Scripts,
	pub audio: &'a Option<Audio>
}

impl<'a> TextContext<'a> {
	/// Constructs a new [`TextContext`] by accessing [`Resources`] internals.
	pub fn new(config: &'a Manifest, notes: Notes, variables: Variables, lang: &str, resources: &'a Resources) -> Self {
		TextContext { 
			config, 
			notes,
			variables,
			lang: lang.to_owned(),
			lang_file: resources.lang_file(lang), 
			scripts: &resources.scripts,
			audio: &resources.audio
		}
	}

	/// Attempts to fetch a global variable for direct templating.
	/// These variables are prefixed under `nage:`.
	/// 
	/// The `game_authors` variable is separated by commas.
	pub fn global_variable(&self, var: &str) -> Option<String> {
		var.to_lowercase().strip_prefix("nage:").map(|name| {
			match name {
				"game_name" => Some(self.config.metadata.name.clone()),
				"game_authors" => Some(self.config.metadata.authors.join(", ")),
				"game_version" => Some(self.config.metadata.version.to_string()),
				"lang" => Some(self.lang.to_owned()),
				_ => None
			}
		})
		.flatten()
	}

	/// Creates a global variable table for use in scripts.
	/// This should be set as a global `nage` table.
	pub fn create_variable_table<'b>(&self, context: &Context<'b>) -> Result<Table<'b>, rlua::Error> {
		let table = context.create_table()?;
		table.set("game_name", self.config.metadata.name.clone())?;
		table.set("game_authors", context.create_sequence_from(self.config.metadata.authors.clone())?)?;
		table.set("game_version", self.config.metadata.version.to_string())?;
		table.set("lang", self.lang.clone())?;
		Ok(table)
	}
}