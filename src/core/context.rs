use rlua::{Context, Table};

use crate::core::{
    manifest::Manifest,
    resources::Resources,
    state::{notes::Notes, variables::Variables},
    text::display::TranslationFile,
};

/// A wrapper for content that is explicitly constant from after the game is loaded until its end.
///
/// This struct is meant to be created once explicitly and then passed around freely
/// between different types or functions where its contents would be useful.
///
/// Its contents are public with the contract that they are unmodifiable.
pub struct StaticContext<'a> {
    pub config: &'a Manifest,
    pub resources: &'a Resources,
}

impl<'a> StaticContext<'a> {
    pub fn new(config: &'a Manifest, resources: &'a Resources) -> Self {
        Self { config, resources }
    }
}

impl<'a> Clone for StaticContext<'a> {
    fn clone(&self) -> Self {
        Self::new(self.config, self.resources)
    }
}

/// A wrapper for all data relevant for filling in [`TemplatableString`]s.
///
/// This struct holds 'snapshots' of mutable player data as well as [`StaticContext`].
///
/// A set of global "nage" variables consistent between both templating and scripts are derived from this context.
/// They are as follows:
/// - `game_name`: The metadata's `name` key
/// - `game_authors`: The metadata's `authors` key, represented as a sequence
/// - `game_version`: The metadata's `version` key
/// - `lang`: The currently loaded language key
pub struct TextContext<'a> {
    stc: StaticContext<'a>,
    lang: String,
    pub notes: Notes,
    pub variables: Variables,
}

impl<'a> TextContext<'a> {
    /// Constructs a new [`TextContext`] object using owned snapshots of player data and a [`StaticContext`] reference.
    ///
    /// The resulting text context does not own the provided [`StaticContext`] reference, rather a new copy based on
    /// the static context [`Clone`] implementation, which preserves the internal references.
    pub fn new(stc: &'a StaticContext, lang: String, notes: Notes, variables: Variables) -> Self {
        TextContext {
            stc: stc.clone(),
            lang,
            notes,
            variables,
        }
    }

    pub fn config(&self) -> &Manifest {
        &self.stc.config
    }

    pub fn resources(&self) -> &Resources {
        &self.stc.resources
    }

    pub fn lang_file(&self) -> Option<&TranslationFile> {
        self.stc.resources.lang_file(&self.lang)
    }

    /// Attempts to fetch a global variable for direct templating.
    /// These variables are prefixed under `nage:`.
    ///
    /// The `game_authors` variable is separated by commas.
    pub fn global_variable(&self, var: &str) -> Option<String> {
        var.to_lowercase()
            .strip_prefix("nage:")
            .map(|name| match name {
                "game_name" => Some(self.stc.config.metadata.name.clone()),
                "game_authors" => Some(self.stc.config.metadata.authors.join(", ")),
                "game_version" => Some(self.stc.config.metadata.version.to_string()),
                "lang" => Some(self.lang.to_owned()),
                _ => None,
            })
            .flatten()
    }

    /// Creates a global variable table for use in scripts.
    /// This should be set as a global `nage` table.
    pub fn create_variable_table<'b>(
        &self,
        context: &Context<'b>,
    ) -> Result<Table<'b>, rlua::Error> {
        let table = context.create_table()?;
        table.set("game_name", self.stc.config.metadata.name.clone())?;
        table.set(
            "game_authors",
            context.create_sequence_from(self.stc.config.metadata.authors.clone())?,
        )?;
        table.set("game_version", self.stc.config.metadata.version.to_string())?;
        table.set("lang", self.lang.clone())?;
        Ok(table)
    }
}

#[macro_export]
macro_rules! text_context {
    ($stc:expr, $player:expr) => {
        TextContext::new(
            $stc,
            $player.lang.clone(),
            $player.notes.clone(),
            $player.variables.clone(),
        )
    };
}
