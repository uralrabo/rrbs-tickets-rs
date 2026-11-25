//! Ticket attachments, stored as plain files:
//!
//!     attachments/<adapter name>/<ticket id>/<file name>
//!
//! The folder is relative to the directory the program is started in, just like `local.db3`.
//! The ticket id is never reused (the table uses AUTOINCREMENT), so a deleted ticket can leave
//! an unused folder behind but can never hand its files to a new ticket.
//! Moving a ticket to another adapter gives it a new id, so its attachments stay with the old one.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use eframe::egui::{Button, Id, RichText, Ui};

const ROOT: &str = "attachments";

fn safe_component(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' { c } else { '_' })
        .collect()
}

pub fn ticket_dir(adapter: &str, ticket_id: i64) -> PathBuf {
    Path::new(ROOT).join(safe_component(adapter)).join(ticket_id.to_string())
}

fn unique_path(dir: &Path, file_name: &str) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }

    let path = Path::new(file_name);
    let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "file".to_string());
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();

    let mut n = 2;
    loop {
        let candidate = dir.join(format!("{} ({}){}", stem, n, ext));
        if !candidate.exists() {
            return candidate;
        }
        n += 1;
    }
}

/// File name and size of every attachment of the ticket, sorted by name.
pub fn list(adapter: &str, ticket_id: i64) -> Vec<(String, u64)> {
    let mut out: Vec<(String, u64)> = vec![];

    if let Ok(entries) = fs::read_dir(ticket_dir(adapter, ticket_id)) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                if meta.is_file() {
                    out.push((entry.file_name().to_string_lossy().to_string(), meta.len()));
                }
            }
        }
    }

    out.sort();
    out
}

/// Copies a file into the ticket's folder. Returns the name it was stored under.
pub fn add(adapter: &str, ticket_id: i64, source: &Path) -> Result<String, String> {
    if !source.is_file() {
        return Err(format!("{} is not a file", source.display()));
    }

    let file_name = source
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "the file has no name".to_string())?;

    let dir = ticket_dir(adapter, ticket_id);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let target = unique_path(&dir, &file_name);
    fs::copy(source, &target).map_err(|e| e.to_string())?;

    Ok(target
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or(file_name))
}

fn attachment_path(adapter: &str, ticket_id: i64, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name == ".." || name.contains('/') || name.contains('\\') {
        return Err("invalid file name".to_string());
    }
    Ok(ticket_dir(adapter, ticket_id).join(name))
}

pub fn remove(adapter: &str, ticket_id: i64, name: &str) -> Result<(), String> {
    let path = attachment_path(adapter, ticket_id, name)?;
    fs::remove_file(&path).map_err(|e| e.to_string())?;
    // Drops the folder again when it is empty; fails harmlessly when it is not.
    let _ = fs::remove_dir(ticket_dir(adapter, ticket_id));
    Ok(())
}

fn download_dir() -> PathBuf {
    match std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        Some(home) => PathBuf::from(home).join("Downloads"),
        None => PathBuf::from("."),
    }
}

/// Copies the attachment into the user's Downloads folder and returns where it ended up.
pub fn save_copy(adapter: &str, ticket_id: i64, name: &str) -> Result<PathBuf, String> {
    let source = attachment_path(adapter, ticket_id, name)?;
    let dir = download_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;

    let target = unique_path(&dir, name);
    fs::copy(&source, &target).map_err(|e| e.to_string())?;
    Ok(target)
}

#[cfg(target_os = "windows")]
fn open_path(path: &Path) -> Result<(), String> {
    Command::new("cmd").args(["/C", "start", ""]).arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn open_path(path: &Path) -> Result<(), String> {
    Command::new("open").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn open_path(path: &Path) -> Result<(), String> {
    Command::new("xdg-open").arg(path).spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Opens the attachment with the program the system uses for that file type.
pub fn open(adapter: &str, ticket_id: i64, name: &str) -> Result<(), String> {
    let path = attachment_path(adapter, ticket_id, name)?;
    open_path(&path)
}

/// What the file dialog thread hands back: the chosen files, or an error message.
type PickResult = Arc<Mutex<Option<Result<Vec<PathBuf>, String>>>>;

fn lines_to_paths(bytes: &[u8]) -> Vec<PathBuf> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(|l| l.trim_end_matches('\r'))
        .filter(|l| !l.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Shows the system's file dialog through zenity or kdialog, so no extra library is needed.
/// A cancelled dialog returns an empty list.
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn pick_files() -> Result<Vec<PathBuf>, String> {
    let attempts: Vec<(&str, Vec<&str>)> = vec![
        ("zenity", vec!["--file-selection", "--multiple", "--separator=\n", "--title=Attach files"]),
        ("kdialog", vec!["--getopenfilename", ".", "--multiple", "--separate-output", "--title", "Attach files"]),
    ];

    for (program, args) in attempts.iter() {
        match Command::new(program).args(args.iter()).output() {
            Ok(output) => {
                // 0 means files were chosen, 1 means the dialog was cancelled.
                let code = output.status.code();
                if code != Some(0) && code != Some(1) {
                    let text = String::from_utf8_lossy(&output.stderr).trim().to_string();
                    return Err(if text.is_empty() { format!("{} failed", program) } else { text });
                }
                return Ok(lines_to_paths(&output.stdout));
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err.to_string()),
        }
    }

    Err("No file dialog found. Install zenity or kdialog, or type the path.".to_string())
}

#[cfg(target_os = "macos")]
fn pick_files() -> Result<Vec<PathBuf>, String> {
    let output = Command::new("osascript")
        .args(["-e", "POSIX path of (choose file)"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Ok(vec![]);
    }
    Ok(lines_to_paths(&output.stdout))
}

#[cfg(target_os = "windows")]
fn pick_files() -> Result<Vec<PathBuf>, String> {
    let script = "Add-Type -AssemblyName System.Windows.Forms; $d = New-Object System.Windows.Forms.OpenFileDialog; $d.Multiselect = $true; if ($d.ShowDialog() -eq 'OK') { $d.FileNames -join \"`n\" }";
    let output = Command::new("powershell")
        .args(["-NoProfile", "-STA", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    Ok(lines_to_paths(&output.stdout))
}

fn human_size(size: u64) -> String {
    let s = size as f64;
    if s < 1024.0 {
        format!("{} B", size)
    } else if s < 1024.0 * 1024.0 {
        format!("{:.1} KB", s / 1024.0)
    } else {
        format!("{:.1} MB", s / (1024.0 * 1024.0))
    }
}

fn expand_path(text: &str) -> PathBuf {
    let text = text.trim().trim_matches('"');
    if let Some(rest) = text.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(text)
}

/// The attachments section for the ticket masks. Files dropped onto the window are attached to
/// the ticket, or a path can be typed in. Does nothing for a ticket that has no id yet.
pub fn attachments_field(ui: &mut Ui, id: &str, adapter: &str, ticket_id: i64) {
    if ticket_id <= 0 {
        return;
    }

    let message_key = Id::new(("attachment_message", id));
    let path_key = Id::new(("attachment_path", id));
    let mut message: Option<String> = None;

    ui.add_space(6.0);
    ui.label(RichText::new("Attachments").strong());

    // Files dropped onto the window.
    let dropped: Vec<PathBuf> = ui.ctx().input(|i| {
        i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect()
    });
    for path in dropped {
        message = Some(match add(adapter, ticket_id, &path) {
            Ok(name) => format!("Attached {}", name),
            Err(err) => format!("Could not attach: {}", err),
        });
    }

    // Result of the file dialog, if one was opened.
    let pick_key = Id::new(("attachment_pick", id));
    let pending: Option<PickResult> = ui.ctx().data(|d| d.get_temp(pick_key));
    if let Some(pending_pick) = &pending {
        let finished = pending_pick.lock().ok().and_then(|mut guard| guard.take());
        match finished {
            Some(Ok(paths)) => {
                for path in paths {
                    message = Some(match add(adapter, ticket_id, &path) {
                        Ok(name) => format!("Attached {}", name),
                        Err(err) => format!("Could not attach: {}", err),
                    });
                }
                ui.ctx().data_mut(|d| d.remove::<PickResult>(pick_key));
            },
            Some(Err(err)) => {
                message = Some(format!("File dialog: {}", err));
                ui.ctx().data_mut(|d| d.remove::<PickResult>(pick_key));
            },
            None => ui.ctx().request_repaint_after(Duration::from_millis(200)),
        }
    }
    let waiting = ui.ctx().data(|d| d.get_temp::<PickResult>(pick_key)).is_some();

    let files = list(adapter, ticket_id);
    if files.is_empty() {
        ui.label(RichText::new("No attachments yet. Use Browse..., drop files onto this window or type a path below.").small());
    }

    for (name, size) in &files {
        ui.horizontal(|ui| {
            ui.label(format!("{} ({})", name, human_size(*size)));

            if ui.small_button("Open").on_hover_text("Opens the file with the default program.").clicked() {
                message = Some(match open(adapter, ticket_id, name) {
                    Ok(_) => format!("Opened {}", name),
                    Err(err) => format!("Could not open {}: {}", name, err),
                });
            }

            if ui.small_button("Save copy").on_hover_text("Copies the file into your Downloads folder.").clicked() {
                message = Some(match save_copy(adapter, ticket_id, name) {
                    Ok(path) => format!("Saved a copy to {}", path.display()),
                    Err(err) => format!("Could not save {}: {}", name, err),
                });
            }

            if ui.small_button("Remove").on_hover_text("Deletes the attachment from this ticket.").clicked() {
                message = Some(match remove(adapter, ticket_id, name) {
                    Ok(_) => format!("Removed {}", name),
                    Err(err) => format!("Could not remove {}: {}", name, err),
                });
            }
        });
    }

    ui.horizontal(|ui| {
        if ui.add_enabled(!waiting, Button::new("Browse..."))
            .on_hover_text("Choose files with the system file dialog.")
            .clicked() {
            let result: PickResult = Arc::new(Mutex::new(None));
            let thread_result = result.clone();
            let ctx = ui.ctx().clone();

            // The dialog runs in its own thread so the window stays responsive.
            thread::spawn(move || {
                let picked = pick_files();
                if let Ok(mut guard) = thread_result.lock() {
                    *guard = Some(picked);
                }
                ctx.request_repaint();
            });

            ui.ctx().data_mut(|d| d.insert_temp(pick_key, result));
        }
        if waiting {
            ui.label(RichText::new("Waiting for the file dialog...").small());
        }
    });

    // Typing a path is the fallback when no file dialog is available.
    let mut path_text: String = ui.ctx().data(|d| d.get_temp(path_key)).unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label("Add file:");
        ui.text_edit_singleline(&mut path_text);
        if ui.button("Add").clicked() && !path_text.trim().is_empty() {
            message = Some(match add(adapter, ticket_id, &expand_path(&path_text)) {
                Ok(name) => {
                    path_text.clear();
                    format!("Attached {}", name)
                },
                Err(err) => format!("Could not attach: {}", err),
            });
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(path_key, path_text));

    if let Some(text) = message {
        ui.ctx().data_mut(|d| d.insert_temp(message_key, text));
    }
    if let Some(text) = ui.ctx().data(|d| d.get_temp::<String>(message_key)) {
        ui.label(RichText::new(text).small());
    }
}
