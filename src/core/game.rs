use anyhow::{Result, anyhow};

use crate::{input::{InputController, InputResult, InputContext}, core::choice::Choice};

use super::{player::Player, manifest::Manifest, prompt::{Prompt, Prompts, PromptModel}, text::{Text, TextSpeed}};

#[derive(Debug)]
pub struct Game {
	pub config: Manifest,
	pub player: Player,
	prompts: Prompts,
	input: InputController
}

pub enum InputLoopResult {
	Retry,
	Continue,
	Shutdown(bool)
}

impl Game {
	pub fn load() -> Result<Self> {
		let config = Manifest::load()?;
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

	pub fn handle_quit(shutdown: bool) -> InputLoopResult {
		use InputLoopResult::*;
		if shutdown { 
			Shutdown(false)
		}
		else { 
			println!("Signal quit again or use `.quit` to exit");
			Retry
		}
	}

	pub fn handle_choice(player: &mut Player, config: &Manifest, choice: &Choice) -> Result<InputLoopResult> {
		use InputLoopResult::*;
		player.choose(choice)?;
		if let Some(ending) = &choice.ending {
			Text::print_lines(ending, &player.variables, &config);
			return Ok(Shutdown(true));
		}
		Ok(Continue)
	}

	//pub fn handle_variable(player: )

	pub fn next_input_context(model: &PromptModel, choices: &Vec<&Choice>) -> Option<InputContext> {
		use PromptModel::*;
		match model {
			Response => Some(InputContext::Choices(choices.len())),
			&Input(name, prompt) => Some(InputContext::Variable(name.clone(), prompt.map(|s| s.clone()))),
			_ => None
		}
	}

	pub fn take_input(input: &mut InputController, player: &mut Player, config: &Manifest, choices: &Vec<&Choice>, context: &InputContext) -> Result<InputLoopResult> {
		use InputLoopResult::*;
		let result = match input.take(context) {
			Err(err) => {
				println!("{err}");
				Retry
			},
			Ok(result) => match result {
				InputResult::Quit(shutdown) => Self::handle_quit(shutdown),
				InputResult::Choice(i) => Self::handle_choice(player, config, choices[i - 1])?,
				InputResult::Variable(name, input) => {
					player.variables.insert(name, input);
					player.choose(choices[0])?;
					Continue
				}
			}
		};
		Ok(result)
	}

	pub fn begin(&mut self) -> Result<bool> {
		if !self.player.began {
			self.init();
		}
		let silent = 'outer: loop {
			let entry = self.player.latest_entry()?;
			let next_prompt = Prompt::get_from_path(&self.prompts, &entry.path)?;
			let model = next_prompt.model();
			let choices = next_prompt.usable_choices(&self.player.notes);

			next_prompt.print(&model, entry.display, &choices, &self.player.variables, &self.config);

			match model {
				PromptModel::Redirect(choice) => self.player.choose(choice)?,
				PromptModel::Ending(lines) => {
					Text::print_lines(lines, &self.player.variables, &self.config);
					break 'outer true
				},
				_ => loop {
					let context = Self::next_input_context(&model, &choices)
        				.ok_or(anyhow!("Could not resolve input context"))?;
					// Borrow-checker coercion; only using necessary fields in static method
					match Self::take_input(&mut self.input, &mut self.player, &self.config, &choices, &context)? {
						InputLoopResult::Retry => println!(),
						InputLoopResult::Continue => { println!(); break },
						InputLoopResult::Shutdown(silent) => break 'outer silent
					}
				}
			}
		};
		Ok(silent)
	}

	pub fn shutdown(&self, silent: bool) {
		if self.config.settings.save {
			self.player.save();
		}
		if !silent {
			println!("Exiting...");
		}
	}
}