use std::{collections::HashMap, path::PathBuf};

use anyhow::Result;
use clap::Parser;
use requestty::Question;
use semver::Version;
use tinytemplate::TinyTemplate;

use crate::{core::manifest::Manifest, loading::{base::Loader, saves::SaveManager}};

pub const TEMPLATE_MANIFEST: &'static str = include_str!("../template/nage.yml");
pub const TEMPLATE_MAIN: &'static str = include_str!("../template/main.yml");

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub enum CliCommand {
	#[command(about = "Run a Nagame", alias = "r")]
	Run {
		#[arg(help = "The game directory. Defaults to the current directory")]
		path: Option<PathBuf>,
		#[arg(short, long, help = "Start a new save file")]
		new: bool,
		#[arg(short, long, help = "Pick from a list of multiple saves instead of the last used")]
		pick: bool
	},
	#[command(about = "Create a new Nagame template")]
	New {
		#[arg(short, long, help = "Create all extra content directories")]
		full: bool
	},
	#[command(about = "Open the save directory")]
	Saves
}

impl CliCommand {
	fn new_properties() -> Result<HashMap<String, String>> {
		let module = requestty::PromptModule::new(vec![
			Question::input("Game name").build(),
			Question::input("Author").build(),
			Question::input("Version")
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

	/// Handles a [`Saves`](CliCommand::Saves) command.
	fn saves() -> Result<()> {
		let loader = Loader::new(PathBuf::new());
		let config = Manifest::load(&loader)?;
		let _ = open::that(SaveManager::dir(&config, false)?)?;
		Ok(())
	}

	pub fn run(&self) -> Result<()> {
		use CliCommand::*;
		match self {
			&New { full } => Self::new(full),
			Saves => Self::saves(),
			_ => unreachable!()
		}
	}
}