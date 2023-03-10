#![feature(result_flattening)]
#![feature(iterator_try_collect)]

use std::path::PathBuf;

use crate::core::{manifest::Manifest, player::Player, resources::Resources};

use anyhow::{Result, Context};
use clap::Parser;
use cmd::cli::CliCommand;
use game::{main::{begin, crash_context, shutdown}, input::InputController};
use loading::base::Loader;

mod core;
mod game;
mod cmd;
mod loading;

fn run(path: PathBuf) -> Result<()> {
    // Create content loader
    let loader = Loader::new(path);
    // Load content and data
    let config = Manifest::load(&loader)?;
	let mut player = Player::load(&loader, &config);
    let resources = Resources::load(&loader, &config)?;
    // Validate loaded resources
    resources.validate()?;
    // Create input controller
    let mut input = InputController::new()?;
    // Begin game loop
    let silent = begin(&config, &mut player, &resources, &mut input)
        .with_context(|| crash_context(&config))?;
    // Shut down game with silence based on game loop result
    shutdown(&config, &player, silent);

    Ok(())
}

fn main() -> Result<()> {
    // Parse CLI command - if 'run', use logic above
    // ootherwise, uses its own method
    let command = CliCommand::parse();
    if let CliCommand::Run { path } = command {
        return run(path.unwrap_or(PathBuf::new()));
    }
    command.run()
}
