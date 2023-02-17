use std::fmt::Display;

use anyhow::Result;

use crate::{input::{InputController, InputResult}, core::choice::Choice};

use super::{player::Player, config::NageConfig, prompt::{Prompt, Prompts}, text::{Text, TextSpeed}};

#[derive(Debug)]
pub struct Game {
	pub config: NageConfig,
	pub player: Player,
	prompts: Prompts,
	input: InputController
}

impl Game {
	pub fn load() -> Result<Self> {
		let config = NageConfig::load()?;
		let player = Player::load(&config.entry)?;
		let prompts = Prompt::load_all()?;
		let input = InputController::new()?;
		Ok(Self { config, player, prompts, input })
	}

	pub fn validate(&self) -> Result<()> {
		for (file_name, prompt_file) in &self.prompts {
			for (name, prompt) in prompt_file {
				let _ = prompt.validate(name, file_name, &self.prompts)?;
			}
		}
		Ok(())
	}

	pub fn speed(&self) -> &TextSpeed {
		&self.config.settings.speed
	}

	pub fn init(&mut self) {
		self.speed().print_nl(&self.config.metadata);
		if let Some(background) = &self.config.entry.background {
			Text::print_lines_nl(background, &self.player.variables, &self.config);
		}
		self.player.began = true;
	}

	pub fn handle_quit(&self, shutdown: bool) {
		if shutdown { self.shutdown() }
		else { println!("Signal quit again or use `.quit` to exit") }
	}

	pub fn begin(&mut self) -> Result<()> {
		if !self.player.began {
			self.init();
		}
		loop {
			let entry = self.player.latest_entry()?;
			let next_prompt = Prompt::get_from_path(&self.prompts, &entry.path)?;
			let model = next_prompt.model();
			next_prompt.print(model, &self.player.variables, &self.config);
			
			loop {
				match self.input.take(next_prompt.choices.len()) {
					Err(err) => println!("{err}"),
					Ok(result) => match result {
						InputResult::Quit(shutdown) => self.handle_quit(shutdown),
						InputResult::Choice(choice) => {
							self.player.push_history(&next_prompt.choices[choice - 1])?;
							break;
						}
					}
				}
				println!();
			}
		}
	}

	pub fn shutdown(&self) {
		if self.config.settings.save {
			self.player.save();
		}
		std::process::exit(0);
	}
}