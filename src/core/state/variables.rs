use std::collections::HashMap;

use anyhow::Result;
use serde::{
    de::{
        value::{MapAccessDeserializer, SeqAccessDeserializer},
        Visitor,
    },
    Deserialize, Serialize,
};

use crate::core::{context::TextContext, text::templating::TemplatableString};

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

/// Variable applications whose name values are non-templatable keys.
pub type StaticVariableApplications = HashMap<String, TemplatableString>;

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A variable application that preserves the key-static value-templatable model.
pub struct VariableApplicationContents {
    #[serde(alias = "variable")]
    /// The name of the variable.
    name: TemplatableString,
    /// The value to set variable to.
    value: TemplatableString,
}

pub type VariableApplicationsInner = Vec<VariableApplicationContents>;

impl VariableApplicationContents {
    pub fn from_static(values: StaticVariableApplications) -> VariableApplicationsInner {
        values
            .into_iter()
            .map(|(name, value)| VariableApplicationContents {
                name: name.into(),
                value,
            })
            .collect()
    }
}

pub struct VariableApplicationsVisitor;

impl<'de> Visitor<'de> for VariableApplicationsVisitor {
    type Value = VariableApplicationsInner;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("map or sequence")
    }

    fn visit_map<A>(self, map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let vars = StaticVariableApplications::deserialize(MapAccessDeserializer::new(map))?;
        Ok(VariableApplicationContents::from_static(vars))
    }

    fn visit_seq<A>(self, seq: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        VariableApplicationsInner::deserialize(SeqAccessDeserializer::new(seq))
    }
}

#[derive(Debug)]
pub struct VariableApplications {
    pub applications: VariableApplicationsInner,
}

impl<'de> Deserialize<'de> for VariableApplications {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self {
            applications: deserializer.deserialize_any(VariableApplicationsVisitor)?,
        })
    }
}

impl Serialize for VariableApplications {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self.into_static() {
            Some(values) => values.serialize(serializer),
            None => self.applications.serialize(serializer),
        }
    }
}

impl VariableApplications {
    fn into_static(&self) -> Option<StaticVariableApplications> {
        let result = self
            .applications
            .iter()
            .map(|app| Some((app.name.content()?.to_owned(), app.value.clone())))
            .try_collect()?;
        Some(result)
    }
}

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
            .applications
            .iter()
            .map(|app| {
                let named = NamedVariableEntry::new(
                    app.name.fill(text_context)?,
                    app.value.fill(text_context)?,
                    globals,
                );
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
