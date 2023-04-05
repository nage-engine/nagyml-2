use std::time::{self, SystemTime};

use anyhow::Result;
use discord_rich_presence::{
    activity::{Activity, Assets, Timestamps},
    DiscordIpc, DiscordIpcClient,
};
use result::OptionResultExt;
use serde::Deserialize;

use crate::text::templating::TemplatableString;

use super::{context::TextContext, manifest::RichPresenceSettings, player::HistoryEntry};

#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum RichPresenceMode {
    Id,
    Custom { fallback: bool },
}

impl RichPresenceMode {
    /// Determines the next rich presence status for the game.
    ///
    /// If the mode is [`Id`](RichPresenceMode::Id), returns the prompt ID of the latest entry destination.
    /// This latest entry should have just been applied.
    ///
    /// If the mode is [`Custom`](RichPresenceMode::Custom), attempts to use the `drp` key on a choice.
    /// If no such value is present and `fallback` is set to `true`, again attempts to use the `log` key on the same choice.
    pub fn get_state(
        &self,
        latest: &HistoryEntry,
        drp: Option<&TemplatableString>,
        log: Option<&str>,
        text_context: Option<&TextContext>,
    ) -> Result<Option<String>> {
        use RichPresenceMode::*;
        let result = match self {
            Id => Some(latest.path.to_string()),
            &Custom { fallback } => drp
                .as_ref()
                .map(|drp| drp.fill(text_context.as_ref().unwrap()))
                .invert()?
                .and_then(|_| {
                    if fallback {
                        log.map(str::to_owned)
                    } else {
                        None
                    }
                }),
        };
        Ok(result)
    }
}

pub struct RichPresence {
    start: i64,
    client: DiscordIpcClient,
}

impl RichPresence {
    pub fn new() -> Option<Self> {
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

    fn assets<'a>(settings: &'a RichPresenceSettings, game_name: &'a str) -> Assets<'a> {
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

    pub fn set_state(
        &mut self,
        settings: &RichPresenceSettings,
        game_name: &str,
        state: &str,
    ) -> Result<()> {
        let details = Self::details(settings, game_name);
        let _ = self.client.set_activity(Self::activity(
            Self::assets(settings, game_name),
            self.start,
            &details,
            &state,
        ));
        Ok(())
    }
}
