use anyhow::{Result, anyhow};

use crate::{input::{controller::{InputController, InputResult, InputContext}, commands::CommandResult}, core::choice::Choice, loading::load_content};

use super::{player::Player, manifest::Manifest, prompt::{Prompt, Prompts, PromptModel}, text::{Text, TextSpeed, Translations, TranslationFile}};

#[derive(Debug)]
pub struct Game {
	pub config: Manifest,
	pub player: Player,
	prompts: Prompts,
	translations: Translations,
	input: InputController
}

pub enum InputLoopResult {
	Retry(bool),
	Continue,
	Shutdown(bool)
}

impl Game {
	pub fn load() -> Result<Self> {
		let config = Manifest::load()?;
		let player = Player::load(&config)?;
		let prompts = load_content("prompts")?;
		let translations = load_content("lang")?;
		let input = InputController::new()?;
		Ok(Self { config, player, prompts, translations, input })
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
			let lang_file = Text::lang_file(&self.translations, &self.player.lang);
			Text::print_lines_nl(background, &self.player.variables, &self.config, lang_file);
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
			Retry(true)
		}
	}

	pub fn handle_choice(player: &mut Player, config: &Manifest, lang_file: Option<&TranslationFile>, choice: &Choice) -> Result<InputLoopResult> {
		use InputLoopResult::*;
		player.choose(choice, None, config)?;
		if let Some(ending) = &choice.ending {
			println!();
			Text::print_lines(ending, &player.variables, &config, lang_file);
			return Ok(Shutdown(true));
		}
		Ok(Continue)
	}

	pub fn next_input_context(model: &PromptModel, choices: &Vec<&Choice>) -> Option<InputContext> {
		use PromptModel::*;
		match model {
			Response => Some(InputContext::Choices(choices.len())),
			&Input(name, prompt) => Some(InputContext::Variable(name.clone(), prompt.map(|s| s.clone()))),
			_ => None
		}
	}

	pub fn take_input(input: &mut InputController, prompts: &Prompts, player: &mut Player, config: &Manifest, translations: &Translations, lang_file: Option<&TranslationFile>, choices: &Vec<&Choice>, context: &InputContext) -> Result<InputLoopResult> {
		use InputLoopResult::*;
		let result = match input.take(context) {
			Err(err) => {
				println!("{err}");
				Retry(true)
			},
			Ok(result) => match result {
				InputResult::Quit(shutdown) => Self::handle_quit(shutdown),
				InputResult::Choice(i) => Self::handle_choice(player, config, lang_file, choices[i - 1])?,
				InputResult::Variable(result) => {
					// Modify variables after the choose call since history entries are sensitive to this order
					player.choose(choices[0], Some(&result), config)?;
					player.variables.insert(result.0.clone(), result.1.clone());
					Continue
				},
				InputResult::Command(parse) => {
					match &parse {
						Err(err) => println!("\n{err}"), // Clap error
						Ok(command) => {
							match command.run(prompts, player, config, translations) {
								Err(err) => println!("Error: {err}"), // Command runtime error
								Ok(result) => {
									match result {
										CommandResult::Submit(loop_result) => return Ok(loop_result),
										CommandResult::Output(output) => println!("{output}")
									}
								}
							}
						}
					};
					Retry(parse.is_ok())
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
			let lang_file = Text::lang_file(&self.translations, &self.player.lang);
			let entry = self.player.latest_entry()?;
			let next_prompt = Prompt::get_from_path(&self.prompts, &entry.path)?;
			let model = next_prompt.model();
			let choices = next_prompt.usable_choices(&self.player.notes);

			if choices.is_empty() {
				return Err(anyhow!("No usable choices"))
			}
			
			next_prompt.print(&model, entry.display, &choices, &self.player.variables, &self.config, lang_file);

			match model {
				PromptModel::Redirect(choice) => self.player.choose(choice, None, &self.config)?,
				PromptModel::Ending(lines) => {
					println!();
					Text::print_lines(lines, &self.player.variables, &self.config, lang_file);
					break 'outer true
				},
				_ => loop {
					let context = Self::next_input_context(&model, &choices)
        				.ok_or(anyhow!("Could not resolve input context"))?;
					// Borrow-checker coercion; only using necessary fields in static method
					match Self::take_input(&mut self.input, &self.prompts, &mut self.player, &self.config, &self.translations, lang_file, &choices, &context)? {
						InputLoopResult::Retry(flush) => if flush { println!() },
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

	pub fn crash_context(&self) -> String {
		let contact = self.config.metadata.contact.as_ref().map(|info| {
			let strings: Vec<String> = info.iter()
    			.map(|value| format!("- {value}"))
				.collect();
			format!("\n\nContact the developers:\n{}", strings.join("\n"))
		});
		format!("The game has crashed; it's not your fault!{}", contact.unwrap_or(String::new()))
	}
}