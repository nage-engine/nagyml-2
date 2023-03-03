use std::{collections::{HashMap, HashSet}, path::PathBuf};

use anyhow::Result;

use crate::loading::{load_content, get_content_iterator, load_files};

use super::{scripts::Scripts, text::{Translations, TranslationFile}, prompt::{Prompts, Prompt}};

pub type InfoPages = HashMap<String, String>;
pub type UnlockedInfoPages = HashSet<String>;

pub type Audio = HashMap<String, PathBuf>;

pub struct Resources {
	pub prompts: Prompts,
	pub translations: Translations,
	pub info_pages: InfoPages,
	pub scripts: Scripts,
	pub audio: Audio
}

impl Resources {
	pub fn load() -> Result<Self> {
		let result = Resources {
			prompts: load_content("prompts")?,
			translations: load_content("lang")?,
			info_pages: load_files("info")?,
			scripts: Scripts::load()?,
			audio: get_content_iterator("audio").collect()
		};
		Ok(result)
	}

	pub fn validate(&self) -> Result<()> {
		let _ = Prompt::validate_all(&self.prompts)?;
		Ok(())
	}

	pub fn lang_file(&self, lang: &str) -> Option<&TranslationFile> {
		self.translations.get(lang)
	}

}