use std::{collections::HashMap, time::Duration};

use anyhow::{Result, anyhow};
use playback_rs::{Player as AudioPlayer, Song};
use result::OptionResultExt;
use rlua::{Context, Table};

use crate::loading::get_content_iterator;

use super::{manifest::Manifest, choice::{SoundAction, SoundActionMode}, text::TextContext};

/// A map of channel names to audio player instances and whether they are currently enabled.
pub type AudioPlayers = HashMap<String, AudioPlayer>;
/// A map of song names to decoded song content.
pub type Sounds = HashMap<String, Song>;

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
	pub sounds: Sounds
}

impl Audio {
	/// Creates [`AudioPlayers`]s and maps them to the config settings' `channels`.
	fn load_players(config: &Manifest) -> Option<Result<AudioPlayers>> {
		config.settings.channels.as_ref().map(|channels| {
			channels.iter()
    			.map(|(channel, _)| {
					AudioPlayer::new(None)
						.map(|player| (channel.clone(), player))
    					.map_err(|err| anyhow!(err))
				})
        		.try_collect()
		})
	}

	/// Loads and parses [`Sounds`] from the `sounds` directory.
	fn load_sounds() -> Result<Sounds> {
		get_content_iterator("sounds")
    		.map(|(key, path)| {
				Song::from_file(path, None)
					.map(|song| (key, song))
					.map_err(|err| anyhow!(err))
			})
    		.collect()
	}

	/// Loads an [`Audio`] container.
	/// 
	/// If [`AudioPlayer`] creation using [`load_players`](Self::load_players) fails, it fails silently
	/// and brings the down the whole audio system with it, signaling [None] within the wrapped option.
	/// 
	/// An [`Err`] is only returned if [`load_sounds`](Self::load_sounds) errors.
	pub fn load(config: &Manifest) -> Result<Option<Self>> {
		Self::load_players(config).map(|result| {
			result.ok().map(|players| {
				Self::load_sounds().map(|sounds| {
					Self { players, sounds }
				})
			})
		})
		.flatten()
		.invert()
	}

	/// Retrieves an [`AudioPlayer`], if any, by a channel name.
	pub fn get_player(&self, channel: &str) -> Result<&AudioPlayer> {
		self.players.get(channel)
    		.ok_or(anyhow!("Invalid sound channel '{channel}'"))
	}

	pub fn create_audio_table<'a>(&self, context: &Context<'a>) -> Result<Table<'a>, rlua::Error> {
		let table = context.create_table()?;
		for (channel, player) in &self.players {
			let channel_table = context.create_table()?;
			channel_table.set("is_playing", player.is_playing())?;
			channel_table.set("has_sound", player.has_current_song())?;
			channel_table.set("has_sound_queued", player.has_next_song())?;
			if let Some((pos, duration)) = player.get_playback_position() {
				channel_table.set("position", pos.as_millis())?;
				channel_table.set("song_duration", duration.as_millis())?;
			}
			table.set(channel.clone(), channel_table)?;
		}
		Ok(table)
	}

	/// Applies actions requiring that a specified sound file is **not** present.
	fn accept_general_actions(player: &AudioPlayer, seek: Option<Duration>, mode: SoundActionMode) {
		use SoundActionMode::*;
		if let Some(duration) = seek {
			player.seek(duration);
		}
		match mode {
			Skip => player.skip(),
			Playing(is_playing) => player.set_playing(is_playing),
			_ => ()
		}
	}

	/// Applies actions requiring both a [`SoundActionMode`] and accompanying sound effect.
	fn accept_mode(player: &AudioPlayer, sfx: &Song, seek: Option<Duration>, mode: SoundActionMode) {
		use SoundActionMode::*;
		let _ = match mode {
			Queue => player.play_song_next(sfx, seek),
			Overwrite => player.play_song_now(sfx, seek),
			Passive => { 
				if !player.has_current_song() {
					player.play_song_now(sfx, seek)
				}
				else {
					Ok(())
				}
			},
			_ => Ok(())
		};
	}

	/// Applies a [`SoundAction`] to a particular channel.
	pub fn accept(&self, action: &SoundAction, text_context: &TextContext) -> Result<()> {
		let channel = action.channel.fill(text_context)?;
		let player = self.get_player(&channel)?;

		let seek = action.seek.as_ref().map(|ms| {
			ms.get_value(text_context).map(|amt| Duration::from_millis(amt))
		}).invert()?;
		
		let mode = action.mode.get_value(text_context)?;

		match &action.name {
			None => Self::accept_general_actions(player, seek, mode),
			Some(name) => {
				let sound = name.fill(text_context)?;
				let sfx = self.sounds.get(&sound)
					.ok_or(anyhow!("Invalid sound file '{sound}'"))?;
				Self::accept_mode(player, sfx, seek, mode);
			}
		}

		if let Some(speed) = &action.speed {
			player.set_playback_speed(speed.get_value(text_context)?);
		}

		Ok(())
	}
}