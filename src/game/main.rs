use anyhow::{Result, anyhow};

use crate::{core::{prompt::{Prompt, PromptModel}, text::{Text, TextContext}, manifest::Manifest, player::Player, resources::Resources}, loading::saves::SaveManager};

use super::{gloop::{next_input_context, take_input, GameLoopResult}, input::InputController};

pub fn first_play_init(config: &Manifest, player: &mut Player, resources: &Resources) -> Result<()> {
	if let Some(background) = &config.entry.background {
		Text::print_lines_nl(background, &TextContext::new(config, player.notes.clone(), player.variables.clone(), &player.lang, resources))?;
	}
	player.began = true;
	Ok(())
}

pub fn begin(config: &Manifest, player: &mut Player, saves: &SaveManager, resources: &Resources, input: &mut InputController) -> Result<bool> {
	if !player.began {
		first_play_init(config, player, resources)?;
	}
	let silent = 'outer: loop {
		// Text context owns variables to avoid immutable and mutable borrow overlap
		let text_context = TextContext::new(config, player.notes.clone(), player.variables.clone(), &player.lang, resources);
		let entry = player.latest_entry()?;
		let next_prompt = Prompt::get_from_path(&resources.prompts, &entry.path)?;
		let model = next_prompt.model(&text_context)?;
		let choices = next_prompt.usable_choices(&player.notes, &text_context)?;

		if choices.is_empty() {
			return Err(anyhow!("No usable choices"))
		}
		
		next_prompt.print(&model, entry.display, &choices, &text_context)?;

		match model {
			PromptModel::Redirect(choice) => player.choose_full(choice, None, config, resources, &model, &text_context)?,
			PromptModel::Ending(lines) => {
				Text::print_lines(lines, &text_context)?;
				break 'outer true
			},
			_ => loop {
				let context = next_input_context(&model, &choices, &text_context)?
					.ok_or(anyhow!("Could not resolve input context"))?;
				// Borrow-checker coercion; only using necessary fields in static method
				match take_input(input, &context, config, player, saves, resources, &model, &text_context, &choices)? {
					GameLoopResult::Retry(flush) => if flush { println!() },
					GameLoopResult::Continue => { println!(); break },
					GameLoopResult::Shutdown(silent) => break 'outer silent
				}
			}
		}
	};
	Ok(silent)
}

pub fn crash_context(config: &Manifest) -> String {
	let contact = config.metadata.contact.as_ref().map(|info| {
		let strings: Vec<String> = info.iter()
			.map(|value| format!("- {value}"))
			.collect();
		format!("\n\nContact the developers:\n{}", strings.join("\n"))
	});
	format!("The game has crashed; it's not your fault!{}", contact.unwrap_or(String::new()))
}