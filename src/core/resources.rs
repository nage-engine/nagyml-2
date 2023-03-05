use std::collections::{HashMap, HashSet};

use anyhow::{Result, anyhow};
use playback_rs::{Player as AudioPlayer, Song};
use result::OptionResultExt;

use crate::loading::{load_content, get_content_iterator, load_files};

use super::{scripts::Scripts, text::{Translations, TranslationFile}, prompt::{Prompts, Prompt}};

pub type InfoPages = HashMap<String, String>;
pub type UnlockedInfoPages = HashSet<String>;

pub type Sounds = HashMap<String, Song>;

pub struct Audio {
	pub player: AudioPlayer,
	pub sounds: Sounds
}

impl Audio {
	pub fn load_sounds() -> Result<Sounds> {
		get_content_iterator("sounds")
    		.map(|(key, path)| {
				Song::from_file(path, None)
					.map(|song| (key, song))
					.map_err(|err| anyhow!(err))
			})
    		.collect()
	}

	pub fn load() -> Result<Option<Self>> {
		AudioPlayer::new(None).ok().map(|player| {
			Self::load_sounds().map(|sounds| {
				Self { player, sounds }
			})
		})
		.invert()
	}

	pub fn play_sound(&self, sound: &str) -> Result<()> {
		let sfx = self.sounds.get(sound)
			.ok_or(anyhow!("Invalid sound file '{sound}'"))?;
		let _ = self.player.play_song_now(sfx, None);
		Ok(())
	}
}

pub struct Resources {
	pub prompts: Prompts,
	pub translations: Translations,
	pub info_pages: InfoPages,
	pub scripts: Scripts,
	pub audio: Option<Audio>
}

impl Resources {
	pub fn load() -> Result<Self> {
		let result = Resources {
			prompts: load_content("prompts")?,
			translations: load_content("lang")?,
			info_pages: load_files("info")?,
			scripts: Scripts::load()?,
			audio: Audio::load()?
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