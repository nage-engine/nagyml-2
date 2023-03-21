use anyhow::Result;
use requestty::{question::InputBuilder, Answer, Answers, Question};

/// Confirms that a specific answer condition has been met.
/// For use in a question builder `when` clause.
pub fn confirmed(answers: &Answers, id: &str) -> bool {
    answers
        .get(id)
        .and_then(|answer| answer.as_bool())
        .unwrap_or(false)
}

/// Checks that a specific list index was chosen.
/// For use in a question builder `when` clause.
pub fn check_select(answers: &Answers, id: &str, index: usize) -> bool {
    answers
        .get(id)
        .and_then(Answer::as_list_item)
        .map(|item| item.index == index)
        .unwrap_or(false)
}

/// Repeats a building function and appends it to a result list
/// continually based on whether the user decides to continue.
pub fn build_vec<T, F>(confirm: &str, newline: bool, build: F) -> Result<Vec<T>>
where
    F: Fn() -> Result<T>,
{
    let mut lines = vec![build()?];
    loop {
        println!();
        let question = Question::confirm(confirm).build();
        let another = &requestty::prompt_one(question)?.as_bool().unwrap();
        if !another {
            if newline {
                println!();
            }
            break;
        }
        lines.push(build()?)
    }
    Ok(lines)
}

/// Builds and returns a function result based on whether the user decides to use it.
pub fn build_option<T, F>(confirm: &str, default: bool, build: F) -> Result<Option<T>>
where
    F: Fn() -> Result<T>,
{
    let use_q = Question::confirm(confirm).default(default).build();
    let use_s = requestty::prompt_one(use_q)?.as_bool().unwrap();
    let result = if use_s { Some(build()?) } else { None };
    println!();
    Ok(result)
}

/// Creates an [`InputBuilder`] that only accepts non-empty inputs.
pub fn input_builder(id: &str) -> InputBuilder<'static> {
    Question::input(id).validate(|name, _| {
        if name.is_empty() {
            Err("input cannot be empty".into())
        } else {
            Ok(())
        }
    })
}
