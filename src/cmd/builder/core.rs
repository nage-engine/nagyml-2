use anyhow::Result;
use requestty::{Question, Answers, PromptModule};
use strum::IntoEnumIterator;

use crate::{core::{path::Path, choice::{SoundAction, SoundActionMode, VariableInput, NoteApplication, NoteRequirement, NoteActions, VariableApplications}}, text::templating::{TemplatableValue, TemplatableString}};

use super::util::{confirmed, build_option, build_vec, input_builder};

pub fn build_path() -> Result<Path> {
	let module = PromptModule::new(vec![
		input_builder("prompt").message("Prompt to jump to").build(),
		Question::confirm("use_file")
			.message("Is this prompt in another file?")
			.default(false)
			.build(),
		input_builder("file")
			.message("Target file name")
			.when(|answers: &Answers| confirmed(answers, "use_file"))
			.build()
	]);

	let answers = module.prompt_all()?;

	let path = Path {
		prompt: answers["prompt"].as_string().unwrap().to_owned().into(),
		file: answers.get("file").map(|answer| answer.as_string().unwrap().to_owned().into())
	};

	Ok(path)
}

pub fn build_sound_action() -> Result<SoundAction> {
	let module = PromptModule::new(vec![
		Question::select("mode")
			.message("What should this action do?")
			.choices(vec![
				"Queue a sound",
				"Play a new sound immediately",
				"Play a sound if a channel is free",
				"Skip a channel's playing sound",
				"Pause a channel",
				"Unpause a channel"
			])
			.default(2)
			.build(),
		input_builder("sound")
			.message("Sound name")
			.when(|answers: &Answers| {
				answers["mode"].as_list_item()
					.and_then(|item| SoundActionMode::iter().nth(item.index))
					.map(|mode| mode.is_specific())
					.unwrap_or(false)
			})
			.build(),
		input_builder("channel").message("Channel name").build(),
		Question::confirm("use_seek")
			.message("Start the sound at a specific time?")
			.default(false)
			.build(),
		Question::int("seek")
			.message("Position, in milliseconds")
			.when(|answers: &Answers| confirmed(answers, "use_seek"))
			.validate(|seek, _| {
				TryInto::<u64>::try_into(seek)
					.map(|_| ())
					.map_err(|err| err.to_string())
			})
			.build(),
		Question::confirm("use_speed")
			.message("Play the sound at a specific rate?")
			.default(false)
			.build(),
		Question::float("speed")
			.message("Sound speed multiplier")
			.when(|answers: &Answers| confirmed(answers, "use_speed"))
			.build()
	]);

	let answers = module.prompt_all()?;

	let action = SoundAction {
		name: answers.get("sound").map(|answer| answer.as_string().unwrap().to_owned().into()),
		channel: answers["channel"].as_string().unwrap().to_owned().into(),
		mode: TemplatableValue::value(SoundActionMode::iter().nth(answers["mode"].as_list_item().unwrap().index).unwrap()),
		seek: answers.get("seek").map(|answer| TemplatableValue::value(answer.as_int().unwrap().try_into().unwrap())),
		speed: answers.get("speed").map(|answer| TemplatableValue::value(answer.as_float().unwrap()))
	};

	Ok(action)
}

fn build_note_application() -> Result<NoteApplication> {
	let module = PromptModule::new(vec![
		input_builder("name").message("Note name").build(),
		Question::expand("mode")
			.message("Should the note be given or taken away?")
			.choices(vec![
				('g', "Give"),
				('t', "Take")
			])
			.default('g')
			.build()
	]);

	let answers = module.prompt_all()?;

	let app = NoteApplication {
		name: answers["name"].as_string().unwrap().to_owned().into(),
		take: TemplatableValue::value(answers["mode"].as_expand_item().unwrap().key == 't')
	};

	Ok(app)
}

fn build_note_requirement() -> Result<NoteRequirement> {
	let module = PromptModule::new(vec![
		input_builder("name").message("Note name").build(),
		Question::confirm("required")
			.message("Should the player have the note?")
			.default(true)
			.build()
	]);

	let answers = module.prompt_all()?;

	let req = NoteRequirement {
		name: answers["name"].as_string().unwrap().to_owned().into(),
		has: TemplatableValue::value(answers["required"].as_bool().unwrap())
	};

	Ok(req)
}

pub fn build_note_actions() -> Result<NoteActions> {
	let apply = build_option("Apply notes?", false, || build_vec("Add another note application?", false, build_note_application))?;
	let require = build_option("Require notes?", false, || build_vec("Add another note requirement?", false, build_note_requirement))?;
	
	let module = PromptModule::new(vec![
		Question::confirm("use_once")
			.message("Should this choice be usable only once?")
			.default(false)
			.build(),
		input_builder("once")
			.message("Note name to track for 'once' state")
			.when(|answers: &Answers| confirmed(answers, "use_once"))
			.build()
	]);

	let answers = module.prompt_all()?;

	let actions = NoteActions {
		apply, require,
		once: answers.get("once").map(|answer| answer.as_string().unwrap().to_owned().into())
	};

	Ok(actions)
}

fn build_variable() -> Result<(String, TemplatableString)> {
	let module = PromptModule::new(vec![
		input_builder("name").message("Variable name").build(),
		input_builder("variable").message("Variable value").build()
	]);

	let answers = module.prompt_all()?;

	Ok((
		answers["name"].as_string().unwrap().to_owned(), 
		answers["variable"].as_string().unwrap().to_owned().into()
	))
}

pub fn build_variable_applications() -> Result<VariableApplications> {
	build_vec("Add another variable?", true, build_variable)
		.map(|vec| vec.into_iter().collect())
}

pub fn build_input() -> Result<VariableInput> {
	let module = PromptModule::new(vec![
		input_builder("name").message("Variable name").build(),
		Question::confirm("use_text").message("Should the input use a custom prompt?").build(),
		input_builder("text")
			.message("Variable prompt")
			.when(|answers: &Answers| confirmed(answers, "use_text"))
			.build()
	]);

	let answers = module.prompt_all()?;

	let input = VariableInput {
		name: answers["name"].as_string().unwrap().to_owned().into(),
		text: answers.get("text").map(|text| text.as_string().unwrap().to_owned().into())
	};

	Ok(input)
}