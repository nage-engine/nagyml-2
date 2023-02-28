use std::{path::PathBuf, collections::{BTreeMap, HashMap}};

use anyhow::{Result, Context};
use format_serde_error::SerdeError;
use serde::de::DeserializeOwned;
use walkdir::WalkDir;

/// An ordered map of content container names to values within a single file.
pub type ContentFile<T> = BTreeMap<String, T>;
/// An orderer map of file names to content files.
pub type Contents<T> = BTreeMap<String, ContentFile<T>>;

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
	let prefix = format!("{path}/");
	WalkDir::new(path)
		.into_iter()
		.filter_map(|e| e.ok())
		.filter(|e| e.path().is_file())
		.map(move |e| {
			let file_path = e.path().to_path_buf();
			let key_path = file_path.strip_prefix(&prefix).unwrap().with_extension("");
			(file_path, key_path.as_os_str().to_str().unwrap().to_owned())
		})
}

/// Iterates over files using [`get_content_iterator`], reads them, and combines their content into a [`String`] map.
pub fn load_files(path: &str) -> Result<HashMap<String, String>> {
	get_content_iterator(path)
    	.map(|(path, key)| Ok((key, std::fs::read_to_string(path)?)))
    	.collect()
}

/// Iterates over content files using [`get_content_iterator`] and combines them into a [`Contents`] map.
pub fn load_content<T>(path: &str) -> Result<Contents<T>> where T: DeserializeOwned {
	get_content_iterator(path)	
		.map(|(path, key)| Ok((key, load(&path.to_path_buf())?)))
		.collect()
}