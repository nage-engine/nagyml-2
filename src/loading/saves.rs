use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};

use crate::core::{manifest::Manifest, player::Player};

use super::loader::Loader;

pub struct SaveManager {
    dir: Utf8PathBuf,
    pub save_file: Option<Utf8PathBuf>,
}

impl SaveManager {
    pub fn generic_dir() -> Result<Utf8PathBuf> {
        Ok(Loader::config_dir()?.join("games"))
    }

    pub fn game_dir(config: &Manifest) -> Result<Utf8PathBuf> {
        let dir = Self::generic_dir()?
            .join(config.metadata.game_id())
            .join("saves");
        Ok(dir)
    }

    fn dir(config: &Manifest) -> Result<Utf8PathBuf> {
        let dir = Self::game_dir(config)?;
        if !dir.exists() {
            std::fs::create_dir_all(&dir)?;
        }
        Ok(dir)
    }

    pub fn new(config: &Manifest, pick: bool, new: bool) -> Result<Self> {
        let dir = Self::dir(config)?;
        let saves = Self::saves(&dir)?;
        let save_file = if new || saves.is_empty() {
            None
        } else if pick {
            Some(Self::choose_save(&saves)?)
        } else {
            Self::last_save_file(&dir).ok()
        };
        Ok(Self { dir, save_file })
    }

    fn save_name_storage<P>(path: P) -> Utf8PathBuf
    where
        P: AsRef<Utf8Path>,
    {
        path.as_ref().join("save.txt")
    }

    fn last_save_file<P>(path: P) -> Result<Utf8PathBuf>
    where
        P: AsRef<Utf8Path>,
    {
        let string = std::fs::read_to_string(Self::save_name_storage(path))?;
        Ok(Utf8PathBuf::from(string))
    }

    fn load_player<P>(&self, file: P) -> Result<Player>
    where
        P: AsRef<Utf8Path>,
    {
        let content = std::fs::read_to_string(self.dir.join(&file))?;
        Loader::parse(content)
            .with_context(|| anyhow!("Failed to parse save file '{}'", file.as_ref()))
    }

    fn saves<P>(dir: P) -> Result<Vec<Utf8PathBuf>>
    where
        P: AsRef<Utf8Path>,
    {
        let result = std::fs::read_dir(dir.as_ref())?
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| Utf8PathBuf::from_path_buf(entry.path()).ok())
            .filter(|path| path.extension().map(|p| p == "yml").unwrap_or(false))
            .collect();
        Ok(result)
    }

    fn choose_save<P>(saves: &Vec<P>) -> Result<Utf8PathBuf>
    where
        P: AsRef<Utf8Path>,
    {
        let save_names: Vec<String> = saves
            .iter()
            .map(|save| save.as_ref().file_stem().map(ToString::to_string).unwrap())
            .collect();
        let prompt = requestty::Question::select("Choose a save file")
            .choices(save_names)
            .build();
        let choice = requestty::prompt_one(prompt)?.as_list_item().unwrap().index;

        println!();

        Ok(saves[choice].as_ref().to_path_buf())
    }

    pub fn load(&self, config: &Manifest) -> Result<Player> {
        match &self.save_file {
            Some(save) => self.load_player(save),
            None => Ok(Player::new(config)),
        }
    }

    fn prompt_new_save_file() -> Result<String> {
        println!();
        let prompt = requestty::Question::input("Save file name")
            .validate(|file, _| {
                if !sanitize_filename::is_sanitized(file) {
                    return Err("Invalid file name".to_owned());
                }
                Ok(())
            })
            .build();
        let answer = requestty::prompt_one(prompt)?;
        Ok(format!("{}.yml", answer.as_string().unwrap()))
    }

    fn write_player<P>(&self, save_file: P, player: &Player)
    where
        P: AsRef<Utf8Path>,
    {
        if let Ok(content) = serde_yaml::to_string(player) {
            let _ = std::fs::write(self.dir.join(&save_file), content);
        }
    }

    pub fn write(&self, player: &Player) -> Result<()> {
        let save = match &self.save_file {
            Some(value) => value.clone(),
            None => Utf8PathBuf::from(Self::prompt_new_save_file()?),
        };
        self.write_player(&save, player);
        let _ = std::fs::write(Self::save_name_storage(&self.dir), save.to_string());
        Ok(())
    }
}
