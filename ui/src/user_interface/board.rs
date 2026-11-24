use std::collections::HashMap;

use chrono::{Datelike, Local, NaiveDate, TimeZone, Utc};
use eframe::egui::{
    CollapsingHeader,
    Color32,
    Frame,
    Grid,
    Label,
    RichText,
    Rounding,
    Sense,
    Ui,
    style::Margin
};

use tickets_rs_core::Ticket;

use crate::{
    UserInterface,
    UITheme,
    UICache,
    ui_controller::{
        BoardState,
        ticket_actions::{
            TicketAction,
            Identifier
        }
    }
};

/// Tags that count as a priority, from highest to lowest. Matching ignores case.
/// Change these to the tag names you actually use for priorities.
const PRIORITY_TAGS: [&str; 3] = ["high", "medium", "low"];

fn is_priority_tag(tag: &str) -> bool {
    PRIORITY_TAGS.iter().any(|p| tag.eq_ignore_ascii_case(p))
}

/// Index into PRIORITY_TAGS of the highest priority the ticket carries, if any.
fn priority_rank(ticket: &Ticket) -> Option<usize> {
    PRIORITY_TAGS
        .iter()
        .position(|p| ticket.tags.iter().any(|t| t.eq_ignore_ascii_case(p)))
}

fn priority_color(rank: usize) -> Color32 {
    match rank {
        0 => Color32::from_rgb(224, 82, 72),
        1 => Color32::from_rgb(230, 160, 50),
        _ => Color32::from_rgb(110, 165, 120),
    }
}

/// The "warten bis" date from the description. The parsing lives in overlays/wait_until.rs
/// so that the ticket masks and these views always agree.
fn wait_date(ticket: &Ticket) -> Option<NaiveDate> {
    crate::overlays::wait_until::wait_date(&ticket.description)
}

/// Unix seconds -> local calendar date. Returns None for unset (<= 0) values.
fn ticket_date(ts: i64) -> Option<NaiveDate> {
    if ts <= 0 {
        return None;
    }

    Utc.timestamp_opt(ts, 0)
        .single()
        .map(|dt| dt.with_timezone(&Local).naive_local().date())
}

/// Keeps the saved column order, drops columns that no longer exist and appends new ones.
fn sync_columns(board: &mut BoardState, tickets: &[Ticket], cache: &UICache) {
    let mut names: Vec<String> = cache.states.keys().map(|s| s.name.clone()).collect();
    for ticket in tickets {
        names.push(ticket.state_name.clone());
    }
    names.sort();
    names.dedup();

    board.columns.retain(|c| names.contains(c));
    for name in names {
        if !board.columns.contains(&name) {
            board.columns.push(name);
        }
    }
}

/// Next column in the given direction that the ticket's adapter actually has as a state.
fn neighbour_state(columns: &[String], from: usize, step: isize, ticket: &Ticket, cache: &UICache) -> Option<String> {
    let mut i = from as isize + step;
    while i >= 0 && (i as usize) < columns.len() {
        let name = &columns[i as usize];
        if cache.states.keys().any(|s| &s.name == name && s.adapter == ticket.adapter) {
            return Some(name.clone());
        }
        i += step;
    }
    None
}

/// One clickable line for a ticket. Left click edits it, right click opens the normal ticket menu.
fn ticket_chip(ui: &mut Ui, ticket: &Ticket, action: &mut TicketAction, theme: &UITheme, cache: &UICache) {
    ticket_chip_ex(ui, ticket, action, theme, cache, "• ", theme.foreground_secondary);
}

fn ticket_chip_ex(ui: &mut Ui, ticket: &Ticket, action: &mut TicketAction, theme: &UITheme, cache: &UICache, prefix: &str, color: Color32) {
    let response = ui.add(
        Label::new(RichText::new(format!("{}{}", prefix, ticket.title)).color(color))
            .truncate(true)
            .sense(Sense::click())
    ).on_hover_text(format!("{} ({})\nClick to edit, right-click for more options.", ticket.title, ticket.state_name));

    let clicked = response.clicked();
    UserInterface::update_ticket_context_menu(response, action, ui, ticket, theme, cache);

    if clicked {
        *action = TicketAction::UpdateDetails(Identifier::new(&ticket.adapter, ticket.id));
    }
}

impl UserInterface {

    /// Red bar listing every ticket whose "warten bis" date is today or in the past.
    pub fn update_wait_banner(ui: &mut Ui, tickets: &[Ticket]) -> TicketAction {
        let mut action = TicketAction::None;
        let today = Local::now().naive_local().date();

        let mut due: Vec<(NaiveDate, &Ticket)> = tickets
            .iter()
            .filter_map(|t| wait_date(t).filter(|d| *d <= today).map(|d| (d, t)))
            .collect();

        if due.is_empty() {
            return action;
        }
        due.sort_by_key(|(date, ticket)| (*date, ticket.id));

        Frame::none()
            .fill(Color32::from_rgb(150, 35, 35))
            .inner_margin(Margin::same(8.0))
            .rounding(Rounding::same(4.0))
            .show(ui, |ui| {
                ui.label(
                    RichText::new(format!("{} ticket(s) done waiting", due.len()))
                        .strong()
                        .color(Color32::WHITE)
                );

                for (date, ticket) in due {
                    let response = ui.add(
                        Label::new(
                            RichText::new(format!("{} (since {})", ticket.title, date.format("%d.%m.%Y")))
                                .color(Color32::WHITE)
                                .underline()
                        ).sense(Sense::click())
                    ).on_hover_text("Click to open. Remove the 'warten bis' line from the description to dismiss this.");

                    if response.clicked() {
                        action = TicketAction::UpdateDetails(Identifier::new(&ticket.adapter, ticket.id));
                    }
                }
            });

        ui.add_space(8.0);

        action
    }

    pub fn update_kanban(
        ui: &mut Ui,
        tickets: &[Ticket],
        board: &mut BoardState,
        theme: &UITheme,
        cache: &UICache
    ) -> TicketAction {
        let mut action = TicketAction::None;

        sync_columns(board, tickets, cache);

        let today = Local::now().naive_local().date();
        let col_width = (theme.font_size as f32 * 18.0).max(240.0);
        let columns = board.columns.clone();
        let mut swap: Option<(usize, usize)> = None;

        ui.horizontal_top(|ui| {
            for (ci, name) in columns.iter().enumerate() {
                let mut in_column: Vec<&Ticket> = tickets.iter().filter(|t| &t.state_name == name).collect();
                in_column.sort_by_key(|t| priority_rank(t).unwrap_or(usize::MAX));

                ui.vertical(|ui| {
                    ui.set_width(col_width);

                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} ({})", name, in_column.len())).strong());

                        if ci > 0 && ui.small_button("<").on_hover_text("Move column left").clicked() {
                            swap = Some((ci, ci - 1));
                        }
                        if ci + 1 < columns.len() && ui.small_button(">").on_hover_text("Move column right").clicked() {
                            swap = Some((ci, ci + 1));
                        }
                    });

                    ui.separator();

                    for ticket in in_column {
                        Frame::group(ui.style()).show(ui, |ui| {
                            ui.set_width(col_width - 20.0);

                            if let Some(rank) = priority_rank(ticket) {
                                ui.label(
                                    RichText::new(PRIORITY_TAGS[rank].to_uppercase())
                                        .color(priority_color(rank))
                                        .strong()
                                        .small()
                                );
                            }

                            let title = ui.add(
                                Label::new(RichText::new(&ticket.title).strong())
                                    .sense(Sense::click())
                            ).on_hover_text("Click to edit. Right-click for more options.");

                            let clicked = title.clicked();
                            UserInterface::update_ticket_context_menu(title, &mut action, ui, ticket, theme, cache);
                            if clicked {
                                action = TicketAction::UpdateDetails(Identifier::new(&ticket.adapter, ticket.id));
                            }

                            let mut meta: Vec<String> = vec![];
                            if !ticket.assigned_to.is_empty() {
                                meta.push(ticket.assigned_to.clone());
                            }
                            if let Some(date) = ticket_date(ticket.due_at) {
                                meta.push(format!("due {}", date.format("%d.%m.%Y")));
                            }
                            let other_tags: Vec<String> = ticket.tags.iter()
                                .filter(|t| !is_priority_tag(t))
                                .cloned()
                                .collect();
                            if !other_tags.is_empty() {
                                meta.push(other_tags.join(", "));
                            }
                            if !meta.is_empty() {
                                ui.label(RichText::new(meta.join(" · ")).color(theme.foreground_secondary).small());
                            }

                            if let Some(date) = wait_date(ticket) {
                                if date <= today {
                                    ui.label(
                                        RichText::new(format!("Done waiting ({})", date.format("%d.%m.%Y")))
                                            .color(Color32::from_rgb(224, 82, 72))
                                            .strong()
                                    );
                                } else {
                                    ui.label(
                                        RichText::new(format!("Waiting until {}", date.format("%d.%m.%Y")))
                                            .color(Color32::from_rgb(230, 160, 50))
                                    );
                                }
                            }

                            ui.horizontal(|ui| {
                                if let Some(prev) = neighbour_state(&columns, ci, -1, ticket, cache) {
                                    if ui.small_button("<").on_hover_text(format!("Move to {}", prev)).clicked() {
                                        action = TicketAction::UpdateStateImmediate(
                                            Identifier::new(&ticket.adapter, ticket.id),
                                            prev
                                        );
                                    }
                                }
                                if let Some(next) = neighbour_state(&columns, ci, 1, ticket, cache) {
                                    if ui.small_button(">").on_hover_text(format!("Move to {}", next)).clicked() {
                                        action = TicketAction::UpdateStateImmediate(
                                            Identifier::new(&ticket.adapter, ticket.id),
                                            next
                                        );
                                    }
                                }
                            });
                        });
                        ui.add_space(6.0);
                    }
                });

                ui.add_space(8.0);
            }
        });

        if let Some((a, b)) = swap {
            board.columns.swap(a, b);
            board.order_changed = true;
        }

        action
    }

    pub fn update_calendar(
        ui: &mut Ui,
        tickets: &[Ticket],
        board: &mut BoardState,
        theme: &UITheme,
        cache: &UICache
    ) -> TicketAction {
        let mut action = TicketAction::None;

        let today = Local::now().naive_local().date();
        let first = NaiveDate::from_ymd_opt(board.cal_year, board.cal_month, 1).unwrap_or(today);

        ui.horizontal(|ui| {
            if ui.button("<").on_hover_text("Previous month").clicked() {
                if board.cal_month <= 1 {
                    board.cal_month = 12;
                    board.cal_year -= 1;
                } else {
                    board.cal_month -= 1;
                }
            }

            ui.heading(first.format("%B %Y").to_string());

            if ui.button(">").on_hover_text("Next month").clicked() {
                if board.cal_month >= 12 {
                    board.cal_month = 1;
                    board.cal_year += 1;
                } else {
                    board.cal_month += 1;
                }
            }

            if ui.button("Today").clicked() {
                board.cal_year = today.year();
                board.cal_month = today.month();
            }
        });

        ui.add_space(6.0);

        let (next_year, next_month) = if first.month() == 12 {
            (first.year() + 1, 1)
        } else {
            (first.year(), first.month() + 1)
        };
        let next_first = NaiveDate::from_ymd_opt(next_year, next_month, 1).unwrap_or(first);
        let days_in_month = next_first.signed_duration_since(first).num_days().max(0) as u32;

        let mut by_day: HashMap<NaiveDate, Vec<&Ticket>> = HashMap::new();
        let mut undated: Vec<&Ticket> = vec![];
        for ticket in tickets {
            match ticket_date(ticket.due_at) {
                Some(date) => by_day.entry(date).or_default().push(ticket),
                None => undated.push(ticket),
            }
        }

        let mut wait_by_day: HashMap<NaiveDate, Vec<&Ticket>> = HashMap::new();
        for ticket in tickets {
            if let Some(date) = wait_date(ticket) {
                wait_by_day.entry(date).or_default().push(ticket);
            }
        }

        let in_month = tickets.iter()
            .filter(|t| ticket_date(t.due_at).map_or(false, |d| d >= first && d < next_first))
            .count();

        if tickets.is_empty() {
            ui.label(RichText::new("No tickets loaded. Click an entry in the sidebar once, then come back.").color(theme.foreground_secondary));
        } else {
            ui.label(
                RichText::new(format!(
                    "{} tickets in total, {} due this month, {} without due date",
                    tickets.len(), in_month, undated.len()
                )).color(theme.foreground_secondary)
            );
        }
        ui.add_space(4.0);

        let cell_width = (theme.font_size as f32 * 11.0).max(120.0);
        let offset = first.weekday().num_days_from_monday();
        let weeks = (offset + days_in_month + 6) / 7;

        Grid::new("calendar_grid")
            .spacing([6.0, 6.0])
            .min_col_width(cell_width)
            .show(ui, |ui| {
                for weekday in ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"] {
                    ui.label(RichText::new(weekday).strong().color(theme.foreground_secondary));
                }
                ui.end_row();

                for cell in 0..(weeks * 7) {
                    let day = cell as i64 - offset as i64 + 1;

                    ui.vertical(|ui| {
                        ui.set_min_width(cell_width);
                        ui.set_min_height(90.0);

                        if day >= 1 && day <= days_in_month as i64 {
                            let date = NaiveDate::from_ymd_opt(first.year(), first.month(), day as u32);

                            let mut day_text = RichText::new(day.to_string()).strong();
                            if date == Some(today) {
                                day_text = day_text.color(theme.foreground_marker2);
                            }
                            ui.label(day_text);

                            if let Some(list) = date.and_then(|d| by_day.get(&d)) {
                                for ticket in list {
                                    ticket_chip(ui, ticket, &mut action, theme, cache);
                                }
                            }

                            if let Some(list) = date.and_then(|d| wait_by_day.get(&d)) {
                                for ticket in list {
                                    ticket_chip_ex(ui, ticket, &mut action, theme, cache, "wait: ", Color32::from_rgb(230, 160, 50));
                                }
                            }
                        }
                    });

                    if cell % 7 == 6 {
                        ui.end_row();
                    }
                }
            });

        let mut elsewhere: Vec<(NaiveDate, &Ticket)> = vec![];
        for ticket in tickets {
            if let Some(date) = ticket_date(ticket.due_at) {
                if date < first || date >= next_first {
                    elsewhere.push((date, ticket));
                }
            }
        }
        elsewhere.sort_by_key(|(date, ticket)| (*date, ticket.id));

        if !elsewhere.is_empty() {
            ui.add_space(10.0);
            CollapsingHeader::new(format!("Tickets in other months ({})", elsewhere.len()))
                .default_open(in_month == 0)
                .show(ui, |ui| {
                    for (date, ticket) in elsewhere {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(date.format("%d.%m.%Y").to_string()).color(theme.foreground_secondary));
                            ticket_chip(ui, ticket, &mut action, theme, cache);
                        });
                    }
                });
        }

        if !undated.is_empty() {
            ui.add_space(10.0);
            ui.label(RichText::new(format!("No due date ({})", undated.len())).strong());
            for ticket in undated {
                ticket_chip(ui, ticket, &mut action, theme, cache);
            }
        }

        action
    }
}
