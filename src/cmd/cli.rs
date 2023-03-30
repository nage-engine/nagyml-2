use std::collections::HashMap;

use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use requestty::Question;
use semver::Version;
use tinytemplate::TinyTemplate;

use crate::{
    //cmd::builder::prompt::build_prompt,
    core::manifest::Manifest,
    loading::{loader::Loader, saves::SaveManager},
};

pub const TEMPLATE_MANIFEST: &'static str = include_str!("../template/nage.yml");
pub const TEMPLATE_MAIN: &'static str = include_str!("../template/main.yml");

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub enum CliCommand {
    #[command(about = "Run a Nagame", alias = "r")]
    Run {
        #[arg(help = "The game directory. Defaults to the current directory")]
        path: Option<Utf8PathBuf>,
        #[arg(short, long, help = "Start a new save file")]
        new: bool,
        #[arg(short, long, help = "Pick from a list of multiple saves instead of the last used")]
        pick: bool,
    },
    #[command(about = "Create a new Nagame template")]
    New {
        #[arg(short, long, help = "Create all extra content directories")]
        full: bool,
    },
    #[command(about = "Build a prompt from the command line")]
    Builder,
    #[command(about = "Open the save directory")]
    Saves,
}

impl CliCommand {
    fn new_properties() -> Result<HashMap<String, String>> {
        let module = requestty::PromptModule::new(vec![
            Question::input("name").message("Game name").build(),
            Question::input("author").message("Author").build(),
            Question::input("version")
                .message("Version")
                .validate(|ver, _| {
                    Version::parse(ver)
                        .map(|_| ())
                        .map_err(|err| err.to_string())
                })
                .build(),
        ]);

        let result = module
            .prompt_all()?
            .iter()
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

    /// Handles a [`Builder`](CliCommand::Builder) command.
    fn builder() -> Result<()> {
        /*let prompt = build_prompt()?;
        let yaml = serde_yaml::to_string(&prompt)?;
        let stripped = yaml.strip_prefix("---").unwrap();
        println!("{stripped}");*/
        println!("Temporarily out of service!");
        Ok(())
    }

    /// Handles a [`Saves`](CliCommand::Saves) command.
    fn saves() -> Result<()> {
        let loader = Loader::current_dir();
        match Manifest::load(&loader) {
            Ok(config) => open::that(SaveManager::game_dir(&config)?)
                .with_context(|| anyhow!("Unable to open game save directory"))?,
            Err(_) => open::that(SaveManager::generic_dir()?)
                .with_context(|| anyhow!("Unable to open global save directory"))?,
        };
        Ok(())
    }

    pub fn run(&self) -> Result<()> {
        use CliCommand::*;
        match self {
            &New { full } => Self::new(full),
            Builder => Self::builder(),
            Saves => Self::saves(),
            _ => unreachable!(),
        }
    }
}
