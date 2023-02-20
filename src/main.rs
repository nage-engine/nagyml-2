#![feature(result_flattening)]

use std::{path::{PathBuf, Path}, collections::HashMap};

use anyhow::Result;
use rustyline::Editor;
use crate::core::{player::Player, manifest::Manifest};
use walkdir::WalkDir;

use crate::{core::{prompt::PromptFile, text::{Text, TextMode}, game::Game}};

mod core;
mod input;
mod loading;

fn main() -> Result<()> {
    /*let text = Text {
        content: String::from("bruh <guy>"),
        mode: Mode::Dialogue
    };
    let text2 = Text {
        content: String::from("the guy stops"),
        mode: Mode::Action
    };
    let mut variables: HashMap<String, String> = HashMap::new();
    variables.insert("guy".to_owned(), "hello".to_owned());
    let lines: Vec<Text> = vec![text, text2];
    println!("{}", Text::get_lines(&lines, &variables));*/

    let mut game = Game::load()?;
    let _ = game.validate()?;
    let silent = game.begin()?;
    game.shutdown(silent);

    /*let choice = game.get_prompt(&"work_to_do".to_owned(), &"main".to_owned())?.choices[1];
    dbg!(choice);

    game.player.accept_note_actions(choice.notes.unwrap());*/
    //println!("{:#?}", game);

    /*let mut rl = Editor::<()>::new()?;

    loop {
        let readline = rl.readline("> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str());
            },
            Err(err) => {
                println!("{err}");
                break
            }
        }
    }*/
    
    Ok(())
}
