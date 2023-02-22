use std::collections::VecDeque;

use anyhow::{Result, anyhow};
use clap::Parser;
use rustyline::Editor;

use super::commands::RuntimeCommand;

#[derive(Debug)]
pub struct InputController {
	rl: Editor<()>,
	quit: bool
}

pub enum InputContext {
	Choices(usize),
	Variable(String, Option<String>)
}

impl InputContext {
	const PROMPT: &'static str = "> ";

	pub fn prompt(&self) -> &str {
		use InputContext::*;
		match self {
			Choices(_) => Self::PROMPT,
			Variable(_, prompt) => prompt.as_deref().unwrap_or(Self::PROMPT)
		}
	}
}

pub enum InputResult {
	Quit(bool),
	Choice(usize),
	Variable(String, String),
	Command(Result<RuntimeCommand>)
}

impl InputController {
	pub fn new() -> Result<Self> {
		Ok(Self {
			rl: Editor::new()?,
			quit: false
		})
	}

	pub fn parse_command(line: String) -> Result<RuntimeCommand> {
		// Split line into command + arguments after '.' starting character
		let mut args: VecDeque<String> = line.strip_prefix(".").unwrap().split(" ")
			.map(|s| s.to_owned())
			.collect();
		// Hack to treat 'runtime' as the main command and parse subcommands with clap
		args.push_front(String::from("runtime"));
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
			InputContext::Variable(name, _) => Ok(InputResult::Variable(name.clone(), line))
		}
	}

	pub fn take(&mut self, context: &InputContext) -> Result<InputResult> {
		use InputResult::*;
		match self.rl.readline(context.prompt()) {
			Ok(line) => {
				if self.quit {
					self.quit = false;
				}
				let result = Self::handle_line(line.trim().to_owned(), context)?;
				self.rl.add_history_entry(line);
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