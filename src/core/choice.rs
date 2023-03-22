use std::collections::{HashMap, HashSet};

use crate::{
    game::input::VariableInputResult,
    text::{
        context::TextContext,
        display::{Text, TextLines},
        templating::{TemplatableString, TemplatableValue},
    },
};

use super::{
    manifest::Manifest,
    path::Path,
    player::{HistoryEntry, NoteEntries, NoteEntry, VariableEntries, VariableEntry},
    prompt::{Prompt, PromptModel, Prompts},
};

use anyhow::{anyhow, Context, Result};
use result::OptionResultExt;
use serde::{Deserialize, Serialize};
use strum::{Display, EnumIter, EnumString};

pub fn default_true() -> TemplatableValue<bool> {
    TemplatableValue::value(true)
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A note action that gives or takes a note from the player.
pub struct NoteApplication {
    /// The note name to give or take.
    pub name: TemplatableString,
    #[serde(default)]
    /// Whether to take the note. If `false`, gives the note.
    pub take: TemplatableValue<bool>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A note action that requires that a player has or doesn't have a note.
pub struct NoteRequirement {
    /// The note name to check against.
    pub name: TemplatableString,
    #[serde(default = "default_true")]
    /// Whether the player should have the note.
    pub has: TemplatableValue<bool>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A collection of actions that apply or check against the player's note state.
pub struct NoteActions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Actions that apply state to the player.
    pub apply: Option<Vec<NoteApplication>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Actions that check the player's state.
    pub require: Option<Vec<NoteRequirement>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Passes state check if the player **does not** have the specified note name.
    /// Afterwards, applies this note name.
    /// Allows easy creation of one-off choices.
    pub once: Option<TemplatableString>,
}

impl NoteActions {
    /// Creates a list of [`NoteEntries`] from the note actions' [`apply`](NoteAction::apply) and [`once`](NoteAction::once) fields.
    pub fn to_note_entries(&self, text_context: &TextContext) -> Result<NoteEntries> {
        let mut entries: NoteEntries = self
            .apply
            .as_ref()
            .map(|apps| {
                apps.iter()
                    .map(|app| NoteEntry::from_application(app, text_context))
                    .collect::<Result<NoteEntries>>()
            })
            .invert()?
            .unwrap_or(Vec::new());

        if let Some(once) = &self.once {
            entries.push(NoteEntry::new(once, false, text_context)?);
        }
        Ok(entries)
    }
}

/// A list of string symbols tracked on a player.
pub type Notes = HashSet<String>;

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A container specifying how to take player input and where to save it to.
pub struct VariableInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// A custom prompt for user input.
    pub text: Option<TemplatableString>,
    #[serde(rename = "variable")]
    /// The variable name to save the user input to.
    pub name: TemplatableString,
}

/// A map of display variables wherein the key is the variable name and the value is the variable's display.
pub type Variables = HashMap<String, String>;

/// A map of variable names to values to apply to a player statically.
pub type VariableApplications = HashMap<String, TemplatableString>;

#[derive(Deserialize, Serialize, Display, Debug, Clone, EnumString, EnumIter)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
/// A [`SoundAction`] method type.
///
/// Modes that require specific sound files will return `true` from [`is_specific`](SoundActionMode::is_specific).
pub enum SoundActionMode {
    /// Queue a sound if the channel is already playing another sound.
    Queue,
    /// Immediately plays a sound on the channel regardless of whether it is already playing a sound.
    Overwrite,
    /// Plays a sound if and only if there is no sound already playing in a channel.
    Passive,
    /// Skips a sound if one is playing in a channel.
    Skip,
    /// Pauses a channel.
    Pause,
    /// Un-pauses a channel.
    Play,
}

impl Default for SoundActionMode {
    fn default() -> Self {
        Self::Passive
    }
}

impl SoundActionMode {
    /// Whether this action requires a specific sound file to be present.
    pub fn is_specific(&self) -> bool {
        use SoundActionMode::*;
        matches!(&self, Queue | Overwrite | Passive)
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A container allowing choices to control audio playback through the [`Audio`] resource.
/// Essentially a wrapper around [`playback_rs`] functionality.
pub struct SoundAction {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The sound file to submit.
    /// Only required for specific [`SoundActionMode`]s.
    pub name: Option<TemplatableString>,
    /// The channel to modify playback on.
    pub channel: TemplatableString,
    #[serde(default)]
    /// The method to apply to the sound channel.
    pub mode: TemplatableValue<SoundActionMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The specific point in a sound to start from, in milliseconds.
    pub seek: Option<TemplatableValue<u64>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The playback multiplier of the sound.
    pub speed: Option<TemplatableValue<f64>>,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A container for the second component of the "prompt-choice" model.
///
/// Upon a player making a choice, A definitive result is guaranteed to be reached: jumping to another prompt or ending the game.
///
/// Choices can require specific player state be present to be usable, and also modify player state.
pub struct Choice {
    #[serde(skip_serializing_if = "Option::is_none")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
    pub info_pages: Option<Vec<TemplatableString>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Ordered sound actions to submit to the game's [`Audio`] resource upon using this choice.
    pub sounds: Option<Vec<SoundAction>>,
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
    pub fn validate(
        &self,
        local_file: &String,
        has_company: bool,
        prompts: &Prompts,
    ) -> Result<()> {
        match &self.jump {
            None => {
                if self.ending.is_none() {
                    return Err(anyhow!(
                        "Lacks `jump` section, but doesn't have an `ending` section"
                    ));
                }
            }
            Some(jump) => {
                if jump.is_validatable() {
                    let file = jump
                        .file
                        .as_ref()
                        .map(|t| t.content.clone())
                        .unwrap_or(local_file.clone());
                    let _ = Prompt::get(prompts, &jump.prompt.content, &file)
                        .with_context(|| "`jump` section points to invalid prompt")?;
                }
            }
        }
        if has_company && self.response.is_none() {
            return Err(anyhow!(
                "Lacks `response` section, but multiple choices are present in prompt"
            ));
        }
        Ok(())
    }

    /// Creates a map of variable entries to use when creating a new [`HistoryEntry`].
    ///
    /// If both the input result and this choice's `variables` key are [`None`], returns none.
    /// Otherwise, returns a combined map based on which inputs are present.
    pub fn create_variable_entries(
        &self,
        input: Option<&VariableInputResult>,
        variables: &Variables,
        text_context: &TextContext,
    ) -> Result<Option<VariableEntries>> {
        let input_entry = input.map(|result| result.to_variable_entry(variables));
        let var_entries = self
            .variables
            .as_ref()
            .map(|vars| VariableEntry::from_map(&vars, variables, text_context))
            .invert()?;
        if input_entry.is_none() && var_entries.is_none() {
            return Ok(None);
        }
        let mut entries = var_entries.unwrap_or(HashMap::new());
        if let Some((name, entry)) = input_entry {
            entries.insert(name.clone(), entry);
        }
        Ok(Some(entries))
    }

    /// Constructs a [`HistoryEntry`] based on this choice object.
    ///
    /// Copies over control flags, the path based on the latest history entry, and notes and variable applications.
    pub fn to_history_entry(
        &self,
        latest: &HistoryEntry,
        input: Option<&VariableInputResult>,
        config: &Manifest,
        variables: &Variables,
        model: &PromptModel,
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
                    .unwrap_or(config.settings.history.locked),
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
                    if requirement.has.get_value(text_context)?
                        != notes.contains(&requirement.name.fill(text_context)?)
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
    pub fn has_jump_to(&self, file: &String, other_name: &String, other_file: &String) -> bool {
        match &self.jump {
            None => false,
            Some(jump) => jump.matches(file, other_name, other_file),
        }
    }
}
