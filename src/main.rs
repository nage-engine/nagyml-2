#![feature(result_flattening)]
#![feature(iterator_try_collect)]

use std::path::PathBuf;

use crate::core::{manifest::Manifest, resources::Resources};

use anyhow::{Result, Context};
use clap::Parser;
use cmd::cli::CliCommand;
use game::{main::{begin, crash_context}, input::InputController};
use loading::{base::Loader, saves::SaveManager};

mod core;
mod game;
mod cmd;
mod loading;
mod text;

fn run(path: PathBuf, pick: bool, new: bool) -> Result<()> {
    // Create content loader
    let loader = Loader::new(path);
    // Load content and data
    let config = Manifest::load(&loader)?;
    let resources = Resources::load(&loader, &config)?;
    // Load player
    let saves = SaveManager::new(&config)?;
    let (mut player, save_file) = saves.load(&config, pick, new)?;
    // Validate loaded resources
    resources.validate()?;
    // Create input controller
    let mut input = InputController::new()?;
    // Begin game loop
    let silent = begin(&config, &mut player, &saves, &resources, &mut input)
        .with_context(|| crash_context(&config))?;
    // Shut down game with silence based on game loop result
    if !silent {
        println!("Exiting...");
    }
    // Save player data
    saves.write(&player, save_file, new)?;

    Ok(())
}

fn main() -> Result<()> {
    // Parse CLI command - if 'run', use logic above
    // otherwise, uses its own method
    let command = CliCommand::parse();
    if let CliCommand::Run { path, pick, new } = command {
        return run(path.unwrap_or(PathBuf::new()), pick, new);
    }
    command.run()
}
