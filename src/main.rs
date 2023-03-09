#![feature(result_flattening)]
#![feature(iterator_try_collect)]

use crate::core::{manifest::Manifest, player::Player, resources::Resources};

use anyhow::{Result, Context};
use clap::Parser;
use cmd::cli::CliCommand;
use game::{main::{begin, crash_context, shutdown}, input::InputController};

mod core;
mod game;
mod cmd;
mod loading;

fn run() -> Result<()> {
    // Load content and data
    let config = Manifest::load()?;
	let mut player = Player::load(&config)?;
    let resources = Resources::load(&config)?;
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
    if let CliCommand::Run = command {
        return run();
    }
    command.run()
}
