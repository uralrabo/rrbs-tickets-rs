//! Ticket priorities.
//!
//! The ticket has no priority field, so a priority is a tag. The names below decide which tags
//! count as priorities, from the highest to the lowest. Matching ignores case.
//! Change them to the tag names you actually use (and keep the number in the brackets in sync).

use eframe::egui::{Color32, ComboBox, RichText, Ui};

pub const PRIORITY_TAGS: [&str; 3] = ["high", "medium", "low"];

pub fn is_priority_tag(tag: &str) -> bool {
    PRIORITY_TAGS.iter().any(|p| tag.eq_ignore_ascii_case(p))
}

/// Index into PRIORITY_TAGS of the highest priority among the tags, if there is one.
pub fn priority_rank(tags: &[String]) -> Option<usize> {
    PRIORITY_TAGS
        .iter()
        .position(|p| tags.iter().any(|t| t.eq_ignore_ascii_case(p)))
}

pub fn priority_color(rank: usize) -> Color32 {
    match rank {
        0 => Color32::from_rgb(224, 82, 72),
        1 => Color32::from_rgb(230, 160, 50),
        _ => Color32::from_rgb(110, 165, 120),
    }
}

/// Dropdown for the ticket masks. Picking a priority replaces any other priority tag on the
/// ticket, "none" removes them. `existing` holds the tag names that exist for the ticket's
/// adapter, so the mask can warn about a priority tag that has not been created yet.
pub fn priority_field(ui: &mut Ui, id: &str, tags: &mut Vec<String>, existing: Option<&[String]>) {
    let current = priority_rank(tags);
    let mut selected = current;

    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Priority:");

        let shown = match selected {
            Some(rank) => PRIORITY_TAGS[rank].to_string(),
            None => "none".to_string(),
        };

        ComboBox::from_id_source(id)
            .selected_text(shown)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, None, "none");
                for (rank, name) in PRIORITY_TAGS.iter().enumerate() {
                    ui.selectable_value(&mut selected, Some(rank), *name);
                }
            });
    });

    if selected != current {
        tags.retain(|t| !is_priority_tag(t));

        if let Some(rank) = selected {
            // Use the spelling of the existing tag, if the adapter has one.
            let name = existing
                .and_then(|names| names.iter().find(|n| n.eq_ignore_ascii_case(PRIORITY_TAGS[rank])))
                .cloned()
                .unwrap_or_else(|| PRIORITY_TAGS[rank].to_string());
            tags.push(name);
        }
    }

    if let (Some(rank), Some(names)) = (selected, existing) {
        if !names.iter().any(|n| n.eq_ignore_ascii_case(PRIORITY_TAGS[rank])) {
            ui.label(
                RichText::new(format!(
                    "The tag '{}' does not exist for this adapter yet. Create it in the sidebar first.",
                    PRIORITY_TAGS[rank]
                ))
                .color(Color32::from_rgb(230, 160, 50))
                .small()
            );
        }
    }
}
