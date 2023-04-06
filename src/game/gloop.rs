use anyhow::Result;
use result::OptionResultExt;

use crate::{
    cmd::runtime::{CommandResult, RuntimeCommand},
    core::{
        choice::Choice,
        context::{StaticContext, TextContext},
        discord::RichPresence,
        player::Player,
        prompt::PromptModel,
        state::NamedVariableEntry,
    },
    game::input::{InputContext, InputResult},
    loading::saves::SaveManager,
    text::display::Text,
};

use super::input::InputController;

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

pub fn handle_choice(
    choice: &Choice,
    player: &mut Player,
    drpc: &mut Option<RichPresence>,
    model: &PromptModel,
    stc: &StaticContext,
    text_context: &TextContext,
) -> Result<GameLoopResult> {
    use GameLoopResult::*;
    player.choose_full(choice, None, drpc, model, stc, text_context)?;
    if let Some(ending) = &choice.ending {
        println!();
        Text::print_lines(ending, player, text_context)?;
        return Ok(Shutdown(true));
    }
    Ok(Continue)
}

pub fn handle_command(
    parse: Result<RuntimeCommand>,
    player: &mut Player,
    saves: &SaveManager,
    stc: &StaticContext,
    text_context: &TextContext,
) -> Result<GameLoopResult> {
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
    Ok(GameLoopResult::Retry(parse.is_ok()))
}

pub fn take_input(
    input: &mut InputController,
    context: &InputContext,
    player: &mut Player,
    saves: &SaveManager,
    drpc: &mut Option<RichPresence>,
    model: &PromptModel,
    choices: &Vec<&Choice>,
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
                handle_choice(choices[i - 1], player, drpc, model, stc, text_context)?
            }
            InputResult::Variable { name, value } => {
                // Modify variables after the choose call since history entries are sensitive to this order
                let entry = NamedVariableEntry::new(name.clone(), value.clone(), &player.variables);
                player.choose(choices[0], Some(entry), model, stc, text_context)?;
                player.variables.insert(name, value);
                player.after_choice(choices[0], stc, drpc)?;
                Continue
            }
            InputResult::Command(parse) => handle_command(parse, player, saves, stc, text_context)?,
        },
    };
    Ok(result)
}

pub fn next_input_context(
    model: &PromptModel,
    choices: &Vec<&Choice>,
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
