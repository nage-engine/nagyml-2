use std::{collections::{HashMap, HashSet, VecDeque}, vec};

use anyhow::{Result, Context, anyhow};
use serde::{Serialize, Deserialize};

use crate::{loading::parse, input::controller::VariableInputResult};

use super::{choice::{NoteApplication, Notes, Variables, Choice, VariableApplications}, manifest::Manifest, text::{TextContext, TemplatableString}, resources::{UnlockedInfoPages, Resources}, prompt::PromptModel};

#[derive(Serialize, Deserialize, Debug)]
/// A single variable value recording.
pub struct VariableEntry {
	/// The new variable value.
	pub value: String,
	/// The previous variable value if being overriden.
	pub previous: Option<String>
}

/// A map of variable names to value recordings.
pub type VariableEntries = HashMap<String, VariableEntry>;

impl VariableEntry {
	pub fn new(name: &String, value: String, variables: &Variables) -> Self {
		VariableEntry { 
			value: value.clone(), 
			previous: variables.get(name).map(|prev| prev.clone())
		}
	}

	fn from_template(name: &String, value: &TemplatableString, variables: &Variables, text_context: &TextContext) -> Result<Self> {
		Ok(VariableEntry::new(name, value.fill(text_context)?, variables))
	}

	pub fn from_map(applying: &VariableApplications, globals: &Variables, text_context: &TextContext) -> Result<VariableEntries> {
		applying.iter()
			.map(|(name, value)| {
				let entry = VariableEntry::from_template(name, value, globals, text_context);
				entry.map(|e| (name.clone(), e))
			})
			.collect()
	}
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NoteEntry {
	pub value: String,
	pub take: bool
}

pub type NoteEntries = Vec<NoteEntry>;

impl NoteEntry {
	pub fn new(name: &TemplatableString, take: bool, text_context: &TextContext) -> Result<Self> {
		let entry = NoteEntry { 
			value: name.fill(text_context)?, 
			take
		};
		Ok(entry)
	}

	pub fn from_application(app: &NoteApplication, text_context: &TextContext) -> Result<Self> {
		Self::new(&app.name, app.take, text_context)
	}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PathEntry {
	pub file: String,
	pub prompt: String
}

#[derive(Serialize, Deserialize, Debug)]
/// A reversible recording of a prompt jump.
pub struct HistoryEntry {
	/// The prompt path the player jumped to.
	pub path: PathEntry,
	/// Whether the new prompt's introduction text was displayed according to [`Choice::display`].
	pub display: bool,
	/// Whether this history entry can be reversed according to [`Choice::lock`].
	pub locked: bool,
	/// Whether this entry was a jump with no player input.
	pub redirect: bool,
	/// The note actions executed during this entry, if any.
	pub notes: Option<NoteEntries>,
	/// The variables applied during this entry, if any.
	pub variables: Option<VariableEntries>,
	/// Whether a log entry was gained during this entry.
	pub log: bool
}

impl HistoryEntry {
	/// Constructs a player's first history entry based on an entrypoint path.
	pub fn new(path: &PathEntry) -> Self {
		Self {
			path: path.clone(),
			display: true,
			locked: false,
			redirect: false,
			notes: None,
			variables: None,
			log: false
		}
	}
}

#[derive(Serialize, Deserialize, Debug)]
/// The player data tracker.
pub struct Player {
	/// Whether the player has started playing the game.
	pub began: bool,
	/// The player's display language.
	pub lang: String,
	/// The player's current notes.
	pub notes: Notes,
	/// The player's current variables.
	pub variables: Variables,
	/// The player's current unlocked info pages.
	pub info_pages: UnlockedInfoPages,
	/// The player's current log entries.
	pub log: Vec<String>,
	/// Recordings of each prompt jump and their associated value changes.
	pub history: VecDeque<HistoryEntry>
}

impl Player {
	const FILE: &'static str = "data.yml";

	/// Constructs a player based on a [`Manifest`].
	fn new(config: &Manifest) -> Self {
		let entry = HistoryEntry::new(&config.entry.path);
		Self {
			began: false,
			lang: config.settings.lang.clone().unwrap_or(String::from("en_us")),
			notes: config.entry.notes.clone().unwrap_or(HashSet::new()),
			variables: config.entry.variables.clone().unwrap_or(HashMap::new()),
			info_pages: config.entry.info_pages.clone().unwrap_or(HashSet::new()),
			log: config.entry.log.clone().unwrap_or(Vec::new()),
			history: VecDeque::from(vec![entry])
		}
	}

	/// Attempts to load player data from the local `data.yml` file. 
	/// If this file does not exist, defaults to constructing the player data with [`Player::new`].
	pub fn load(config: &Manifest) -> Result<Self> {
		match std::fs::read_to_string(Self::FILE) {
			Ok(content) => parse(Self::FILE, &content).with_context(|| "Failed to parse player data"),
			Err(_) => Ok(Self::new(config))
		}
	}

	/// Saves the player data to `data.yml`. Ignores any errors that may arise.
	pub fn save(&self) {
		if let Ok(content) = serde_yaml::to_string(self) {
			let _ = std::fs::write(Self::FILE, content);
		}
	}

	/// Accepts a single [`NoteApplication`].
	/// 
	/// If `take` is `true`, attempts to remove the note.
	/// Otherwise, inserts the note if not already present.
	pub fn apply_note(&mut self, name: &str, take: bool, reverse: bool) -> Result<()> {
		let take = if reverse { !take } else { take };
		if take {
			self.notes.remove(name);
		}
		else {
			self.notes.insert(name.to_owned());
		}
		Ok(())
	}

	/// Returns the latest history entry, if any.
	pub fn latest_entry(&self) -> Result<&HistoryEntry> {
		self.history.back().ok_or(anyhow!("History empty"))
	}

	/// If the latest history entry is able to be reversed, pops and returns it from the entry list.
	pub fn pop_latest_entry(player: &mut Player) -> Result<HistoryEntry> {
		let latest = player.latest_entry()?;
		if latest.locked {
			return Err(anyhow!("Can't go back right now!"))
		}
		Ok(player.history.pop_back().unwrap())
	}

	/// Pops the latest [`HistoryEntry`] off the stack using [`Player::pop_latest_entry`] and reverses its effects.
	pub fn back(&mut self) -> Result<()> {
		loop {
			let latest = Self::pop_latest_entry(self)?;
			if let Some(apps) = &latest.notes {
				for app in apps {
					self.apply_note(&app.value, app.take, true)?;
				}
			}
			if let Some(vars) = latest.variables {
				for (name, variable_entry) in vars {
					match variable_entry.previous {
						Some(previous) => self.variables.insert(name, previous),
						None => self.variables.remove(&name)
					};
				}
			}
			if latest.log {
				self.log.pop();
			}
			if !latest.redirect {
				break;
			}
		}
		Ok(())
	}

	/// Applies the effects of a new history entry along with choice data.
	/// 
	/// The following data is applied:
	/// - `notes` actions
	/// - `variables` map
	/// - `info` unlocks
	/// 
	/// The applied data is sensitive and relies on the previous unaltered state.
	/// For this reason, `log` data, which relies on the altered state, is **not** applied in this function.
	/// To combine this choosing functionality with `log` entry pushes, use [`Player:choose_full`].
	fn apply_entry(&mut self, entry: &HistoryEntry, choice: &Choice, text_context: &TextContext) -> Result<()> {
		if let Some(entries) = &entry.notes {
			for entry in entries {
				self.apply_note(&entry.value, entry.take, false)?;
			}
		}
		if let Some(variables) = &entry.variables {
			let values: Variables = variables.iter()
    			.map(|(k, v)| (k.clone(), v.value.clone()))
				.collect();
			self.variables.extend(values);
		}
		// Info pages are not stored in history entries, so we can fill the name here
		if let Some(pages) = &choice.info {
			for page in pages {
				self.info_pages.insert(page.fill(text_context)?);
			}
		}
		Ok(())
	}


	pub fn choose(&mut self, choice: &Choice, input: Option<&VariableInputResult>, config: &Manifest, model: &PromptModel, resources: &Resources, text_context: &TextContext) -> Result<()> {
		let latest = self.latest_entry()?;
		if let Some(result) = choice.to_history_entry(&latest, input, config, &self.variables, model, text_context) {
			let entry = result?;
			self.apply_entry(&entry, choice, text_context)?;
			self.history.push_back(entry);
			if self.history.len() > config.settings.history.size {
				self.history.pop_front();
			}
		}
		if let Some(audio) = &resources.audio {
			if let Some(sound) = &choice.sound {
				audio.play_sound(&sound.fill(text_context)?)?;
			}
		}
		Ok(())
	}

	pub fn try_push_log(&mut self, choice: &Choice, config: &Manifest, resources: &Resources) -> Result<()> {
		if let Some(log) = &choice.log {
			// Create a new text context using the new variable and note values for the logs
			// Log page names are not stored in history entries, just whether they were given, so we can fill the name here
			let new_text_context = TextContext::new(config, self.notes.clone(), self.variables.clone(), &self.lang, resources);
			self.log.push(log.fill(&new_text_context)?);
		}
		Ok(())
	}

	pub fn choose_full(&mut self, choice: &Choice, input: Option<&VariableInputResult>, config: &Manifest, resources: &Resources, model: &PromptModel, text_context: &TextContext) -> Result<()> {
		self.choose(choice, input, config, model, resources, text_context)?;
		self.try_push_log(choice, config, resources)
	}
}