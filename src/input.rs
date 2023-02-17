use anyhow::{Result, anyhow};
use rustyline::Editor;

#[derive(Debug)]
pub struct InputController {
	rl: Editor<()>,
	quit: bool
}

pub enum InputResult {
	Quit(bool),
	Choice(usize)
}

impl InputController {
	const PROMPT: &'static str = "> ";

	pub fn new() -> Result<Self> {
		Ok(Self {
			rl: Editor::new()?,
			quit: false
		})
	}

	pub fn handle_line(line: &String, choices: usize) -> Result<InputResult> {
		use InputResult::*;
		if line.is_empty() {
			return Err(anyhow!("Input cannot be empty"));
		}
		let choice = line.parse::<usize>()
			.map_err(|_| anyhow!("Input must be a number"))?;
		if choice < 1 || choice > choices {
			return Err(anyhow!("Input out of range"))
		}
		Ok(Choice(choice))
	}

	pub fn take_prompt(&mut self, prompt: &str, choices: usize) -> Result<InputResult> {
		use InputResult::*;
		match self.rl.readline(prompt) {
			Ok(line) => {
				if self.quit {
					self.quit = false;
				}
				let result = Self::handle_line(&line, choices)?;
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

	pub fn take(&mut self, choices: usize) -> Result<InputResult> {
		self.take_prompt(Self::PROMPT, choices)
	}
}