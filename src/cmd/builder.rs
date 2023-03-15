use anyhow::{Result, anyhow};
use requestty::{Question, Answers, Answer, PromptModule};
use strum::IntoEnumIterator;

use crate::{core::{path::Path, choice::{SoundAction, SoundActionMode, Choice, VariableInput, NoteApplication, NoteRequirement, NoteActions, VariableApplications}, prompt::Prompt}, text::{templating::{TemplatableValue, TemplatableString}, display::{Text, TextSpeed, TextMode}}};

fn confirmed(answers: &Answers, id: &str) -> bool {
	answers.get(id).and_then(|answer| answer.as_bool()).unwrap_or(false)
}

fn check_select(answers: &Answers, id: &str, index: usize) -> bool {
	answers.get(id)
		.and_then(Answer::as_list_item)
		.map(|item| item.index == index)
		.unwrap_or(false)
}

fn build_vec<T, F>(confirm: &str, build: F) -> Result<Vec<T>> where F: Fn() -> Result<T> {
	let mut lines = vec![build()?];
	loop {
		println!();
		let question = Question::confirm(confirm).build();
		let another = &requestty::prompt_one(question)?.as_bool().unwrap();
		if !another {
			println!();
			break;
		}
		lines.push(build()?)
	}
	Ok(lines)
}

fn build_option<T, F>(confirm: &str, build: F) -> Result<Option<T>> where F: Fn() -> Result<T> {
	let use_q = Question::confirm(confirm).build();
	let use_s = requestty::prompt_one(use_q)?.as_bool().unwrap();
	let result = if use_s { Some(build()?) } else { None };
	Ok(result)
}

fn text_mode_question(system: bool) -> Question<'static> {
	let mut builder = Question::select("mode")
		.message("What is this text meant for?")
		.choices(vec![
			"Spoken dialogue",
			"Actions or narrations"
		]);
	if system {
		builder = builder.choice("System messages");
	}
	builder.build()
}

fn prompt_text_answers() -> Result<Answers> {
	let module = PromptModule::new(vec![
		Question::confirm("use_speed").message("Should this text have a custom display speed?").build(),
		Question::select("speed")
			.message("Display speed method")
			.choices(vec![
				"Delay a certain amount after each character",
				"Static rate of characters per second",
				"Specific duration regardless of length"
			])
			.when(|answers: &Answers| confirmed(answers, "use_speed"))
			.build(),
		Question::int("speed_input")
			.message("Delay between each character, in milliseconds")
			.when(|answers: &Answers| check_select(answers, "speed", 0))
			.build(),
		Question::float("speed_input")
			.message("Rate, in characters per second")
			.when(|answers: &Answers| check_select(answers, "speed", 1))
			.build(),
		Question::int("speed_input")
			.message("Total duration, in milliseconds")
			.when(|answers: &Answers| check_select(answers, "speed", 2))
			.build(),
		Question::expand("newline")
			.message("Should this and the next line be separated?")
			.choices(vec![
				('y', "Yes"),
				('n', "No"),
				('m', "Separate by mode difference")
			])
			.default('m')
			.build(),
		Question::confirm("use_wait").message("Should the program wait after this text is printed?").build(),
		Question::int("wait")
			.message("The duration to wait for, in milliseconds")
			.when(|answers: &Answers| confirmed(answers, "use_wait"))
			.build()
	]);

	let answers = module.prompt_all()?;
	Ok(answers)
}

fn get_text_speed(answers: &Answers) -> Option<TextSpeed> {
	use TextSpeed::*;

	answers.get("speed").map(|answer| {
		let index = answer.as_list_item().unwrap().index;

		if index == 1 {
			Rate(TemplatableValue::value(answers["speed_input"].as_float().unwrap() as f32))
		}
		else {
			let value = TemplatableValue::value(answers["speed_input"].as_int().unwrap() as usize);
			match index {
				0 => Delay(value),
				2 => Duration(value),
				_ => unreachable!()
			}
		}
	})
}

fn build_text(full: bool, kind: &str) -> Result<Text> {
	let module = PromptModule::new(vec![
		Question::input("text").message(format!("{kind} text content")).build(),
		text_mode_question(full)
	]);

	let answers = module.prompt_all()?;
	let prompt_answers = if full { Some(prompt_text_answers()?) } else { None };

	let newline = prompt_answers.as_ref().and_then(|ans| {
		match ans["newline"].as_expand_item().unwrap().key {
			'y' => Some(true),
			'n' => Some(false),
			'm' => None,
			_ => unreachable!()
		}
	});

	let text = Text {
		content: answers["text"].as_string().unwrap().to_owned().into(),
		mode: TemplatableValue::value(TextMode::iter().nth(answers["mode"].as_list_item().unwrap().index).unwrap()),
		speed: prompt_answers.as_ref().and_then(get_text_speed),
		newline: newline.map(TemplatableValue::value),
		wait: prompt_answers.as_ref().and_then(|ans| ans.get("wait").map(|answer| TemplatableValue::value(answer.as_int().unwrap() as u64)))
	};

	Ok(text)
}

fn build_path() -> Result<Path> {
	let module = PromptModule::new(vec![
		Question::input("prompt").message("Prompt to jump to").build(),
		Question::confirm("use_file").message("Is this prompt in another file?").build(),
		Question::input("file")
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

fn build_sound_action() -> Result<SoundAction> {
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
		Question::input("sound")
			.message("Sound name")
			.when(|answers: &Answers| {
				answers["mode"].as_list_item()
					.and_then(|item| SoundActionMode::iter().nth(item.index))
					.map(|mode| mode.is_specific())
					.unwrap_or(false)
			})
			.build(),
		Question::input("channel").message("Channel name").build(),
		Question::confirm("use_seek").message("Start the sound at a specific time?").build(),
		Question::int("seek")
			.message("Position, in milliseconds")
			.when(|answers: &Answers| confirmed(answers, "use_seek"))
			.validate(|seek, _| {
				TryInto::<u64>::try_into(seek)
					.map(|_| ())
					.map_err(|err| err.to_string())
			})
			.build(),
		Question::confirm("use_speed").message("Play the sound at a specific rate?").build(),
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
		Question::input("name").message("Note name").build(),
		Question::expand("mode")
			.message("Should the note be given or taken away?")
			.choices(vec![
				('g', "Give"),
				('t', "Take")
			])
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
		Question::input("name").message("Note name").build(),
		Question::confirm("required").message("Should the player have the note?").build()
	]);

	let answers = module.prompt_all()?;

	let req = NoteRequirement {
		name: answers["name"].as_string().unwrap().to_owned().into(),
		has: TemplatableValue::value(answers["required"].as_bool().unwrap())
	};

	Ok(req)
}

fn build_note_actions() -> Result<NoteActions> {
	let apply = build_option("Apply notes?", || build_vec("Add another note application?", build_note_application))?;
	let require = build_option("Require notes?", || build_vec("Add another note requirement?", build_note_requirement))?;
	
	let module = PromptModule::new(vec![
		Question::confirm("use_once").message("Should this choice be usable only once?").build(),
		Question::input("once")
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
		Question::input("name").message("Variable name").build(),
		Question::input("variable").message("Variable value").build()
	]);

	let answers = module.prompt_all()?;

	Ok((
		answers["name"].as_string().unwrap().to_owned(), 
		answers["variable"].as_string().unwrap().to_owned().into()
	))
}

fn build_variable_applications() -> Result<VariableApplications> {
	build_vec("Add another variable?", build_variable)
		.map(|vec| vec.into_iter().collect())
}

fn build_input() -> Result<VariableInput> {
	let module = PromptModule::new(vec![
		Question::input("name").message("Variable name").build(),
		Question::confirm("use_text").message("Should the input use a custom prompt?").build(),
		Question::input("text")
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

fn static_choice_answers() -> Result<Answers> {
	let module = PromptModule::new(vec![
		Question::confirm("display")
			.message("Should the next prompt display its intro text?")
			.default(true)
			.build(),
		Question::expand("lock")
			.message("Should the player be allowed to undo this choice?")
			.choices(vec![
				('y', "Yes"),
				('n', "No"),
				('c', "Default to config")
			])
			.default('c')
			.build(),
		Question::confirm("use_log").message("Should this choice append to the player log?").build(),
		Question::input("log")
			.message("Log entry to append")
			.when(|answers: &Answers| confirmed(answers, "use_log"))
			.build()
	]);

	let answers = module.prompt_all()?;
	Ok(answers)
}

fn build_choice_response(model: usize) -> Result<(Option<Text>, Option<Answer>)> {
	let result = if model == 0 {
		let text = build_text(false, "Response")?; println!();	
		let tag = build_option(
			"Should a trait tag display next to the response?", 
			|| {
				let tag_q = Question::input("Tag trait").build();
				requestty::prompt_one(tag_q).map_err(|err| anyhow!(err))
			}
		)?;
		(Some(text), tag)
	} else { 
		(None, None) 
	};
	Ok(result)
}

fn build_choice(model: usize) -> Result<Choice> {
	let (response, tag) = build_choice_response(model)?;

	let input = if model == 1 {
		Some(build_input()?)
	} else { None };

	let static_answers = static_choice_answers()?; println!();
	let notes = build_option("Add note actions?", build_note_actions)?; println!();
	let variables = build_option("Apply static variables?", build_variable_applications)?;

	let info_pages: Option<Vec<TemplatableString>> = build_option("Unlock info pages?", || {
		build_vec("Add another info page?", || {
			let info_q = Question::input("Info page name").build();
			let answer = requestty::prompt_one(info_q).map_err(|err| anyhow!(err))?;
			Ok(answer.as_string().unwrap().to_owned().into())
		})
	})?;

	let sounds = build_option("Add sound actions?", || build_vec("Add another sound action?", build_sound_action))?;

	let use_jump = if model < 2 {
		let question = Question::expand("Jump to another prompt or end the game?")
    		.choices(vec![
				('j', "Jump"),
				('e', "End")
			])
    		.default('j')
			.build();
		requestty::prompt_one(question)?.as_expand_item().unwrap().key == 'j'
	}
	else {
		model == 2
	};

	println!();

	let jump = if use_jump {
		Some(build_path()?)
	} else { None };

	let ending = if !use_jump {
		Some(build_vec("Add another ending text object?", || build_text(true, "Ending"))?)
	} else { None };

	let lock = match static_answers["lock"].as_expand_item().unwrap().key {
		'y' => Some(true),
		'n' => Some(false),
		'c' => None,
		_ => unreachable!()
	};

	let choice = Choice {
		response,
		tag: tag.map(|t| t.as_string().unwrap().to_owned().into()),
		input,
		jump,
		display: TemplatableValue::value(static_answers["display"].as_bool().unwrap()),
		lock: lock.map(TemplatableValue::value),
		notes,
		variables,
		log: static_answers.get("log").map(|log| log.as_string().unwrap().to_owned().into()),
		info_pages,
		sounds,
		ending
	};

	Ok(choice)
}

pub fn build_prompt() -> Result<Prompt> {
	let text_lines = build_option(
		"Should this prompt display text?", 
		|| build_vec("Add another prompt text object?", || build_text(true, "Prompt"))
	)?;

	let model_question = Question::select("model")
		.message("What should this prompt do?")
		.choices(vec![
			"Present the player with choices",
			"Take input from the player",
			"Jump to another prompt without input",
			"End the game"
		])
		.build();

	let model = requestty::prompt_one(model_question)?.as_list_item().unwrap().index;

	let choices = if model == 0 {
		vec![build_choice(model)?]
	}
	else {
		build_vec("Add another choice?", || build_choice(model))?
	};

	let prompt = Prompt {
		text: text_lines,
		choices
	};

	Ok(prompt)
}