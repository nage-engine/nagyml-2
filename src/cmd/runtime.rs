use anyhow::{anyhow, Result};
use clap::Parser;

use crate::{
    core::{
        audio::Audio,
        context::{StaticContext, TextContext},
        path::{PathData, PathLookup},
        player::Player,
        prompt::Prompt as PromptUtil,
        resources::Resources,
        state::{InfoPages, Notes, UnlockedInfoPages},
    },
    game::gloop::GameLoopResult,
    loading::saves::SaveManager,
    text::display::Translations,
};

#[derive(Parser, Debug, PartialEq)]
#[command(multicall = true)]
pub enum RuntimeCommand {
    #[command(about = "Try going back a choice")]
    Back,
    #[command(about = "Manage the display language")]
    Lang,
    #[command(about = "Display an info page")]
    Info,
    #[command(about = "Display an action log page")]
    Log,
    #[command(about = "Manage sound effects and music channels")]
    Sound,
    #[command(about = "Save the player data")]
    Save,
    #[command(about = "Save and quits the game")]
    Quit,
    #[command(about = "Display debug info about a prompt", hide = true)]
    Prompt,
    #[command(about = "List the currently applied notes", hide = true)]
    Notes,
    #[command(about = "List the currently applied variable names and their values", hide = true)]
    Variables,
}

/// The result of a runtime command.
pub enum CommandResult {
    /// Returns an input loop result to the original input call.
    Submit(GameLoopResult),
    /// Outputs a specified string and submits [`Retry`](InputLoopResult::Retry).
    Output(String),
}

impl CommandResult {
    pub fn retry() -> CommandResult {
        Self::Submit(GameLoopResult::Retry(true))
    }
}

impl RuntimeCommand {
    /// Determines if this command is allowed in a default, non-debug environment.
    fn is_normal(&self) -> bool {
        use RuntimeCommand::*;
        matches!(&self, Back | Lang | Info | Log | Sound | Save | Quit)
    }

    /// Handles a [`Back`](RuntimeCommand::Back) command.
    fn back(player: &mut Player) -> Result<CommandResult> {
        if player.history.len() <= 1 {
            return Err(anyhow!("Can't go back right now!"));
        }
        player.back()?;
        Ok(CommandResult::Submit(GameLoopResult::Continue))
    }

    /// Handles a [`Lang`](RuntimeCommand::Lang) command.
    fn lang(player: &mut Player, translations: &Translations) -> Result<CommandResult> {
        if translations.is_empty() {
            return Err(anyhow!("No display languages loaded"));
        }

        println!();

        let lang_question = requestty::Question::select("Select a language")
            .choices(translations.keys())
            .build();
        let lang_choice = requestty::prompt_one(lang_question)?;
        player.lang = lang_choice.as_list_item().unwrap().text.clone();

        Ok(CommandResult::retry())
    }

    /// Handles an [`Info`](RuntimeCommand::Info) command.
    fn info(unlocked_pages: &UnlockedInfoPages, pages: &InfoPages) -> Result<CommandResult> {
        if unlocked_pages.is_empty() {
            return Err(anyhow!("No info pages unlocked"));
        }

        println!();

        let choices: Vec<&str> = unlocked_pages
            .iter()
            .map(|page| page.as_name.as_str())
            .collect();
        let info_question = requestty::Question::select("Select an info page")
            .choices(choices)
            .build();

        let info_choice = requestty::prompt_one(info_question)?;
        let page = &unlocked_pages[info_choice.as_list_item().unwrap().index];

        println!();

        termimad::print_text(pages.get(&page.name).unwrap());

        Ok(CommandResult::retry())
    }

    /// Handles a [`Log`](RuntimeCommand::Log) command.
    fn log(player: &Player) -> Result<CommandResult> {
        if player.log.is_empty() {
            return Err(anyhow!("No log entries to display"));
        }

        println!();

        let pages = player.log_pages();
        let page_question = requestty::Question::raw_select("Log page")
            .choices(Player::log_page_fronts(&pages))
            .build();
        let page_choice = requestty::prompt_one(page_question)?;

        let page_content = pages
            .get(page_choice.as_list_item().unwrap().index)
            .unwrap();
        let entries = page_content.join("\n\n");
        Ok(CommandResult::Output(format!("\n{entries}")))
    }

    /// Handles a [`Sound`](RuntimeCommand::Sound) command.
    fn sound(player: &mut Player, audio_res: &Option<Audio>) -> Result<CommandResult> {
        let audio = audio_res
            .as_ref()
            .ok_or(anyhow!("No sound channels loaded"))?;

        println!();

        // Multi-selection where selected represents the channel being enabled and vice versa
        let channel_data = audio.channel_statuses(player);
        let channel_selection = requestty::Question::multi_select("Select sound channels")
            .choices_with_default(channel_data)
            .build();
        let channel_choices = requestty::prompt_one(channel_selection)?;

        // The selected channels
        let enabled_channels: Vec<String> = channel_choices
            .as_list_items()
            .unwrap()
            .iter()
            .map(|choice| choice.text.clone())
            .collect();

        // Each possible channel will either be selected or not; if so, append to player's
        // enabled channel list if not already present, otherwise remove and stop the channel playback if necessary
        for channel in audio.players.keys() {
            if enabled_channels.contains(channel) {
                player.channels.insert(channel.clone());
            } else {
                player.channels.remove(channel);
                audio.get_player(channel)?.stop();
            }
        }

        Ok(CommandResult::retry())
    }

    /// Handles a [`Prompt`](RuntimeCommand::Prompt) command.
    fn prompt(
        notes: &Notes,
        resources: &Resources,
        text_context: &TextContext,
    ) -> Result<CommandResult> {
        println!();

        let file_question = requestty::Question::select("Prompt file")
            .choices(resources.prompts.keys())
            .build();
        let file_choice = requestty::prompt_one(file_question)?;
        let file = &file_choice.as_list_item().unwrap().text;

        let prompt_question = requestty::Question::select(format!("Prompt in '{}'", file))
            .choices(PromptUtil::get_file(&resources.prompts, file)?.keys())
            .build();
        let prompt_choice = requestty::prompt_one(prompt_question)?;
        let prompt = &prompt_choice.as_list_item().unwrap().text;

        let lookup: PathData = PathLookup::new(file, prompt).into();
        let prompt = PromptUtil::get(&resources.prompts, &lookup)?;
        Ok(CommandResult::Output(prompt.debug_info(
            &lookup.into(),
            &resources.prompts,
            notes,
            text_context,
        )?))
    }

    /// Handles a [`Notes`](RuntimeCommand::Notes) command.
    fn notes(player: &Player) -> Result<CommandResult> {
        if player.notes.is_empty() {
            return Err(anyhow!("No notes applied"));
        }
        let result = itertools::join(&player.notes, ", ");
        Ok(CommandResult::Output(result))
    }

    /// Handles a [`Variables`](RuntimeCommand::Variables) command.
    fn variables(player: &Player) -> Result<CommandResult> {
        if player.variables.is_empty() {
            return Err(anyhow!("No variables applied"));
        }
        let vars = player
            .variables
            .clone()
            .into_iter()
            .map(|(name, value)| format!("{name}: {value}"))
            .collect::<Vec<String>>()
            .join("\n");
        Ok(CommandResult::Output(format!("\n{vars}")))
    }

    /// Executes a runtime command if the player has permission to do so.
    ///
    /// Any errors will be reported to the input loop with a retry following.
    pub fn run(
        &self,
        player: &mut Player,
        saves: &SaveManager,
        stc: &StaticContext,
        text_context: &TextContext,
    ) -> Result<CommandResult> {
        if !self.is_normal() && !stc.config.settings.debug {
            return Err(anyhow!("Unable to access debug commands"));
        }
        use CommandResult::*;
        use RuntimeCommand::*;
        let result = match self {
            Back => Self::back(player)?,
            Lang => Self::lang(player, &stc.resources.translations)?,
            Info => Self::info(&player.info_pages, &stc.resources.info_pages)?,
            Log => Self::log(&player)?,
            Sound => Self::sound(player, &stc.resources.audio)?,
            Save => {
                saves.write(player)?;
                Output("Saving... ".to_owned())
            }
            Quit => Submit(GameLoopResult::Shutdown(false)),
            Prompt => Self::prompt(&player.notes, stc.resources, text_context)?,
            Notes => Self::notes(player)?,
            Variables => Self::variables(player)?,
        };
        Ok(result)
    }
}
