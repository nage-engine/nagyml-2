use std::str::FromStr;

use anyhow::{Result, anyhow, Context};
use serde::{Deserialize, Serialize, de::{DeserializeOwned, Error as DeError}, Deserializer};

use crate::core::choice::Variables;

use super::{display::TranslationFile, context::TextContext};

#[derive(Deserialize, Serialize, Debug)]
#[serde(transparent)]
/// A string that is able to undergo transformations based on templating variables or custom scripts
/// or via translation file matching.
pub struct TemplatableString {
	pub content: String
}

impl From<String> for TemplatableString {
    fn from(content: String) -> Self {
		TemplatableString { content }
    }
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

impl<T> Serialize for TemplatableValue<T> where T: Serialize + Clone + ToString {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> where S: serde::Serializer {
		if let Some(value) = &self.value {
			return value.serialize(serializer);
		}
		if let Some(template) = &self.template {
			return template.serialize(serializer);
		}
		unreachable!()
    }
}

impl<T> Default for TemplatableValue<T> where T: Default {
	fn default() -> Self {
		Self::value(T::default())
	}
}

impl<T> TemplatableValue<T> {
	/// Constructs a [`TemplatableValue`] from the actual value type.	
	pub fn value(value: T) -> Self {
		Self {
			value: Some(value),
			template: None
		}
	}

	/// Constructs a [`TemplatableValue`] from a templatable string.
	pub fn template(content: String) -> Self {
		Self {
			value: None,
			template: Some(TemplatableString { content })
		}
	}

	/// Gets the value of type `T` from the templatable value.
	/// 
	/// If the value is provided as-is, returns a clone of that value.
	/// If the value is a templatable string, fills and parses that string as a value of type `T`.
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