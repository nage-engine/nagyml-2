use anyhow::{Result, anyhow};
use rustyline::Editor;

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
	Variable(String, String)
}

impl InputController {
	pub fn new() -> Result<Self> {
		Ok(Self {
			rl: Editor::new()?,
			quit: false
		})
	}

	pub fn handle_line(line: &String, context: &InputContext) -> Result<InputResult> {
		if line.is_empty() {
			return Err(anyhow!("Input cannot be empty"));
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
			InputContext::Variable(name, _) => Ok(InputResult::Variable(name.clone(), line.trim().to_owned()))
		}
	}

	pub fn take(&mut self, context: &InputContext) -> Result<InputResult> {
		use InputResult::*;
		match self.rl.readline(context.prompt()) {
			Ok(line) => {
				if self.quit {
					self.quit = false;
				}
				let result = Self::handle_line(&line, context)?;
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