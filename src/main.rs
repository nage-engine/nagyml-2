#![feature(result_flattening)]

use anyhow::Result;

use crate::core::game::Game;

mod core;
mod input;
mod loading;

fn main() -> Result<()> {
    let mut game = Game::load()?;
    game.validate()?;
    let silent = game.begin()?;
    game.shutdown(silent);
    
    Ok(())
}
