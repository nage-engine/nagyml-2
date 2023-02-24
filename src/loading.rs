use std::path::PathBuf;

use anyhow::{Result, Context};
use format_serde_error::SerdeError;
use serde::de::DeserializeOwned;
use walkdir::WalkDir;

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

/// Returns an iterator over a recursive set of files in a given directory.
/// 
/// Each entry in the iterator is a tuple of the actual path and the file "key", 
/// wherein the key is formatted as `relative/dir/file_name`, without the preceding input directory and file extension.
pub fn get_content_iterator(path: &str) -> impl Iterator<Item = (PathBuf, String)> {
	WalkDir::new(path)
		.into_iter()
		.filter_map(|e| e.ok())
		.filter(|e| e.path().is_file())
		.map(|e| {
			let path = e.path().to_path_buf();
			let key_path = path.strip_prefix("prompts/").unwrap().with_extension("");
			(path, key_path.as_os_str().to_str().unwrap().to_owned())
		})
}