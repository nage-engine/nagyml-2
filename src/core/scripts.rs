use anyhow::{Result, Context as ContextTrait, anyhow};
use rand::{Rng, thread_rng};
use result::OptionResultExt;
use rlua::{Lua, Context, Table, Function, Chunk};

use crate::loading::{Loader, RawContents};

use super::text::TextContext;

#[derive(Debug)]
/// A container for script files and script running context.
pub struct Scripts {
	pub files: RawContents,
	pub lua: Lua
} 

impl Scripts {
	/// Loads all scripts from the `scripts` directory and creates a new [`Lua`] object.
	pub fn load(loader: &Loader) -> Result<Self> {
		let result = Scripts {
			files: loader.load_raw_content("scripts")?,
			lua: Lua::new()
		};
		Ok(result)
	}

	/// Modifies a Lua [`Context`] to ensure stateful randomness between different loaded contexts.
	fn random_seed(&self, context: &Context) -> Result<(), rlua::Error> {
		let fake_time: u32 = thread_rng().gen();
		context.load(&format!("math.randomseed({fake_time})")).exec()
	}

	/// Adds global values to the specified [`Context`] based on the text context.
	/// 
	/// The following values are added:
	/// - A `notes` sequence based on the player [`Notes`]
	/// - A `variables` table based on the player [`Variables`]
	/// - A `nage` globals table based on the global variables
	/// - An `audio` table mapping channels to their data
	/// 
	/// Player data values do not represent the data itself and are merely snapshots of the data.
	/// Scripts cannot modify data directly and must instead be used in other central systems.
	fn add_globals(&self, context: &Context, text_context: &TextContext) -> Result<(), rlua::Error> {
		let notes_seq = context.create_sequence_from(text_context.notes.clone())?;
		let vars_table = context.create_table_from(text_context.variables.clone())?;
		context.globals().set("notes", notes_seq)?;
		context.globals().set("variables", vars_table)?;
		context.globals().set("nage", text_context.create_variable_table(context)?)?;
		if let Some(audio) = text_context.audio {
			context.globals().set("audio", audio.create_audio_table(context)?)?;
		}
		Ok(())
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

	/// Evaluates a script resource given a filename and text context.
	pub fn get(&self, file: &str, text_context: &TextContext) -> Result<Option<String>> {
		let components = Self::file_components(file);
		let result = self.files.get(components.0).map(|script| {
			self.lua.context(|lua_ctx| {
				self.random_seed(&lua_ctx)?;
				self.add_globals(&lua_ctx, text_context)?;
				let loaded = lua_ctx.load(script);
				Self::eval(loaded, components.1)
					.with_context(|| anyhow!("failed to evaluate script component {file}"))
			})
		});
		Ok(result.invert()?)
	}
}