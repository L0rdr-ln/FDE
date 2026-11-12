//! Who may use the protocols that see or control other programs: screen capture, the window list
//! (and closing or minimizing other windows), and locking the screen.
//!
//! A program gets them when it is one of FDE's own (it lives next to the compositor) or when it is
//! named in `[security] trusted_programs` and installed in a system directory (/usr, /bin, /opt ...,
//! owned by root and writable by nobody else). Everything else (an ordinary app, a sandboxed Flatpak, a script in
//! your home directory) does not even see those protocols. `trusted_programs = []` and
//! `restrict = false` are the two ways to change that.

use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use smithay::reexports::wayland_server::Client;

use crate::state::ClientState;

/// Programs from outside FDE that may capture the screen, list windows or lock the screen.
pub const DEFAULT_TRUSTED: &[&str] = &[
    "grim",
    "wf-recorder",
    "wl-screenrec",
    "xdg-desktop-portal-wlr",
    "swaylock",
    "waylock",
    "obs",
    "waybar",
    "nwg-panel",
    // launchers, menus and notification programs that use overlays or take the keyboard
    "fuzzel",
    "wofi",
    "rofi",
    "tofi",
    "bemenu",
    "bemenu-run",
    "nwg-drawer",
    "wlogout",
    "mako",
    "dunst",
    "swaync",
];

/// The program on the other end of a client connection (from the kernel: SO_PEERCRED).
pub fn peer_executable(stream: &std::os::unix::net::UnixStream) -> Option<PathBuf> {
    use std::os::fd::AsRawFd;
    let mut cred = libc::ucred { pid: 0, uid: 0, gid: 0 };
    let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
    let r = unsafe { libc::getsockopt(stream.as_raw_fd(), libc::SOL_SOCKET, libc::SO_PEERCRED, &mut cred as *mut _ as *mut libc::c_void, &mut len) };
    if r != 0 || cred.pid <= 0 {
        return None;
    }
    std::fs::read_link(format!("/proc/{}/exe", cred.pid)).ok()
}

/// Where system packages put programs.
const SYSTEM_DIRS: &[&str] = &["/usr/", "/bin/", "/sbin/", "/opt/", "/nix/store/"];

/// In a system folder, and neither the file nor its directory can be changed by anyone but root.
fn installed_by_root(path: &Path) -> bool {
    let in_system = SYSTEM_DIRS.iter().any(|d| path.starts_with(d));
    let safe = |p: &Path| std::fs::metadata(p).map(|m| m.uid() == 0 && m.mode() & 0o022 == 0).unwrap_or(false);
    in_system && safe(path) && path.parent().map(safe).unwrap_or(false)
}

/// May the program at `exe` use the protocols that see or control other programs?
/// `own_dir` is where the compositor itself is installed.
pub fn is_trusted(exe: &Path, own_dir: Option<&Path>, trusted: &[String]) -> bool {
    let exe = std::fs::canonicalize(exe).unwrap_or_else(|_| exe.to_path_buf());
    // FDE's own programs, next to the compositor
    if let (Some(dir), Some(parent)) = (own_dir, exe.parent()) {
        let dir = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        let ours = ["fde-panel", "fde-lock", "fde-notify", "fde-shot", "fde-settings", "fde-music"];
        let name = exe.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if parent == dir && ours.contains(&name) {
            return true;
        }
    }
    // named in the configuration and installed where only root can put it
    let name = exe.file_name().and_then(|n| n.to_str()).unwrap_or("");
    trusted.iter().any(|t| t == name) && installed_by_root(&exe)
}

/// The data a new client connection starts with.
pub fn client_state_for(stream: &std::os::unix::net::UnixStream, config: &crate::config::Config) -> ClientState {
    let privileged = if !config.security.restrict {
        true
    } else {
        let own_dir = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf));
        match peer_executable(stream) {
            Some(exe) => {
                let ok = is_trusted(&exe, own_dir.as_deref(), &config.security.trusted_programs);
                tracing::debug!("client {}: {}", exe.display(), if ok { "trusted" } else { "ordinary" });
                ok
            }
            None => false,
        }
    };
    ClientState { privileged, ..Default::default() }
}

/// May this client see the protocols that see or control other programs?
pub fn privileged(client: &Client) -> bool {
    client.get_data::<ClientState>().map(|d| d.privileged).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fde_programs_next_to_the_compositor_and_root_installed_named_programs_are_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let panel = dir.path().join("fde-panel");
        std::fs::write(&panel, "").unwrap();
        let stranger = dir.path().join("evil");
        std::fs::write(&stranger, "").unwrap();
        assert!(is_trusted(&panel, Some(dir.path()), &[]), "our own task bar");
        assert!(!is_trusted(&stranger, Some(dir.path()), &[]), "another program in the same folder is not ours");
        assert!(!is_trusted(&panel, Some(Path::new("/nonexistent")), &[]), "an fde-panel elsewhere is not ours");
        // a named program counts only where only root can put it
        let named = vec!["env".to_string(), "grim".to_string()];
        if let Ok(env) = std::fs::canonicalize("/usr/bin/env") {
            if installed_by_root(&env) {
                assert!(is_trusted(&env, None, &named));
            }
        }
        let fake = dir.path().join("grim");
        std::fs::write(&fake, "").unwrap();
        assert!(!is_trusted(&fake, None, &named), "a grim in a folder the user can write to is not trusted");
    }
}
