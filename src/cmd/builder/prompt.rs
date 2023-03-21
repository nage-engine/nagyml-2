use anyhow::{anyhow, Result};
use requestty::{Answer, Answers, PromptModule, Question};

use crate::{
    cmd::builder::{
        core::{
            build_input, build_note_actions, build_path, build_sound_action,
            build_variable_applications,
        },
        text::build_text,
        util::{build_option, build_vec},
    },
    core::{choice::Choice, prompt::Prompt},
    text::{
        display::Text,
        templating::{TemplatableString, TemplatableValue},
    },
};

use super::util::{confirmed, input_builder};

fn static_choice_answers() -> Result<Answers> {
    let module = PromptModule::new(vec![
        Question::confirm("display")
            .message("Should the next prompt display its intro text?")
            .default(true)
            .build(),
        Question::expand("lock")
            .message("Should the player be allowed to undo this choice?")
            .choices(vec![('y', "Yes"), ('n', "No"), ('c', "Default to config")])
            .default('c')
            .build(),
        Question::confirm("use_log")
            .message("Should this choice append to the player log?")
            .default(false)
            .build(),
        input_builder("log")
            .message("Log entry to append")
            .when(|answers: &Answers| confirmed(answers, "use_log"))
            .build(),
        Question::confirm("use_drp")
            .message("Should this choice modify Discord Rich Presence?")
            .default(false)
            .build(),
        input_builder("drp")
            .message("Rich Presence details")
            .when(|answers: &Answers| confirmed(answers, "use_drp"))
            .build(),
    ]);

    let answers = module.prompt_all()?;
    Ok(answers)
}

fn build_choice_response(model: usize) -> Result<(Option<Text>, Option<Answer>)> {
    let result = if model == 0 {
        let text = build_text(false, "Response")?;
        println!();
        let tag = build_option("Should a trait tag display next to the response?", false, || {
            let tag_q = input_builder("Tag trait").build();
            requestty::prompt_one(tag_q).map_err(|err| anyhow!(err))
        })?;
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
    } else {
        None
    };

    let static_answers = static_choice_answers()?;
    println!();
    let notes = build_option("Add note actions?", false, build_note_actions)?;
    let variables = build_option("Apply static variables?", false, build_variable_applications)?;

    let info_pages: Option<Vec<TemplatableString>> =
        build_option("Unlock info pages?", false, || {
            build_vec("Add another info page?", false, || {
                let info_q = input_builder("Info page name").build();
                let answer = requestty::prompt_one(info_q).map_err(|err| anyhow!(err))?;
                Ok(answer.as_string().unwrap().to_owned().into())
            })
        })?;

    let sounds = build_option("Add sound actions?", false, || {
        build_vec("Add another sound action?", false, build_sound_action)
    })?;

    let use_jump = if model < 2 {
        let question = Question::expand("Jump to another prompt or end the game?")
            .choices(vec![('j', "Jump"), ('e', "End")])
            .default('j')
            .build();
        requestty::prompt_one(question)?
            .as_expand_item()
            .unwrap()
            .key
            == 'j'
    } else {
        model == 2
    };

    println!();

    let jump = if use_jump { Some(build_path()?) } else { None };

    let ending = if !use_jump {
        Some(build_vec("Add another ending text object?", true, || build_text(true, "Ending"))?)
    } else {
        None
    };

    let lock = match static_answers["lock"].as_expand_item().unwrap().key {
        'y' => Some(true),
        'n' => Some(false),
        'c' => None,
        _ => unreachable!(),
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
        log: static_answers
            .get("log")
            .map(|log| log.as_string().unwrap().to_owned().into()),
        info_pages,
        sounds,
        ending,
        drp: static_answers
            .get("drp")
            .map(|drp| drp.as_string().unwrap().to_owned().into()),
    };

    Ok(choice)
}

pub fn build_prompt() -> Result<Prompt> {
    let text_lines = build_option("Should this prompt display text?", true, || {
        build_vec("Add another prompt text object?", false, || build_text(true, "Prompt"))
    })?;

    let model_question = Question::select("model")
        .message("What should this prompt do?")
        .choices(vec![
            "Present the player with choices",
            "Take input from the player",
            "Jump to another prompt without input",
            "End the game",
        ])
        .build();

    let model = requestty::prompt_one(model_question)?
        .as_list_item()
        .unwrap()
        .index;

    let choices = if model == 0 {
        build_vec("Add another choice?", true, || build_choice(model))?
    } else {
        vec![build_choice(model)?]
    };

    let prompt = Prompt {
        text: text_lines,
        choices,
    };

    Ok(prompt)
}
