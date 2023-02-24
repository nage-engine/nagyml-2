use anyhow::{Result, anyhow};
use clap::Parser;
use itertools::Itertools;

use crate::core::{player::Player, prompt::{Prompts, Prompt as PromptUtil}, game::InputLoopResult, manifest::Manifest};

#[derive(Parser, Debug, PartialEq)]
#[command(multicall = true)]
/// The commands used for the runtime REPL.
/// 
/// Debug commands are hidden from the `help` output.
pub enum RuntimeCommand {
	#[command(about = "Tries going back a choice")]
	Back,
	#[command(about = "Saves the player data")]
	Save,
	#[command(about = "Saves and quits the game")]
	Quit,
	#[command(about = "Lists the loaded prompt files", hide = true)]
	Files,
	#[command(about = "Lists the loaded prompts in a file", hide = true)]
	Prompts { 
		#[arg(help = "The keyed prompt file")]
		file: String 
	},
	#[command(about = "Display debug info about a prompt", hide = true)]
	Prompt { 
		#[arg(help = "The keyed prompt file")]
		file: String, 
		#[arg(help = "The prompt name")]
		name: String 
	},
	#[command(about = "Lists the currently applied notes", hide = true)]
	Notes,
	#[command(about = "Lists the currently applied variable names and their values", hide = true)]
	Variables,
}

/// The result of a runtime command.
pub enum CommandResult {
	/// Returns an input loop result to the original input call.
	Submit(InputLoopResult),
	/// Outputs a specified string and submits [`Retry`](InputLoopResult::Retry).
	Output(String)
}

impl RuntimeCommand {
	/// The commands allowed in a default, non-debug environment.
	pub const DEFAULT_COMMANDS: [RuntimeCommand; 3] = [
		RuntimeCommand::Back, 
		RuntimeCommand::Save, 
		RuntimeCommand::Quit
	];

	/// Executes a runtime command if the player has permission to do so.
	///
	/// Any errors will be reported to the input loop with a retry following.
	pub fn run(&self, prompts: &Prompts, player: &mut Player, config: &Manifest) -> Result<CommandResult> {
		if !Self::DEFAULT_COMMANDS.contains(&self) && !config.settings.debug {
			return Err(anyhow!("Unable to access debug commands"));
		}
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