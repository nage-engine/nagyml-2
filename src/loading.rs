use std::{path::{PathBuf, Path}, collections::BTreeMap};

use anyhow::{Result, Context};
use format_serde_error::SerdeError;
use serde::de::DeserializeOwned;
use walkdir::WalkDir;

/// An ordered map of content container names to values within a single file.
pub type ContentFile<T> = BTreeMap<String, T>;
/// An ordered map of file names to content files.
pub type Contents<T> = BTreeMap<String, ContentFile<T>>;
/// An ordered map of file names to their raw content.
pub type RawContents = BTreeMap<String, String>;

/// Handles the loading of content and data through the file system.
pub struct Loader {
	pub dir: PathBuf
}

impl Loader {
	/// Constructs a loader from a base directory.
	/// Any input paths will be inside this directory.
	pub fn new(dir: PathBuf) -> Self {
		Loader { dir }
	}

	/// Gets the path relative to the inside of the base directory.
	fn get_path<P>(&self, path: P) -> PathBuf where P: AsRef<Path> {
		self.dir.join(path)
	}

	/// Parses some [`String`] content into a deserializable type.
	fn parse<T>(content: String) -> Result<T> where T: DeserializeOwned {
		let parsed = serde_yaml::from_str::<T>(&content)
			.map_err(|err| SerdeError::new(content.clone(), err))?;
		Ok(parsed)
	}

	/// Reads a file given a path and deserializes it into the specified type.
	fn load<P, T>(path: P) -> Result<T> where P: AsRef<Path>, T: DeserializeOwned {
		let content = std::fs::read_to_string(&path)
    		.with_context(|| format!("{} doesn't exist", path.as_ref().display()))?;
		Self::parse(content)
    		.with_context(|| format!("Failed to parse {}", path.as_ref().display()))
	}

	/// Returns an iterator over a recursive set of files in a given directory.
	/// 
	/// Each entry in the iterator is a tuple of the actual path and the file "key", 
	/// wherein the key is formatted as `relative/dir/file_name`, without the preceding input directory and file extension.
	fn get_content_iterator<P>(&self, path: P) -> impl Iterator<Item = (String, PathBuf)> where P: AsRef<Path> {
		let full = self.get_path(path);
		let prefix = format!("{}/", full.to_str().unwrap());
		WalkDir::new(full)
			.into_iter()
			.filter_map(|e| e.ok())
			.filter(|e| e.path().is_file())
			.map(move |e| {
				let file_path = e.path().to_path_buf();
				let key_path = file_path.strip_prefix(&prefix).unwrap().with_extension("");
				(key_path.as_os_str().to_str().unwrap().to_owned(), file_path)
			})
	}

	/// Iterates over content files, performs the specified operation on the file path, 
	/// and combines the results into an ordered [`BTreeMap`].
	pub fn map_content<P, T, F>(&self, path: P, mapper: F) -> Result<BTreeMap<String, T>> where P: AsRef<Path>, F: Fn(PathBuf) -> Result<T> {
		self.get_content_iterator(path)
    		.map(|(key, path)| Ok((key, mapper(path)?)))
    		.collect()
	}

	/// Iterates over content files, reads them, and combines their content into a [`String`] map.
	pub fn load_raw_content<P>(&self, path: P) -> Result<RawContents> where P: AsRef<Path> {
		self.map_content(path, |local| Ok(std::fs::read_to_string(local)?))
	}

	/// Iterates over content files, deserializes their content, and combines them into a [`Contents`] map.
	pub fn load_content<P, T>(&self, path: P) -> Result<Contents<T>> where P: AsRef<Path>, T: DeserializeOwned {
		self.map_content(path, |local| Ok(Self::load(local)?))
	}

	/// Reads and parses a single file.
	pub fn load_file<P, T>(&self, path: P) -> Result<T> where P: AsRef<Path>, T: DeserializeOwned {
		Self::load(self.get_path(path))
	}
}

