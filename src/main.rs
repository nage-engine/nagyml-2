#![feature(result_flattening)]
#![feature(iterator_try_collect)]

use crate::core::{manifest::Manifest, resources::Resources};

use anyhow::{Context, Result};
use camino::Utf8PathBuf;
use clap::Parser;
use cmd::cli::CliCommand;
use game::{
    input::InputController,
    main::{begin, crash_context},
};
use loading::{loader::Loader, saves::SaveManager};

mod cmd;
mod core;
mod game;
mod loading;
mod text;

pub const NAGE_VERSION: &str = env!("CARGO_PKG_VERSION");

fn run(path: Utf8PathBuf, pick: bool, new: bool) -> Result<()> {
    // Create content loader
    let mapping = Loader::mapping(&path)?;
    let archive = Loader::archive(&mapping)?;
    let tree = Loader::tree(&archive)?;
    let loader = Loader::new(path, &archive, &tree)?;
    // Load content and data
    let config = Manifest::load(&loader)?;
    let resources = Resources::load(&loader, &config)?;
    // Load player
    let saves = SaveManager::new(&config, pick, new)?;
    let mut player = saves.load(&config)?;
    // Validate loaded resources
    resources.validate()?;
    // Load rich presence
    let mut drpc = config.connect_rich_presence();
    // Create input controller
    let mut input = InputController::new()?;
    // Begin game loop
    let silent = begin(&config, &mut player, &saves, &resources, &mut drpc, &mut input)
        .with_context(|| crash_context(&config))?;
    // Shut down game with silence based on game loop result
    if !silent {
        println!("Exiting...");
    }
    // Save player data
    if config.settings.save {
        saves.write(&player)?;
    }

    Ok(())
}

fn main() -> Result<()> {
    // Parse CLI command - if 'run', use logic above
    // otherwise, uses its own method
    let command = CliCommand::parse();
    if let CliCommand::Run { path, pick, new } = command {
        return run(path.unwrap_or(Utf8PathBuf::from(".")), pick, new);
    }
    command.run()
}
