use std::collections::HashMap;

use anyhow::{Result, anyhow};
use playback_rs::{Player as AudioPlayer, Song};
use result::OptionResultExt;

use crate::loading::get_content_iterator;

use super::{manifest::Manifest, choice::{SoundAction, SoundActionMode}, text::TextContext};

pub type AudioPlayers = HashMap<String, AudioPlayer>;
pub type Sounds = HashMap<String, Song>;

pub struct Audio {
	pub players: AudioPlayers,
	pub sounds: Sounds
}

impl Audio {
	fn load_players(config: &Manifest) -> Option<Result<AudioPlayers>> {
		config.settings.channels.as_ref().map(|channels| {
			channels.iter()
    			.map(|channel| {
					AudioPlayer::new(None)
						.map(|player| (channel.clone(), player))
    					.map_err(|err| anyhow!(err))
				})
        		.try_collect()
		})
	}

	fn load_sounds() -> Result<Sounds> {
		get_content_iterator("sounds")
    		.map(|(key, path)| {
				Song::from_file(path, None)
					.map(|song| (key, song))
					.map_err(|err| anyhow!(err))
			})
    		.collect()
	}

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

	pub fn accept(&self, action: &SoundAction, text_context: &TextContext) -> Result<()> {
		use SoundActionMode::*;
		let channel = action.channel.fill(text_context)?;
		let player = self.players.get(&channel)
    		.ok_or(anyhow!("Invalid sound channel '{channel}'"))?;
		let sound = action.name.fill(text_context)?;
		let sfx = self.sounds.get(&sound)
			.ok_or(anyhow!("Invalid sound file '{sound}'"))?;
		let _ = match action.mode {
			Queue => player.play_song_next(sfx, None),
			Overwrite => player.play_song_now(sfx, None),
			Passive => { 
				if !player.has_current_song() {
					player.play_song_now(sfx, None)
				}
				else {
					Ok(())
				}
			}
		};
		Ok(())
	}
}