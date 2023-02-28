use std::{collections::{HashMap, HashSet}};

use crate::input::controller::VariableInputResult;

use super::{text::{Text, TextLines, TemplatableString, TextContext}, path::Path, prompt::{Prompts, Prompt}, player::{HistoryEntry, VariableEntry, VariableEntries}, manifest::Manifest};

use anyhow::{Result, anyhow, Context};
use serde::{Serialize, Deserialize};

pub fn default_true() -> bool { true }

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct NoteApplication {
	pub name: String,
	#[serde(default)]
	pub take: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct NoteRequirement {
	name: String,
	#[serde(default = "default_true")]
	has: bool
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct NoteActions {
	pub apply: Option<Vec<NoteApplication>>,
	require: Option<Vec<NoteRequirement>>,
	pub once: Option<String>
}

/// A list of string symbols tracked on a player.
pub type Notes = HashSet<String>;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct VariableInput {
	pub text: Option<TemplatableString>,
	#[serde(rename = "variable")]
	pub name: String
}

/// A map of display variables wherein the key is the variable name and the value is the variable's display.
pub type Variables = HashMap<String, String>;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Choice {
	pub response: Option<Text>,
	tag: Option<TemplatableString>,
	pub input: Option<VariableInput>,
	pub jump: Option<Path>,
	#[serde(default = "default_true")]
	pub display: bool,
	// This is an option for easier defaulting to the config state
	pub lock: Option<bool>,
	pub notes: Option<NoteActions>,
	pub variables: Option<Variables>,
	pub log: Option<TemplatableString>,
	pub ending: Option<TextLines>
}

pub type Choices = Vec<Choice>;

impl Choice {
	/// Validates a choice amongst the global prompt context.
	/// 
	/// A choice is valid if:
	/// - It has either a `jump` or `ending` section
	/// - Its `jump` section points to a valid prompt
	/// 	- The `file` key has to exist and the `prompt` key has to exist in that [`PromptFile`]
	/// - It has a `response` section if there is more than one choice in the prompt
	pub fn validate(&self, local_file: &String, has_company: bool, prompts: &Prompts) -> Result<()> {
		match &self.jump {
			None => if self.ending.is_none() {
				return Err(anyhow!("Lacks `jump` section, but doesn't have an `ending` section"))
			},
			Some(jump) => {
				let file = jump.file.clone().unwrap_or(local_file.clone());
				let _ = Prompt::get(prompts, &jump.prompt, &file)
        			.with_context(|| "`jump` section points to invalid prompt")?;
			},
		}
		if has_company && self.response.is_none() {
			return Err(anyhow!("Lacks `response` section, but multiple choices are present in prompt"))
		}
		Ok(())
	}

	/// Creates a map of variable entries to use when creating a new [`HistoryEntry`].
	/// 
	/// If both the input result and this choice's `variables` key are [`None`], returns none.
	/// Otherwise, returns a combined map based on which inputs are present.
	pub fn create_variable_entries(&self, input: Option<&VariableInputResult>, variables: &Variables) -> Option<VariableEntries> {
		let input_entry = input.map(|result| result.to_variable_entry(variables));
		let var_entries = self.variables.clone().map(|vars| VariableEntry::from_map(&vars, variables));
		if input_entry.is_none() && var_entries.is_none() {
			return None;
		}
		let mut entries = var_entries.unwrap_or(HashMap::new());
		if let Some((name, entry)) = input_entry {
			entries.insert(name.clone(), entry);
		}
		Some(entries)
	}

	/// Constructs a [`HistoryEntry`] based on this choice object. 
	/// 
	/// Copies over control flags, the path based on the latest history entry, and notes and variable applications.
	pub fn to_history_entry(&self, latest: &HistoryEntry, input: Option<&VariableInputResult>, variables: &Variables, config: &Manifest) -> Option<Result<HistoryEntry>> {
		self.jump.as_ref().map(|jump| {
			Ok(HistoryEntry {
				path: jump.fill(&latest.path)?,
				display: self.display,
				locked: self.lock.unwrap_or(config.settings.history.locked),
				notes: self.notes.clone().map(|actions| actions.apply).flatten(),
				variables: self.create_variable_entries(input, variables)
			})
		})
	}

	/// Determines if a player can use this choice.
	/// 
	/// This check passes if:
	/// - All note requirement `has` fields match the state of the provided [`Notes`] object, and
	/// - The notes object does not contain the `once` value, if any is present
	pub fn can_player_use(&self, notes: &Notes) -> bool {
		if let Some(actions) = &self.notes {
			if let Some(require) = &actions.require {
				for requirement in require {
					if requirement.has != notes.contains(&requirement.name) {
						return false;
					}
				}
			}
			if let Some(once) = &actions.once {
				if notes.contains(once) {
					return false;
				}
			}
		}
		true
	}

	/// Fills in and formats tag content, if any.
	/// 
	/// If [`Some`], returns `[VALUE] `, trailing space included.
	/// If [`None`], returns an empty [`String`].
	fn tag(&self, text_context: &TextContext) -> String {
		self.tag.as_ref()
			.map(|s| format!("[{}] ", s.fill(text_context)))
			.unwrap_or(String::new())
	}

	/// Constructs the response line for display in the game's runtime.
	/// 
	/// ### Examples
	/// 
	/// - `1) [ROGUE] "Ain't no thief."`
	/// - `2) Put down the sword`
	fn response_line(&self, index: usize, text_context: &TextContext) -> String {
		let tag = self.tag(text_context);
		let response = self.response.as_ref().unwrap().get(text_context);
		format!("{index}) {tag}{response}")
	}

	/// Constructs a [`String`] of ordered choice responses.
	pub fn display(choices: &Vec<&Choice>, text_context: &TextContext) -> String {
		choices.iter().enumerate()
			.filter(|(_, choice)| choice.response.is_some())
			.map(|(index, choice)| choice.response_line(index + 1, text_context))
			.collect::<Vec<String>>()
			.join("\n")
	}

	/// Whether this choice jumps to a specific prompt.
	/// 
	/// Returns `true` if the choice has a `jump` path and [`Path::matches`] passes.
	pub fn has_jump_to(&self, file: &String, other_name: &String, other_file: &String) -> bool {
		match &self.jump {
			None => false,
			Some(jump) => jump.matches(file, other_name, other_file)
		}
	}
}