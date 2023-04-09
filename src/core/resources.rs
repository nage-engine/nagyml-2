use anyhow::Result;

use crate::{
    loading::loader::Loader,
    text::display::{TranslationFile, Translations},
};

use super::{
    audio::{Audio, SoundActions},
    context::{StaticContext, TextContext},
    manifest::Manifest,
    player::Player,
    prompt::{Prompt, Prompts},
    scripts::Scripts,
    state::InfoPages,
};

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

    pub fn validate(&self, stc: &StaticContext) -> Result<()> {
        let _ = Prompt::validate_all(stc)?;
        Ok(())
    }

    pub fn lang_file(&self, lang: &str) -> Option<&TranslationFile> {
        self.translations.get(lang)
    }

    /// If the [`Audio`] resource exists, submits a collection of [`SoundActions`] to it.
    pub fn submit_audio(
        &self,
        player: &Player,
        sounds: &SoundActions,
        text_context: &TextContext,
    ) -> Result<()> {
        if let Some(audio) = &self.audio {
            for sound in sounds {
                audio.accept(player, sound, text_context)?;
            }
        }
        Ok(())
    }
}
