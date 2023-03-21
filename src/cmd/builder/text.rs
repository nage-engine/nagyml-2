use anyhow::Result;
use requestty::{Answers, PromptModule, Question};
use strum::IntoEnumIterator;

use crate::text::{
    display::{Text, TextMode, TextSpeed},
    templating::TemplatableValue,
};

use super::util::{check_select, confirmed, input_builder};

fn text_mode_question(system: bool) -> Question<'static> {
    let mut builder = Question::select("mode")
        .message("What is this text meant for?")
        .choices(vec!["Spoken dialogue", "Actions or narrations"]);
    if system {
        builder = builder.choice("System messages");
    }
    builder.build()
}

fn prompt_text_answers() -> Result<Answers> {
    let module = PromptModule::new(vec![
        Question::confirm("use_speed")
            .message("Should this text have a custom display speed?")
            .default(false)
            .build(),
        Question::select("speed")
            .message("Display speed method")
            .choices(vec![
                "Delay a certain amount after each character",
                "Static rate of characters per second",
                "Specific duration regardless of length",
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
                ('m', "Separate by mode difference"),
            ])
            .default('m')
            .build(),
        Question::confirm("use_wait")
            .message("Should the program wait after this text is printed?")
            .default(false)
            .build(),
        Question::int("wait")
            .message("The duration to wait for, in milliseconds")
            .when(|answers: &Answers| confirmed(answers, "use_wait"))
            .build(),
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
        } else {
            let value = TemplatableValue::value(answers["speed_input"].as_int().unwrap() as usize);
            match index {
                0 => Delay(value),
                2 => Duration(value),
                _ => unreachable!(),
            }
        }
    })
}

pub fn build_text(full: bool, kind: &str) -> Result<Text> {
    let module = PromptModule::new(vec![
        input_builder("text")
            .message(format!("{kind} text content"))
            .build(),
        text_mode_question(full),
    ]);

    let answers = module.prompt_all()?;
    let prompt_answers = if full {
        Some(prompt_text_answers()?)
    } else {
        None
    };

    let newline = prompt_answers.as_ref().and_then(|ans| {
        match ans["newline"].as_expand_item().unwrap().key {
            'y' => Some(true),
            'n' => Some(false),
            'm' => None,
            _ => unreachable!(),
        }
    });

    let text = Text {
        content: answers["text"].as_string().unwrap().to_owned().into(),
        mode: TemplatableValue::value(
            TextMode::iter()
                .nth(answers["mode"].as_list_item().unwrap().index)
                .unwrap(),
        ),
        speed: prompt_answers.as_ref().and_then(get_text_speed),
        newline: newline.map(TemplatableValue::value),
        wait: prompt_answers.as_ref().and_then(|ans| {
            ans.get("wait")
                .map(|answer| TemplatableValue::value(answer.as_int().unwrap() as u64))
        }),
    };

    Ok(text)
}
