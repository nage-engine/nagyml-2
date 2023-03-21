use std::collections::{BTreeMap, HashSet};

use anyhow::Result;

use crate::{
    loading::loader::Loader,
    text::display::{TranslationFile, Translations},
};

use super::{
    audio::Audio,
    manifest::Manifest,
    prompt::{Prompt, Prompts},
    scripts::Scripts,
};

pub type InfoPages = BTreeMap<String, String>;
pub type UnlockedInfoPages = HashSet<String>;

pub struct Resources {
    pub prompts: Prompts,
    pub translations: Translations,
    pub info_pages: InfoPages,
    pub scripts: Scripts,
    pub audio: Option<Audio>,
}

impl Resources {
    pub fn load(loader: &Loader, config: &Manifest) -> Result<Self> {
        let result = Resources {
            prompts: loader.load_content("prompts")?,
            translations: loader.load_content("lang")?,
            info_pages: loader.load_raw_content("info")?,
            scripts: Scripts::load(loader)?,
            audio: Audio::load(loader, config)?,
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
