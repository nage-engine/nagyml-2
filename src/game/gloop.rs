use anyhow::Result;
use result::OptionResultExt;

use crate::{
    cmd::runtime::CommandResult,
    core::{
        choice::UsableChoices,
        context::{StaticContext, TextContext},
        discord::RichPresence,
        player::Player,
        prompt::PromptModel,
        state::variables::NamedVariableEntry,
        text::display::Text,
    },
    game::input::{InputContext, InputResult},
    loading::saves::SaveManager,
};

use super::input::InputController;

pub fn next_input_context(
    model: &PromptModel,
    choices: &UsableChoices,
    text_context: &TextContext,
) -> Result<Option<InputContext>> {
    use PromptModel::*;
    let result = match &model {
        Response => Some(InputContext::Choices(choices.len())),
        &Input(name, prompt) => Some(InputContext::Variable(
            name.clone(),
            prompt.map(|s| s.fill(text_context)).invert()?,
        )),
        _ => None,
    };
    Ok(result)
}

pub enum GameLoopResult {
    Retry(bool),
    Continue,
    Shutdown(bool),
}

pub fn handle_quit(shutdown: bool) -> GameLoopResult {
    use GameLoopResult::*;
    if shutdown {
        Shutdown(false)
    } else {
        println!("Signal quit again or use '.quit' to exit");
        Retry(true)
    }
}

pub fn take_input(
    input: &mut InputController,
    context: &InputContext,
    player: &mut Player,
    saves: &SaveManager,
    drpc: &mut Option<RichPresence>,
    model: &PromptModel,
    choices: &UsableChoices,
    stc: &StaticContext,
    text_context: &TextContext,
) -> Result<GameLoopResult> {
    use GameLoopResult::*;
    let result = match input.take(context) {
        Err(err) => {
            println!("{err}");
            Retry(true)
        }
        Ok(result) => match result {
            InputResult::Quit(shutdown) => handle_quit(shutdown),
            InputResult::Choice(i) => {
                let (choice, once) = &choices[i - 1];
                player.choose_full(choice, once, None, drpc, model, stc, text_context)?;

                match &choice.ending {
                    Some(ending) => {
                        println!();
                        Text::print_lines(ending, player, text_context)?;
                        Shutdown(true)
                    }
                    None => Continue,
                }
            }
            InputResult::Variable { name, value } => {
                // Modify variables after the choose call since history entries are sensitive to this order
                let entry = NamedVariableEntry::new(name.clone(), value.clone(), &player.variables);
                let (choice, once) = &choices[0];
                player.choose(choice, once, Some(entry), model, stc, text_context)?;
                player.variables.insert(name, value);
                player.after_choice(choice, stc, drpc)?;
                Continue
            }
            InputResult::Command(parse) => {
                match &parse {
                    Err(err) => println!("\n{err}"), // Clap error
                    Ok(command) => {
                        match command.run(player, saves, stc, text_context) {
                            Err(err) => println!("Error: {err}"), // Command runtime error
                            Ok(result) => match result {
                                CommandResult::Submit(loop_result) => return Ok(loop_result),
                                CommandResult::Output(output) => println!("{output}"),
                            },
                        }
                    }
                };
                GameLoopResult::Retry(parse.is_ok())
            }
        },
    };
    Ok(result)
}
