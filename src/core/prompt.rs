use std::{collections::{BTreeMap, HashMap}, fmt::Display};

use anyhow::{Result, Context, anyhow};
use serde::{Serialize, Deserialize};
use walkdir::WalkDir;

use crate::loading::load;

use super::{text::{TextLines, Text}, choice::{Choices, Variables, Choice, Notes}, path::Path, manifest::Manifest};

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
/// The standard gameplay container to which a player visits during a playthrough.
/// 
/// When a player visits a prompt, they are optionally given some introductory text (a "text prompt"). The player then is given a list of choices, each jumping to a new prompt or ending the game.
pub struct Prompt {
	#[serde(rename = "prompt")]
	pub text: Option<TextLines>,
	pub choices: Choices
}

#[derive(Debug)]
/// A prompt's overarching function based on its choices.
pub enum PromptModel<'a> {
	/// Has one choice. This choice has an `input` field.
	Input(&'a String, Option<&'a String>),
	/// A normal prompt-choice container model.
	Response,
	/// Has one choice. This choice lacks response or input; immediately jumps to another prompt.
	Redirect(&'a Choice),
	/// Has one choice. This choice ends the game.
	Ending(&'a TextLines)
}

impl<'a> Display for PromptModel<'a> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		write!(f, "Model: {}", self.description())
	}
}

impl<'a> PromptModel<'a> {
	/// The prompt model's readable description for debug purposes.
	/// 
	/// Outputs the model's name and then its function.
	pub fn description(&self) -> String {
		use PromptModel::*;
		match self {
			Input(name, _) => format!("Input; takes user input for the variable '{name}'"),
			Response => "Response; standard prompt-choice model".to_owned(),
			Redirect(_) => "Redirect; jumps to another prompt without input".to_owned(),
			Ending(_) => "Ending; the game is forced to end".to_owned()
		}
	}
}

/// A map of prompt names to prompt containers within a single file.
pub type PromptFile = BTreeMap<String, Prompt>;

/// A map of file names to prompt files.
pub type Prompts = BTreeMap<String, PromptFile>;

impl Prompt {
	/// Loads a [`PromptFile`] using [`load`] and returns a tuple of the file key and the loaded content.
	/// 
	/// For example, the path `prompts/dir/file.yml` would yield the key `dir/file`.
	fn load(path: &std::path::Path) -> Result<(String, PromptFile)> {
		let key_path = path.strip_prefix("prompts/").unwrap().with_extension("");
		let key = key_path.as_os_str().to_str().unwrap().to_owned();
		let loaded = load(&path.to_path_buf())?;
		Ok((key, loaded))
	}

	/// Recursively walks, loads, and collects all [`PromptFile`]s from a local `prompts` directory into a [`Prompts`] object using [`load_prompt`].
	pub fn load_all() -> Result<Prompts> {
		WalkDir::new("prompts")
			.into_iter()
			.filter_map(|e| e.ok())
			.filter(|e| e.path().is_file())
			.map(|e| Self::load(e.path()))
			.collect()
	}

	/// Finds a specific prompt file within a [`Prompts`] object.
	pub fn get_file<'a>(prompts: &'a Prompts, file: &String) -> Result<&'a PromptFile> {
		prompts.get(file)
			.ok_or(anyhow!("Invalid prompt file '{file}'"))
	}

	/// Finds a specific prompt within a [`Prompts`] object.
	pub fn get<'a>(prompts: &'a Prompts, name: &String, file: &String) -> Result<&'a Prompt> {
		Self::get_file(prompts, file)
			.map(|prompt_file| {
				prompt_file.get(name).ok_or(anyhow!("Invalid prompt '{name}'; not found in file '{file}'"))
			})
			.flatten()
	}

	/// Uses [`Prompt::get`] to find a specific prompt.
	/// 
	/// The input path **must** have a `file` key.
	pub fn get_from_path<'a>(prompts: &'a Prompts, path: &Path) -> Result<&'a Prompt> {
		let file = path.file.as_ref().ok_or(anyhow!("Path must have a 'file' key"))?;
		Self::get(prompts, &path.prompt, file)
	}

	/// Validates this prompt's choices using [`Choice::validate`].
	pub fn validate(&self, name: &String, file: &String, prompts: &Prompts) -> Result<()> {
		let has_company = self.choices.len() > 1;
		// Validate all independent choices
		self.choices.iter().enumerate()
			.map(|(index, choice)| {
				choice.validate(file, has_company, prompts)
					.with_context(|| format!("Error when validating choice #{} of prompt '{name}' in file '{file}'", index + 1))
			})
			.collect()
	}

	/// Returns the [`PromptModel`] based on this prompt's choices. See the enum's fields for criteria.
	pub fn model(&self) -> PromptModel {
		use PromptModel::*;
		if self.choices.len() == 1 {
			let choice = &self.choices[0];
			if let Some(input) = &choice.input {
				return Input(&input.name, input.text.as_ref());
			}
			else if choice.response.is_none() {
				if let Some(ending) = &choice.ending {
					return Ending(ending);
				}
				return Redirect(choice);
			}
		}
		Response
	}

	/// Gathers all choices that a player can use based on 
	pub fn usable_choices(&self, notes: &Notes) -> Vec<&Choice> {
		self.choices.iter()
			.filter(|choice| choice.can_player_use(notes))
			.collect()
	}

	/// Prints the prompt text, if any, and the choices display, if any are responses.
	pub fn print(&self, model: &PromptModel, display: bool, usable_choices: &Vec<&Choice>, variables: &Variables, config: &Manifest) {
		if display {
			if let Some(lines) = &self.text {
				Text::print_lines_nl(lines, variables, config);
			}
		}
		if let PromptModel::Response = model {
			println!("{}\n", Choice::display(usable_choices, variables));
		}
	}

	/// Returns the indices of any of this prompt's choices that jump to another prompt.
	/// 
	/// Uses [`Choice::has_jump_to`].
	pub fn get_jumps_to(&self, file: &String, other_name: &String, other_file: &String) -> Vec<usize> {
		self.choices.iter().enumerate()
    		.filter(|(_, choice)| choice.has_jump_to(file, other_name, other_file))
    		.map(|(index, _)| index)
			.collect()
	}

	/// Finds all prompts that have choices that jump to a specific prompt name and file.
	/// 
	/// Uses [`Prompt::get_jumps_to`] to find the indices of the choices, if any.
	pub fn external_jumps<'a>(name: &String, file: &String, prompts: &'a Prompts) -> HashMap<String, Vec<usize>> {
		prompts.iter()
    		.map(|(other_file_name, prompt_file)| {
				prompt_file.iter()
					.map(|(other_prompt_name, other_prompt)| {
						let id = format!("{}/{}", other_file_name.clone(), other_prompt_name.clone());
						(id, other_prompt.get_jumps_to(other_file_name, name, file))
					})
    				.filter(|(_, choices)| !choices.is_empty())
			})
    		.flatten()
    		.collect()
	}

	pub fn debug_info(&self, name: &String, file: &String, prompts: &Prompts, notes: &Notes) -> String {
		let model = self.model();
		let choices_amt = self.choices.len();
		let usable_choices = self.usable_choices(notes).len();
		let external_jumps: Vec<String> = Self::external_jumps(name, file, prompts).iter()
    		.map(|(other_id, choices)| {
				let indices: Vec<String> = choices.iter().map(|i| format!("#{}", i + 1)).collect();
				format!("- {other_id}: {}", indices.join(", "))
			})
    		.collect();
		let id_and_model = format!("ID: {file}/{name}\n{model}");
		let choices = format!("{choices_amt} choice(s)\n{usable_choices} of them accessible");
		let jumps = format!("Prompts that jump here:\n{}", external_jumps.join("\n"));
		format!("\n{id_and_model}\n\n{choices}\n\n{jumps}")
	}
}