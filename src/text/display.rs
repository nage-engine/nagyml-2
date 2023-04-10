use std::{
    fmt::{Debug, Display},
    time::Duration,
};

use anyhow::{Context, Result};
use crossterm::style::Stylize;
use result::OptionResultExt;
use serde::{de, Deserialize, Deserializer, Serialize};
use snailshell::{snailprint_d, snailprint_s};
use strum::{Display, EnumIter, EnumString};

use crate::{
    core::{
        audio::{Audio, SoundAction, SoundActions},
        context::TextContext,
        player::Player,
    },
    loading::loader::{ContentFile, Contents},
};

use super::templating::{TemplatableString, TemplatableValue};

#[derive(Deserialize, Serialize, Display, Debug, PartialEq, Clone, EnumString, EnumIter)]
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
    System,
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
            System => format!("{} {text}", "▐".dark_grey()),
        }
    }
}

/// The speed at which text should be printed.
#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "snake_case")]
pub enum TextSpeed {
    /// The amount of milliseconds to wait between each character.
    Delay(TemplatableValue<usize>),
    /// The rate, in characters per second, at which the text is printed.
    Rate(TemplatableValue<f32>),
    /// The amount of milliseconds that the text should take to print regardless of content length.
    Duration(TemplatableValue<usize>),
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
            _ => unreachable!(),
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
    pub fn print<T>(&self, content: &T, context: &TextContext) -> Result<()>
    where
        T: Display,
    {
        let result = match &self {
            TextSpeed::Duration(duration) => {
                snailprint_d(content, duration.get_value(context)? as f32 / 1000.0)
            }
            _ => snailprint_s(content, self.rate(context)?),
        };
        Ok(result)
    }
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(deny_unknown_fields)]
/// A formattable piece of text.
pub struct Text {
    #[serde(rename = "text")]
    /// The unformatted text content.
    pub content: TemplatableString,
    #[serde(default)]
    /// The mode in which the text content should be formatted upon retrieval.
    pub mode: TemplatableValue<TextMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// The speed at which the text should be printed.
    pub speed: Option<TextSpeed>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Whether to print a newline before the text.
    pub newline: Option<TemplatableValue<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// How long, in milliseconds, to wait aftet the text is printed.
    pub wait: Option<TemplatableValue<u64>>,
    /// Ordered sound actions to submit to the game's [`Audio`] resource as this text is displayed.
    pub sounds: Option<SoundActions>,
}

/// An ordered list of text objects.
pub type TextLines = Vec<Text>;
/// An ordered list of text objects with a flag representing whether the last entry was of the same [`TextMode`].
pub type SeparatedTextLines<'a> = Vec<(bool, &'a Text)>;

pub fn choice_text<'de, D>(deserializer: D) -> Result<Option<Text>, D::Error>
where
    D: Deserializer<'de>,
{
    let opt = Option::<Text>::deserialize(deserializer)?;
    if let Some(text) = &opt {
        if text.speed.is_some()
            || text.newline.is_some()
            || text.wait.is_some()
            || text.sounds.is_some()
        {
            return Err(de::Error::custom(
                "only fields 'text' and 'mode' are available in choice responses",
            ));
        }
    }
    Ok(opt)
}

pub type TranslationFile = ContentFile<String>;
pub type Translations = Contents<String>;

impl Text {
    /// Retrieves text content with [`TemplatableString::fill`] and formats it based on the [`TextMode`].
    pub fn get(&self, context: &TextContext) -> Result<String> {
        let string = self
            .mode
            .get_value(context)?
            .format(&self.content.fill(context)?);
        Ok(termimad::inline(&string).to_string())
    }

    fn wait(&self, context: &TextContext) -> Result<Option<u64>> {
        let result = self
            .wait
            .as_ref()
            .map(|wait| wait.get_value(context))
            .invert()?
            .or(context.config().settings.text.wait);
        Ok(result)
    }

    /// Formats and snailprints text based on its [`TextSpeed`].
    ///
    /// If the text object does not contain a `speed` field, defaults to the provided config settings.
    pub fn print(&self, player: &Player, context: &TextContext) -> Result<()> {
        let speed = self
            .speed
            .as_ref()
            .unwrap_or(&context.config().settings.text.speed);
        speed.print(&self.get(context)?, context)?;
        if let Some(sounds) = &self.sounds {
            context.resources().submit_audio(player, sounds, context)?;
        }
        if let &Some(wait) = &self.wait(context)? {
            std::thread::sleep(Duration::from_millis(wait));
        }
        Ok(())
    }

    /// Whether a newline should be printed before this line.
    /// Uses the `newline` key, otherwise defaulting to comparing the [`TextMode`] between this and the previous line, if any.
    fn is_newline(&self, previous: Option<&Text>, context: &TextContext) -> Result<bool> {
        self.newline
            .as_ref()
            .map(|nl| nl.get_value(context))
            .unwrap_or(
                previous
                    .map(|line| Ok(self.mode.get_value(context)? != line.mode.get_value(context)?))
                    .unwrap_or(Ok(false)),
            )
    }

    /// Calculates some [`SeparatedTextLines`] based on some text lines.
    fn get_separated_lines<'a>(
        lines: &'a TextLines,
        context: &TextContext,
    ) -> Result<SeparatedTextLines<'a>> {
        lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                Ok((line.is_newline(index.checked_sub(1).map(|i| &lines[i]), context)?, line))
            })
            .collect()
    }

    /// Formats and separates text lines and prints them sequentially.
    pub fn print_lines(lines: &TextLines, player: &Player, context: &TextContext) -> Result<()> {
        for (newline, line) in Self::get_separated_lines(lines, context)? {
            if newline {
                println!();
            }
            line.print(player, context)?;
        }
        Ok(())
    }

    /// Calls [`Text::print_lines`] and prints a newline at the end.
    pub fn print_lines_nl(lines: &TextLines, player: &Player, context: &TextContext) -> Result<()> {
        Self::print_lines(lines, player, context)?;
        println!();
        Ok(())
    }

    /// Validates a list of [`TextLines`] in order.
    /// Delegates validation to [`SoundAction::validate_all`] if sounds are present.
    pub fn validate_all(lines: &TextLines, audio: &Audio) -> Result<()> {
        for (index, line) in lines.iter().enumerate() {
            if let Some(sounds) = &line.sounds {
                SoundAction::validate_all(sounds, audio)
                    .with_context(|| format!("Failed to validate text object #{}", index + 1))?;
            }
        }
        Ok(())
    }
}
