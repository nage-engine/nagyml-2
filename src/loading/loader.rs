use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, Cursor, Read, Seek},
};

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use directories::ProjectDirs;
use format_serde_error::SerdeError;
use memmap::Mmap;
use piz::{
    read::{as_tree, DirectoryContents, FileTree},
    ZipArchive,
};
use playback_rs::{Hint, Song};
use result::OptionResultExt;
use serde::de::DeserializeOwned;
use walkdir::WalkDir;

use crate::core::audio::Sounds;

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
    pub fn new<P>(path: Utf8PathBuf, kind: P) -> Option<Self>
    where
        P: AsRef<Utf8Path>,
    {
        let key = path
            .strip_prefix(kind.as_ref())
            .ok()?
            .with_extension("")
            .to_string()
            .replace("\\", "/");
        Some(KeyedPath(key, path))
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

    pub fn from_current_dir() -> Self {
        Self {
            dir: Utf8PathBuf::from("."),
            backend: Backend::Folder,
        }
    }

    pub fn config_dir() -> Result<Utf8PathBuf> {
        ProjectDirs::from("com", "acikek", "nage")
            .map(|dirs| Utf8PathBuf::from_path_buf(dirs.config_dir().to_path_buf()))
            .invert()
            .map_err(|_| anyhow!("Config directory is not valid UTF-8"))?
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

    /// Parses some [`String`] content into a deserializable type.
    pub fn parse<T>(content: String) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let parsed = serde_yaml::from_str::<T>(&content)
            .map_err(|err| SerdeError::new(content.clone(), err))?;
        Ok(parsed)
    }

    fn create_reader<P>(
        archive: &'a ZipArchive,
        tree: &DirectoryContents,
        path: P,
    ) -> Result<Box<dyn Read + Send + 'a>>
    where
        P: AsRef<Utf8Path>,
    {
        let metadata = tree.lookup(path)?;
        let result = archive.read(&metadata)?;
        Ok(result)
    }

    /// Given a path, reads some target file and outputs its contents.
    ///
    /// - For a [`Folder`](Backend::Folder) backend, reads the file using [`std::fs`].
    /// - For a [`Zip`](Backend::Zip) backend, looks up the archived file in the parsed tree.
    ///
    /// If `raw` is `true`, prepends the given path with the relevant directory relative
    /// to the current location.
    fn read_internal<P>(&self, path: P, raw: bool) -> Result<String>
    where
        P: AsRef<Utf8Path>,
    {
        use Backend::*;
        let full = if raw {
            self.get_path(path)
        } else {
            path.as_ref().to_path_buf()
        };
        let result = match &self.backend {
            Folder => std::fs::read_to_string(full)?,
            Zip(archive, tree) => {
                let reader = Self::create_reader(archive, tree, full)?;
                io::read_to_string(reader)?
            }
        };
        Ok(result)
    }

    pub fn read<P>(&self, path: P, raw: bool) -> Result<String>
    where
        P: AsRef<Utf8Path>,
    {
        self.read_internal(&path, raw)
            .with_context(|| format!("{} doesn't exist", path.as_ref()))
    }

    /// Reads a file given a path and deserializes it into the specified type.
    pub fn load<P, T>(&self, path: P, raw: bool) -> Result<T>
    where
        P: AsRef<Utf8Path>,
        T: DeserializeOwned,
    {
        let content = self.read(&path, raw)?;
        Self::parse(content).with_context(|| format!("Failed to parse {}", path.as_ref()))
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
                        Some(KeyedPath::new(file_path, &full)?)
                    })
                    .map(|KeyedPath(key, path)| Ok((key, mapper(path)?)))
                    .collect()
            }
            Zip(_, tree) => {
                let full = self.get_path(&path);
                tree.files()
                    .filter_map(|file| KeyedPath::new(file.path.to_owned().into_owned(), &full))
                    .map(|KeyedPath(key, path)| Ok((key, mapper(path.to_path_buf())?)))
                    .collect()
            }
        }
    }

    /// Iterates over content files, reads them, and combines their content into a [`String`] map.
    pub fn load_raw_content<P>(&self, path: P) -> Result<RawContents>
    where
        P: AsRef<Utf8Path>,
    {
        self.map_content(path, |local| Ok(self.read(local, false)?))
    }

    /// Iterates over content files, deserializes their content, and combines them into a [`Contents`] map.
    pub fn load_content<P, T>(&self, path: P) -> Result<Contents<T>>
    where
        P: AsRef<Utf8Path>,
        T: DeserializeOwned,
    {
        self.map_content(path, |local| Ok(self.load(local, false)?))
    }

    fn load_sound_file<P>(&self, path: P) -> Result<Song>
    where
        P: AsRef<Utf8Path>,
    {
        use Backend::*;
        let result = match self.backend {
            Folder => Song::from_file(path.as_ref(), None),
            Zip(archive, tree) => {
                let mut hint = Hint::new();
                if let Some(extension) = path.as_ref().extension() {
                    hint.with_extension(extension);
                }
                let mut reader = Self::create_reader(archive, tree, path)?;
                let mut cursor = Cursor::new(Vec::new());
                io::copy(&mut reader, &mut cursor)?;
                cursor.seek(io::SeekFrom::Start(0))?;
                Song::new(Box::new(cursor), &hint, None)
            }
        };
        result.map_err(|err| anyhow!(err))
    }

    /// Loads and parses sounds using [`load_sound_file`].
    pub fn load_sounds<P>(&self, path: P) -> Result<Sounds>
    where
        P: AsRef<Utf8Path>,
    {
        self.map_content(path, |local| Ok(self.load_sound_file(local)?))
    }
}
