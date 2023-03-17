use std::{path::{PathBuf, Path}, ffi::OsStr};

use anyhow::{Result, anyhow, Context};

use crate::core::{player::Player, manifest::Manifest};

use super::loader::Loader;

pub struct SaveManager {
	dir: PathBuf
}

impl SaveManager {
	pub fn dir(config: &Manifest, create: bool) -> Result<PathBuf> {
		let dir = Loader::config_dir()?
    		.join("games")
			.join(config.metadata.game_id())
			.join("saves");
		if !dir.exists() {
			if create {
				std::fs::create_dir_all(&dir)?;
			}
			else {
				return Err(anyhow!("Saves directory does not exist"))
			}
		}
		Ok(dir)
	}

	pub fn new(config: &Manifest) -> Result<Self> {
		let saves = Self { 
			dir: SaveManager::dir(config, true)?
		};
		Ok(saves)
	}

	fn save_name_storage(&self) -> PathBuf {
		self.dir.join("save.txt")
	}

	fn last_save_file(&self) -> Result<String> {
		std::fs::read_to_string(self.save_name_storage())
    		.map_err(|err| anyhow!(err))
	}

	fn load_player<P>(&self, file: P) -> Result<Player> where P: AsRef<Path> {
		let content = std::fs::read_to_string(self.dir.join(&file))?;
		Loader::parse(content)
			.with_context(|| anyhow!("Failed to parse save file '{}'", file.as_ref().display()))
	}
	
	fn load_last_save(&self) -> Result<(Player, PathBuf)> {
		let file = self.last_save_file()?;
		Ok((self.load_player(file.clone())?, PathBuf::from(file)))
	}

	fn saves(&self) -> Result<Vec<PathBuf>> {
		let result = std::fs::read_dir(&self.dir)?
			.filter_map(|entry| entry.ok())
			.map(|entry| entry.path())
			.filter(|path| path.extension().and_then(OsStr::to_str).map(|p| p == "yml").unwrap_or(false))
			.collect();
		Ok(result)
	}

	fn choose_save(saves: &Vec<PathBuf>) -> Result<PathBuf> {
		let save_names: Vec<&str> = saves.iter()
    		.map(|save| save.file_stem().and_then(OsStr::to_str).unwrap())
			.collect();
		let prompt = requestty::Question::select("Choose a save file")
    		.choices(save_names)
			.build();
		let choice = requestty::prompt_one(prompt)?.as_list_item().unwrap().index;

		println!();

		Ok(saves[choice].clone())
	}

	pub fn load(&self, config: &Manifest, pick: bool, new: bool) -> Result<(Player, Option<PathBuf>)> {
		let saves = self.saves()?;
		if new || saves.is_empty() {
			return Ok((Player::new(config), None));
		}
		if pick {
			let save = Self::choose_save(&saves)?;
			Ok((self.load_player(&save)?, Some(save)))
		}
		else {
			self.load_last_save().map(|(player, path)| (player, Some(path)))
		}
	}

	fn prompt_new_save_file() -> Result<String> {
		println!();
		let prompt = requestty::Question::input("Save file name")
    		.validate(|file, _| {
				if !sanitize_filename::is_sanitized(file) {
					return Err("Invalid file name".to_owned())
				}
				Ok(())
			})
			.build();
		let answer = requestty::prompt_one(prompt)?;
		Ok(format!("{}.yml", answer.as_string().unwrap()))
	}
	
	fn write_player(&self, save_file: &PathBuf, player: &Player) {
		if let Ok(content) = serde_yaml::to_string(player) {
			let _ = std::fs::write(self.dir.join(&save_file), content);
		}	
	}

	pub fn write(&self, player: &Player, save_file: Option<PathBuf>, new: bool) -> Result<()> {
		let save = if new || save_file.is_none() {
			PathBuf::from(Self::prompt_new_save_file()?)
		}
		else {
			save_file.unwrap()
		};
		self.write_player(&save, player);
		let _ = std::fs::write(self.save_name_storage(), save.to_str().unwrap());
		Ok(())
	}
}