//! Session restore: when the session ends, the programs that had windows are written to
//! `~/.local/state/fde/session`; the next session starts them again (`session.restore = true`).
//! Programs are found again by their app id in the installed applications, so only programs
//! with a desktop file come back (and each only once). The windows open where they were last
//! closed (`shell.remember_windows`).

use std::path::PathBuf;

use crate::apps::App;

pub fn state_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME").map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("fde").join("session"))
}

/// The app ids to write: each once, in the order of the windows; names that would break the
/// file are left out.
pub fn to_text(ids: &[String]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for id in ids {
        if !id.is_empty() && !id.contains(['\t', '\n', '\r']) && !seen.contains(&id.as_str()) {
            seen.push(id);
        }
    }
    seen.iter().take(32).map(|s| format!("{s}\n")).collect()
}

pub fn parse(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty()).take(64) {
        if !out.iter().any(|o| o == l) {
            out.push(l.to_string());
        }
    }
    out
}

/// The commands to start: the installed program of each saved id, except those that run
/// already (`running`: app ids of the open windows, e.g. started by autostart).
pub fn commands(saved: &[String], apps: &[App], running: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for id in saved {
        if running.contains(id) {
            continue;
        }
        if let Some(app) = crate::apps::for_window(apps, id) {
            if !app.exec.trim().is_empty() && !out.contains(&app.exec) {
                out.push(app.exec.clone());
            }
        }
    }
    out
}

/// Write the list (an empty list removes the file).
pub fn save(ids: &[String]) {
    let Some(f) = state_file() else { return };
    let text = to_text(ids);
    if text.is_empty() {
        let _ = std::fs::remove_file(&f);
        return;
    }
    if let Some(dir) = f.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let tmp = f.with_extension("tmp");
    if std::fs::write(&tmp, text).is_ok() {
        let _ = std::fs::rename(&tmp, &f);
    }
}

pub fn load() -> Vec<String> {
    state_file().and_then(|f| std::fs::read_to_string(f).ok()).map(|t| parse(&t)).unwrap_or_default()
}

/// Start the saved programs (the file is read once and removed, so a crash loop cannot restart
/// a program that crashes the session again and again).
pub fn restore() {
    let saved = load();
    if let Some(f) = state_file() {
        let _ = std::fs::remove_file(f);
    }
    for cmd in commands(&saved, &crate::apps::installed(), &[]) {
        tracing::info!("session restore: {cmd}");
        if let Err(e) = std::process::Command::new("sh").arg("-c").arg(&cmd).spawn() {
            tracing::warn!("cannot restore {cmd}: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(id: &str, exec: &str) -> App {
        App { name: id.into(), exec: exec.into(), id: id.into(), ..Default::default() }
    }

    #[test]
    fn the_list_is_written_and_read_back_without_duplicates_or_broken_names() {
        let ids: Vec<String> = ["foot", "foot", "org.gnome.Nautilus", "bad\tname", "", "x\ny"].iter().map(|s| s.to_string()).collect();
        assert_eq!(to_text(&ids), "foot\norg.gnome.Nautilus\n");
        assert_eq!(parse("foot\n\n  org.gnome.Nautilus \nfoot\n"), vec!["foot", "org.gnome.Nautilus"]);
        assert!(parse("").is_empty());
    }

    #[test]
    fn only_installed_programs_that_are_not_running_come_back() {
        let apps = vec![app("foot", "foot"), app("org.gnome.Nautilus", "nautilus --new-window")];
        let saved: Vec<String> = ["foot", "org.gnome.Nautilus", "ghost"].iter().map(|s| s.to_string()).collect();
        assert_eq!(commands(&saved, &apps, &[]), vec!["foot", "nautilus --new-window"]);
        assert_eq!(commands(&saved, &apps, &["foot".to_string()]), vec!["nautilus --new-window"]);
        assert!(commands(&[], &apps, &[]).is_empty());
    }
}
