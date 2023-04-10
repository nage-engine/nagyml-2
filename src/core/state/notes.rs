use std::collections::HashSet;

use anyhow::Result;
use result::OptionResultExt;
use serde::{
    de::{value::MapAccessDeserializer, Visitor},
    Deserialize, Serialize,
};

use crate::core::{
    context::TextContext,
    text::templating::{TemplatableString, TemplatableValue},
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
    pub fn to_note_entries(
        &self,
        once: Option<String>,
        text_context: &TextContext,
    ) -> Result<NoteEntries> {
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

        if let Some(once_value) = once {
            entries.push(NoteEntry::new(once_value, false));
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
    pub fn new(value: String, take: bool) -> Self {
        NoteEntry { value, take }
    }

    pub fn from_application(app: &NoteStateContents, text_context: &TextContext) -> Result<Self> {
        Ok(Self::new(app.name.fill(text_context)?, !app.get_state(text_context)?))
    }
}
