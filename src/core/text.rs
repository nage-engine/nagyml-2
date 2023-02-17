use std::fmt::Display;

use serde::{Serialize, Deserialize};
use snailshell::snailprint_s;

use super::{choice::Variables, config::NageConfig};

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
        TextMode::Action
    }
}

impl TextMode {
	/// Formats a [`String`] based on the selected text mode.
	/// 
	/// See [`Mode`] types to view how a text mode will format content.
	pub fn format(&self, text: &String) -> String {
		use TextMode::*;
		match self {
			Dialogue => format!("\"{text}\""),
			Action => text.clone()
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

	/// Calls [`TextSpeed::print`] and prints a newline at the end.
	pub fn print_nl<T>(&self, content: &T) where T: Display {
		self.print(content);
		println!();
	}
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
/// A formattable piece of text.
pub struct Text {
	#[serde(rename = "text")]
	/// The unformatted text content.
	pub content: String,
	#[serde(default)]
	/// The mode in which the text content should be formatted upon retrieval.
	pub mode: TextMode,
	pub speed: Option<TextSpeed>
}

pub type TextLines = Vec<Text>;

impl Display for Text {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.mode.format(&self.content))
    }
}

impl Text {
	/// The default value for an undefined interpolation variable.
	pub const DEFAULT_VARIABLE: &'static str = "UNDEFINED";

	/// Fills a templated string with the provided variable map.
	/// 
	/// A templated variable that does not exist in the map will yield [`Text::DEFAULT_VARIABLE`].
	/// 
	/// If no templating characters or variables exist, returns the input string.
	pub fn fill(formatted: String, variables: &Variables) -> String {
		if !formatted.contains("<") || variables.is_empty() {
			return formatted;
		}
		// Initialize with some capacity to avoid most allocations
		let mut result = String::with_capacity(formatted.len());
		let mut last_lb: Option<usize> = None;
		for (index, c) in formatted.char_indices() {
			match c {
				'<' => last_lb = Some(index),
				'>' => {
					if let Some(lb) = last_lb {
						let var = &formatted[(lb + 1)..index];
						result.push_str(variables.get(var).unwrap_or(&Text::DEFAULT_VARIABLE.to_owned()));
						last_lb = None;
					}
				}
				_ => {
					if last_lb.is_none() {
						result.push(c);
					}
				}
			}
		}
		result
	}

	/// Formats the text content based on its [`Mode`] and fills in any interpolation variables.
	pub fn get(&self, variables: &Variables) -> String {
		Self::fill(self.to_string(), variables)
	}

	/// Formats and snailprints text based on its [`TextSpeed`]. 
	/// 
	/// If the text object does not contain a `speed` field, defaults to the provided config settings.
	pub fn print(&self, variables: &Variables, config: &NageConfig) {
		let speed = self.speed.as_ref().unwrap_or(&config.settings.speed);
		speed.print(self);
	}

	/// Formats lines of text and prints them sequentially.
	/// 
	/// Separates each formatted line with newlines. Between two text lines, if their text modes differ, uses two newlines; otherwise, uses one.
	pub fn print_lines(lines: &TextLines, variables: &Variables, config: &NageConfig) {
		for (index, line) in lines.iter().enumerate() {
			if index > 0 && lines[index - 1].mode != line.mode {
				println!(); // Newline
			}
			line.print(variables, config);
		}
	}

	/// Calls [`Text::print_lines`] and prints a newline at the end.
	pub fn print_lines_nl(lines: &TextLines, variables: &Variables, config: &NageConfig) {
		Self::print_lines(lines, variables, config);
		println!();
	}
}