use std::{collections::{HashMap, HashSet}};

use crate::game::input::VariableInputResult;

use super::{text::{Text, TextLines, TemplatableString, TextContext, TemplatableValue}, path::Path, prompt::{Prompts, Prompt, PromptModel}, player::{HistoryEntry, VariableEntry, VariableEntries, NoteEntry, NoteEntries}, manifest::Manifest};

use anyhow::{Result, anyhow, Context};
use result::OptionResultExt;
use serde::Deserialize;
use strum::EnumString;

pub fn default_true() -> TemplatableValue<bool> { TemplatableValue::value(true) }

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct NoteApplication {
	pub name: TemplatableString,
	#[serde(default)]
	pub take: TemplatableValue<bool>,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct NoteRequirement {
	name: TemplatableString,
	#[serde(default = "default_true")]
	has: TemplatableValue<bool>
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct NoteActions {
	pub apply: Option<Vec<NoteApplication>>,
	require: Option<Vec<NoteRequirement>>,
	pub once: Option<TemplatableString>
}

impl NoteActions {
	pub fn to_note_entries(&self, text_context: &TextContext) -> Result<NoteEntries> {
		let mut entries: NoteEntries = self.apply.as_ref().map(|apps| {
			apps.iter()
				.map(|app| NoteEntry::from_application(app, text_context))
				.collect::<Result<NoteEntries>>()
		})
		.invert()?
		.unwrap_or(Vec::new());

		if let Some(once) = &self.once {
			entries.push(NoteEntry::new(once, false, text_context)?);
		}
		Ok(entries)
	}
}

/// A list of string symbols tracked on a player.
pub type Notes = HashSet<String>;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct VariableInput {
	pub text: Option<TemplatableString>,
	#[serde(rename = "variable")]
	pub name: TemplatableString
}

/// A map of display variables wherein the key is the variable name and the value is the variable's display.
pub type Variables = HashMap<String, String>;

pub type VariableApplications = HashMap<String, TemplatableString>;

#[derive(Deserialize, Debug, Clone, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SoundActionMode {
	Queue,
	Overwrite,
	Passive,
	Skip,
	Playing(bool)
}

impl Default for SoundActionMode {
	fn default() -> Self {
		Self::Passive
	}
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct SoundAction {
	pub name: Option<TemplatableString>,
	pub channel: TemplatableString,
	#[serde(default)]
	pub mode: TemplatableValue<SoundActionMode>,
	pub seek: Option<TemplatableValue<u64>>,
	pub speed: Option<TemplatableValue<f64>>
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct Choice {
	pub response: Option<Text>,
	tag: Option<TemplatableString>,
	pub input: Option<VariableInput>,
	pub jump: Option<Path>,
	#[serde(default = "default_true")]
	pub display: TemplatableValue<bool>,
	// This is an option for easier defaulting to the config state
	pub lock: Option<TemplatableValue<bool>>,
	pub notes: Option<NoteActions>,
	pub variables: Option<VariableApplications>,
	pub log: Option<TemplatableString>,
	#[serde(rename = "info")]
	pub info_pages: Option<Vec<TemplatableString>>,
	pub sounds: Option<Vec<SoundAction>>,
	pub ending: Option<TextLines>
}

pub type Choices = Vec<Choice>;

impl Choice {
	/// Validates a choice amongst the global prompt context.
	/// 
	/// A choice is valid if:
	/// - It has either a `jump` or `ending` section
	/// - Its `jump` section **is not templatable** and points to a valid prompt
	/// 	- The `file` key has to exist and the `prompt` key has to exist in that [`PromptFile`]
	/// - It has a `response` section if there is more than one choice in the prompt
	pub fn validate(&self, local_file: &String, has_company: bool, prompts: &Prompts) -> Result<()> {
		match &self.jump {
			None => if self.ending.is_none() {
				return Err(anyhow!("Lacks `jump` section, but doesn't have an `ending` section"))
			},
			Some(jump) => {
				if jump.is_not_templatable() {
					let file = jump.file.as_ref().map(|t| t.content.clone())
						.unwrap_or(local_file.clone());
					let _ = Prompt::get(prompts, &jump.prompt.content, &file)
						.with_context(|| "`jump` section points to invalid prompt")?;
				}
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
	pub fn create_variable_entries(&self, input: Option<&VariableInputResult>, variables: &Variables, text_context: &TextContext) -> Result<Option<VariableEntries>> {
		let input_entry = input.map(|result| result.to_variable_entry(variables));
		let var_entries = self.variables.as_ref().map(|vars| VariableEntry::from_map(&vars, variables, text_context)).invert()?;
		if input_entry.is_none() && var_entries.is_none() {
			return Ok(None);
		}
		let mut entries = var_entries.unwrap_or(HashMap::new());
		if let Some((name, entry)) = input_entry {
			entries.insert(name.clone(), entry);
		}
		Ok(Some(entries))
	}

	/// Constructs a [`HistoryEntry`] based on this choice object. 
	/// 
	/// Copies over control flags, the path based on the latest history entry, and notes and variable applications.
	pub fn to_history_entry(&self, latest: &HistoryEntry, input: Option<&VariableInputResult>, config: &Manifest, variables: &Variables, model: &PromptModel, text_context: &TextContext) -> Option<Result<HistoryEntry>> {
		self.jump.as_ref().map(|jump| {
			Ok(HistoryEntry {
				path: jump.fill(&latest.path, text_context)?,
				display: self.display.get_value(text_context)?,
				locked: self.lock.as_ref().map(|lock| lock.get_value(text_context)).invert()?.unwrap_or(config.settings.history.locked),
				redirect: matches!(model, PromptModel::Redirect(_)),
				notes: self.notes.as_ref().map(|n| n.to_note_entries(text_context)).invert()?,
				variables: self.create_variable_entries(input, variables, text_context)?,
				log: self.log.is_some()
			})
		})
	}

	/// Determines if a player can use this choice.
	/// 
	/// This check passes if:
	/// - All note requirement `has` fields match the state of the provided [`Notes`] object, and
	/// - The notes object does not contain the `once` value, if any is present
	pub fn can_player_use(&self, notes: &Notes, text_context: &TextContext) -> Result<bool> {
		if let Some(actions) = &self.notes {
			if let Some(require) = &actions.require {
				for requirement in require {
					if requirement.has.get_value(text_context)? != notes.contains(&requirement.name.fill(text_context)?) {
						return Ok(false);
					}
				}
			}
			if let Some(once) = &actions.once {
				if notes.contains(&once.fill(text_context)?) {
					return Ok(false);
				}
			}
		}
		Ok(true)
	}

	/// Fills in and formats tag content, if any.
	/// 
	/// If [`Some`], returns `[VALUE] `, trailing space included.
	/// If [`None`], returns an empty [`String`].
	fn tag(&self, text_context: &TextContext) -> Result<String> {
		let result = match &self.tag {
			Some(tag) => format!("[{}] ", tag.fill(text_context)?),
			None => String::new()
		};
		Ok(result)
	}

	/// Constructs the response line for display in the game's runtime.
	/// 
	/// ### Examples
	/// 
	/// - `1) [ROGUE] "Ain't no thief."`
	/// - `2) Put down the sword`
	fn response_line(&self, index: usize, text_context: &TextContext) -> Result<String> {
		let tag = self.tag(text_context)?;
		let response = self.response.as_ref().unwrap().get(text_context)?;
		Ok(format!("{index}) {tag}{response}"))
	}

	/// Constructs a [`String`] of ordered choice responses.
	pub fn display(choices: &Vec<&Choice>, text_context: &TextContext) -> Result<String> {
		let result = choices.iter().enumerate()
			.filter(|(_, choice)| choice.response.is_some())
			.map(|(index, choice)| choice.response_line(index + 1, text_context))
			.try_collect::<Vec<String>>()?
			.join("\n");
		Ok(result)
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