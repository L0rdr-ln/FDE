//! The programs that make the session a desktop: started by the compositor once its socket exists.

use std::process::Command;

/// Start a task bar (`fde-panel`, installed next to the compositor) for the monitor called
/// `output`. `FDE_NO_PANEL=1` or `shell.panel = false` skips it.
pub fn start_panel(config: &crate::config::Config, output: &str) -> Option<std::process::Child> {
    if std::env::var_os("FDE_NO_PANEL").is_some() || !config.shell.panel {
        return None;
    }
    let panel = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.join("fde-panel")))
        .filter(|p| p.exists());
    match panel {
        Some(p) => match Command::new(&p).env("FDE_PANEL_OUTPUT", output).spawn() {
            Ok(child) => Some(child),
            Err(e) => {
                tracing::warn!("cannot start {}: {e}", p.display());
                None
            }
        },
        None => {
            tracing::info!("no fde-panel next to the compositor: starting without a task bar");
            None
        }
    }
}

/// Where the common polkit authentication agents live.
pub const POLKIT_AGENTS: &[&str] = &[
    "/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1",
    "/usr/libexec/polkit-gnome-authentication-agent-1",
    "/usr/lib/x86_64-linux-gnu/libexec/polkit-kde-authentication-agent-1",
    "/usr/libexec/polkit-kde-authentication-agent-1",
    "/usr/lib/polkit-kde-authentication-agent-1",
    "/usr/libexec/polkit-mate-authentication-agent-1",
    "/usr/bin/lxqt-policykit-agent",
    "/usr/bin/lxpolkit",
];

/// The first of `candidates` that exists.
pub fn find_polkit_agent(candidates: &[&str]) -> Option<std::path::PathBuf> {
    candidates.iter().map(std::path::PathBuf::from).find(|p| p.exists())
}

/// Tell the programs of the session what desktop this is, and tell the session bus (so that
/// services it starts, like the screen-sharing portal, find our display).
pub fn export_environment() {
    std::env::set_var("XDG_CURRENT_DESKTOP", "FDE");
    std::env::set_var("XDG_SESSION_DESKTOP", "fde");
    std::env::set_var("XDG_SESSION_TYPE", "wayland");
    if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
        return;
    }
    // in the background: this can wait a long time for a systemd user session that is not there
    let script = "dbus-update-activation-environment --systemd WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_DESKTOP XDG_SESSION_TYPE \
        || dbus-update-activation-environment WAYLAND_DISPLAY DISPLAY XDG_CURRENT_DESKTOP XDG_SESSION_DESKTOP XDG_SESSION_TYPE";
    if let Err(e) = Command::new("timeout").args(["10", "sh", "-c", script]).spawn() {
        tracing::debug!("dbus-update-activation-environment: {e}");
    }
}

/// The rest of the session: the notification daemon and the autostart programs.
/// `FDE_NO_NOTIFY=1` skips the daemon; `FDE_AUTOSTART` may name one more program.
pub fn start_other_clients(config: &crate::config::Config) {
    export_environment();
    crate::sounds::play(crate::sounds::Event::Startup);
    // the theme's colours for GTK and Qt programs (they read them when they start)
    apply_app_style_in_background(config.clone());
    if config.shell.polkit_agent && std::env::var_os("FDE_NO_POLKIT").is_none() {
        match find_polkit_agent(POLKIT_AGENTS) {
            Some(agent) => {
                if let Err(e) = Command::new(&agent).spawn() {
                    tracing::warn!("cannot start {}: {e}", agent.display());
                }
            }
            None => tracing::info!("no polkit agent installed: programs that need administrator rights cannot ask for a password"),
        }
    }
    // the notification daemon needs the session bus
    if std::env::var_os("FDE_NO_NOTIFY").is_none() && std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
        start_sibling("fde-notify");
    }
    // the system tray (a StatusNotifierWatcher on the session bus)
    if std::env::var_os("FDE_NO_TRAY").is_none() && std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some() {
        start_sibling("fde-tray");
    }
    // background music, if switched on
    if config.music.enabled && std::env::var_os("FDE_NO_MUSIC").is_none() {
        start_sibling("fde-music");
    }
    if config.session.restore && std::env::var_os("FDE_NO_RESTORE").is_none() {
        crate::session::restore();
    }
    let extra = std::env::var("FDE_AUTOSTART").ok();
    for cmd in config.shell.autostart.iter().chain(extra.iter()) {
        if let Err(e) = Command::new("sh").arg("-c").arg(cmd).spawn() {
            tracing::warn!("cannot run autostart command {cmd}: {e}");
        }
    }
}

/// Start a program that lives next to the compositor (`fde-lock`).
pub fn start_sibling(name: &str) {
    start_sibling_with(name, &[]);
}

/// The same with arguments (`fde-shot --output DP-1`).
pub fn start_sibling_with(name: &str, args: &[&str]) {
    let path = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|d| d.join(name)));
    match path {
        Some(p) if p.exists() => {
            if let Err(e) = Command::new(&p).args(args).spawn() {
                tracing::warn!("cannot start {}: {e}", p.display());
            }
        }
        _ => tracing::warn!("{name} is not installed next to the compositor"),
    }
}

static TERMINATE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

extern "C" fn on_terminate(_: libc::c_int) {
    TERMINATE.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// End the session cleanly when the compositor is asked to terminate (the "Log out" button, a
/// `kill`, the console closing): the main loops look at `termination_requested` every frame, stop,
/// and give the graphics session back. Also tells the programs it starts which process to ask
/// (`FDE_PID`).
pub fn watch_signals() {
    std::env::set_var("FDE_PID", std::process::id().to_string());
    unsafe {
        for sig in [libc::SIGTERM, libc::SIGINT, libc::SIGHUP] {
            libc::signal(sig, on_terminate as extern "C" fn(libc::c_int) as libc::sighandler_t);
        }
    }
    crate::kblayout::watch_signal();
}

/// Has the compositor been asked to terminate?
pub fn termination_requested() -> bool {
    TERMINATE.load(std::sync::atomic::Ordering::SeqCst)
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_first_installed_polkit_agent_is_found() {
        let dir = tempfile::tempdir().unwrap();
        let second = dir.path().join("second");
        std::fs::write(&second, "").unwrap();
        let (a, b) = (dir.path().join("missing"), second.clone());
        assert_eq!(super::find_polkit_agent(&[a.to_str().unwrap(), b.to_str().unwrap()]), Some(second));
        assert_eq!(super::find_polkit_agent(&[dir.path().join("none").to_str().unwrap()]), None);
    }

    #[test]
    fn a_terminate_signal_asks_the_session_to_end_instead_of_killing_it() {
        super::watch_signals();
        assert!(!super::termination_requested());
        unsafe { libc::raise(libc::SIGTERM) };
        assert!(super::termination_requested(), "the handler noted the request");
        assert_eq!(std::env::var("FDE_PID").unwrap(), std::process::id().to_string());
    }
}

/// `FDE_GREETER=1`: this compositor is the login screen's (started by greetd), not a session.
pub fn greeter_mode() -> bool {
    std::env::var_os("FDE_GREETER").is_some()
}

/// Start `fde-greeter` and end the compositor (SIGTERM to ourselves, which `watch_signals` turns
/// into a clean exit) when it is done: greetd then starts the user's session.
pub fn start_greeter() {
    let path = std::env::current_exe().ok().and_then(|exe| exe.parent().map(|d| d.join("fde-greeter")));
    match path.filter(|p| p.exists()).map(|p| Command::new(p).spawn()) {
        Some(Ok(mut child)) => {
            std::thread::spawn(move || {
                let _ = child.wait();
                unsafe { libc::kill(libc::getpid(), libc::SIGTERM) };
            });
        }
        Some(Err(e)) => tracing::error!("cannot start fde-greeter: {e}"),
        None => tracing::error!("fde-greeter is not installed next to the compositor"),
    }
}

/// Write the theme's colours for GTK and Qt programs (`appearance.style_apps`), without holding up
/// the compositor.
pub fn apply_app_style_in_background(config: crate::config::Config) {
    if !config.appearance.style_apps || greeter_mode() {
        return;
    }
    std::thread::spawn(move || {
        let Some(home) = crate::appstyle::config_home() else { return };
        for o in crate::appstyle::apply(&config, &home) {
            match &o.what {
                crate::appstyle::What::Wrote => tracing::info!("styled {}", o.path.display()),
                crate::appstyle::What::Skipped(why) => tracing::debug!("not styling {}: {why}", o.path.display()),
                crate::appstyle::What::Unchanged => {}
            }
        }
    });
}
