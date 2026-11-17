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

/// A program that was open: its app id and the workspace of its first window (0 = the first).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    pub workspace: usize,
}

/// The entries to write ("id TAB workspace" per line): each program once, in the order of the
/// windows; names that would break the file are left out.
pub fn to_text(entries: &[Entry]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    let mut out = String::new();
    for e in entries {
        if !e.id.is_empty() && !e.id.contains(['\t', '\n', '\r']) && !seen.contains(&e.id.as_str()) && seen.len() < 32 {
            seen.push(&e.id);
            out.push_str(&format!("{}\t{}\n", e.id, e.workspace));
        }
    }
    out
}

/// Read the file (a line without a workspace, as older versions wrote, means the first one).
pub fn parse(text: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for l in text.lines().map(str::trim).filter(|l| !l.is_empty()).take(64) {
        let (id, ws) = l.split_once('\t').unwrap_or((l, "0"));
        let workspace = ws.trim().parse::<usize>().unwrap_or(0).min(8);
        if !out.iter().any(|o| o.id == id) {
            out.push(Entry { id: id.to_string(), workspace });
        }
    }
    out
}

/// The commands to start: the installed program of each saved id, except those that run
/// already (`running`: app ids of the open windows, e.g. started by autostart).
pub fn commands(saved: &[Entry], apps: &[App], running: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in saved {
        if running.contains(&e.id) {
            continue;
        }
        if let Some(app) = crate::apps::for_window(apps, &e.id) {
            if !app.exec.trim().is_empty() && !out.contains(&app.exec) {
                out.push(app.exec.clone());
            }
        }
    }
    out
}

/// Write the list (an empty list removes the file).
pub fn save(entries: &[Entry]) {
    let Some(f) = state_file() else { return };
    let text = to_text(entries);
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

pub fn load() -> Vec<Entry> {
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

    fn entry(id: &str, workspace: usize) -> Entry {
        Entry { id: id.into(), workspace }
    }

    #[test]
    fn the_list_is_written_and_read_back_without_duplicates_or_broken_names() {
        let e = vec![entry("foot", 1), entry("foot", 2), entry("org.gnome.Nautilus", 0), entry("bad\tname", 0), entry("", 0), entry("x\ny", 0)];
        assert_eq!(to_text(&e), "foot\t1\norg.gnome.Nautilus\t0\n");
        assert_eq!(parse(&to_text(&e)), vec![entry("foot", 1), entry("org.gnome.Nautilus", 0)]);
        // the older format has no workspace; nonsense and huge numbers are tamed
        assert_eq!(parse("foot\n\n  org.gnome.Nautilus \nfoot\n"), vec![entry("foot", 0), entry("org.gnome.Nautilus", 0)]);
        assert_eq!(parse("a\tzz\nb\t99999\n"), vec![entry("a", 0), entry("b", 8)]);
        assert!(parse("").is_empty());
    }

    #[test]
    fn only_installed_programs_that_are_not_running_come_back() {
        let apps = vec![app("foot", "foot"), app("org.gnome.Nautilus", "nautilus --new-window")];
        let saved = vec![entry("foot", 0), entry("org.gnome.Nautilus", 2), entry("ghost", 0)];
        assert_eq!(commands(&saved, &apps, &[]), vec!["foot", "nautilus --new-window"]);
        assert_eq!(commands(&saved, &apps, &["foot".to_string()]), vec!["nautilus --new-window"]);
        assert!(commands(&[], &apps, &[]).is_empty());
    }
}
