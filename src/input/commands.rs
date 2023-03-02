use anyhow::{Result, anyhow};
use clap::Parser;

use crate::{core::{player::Player, prompt::Prompt as PromptUtil, manifest::Manifest, text::{Translations, TextContext}, choice::Notes}, game::{gloop::GameLoopResult, main::{Resources, UnlockedInfoPages, InfoPages}}};

#[derive(Parser, Debug, PartialEq)]
#[command(multicall = true)]
pub enum RuntimeCommand {
	#[command(about = "Tries going back a choice")]
	Back,
	#[command(about = "Manages the display language")]
	Lang,
	#[command(about = "Displays an info page")]
	Info,
	#[command(about = "Displays an action log page")]
	Log,
	#[command(about = "Saves the player data")]
	Save,
	#[command(about = "Saves and quits the game")]
	Quit,
	#[command(about = "Displays debug info about a prompt", hide = true)]
	Prompt,
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

impl CommandResult {
	pub fn retry() -> CommandResult {
		Self::Submit(GameLoopResult::Retry(true))
	}
}

impl RuntimeCommand {
	/// Determines if this command is allowed in a default, non-debug environment.
	fn is_normal(&self) -> bool {
		use RuntimeCommand::*;
		match self {
			Back | Save | Quit | Lang | Info | Log => true,
			_ => false
		}
	}

	/// Handles a [`Back`](RuntimeCommand::Back) command.
	fn back(player: &mut Player) -> Result<CommandResult> {
		if player.history.len() <= 1 {
			return Err(anyhow!("Can't go back right now!"));
		}
		player.reverse_history()?;
		Ok(CommandResult::Submit(GameLoopResult::Continue))
	}

	/// Handles a [`Lang`](RuntimeCommand::Lang) command.
	fn lang(player: &mut Player, translations: &Translations) -> Result<CommandResult> {
		if translations.is_empty() {
			return Err(anyhow!("No display languages loaded"));
		}

		println!();

		let lang_question = requestty::Question::select("choose_lang")
    		.message("Select a language")
			.choices(translations.keys())
			.build();
		let lang_choice = requestty::prompt_one(lang_question)?;
		player.lang = lang_choice.as_list_item().unwrap().text.clone();

		Ok(CommandResult::retry())
	}

	/// Handles an [`Info`](RuntimeCommand::Info) command.
	fn info(unlocked_pages: &UnlockedInfoPages, pages: &InfoPages) -> Result<CommandResult> {
		if unlocked_pages.is_empty() {
			return Err(anyhow!("No info pages unlocked"))
		}

		println!();
		
		let info_question = requestty::Question::select("choose_info")
			.message("Select an info page")
			.choices(unlocked_pages)
			.build();
		let info_choice = requestty::prompt_one(info_question)?;

		println!();
		termimad::print_text(pages.get(&info_choice.as_list_item().unwrap().text).unwrap());

		Ok(CommandResult::retry())
	}

	/// Handles a [`Log`](RuntimeCommand::Log) command.
	fn log(log: &Vec<String>) -> Result<CommandResult> {
		if log.is_empty() {
			return Err(anyhow!("No log entries to display"))
		}

		println!();

		let pages: Vec<&[String]> = log.chunks(5).collect();
		let page_choices: Vec<String> = pages.iter()
			.map(|chunk| chunk[0][..25].to_owned())
			.map(|line| format!("{line}..."))
			.collect();
		let page_question = requestty::Question::raw_select("choose_page")
    		.message("Log page")
			.choices(page_choices)
			.build();
		let page_choice = requestty::prompt_one(page_question)?;

		let page_content = pages.get(page_choice.as_list_item().unwrap().index).unwrap();
		let entries = page_content.join("\n\n");
		Ok(CommandResult::Output(format!("\n{entries}")))
	}

	/// Handles a [`Prompt`](RuntimeCommand::Prompt) command.
	fn prompt(notes: &Notes, resources: &Resources, text_context: &TextContext) -> Result<CommandResult> {
		println!();

		let file_question = requestty::Question::select("choose_file")
			.message("Prompt file")
			.choices(resources.prompts.keys())
			.build();
		let file_choice = requestty::prompt_one(file_question)?;
		let file = &file_choice.as_list_item().unwrap().text;

		let prompt_question = requestty::Question::select("choose_prompt")
			.message(format!("Prompt in '{}'", file))
			.choices(PromptUtil::get_file(&resources.prompts, file)?.keys())
			.build();
		let prompt_choice = requestty::prompt_one(prompt_question)?;
		let prompt_name = &prompt_choice.as_list_item().unwrap().text;

		let prompt = PromptUtil::get(&resources.prompts, prompt_name, file)?;
		Ok(CommandResult::Output(prompt.debug_info(prompt_name, file, &resources.prompts, notes, text_context)?))
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
	pub fn run(&self, config: &Manifest, player: &mut Player, resources: &Resources, text_context: &TextContext) -> Result<CommandResult> {
		if !self.is_normal() && !config.settings.debug {
			return Err(anyhow!("Unable to access debug commands"));
		}
		use RuntimeCommand::*;
		use CommandResult::*;
		let result = match self {
			Back => Self::back(player)?,
			Lang => Self::lang(player, &resources.translations)?,
			Info => Self::info(&player.info_pages, &resources.info_pages)?,
			Log => Self::log(&player.log)?,
			Save => {
				player.save();
				Output("Saving... ".to_owned())
			}
			Quit => Submit(GameLoopResult::Shutdown(false)),
			Prompt => Self::prompt(&player.notes, resources, text_context)?,
			Notes => Self::notes(player)?,
			Variables => Self::variables(player)?
		};
		Ok(result)
	}
}