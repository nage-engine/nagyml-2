use std::{
    collections::{HashMap, HashSet, VecDeque},
    vec,
};

use anyhow::{anyhow, Result};
use result::OptionResultExt;
use serde::{Deserialize, Serialize};
use unicode_truncate::UnicodeTruncateStr;

use crate::text_context;

use super::{
    choice::Choice,
    context::{StaticContext, TextContext},
    discord::RichPresence,
    manifest::Manifest,
    path::PathData,
    prompt::PromptModel,
    state::{
        NamedVariableEntry, NoteEntries, Notes, UnlockedInfoPages, VariableEntries, Variables,
    },
};

#[derive(Serialize, Deserialize, Debug)]
/// A reversible recording of a prompt jump.
pub struct HistoryEntry {
    /// The prompt path the player jumped to.
    pub path: PathData,
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
    pub log: bool,
}

impl HistoryEntry {
    /// Constructs a player's first history entry based on an entrypoint path.
    pub fn new(path: &PathData) -> Self {
        Self {
            path: path.clone(),
            display: true,
            locked: false,
            redirect: false,
            notes: None,
            variables: None,
            log: false,
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
/// A player data tracker.
pub struct Player {
    /// Whether the player has started playing the game.
    pub began: bool,
    /// The player's display language.
    pub lang: String,
    /// The player's enabled sound channels.
    pub channels: HashSet<String>,
    /// The player's current notes.
    pub notes: Notes,
    /// The player's current variables.
    pub variables: Variables,
    /// The player's current unlocked info pages.
    pub info_pages: UnlockedInfoPages,
    /// The player's current log entries.
    pub log: Vec<String>,
    /// Recordings of each prompt jump and their associated value changes.
    pub history: VecDeque<HistoryEntry>,
}

impl Player {
    /// Constructs a player based on a [`Manifest`].
    pub fn new(config: &Manifest) -> Self {
        let entry = HistoryEntry::new(&config.entry.path);
        Self {
            began: false,
            lang: config.settings.text.lang(),
            channels: config.settings.enabled_audio_channels(),
            notes: config.entry.notes.clone().unwrap_or(HashSet::new()),
            variables: config.entry.variables.clone().unwrap_or(HashMap::new()),
            info_pages: config.entry.info_pages.clone().unwrap_or(Vec::new()),
            log: config.entry.log.clone().unwrap_or(Vec::new()),
            history: VecDeque::from(vec![entry]),
        }
    }

    /// Accepts a single [`NoteApplication`].
    ///
    /// If `take` is `true`, attempts to remove the note.
    /// Otherwise, inserts the note if not already present.
    fn apply_note(&mut self, name: &str, take: bool, reverse: bool) -> Result<()> {
        let take = if reverse { !take } else { take };
        if take {
            self.notes.remove(name);
        } else {
            self.notes.insert(name.to_owned());
        }
        Ok(())
    }

    /// Returns the latest history entry, if any.
    pub fn latest_entry(&self) -> Result<&HistoryEntry> {
        self.history.back().ok_or(anyhow!("History empty"))
    }

    /// If the latest history entry is able to be reversed, pops and returns it from the entry list.
    fn pop_latest_entry(player: &mut Player) -> Result<HistoryEntry> {
        let latest = player.latest_entry()?;
        if latest.locked {
            return Err(anyhow!("Can't go back right now!"));
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
                        None => self.variables.remove(&name),
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

    /// Whether a specified info page ID has already been unlocked.
    fn is_page_unlocked(&self, page: &str) -> bool {
        for unlocked in &self.info_pages {
            if unlocked.name == page {
                return true;
            }
        }
        return false;
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
    fn apply_entry(
        &mut self,
        entry: &HistoryEntry,
        choice: &Choice,
        text_context: &TextContext,
    ) -> Result<()> {
        if let Some(entries) = &entry.notes {
            for entry in entries {
                self.apply_note(&entry.value, entry.take, false)?;
            }
        }
        if let Some(variables) = &entry.variables {
            let values: Variables = variables
                .iter()
                .map(|(k, v)| (k.clone(), v.value.clone()))
                .collect();
            self.variables.extend(values);
        }
        // Info pages are not stored in history entries, so we can fill the name here
        if let Some(pages) = &choice.info_pages {
            for page in pages {
                let unlocked = page.to_unlocked(text_context)?;
                if !self.is_page_unlocked(&unlocked.name) {
                    self.info_pages.push(unlocked);
                }
            }
        }
        Ok(())
    }

    pub fn choose(
        &mut self,
        choice: &Choice,
        input: Option<NamedVariableEntry>,
        model: &PromptModel,
        stc: &StaticContext,
        text_context: &TextContext,
    ) -> Result<()> {
        let latest = self.latest_entry()?;
        if let Some(result) =
            choice.to_history_entry(&latest, input, &self.variables, model, stc, text_context)
        {
            let entry = result?;
            self.apply_entry(&entry, choice, text_context)?;
            self.history.push_back(entry);
            if self.history.len() > stc.config.settings.history.size {
                self.history.pop_front();
            }
        }
        if let Some(sounds) = &choice.sounds {
            stc.resources.submit_audio(&self, sounds, text_context)?;
        }
        Ok(())
    }

    pub fn after_choice(
        &mut self,
        choice: &Choice,
        stc: &StaticContext,
        drpc: &mut Option<RichPresence>,
    ) -> Result<()> {
        // Create a new text context using the new variable and note values for the logs
        // Log page names are not stored in history entries, just whether they were given, so we can fill the name here
        let text_context = if choice.log.is_some() || choice.drp.is_some() {
            Some(text_context!(stc, self))
        } else {
            None
        };
        // Apply log and Discord rich presence
        let log_filled = choice
            .log
            .as_ref()
            .map(|log| log.fill(text_context.as_ref().unwrap()))
            .invert()?;
        if let Some(log) = &log_filled {
            self.log.push(log.clone());
        };
        if let Some(state) = stc.config.rich_presence_state(
            self.latest_entry()?,
            choice.drp.as_ref(),
            log_filled.as_deref(),
            text_context.as_ref(),
        )? {
            stc.config.set_rich_presence(drpc, &state)?;
        }

        Ok(())
    }

    pub fn choose_full(
        &mut self,
        choice: &Choice,
        input: Option<NamedVariableEntry>,
        drpc: &mut Option<RichPresence>,
        model: &PromptModel,
        stc: &StaticContext,
        text_context: &TextContext,
    ) -> Result<()> {
        self.choose(choice, input, model, stc, text_context)?;
        self.after_choice(choice, stc, drpc)
    }

    /// Returns the player's log entries split into readable chunks of five entries maximum.
    pub fn log_pages(&self) -> Vec<&[String]> {
        self.log.chunks(5).collect()
    }

    /// Gets the "front" of each page in a collection of [`Player::log_pages`]; that is, the first entry
    /// in each page truncated to a readable length.
    pub fn log_page_fronts(pages: &Vec<&[String]>) -> Vec<String> {
        pages
            .iter()
            .map(|chunk| chunk[0].unicode_truncate(25).0.to_owned())
            .map(|line| format!("{line}..."))
            .collect()
    }
}
