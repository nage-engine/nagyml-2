use anyhow::{Result, anyhow};
use clap::Parser;
use rustyline::DefaultEditor;

use crate::{core::{player::VariableEntry, choice::Variables}, cmd::runtime::RuntimeCommand};

#[derive(Debug)]
pub struct InputController {
	rl: DefaultEditor,
	quit: bool
}

pub enum InputContext {
	Choices(usize),
	Variable(String, Option<String>)
}

impl InputContext {
	const PROMPT: &'static str = "> ";

	pub fn prompt(&self) -> String {
		use InputContext::*;
		match self {
			Choices(_) => Self::PROMPT.to_owned(),
			Variable(_, prompt) => prompt.clone().map(|s| format!("{s}: ")).unwrap_or(Self::PROMPT.to_owned())
		}
	}
}

pub struct VariableInputResult(pub String, pub String);

impl VariableInputResult {
	pub fn to_variable_entry(&self, variables: &Variables) -> (&String, VariableEntry) {
		(&self.0, VariableEntry::new(&self.0, self.1.clone(), variables))
	}
}

pub enum InputResult {
	Quit(bool),
	Choice(usize),
	Variable(VariableInputResult),
	Command(Result<RuntimeCommand>)
}

impl InputController {
	pub fn new() -> Result<Self> {
		Ok(Self {
			rl: DefaultEditor::new()?,
			quit: false
		})
	}

	pub fn parse_command(line: String) -> Result<RuntimeCommand> {
		// Split line into command + arguments after '.' starting character
		let args: Vec<String> = line.strip_prefix(".").unwrap().split(" ")
			.map(|s| s.to_owned())
			.collect();
		RuntimeCommand::try_parse_from(args)
			.map_err(|e| anyhow!(e))
	}

	pub fn handle_line(line: String, context: &InputContext) -> Result<InputResult> {
		if line.is_empty() {
			return Err(anyhow!("Input cannot be empty"));
		}
		if line.starts_with(".") {
			return Ok(InputResult::Command(Self::parse_command(line)))
		}
		match context {
			&InputContext::Choices(choices) => {
				let choice = line.parse::<usize>()
					.map_err(|_| anyhow!("Input must be a number"))?;
				if choice < 1 || choice > choices {
					return Err(anyhow!("Input out of range"))
				}
				Ok(InputResult::Choice(choice))
			}
			InputContext::Variable(name, _) => Ok(InputResult::Variable(VariableInputResult(name.clone(), line)))
		}
	}

	pub fn take(&mut self, context: &InputContext) -> Result<InputResult> {
		use InputResult::*;
		match self.rl.readline(&context.prompt()) {
			Ok(line) => {
				if self.quit {
					self.quit = false;
				}
				let result = Self::handle_line(line.trim().to_owned(), context)?;
				self.rl.add_history_entry(line)?;
				Ok(result)
			},
			Err(_) => {
				let result = Quit(self.quit);
				if !self.quit {
					self.quit = true;
				}
				return Ok(result);
			}
		}
	}
}