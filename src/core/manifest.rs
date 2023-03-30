use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
    time::{self, SystemTime},
};

use anyhow::{anyhow, Context, Result};
use discord_rich_presence::{
    activity::{Activity, Assets, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use result::OptionResultExt;
use semver::{Version, VersionReq};
use serde::Deserialize;

use crate::{
    loading::loader::Loader,
    text::{
        context::TextContext,
        display::{TextLines, TextSpeed},
        templating::{TemplatableString, TemplatableValue},
    },
    NAGE_VERSION,
};

use super::{
    choice::{Notes, SoundAction, SoundActionMode, Variables},
    path::PathData,
    player::HistoryEntry,
    resources::UnlockedInfoPages,
};

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
/// A collection of settings that identify information about the game itself and its authors.
pub struct Metadata {
    pub name: String,
    id: Option<String>,
    pub authors: Vec<String>,
    pub version: Version,
    contact: Option<Vec<String>>,
}

impl Metadata {
    pub fn game_id(&self) -> &str {
        self.id.as_ref().unwrap_or(&self.name)
    }

    pub fn contact(&self) -> Option<String> {
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
    pub fn check(&self, nage_version: Version) -> Result<()> {
        self.nage
            .as_ref()
            .map(|nage| {
                if !nage.matches(&nage_version) {
                    Err(anyhow!(
					"dependency `nage` does not match required version (required: {}, provided: {})", 
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
    pub locked: bool,
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
#[serde(rename_all = "snake_case")]
pub enum RichPresenceMode {
    Id,
    Custom { fallback: bool },
}

impl RichPresenceMode {
    pub fn get_state(
        &self,
        latest: &HistoryEntry,
        drp: &Option<TemplatableString>,
        log: Option<String>,
        text_context: &Option<TextContext>,
    ) -> Result<Option<String>> {
        use RichPresenceMode::*;
        let result = match self {
            Id => Some(latest.path.to_string()),
            &Custom { fallback } => drp
                .as_ref()
                .map(|drp| drp.fill(text_context.as_ref().unwrap()))
                .invert()?
                .and_then(|_| if fallback { log } else { None }),
        };
        Ok(result)
    }
}

#[derive(Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct RichPresenceSettings {
    enabled: bool,
    icon: Option<String>,
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

pub struct RichPresence {
    start: i64,
    client: DiscordIpcClient,
}

impl RichPresence {
    fn new() -> Option<Self> {
        DiscordIpcClient::new(RichPresenceSettings::APP_ID)
            .ok()
            .and_then(|mut client| {
                client.connect().ok()?;
                let now = SystemTime::now();
                let since = now
                    .duration_since(time::UNIX_EPOCH)
                    .expect("Time went backwards...");
                Some(Self {
                    start: since.as_secs() as i64,
                    client,
                })
            })
    }

    fn icon<'a>(settings: &'a RichPresenceSettings, game_name: &'a str) -> Assets<'a> {
        let assets = Assets::new();
        match &settings.icon {
            Some(url) => assets
                .large_image(url)
                .large_text(game_name)
                .small_image("icon")
                .small_text("nage"),
            None => assets.large_image("icon").large_text("nage"),
        }
    }

    fn details(settings: &RichPresenceSettings, game_name: &str) -> String {
        match &settings.icon {
            Some(_) => "Playing a Nagame".to_owned(),
            None => format!("Playing \"{game_name}\""),
        }
    }

    fn activity<'a>(
        assets: Assets<'a>,
        start: i64,
        details: &'a str,
        state: &'a str,
    ) -> Activity<'a> {
        Activity::new()
            .assets(assets)
            .timestamps(Timestamps::new().start(start))
            .details(details)
            .state(state)
    }

    fn set_state(
        &mut self,
        settings: &RichPresenceSettings,
        game_name: &str,
        state: &str,
    ) -> Result<()> {
        let details = Self::details(settings, game_name);
        let _ = self.client.set_activity(Self::activity(
            Self::icon(settings, game_name),
            self.start,
            &details,
            &state,
        ));
        Ok(())
    }
}

#[derive(Deserialize, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub save: bool,
    pub debug: bool,
    pub speed: TextSpeed,
    pub wait: Option<u64>,
    pub lang: Option<String>,
    pub channels: Option<HashMap<String, bool>>,
    pub history: HistorySettings,
    #[serde(alias = "discord rich presence")]
    pub drp: RichPresenceSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            save: true,
            debug: false,
            speed: TextSpeed::Delay(TemplatableValue::value(5)),
            wait: None,
            lang: None,
            channels: None,
            history: HistorySettings::default(),
            drp: RichPresenceSettings::default(),
        }
    }
}

impl Settings {
    pub fn enabled_channels(&self) -> HashSet<String> {
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
    pub sounds: Option<Vec<EntrypointSoundAction>>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub metadata: Metadata,
    #[serde(default)]
    pub dependencies: Dependencies,
    #[serde(default)]
    pub settings: Settings,
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

    pub fn set_rich_presence(&self, drpc: &mut Option<RichPresence>, state: &str) -> Result<()> {
        if let Some(client) = drpc {
            client.set_state(&self.settings.drp, &self.metadata.name, state)?;
        }
        Ok(())
    }
}
