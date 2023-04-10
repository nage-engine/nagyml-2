use std::{
    collections::{BTreeMap, HashMap},
    time::Duration,
};

use anyhow::{anyhow, Context as _, Result};
use playback_rs::{Player as AudioPlayer, Song};
use result::OptionResultExt;
use rlua::{Context, Table};
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

use crate::{
    loading::loader::Loader,
    text::templating::{TemplatableString, TemplatableValue},
};

use super::{context::TextContext, manifest::Manifest, player::Player};

/// A map of channel names to audio player instances and whether they are currently enabled.
pub type AudioPlayers = HashMap<String, AudioPlayer>;
/// A map of song names to decoded song content.
pub type Sounds = BTreeMap<String, Song>;

#[derive(Deserialize, Serialize, Display, Debug, Clone, EnumString, EnumIter)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
/// A [`SoundAction`] method type.
///
/// Modes that require specific sound files will return `true` from [`is_specific`](SoundActionMode::is_specific).
pub enum SoundActionMode {
    /// Queue a sound if the channel is already playing another sound.
    Queue,
    /// Immediately plays a sound on the channel regardless of whether it is already playing a sound.
    Overwrite,
    /// Plays a sound if and only if there is no sound already playing in a channel.
    Passive,
    /// Skips a sound if one is playing in a channel.
    Skip,
    /// Pauses a channel.
    Pause,
    /// Un-pauses a channel.
    Play,
}

impl Default for SoundActionMode {
    fn default() -> Self {
        Self::Passive
    }
}

impl SoundActionMode {
    /// Whether this action requires a specific sound file to be present.
    pub fn is_specific(&self) -> bool {
        use SoundActionMode::*;
        matches!(&self, Queue | Overwrite | Passive)
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A container allowing choices to control audio playback through the [`Audio`] resource.
/// Essentially a wrapper around [`playback_rs`] functionality.
pub struct SoundAction {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The sound file to submit.
    /// Only required for specific [`SoundActionMode`]s.
    pub name: Option<TemplatableString>,
    /// The channel to modify playback on.
    pub channel: TemplatableString,
    #[serde(default)]
    /// The method to apply to the sound channel.
    pub mode: TemplatableValue<SoundActionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The specific point in a sound to start from, in milliseconds.
    pub seek: Option<TemplatableValue<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The playback multiplier of the sound.
    pub speed: Option<TemplatableValue<f64>>,
}

/// A collection of ordered [`SoundAction`]s to be submitted in order.
pub type SoundActions = Vec<SoundAction>;

impl SoundAction {
    /// Validates a single [`SoundAction`] against the [`Audio`] resource.
    ///
    /// A sound action is valid if:
    /// - Its `name` key matches a loaded sound effect
    /// - Its `channel` key matches a created audio channel
    /// - The [specificity](SoundActionMode::is_specific) of its `mode` matches whether the sound effect is present
    pub fn validate(&self, audio: &Audio) -> Result<()> {
        if let Some(name) = &self.name {
            if let Some(sound) = name.content() {
                let _ = audio.get_sound(sound)?;
            }
        }
        if let Some(channel) = self.channel.content() {
            let _ = audio.get_player(channel)?;
        }
        if let Some(mode) = &self.mode.value {
            if mode.is_specific() && self.name.is_none() {
                return Err(anyhow!(
                    "Sound action '{mode}' requires a sound effect name, but none is provided"
                ));
            } else if !mode.is_specific() && self.name.is_some() {
                return Err(anyhow!(
                    "Sound action '{mode}' does not use a sound effect, but one is provided"
                ));
            }
        }
        Ok(())
    }

    /// Validates a list of [`SoundActions`] in order using [`SoundAction::validate`].
    pub fn validate_all(sounds: &SoundActions, audio: &Audio) -> Result<()> {
        for (index, sound) in sounds.iter().enumerate() {
            let _ = sound
                .validate(audio)
                .with_context(|| format!("Failed to validate sound action #{}", index + 1))?;
        }
        Ok(())
    }
}

/// A container for [`AudioPlayers`] and [`Sounds`].
///
/// A pair of a channel and an audio player corresponds to a single connection to a sound device,
/// wherein one sound file can be playing at a time. Overlapping sounds requires multiple connections
/// and playing on different channels.
///
/// Channels are only created on startup. They are never dynamically loaded and must
/// be specified in the manifest file prior to runtime.
pub struct Audio {
    pub players: AudioPlayers,
    sounds: Sounds,
}

impl Audio {
    /// Creates [`AudioPlayers`]s and maps them to the config settings' `channels`.
    fn load_players(config: &Manifest) -> Option<Result<AudioPlayers>> {
        config.settings.channels.as_ref().map(|channels| {
            channels
                .iter()
                .map(|(channel, _)| {
                    AudioPlayer::new(None)
                        .map(|player| (channel.clone(), player))
                        .map_err(|err| anyhow!(err))
                })
                .try_collect()
        })
    }

    /// Loads an [`Audio`] container.
    ///
    /// If [`AudioPlayer`] creation using [`load_players`](Self::load_players) fails, it fails silently
    /// and brings the down the whole audio system with it, signaling [None] within the wrapped option.
    ///
    /// An [`Err`] is only returned if [`load_sounds`](Self::load_sounds) errors.
    pub fn load(loader: &Loader, config: &Manifest) -> Result<Option<Self>> {
        Self::load_players(config)
            .map(|result| {
                result.ok().map(|players| {
                    loader
                        .load_sounds("sounds")
                        .map(|sounds| Self { players, sounds })
                })
            })
            .flatten()
            .invert()
    }

    /// Retrieves an [`AudioPlayer`], if any, by a channel name.
    pub fn get_player(&self, channel: &str) -> Result<&AudioPlayer> {
        self.players
            .get(channel)
            .ok_or(anyhow!("Invalid sound channel '{channel}'"))
    }

    /// Retrieves a [`Song`], if any, by a sound name.
    pub fn get_sound(&self, name: &str) -> Result<&Song> {
        self.sounds
            .get(name)
            .ok_or(anyhow!("Invalid sound file '{name}'"))
    }

    /// Returns this controller's channel names mapped to whether they are enabled on the [`Player`].
    pub fn channel_statuses(&self, player: &Player) -> Vec<(String, bool)> {
        self.players
            .keys()
            .map(|channel| (channel.clone(), player.channels.contains(channel)))
            .collect()
    }

    /// Creates a Lua table mapping each loaded audio player to a table of their data.
    ///
    /// This table is formatted as follows:
    /// - `is_playing`: Whether the player is not paused
    /// - `has_sound`: Whether the player has a sound currently playing
    /// - `has_sound_queued`: Whether the player has a sound queued, but not playing
    /// - `position`: If the player has a sound playing, returns the position in milliseconds
    /// - `sound_duration`: If the player has a sound playing, returns its duration in milliseconds
    pub fn create_audio_table<'a>(&self, context: &Context<'a>) -> Result<Table<'a>, rlua::Error> {
        let table = context.create_table()?;
        for (channel, player) in &self.players {
            let channel_table = context.create_table()?;
            channel_table.set("is_playing", player.is_playing())?;
            channel_table.set("has_sound", player.has_current_song())?;
            channel_table.set("has_sound_queued", player.has_next_song())?;
            if let Some((pos, duration)) = player.get_playback_position() {
                channel_table.set("position", pos.as_millis())?;
                channel_table.set("sound_duration", duration.as_millis())?;
            }
            table.set(channel.clone(), channel_table)?;
        }
        Ok(table)
    }

    /// Applies actions requiring that a specified sound file is **not** present.
    fn accept_general(player: &AudioPlayer, seek: Option<Duration>, mode: SoundActionMode) {
        use SoundActionMode::*;
        if let Some(duration) = seek {
            player.seek(duration);
        }
        match mode {
            Skip => player.skip(),
            Play => player.set_playing(true),
            Pause => player.set_playing(false),
            _ => (),
        }
    }

    /// Applies actions requiring both a [`SoundActionMode`] and accompanying sound effect.
    fn accept_specific(
        player: &AudioPlayer,
        sfx: &Song,
        seek: Option<Duration>,
        mode: SoundActionMode,
    ) {
        use SoundActionMode::*;
        let _ = match mode {
            Queue => player.play_song_next(sfx, seek),
            Overwrite => player.play_song_now(sfx, seek),
            Passive => {
                if !player.has_current_song() {
                    player.play_song_now(sfx, seek)
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        };
    }

    /// Applies a [`SoundAction`] to a particular channel.
    pub fn accept(
        &self,
        player: &Player,
        action: &SoundAction,
        text_context: &TextContext,
    ) -> Result<()> {
        let channel = action.channel.fill(text_context)?;
        let audio_player = self.get_player(&channel)?;

        if !player.channels.contains(&channel) {
            return Ok(());
        }

        let seek = action
            .seek
            .as_ref()
            .map(|ms| {
                ms.get_value(text_context)
                    .map(|amt| Duration::from_millis(amt))
            })
            .invert()?;

        let mode = action.mode.get_value(text_context)?;

        match &action.name {
            None => Self::accept_general(audio_player, seek, mode),
            Some(name) => {
                let sound = name.fill(text_context)?;
                let sfx = self.get_sound(&sound)?;
                Self::accept_specific(audio_player, sfx, seek, mode);
            }
        }

        if let Some(speed) = &action.speed {
            audio_player.set_playback_speed(speed.get_value(text_context)?);
        }

        Ok(())
    }
}
