use std::collections::{HashMap, HashSet};

use anyhow::Result;
use result::OptionResultExt;
use serde::{
    de::{value::MapAccessDeserializer, Visitor},
    Deserialize, Serialize,
};

use crate::text::{
    context::TextContext,
    templating::{TemplatableString, TemplatableValue},
};

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// The internal state of a note action.
///
/// For backwards compatibility purposes, this structure supports state values
/// that can be aligned and non-aligned in positive value.
///
/// For example, `has` is aligned with [`Require`](NoteAction::Require).
/// If `has` is `true`, the note is required. With the `take` value for [`Apply`](NoteAction::Apply), this is the opposite.
pub struct NoteStateContents {
    /// The note name to apply or require.
    pub name: TemplatableString,
    #[serde(default, rename = "apply", alias = "has")]
    /// The **aligned** value that corresponds to the [`NoteAction`] type.
    pub state: Option<TemplatableValue<bool>>,
    #[serde(default, rename = "take", alias = "deny")]
    /// The **non-aligned** value that is the inverse of the [`NoteAction`] type.
    pub inverse: Option<TemplatableValue<bool>>,
}

#[derive(Debug)]
/// A wrapper for [`NoteStateContents`].
pub struct NoteState {
    pub state: NoteStateContents,
}

pub type NoteStates = Vec<NoteState>;

struct NoteStateVisitor;

impl<'de> Visitor<'de> for NoteStateVisitor {
    type Value = NoteStateContents;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("string or map")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let (name, state) = match v.strip_suffix('!') {
            Some(after) => (after, false),
            None => (v, true),
        };
        Ok(NoteStateContents {
            name: name.to_owned().into(),
            state: Some(TemplatableValue::value(state)),
            inverse: None,
        })
    }

    fn visit_map<A>(self, map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        Deserialize::deserialize(MapAccessDeserializer::new(map))
    }
}

impl<'de> Deserialize<'de> for NoteState {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self {
            state: deserializer.deserialize_any(NoteStateVisitor)?,
        })
    }
}

impl Serialize for NoteState {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Serialize::serialize(&self.state, serializer)
    }
}

/// The type of action linked to [`NoteStates`].
/*pub enum NoteAction {
    /// Gives or takes notes from the player.
    Apply,
    /// Requires that a player has or doesn't have notes.
    Require,
}

impl NoteAction {
    pub fn default_state(&self) -> bool {
        use NoteAction::*;
        match self {
            Apply => false,  // take: false
            Require => true, // has: true
        }
    }
}*/

impl NoteStateContents {
    pub fn get_state(&self, text_context: &TextContext) -> Result<bool> {
        if let Some(state) = &self.state {
            return state.get_value(text_context);
        }
        if let Some(inverse) = &self.inverse {
            let inv = inverse.get_value(text_context)?;
            return Ok(!inv);
        }
        Ok(true)
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A collection of actions that apply or check against the player's note state.
pub struct NoteActions {
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Actions that apply state to the player.
    pub apply: Option<NoteStates>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Actions that check the player's state.
    pub require: Option<NoteStates>,
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
                    .map(|app| NoteEntry::from_application(&app.state, text_context))
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

    pub fn from_application(app: &NoteStateContents, text_context: &TextContext) -> Result<Self> {
        Self::new(&app.name, app.get_state(text_context)?, text_context)
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

impl VariableEntry {
    pub fn new(name: &str, value: String, variables: &Variables) -> Self {
        Self {
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
