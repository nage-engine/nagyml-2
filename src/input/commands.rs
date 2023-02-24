use anyhow::{Result, anyhow};
use clap::Parser;
use itertools::Itertools;

use crate::core::{player::Player, prompt::{Prompts, Prompt as PromptUtil}, game::InputLoopResult};

#[derive(Parser, Debug)]
pub enum RuntimeCommand {
	#[command(about = "Tries going back a choice")]
	Back,
	#[command(about = "Saves the player data")]
	Save,
	#[command(about = "Saves and quits the game")]
	Quit,
	#[command(about = "Lists the loaded prompt files")]
	Files,
	#[command(about = "Lists the loaded prompts in a file")]
	Prompts { file: String },
	#[command(about = "Display debug info about a prompt")]
	Prompt { file: String, name: String },
	#[command(about = "Lists the currently applied notes")]
	Notes,
	#[command(about = "Lists the currently applied variable names and their values")]
	Variables,
}

pub enum CommandResult {
	Submit(InputLoopResult),
	Output(String)
}

impl RuntimeCommand {
	pub fn run(&self, prompts: &Prompts, player: &mut Player) -> Result<CommandResult> {
		use RuntimeCommand::*;
		use CommandResult::*;
		let result = match self {
			Back => {
				if player.history.len() <= 1 {
					return Err(anyhow!("Can't go back right now!"));
				}
				player.reverse_history()?;
				Submit(InputLoopResult::Continue)
			}
			Save => {
				player.save();
				Output("Saving... ".to_owned())
			}
			Quit => Submit(InputLoopResult::Shutdown(false)),
			Files => Output(prompts.keys().join(", ")),
			Prompts { file } => {
				let prompt_file = PromptUtil::get_file(prompts, file)?;
				Output(prompt_file.keys().join(", "))
			},
			Prompt { file, name } => {
				let prompt = PromptUtil::get(prompts, name, file)?;
				Output(prompt.debug_info(name, file, prompts, &player.notes))
			}
			Notes => {
				if player.notes.is_empty() {
					return Err(anyhow!("No notes applied"))
				}
				Output(itertools::join(&player.notes, ", "))
			},
			Variables => {
				if player.variables.is_empty() {
					return Err(anyhow!("No variables applied"))
				}
				let vars = player.variables.clone().into_iter()
					.map(|(name, value)| format!("{name}: {value}"))
					.collect::<Vec<String>>()
					.join("\n");
				Output(format!("\n{vars}"))
			},
		};
		Ok(result)
	}
}