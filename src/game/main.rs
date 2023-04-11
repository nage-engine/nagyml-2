use anyhow::{anyhow, Result};

use crate::{
    core::{
        choice::Choice,
        context::{StaticContext, TextContext},
        discord::RichPresence,
        manifest::Manifest,
        player::Player,
        prompt::{Prompt, PromptModel},
        text::display::Text,
    },
    loading::saves::SaveManager,
    text_context,
};

use super::{
    gloop::{next_input_context, take_input, GameLoopResult},
    input::InputController,
};

pub fn first_play_init(stc: &StaticContext, player: &mut Player) -> Result<()> {
    let text_context = text_context!(stc, player);
    if let Some(background) = &stc.config.entry.background {
        Text::print_lines_nl(background, player, &text_context)?;
    }
    stc.config.entry.submit_sounds(player, stc, &text_context)?;
    player.began = true;
    Ok(())
}

pub fn begin(
    stc: &StaticContext,
    player: &mut Player,
    saves: &SaveManager,
    drpc: &mut Option<RichPresence>,
    input: &mut InputController,
) -> Result<bool> {
    if !player.began {
        first_play_init(stc, player)?;
    }

    stc.config
        .set_rich_presence(drpc, &player.latest_entry()?.path.to_string())?;

    let silent = 'outer: loop {
        // Text context owns variables to avoid immutable and mutable borrow overlap
        let text_context = text_context!(stc, player);
        let entry = player.latest_entry()?;
        let next_prompt = Prompt::get(&stc.resources.prompts, &entry.path)?;
        let model = next_prompt.model(&text_context)?;
        let choices = next_prompt.usable_choices(&player.notes, &text_context)?;

        if choices.is_empty() {
            return Err(anyhow!("No usable choices"));
        }

        let raw_choices: Vec<&Choice> = choices.iter().map(|(choice, _)| *choice).collect();
        next_prompt.print(player, &model, entry.display, &raw_choices, &text_context)?;

        match model {
            PromptModel::Redirect(choice) => {
                player.choose_full(choice, &None, None, drpc, &model, stc, &text_context)?
            }
            PromptModel::Ending(lines) => {
                Text::print_lines(lines, player, &text_context)?;
                break 'outer true;
            }
            _ => loop {
                let context = next_input_context(&model, &choices, &text_context)?
                    .ok_or(anyhow!("Could not resolve input context"))?;

                match take_input(
                    input,
                    &context,
                    player,
                    saves,
                    drpc,
                    &model,
                    &choices,
                    stc,
                    &text_context,
                )? {
                    GameLoopResult::Retry(flush) => {
                        if flush {
                            println!()
                        }
                    }
                    GameLoopResult::Continue => {
                        println!();
                        break;
                    }
                    GameLoopResult::Shutdown(silent) => break 'outer silent,
                }
            },
        }
    };
    Ok(silent)
}

pub fn crash_context(config: &Manifest) -> String {
    let contact = config
        .metadata
        .game_contact()
        .map(|msg| format!("\n\n{msg}"))
        .unwrap_or(String::new());
    format!("The game has crashed; it's not your fault!{contact}")
}
