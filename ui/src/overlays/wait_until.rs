//! "Wait until" support.
//!
//! A ticket waits until a date when its description contains a line such as
//! `warten bis: 2026-12-01` or `Warten bis 01.12.2026`. The ticket itself has no field for it,
//! so the date lives in that line. The Kanban board, the calendar and the red banner read it
//! through `wait_date`, and the ticket masks edit it through `wait_field`.

use chrono::{Local, NaiveDate};
use eframe::egui::Ui;
use egui_extras::DatePickerButton;

const PREFIXES: [&str; 3] = ["warten bis", "warten-bis", "wait until"];

/// Returns what follows the wait keyword, lowercased, if the line is a wait line.
fn strip_wait_prefix(line: &str) -> Option<String> {
    let lower = line
        .trim()
        .trim_start_matches(|c: char| c == '*' || c == '_' || c == '-' || c == '#' || c == '>' || c.is_whitespace())
        .to_lowercase();

    for prefix in PREFIXES.iter() {
        if let Some(rest) = lower.strip_prefix(*prefix) {
            return Some(rest.to_string());
        }
    }
    None
}

fn parse_date_token(token: &str) -> Option<NaiveDate> {
    let token = token.trim_matches(|c: char| !c.is_ascii_digit() && c != '-' && c != '.');
    NaiveDate::parse_from_str(token, "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(token, "%d.%m.%Y").ok())
}

/// The date of the first valid wait line in the description.
pub fn wait_date(description: &str) -> Option<NaiveDate> {
    for line in description.lines() {
        if let Some(rest) = strip_wait_prefix(line) {
            let rest = rest.trim_start_matches(|c: char| c == ':' || c == '*' || c.is_whitespace());
            if let Some(date) = rest.split_whitespace().next().and_then(parse_date_token) {
                return Some(date);
            }
        }
    }
    None
}

/// States whose name starts with "warten" (or "wait") show the date field.
pub fn is_wait_state(state_name: &str) -> bool {
    let lower = state_name.trim().to_lowercase();
    lower.starts_with("warten") || lower.starts_with("wait")
}

/// Writes the wait line: replaces the first existing one, or appends it at the end.
pub fn set_wait_line(description: &mut String, date: NaiveDate) {
    let new_line = format!("warten bis: {}", date.format("%Y-%m-%d"));
    let mut replaced = false;
    let mut out: Vec<String> = vec![];

    for line in description.lines() {
        if !replaced && strip_wait_prefix(line).is_some() {
            out.push(new_line.clone());
            replaced = true;
        } else {
            out.push(line.to_string());
        }
    }

    if !replaced {
        if out.last().map_or(false, |l| !l.trim().is_empty()) {
            out.push(String::new());
        }
        out.push(new_line);
    }

    *description = out.join("\n");
}

/// Removes every wait line from the description.
pub fn remove_wait_line(description: &mut String) {
    let joined = description
        .lines()
        .filter(|l| strip_wait_prefix(l).is_none())
        .collect::<Vec<&str>>()
        .join("\n")
        .trim_end()
        .to_string();
    *description = joined;
}

/// The date field for the ticket masks. It shows up when the ticket is in a "warten ..."
/// state, or when the description already has a wait line, and edits that line.
pub fn wait_field(ui: &mut Ui, id: &str, state_name: &str, description: &mut String) {
    if !is_wait_state(state_name) && wait_date(description).is_none() {
        return;
    }

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Wait until:");

        match wait_date(description) {
            Some(mut date) => {
                let before = date;
                ui.add(DatePickerButton::new(&mut date).id_source(id));
                if date != before {
                    set_wait_line(description, date);
                }
                if ui.small_button("Remove")
                    .on_hover_text("Removes the 'warten bis' line from the description.")
                    .clicked() {
                    remove_wait_line(description);
                }
            },
            None => {
                if ui.button("Set date")
                    .on_hover_text("Adds a 'warten bis' line to the description, starting with today.")
                    .clicked() {
                    set_wait_line(description, Local::now().naive_local().date());
                }
            }
        }
    });
}
