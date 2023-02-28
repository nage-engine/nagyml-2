use anyhow::Result;

use crate::{core::{player::Player, manifest::Manifest, text::{Text, TextContext}, choice::Choice, prompt::PromptModel}, input::{controller::{InputContext, InputController, InputResult}, commands::{RuntimeCommand, CommandResult}}};

use super::main::Resources;

pub enum GameLoopResult {
	Retry(bool),
	Continue,
	Shutdown(bool)
}

pub fn handle_quit(shutdown: bool) -> GameLoopResult {
	use GameLoopResult::*;
	if shutdown { 
		Shutdown(false)
	}
	else { 
		println!("Signal quit again or use '.quit' to exit");
		Retry(true)
	}
}

pub fn handle_choice(choice: &Choice, config: &Manifest, player: &mut Player, text_context: &TextContext) -> Result<GameLoopResult> {
	use GameLoopResult::*;
	player.choose(choice, None, config)?;
	if let Some(ending) = &choice.ending {
		println!();
		Text::print_lines(ending, text_context);
		return Ok(Shutdown(true));
	}
	Ok(Continue)
}

pub fn handle_command(parse: Result<RuntimeCommand>, config: &Manifest, player: &mut Player, resources: &Resources) -> Result<GameLoopResult> {
	match &parse {
		Err(err) => println!("\n{err}"), // Clap error
		Ok(command) => {
			match command.run(config, player, resources) {
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
	Ok(GameLoopResult::Retry(parse.is_ok()))
}

pub fn take_input(input: &mut InputController, context: &InputContext, config: &Manifest, player: &mut Player, resources: &Resources, text_context: &TextContext, choices: &Vec<&Choice>) -> Result<GameLoopResult> {
	use GameLoopResult::*;
	let result = match input.take(context) {
		Err(err) => {
			println!("{err}");
			Retry(true)
		},
		Ok(result) => match result {
			InputResult::Quit(shutdown) => handle_quit(shutdown),
			InputResult::Choice(i) => handle_choice(choices[i - 1], config, player, text_context)?,
			InputResult::Variable(result) => {
				// Modify variables after the choose call since history entries are sensitive to this order
				player.choose(choices[0], Some(&result), config)?;
				player.variables.insert(result.0.clone(), result.1.clone());
				Continue
			},
			InputResult::Command(parse) => handle_command(parse, config, player, resources)?
		}
	};
	Ok(result)
}

pub fn next_input_context(model: &PromptModel, choices: &Vec<&Choice>, text_context: &TextContext) -> Option<InputContext> {
	use PromptModel::*;
	match model {
		Response => Some(InputContext::Choices(choices.len())),
		&Input(name, prompt) => Some(InputContext::Variable(name.clone(), prompt.map(|s| s.fill(text_context)))),
		_ => None
	}
}