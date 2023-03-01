#![feature(result_flattening)]
#![feature(iterator_try_collect)]

use crate::core::{manifest::Manifest, player::Player};

use anyhow::{Result, Context};
use game::main::{Resources, begin, crash_context, shutdown};
use input::controller::InputController;

mod core;
mod game;
mod input;
mod loading;

fn main() -> Result<()> {
    // Load content and data
    let config = Manifest::load()?;
	let mut player = Player::load(&config)?;
    let resources = Resources::load()?;
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
