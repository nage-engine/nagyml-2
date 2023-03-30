use std::fmt::Display;

use anyhow::Result;
use result::OptionResultExt;
use serde::{
    de::{value::MapAccessDeserializer, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::text::{context::TextContext, templating::TemplatableString};

#[derive(Serialize, Deserialize, Debug)]
pub struct PathContents {
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<TemplatableString>,
    prompt: TemplatableString,
}

#[derive(Serialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Path {
    contents: PathContents,
}

struct PathVisitor;

impl<'de> Visitor<'de> for PathVisitor {
    type Value = PathContents;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("string or map")
    }

    fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        // rsplit: some/file/prompt -> some/file + prompt
        let result = match v.rsplit_once('/') {
            Some((file, prompt)) => PathContents {
                file: Some(file.to_owned().into()),
                prompt: prompt.to_owned().into(),
            },
            None => PathContents {
                file: None,
                prompt: v.to_owned().into(),
            },
        };
        Ok(result)
    }

    fn visit_map<A>(self, map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        Deserialize::deserialize(MapAccessDeserializer::new(map))
    }
}

impl<'de> Deserialize<'de> for Path {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self {
            contents: deserializer.deserialize_any(PathVisitor)?,
        })
    }
}

impl Path {
    pub fn file(&self) -> &Option<TemplatableString> {
        &self.contents.file
    }

    pub fn prompt(&self) -> &TemplatableString {
        &self.contents.prompt
    }

    /// Whether this path is validatable.
    ///
    /// **Both** components of the path must be validatable in order to qualify as a whole.
    ///
    /// If the path has a templatable file and a non-templatable prompt, the prompt key cannot be validated
    /// since the other file's prompts cannot be determined.
    ///
    /// If the path does not have a file, then this call is only concerned with whether the prompt key
    /// is templatable.
    pub fn is_validatable(&self) -> bool {
        self.file()
            .as_ref()
            .map(|t| t.is_validatable())
            .unwrap_or(true)
            && self.prompt().is_validatable()
    }

    pub fn fill(&self, current: &PathData, text_context: &TextContext) -> Result<PathData> {
        let file = self
            .file()
            .as_ref()
            .map(|t| t.fill(text_context))
            .invert()?
            .unwrap_or(current.file.clone());
        Ok(PathData {
            file,
            prompt: self.prompt().fill(text_context)?,
        })
    }

    pub fn static_file(&self, current_file: &str) -> Option<String> {
        if !self.is_validatable() {
            return None;
        }
        let result = self
            .file()
            .as_ref()
            .map(|t| t.content.clone())
            .unwrap_or(current_file.to_owned());
        Some(result)
    }

    fn static_data(&self, current_file: &str) -> Option<PathData> {
        self.static_file(&current_file).map(|file| PathData {
            file,
            prompt: self.prompt().content.clone(),
        })
    }

    pub fn matches(&self, current_file: &str, other: &PathData) -> bool {
        match self.static_data(current_file) {
            Some(data) => data.prompt == other.prompt && data.file == other.file,
            None => false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PathData {
    pub file: String,
    pub prompt: String,
}

impl Display for PathData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.file, self.prompt)
    }
}

pub struct PathLookup<'a> {
    file: &'a str,
    prompt: &'a str,
}

impl<'a> Into<PathData> for PathLookup<'a> {
    fn into(self) -> PathData {
        PathData {
            file: self.file.to_owned(),
            prompt: self.prompt.to_owned(),
        }
    }
}

impl<'a> PathLookup<'a> {
    pub fn new(file: &'a str, prompt: &'a str) -> Self {
        PathLookup { file, prompt }
    }
}
