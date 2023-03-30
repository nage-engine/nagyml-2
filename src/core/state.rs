use std::collections::{HashMap, HashSet};

use anyhow::Result;
use result::OptionResultExt;
use serde::{Deserialize, Serialize};

use crate::text::{
    context::TextContext,
    templating::{TemplatableString, TemplatableValue},
};

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

#[derive(Serialize, Deserialize, Debug)]
pub struct NoteEntry {
    pub value: String,
    pub take: bool,
}

pub type NoteEntries = Vec<NoteEntry>;

impl NoteEntry {
    pub fn new(name: &TemplatableString, take: bool, text_context: &TextContext) -> Result<Self> {
        let entry = NoteEntry {
            value: name.fill(text_context)?,
            take,
        };
        Ok(entry)
    }

    pub fn from_application(app: &NoteApplication, text_context: &TextContext) -> Result<Self> {
        Self::new(&app.name, app.take.get_value(text_context)?, text_context)
    }
}

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

#[derive(Serialize, Deserialize, Debug)]
/// A single variable value recording.
pub struct VariableEntry {
    /// The new variable value.
    pub value: String,
    /// The previous variable value if being overriden.
    pub previous: Option<String>,
}

/// A map of variable names to value recordings.
pub type VariableEntries = HashMap<String, VariableEntry>;

pub struct NamedVariableEntry {
    pub name: String,
    pub entry: VariableEntry,
}

impl Into<(String, VariableEntry)> for NamedVariableEntry {
    fn into(self) -> (String, VariableEntry) {
        (self.name, self.entry)
    }
}

impl NamedVariableEntry {
    pub fn new(name: String, value: String, variables: &Variables) -> Self {
        Self {
            entry: VariableEntry::new(&name, value, variables),
            name,
        }
    }
}

impl VariableEntry {
    pub fn new(name: &str, value: String, variables: &Variables) -> Self {
        VariableEntry {
            value: value.clone(),
            previous: variables.get(name).map(|prev| prev.clone()),
        }
    }

    pub fn from_map(
        applying: &VariableApplications,
        globals: &Variables,
        text_context: &TextContext,
    ) -> Result<VariableEntries> {
        applying
            .iter()
            .map(|(name, value)| {
                let named =
                    NamedVariableEntry::new(name.clone(), value.fill(text_context)?, globals);
                Ok(named.into())
            })
            .collect()
    }
}
