use clap::Parser;

#[derive(Parser, Debug)]
pub enum RuntimeCommand {
	Echo { text: String }
}

impl RuntimeCommand {
	pub fn run(&self) {
		use RuntimeCommand::*;
		match self {
			Echo { text } => println!("{text}")
		}
	}
}