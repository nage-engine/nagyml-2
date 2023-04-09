use std::collections::HashMap;

use crate::text::{
    display::{choice_text, Text, TextLines},
    templating::{TemplatableString, TemplatableValue},
};

use super::{
    audio::SoundActions,
    context::{StaticContext, TextContext},
    path::{Path, PathData, PathLookup},
    player::HistoryEntry,
    prompt::{Prompt, PromptModel},
    state::{
        InfoApplications, NamedVariableEntry, NoteActions, Notes, VariableApplications,
        VariableEntries, VariableEntry, VariableInput, Variables,
    },
};

use anyhow::{anyhow, Context, Result};
use result::OptionResultExt;
use serde::{Deserialize, Serialize};

pub fn default_true() -> TemplatableValue<bool> {
    TemplatableValue::value(true)
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A container for the second component of the "prompt-choice" model.
///
/// Upon a player making a choice, A definitive result is guaranteed to be reached: jumping to another prompt or ending the game.
///
/// Choices can require specific player state be present to be usable, and also modify player state.
pub struct Choice {
    #[serde(default, deserialize_with = "choice_text", skip_serializing_if = "Option::is_none")]
    /// The response text to display, in order, when a player is presented with [`Choices`].
    /// Only required when there is more than one choice available.
    /// Mutually exclusive with `input`.
    /// The text options for this field are limited to only the `content` and the `mode`.
    pub response: Option<Text>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// A "trait" tag to display in front of choice responses.
    /// See [`Choice::tag`] for more information.
    pub tag: Option<TemplatableString>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// A container to prompt player input to save to a variable.
    /// There can only be one choice in an input prompt. It also has its own prompt model: [`Input`](PromptModel::Input).
    pub input: Option<VariableInput>,
    #[serde(
        skip_serializing_if = "Option::is_none",
        //deserialize_with = "crate::core::path::deserialize"
    )]
    /// The prompt to jump to after the choice is made and state is modified.
    /// Mutually exclusive with `ending`.
    pub jump: Option<Path>,
    #[serde(default = "default_true")]
    /// Whether to display the next prompt's introductory text.
    pub display: TemplatableValue<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether to prevent a player from reversing this choice in their history.
    /// If [`None`], defaults to the config.
    pub lock: Option<TemplatableValue<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Note actions to apply and require from a player.
    pub notes: Option<NoteActions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Variables to statically apply to a player without their input.
    pub variables: Option<VariableApplications>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// A singular log string to append to a player's log entries.
    pub log: Option<TemplatableString>,
    #[serde(rename = "info", skip_serializing_if = "Option::is_none")]
    /// Info pages to unlock for a player upon using this choice.
    pub info_pages: Option<InfoApplications>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Ordered sound actions to submit to the game's [`Audio`] resource upon using this choice.
    pub sounds: Option<SoundActions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Text lines to signify the ending of a game. Printed in the same way as prompt text.
    /// If this ending choice is the only one in a prompt, `response` is optional.
    /// If in this case `response` is [`None`], the prompt will have the [`Ending`](PromptModel::Ending) model.
    pub ending: Option<TextLines>,
    #[serde(alias = "discord rich presence", skip_serializing_if = "Option::is_none")]
    /// A custom detail to show up in Discord Rich Presence after this choice is taken.
    pub drp: Option<TemplatableString>,
}

/// A list of ordered [`Choice`]s.
pub type Choices = Vec<Choice>;

impl Choice {
    /// Validates a choice amongst the global prompt context.
    ///
    /// A choice is valid if:
    /// - It has either a `jump` or `ending` section
    /// - Its `jump` section **is not templatable** and points to a valid prompt
    /// 	- The `file` key has to exist and the `prompt` key has to exist in that [`PromptFile`]
    /// - It has a `response` section if there is more than one choice in the prompt
    pub fn validate(&self, local_file: &str, has_company: bool, stc: &StaticContext) -> Result<()> {
        match &self.jump {
            None => {
                if self.ending.is_none() {
                    return Err(anyhow!(
                        "Lacks `jump` section, but doesn't have an `ending` section"
                    ));
                }
            }
            Some(jump) => {
                if let Some(file) = &jump.static_file(local_file) {
                    if let Some(prompt) = jump.prompt().content() {
                        let _ = Prompt::get(
                            &stc.resources.prompts,
                            &PathLookup::new(&file, prompt).into(),
                        )
                        .with_context(|| "`jump` section points to invalid prompt")?;
                    }
                }
            }
        }
        if has_company && self.response.is_none() {
            return Err(anyhow!(
                "Lacks `response` section, but multiple choices are present in prompt"
            ));
        }
        if let Some(audio) = &stc.resources.audio {
            if let Some(sounds) = &self.sounds {
                for (index, sound) in sounds.iter().enumerate() {
                    let _ = sound.validate(audio).with_context(|| {
                        format!("Failed to validate sound action #{}", index + 1)
                    })?;
                }
            }
        }
        Ok(())
    }

    /// Creates a map of variable entries to use when creating a new [`HistoryEntry`].
    ///
    /// If both the input result and this choice's `variables` key are [`None`], returns none.
    /// Otherwise, returns a combined map based on which inputs are present.
    pub fn create_variable_entries(
        &self,
        input: Option<NamedVariableEntry>,
        variables: &Variables,
        text_context: &TextContext,
    ) -> Result<Option<VariableEntries>> {
        let var_entries = self
            .variables
            .as_ref()
            .map(|vars| VariableEntry::from_map(&vars, variables, text_context))
            .invert()?;
        if input.is_none() && var_entries.is_none() {
            return Ok(None);
        }
        let mut entries = var_entries.unwrap_or(HashMap::new());
        if let Some(named) = input {
            entries.insert(named.name, named.entry);
        }
        Ok(Some(entries))
    }

    /// Constructs a [`HistoryEntry`] based on this choice object.
    ///
    /// Copies over control flags, the path based on the latest history entry, and notes and variable applications.
    pub fn to_history_entry(
        &self,
        latest: &HistoryEntry,
        input: Option<NamedVariableEntry>,
        variables: &Variables,
        model: &PromptModel,
        stc: &StaticContext,
        text_context: &TextContext,
    ) -> Option<Result<HistoryEntry>> {
        self.jump.as_ref().map(|jump| {
            Ok(HistoryEntry {
                path: jump.fill(&latest.path, text_context)?,
                display: self.display.get_value(text_context)?,
                locked: self
                    .lock
                    .as_ref()
                    .map(|lock| lock.get_value(text_context))
                    .invert()?
                    .unwrap_or(stc.config.settings.history.locked),
                redirect: matches!(model, PromptModel::Redirect(_)),
                notes: self
                    .notes
                    .as_ref()
                    .map(|n| n.to_note_entries(text_context))
                    .invert()?,
                variables: self.create_variable_entries(input, variables, text_context)?,
                log: self.log.is_some(),
            })
        })
    }

    /// Determines if a player can use this choice.
    ///
    /// This check passes if:
    /// - All note requirement `has` fields match the state of the provided [`Notes`] object, and
    /// - The notes object does not contain the `once` value, if any is present
    pub fn can_player_use(&self, notes: &Notes, text_context: &TextContext) -> Result<bool> {
        if let Some(actions) = &self.notes {
            if let Some(require) = &actions.require {
                for requirement in require {
                    if requirement.state.get_state(text_context)?
                        != notes.contains(&requirement.state.name.fill(text_context)?)
                    {
                        return Ok(false);
                    }
                }
            }
            if let Some(once) = &actions.once {
                if notes.contains(&once.fill(text_context)?) {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// Fills in and formats tag content, if any.
    ///
    /// If [`Some`], returns `[VALUE] `, trailing space included.
    /// If [`None`], returns an empty [`String`].
    fn tag(&self, text_context: &TextContext) -> Result<String> {
        let result = match &self.tag {
            Some(tag) => format!("[{}] ", tag.fill(text_context)?),
            None => String::new(),
        };
        Ok(result)
    }

    /// Constructs the response line for display in the game's runtime.
    ///
    /// ### Examples
    ///
    /// - `1) [ROGUE] "Ain't no thief."`
    /// - `2) Put down the sword`
    fn response_line(&self, index: usize, text_context: &TextContext) -> Result<String> {
        let tag = self.tag(text_context)?;
        let response = self.response.as_ref().unwrap().get(text_context)?;
        Ok(format!("{index}) {tag}{response}"))
    }

    /// Constructs a [`String`] of ordered choice responses.
    pub fn display(choices: &Vec<&Choice>, text_context: &TextContext) -> Result<String> {
        let result = choices
            .iter()
            .enumerate()
            .filter(|(_, choice)| choice.response.is_some())
            .map(|(index, choice)| choice.response_line(index + 1, text_context))
            .try_collect::<Vec<String>>()?
            .join("\n");
        Ok(result)
    }

    /// Whether this choice jumps to a specific prompt.
    ///
    /// Returns `true` if the choice has a `jump` path and [`Path::matches`] passes.
    pub fn has_jump_to(&self, current_file: &str, other: &PathData) -> bool {
        match &self.jump {
            None => false,
            Some(jump) => jump.matches(current_file, other),
        }
    }
}
