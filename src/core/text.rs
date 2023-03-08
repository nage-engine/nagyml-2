use std::{fmt::{Display, Debug}, time::Duration, str::FromStr};

use anyhow::{Result, anyhow, Context as ContextTrait};
use crossterm::style::Stylize;
use rlua::{Table, Context};
use serde::{Deserialize, Deserializer, de::{Error as DeError, DeserializeOwned}};
use snailshell::{snailprint_s, snailprint_d};
use strum::EnumString;

use crate::loading::{Contents, ContentFile};

use super::{choice::{Variables, Notes}, manifest::Manifest, scripts::Scripts, resources::Resources, audio::Audio};

/// A wrapper for all data relevant for filling in [`TemplatableString`]s.
/// 
/// This struct must own copies of mutable player data (notes and variables).
/// Immutable resource data must be referenced.
pub struct TextContext<'a> {
	config: &'a Manifest,
	pub notes: Notes,
	pub variables: Variables,
	lang: String,
	lang_file: Option<&'a TranslationFile>,
	scripts: &'a Scripts,
	pub audio: &'a Option<Audio>
}

impl<'a> TextContext<'a> {
	/// Constructs a new [`TextContext`] by accessing [`Resources`] internals.
	pub fn new(config: &'a Manifest, notes: Notes, variables: Variables, lang: &str, resources: &'a Resources) -> Self {
		TextContext { 
			config, 
			notes,
			variables,
			lang: lang.to_owned(),
			lang_file: resources.lang_file(lang), 
			scripts: &resources.scripts,
			audio: &resources.audio
		}
	}

	pub fn global_variable(&self, var: &str) -> Option<String> {
		var.to_lowercase().strip_prefix("nage:").map(|name| {
			match name {
				"game_name" => Some(self.config.metadata.name.clone()),
				"game_authors" => Some(self.config.metadata.authors.join(", ")),
				"game_version" => Some(self.config.metadata.version.to_string()),
				"lang" => Some(self.lang.to_owned()),
				_ => None
			}
		})
		.flatten()
	}

	pub fn create_variable_table<'b>(&self, context: &Context<'b>) -> Result<Table<'b>, rlua::Error> {
		let table = context.create_table()?;
		table.set("game_name", self.config.metadata.name.clone())?;
		table.set("game_authors", context.create_sequence_from(self.config.metadata.authors.clone())?)?;
		table.set("game_version", self.config.metadata.version.to_string())?;
		table.set("lang", self.lang.to_owned())?;
		Ok(table)
	}
}

#[derive(Deserialize, Debug)]
#[serde(transparent)]
/// A string that is able to undergo transformations based on templating variables or custom scripts
/// or via translation file matching.
pub struct TemplatableString {
	pub content: String
}

impl TemplatableString {
	/// The default value for an undefined interpolation component.
	pub const DEFAULT_VALUE: &'static str = "UNDEFINED";

	/// Whether this string's content can be **templated** by variables or scripts.
	/// This does not check for language file matching.
	pub fn is_str_templatable(content: &str) -> bool {
		content.contains('(') || content.contains('<')
	}

	pub fn is_templatable(&self) -> bool {
		Self::is_str_templatable(&self.content)
	}

	/// Fills a templatable string based on the input delimiter characters and a filler function.
	/// 
	/// If the filler function returns [`None`], yields [`TemplatableString::DEFAULT_VARIABLE`].
	/// 
	/// If no templating characters exist, returns the input string.
	fn template<'a, F>(content: &str, before: char, after: char, filler: F) -> Result<String> where F: Fn(&str) -> Result<Option<String>> {
		if !content.contains(before) {
			return Ok(content.to_owned());
		}
		let mut result = String::with_capacity(content.len());
		let mut last_opener: Option<usize> = None;
		for (index, c) in content.char_indices() {
			if c == before {
				last_opener = Some(index);
			}
			else if c == after {
				if let Some(lb) = last_opener {
					let var = &content[(lb + 1)..index];
					result.push_str(&filler(var)?.unwrap_or(Self::DEFAULT_VALUE.to_owned()));
					last_opener = None;
				}
			}
			else {
				if last_opener.is_none() {
					result.push(c);
				}
			}
		}
		Ok(result)
	}

	/// Attempts to retrieve a content string from the passed-in lang file.
	/// 
	/// Prior to formatting, the text content may represent a language key such as `some.key.here`.
	/// It bears no difference to actual text content, but if it can be found within a lang file, that value will be used.
	/// Thus, it is vital that the value is retrieved before any formatting is performed on the content.
	fn lang_file_content<'a>(&'a self, lang_file: Option<&'a TranslationFile>) -> &'a String {
		lang_file.map(|file| file.get(&self.content))
			.flatten()
			.unwrap_or(&self.content)
	}

	fn fill_variable<'a>(var: &str, variables: &'a Variables, context: &TextContext) -> Option<String> {
		context.global_variable(var).or(variables.get(var).cloned())
	}

	pub fn fill(&self, context: &TextContext) -> Result<String> {
		let content = self.lang_file_content(context.lang_file);
		let scripted = Self::template(content, '(', ')', move |var| {
			context.scripts.get(var, context)
		})?;
		Self::template(&scripted, '<', '>', move |var| {
			let filled = Self::fill_variable(var, &context.variables, &context)
				.map(|s| s.clone());
			Ok(filled)
		})
	}
}

#[derive(Debug)]
/// A string that can either be parsed as `T` directly or via templating it.
pub struct TemplatableValue<T> {
	pub value: Option<T>,
	pub template: Option<TemplatableString>
}

impl<'de, T> Deserialize<'de> for TemplatableValue<T> where T: DeserializeOwned + Clone + FromStr {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error> where D: Deserializer<'de> {
		let string = String::deserialize(deserializer)?;
		let result = serde_yaml::from_str::<T>(&string).map_err(DeError::custom)
			.map(|value| TemplatableValue {
				value: Some(value),
				template: None
			});
		if let Err(_) = &result {
			if TemplatableString::is_str_templatable(&string) {
				return Ok(TemplatableValue::template(string));
			}
		}
		result
    }
}

impl<T> Default for TemplatableValue<T> where T: Default {
	fn default() -> Self {
		Self::value(T::default())
	}
}

impl<T> TemplatableValue<T>  {	
	pub fn value(value: T) -> Self {
		Self {
			value: Some(value),
			template: None
		}
	}

	pub fn template(content: String) -> Self {
		Self {
			value: None,
			template: Some(TemplatableString { content })
		}
	}

	pub fn get_value<E>(&self, context: &TextContext) -> Result<T>
		where
			T: Clone + FromStr<Err = E>, anyhow::Error: From<E> {
		if let Some(value) = &self.value {
			return Ok(value.clone());
		}
		if let Some(string) = &self.template {
			let filled = string.fill(context)?;
			let result = filled.parse::<T>()
				.map_err(|err| anyhow!(err))
				.with_context(|| format!("Failed to parse value '{filled}' templated from '{}'", string.content))?;
			return Ok(result);
		}
		unreachable!()
	}
}

#[derive(Deserialize, Debug, PartialEq, Clone, EnumString)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
/// Represents how text should be formatted disregarding its contents.
pub enum TextMode {
	#[serde(alias = "dialog")]
	/// Wraps text in quotes.
	Dialogue,
	/// Returns text as-is.
	Action,
	/// Prefixes text with a quote character.
	System
}

impl Default for TextMode {
	fn default() -> Self {
		Self::Dialogue
	}
}

impl TextMode {
	/// Formats a [`String`] based on the selected text mode.
	/// 
	/// See [`Mode`] types to view how a text mode will format content.
	pub fn format(&self, text: &str) -> String {
		use TextMode::*;
		match self {
			Dialogue => format!("\"{text}\""),
			Action => text.to_owned(),
			System => format!("{} {text}", "▐".dark_grey())
		}
	}
}

/// The speed at which text should be printed.
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TextSpeed {
	/// The amount of milliseconds to wait between each character.
	Delay(TemplatableValue<usize>),
	/// The rate, in characters per second, at which the text is printed.
	Rate(TemplatableValue<f32>),
	/// The amount of milliseconds that the text should take to print regardless of content length.
	Duration(TemplatableValue<usize>)
}

impl Default for TextSpeed {
	fn default() -> Self {
		TextSpeed::Rate(TemplatableValue::value(200.0))
	}
}

impl TextSpeed {
	/// Calculates or returns the rate in charatcers per second
	/// to be used in [`snailprint_s`].
	/// 
	/// If this object is [`Rate`](TextSpeed::Rate), returns the contained value.
	/// If it is [`Delay`](TextSpeed::Delay), calculates the rate with `(1.0 / delay) * 1000.0`.
	pub fn rate(&self, context: &TextContext) -> Result<f32> {
		use TextSpeed::*;
		let result = match &self {
			Rate(rate) => rate.get_value(context)?,
			Delay(delay) => 1.0 / delay.get_value(context)? as f32 * 1000.0,
			_ => unreachable!()
		};
		Ok(result)
	}

	/// Snailprints some content.
	/// 
	/// If the object is [`Rate`](TextSpeed::Rate) or [`Delay`](TextSpeed::Delay), uses [`snailprint_s`]
	/// with the rate returned from [`TextSpeed::rate`].
	/// 
	/// Otherwise, if the object is [`Duration`](TextSpeed::Duration), uses [`snailprint_d`] with the
	/// specified length of time.
	pub fn print<T>(&self, content: &T, context: &TextContext) -> Result<()> where T: Display {
		let result = match &self {
			TextSpeed::Duration(duration) => snailprint_d(content, duration.get_value(context)? as f32 / 1000.0),
			_ => snailprint_s(content, self.rate(context)?)
		};
		Ok(result)
	}
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
/// A formattable piece of text.
pub struct Text {
	#[serde(rename = "text")]
	/// The unformatted text content.
	pub content: TemplatableString,
	#[serde(default)]
	/// The mode in which the text content should be formatted upon retrieval.
	pub mode: TemplatableValue<TextMode>,
	pub speed: Option<TextSpeed>,
	pub newline: Option<TemplatableValue<bool>>,
	pub wait: Option<TemplatableValue<u64>>
}

/// An ordered list of text objects.
pub type TextLines = Vec<Text>;
/// An ordered list of text objects with a flag representing whether the last entry was of the same [`TextMode`].
pub type SeparatedTextLines<'a> = Vec<(bool, &'a Text)>;

pub type TranslationFile = ContentFile<String>;
pub type Translations = Contents<String>;

impl Text {
	/// Retrieves text content with [`TemplatableString::fill`] and formats it based on the [`TextMode`].
	pub fn get(&self, context: &TextContext) -> Result<String> {
		Ok(self.mode.get_value(context)?.format(&self.content.fill(context)?))
	}

	/// Formats and snailprints text based on its [`TextSpeed`]. 
	/// 
	/// If the text object does not contain a `speed` field, defaults to the provided config settings.
	pub fn print(&self, context: &TextContext) -> Result<()> {
		let speed = self.speed.as_ref().unwrap_or(&context.config.settings.speed);
		speed.print(&self.get(context)?, context)?;
		if let Some(wait) = &self.wait {
			std::thread::sleep(Duration::from_millis(wait.get_value(context)?));
		}
		Ok(())
	}

	/// Whether a newline should be printed before this line.
	/// Uses the `newline` key, otherwise defaulting to comparing the [`TextMode`] between this and the previous line, if any.
	fn newline(&self, previous: Option<&Text>, context: &TextContext) -> Result<bool> {
		self.newline.as_ref()
			.map(|nl| nl.get_value(context))
    		.unwrap_or(previous
				.map(|line| Ok(self.mode.get_value(context)? != line.mode.get_value(context)?))
				.unwrap_or(Ok(false))
			)
	}

	/// Calculates some [`SeparatedTextLines`] based on some text lines.
	fn get_separated_lines<'a>(lines: &'a TextLines, context: &TextContext) -> Result<SeparatedTextLines<'a>> {
		lines.iter().enumerate()
    		.map(|(index, line)| Ok((line.newline(index.checked_sub(1).map(|i| &lines[i]), context)?, line)))
    		.collect()
	}

	/// Formats and separates text lines and prints them sequentially.
	pub fn print_lines(lines: &TextLines, context: &TextContext) -> Result<()> {
		for (newline, line) in Self::get_separated_lines(lines, context)? {
			if newline {
				println!();
			}
			line.print(context)?;
		}
		Ok(())
	}

	/// Calls [`Text::print_lines`] and prints a newline at the end.
	pub fn print_lines_nl(lines: &TextLines, context: &TextContext) -> Result<()> {
		Self::print_lines(lines, context)?;
		println!();
		Ok(())
	}
}