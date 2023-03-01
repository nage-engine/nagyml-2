use std::collections::HashMap;

use anyhow::{Result, Context as ContextTrait, anyhow};
use rand::{Rng, thread_rng};
use result::OptionResultExt;
use rlua::{Lua, Context, Table, Function, Chunk};

use crate::loading::load_files;

use super::choice::{Notes, Variables};

#[derive(Debug)]
/// A container for script files and script running context.
pub struct Scripts {
	pub files: HashMap<String, String>,
	pub lua: Lua
} 

impl Scripts {
	/// Loads all scripts from the `scripts` directory and creates a new [`Lua`] object.
	pub fn load() -> Result<Self> {
		let result = Scripts {
			files: load_files("scripts")?,
			lua: Lua::new()
		};
		Ok(result)
	}

	/// Modifies a Lua [`Context`] to ensure stateful randomness between different loaded contexts.
	fn random_seed(&self, context: &Context) -> Result<(), rlua::Error> {
		let fake_time: u32 = thread_rng().gen();
		context.load(&format!("math.randomseed({fake_time})")).exec()
	}

	/// Adds global values to the specified [`Context`] based on the player data.
	/// 
	/// The following values are added:
	/// - A `notes` sequence based on the player [`Notes`]
	/// - A `variables` table based on the player [`Variables`]
	/// 
	/// These values do not represent the player data itself and are merely snapshots of the data.
	/// Scripts cannot modify data directly and must instead be used in other central systems.
	fn add_globals(&self, context: &Context, notes: &Notes, variables: &Variables) -> Result<(), rlua::Error> {
		let notes_seq = context.create_sequence_from(notes.clone())?;
		let vars_table = context.create_table_from(variables.clone())?;
		context.globals().set("notes", notes_seq)?;
		context.globals().set("variables", vars_table)
	}

	/// Given a file string, splits it based on the function delimiter character `:`.
	/// If there is no function delimiter, returns only the file name.
	fn file_components(file: &str) -> (&str, Option<&str>) {
		let components = file.split_once(":");
		match components {
			Some((f, func)) => (f, Some(func)),
			None => (file, None)
		}
	}

	/// Given a loaded Lua chunk, and an optional function name, evaluates the result.
	fn eval(loaded: Chunk, func: Option<&str>) -> Result<String, rlua::Error> {
		match func {
			Some(func) => {
				let table: Table = loaded.eval()?;
				let value: Function = table.get(func)?;
				value.call(())
			},
			None => loaded.eval()
		}
	}

	/// Evaluates a script resource given a filename and some player data context properties.
	pub fn get(&self, file: &str, notes: &Notes, variables: &Variables) -> Result<Option<String>> {
		let components = Self::file_components(file);
		let result = self.files.get(components.0).map(|script| {
			self.lua.context(|lua_ctx| {
				self.random_seed(&lua_ctx)?;
				self.add_globals(&lua_ctx, notes, variables)?;
				let loaded = lua_ctx.load(script);
				Self::eval(loaded, components.1)
					.with_context(|| anyhow!("failed to evaluate script component {file}"))
			})
		});
		Ok(result.invert()?)
	}
}