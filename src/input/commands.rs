use anyhow::{Result, anyhow};
use clap::Parser;
use itertools::Itertools;

use crate::{core::{player::Player, prompt::Prompt as PromptUtil, manifest::Manifest, text::Translations}, game::{gloop::GameLoopResult, main::Resources}};

#[derive(Parser, Debug, PartialEq)]
#[command(multicall = true)]
/// The commands used for the runtime REPL.
/// 
/// Debug commands are hidden from the `help` output.
pub enum RuntimeCommand {
	#[command(about = "Tries going back a choice")]
	Back,
	#[command(about = "Manages the display language")]
	Lang {
		#[arg(help = "The language code to switch to. If none, lists all loaded languages")]
		lang: Option<String>
	},
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
	Submit(GameLoopResult),
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

	/// Handles a [`Back`](RuntimeCommand::Back) command.
	fn back(player: &mut Player) -> Result<CommandResult> {
		if player.history.len() <= 1 {
			return Err(anyhow!("Can't go back right now!"));
		}
		player.reverse_history()?;
		Ok(CommandResult::Submit(GameLoopResult::Continue))
	}

	/// Handles a [`Lang`](RuntimeCommand::Lang) command.
	fn lang(lang: &Option<String>, player: &mut Player, translations: &Translations) -> Result<CommandResult> {
		use CommandResult::*;
		let result = match lang {
			Some(code) => {
				if !translations.contains_key(code) {
					return Err(anyhow!("Invalid display language"));
				}
				player.lang = code.clone();
				Output(format!("Set display language to '{code}'"))
			},
			None => {
				if translations.is_empty() {
					return Err(anyhow!("No display languages loaded"));
				}
				Output(translations.keys().join(", "))
			}
		};
		Ok(result)
	}

	/// Handles a [`Notes`](RuntimeCommand::Notes) command.
	fn notes(player: &Player) -> Result<CommandResult> {
		if player.notes.is_empty() {
			return Err(anyhow!("No notes applied"))
		}
		let result = itertools::join(&player.notes, ", ");
		Ok(CommandResult::Output(result))
	}
	
	/// Handles a [`Variables`](RuntimeCommand::Variables) command.
	fn variables(player: &Player) -> Result<CommandResult> {
		if player.variables.is_empty() {
			return Err(anyhow!("No variables applied"))
		}
		let vars = player.variables.clone().into_iter()
			.map(|(name, value)| format!("{name}: {value}"))
			.collect::<Vec<String>>()
			.join("\n");
		Ok(CommandResult::Output(format!("\n{vars}")))
	}

	/// Executes a runtime command if the player has permission to do so.
	///
	/// Any errors will be reported to the input loop with a retry following.
	pub fn run(&self, config: &Manifest, player: &mut Player, resources: &Resources) -> Result<CommandResult> {
		if !Self::DEFAULT_COMMANDS.contains(&self) && !config.settings.debug {
			return Err(anyhow!("Unable to access debug commands"));
		}
		use RuntimeCommand::*;
		use CommandResult::*;
		let result = match self {
			Back => Self::back(player)?,
			Lang { lang } => Self::lang(lang, player, &resources.translations)?,
			Save => {
				player.save();
				Output("Saving... ".to_owned())
			}
			Quit => Submit(GameLoopResult::Shutdown(false)),
			Files => Output(resources.prompts.keys().join(", ")),
			Prompts { file } => {
				let prompt_file = PromptUtil::get_file(&resources.prompts, file)?;
				Output(prompt_file.keys().join(", "))
			},
			Prompt { file, name } => {
				let prompt = PromptUtil::get(&resources.prompts, name, file)?;
				Output(prompt.debug_info(name, file, &resources.prompts, &player.notes))
			}
			Notes => Self::notes(player)?,
			Variables => Self::variables(player)?
		};
		Ok(result)
	}
}