use anyhow::{Result, anyhow};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Path {
	pub file: Option<String>,
	pub prompt: String
}

impl Path {
	pub fn fill(&self, full: &Path) -> Result<Path> {
		let file = self.file.as_ref().unwrap_or(
			full.file.as_ref().ok_or(anyhow!("Path must have a 'file' entry"))?
		);
		Ok(Path {
			prompt: self.prompt.clone(),
			file: Some(file.clone())
		})
	}
}