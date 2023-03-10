use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use requestty::Question;
use semver::Version;
use tinytemplate::TinyTemplate;

use crate::core::manifest::Manifest;

pub const TEMPLATE_MANIFEST: &'static str = include_str!("../template/nage.yml");
pub const TEMPLATE_MAIN: &'static str = include_str!("../template/main.yml");

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub enum CliCommand {
	#[command(about = "Run a Nagame", alias = "r")]
	Run {
		#[arg(help = "The game directory. Defaults to the current directory")]
		path: PathBuf
	},
	#[command(about = "Create a new Nagame template")]
	New {
		#[arg(short, long, help = "Create all extra content directories")]
		full: bool
	}
}

impl CliCommand {

	fn new_properties() -> Result<HashMap<String, String>> {
		let module = requestty::PromptModule::new(vec![
			Question::input("name")
				.message("Game name")
				.build(),
			Question::input("author")
				.message("Author")
				.build(),
			Question::input("version")
				.message("Version")
				.validate(|ver, _| {
					Version::parse(ver)
						.map(|_| ())
						.map_err(|err| err.to_string())
				})
				.build()
		]);

		let result = module.prompt_all()?.iter()
    		.map(|(k, v)| (k.clone(), v.as_string().unwrap().to_owned()))
			.collect();
		Ok(result)
	}

	/// Handles a [`New`](CliCommand::New) command.
	fn new(full: bool) -> Result<()> {
		let properties = Self::new_properties()?;

		let mut tt = TinyTemplate::new();
		tt.add_template("manifest", TEMPLATE_MANIFEST)?;
		let manifest = tt.render("manifest", &properties)?;

		std::fs::write(Manifest::FILE, manifest)?;
		let _ = std::fs::create_dir("prompts");
		std::fs::write("prompts/main.yml", TEMPLATE_MAIN)?;

		if full {
			for dir in ["info", "lang", "scripts", "sounds"] {
				let _ = std::fs::create_dir(dir);
			}
		}

		Ok(())
	}

	pub fn run(&self) -> Result<()> {
		use CliCommand::*;
		match self {
			&New { full } => Self::new(full),
			_ => unreachable!()
		}
	}
}