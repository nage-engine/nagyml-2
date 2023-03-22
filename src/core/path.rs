use anyhow::Result;
use result::OptionResultExt;
use serde::{Deserialize, Serialize};

use crate::text::{context::TextContext, templating::TemplatableString};

use super::player::PathEntry;

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Path {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<TemplatableString>,
    pub prompt: TemplatableString,
}

impl Path {
    /// Whether this path is validatable (not templatable).
    ///
    /// Both components of the path **must not** be templatable in order to qualify for validation.
    ///
    /// If the path has a templatable file and a non-templatable prompt, the prompt key cannot be validated
    /// since the other file's prompts cannot be determined.
    ///
    /// If the path does not have a file, then this call is only concerned with whether the prompt key
    /// is templatable.
    pub fn is_validatable(&self) -> bool {
        self.file
            .as_ref()
            .map(|t| !t.is_templatable())
            .unwrap_or(true)
            && !self.prompt.is_templatable()
    }

    pub fn fill(&self, full: &PathEntry, text_context: &TextContext) -> Result<PathEntry> {
        let file = self
            .file
            .as_ref()
            .map(|t| t.fill(text_context))
            .invert()?
            .unwrap_or(full.file.clone());
        Ok(PathEntry {
            file,
            prompt: self.prompt.fill(text_context)?,
        })
    }

    pub fn matches(&self, file: &String, other_name: &String, other_file: &String) -> bool {
        let pointing_file = self.file.as_ref().map(|t| &t.content).unwrap_or(file);
        self.prompt.content.eq(other_name) && pointing_file.eq(other_file)
    }
}
