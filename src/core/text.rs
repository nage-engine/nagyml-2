use std::fmt::Display;

use serde::{Serialize, Deserialize};
use snailshell::snailprint_s;

use crate::{loading::{Contents, ContentFile}, game::main::{Scripts, Resources}};

use super::{choice::Variables, manifest::{Manifest, Metadata}};

pub struct TextContext<'a> {
	config: &'a Manifest,
	variables: Variables,
	lang_file: Option<&'a TranslationFile>,
	scripts: &'a Scripts
}

impl<'a> TextContext<'a> {
	pub fn new(config: &'a Manifest, variables: Variables, lang: &str, resources: &'a Resources) -> Self {
		TextContext { 
			config, 
			variables, 
			lang_file: resources.lang_file(lang), 
			scripts: &resources.scripts 
		}
	}
}

#[derive(Debug)]
/// A string that is able to undergo template transformations based on variables or custom scripts.
pub struct TemplatableString {
	content: String
}

impl<'de> Deserialize<'de> for TemplatableString {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::Deserializer<'de> {
		let content = String::deserialize(deserializer)?;
		Ok(TemplatableString { content })
	}
}

impl TemplatableString {
	/// The default value for an undefined interpolation component.
	pub const DEFAULT_VARIABLE: &'static str = "UNDEFINED";

	/// Fills a templatable string based on the input delimiter characters and a filler function.
	/// 
	/// If the filler function returns [`None`], yields [`TemplatableString::DEFAULT_VARIABLE`].
	/// 
	/// If no templating characters or variables exist, returns the input string.
	fn template<'a, F>(content: &str, before: char, after: char, filler: F) -> String where F: Fn(&str) -> Option<String> {
		if !content.contains(before) {
			return content.to_owned();
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
					result.push_str(&filler(var).unwrap_or(Self::DEFAULT_VARIABLE.to_owned()));
					last_opener = None;
				}
			}
			else {
				if last_opener.is_none() {
					result.push(c);
				}
			}
		}
		result
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

	fn fill_variable<'a>(var: &str, variables: &'a Variables, metadata: &'a Metadata) -> Option<String> {
		metadata.global_variable(var).or(variables.get(var).cloned())
	}

	pub fn fill(&self, context: &TextContext) -> String {
		let content = self.lang_file_content(context.lang_file);
		Self::template(content, '<', '>', move |var| {
			Self::fill_variable(var, &context.variables, &context.config.metadata).map(|s| s.clone())
		})
	}
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "snake_case")]
/// Represents how text should be formatted disregarding its contents.
pub enum TextMode {
	#[serde(alias = "dialog")]
	/// Wraps text in quotes.
	Dialogue,
	/// Returns text as-is.
	Action
}

impl Default for TextMode {
    fn default() -> Self {
        TextMode::Dialogue
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
			Action => text.to_owned()
		}
	}
}

/// The speed at which text should be printed.
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TextSpeed {
	/// The amount of milliseconds to wait between each character.
	Delay(usize),
	/// The rate, in characters per second, at which the text is printed.
	Rate(f32)
}

impl Default for TextSpeed {
	fn default() -> Self {
		TextSpeed::Rate(200.0)
	}
}

impl TextSpeed {
	/// Calculates or returns the rate in charatcers per second.
	/// 
	/// If this object is [`Rate`](TextSpeed::Rate), returns the contained value.
	/// If it is [`Delay`](TextSpeed::Delay), calculates the rate with `(1.0 / delay) * 1000.0`.
	pub fn rate(&self) -> f32 {
		use TextSpeed::*;
		match &self {
			Rate(rate) => *rate,
			Delay(delay) => 1.0 / *delay as f32 * 1000.0
		}
	}

	/// Snailprints some content based on the [`rate`](TextSpeed::rate).
	pub fn print<T>(&self, content: &T) where T: Display {
		snailprint_s(content, self.rate());
	}

	// Calls [`TextSpeed::print`] and prints a newline at the end.
	/*pub fn print_nl<T>(&self, content: &T) where T: Display {
		self.print(content);
		println!();
	}*/
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
	pub mode: TextMode,
	pub speed: Option<TextSpeed>
}

/// An ordered list of text objects.
pub type TextLines = Vec<Text>;
/// An ordered list of text objects with a flag representing whether the last entry was of the same [`TextMode`].
pub type SeparatedTextLines<'a> = Vec<(bool, &'a Text)>;

pub type TranslationFile = ContentFile<String>;
pub type Translations = Contents<String>;

impl Text {
	/// Retrieves text content with [`TemplatableString::fill`] and formats it based on the [`TextMode`].
	pub fn get(&self, context: &TextContext) -> String {
		self.mode.format(&self.content.fill(context))
	}

	/// Formats and snailprints text based on its [`TextSpeed`]. 
	/// 
	/// If the text object does not contain a `speed` field, defaults to the provided config settings.
	pub fn print(&self, context: &TextContext) {
		let speed = self.speed.as_ref().unwrap_or(&context.config.settings.speed);
		speed.print(&self.get(context));
	}

	/// Calculates some [`SeparatedTextLines`] based on some text lines.
	fn get_separated_lines<'a>(lines: &'a TextLines) -> SeparatedTextLines<'a> {
		lines.iter().enumerate()
    		.map(|(index, line)| (index > 0 && lines[index - 1].mode != line.mode, line))
    		.collect()
	}

	/// Formats and separates text lines and prints them sequentially.
	pub fn print_lines(lines: &TextLines, context: &TextContext) {
		for (newline, line) in Self::get_separated_lines(lines) {
			if newline {
				println!();
			}
			line.print(context);
		}
	}

	/// Calls [`Text::print_lines`] and prints a newline at the end.
	pub fn print_lines_nl(lines: &TextLines, context: &TextContext) {
		Self::print_lines(lines, context);
		println!();
	}
}