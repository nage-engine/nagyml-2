use std::path::PathBuf;

use anyhow::{Result, Context};
use format_serde_error::SerdeError;
use serde::de::DeserializeOwned;

/// Parses some [`String`] content into a deserializable type.
pub fn parse<T>(path: &str, content: &String) -> Result<T> where T: DeserializeOwned {
	let parsed = serde_yaml::from_str::<T>(&content)
		.map_err(|err| SerdeError::new(content.clone(), err))
		.with_context(|| format!("Failed to parse {path}"))?; //.expect(&format!("Failed to parse file {path}"))
	Ok(parsed)
}

/// Reads a file given a path and deserializes it into the specified type.
pub fn load<T>(path: &PathBuf) -> Result<T> where T: DeserializeOwned {
	let content = std::fs::read_to_string(path)?;
	parse(path.as_os_str().to_str().unwrap(), &content)
}