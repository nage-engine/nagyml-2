use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use anyhow::{anyhow, Context, Result};

use semver::{Version, VersionReq};
use serde::Deserialize;

use crate::{
    loading::loader::Loader,
    text::{
        display::{TextLines, TextSpeed},
        templating::{TemplatableString, TemplatableValue},
    },
    NAGE_VERSION,
};

use super::{
    audio::{SoundAction, SoundActionMode},
    context::{StaticContext, TextContext},
    discord::{RichPresence, RichPresenceMode},
    path::PathData,
    player::{HistoryEntry, Player},
    state::{Notes, UnlockedInfoPages, Variables},
};

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
/// A collection of settings that identify information about the game itself and its authors.
pub struct Metadata {
    pub name: String,
    id: Option<String>,
    pub authors: Vec<String>,
    pub version: Version,
    #[serde(alias = "contact lines")]
    contact: Option<Vec<String>>,
}

impl Metadata {
    pub fn game_id(&self) -> &str {
        self.id.as_ref().unwrap_or(&self.name)
    }

    pub fn game_contact(&self) -> Option<String> {
        self.contact.as_ref().map(|info| {
            let contact_lines: Vec<String> =
                info.iter().map(|value| format!("- {value}")).collect();
            let joined = contact_lines.join("\n");
            format!("Contact the developers:\n{joined}")
        })
    }
}

#[derive(Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Dependencies {
    pub nage: Option<VersionReq>,
}

impl Default for Dependencies {
    fn default() -> Self {
        Self { nage: None }
    }
}

impl Dependencies {
    fn check(&self, nage_version: Version) -> Result<()> {
        self.nage
            .as_ref()
            .map(|nage| {
                if !nage.matches(&nage_version) {
                    Err(anyhow!(
                        "Dependency `nage` does not match required version (required: {}, provided: {})", 
                        nage, NAGE_VERSION
                    ))
                } else {
                    Ok(())
                }
            })
            .unwrap_or(Ok(()))
    }
}

#[derive(Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct HistorySettings {
    #[serde(alias = "locked by default")]
    pub locked: bool,
    #[serde(alias = "max size", alias = "max entries")]
    pub size: usize,
}

impl Default for HistorySettings {
    fn default() -> Self {
        Self {
            locked: false,
            size: 5,
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct TextSettings {
    pub speed: TextSpeed,
    pub wait: Option<u64>,
    #[serde(alias = "language")]
    lang: Option<String>,
}

impl Default for TextSettings {
    fn default() -> Self {
        Self {
            speed: TextSpeed::Delay(TemplatableValue::value(5)),
            wait: None,
            lang: None,
        }
    }
}

impl TextSettings {
    pub const DEFAULT_LANG: &'static str = "en_us";

    pub fn lang(&self) -> String {
        self.lang.clone().unwrap_or(Self::DEFAULT_LANG.to_owned())
    }
}

#[derive(Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct RichPresenceSettings {
    enabled: bool,
    pub icon: Option<String>,
    pub mode: RichPresenceMode,
}

impl Default for RichPresenceSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            icon: None,
            mode: RichPresenceMode::Id,
        }
    }
}

impl RichPresenceSettings {
    pub const APP_ID: &'static str = "1086477002770489417";
}

#[derive(Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    #[serde(alias = "save on quit")]
    pub save: bool,
    #[serde(alias = "developer mode")]
    pub debug: bool,
    #[serde(alias = "sound channels", alias = "audio")]
    pub channels: Option<HashMap<String, bool>>,
    pub history: HistorySettings,
    pub text: TextSettings,
    #[serde(alias = "discord rich presence")]
    drp: RichPresenceSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            save: true,
            debug: false,
            channels: None,
            history: HistorySettings::default(),
            text: TextSettings::default(),
            drp: RichPresenceSettings::default(),
        }
    }
}

impl Settings {
    pub fn enabled_audio_channels(&self) -> HashSet<String> {
        self.channels
            .as_ref()
            .map(|map| {
                map.iter()
                    .filter(|(_, &enabled)| enabled)
                    .map(|(key, _)| key.clone())
                    .collect()
            })
            .unwrap_or(HashSet::new())
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct EntrypointSoundAction {
    name: String,
    channel: String,
    seek: Option<u64>,
    speed: Option<f64>,
}

impl Into<SoundAction> for EntrypointSoundAction {
    fn into(self) -> SoundAction {
        SoundAction {
            name: Some(self.name.into()),
            channel: self.channel.into(),
            mode: TemplatableValue::value(SoundActionMode::default()),
            seek: self.seek.map(TemplatableValue::value),
            speed: self.speed.map(TemplatableValue::value),
        }
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Entrypoint {
    pub path: PathData,
    pub background: Option<TextLines>,
    pub notes: Option<Notes>,
    pub variables: Option<Variables>,
    #[serde(rename = "info")]
    pub info_pages: Option<UnlockedInfoPages>,
    pub log: Option<Vec<String>>,
    sounds: Option<Vec<EntrypointSoundAction>>,
}

impl Entrypoint {
    pub fn submit_sounds(
        &self,
        player: &Player,
        stc: &StaticContext,
        text_context: &TextContext,
    ) -> Result<()> {
        if let Some(sounds) = self.sounds.clone() {
            let into: Vec<SoundAction> = sounds.into_iter().map(Into::into).collect();
            stc.resources.submit_audio(player, &into, text_context)?;
        }
        Ok(())
    }
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub metadata: Metadata,
    #[serde(default)]
    dependencies: Dependencies,
    #[serde(default, alias = "config")]
    pub settings: Settings,
    #[serde(alias = "entrypoint")]
    pub entry: Entrypoint,
}

impl Manifest {
    pub const FILE: &'static str = "nage.yml";

    pub fn load(loader: &Loader) -> Result<Self> {
        let config: Self = loader.load(Self::FILE, true)?;
        config
            .validate()
            .with_context(|| "Failed to validate manifest")?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        if self.settings.history.size == 0 {
            return Err(anyhow!("`settings.history.size` must be non-zero"));
        }
        let nage_version = Version::from_str(NAGE_VERSION)?;
        self.dependencies.check(nage_version)?;
        Ok(())
    }

    pub fn connect_rich_presence(&self) -> Option<RichPresence> {
        if !self.settings.drp.enabled {
            return None;
        }
        RichPresence::new()
    }

    pub fn rich_presence_state(
        &self,
        latest: &HistoryEntry,
        drp: Option<&TemplatableString>,
        log: Option<&str>,
        text_context: Option<&TextContext>,
    ) -> Result<Option<String>> {
        self.settings
            .drp
            .mode
            .get_state(latest, drp, log, text_context)
    }

    pub fn set_rich_presence(&self, drpc: &mut Option<RichPresence>, state: &str) -> Result<()> {
        if let Some(client) = drpc {
            client.set_state(&self.settings.drp, &self.metadata.name, state)?;
        }
        Ok(())
    }
}
