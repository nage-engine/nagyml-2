use std::{collections::BTreeMap, fs::File, io, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Component, Utf8Path, Utf8PathBuf};
use directories::ProjectDirs;
use format_serde_error::SerdeError;
use memmap::Mmap;
use piz::{
    read::{as_tree, DirectoryContents, FileMetadata, FileTree},
    ZipArchive,
};
use result::OptionResultExt;
use serde::de::DeserializeOwned;
use walkdir::WalkDir;

/// An ordered map of content container names to values within a single file.
pub type ContentFile<T> = BTreeMap<String, T>;
/// An ordered map of file names to content files.
pub type Contents<T> = BTreeMap<String, ContentFile<T>>;
/// An ordered map of file names to their raw content.
pub type RawContents = BTreeMap<String, String>;

/// Handles the loading of content and data through the file system.
pub enum Backend<'a> {
    Folder,
    Zip(&'a ZipArchive<'a>, &'a DirectoryContents<'a>),
}

pub struct Loader<'a> {
    dir: Utf8PathBuf,
    backend: Backend<'a>,
}

pub struct KeyedPath(String, Utf8PathBuf);

impl KeyedPath {
    pub fn new<P>(path: Utf8PathBuf, kind: P) -> Self
    where
        P: AsRef<Utf8Path>,
    {
        let key = path
            .strip_prefix(kind.as_ref())
            .unwrap()
            .with_extension("")
            .to_string()
            .replace("\\", "/");
        KeyedPath(key, path)
    }
}

impl<'a> Loader<'a> {
    pub fn mapping(target: &Utf8PathBuf) -> Result<Option<Vec<u8>>> {
        if let Some(ext) = target.extension() {
            if ext == "zip" {
                let file = File::open(target)?;
                let mapping = unsafe { Mmap::map(&file)? }.to_owned();
                return Ok(Some(mapping));
            }
        }
        Ok(None)
    }

    pub fn archive(mapping: &'a Option<Vec<u8>>) -> Result<Option<ZipArchive<'a>>> {
        let result = mapping.as_ref().map(|m| ZipArchive::new(m)).invert()?;
        Ok(result)
    }

    pub fn tree(archive: &'a Option<ZipArchive<'a>>) -> Result<Option<DirectoryContents<'a>>> {
        let result = archive.as_ref().map(|a| as_tree(a.entries())).invert()?;
        Ok(result)
    }

    fn backend(
        target: &Utf8PathBuf,
        archive: &'a Option<ZipArchive<'a>>,
        tree: &'a Option<DirectoryContents<'a>>,
    ) -> Result<Backend<'a>> {
        use Backend::*;
        if target.is_dir() {
            return Ok(Folder);
        }
        if let Some(ctx) = archive {
            return Ok(Zip(ctx, tree.as_ref().unwrap()));
        }
        Err(anyhow!("Unrecognized game type"))
    }

    /// Constructs a loader from a base directory.
    /// Any input paths will be inside this directory.
    pub fn new(
        target: Utf8PathBuf,
        archive: &'a Option<ZipArchive<'a>>,
        tree: &'a Option<DirectoryContents<'a>>,
    ) -> Result<Self> {
        let backend = Self::backend(&target, archive, tree)?;
        let result = Self {
            dir: target,
            backend,
        };
        Ok(result)
    }

    pub fn current_dir() -> Self {
        Self {
            dir: Utf8PathBuf::from("."),
            backend: Backend::Folder,
        }
    }

    pub fn config_dir() -> Result<PathBuf> {
        ProjectDirs::from("com", "acikek", "nage")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .ok_or(anyhow!("Failed to resolve config directory"))
    }

    /// Gets the path relative to the inside of the base directory.
    pub fn get_path<P>(&self, path: P) -> Utf8PathBuf
    where
        P: AsRef<Utf8Path>,
    {
        use Backend::*;
        match &self.backend {
            Folder => self.dir.join(path),
            Zip(_, _) => Utf8PathBuf::from(self.dir.file_stem().unwrap()).join(path),
        }
    }

    // Provided by zip reader: game/prompts/abc.yml
    // Want to read that
    // Want to parse to prompts/abc
    // Want to know it's from prompts
    // At the same time, need nage.yml to be game/nage.yml

    /// Parses some [`String`] content into a deserializable type.
    pub fn parse<T>(content: String) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let parsed = serde_yaml::from_str::<T>(&content)
            .map_err(|err| SerdeError::new(content.clone(), err))?;
        Ok(parsed)
    }

    fn read_internal<P>(&self, path: P) -> Result<String>
    where
        P: AsRef<Utf8Path>,
    {
        use Backend::*;
        let result = match &self.backend {
            Folder => std::fs::read_to_string(path.as_ref())?,
            Zip(archive, tree) => {
                let metadata = tree.lookup(path)?;
                let reader = archive.read(&metadata)?;
                io::read_to_string(reader)?
            }
        };
        Ok(result)
    }

    pub fn read<P>(&self, path: P) -> Result<String>
    where
        P: AsRef<Utf8Path>,
    {
        self.read_internal(&path).with_context(|| format!("{} doesn't exist", path.as_ref()))
    }

    /// Reads a file given a path and deserializes it into the specified type.
    pub fn load<P, T>(&self, path: P) -> Result<T>
    where
        P: AsRef<Utf8Path>,
        T: DeserializeOwned,
    {
        let content = self.read(&path)?;
        Self::parse(content).with_context(|| format!("Failed to parse {}", path.as_ref()))
    }

    /// Converts a [`FileMetadata`] reference into a [`KeyedPath`] if it holds the correct content type.
    /// Errors are ignored but result in a [`None`] value.
    ///
    /// All archived content files start with the archive's name.
    /// It is ideal to be able to read those files from an input that does not begin with that name.
    /// Therefore, this conversion strips the archive name from the path and allows it to be added on when reading.
    fn archived_content_file<P>(file: &FileMetadata, kind: P) -> Option<KeyedPath>
    where
        P: AsRef<Utf8Path>,
    {
        let components: Vec<Utf8Component> = file.path.components().collect();
        let archive_name = components[0];
        let kind_comp = components.get(1)?;
        if kind_comp.as_str() != kind.as_ref() {
            return None;
        }
        let stripped = file.path.strip_prefix(archive_name).ok()?.to_owned();
        Some(KeyedPath::new(stripped, kind))
    }

    /// Iterates over content files, performs the specified operation on the file path,
    /// and combines the results into an ordered [`BTreeMap`].
    pub fn map_content<P, T, F>(&self, path: P, mapper: F) -> Result<BTreeMap<String, T>>
    where
        P: AsRef<Utf8Path>,
        F: Fn(Utf8PathBuf) -> Result<T>,
    {
        use Backend::*;
        match &self.backend {
            Folder => {
                let full = self.get_path(&path);
                WalkDir::new(&full)
                    .into_iter()
                    .filter_map(|e| e.ok())
                    .filter(|e| e.path().is_file())
                    .filter_map(move |e| {
                        let file_path = Utf8PathBuf::from_path_buf(e.path().to_path_buf()).ok()?;
                        Some(KeyedPath::new(file_path, &full))
                    })
                    .map(|KeyedPath(key, path)| Ok((key, mapper(path)?)))
                    .collect()
            }
            Zip(_, tree) => tree
                .files()
                .filter_map(|file| Self::archived_content_file(file, &path))
                .map(|KeyedPath(key, path)| Ok((key, mapper(path.to_path_buf())?)))
                .collect(),
        }
    }

    /// Iterates over content files, reads them, and combines their content into a [`String`] map.
    pub fn load_raw_content<P>(&self, path: P) -> Result<RawContents>
    where
        P: AsRef<Utf8Path>,
    {
        self.map_content(path, |local| Ok(self.read(local)?))
    }

    /// Iterates over content files, deserializes their content, and combines them into a [`Contents`] map.
    pub fn load_content<P, T>(&self, path: P) -> Result<Contents<T>>
    where
        P: AsRef<Utf8Path>,
        T: DeserializeOwned,
    {
        self.map_content(path, |local| Ok(self.load(local)?))
    }
}
