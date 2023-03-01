use std::{collections::{HashMap, HashSet, VecDeque}, vec};

use anyhow::{Result, Context, anyhow};
use serde::{Serialize, Deserialize};

use crate::{loading::parse, input::controller::VariableInputResult};

use super::{path::Path, choice::{NoteApplication, Notes, Variables, NoteActions, Choice}, manifest::Manifest};

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
	pub fn new(name: &String, value: &String, variables: &Variables) -> Self {
		VariableEntry { 
			value: value.clone(), 
			previous: variables.get(name).map(|prev| prev.clone())
		}
	}

	pub fn from_map(applying: &Variables, globals: &Variables) -> VariableEntries {
		applying.iter()
			.map(|(name, value)| {
				(name.clone(), VariableEntry::new(name, value, globals))
			})
			.collect()
	}
}

#[derive(Serialize, Deserialize, Debug)]
/// A reversible recording of a prompt jump.
pub struct HistoryEntry {
	/// The prompt path the player jumped to.
	pub path: Path,
	/// Whether the new prompt's introduction text was displayed according to [`Choice::display`].
	pub display: bool,
	/// Whether this history entry can be reversed according to [`Choice::lock`].
	pub locked: bool,
	/// The note actions executed during this entry, if any.
	pub notes: Option<Vec<NoteApplication>>,
	/// The variables applied during this entry, if any.
	pub variables: Option<VariableEntries>
}

impl HistoryEntry {
	/// Constructs a player's first history entry based on an entrypoint path.
	pub fn new(path: &Path) -> Self {
		Self {
			path: path.clone(),
			display: true,
			locked: false,
			notes: None,
			variables: None
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
	pub fn apply_note(&mut self, app: &NoteApplication, reverse: bool) {
		let take = if reverse { !app.take } else { app.take };
		if take {
			self.notes.remove(&app.name);
		}
		else {
			self.notes.insert(app.name.clone());
		}
	}

	/// Accepts a [`NoteActions`] object.
	/// 
	/// Uses [`Self::apply_note`] for note applications and attempts to insert the `once` value.
	pub fn accept_note_actions(&mut self, actions: &NoteActions) {
		if let Some(apps) = &actions.apply {
			for app in apps {
				self.apply_note(app, false);
			}
		}
		if let Some(once) = &actions.once {
			self.notes.insert(once.clone());
		}
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

	/// Appends a new entry to the history data based on what happened during the last input loop.
	/// 
	/// There must always be at least one history entry to take modifications from.
	/// Additionally, this call respect's the game's [`HistorySettings`] and prunes the front of the history list based on the size.
	pub fn push_history(&mut self, choice: &Choice, input: Option<&VariableInputResult>, config: &Manifest) -> Result<()> {
		let latest = self.latest_entry()?;
		if let Some(entry) = choice.to_history_entry(&latest, input, &self.variables, config) {
			self.history.push_back(entry?);
		}
		if self.history.len() > config.settings.history.size {
			self.history.pop_front();
		}
		Ok(())
	}

	pub fn reverse_history(&mut self) -> Result<()> {
		let latest = Self::pop_latest_entry(self)?;
		if let Some(apps) = &latest.notes {
			for app in apps {
				self.apply_note(&app, true);
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
		Ok(())
	}

	pub fn choose(&mut self, choice: &Choice, input: Option<&VariableInputResult>, config: &Manifest) -> Result<()> {
		self.push_history(choice, input, config)?;
		if let Some(actions) = &choice.notes {
			self.accept_note_actions(actions);
		}
		if let Some(variables) = &choice.variables {
			self.variables.extend(variables.clone());
		}
		Ok(())
	}
}