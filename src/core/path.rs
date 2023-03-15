use anyhow::Result;
use result::OptionResultExt;
use serde::{Deserialize, Serialize};

use crate::text::{templating::TemplatableString, context::TextContext};

use super::player::PathEntry;

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Path {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub file: Option<TemplatableString>,
	pub prompt: TemplatableString
}

impl Path {
	pub fn is_not_templatable(&self) -> bool {
		self.prompt.is_templatable()
			&& self.file.as_ref().map(|t| t.is_templatable()).unwrap_or(false)
	}

	pub fn fill(&self, full: &PathEntry, text_context: &TextContext) -> Result<PathEntry> {
		let file = self.file.as_ref().map(|t| t.fill(text_context))
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